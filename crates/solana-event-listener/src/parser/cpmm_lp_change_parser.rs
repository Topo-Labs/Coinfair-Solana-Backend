use crate::{
    config::EventListenerConfig,
    error::{EventListenerError, Result},
    parser::{EventParser, ParsedEvent},
};
use async_trait::async_trait;
use base64::{engine::general_purpose, Engine as _};
use borsh::{BorshDeserialize, BorshSerialize};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use tracing::{debug, info, warn};

/// LP变更事件的原始数据结构（与智能合约保持一致）
/// 注意：字段顺序必须与智能合约中的事件结构体完全一致，否则Borsh反序列化会失败
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct LpChangeEvent {
    /// 用户钱包地址
    pub user_wallet: Pubkey,
    /// 池子ID
    pub pool_id: Pubkey,
    /// LP mint地址
    pub lp_mint: Pubkey,
    /// token_0 mint地址
    pub token_0_mint: Pubkey,
    /// token_1 mint地址
    pub token_1_mint: Pubkey,
    /// 变更前的LP数量
    pub lp_amount_before: u64,
    /// 变更前的token_0金库余额（扣除交易费后）
    pub token_0_vault_before: u64,
    /// 变更前的token_1金库余额（扣除交易费后）
    pub token_1_vault_before: u64,
    /// token_0操作数量（不含转账费）
    pub token_0_amount: u64,
    /// token_1操作数量（不含转账费）
    pub token_1_amount: u64,
    /// token_0转账费
    pub token_0_transfer_fee: u64,
    /// token_1转账费
    pub token_1_transfer_fee: u64,
    /// 变更类型：0=存款，1=取款，2=初始化
    pub change_type: u8,
    /// LP mint的程序ID
    pub lp_mint_program_id: Pubkey,
    /// token_0的程序ID
    pub token_0_program_id: Pubkey,
    /// token_1的程序ID
    pub token_1_program_id: Pubkey,
    /// LP mint的精度
    pub lp_mint_decimals: u8,
    /// token_0的精度
    pub token_0_decimals: u8,
    /// token_1的精度
    pub token_1_decimals: u8,
}

/// LP变更事件数据（用于事件监听器解析结果）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LpChangeEventData {
    // 用户和池子信息
    pub user_wallet: String,
    pub pool_id: String,
    pub lp_mint: String,
    pub token_0_mint: String,
    pub token_1_mint: String,

    // 变更类型
    pub change_type: u8, // 0: deposit, 1: withdraw, 2: initialize

    // LP数量变化
    pub lp_amount_before: u64,
    pub lp_amount_after: u64,
    pub lp_amount_change: i64, // 可为负数

    // 代币数量
    pub token_0_amount: u64,
    pub token_1_amount: u64,
    pub token_0_transfer_fee: u64,
    pub token_1_transfer_fee: u64,

    // 池子状态
    pub token_0_vault_before: u64,
    pub token_1_vault_before: u64,
    pub token_0_vault_after: u64,
    pub token_1_vault_after: u64,

    // 程序ID和精度
    pub lp_mint_program_id: String,
    pub token_0_program_id: String,
    pub token_1_program_id: String,
    pub lp_mint_decimals: u8,
    pub token_0_decimals: u8,
    pub token_1_decimals: u8,

    // 交易信息
    pub signature: String,
    pub slot: u64,
    pub processed_at: String,
}

/// LP变更事件解析器
pub struct LpChangeParser {
    /// 事件的discriminator（需要从合约IDL获取）
    discriminator: [u8; 8],
    /// 目标程序ID
    target_program_id: Pubkey,
}

impl LpChangeParser {
    /// 创建新的LP变更事件解析器
    pub fn new(_config: &EventListenerConfig, program_id: Pubkey) -> Result<Self> {
        // 根据设计文档，使用事件类型名称计算discriminator
        let discriminator = crate::parser::event_parser::calculate_event_discriminator("LpChangeEvent");

        info!(
            "✅ 创建LpChangeParser: 程序ID={}, discriminator={:?}",
            program_id, discriminator
        );

        Ok(Self {
            discriminator,
            target_program_id: program_id,
        })
    }

