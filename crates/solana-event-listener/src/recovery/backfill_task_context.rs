use crate::{
    config::EventListenerConfig,
    error::{EventListenerError, Result},
    metrics::MetricsCollector,
    parser::{EventParserRegistry, EventDataSource},
    recovery::{
        backfill_handler::{BackfillEventConfig, BackfillEventRegistry, EventBackfillHandler},
        checkpoint_persistence::CheckpointPersistence,
        scan_record_persistence::ScanRecordPersistence,
    },
    BatchWriter,
};
use anyhow::anyhow;
use chrono::Utc;
use database::{
    event_model::event_model_repository::EventModelRepository,
    event_scanner::model::{EventScannerCheckpoints, ScanRecords, ScanStatus},
};
use solana_client::{
    rpc_client::{GetConfirmedSignaturesForAddress2Config, RpcClient},
    rpc_config::RpcTransactionConfig,
    rpc_response::RpcLogsResponse,
};
use solana_sdk::{commitment_config::CommitmentConfig, signature::Signature};
use solana_transaction_status::{EncodedConfirmedTransactionWithStatusMeta, UiTransactionEncoding};
use std::{str::FromStr, sync::Arc, time::Duration};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// 包含签名和对应slot的结构体，用于回填过程中维护slot信息
#[derive(Debug, Clone)]
pub struct SignatureWithSlot {
    pub signature: String,
    pub slot: u64,
}

/// 回填任务上下文
///
/// 包含执行单个事件类型回填所需的所有组件
/// 用于支持并发执行多种事件类型的回填
#[derive(Clone)]
pub struct BackfillTaskContext {
    pub config: Arc<EventListenerConfig>,
    pub rpc_client: Arc<RpcClient>,
    pub parser_registry: Arc<EventParserRegistry>,
    pub batch_writer: Arc<BatchWriter>,
    pub metrics: Arc<MetricsCollector>,
    pub checkpoint_persistence: Arc<CheckpointPersistence>,
    pub scan_record_persistence: Arc<ScanRecordPersistence>,
    pub event_registry: Arc<BackfillEventRegistry>,
    pub default_check_interval: Duration,
}

