use crate::{
    config::EventListenerConfig,
    error::{EventListenerError, Result},
    parser::{event_parser::PoolCreationEventData, EventParser, ParsedEvent},
};
use async_trait::async_trait;
use base64::{engine::general_purpose, Engine as _};
use borsh::{BorshDeserialize, BorshSerialize};
use solana_sdk::pubkey::Pubkey;
use tracing::{debug, info, warn};

/// 池子创建事件的原始数据结构（与Raydium CLMM智能合约保持一致）
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct PoolCreationEvent {
    /// CLMM池子地址
    pub pool_address: Pubkey,
    /// 代币A的mint地址
    pub token_a_mint: Pubkey,
    /// 代币B的mint地址  
    pub token_b_mint: Pubkey,
    /// 代币A的小数位数
    pub token_a_decimals: u8,
    /// 代币B的小数位数
    pub token_b_decimals: u8,
    /// 手续费率 (单位: 万分之一, 如3000表示0.3%)
    pub fee_rate: u32,
    /// 初始sqrt价格
    pub sqrt_price_x64: u128,
    /// 初始tick
    pub tick: i32,
    /// 池子创建者
    pub creator: Pubkey,
    /// CLMM配置地址
    pub clmm_config: Pubkey,
    /// 创建时间戳
    pub created_at: i64,
}

/// 池子创建事件解析器
pub struct PoolCreationParser {
    /// 事件的discriminator（从Raydium CLMM IDL获取）
    discriminator: [u8; 8],
}

impl PoolCreationParser {
    /// 创建新的池子创建事件解析器
    pub fn new(_config: &EventListenerConfig) -> Result<Self> {
        // Raydium CLMM PoolCreated事件的discriminator
        // 注意：实际部署时需要从Raydium IDL获取正确的discriminator
        let discriminator = [89, 202, 187, 172, 108, 193, 190, 8];

        Ok(Self { discriminator })
    }

    /// 从程序数据解析池子创建事件
    fn parse_program_data(&self, data_str: &str) -> Result<PoolCreationEvent> {
        // Base64解码
        let data = general_purpose::STANDARD
            .decode(data_str)
            .map_err(|e| EventListenerError::EventParsing(format!("Base64解码失败: {}", e)))?;

        if data.len() < 8 {
            return Err(EventListenerError::EventParsing("数据长度不足，无法包含discriminator".to_string()));
        }

        // 验证discriminator
        let discriminator = &data[0..8];
        if discriminator != self.discriminator {
            return Err(EventListenerError::DiscriminatorMismatch);
        }

        // Borsh反序列化事件数据
        let event_data = &data[8..];
        let event = PoolCreationEvent::try_from_slice(event_data).map_err(|e| EventListenerError::EventParsing(format!("Borsh反序列化失败: {}", e)))?;

        debug!("✅ 成功解析池子创建事件: 池子={}, 代币对={}/{}", event.pool_address, event.token_a_mint, event.token_b_mint);
        Ok(event)
    }

    /// 计算池子相关指标
    fn calculate_pool_metrics(&self, event: &PoolCreationEvent) -> (f64, f64, String) {
        // 计算价格 (从sqrt_price_x64反推)
        let price_ratio = if event.sqrt_price_x64 > 0 {
            let sqrt_price = event.sqrt_price_x64 as f64 / (1u128 << 64) as f64;
            sqrt_price * sqrt_price
        } else {
            0.0
        };

        // 计算年化手续费率
        let annual_fee_rate = (event.fee_rate as f64 / 10000.0) * 365.0; // 假设每天交易一次

        // 确定池子类型
        let pool_type = match event.fee_rate {
            100 => "超低费率".to_string(),  // 0.01%
            500 => "低费率".to_string(),    // 0.05%
            2500 => "标准费率".to_string(), // 0.25%
            3000 => "标准费率".to_string(), // 0.3%
            10000 => "高费率".to_string(),  // 1%
            _ => format!("自定义费率({})", event.fee_rate as f64 / 10000.0),
        };

        (price_ratio, annual_fee_rate, pool_type)
    }