    /// 从程序数据解析LP变更事件
    fn parse_program_data(&self, data_str: &str) -> Result<LpChangeEvent> {
        // Base64解码
        let data = general_purpose::STANDARD.decode(data_str).map_err(|e| {
            warn!("❌ Base64解码失败: {}, data: {}...", e, &data_str[..50.min(data_str.len())]);
            EventListenerError::EventParsing(format!("Base64解码失败: {}", e))
        })?;

        debug!("📊 解码后数据长度: {} bytes", data.len());

        if data.len() < 8 {
            warn!("❌ 数据长度不足: {} bytes", data.len());
            return Err(EventListenerError::EventParsing("数据长度不足，无法包含discriminator".to_string()));
        }

        // 验证discriminator
        let discriminator = &data[0..8];
        debug!("🔍 实际discriminator: {:?}", discriminator);
        debug!("🔍 期望discriminator: {:?}", self.discriminator);

        if discriminator != self.discriminator {
            warn!(
                "❌ Discriminator不匹配: 实际={:?}, 期望={:?}",
                discriminator, self.discriminator
            );
            return Err(EventListenerError::DiscriminatorMismatch);
        }

        // Borsh反序列化事件数据
        let event_data = &data[8..];
        debug!("📊 事件数据长度: {} bytes", event_data.len());

        let event = LpChangeEvent::try_from_slice(event_data).map_err(|e| {
            warn!("❌ Borsh反序列化失败: {}", e);
            EventListenerError::EventParsing(format!("Borsh反序列化失败: {}", e))
        })?;

        info!(
            "✅ 成功解析LP变更事件: 用户={}, 池子={}, 类型={}",
            event.user_wallet, event.pool_id, event.change_type
        );

        Ok(event)
    }

    /// 获取变更类型名称
    fn get_change_type_name(&self, change_type: u8) -> String {
        match change_type {
            0 => "deposit".to_string(),
            1 => "withdraw".to_string(),
            2 => "initialize".to_string(),
            _ => "unknown".to_string(),
        }
    }

    /// 将原始事件转换为ParsedEvent
    async fn convert_to_parsed_event(&self, event: LpChangeEvent, signature: String, slot: u64) -> Result<ParsedEvent> {
        // 计算派生字段
        // 根据change_type计算token_0和token_1的变化方向
        let (token_0_delta, token_1_delta) = match event.change_type {
            0 => {
                // deposit: token增加，vault增加
                (
                    event.token_0_amount as i64 + event.token_0_transfer_fee as i64,
                    event.token_1_amount as i64 + event.token_1_transfer_fee as i64,
                )
            }
            1 => {
                // withdraw: token减少，vault减少
                (
                    -(event.token_0_amount as i64 + event.token_0_transfer_fee as i64),
                    -(event.token_1_amount as i64 + event.token_1_transfer_fee as i64),
                )
            }
            2 => {
                // initialize: 初始化，token增加
                (
                    event.token_0_amount as i64 + event.token_0_transfer_fee as i64,
                    event.token_1_amount as i64 + event.token_1_transfer_fee as i64,
                )
            }
            _ => (0, 0),
        };

        // 计算vault_after
        let token_0_vault_after = (event.token_0_vault_before as i64 + token_0_delta) as u64;
        let token_1_vault_after = (event.token_1_vault_before as i64 + token_1_delta) as u64;

        // 计算LP数量变化
        // 对于deposit和initialize，LP增加；对于withdraw，LP减少
        let (lp_amount_after, lp_amount_change) = match event.change_type {
            0 | 2 => {
                // deposit或initialize: LP增加
                // 需要根据AMM公式计算，这里简化处理，实际应该从合约获取
                // 暂时使用token_0_amount作为近似值
                let lp_delta = event.token_0_amount; // 简化处理
                (event.lp_amount_before + lp_delta, lp_delta as i64)
            }
            1 => {
                // withdraw: LP减少
                let lp_delta = event.token_0_amount; // 简化处理
                (event.lp_amount_before.saturating_sub(lp_delta), -(lp_delta as i64))
            }
            _ => (event.lp_amount_before, 0),
        };

        let lp_change_event = LpChangeEventData {
            user_wallet: event.user_wallet.to_string(),
            pool_id: event.pool_id.to_string(),
            lp_mint: event.lp_mint.to_string(),
            token_0_mint: event.token_0_mint.to_string(),
            token_1_mint: event.token_1_mint.to_string(),

            change_type: event.change_type,

            // LP数量变化 - 计算得出
            lp_amount_before: event.lp_amount_before,
            lp_amount_after,
            lp_amount_change,

            // 代币数量 - 原始数值
            token_0_amount: event.token_0_amount,
            token_1_amount: event.token_1_amount,
            token_0_transfer_fee: event.token_0_transfer_fee,
            token_1_transfer_fee: event.token_1_transfer_fee,

            // 池子状态 - 原始和计算值
            token_0_vault_before: event.token_0_vault_before,
            token_1_vault_before: event.token_1_vault_before,
            token_0_vault_after,
            token_1_vault_after,

            // 程序ID和精度信息
            lp_mint_program_id: event.lp_mint_program_id.to_string(),
            token_0_program_id: event.token_0_program_id.to_string(),
            token_1_program_id: event.token_1_program_id.to_string(),

            lp_mint_decimals: event.lp_mint_decimals,
            token_0_decimals: event.token_0_decimals,
            token_1_decimals: event.token_1_decimals,

            // 交易信息
            signature,
            slot,
            processed_at: Utc::now().to_rfc3339(),
        };

        Ok(ParsedEvent::LpChange(lp_change_event))
    }