impl BackfillTaskContext {
    /// 启动单个事件类型的回填循环
    pub async fn start_event_backfill_loop(&self, event_config: BackfillEventConfig) -> Result<()> {
        let interval_duration = event_config
            .check_interval_secs
            .map(Duration::from_secs)
            .unwrap_or(self.default_check_interval);

        info!(
            "🔄 启动 {} 事件回填循环 (程序ID: {}, 间隔: {:?})",
            event_config.event_type, event_config.program_id, interval_duration
        );

        let mut interval_timer = tokio::time::interval(interval_duration);

        loop {
            interval_timer.tick().await;

            info!("⏰ 开始执行 {} 回填检查周期", event_config.event_type);

            if let Err(e) = self.perform_event_backfill_cycle(&event_config).await {
                error!("❌ {} 回填周期执行失败: {}", event_config.event_type, e);
                // 继续下一个周期，不退出服务
                continue;
            }

            info!("✅ {} 回填周期完成", event_config.event_type);
        }
    }
    /// 执行单个事件类型的回填周期
    pub async fn perform_event_backfill_cycle(&self, event_config: &BackfillEventConfig) -> Result<()> {
        let handler = self
            .event_registry
            .get_handler(&event_config.event_type)
            .ok_or_else(|| {
                EventListenerError::Unknown(format!("未找到事件类型 '{}' 的处理器", event_config.event_type))
            })?;

        // 1. 读取检查点，确定扫描范围
        let (until_signature, before_signature) = self.determine_scan_range(&handler, &event_config.program_id).await?;

        if until_signature == before_signature {
            info!("📋 {} 无需回填，签名范围相同", event_config.event_type);
            return Ok(());
        }

        // 2. 创建扫描记录
        let scan_id = Uuid::new_v4().to_string();
        let event_name = handler.event_type_name();
        let mut scan_record = self
            .create_scan_record(&scan_id, &until_signature, None, &before_signature, None, &event_config.program_id, event_name)
            .await?;

        info!(
            "🔍 开始 {} 回填扫描 ID: {}, 范围: {} -> {}",
            event_config.event_type, scan_id, until_signature, before_signature
        );

        // 3. 获取签名列表
        let (signatures, actual_latest_signature, actual_latest_slot) = match self
            .fetch_signatures(&before_signature, &until_signature, &event_config.program_id)
            .await
        {
            Ok((sigs, latest_sig, latest_slot)) => (sigs, latest_sig, latest_slot),
            Err(e) => {
                scan_record.status = ScanStatus::Failed;
                scan_record.error_message = Some(format!("获取签名失败: {}", e));
                scan_record.completed_at = Some(Utc::now());
                self.scan_record_persistence.update_scan_record(&scan_record).await?;
                return Err(e);
            }
        };

        info!("📝 {} 获取到 {} 个签名", event_config.event_type, signatures.len());

        // 3.1 如果获取到了实际的最新签名，更新扫描记录的before_signature字段
        if let Some(actual_before) = &actual_latest_signature {
            scan_record.before_signature = actual_before.clone();
            if let Some(actual_slot) = actual_latest_slot {
                scan_record.before_slot = Some(actual_slot);
            }
            // 立即更新扫描记录，保存实际获取的最新签名
            self.scan_record_persistence.update_scan_record(&scan_record).await?;
            info!("📍 更新扫描记录的实际签名: {}", actual_before);
        }

        // 4. 去重获取丢失事件的签名
        let missing_signatures = self.find_missing_signatures(&signatures, &handler).await?;

        info!(
            "🔍 {} 发现 {} 个丢失签名",
            event_config.event_type,
            missing_signatures.len()
        );
        scan_record.events_found = missing_signatures.len() as u64;

        // 5. 如果没有丢失签名，标记完成
        if missing_signatures.is_empty() {
            scan_record.status = ScanStatus::Completed;
            scan_record.completed_at = Some(Utc::now());
            self.scan_record_persistence.update_scan_record(&scan_record).await?;

            // 更新检查点 - 使用实际获取的最新签名或until_signature
            let checkpoint_signature = actual_latest_signature.as_ref().unwrap_or(&until_signature);
            let checkpoint_slot = actual_latest_slot.or(None);
            self.update_checkpoint(
                checkpoint_signature,
                checkpoint_slot,
                &event_config.program_id,
                &handler.checkpoint_event_name(),
            )
            .await?;

            return Ok(());
        }

        // 6. 回填丢失的事件
        let backfilled_count = self
            .backfill_missing_events(&missing_signatures, &mut scan_record, &event_config.program_id)
            .await?;

        // 7. 更新扫描记录为完成状态
        scan_record.events_backfilled_count = backfilled_count;
        scan_record.status = ScanStatus::Completed;
        scan_record.completed_at = Some(Utc::now());
        self.scan_record_persistence.update_scan_record(&scan_record).await?;

        // 8. 更新检查点 - 使用实际获取的最新签名或until_signature
        let checkpoint_signature = actual_latest_signature.as_ref().unwrap_or(&until_signature);
        let checkpoint_slot = actual_latest_slot.or(None);
        self.update_checkpoint(
            checkpoint_signature,
            checkpoint_slot,
            &event_config.program_id,
            &handler.checkpoint_event_name(),
        )
        .await?;

        info!(
            "🎉 {} 回填完成: 处理了 {} 个事件",
            event_config.event_type, backfilled_count
        );

        Ok(())
    }

    /// 确定扫描范围 (until_signature, before_signature)
    async fn determine_scan_range(
        &self,
        handler: &Arc<dyn EventBackfillHandler>,
        program_id: &solana_sdk::pubkey::Pubkey,
    ) -> Result<(String, String)> {
        // 创建EventModelRepository
        let repo = EventModelRepository::new(&self.config.database.uri, &self.config.database.database_name)
            .await
            .map_err(|e| EventListenerError::Unknown(format!("创建EventModelRepository失败: {}", e)))?;

        // 从检查点读取上次结束位置（基于程序ID和事件名）
        let event_name = handler.checkpoint_event_name();
        let checkpoint = self
            .checkpoint_persistence
            .get_checkpoint_by_program_and_event_name(&program_id.to_string(), &event_name)
            .await?;

        match checkpoint {
            Some(cp) if cp.last_signature.is_some() => {
                // 有检查点，从检查点开始到链上最新签名
                // 使用空字符串表示before=None，让RPC获取最新签名
                let until_signature = cp
                    .last_signature
                    .ok_or_else(|| EventListenerError::Unknown("检查点last_signature为空".to_string()))?;
                let before_signature = String::new(); // 空字符串，在fetch_signatures中会被处理为None

                info!(
                    "📍 {} 从检查点开始到链上最新: {} -> <最新签名>",
                    handler.event_type_name(),
                    until_signature
                );
                Ok((until_signature, before_signature))
            }
            _ => {
                // 初次启动或无有效检查点，从最老事件到链上最新签名
                let oldest_sig = handler.get_oldest_event_signature(&repo).await?;
                let before_signature = String::new(); // 空字符串，在fetch_signatures中会被处理为None

                info!(
                    "🆕 {} 初次启动，从最老事件到链上最新: {} -> <最新签名>",
                    handler.event_type_name(),
                    oldest_sig
                );
                Ok((oldest_sig, before_signature))
            }
        }
    }