    /// 将原始事件转换为ParsedEvent
    fn convert_to_parsed_event(&self, event: PoolCreationEvent, signature: String, slot: u64) -> ParsedEvent {
        let (initial_price, annual_fee_rate, pool_type) = self.calculate_pool_metrics(&event);

        ParsedEvent::PoolCreation(PoolCreationEventData {
            pool_address: event.pool_address,
            token_a_mint: event.token_a_mint,
            token_b_mint: event.token_b_mint,
            token_a_decimals: event.token_a_decimals,
            token_b_decimals: event.token_b_decimals,
            fee_rate: event.fee_rate,
            fee_rate_percentage: event.fee_rate as f64 / 10000.0,
            annual_fee_rate,
            pool_type,
            sqrt_price_x64: event.sqrt_price_x64,
            initial_price,
            initial_tick: event.tick,
            creator: event.creator,
            clmm_config: event.clmm_config,
            is_stable_pair: false,        // 需要通过代币分析确定
            estimated_liquidity_usd: 0.0, // 创建时暂无流动性
            created_at: event.created_at,
            signature,
            slot,
            processed_at: chrono::Utc::now().to_rfc3339(),
        })
    }

    /// 验证池子创建事件数据
    fn validate_pool_creation(&self, event: &PoolCreationEventData) -> Result<bool> {
        // 验证池子地址
        if event.pool_address == Pubkey::default() {
            warn!("❌ 无效的池子地址");
            return Ok(false);
        }

        // 验证代币地址
        if event.token_a_mint == Pubkey::default() || event.token_b_mint == Pubkey::default() {
            warn!("❌ 无效的代币地址: {} 或 {}", event.token_a_mint, event.token_b_mint);
            return Ok(false);
        }

        // 验证代币不能相同
        if event.token_a_mint == event.token_b_mint {
            warn!("❌ 代币A和代币B不能相同: {}", event.token_a_mint);
            return Ok(false);
        }

        // 验证小数位数合理性
        if event.token_a_decimals > 18 || event.token_b_decimals > 18 {
            warn!("❌ 代币小数位数超出合理范围: A={}, B={}", event.token_a_decimals, event.token_b_decimals);
            return Ok(false);
        }

        // 验证手续费率合理性 (0.01% - 10%)
        if event.fee_rate == 0 || event.fee_rate > 100000 {
            warn!("❌ 手续费率不合理: {}", event.fee_rate);
            return Ok(false);
        }

        // 验证sqrt价格
        if event.sqrt_price_x64 == 0 {
            warn!("❌ sqrt价格不能为0");
            return Ok(false);
        }

        // 验证创建者地址
        if event.creator == Pubkey::default() {
            warn!("❌ 无效的创建者地址");
            return Ok(false);
        }

        // 验证CLMM配置地址
        if event.clmm_config == Pubkey::default() {
            warn!("❌ 无效的CLMM配置地址");
            return Ok(false);
        }

        // 验证时间戳合理性
        let now = chrono::Utc::now().timestamp();
        if event.created_at > now || event.created_at < (now - 86400) {
            warn!("❌ 创建时间戳异常: {}", event.created_at);
            return Ok(false);
        }

        // 验证tick范围 (Raydium CLMM的tick范围)
        if event.initial_tick < -887272 || event.initial_tick > 887272 {
            warn!("❌ 初始tick超出范围: {}", event.initial_tick);
            return Ok(false);
        }

        Ok(true)
    }
}

#[async_trait]
impl EventParser for PoolCreationParser {
    fn get_discriminator(&self) -> [u8; 8] {
        self.discriminator
    }