    /// 验证LP变更事件数据
    fn validate_lp_change_event(&self, event: &LpChangeEventData) -> Result<bool> {
        // 验证用户钱包地址
        if event.user_wallet.trim().is_empty() {
            warn!("❌ 用户钱包地址为空");
            return Ok(false);
        }

        // 验证池子ID
        if event.pool_id.trim().is_empty() {
            warn!("❌ 池子ID为空");
            return Ok(false);
        }

        // 验证LP mint地址
        if event.lp_mint.trim().is_empty() {
            warn!("❌ LP mint地址为空");
            return Ok(false);
        }

        // 验证代币mint地址
        if event.token_0_mint.trim().is_empty() || event.token_1_mint.trim().is_empty() {
            warn!("❌ 代币mint地址为空");
            return Ok(false);
        }

        // 验证变更类型
        if event.change_type > 2 {
            warn!("❌ 无效的变更类型: {}", event.change_type);
            return Ok(false);
        }

        // 验证数量一致性 - 非初始化操作
        if event.change_type != 2 && event.lp_amount_before == 0 {
            warn!("❌ 非初始化操作但LP数量为0");
            return Ok(false);
        }

        // 验证数量一致性 - 初始化操作
        if event.change_type == 2 && event.lp_amount_before != 0 {
            warn!("❌ 初始化操作但LP已有数量不为0");
            return Ok(false);
        }

        // 验证精度范围
        if event.lp_mint_decimals > 18 || event.token_0_decimals > 18 || event.token_1_decimals > 18 {
            warn!("❌ 代币精度超出合理范围");
            return Ok(false);
        }

        // 验证交易签名
        if event.signature.trim().is_empty() {
            warn!("❌ 交易签名为空");
            return Ok(false);
        }

        // 验证slot
        if event.slot == 0 {
            warn!("❌ 无效的slot: {}", event.slot);
            return Ok(false);
        }

        Ok(true)
    }
}

#[async_trait]
impl EventParser for LpChangeParser {
    fn get_program_id(&self) -> Pubkey {
        self.target_program_id
    }

    fn get_discriminator(&self) -> [u8; 8] {
        self.discriminator
    }

