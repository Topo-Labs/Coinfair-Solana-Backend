use crate::{
    config::EventListenerConfig,
    error::{EventListenerError, Result},
    metrics::MetricsCollector,
    parser::EventParserRegistry,
    persistence::{checkpoint_persistence::CheckpointPersistence, scan_record_persistence::ScanRecordPersistence},
    recovery::CheckpointManager,
    subscriber::backfill_handler::{BackfillEventConfig, BackfillEventRegistry, EventBackfillHandler},
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
    pub checkpoint_manager: Arc<CheckpointManager>,
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
        let mut scan_record = self
            .create_scan_record(&scan_id, &until_signature, &before_signature, &event_config.program_id)
            .await?;

        info!(
            "🔍 开始 {} 回填扫描 ID: {}, 范围: {} -> {}",
            event_config.event_type, scan_id, until_signature, before_signature
        );

        // 3. 获取签名列表
        let signatures = match self
            .fetch_signatures(&before_signature, &until_signature, &event_config.program_id)
            .await
        {
            Ok(sigs) => sigs,
            Err(e) => {
                scan_record.status = ScanStatus::Failed;
                scan_record.error_message = Some(format!("获取签名失败: {}", e));
                scan_record.completed_at = Some(Utc::now());
                self.scan_record_persistence.update_scan_record(&scan_record).await?;
                return Err(e);
            }
        };

        info!("📝 {} 获取到 {} 个签名", event_config.event_type, signatures.len());

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

            // 更新检查点
            self.update_checkpoint(
                &until_signature,
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

        // 8. 更新检查点
        self.update_checkpoint(
            &until_signature,
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
                // 有检查点，从检查点开始到最新事件
                let until_signature = cp
                    .last_signature
                    .ok_or_else(|| EventListenerError::Unknown("检查点last_signature为空".to_string()))?;
                let before_signature = handler.get_latest_event_signature(&repo).await?;

                info!(
                    "📍 {} 从检查点开始: {} -> {}",
                    handler.event_type_name(),
                    until_signature,
                    before_signature
                );
                Ok((until_signature, before_signature))
            }
            _ => {
                // 初次启动或无有效检查点，从事件中读取范围
                let oldest_sig = handler.get_oldest_event_signature(&repo).await?;
                let latest_sig = handler.get_latest_event_signature(&repo).await?;

                info!(
                    "🆕 {} 初次启动，从事件范围: {} -> {}",
                    handler.event_type_name(),
                    oldest_sig,
                    latest_sig
                );
                Ok((oldest_sig, latest_sig))
            }
        }
    }

    /// 创建扫描记录
    async fn create_scan_record(
        &self,
        scan_id: &str,
        until_signature: &str,
        before_signature: &str,
        program_id: &solana_sdk::pubkey::Pubkey,
    ) -> Result<ScanRecords> {
        let scan_record = ScanRecords {
            id: None,
            scan_id: scan_id.to_string(),
            until_slot: None,
            before_slot: None,
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
        };

        self.scan_record_persistence.create_scan_record(&scan_record).await?;
        Ok(scan_record)
    }

    /// 获取签名列表
    async fn fetch_signatures(
        &self,
        before: &str,
        until: &str,
        program_id: &solana_sdk::pubkey::Pubkey,
    ) -> Result<Vec<String>> {
        let config = GetConfirmedSignaturesForAddress2Config {
            before: if before != "1111111111111111111111111111111111111111111111111111111111111111" {
                Some(Signature::from_str(before).map_err(|e| anyhow!("before 签名错误：{}", e))?)
            } else {
                None
            },
            until: if until != "1111111111111111111111111111111111111111111111111111111111111111" {
                Some(Signature::from_str(until).map_err(|e| anyhow!("until 签名错误：{}", e))?)
            } else {
                None
            },
            limit: Some(1000), // 一次最多获取1000个
            commitment: Some(CommitmentConfig::confirmed()),
        };

        let signatures = self
            .rpc_client
            .get_signatures_for_address_with_config(program_id, config)
            .map_err(|e| EventListenerError::SolanaRpc(format!("获取签名列表失败: {}", e)))?;

        Ok(signatures.into_iter().map(|sig| sig.signature).collect())
    }

    /// 查找丢失的签名
    async fn find_missing_signatures(
        &self,
        all_signatures: &[String],
        handler: &Arc<dyn EventBackfillHandler>,
    ) -> Result<Vec<String>> {
        let repo = EventModelRepository::new(&self.config.database.uri, &self.config.database.database_name)
            .await
            .map_err(|e| EventListenerError::Unknown(format!("查询EventModelRepository失败: {}", e)))?;

        let mut missing = Vec::new();

        // 分批检查签名是否存在，避免一次性查询过多
        const BATCH_SIZE: usize = 50;
        for chunk in all_signatures.chunks(BATCH_SIZE) {
            for signature in chunk {
                match handler.signature_exists(&repo, signature).await {
                    Ok(exists) => {
                        if !exists {
                            missing.push(signature.clone());
                        }
                    }
                    Err(e) => {
                        warn!("检查签名 {} 存在性时出错: {}", signature, e);
                        // 出错时假设不存在，进行回填
                        missing.push(signature.clone());
                    }
                }
            }
        }

        Ok(missing)
    }

    /// 回填丢失的事件
    async fn backfill_missing_events(
        &self,
        signatures: &[String],
        scan_record: &mut ScanRecords,
        program_id: &solana_sdk::pubkey::Pubkey,
    ) -> Result<u64> {
        let mut backfilled_count = 0u64;
        let mut backfilled_signatures = Vec::new();

        for signature in signatures {
            match self.process_missing_transaction(signature, program_id).await {
                Ok(processed) => {
                    if processed {
                        backfilled_count += 1;
                        backfilled_signatures.push(signature.clone());

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
                    warn!("⚠️ 处理交易失败 {}: {}", signature, e);
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

        // 解析事件
        match self
            .parser_registry
            .parse_all_events_with_context(&logs_response.logs, signature, slot, &vec![*program_id])
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
        program_id: &solana_sdk::pubkey::Pubkey,
        event_name: &str,
    ) -> Result<()> {
        let checkpoint = EventScannerCheckpoints {
            id: None,
            program_id: Some(program_id.to_string()),
            event_name: Some(event_name.to_string()),
            slot: None,
            last_signature: Some(last_signature.to_string()),
            updated_at: Utc::now(),
            created_at: Utc::now(),
        };

        self.checkpoint_persistence.update_checkpoint(&checkpoint).await?;

        info!("📍 更新检查点 {}: {}", event_name, last_signature);
        Ok(())
    }
}