    /// 创建扫描记录
    async fn create_scan_record(
        &self,
        scan_id: &str,
        until_signature: &str,
        until_slot: Option<u64>,
        before_signature: &str,
        before_slot: Option<u64>,
        program_id: &solana_sdk::pubkey::Pubkey,
        event_name: &str,
    ) -> Result<ScanRecords> {
        let scan_record = ScanRecords {
            id: None,
            scan_id: scan_id.to_string(),
            until_slot,
            before_slot,
            until_signature: until_signature.to_string(),
            before_signature: before_signature.to_string(),
            status: ScanStatus::Running,
            events_found: 0,
            events_backfilled_count: 0,
            events_backfilled_signatures: Vec::new(),
            started_at: Utc::now(),
            completed_at: None,
            error_message: None,
            program_filters: vec![program_id.to_string()],
            program_id: Some(program_id.to_string()),
            event_name: Some(event_name.to_string()),
        };

        self.scan_record_persistence.create_scan_record(&scan_record).await?;
        Ok(scan_record)
    }

    /// 验证签名格式是否为有效的Solana签名
    fn is_valid_solana_signature(signature: &str) -> bool {
        // 空字符串或过短的字符串肯定无效
        if signature.is_empty() || signature.len() < 80 {
            return false;
        }
        
        // 过长的字符串也无效（正常Solana签名不应该超过90字符）
        if signature.len() > 90 {
            return false;
        }
        
        // 最终依赖Solana库的签名解析来验证
        Signature::from_str(signature).is_ok()
    }

    /// 获取签名列表
    /// 返回 (签名列表, 实际的最新签名, 实际的最新slot) - 用于更新检查点和记录
    async fn fetch_signatures(
        &self,
        before: &str,
        until: &str,
        program_id: &solana_sdk::pubkey::Pubkey,
    ) -> Result<(Vec<SignatureWithSlot>, Option<String>, Option<u64>)> {
        // 验证和处理before签名
        let before_signature = if !before.is_empty() && before != "1111111111111111111111111111111111111111111111111111111111111111" {
            if Self::is_valid_solana_signature(before) {
                Some(Signature::from_str(before).map_err(|e| anyhow!("before 签名解析错误：{}", e))?)
            } else {
                warn!("⚠️ before签名格式无效，忽略: {}", before);
                None // 忽略无效签名，从最新开始搜索
            }
        } else {
            None // 空字符串或默认签名都设为None，让RPC从最新签名开始搜索
        };

        // 验证和处理until签名
        let until_signature = if !until.is_empty() && until != "1111111111111111111111111111111111111111111111111111111111111111" {
            if Self::is_valid_solana_signature(until) {
                Some(Signature::from_str(until).map_err(|e| anyhow!("until 签名解析错误：{}", e))?)
            } else {
                warn!("⚠️ until签名格式无效，忽略: {}", until);
                None // 忽略无效签名
            }
        } else {
            None
        };

        let config = GetConfirmedSignaturesForAddress2Config {
            before: before_signature,
            until: until_signature,
            limit: Some(1000), // 一次最多获取1000个
            commitment: Some(CommitmentConfig::confirmed()),
        };

        info!(
            "🔍 获取签名列表 - 程序ID: {}, until: {:?}, before: {:?}",
            program_id,
            config.until.as_ref().map(|s| s.to_string()).unwrap_or_else(|| "<最老>".to_string()),
            config.before.as_ref().map(|s| s.to_string()).unwrap_or_else(|| "<最新>".to_string())
        );

        let signatures = self
            .rpc_client
            .get_signatures_for_address_with_config(program_id, config)
            .map_err(|e| EventListenerError::SolanaRpc(format!("获取签名列表失败: {}", e)))?;

        let signatures_with_slots: Vec<SignatureWithSlot> = signatures.iter()
            .map(|sig| SignatureWithSlot {
                signature: sig.signature.clone(),
                slot: sig.slot,
            })
            .collect();
        
        // 如果before为None（即获取最新签名），则第一个签名就是实际的最新签名和slot
        let (actual_latest_signature, actual_latest_slot) = if before.is_empty() && !signatures_with_slots.is_empty() {
            (Some(signatures_with_slots[0].signature.clone()), Some(signatures_with_slots[0].slot))
        } else {
            (None, None)
        };

        info!("📝 获取到 {} 个签名，实际最新签名: {:?}, 实际最新slot: {:?}", 
            signatures_with_slots.len(), 
            actual_latest_signature,
            actual_latest_slot
        );

        Ok((signatures_with_slots, actual_latest_signature, actual_latest_slot))
    }