    fn get_event_type(&self) -> &'static str {
        "lp_change"
    }

    fn supports_program(&self, program_id: &Pubkey) -> Option<bool> {
        Some(*program_id == self.target_program_id)
    }

    async fn parse_from_logs(&self, logs: &[String], signature: &str, slot: u64) -> Result<Option<ParsedEvent>> {
        for (index, log) in logs.iter().enumerate() {
            if log.starts_with("Program data: ") {
                if let Some(data_part) = log.strip_prefix("Program data: ") {
                    match self.parse_program_data(data_part) {
                        Ok(event) => {
                            info!(
                                "💰 第{}行发现LP变更事件: 用户={}, 池子={}, 类型={}",
                                index + 1,
                                event.user_wallet,
                                event.pool_id,
                                self.get_change_type_name(event.change_type)
                            );

                            // 转换为ParsedEvent
                            let parsed_event = self.convert_to_parsed_event(event, signature.to_string(), slot).await?;

                            // 验证事件数据
                            if let ParsedEvent::LpChange(ref lp_change_data) = parsed_event {
                                match self.validate_lp_change_event(lp_change_data) {
                                    Ok(true) => {
                                        info!("✅ LP变更事件验证通过");
                                        return Ok(Some(parsed_event));
                                    }
                                    Ok(false) => {
                                        warn!("❌ LP变更事件验证失败，跳过此事件");
                                        continue;
                                    }
                                    Err(e) => {
                                        warn!("❌ LP变更事件验证出错: {}", e);
                                        continue;
                                    }
                                }
                            }

                            return Ok(Some(parsed_event));
                        }
                        Err(EventListenerError::DiscriminatorMismatch) => {
                            // Discriminator不匹配是正常情况，继续尝试下一条日志
                            continue;
                        }
                        Err(e) => {
                            debug!("⚠️ 第{}行LP变更事件解析失败: {}", index + 1, e);
                            continue;
                        }
                    }
                }
            }
        }
        Ok(None)
    }

    async fn validate_event(&self, event: &ParsedEvent) -> Result<bool> {
        match event {
            ParsedEvent::LpChange(lp_event) => self.validate_lp_change_event(lp_event),
            _ => Ok(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::EventListenerConfig;

    fn create_test_config() -> EventListenerConfig {
        use crate::config::settings::*;
        EventListenerConfig {
            listener: ListenerConfig {
                batch_size: 10,
                sync_interval_secs: 30,
                max_retries: 3,
                retry_delay_ms: 1000,
                signature_cache_size: 1000,
                checkpoint_save_interval_secs: 60,
                backoff: BackoffConfig {
                    initial_delay_ms: 1000,
                    max_delay_ms: 30000,
                    multiplier: 2.0,
                    max_retries: Some(5),
                    enable_simple_reconnect: true,
                    simple_reconnect_interval_ms: 500,
                },
                batch_write: BatchWriteConfig {
                    batch_size: 10,
                    max_wait_ms: 1000,
                    buffer_size: 100,
                    concurrent_writers: 1,
                },
            },
            solana: SolanaConfig {
                rpc_url: "https://api.devnet.solana.com".to_string(),
                ws_url: "wss://api.devnet.solana.com".to_string(),
                commitment: "finalized".to_string(),
                program_ids: vec![],
                private_key: None,
            },
            database: DatabaseConfig {
                uri: "mongodb://localhost:27017".to_string(),
                database_name: "test".to_string(),
                max_connections: 10,
                min_connections: 1,
            },
            monitoring: MonitoringConfig {
                metrics_interval_secs: 60,
                enable_performance_monitoring: true,
                health_check_interval_secs: 30,
            },
            backfill: None,
        }
    }

    fn test_program_id() -> Pubkey {
        Pubkey::new_unique()
    }

    fn create_test_lp_change_event() -> LpChangeEventData {
        LpChangeEventData {
            user_wallet: "test_user".to_string(),
            pool_id: "test_pool".to_string(),
            lp_mint: "test_lp_mint".to_string(),
            token_0_mint: "test_token_0".to_string(),
            token_1_mint: "test_token_1".to_string(),
            change_type: 0,
            lp_amount_before: 1000,
            lp_amount_after: 2000,
            lp_amount_change: 1000,
            token_0_amount: 500,
            token_1_amount: 500,
            token_0_transfer_fee: 10,
            token_1_transfer_fee: 10,
            token_0_vault_before: 10000,
            token_1_vault_before: 10000,
            token_0_vault_after: 10500,
            token_1_vault_after: 10500,
            lp_mint_program_id: "test_program".to_string(),
            token_0_program_id: "test_program".to_string(),
            token_1_program_id: "test_program".to_string(),
            lp_mint_decimals: 9,
            token_0_decimals: 9,
            token_1_decimals: 9,
            signature: "test_signature".to_string(),
            slot: 12345,
            processed_at: Utc::now().to_rfc3339(),
        }
    }

    #[test]
    fn test_lp_change_parser_creation() {
        let config = create_test_config();
        let program_id = test_program_id();
        let parser = LpChangeParser::new(&config, program_id);

        assert!(parser.is_ok());
        let parser = parser.unwrap();
        assert_eq!(parser.get_program_id(), program_id);
        assert_eq!(parser.get_event_type(), "lp_change");
    }

    #[test]
    fn test_validate_lp_change_event() {
        let config = create_test_config();
        let parser = LpChangeParser::new(&config, test_program_id()).unwrap();

        // 测试有效事件
        let valid_event = create_test_lp_change_event();
        assert!(parser.validate_lp_change_event(&valid_event).unwrap());

        // 测试无效事件 - 空钱包地址
        let mut invalid_event = create_test_lp_change_event();
        invalid_event.user_wallet = String::new();
        assert!(!parser.validate_lp_change_event(&invalid_event).unwrap());

        // 测试无效事件 - 无效变更类型
        let mut invalid_type_event = create_test_lp_change_event();
        invalid_type_event.change_type = 5;
        assert!(!parser.validate_lp_change_event(&invalid_type_event).unwrap());

        // 测试无效事件 - 精度超出范围
        let mut invalid_decimals_event = create_test_lp_change_event();
        invalid_decimals_event.lp_mint_decimals = 20;
        assert!(!parser.validate_lp_change_event(&invalid_decimals_event).unwrap());
    }

    #[tokio::test]
    async fn test_validate_event() {
        let config = create_test_config();
        let parser = LpChangeParser::new(&config, test_program_id()).unwrap();
        let event_data = create_test_lp_change_event();
        let event = ParsedEvent::LpChange(event_data);

        assert!(parser.validate_event(&event).await.unwrap());
    }

    #[test]
    fn test_get_change_type_name() {
        let config = create_test_config();
        let parser = LpChangeParser::new(&config, test_program_id()).unwrap();

        assert_eq!(parser.get_change_type_name(0), "deposit");
        assert_eq!(parser.get_change_type_name(1), "withdraw");
        assert_eq!(parser.get_change_type_name(2), "initialize");
        assert_eq!(parser.get_change_type_name(99), "unknown");
    }

    #[test]
    fn test_supports_program() {
        let config = create_test_config();
        let program_id = test_program_id();
        let parser = LpChangeParser::new(&config, program_id).unwrap();

        assert_eq!(parser.supports_program(&program_id), Some(true));
        assert_eq!(parser.supports_program(&Pubkey::new_unique()), Some(false));
    }

    #[test]
    fn test_data_compatibility() {
        // 测试LpChangeEventData与数据库模型的兼容性
        let event_data = create_test_lp_change_event();

        // 验证字段类型兼容性
        assert_eq!(event_data.lp_amount_before, 1000u64);
        assert_eq!(event_data.lp_amount_change, 1000i64);
        assert_eq!(event_data.change_type, 0u8);

        // 验证地址字段为String类型
        assert!(!event_data.user_wallet.is_empty());
        assert!(!event_data.signature.is_empty());
    }

    #[test]
    fn test_lp_change_event_discriminator() {
        // 测试并显示LpChangeEvent的discriminator值
        let discriminator = crate::parser::event_parser::calculate_event_discriminator("LpChangeEvent");
        println!("✅ LpChangeEvent discriminator: {:?}", discriminator);

        // 验证discriminator不是全零
        assert_ne!(discriminator, [0, 0, 0, 0, 0, 0, 0, 0]);

        // 验证discriminator的一致性（多次计算应该得到相同结果）
        let discriminator2 = crate::parser::event_parser::calculate_event_discriminator("LpChangeEvent");
        assert_eq!(discriminator, discriminator2);
    }
}