    fn get_event_type(&self) -> &'static str {
        "pool_creation"
    }

    async fn parse_from_logs(&self, logs: &[String], signature: &str, slot: u64) -> Result<Option<ParsedEvent>> {
        for (index, log) in logs.iter().enumerate() {
            if log.starts_with("Program data: ") {
                if let Some(data_part) = log.strip_prefix("Program data: ") {
                    match self.parse_program_data(data_part) {
                        Ok(event) => {
                            info!("🏊 第{}行发现池子创建事件: {} (费率: {}%)", index + 1, event.pool_address, event.fee_rate as f64 / 10000.0);
                            let parsed_event = self.convert_to_parsed_event(event, signature.to_string(), slot);
                            return Ok(Some(parsed_event));
                        }
                        Err(EventListenerError::DiscriminatorMismatch) => {
                            // Discriminator不匹配是正常情况，继续尝试下一条日志
                            continue;
                        }
                        Err(e) => {
                            debug!("⚠️ 第{}行池子创建事件解析失败: {}", index + 1, e);
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
            ParsedEvent::PoolCreation(pool_event) => self.validate_pool_creation(pool_event),
            _ => Ok(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn create_test_config() -> EventListenerConfig {
        EventListenerConfig {
            solana: crate::config::settings::SolanaConfig {
                rpc_url: "https://api.devnet.solana.com".to_string(),
                ws_url: "wss://api.devnet.solana.com".to_string(),
                commitment: "confirmed".to_string(),
                program_id: Pubkey::from_str("FA1RJDDXysgwg5Gm3fJXWxt26JQzPkAzhTA114miqNUX").unwrap(),
                private_key: None,
            },
            database: crate::config::settings::DatabaseConfig {
                uri: "mongodb://localhost:27017".to_string(),
                database_name: "test".to_string(),
                max_connections: 10,
                min_connections: 2,
            },
            listener: crate::config::settings::ListenerConfig {
                batch_size: 100,
                sync_interval_secs: 30,
                max_retries: 3,
                retry_delay_ms: 1000,
                signature_cache_size: 10000,
                checkpoint_save_interval_secs: 60,
                backoff: crate::config::settings::BackoffConfig::default(),
                batch_write: crate::config::settings::BatchWriteConfig::default(),
            },
            monitoring: crate::config::settings::MonitoringConfig {
                metrics_interval_secs: 60,
                enable_performance_monitoring: true,
                health_check_interval_secs: 30,
            },
        }
    }

    fn create_test_pool_creation_event() -> PoolCreationEvent {
        PoolCreationEvent {
            pool_address: Pubkey::new_unique(),
            token_a_mint: Pubkey::new_unique(),
            token_b_mint: Pubkey::new_unique(),
            token_a_decimals: 9,
            token_b_decimals: 6,
            fee_rate: 3000,              // 0.3%
            sqrt_price_x64: 1u128 << 64, // 价格为1.0
            tick: 0,
            creator: Pubkey::new_unique(),
            clmm_config: Pubkey::new_unique(),
            created_at: chrono::Utc::now().timestamp(),
        }
    }

    #[test]
    fn test_pool_creation_parser_creation() {
        let config = create_test_config();
        let parser = PoolCreationParser::new(&config).unwrap();

        assert_eq!(parser.get_event_type(), "pool_creation");
        assert_eq!(parser.get_discriminator(), [89, 202, 187, 172, 108, 193, 190, 8]);
    }

    #[test]
    fn test_convert_to_parsed_event() {
        let config = create_test_config();
        let parser = PoolCreationParser::new(&config).unwrap();
        let test_event = create_test_pool_creation_event();

        let parsed = parser.convert_to_parsed_event(test_event.clone(), "test_signature".to_string(), 12345);

        match parsed {
            ParsedEvent::PoolCreation(data) => {
                assert_eq!(data.pool_address, test_event.pool_address);
                assert_eq!(data.token_a_mint, test_event.token_a_mint);
                assert_eq!(data.token_b_mint, test_event.token_b_mint);
                assert_eq!(data.fee_rate, test_event.fee_rate);
                assert_eq!(data.fee_rate_percentage, 0.3);
                assert_eq!(data.signature, "test_signature");
                assert_eq!(data.slot, 12345);
            }
            _ => panic!("期望PoolCreation事件"),
        }
    }

    #[tokio::test]
    async fn test_validate_pool_creation() {
        let config = create_test_config();
        let parser = PoolCreationParser::new(&config).unwrap();

        let valid_event = PoolCreationEventData {
            pool_address: Pubkey::new_unique(),
            token_a_mint: Pubkey::new_unique(),
            token_b_mint: Pubkey::new_unique(),
            token_a_decimals: 9,
            token_b_decimals: 6,
            fee_rate: 3000,
            fee_rate_percentage: 0.3,
            annual_fee_rate: 109.5,
            pool_type: "标准费率".to_string(),
            sqrt_price_x64: 1u128 << 64,
            initial_price: 1.0,
            initial_tick: 0,
            creator: Pubkey::new_unique(),
            clmm_config: Pubkey::new_unique(),
            is_stable_pair: false,
            estimated_liquidity_usd: 0.0,
            created_at: chrono::Utc::now().timestamp(),
            signature: "test_sig".to_string(),
            slot: 12345,
            processed_at: chrono::Utc::now().to_rfc3339(),
        };

        assert!(parser.validate_pool_creation(&valid_event).unwrap());

        // 测试无效事件（相同的代币）
        let invalid_event = PoolCreationEventData {
            token_b_mint: valid_event.token_a_mint, // 相同的代币
            ..valid_event.clone()
        };

        assert!(!parser.validate_pool_creation(&invalid_event).unwrap());
    }

    #[test]
    fn test_calculate_pool_metrics() {
        let config = create_test_config();
        let parser = PoolCreationParser::new(&config).unwrap();

        let event = PoolCreationEvent {
            fee_rate: 3000,              // 0.3%
            sqrt_price_x64: 1u128 << 64, // sqrt(1.0)
            ..create_test_pool_creation_event()
        };

        let (price, annual_fee, pool_type) = parser.calculate_pool_metrics(&event);

        assert!((price - 1.0).abs() < 0.0001); // 价格应该接近1.0
        assert_eq!(annual_fee, 109.5); // 0.3% * 365
        assert_eq!(pool_type, "标准费率");
    }

    #[test]
    fn test_borsh_serialization() {
        let event = create_test_pool_creation_event();

        // 测试序列化
        let serialized = borsh::to_vec(&event).unwrap();
        assert!(!serialized.is_empty());

        // 测试反序列化
        let deserialized = PoolCreationEvent::try_from_slice(&serialized).unwrap();
        assert_eq!(deserialized.pool_address, event.pool_address);
        assert_eq!(deserialized.token_a_mint, event.token_a_mint);
        assert_eq!(deserialized.fee_rate, event.fee_rate);
    }

    #[tokio::test]
    async fn test_parse_from_logs_no_program_data() {
        let config = create_test_config();
        let parser = PoolCreationParser::new(&config).unwrap();

        let logs = vec![
            "Program 11111111111111111111111111111111 invoke [1]".to_string(),
            "Program 11111111111111111111111111111111 success".to_string(),
        ];

        let result = parser.parse_from_logs(&logs, "test_sig", 12345).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_validate_event() {
        let config = create_test_config();
        let parser = PoolCreationParser::new(&config).unwrap();

        let event = ParsedEvent::PoolCreation(PoolCreationEventData {
            pool_address: Pubkey::new_unique(),
            token_a_mint: Pubkey::new_unique(),
            token_b_mint: Pubkey::new_unique(),
            token_a_decimals: 9,
            token_b_decimals: 6,
            fee_rate: 3000,
            fee_rate_percentage: 0.3,
            annual_fee_rate: 109.5,
            pool_type: "标准费率".to_string(),
            sqrt_price_x64: 1u128 << 64,
            initial_price: 1.0,
            initial_tick: 0,
            creator: Pubkey::new_unique(),
            clmm_config: Pubkey::new_unique(),
            is_stable_pair: false,
            estimated_liquidity_usd: 0.0,
            created_at: chrono::Utc::now().timestamp(),
            signature: "test_sig".to_string(),
            slot: 12345,
            processed_at: chrono::Utc::now().to_rfc3339(),
        });

        assert!(parser.validate_event(&event).await.unwrap());
    }

    #[test]
    fn test_discriminator_mismatch_error() {
        let config = create_test_config();
        let parser = PoolCreationParser::new(&config).unwrap();

        // 创建一个带有错误discriminator的base64数据
        let mut data = vec![0u8; 100];
        // 设置一个错误的discriminator（不是池子创建事件的）
        data[0..8].copy_from_slice(&[99, 99, 99, 99, 99, 99, 99, 99]);

        let data_str = general_purpose::STANDARD.encode(&data);
        let result = parser.parse_program_data(&data_str);

        // 验证返回的是DiscriminatorMismatch错误
        assert!(matches!(result, Err(EventListenerError::DiscriminatorMismatch)));
    }

    #[tokio::test]
    async fn test_parse_from_logs_skips_discriminator_mismatch() {
        let config = create_test_config();
        let parser = PoolCreationParser::new(&config).unwrap();

        // 创建一个带有错误discriminator的日志
        let mut wrong_data = vec![0u8; 100];
        wrong_data[0..8].copy_from_slice(&[99, 99, 99, 99, 99, 99, 99, 99]);
        let wrong_log = format!("Program data: {}", general_purpose::STANDARD.encode(&wrong_data));

        // 创建一个正确的日志（但没有完整的事件数据，只是为了测试流程）
        let logs = vec!["Some other log".to_string(), wrong_log, "Another log".to_string()];

        // 解析日志，应该跳过discriminator不匹配的日志，返回None（因为没有匹配的事件）
        let result = parser.parse_from_logs(&logs, "test_signature", 12345).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }
}