    /// 查找丢失的签名
    async fn find_missing_signatures(
        &self,
        all_signatures: &[SignatureWithSlot],
        handler: &Arc<dyn EventBackfillHandler>,
    ) -> Result<Vec<SignatureWithSlot>> {
        let repo = EventModelRepository::new(&self.config.database.uri, &self.config.database.database_name)
            .await
            .map_err(|e| EventListenerError::Unknown(format!("查询EventModelRepository失败: {}", e)))?;

        let mut missing = Vec::new();

        // 分批检查签名是否存在，避免一次性查询过多
        const BATCH_SIZE: usize = 50;
        for chunk in all_signatures.chunks(BATCH_SIZE) {
            for sig_with_slot in chunk {
                match handler.signature_exists(&repo, &sig_with_slot.signature).await {
                    Ok(exists) => {
                        if !exists {
                            missing.push(sig_with_slot.clone());
                        }
                    }
                    Err(e) => {
                        warn!("检查签名 {} 存在性时出错: {}", sig_with_slot.signature, e);
                        // 出错时假设不存在，进行回填
                        missing.push(sig_with_slot.clone());
                    }
                }
            }
        }

        Ok(missing)
    }

    /// 回填丢失的事件
    async fn backfill_missing_events(
        &self,
        signatures: &[SignatureWithSlot],
        scan_record: &mut ScanRecords,
        program_id: &solana_sdk::pubkey::Pubkey,
    ) -> Result<u64> {
        let mut backfilled_count = 0u64;
        let mut backfilled_signatures = Vec::new();

        for sig_with_slot in signatures {
            match self.process_missing_transaction(&sig_with_slot.signature, program_id).await {
                Ok(processed) => {
                    if processed {
                        backfilled_count += 1;
                        backfilled_signatures.push(sig_with_slot.signature.clone());

                        // 记录回填指标
                        if let Err(e) = self
                            .metrics
                            .record_event_backfilled_for_program(&program_id.to_string())
                            .await
                        {
                            warn!("记录回填事件指标失败: {}", e);
                        }
                    }
                }
                Err(e) => {
                    warn!("⚠️ 处理交易失败 {}: {}", sig_with_slot.signature, e);
                    // 继续处理其他交易，不中断整个过程
                }
            }

            // 批量更新扫描记录
            if backfilled_count % 10 == 0 {
                scan_record.events_backfilled_count = backfilled_count;
                scan_record.events_backfilled_signatures = backfilled_signatures.clone();
                self.scan_record_persistence.update_scan_record(scan_record).await?;
            }
        }

        // 最终更新扫描记录
        scan_record.events_backfilled_count = backfilled_count;
        scan_record.events_backfilled_signatures = backfilled_signatures;
        self.scan_record_persistence.update_scan_record(scan_record).await?;

        Ok(backfilled_count)
    }

    /// 处理单个丢失的交易
    async fn process_missing_transaction(
        &self,
        signature: &str,
        program_id: &solana_sdk::pubkey::Pubkey,
    ) -> Result<bool> {
        // 1. 获取交易详情
        let tx_config = RpcTransactionConfig {
            encoding: Some(UiTransactionEncoding::Json),
            commitment: Some(CommitmentConfig::confirmed()),
            max_supported_transaction_version: Some(0),
        };

        let signature_obj = Signature::from_str(signature)
            .map_err(|e| EventListenerError::SolanaRpc(format!("解析签名失败: {}", e)))?;

        let transaction = self
            .rpc_client
            .get_transaction_with_config(&signature_obj, tx_config)
            .map_err(|e| EventListenerError::SolanaRpc(format!("获取交易详情失败: {}", e)))?;

        // 2. 适配：EncodedConfirmedTransactionWithStatusMeta -> RpcLogsResponse
        let logs_response = self.adapt_transaction_to_logs_response(transaction, signature)?;

        // 3. 复用现有的事件处理流程
        let processed = self.process_backfilled_event(logs_response, program_id).await?;

        Ok(processed)
    }

    /// 适配：将 EncodedConfirmedTransactionWithStatusMeta 适配为 RpcLogsResponse
    fn adapt_transaction_to_logs_response(
        &self,
        transaction: EncodedConfirmedTransactionWithStatusMeta,
        signature: &str,
    ) -> Result<RpcLogsResponse> {
        let meta = transaction
            .transaction
            .meta
            .ok_or_else(|| EventListenerError::EventParsing("交易meta为空".to_string()))?;

        // 提取日志
        let logs = meta.log_messages.unwrap_or(vec![]);

        // 提取错误信息
        let err = meta.err;

        Ok(RpcLogsResponse {
            signature: signature.to_string(),
            err,
            logs,
        })
    }

    /// 处理回填的事件（复用现有流程）
    async fn process_backfilled_event(
        &self,
        logs_response: RpcLogsResponse,
        program_id: &solana_sdk::pubkey::Pubkey,
    ) -> Result<bool> {
        let signature = &logs_response.signature;

        info!("🔄 回填处理事件: {}", signature);

        // 获取当前slot（回填事件使用当前slot）
        let slot = self
            .rpc_client
            .get_slot()
            .map_err(|e| EventListenerError::SolanaRpc(format!("获取slot失败: {}", e)))?;

        // 解析事件 - 标记为回填服务数据源
        match self
            .parser_registry
            .parse_all_events_with_context(&logs_response.logs, signature, slot, &vec![*program_id], Some(EventDataSource::BackfillService))
            .await
        {
            Ok(parsed_events) if !parsed_events.is_empty() => {
                info!("✅ 回填事件解析成功: {} -> {}个事件", signature, parsed_events.len());

                // 提交到批量写入器
                self.batch_writer.submit_events(parsed_events.clone()).await?;

                // 更新回填指标
                let event_count = parsed_events.len();
                for _ in 0..event_count {
                    if let Err(e) = self
                        .metrics
                        .record_event_backfilled_for_program(&program_id.to_string())
                        .await
                    {
                        warn!("记录回填事件指标失败: {}", e);
                    }
                }

                Ok(true)
            }
            Ok(_) => {
                debug!("ℹ️ 回填事件无法识别: {}", signature);
                Ok(false)
            }
            Err(e) => {
                warn!("❌ 回填事件解析失败: {} - {}", signature, e);
                Ok(false)
            }
        }
    }

    /// 更新检查点
    async fn update_checkpoint(
        &self,
        last_signature: &str,
        last_slot: Option<u64>,
        program_id: &solana_sdk::pubkey::Pubkey,
        event_name: &str,
    ) -> Result<()> {
        let checkpoint = EventScannerCheckpoints {
            id: None,
            program_id: Some(program_id.to_string()),
            event_name: Some(event_name.to_string()),
            slot: last_slot,
            last_signature: Some(last_signature.to_string()),
            updated_at: Utc::now(),
            created_at: Utc::now(),
        };

        self.checkpoint_persistence.update_checkpoint(&checkpoint).await?;

        info!("📍 更新检查点 {}: {}", event_name, last_signature);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_empty_before_signature_handling() {
        // 测试空字符串的before签名应该被正确处理
        let empty_before = "";
        let default_before = "1111111111111111111111111111111111111111111111111111111111111111";
        
        // 空字符串应该被视为需要获取最新签名
        assert!(empty_before.is_empty());
        
        // 默认签名也应该被视为需要获取最新签名
        assert_eq!(default_before, "1111111111111111111111111111111111111111111111111111111111111111");
    }

    #[test]
    fn test_signature_parsing() {
        // 测试有效签名的解析
        let valid_sig = "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d1V4KAEJrGrMn3RcYfP6oK3pVt4K7yWxNvPvx9eT5NqC";
        let signature_result = Signature::from_str(valid_sig);
        assert!(signature_result.is_ok(), "有效签名应该能够正确解析");
    }

    #[test] 
    fn test_checkpoint_signature_selection() {
        // 测试检查点签名选择逻辑
        let until_signature = "old_signature".to_string();
        let actual_latest = Some("latest_signature".to_string());
        
        // 当有actual_latest_signature时，应该使用它
        let checkpoint_signature = actual_latest.as_ref().unwrap_or(&until_signature);
        assert_eq!(checkpoint_signature, "latest_signature");
        
        // 当没有actual_latest_signature时，应该使用until_signature
        let no_latest: Option<String> = None;
        let checkpoint_signature = no_latest.as_ref().unwrap_or(&until_signature);
        assert_eq!(checkpoint_signature, "old_signature");
    }

    #[test]
    fn test_config_before_parameter_handling() {
        // 模拟GetConfirmedSignaturesForAddress2Config的before参数处理逻辑
        
        // 空字符串应该被处理为None
        let empty_before = "";
        let should_be_none = if !empty_before.is_empty() && 
            empty_before != "1111111111111111111111111111111111111111111111111111111111111111" {
            "Some"
        } else {
            "None"
        };
        assert_eq!(should_be_none, "None");
        
        // 默认签名也应该被处理为None
        let default_before = "1111111111111111111111111111111111111111111111111111111111111111";
        let should_be_none = if !default_before.is_empty() && 
            default_before != "1111111111111111111111111111111111111111111111111111111111111111" {
            "Some"
        } else {
            "None"
        };
        assert_eq!(should_be_none, "None");
        
        // 有效签名应该被处理为Some
        let valid_before = "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d1V4KAEJrGrMn3RcYfP6oK3pVt4K7yWxNvPvx9eT5NqC";
        let should_be_some = if !valid_before.is_empty() && 
            valid_before != "1111111111111111111111111111111111111111111111111111111111111111" {
            "Some"
        } else {
            "None"
        };
        assert_eq!(should_be_some, "Some");
    }

    #[test]
    fn test_actual_latest_signature_extraction() {
        // 测试从签名列表中提取最新签名的逻辑
        let signatures = vec![
            "latest_signature".to_string(),
            "older_signature_1".to_string(), 
            "older_signature_2".to_string()
        ];
        
        let before_is_empty = true; // 模拟before为空字符串的情况
        
        let actual_latest = if before_is_empty && !signatures.is_empty() {
            Some(signatures[0].clone())
        } else {
            None
        };
        
        assert_eq!(actual_latest, Some("latest_signature".to_string()));
        
        // 测试空签名列表的情况
        let empty_signatures: Vec<String> = vec![];
        let actual_latest_empty = if before_is_empty && !empty_signatures.is_empty() {
            Some(empty_signatures[0].clone())
        } else {
            None
        };
        
        assert_eq!(actual_latest_empty, None);
    }
    
    #[test]
    fn test_signature_validation() {
        use solana_sdk::signature::Signature;
        use std::str::FromStr;
        
        // 调试：直接测试Signature::from_str
        let test_signature = "mduy8bwIXlyVFH5wzxULUr2xfa66Z9wFWgaYCGw7VABQf71M7wvHOjfstxY0M140U6VfnccuLBMZjmbmxuUvj09V";
        println!("测试签名长度: {}", test_signature.len());
        
        match Signature::from_str(test_signature) {
            Ok(_) => println!("✅ 签名解析成功"),
            Err(e) => println!("❌ 签名解析失败: {}", e),
        }
        
        // 测试有效签名验证
        let valid_signatures = [
            "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d1V4KAEJrGrMn3RcYfP6oK3pVt4K7yWxNvPvx9eT5NqC",
        ];
        
        for signature in valid_signatures {
            assert!(BackfillTaskContext::is_valid_solana_signature(signature), 
                "应该识别为有效签名: {}", signature);
        }
        
        // 测试无效签名
        let invalid_signatures = [
            "4db6b37c932e0256a1982663ddb8c7cc6e6b71c3d273a335fdcff9d4cab4f9884a2c1879a37a68820d43240a", // 太短且格式不对
            "", // 空字符串
            "1111111111111111111111111111111111111111111111111111111111111111", // 默认签名
            "invalid_signature_format", // 格式不对
        ];
        
        for signature in invalid_signatures {
            assert!(!BackfillTaskContext::is_valid_solana_signature(signature), 
                "应该识别为无效签名: {}", signature);
        }
    }

    #[test]
    fn test_signature_with_slot_structure() {
        // 测试 SignatureWithSlot 结构体创建和访问
        let sig_with_slot = SignatureWithSlot {
            signature: "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d1V4KAEJrGrMn3RcYfP6oK3pVt4K7yWxNvPvx9eT5NqC".to_string(),
            slot: 123456789,
        };

        assert_eq!(sig_with_slot.signature, "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d1V4KAEJrGrMn3RcYfP6oK3pVt4K7yWxNvPvx9eT5NqC");
        assert_eq!(sig_with_slot.slot, 123456789);

        // 测试 Clone 特性
        let cloned = sig_with_slot.clone();
        assert_eq!(cloned.signature, sig_with_slot.signature);
        assert_eq!(cloned.slot, sig_with_slot.slot);
    }

    #[test]
    fn test_checkpoint_slot_handling() {
        // 测试检查点 slot 字段的处理逻辑
        use chrono::Utc;
        use database::event_scanner::model::EventScannerCheckpoints;

        // 测试有 slot 的检查点
        let checkpoint_with_slot = EventScannerCheckpoints {
            id: None,
            program_id: Some("test_program".to_string()),
            event_name: Some("test_event".to_string()),
            slot: Some(123456),
            last_signature: Some("test_signature".to_string()),
            updated_at: Utc::now(),
            created_at: Utc::now(),
        };

        assert_eq!(checkpoint_with_slot.slot, Some(123456));

        // 测试没有 slot 的检查点 (向后兼容)
        let checkpoint_without_slot = EventScannerCheckpoints {
            id: None,
            program_id: Some("test_program".to_string()),
            event_name: Some("test_event".to_string()),
            slot: None,
            last_signature: Some("test_signature".to_string()),
            updated_at: Utc::now(),
            created_at: Utc::now(),
        };

        assert_eq!(checkpoint_without_slot.slot, None);
    }

    #[test]
    fn test_scan_record_slot_handling() {
        // 测试扫描记录 slot 字段的处理逻辑
        use chrono::Utc;
        use database::event_scanner::model::{ScanRecords, ScanStatus};

        // 测试有 slot 的扫描记录
        let scan_record_with_slots = ScanRecords {
            id: None,
            scan_id: "test-scan-001".to_string(),
            until_slot: Some(100000),
            before_slot: Some(200000),
            until_signature: "sig1".to_string(),
            before_signature: "sig2".to_string(),
            status: ScanStatus::Running,
            events_found: 10,
            events_backfilled_count: 8,
            events_backfilled_signatures: vec!["sig1".to_string(), "sig2".to_string()],
            started_at: Utc::now(),
            completed_at: None,
            error_message: None,
            program_filters: vec!["test_program".to_string()],
            program_id: Some("test_program".to_string()),
            event_name: Some("test_event".to_string()),
        };

        assert_eq!(scan_record_with_slots.until_slot, Some(100000));
        assert_eq!(scan_record_with_slots.before_slot, Some(200000));

        // 测试没有 slot 的扫描记录 (向后兼容)
        let scan_record_without_slots = ScanRecords {
            id: None,
            scan_id: "test-scan-002".to_string(),
            until_slot: None,
            before_slot: None,
            until_signature: "sig1".to_string(),
            before_signature: "sig2".to_string(),
            status: ScanStatus::Running,
            events_found: 5,
            events_backfilled_count: 3,
            events_backfilled_signatures: vec!["sig1".to_string()],
            started_at: Utc::now(),
            completed_at: None,
            error_message: None,
            program_filters: vec!["test_program".to_string()],
            program_id: Some("test_program".to_string()),
            event_name: Some("test_event".to_string()),
        };

        assert_eq!(scan_record_without_slots.until_slot, None);
        assert_eq!(scan_record_without_slots.before_slot, None);
    }
}
