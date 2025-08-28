use crate::{
    config::EventListenerConfig,
    error::{EventListenerError, Result},
    parser::{event_parser::LaunchEventData, EventParser, ParsedEvent},
    services::MigrationClient,
};
use async_trait::async_trait;
use base64::{engine::general_purpose, Engine as _};
use borsh::{BorshDeserialize, BorshSerialize};
use chrono::Utc;
use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use std::sync::Arc;
use tracing::{debug, info, warn};

/// LaunchEvent的原始数据结构（与链上合约保持一致）
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct LaunchEvent {
    /// meme币合约地址
    pub meme_token_mint: Pubkey,
    /// 配对代币地址(通常是SOL或USDC)
    pub base_token_mint: Pubkey,
    /// 用户钱包地址
    pub user_wallet: Pubkey,
    /// CLMM配置索引
    pub config_index: u32,
    /// 初始价格
    pub initial_price: f64,
    /// 池子开放时间戳，0表示立即开放
    pub open_time: u64,
    /// 价格下限
    pub tick_lower_price: f64,
    /// 价格上限  
    pub tick_upper_price: f64,
    /// meme币数量
    pub meme_token_amount: u64,
    /// 配对代币数量
    pub base_token_amount: u64,
    /// 最大滑点百分比
    pub max_slippage_percent: f64,
    /// 是否包含NFT元数据
    pub with_metadata: Option<bool>,
}

/// LaunchEvent解析器
#[allow(dead_code)]
pub struct LaunchEventParser {
    /// 事件的discriminator（需要从合约IDL获取）
    discriminator: [u8; 8],
    /// 目标程序ID
    target_program_id: Pubkey,
    /// RPC客户端
    rpc_client: RpcClient,
    /// 迁移服务客户端
    migration_client: Arc<MigrationClient>,
}

impl LaunchEventParser {
    /// 创建新的LaunchEvent解析器
    pub fn new(config: &EventListenerConfig, program_id: Pubkey) -> Result<Self> {
        let discriminator = [27, 193, 47, 130, 115, 92, 239, 94];

        // 创建RPC客户端
        let rpc_client = RpcClient::new(config.solana.rpc_url.clone());

        // 创建迁移服务客户端
        // 使用环境变量或配置中的后端服务URL
        let migration_service_url =
            std::env::var("MIGRATION_SERVICE_URL").unwrap_or_else(|_| "http://localhost:8765".to_string());

        let migration_client = Arc::new(MigrationClient::new(migration_service_url));

        info!(
            "✅ 创建LaunchEventParser: 程序ID={}, discriminator={:?}",
            program_id, discriminator
        );

        Ok(Self {
            discriminator,
            target_program_id: program_id,
            rpc_client,
            migration_client,
        })
    }

    /// 从程序数据解析LaunchEvent
    fn parse_program_data(&self, data_str: &str) -> Result<LaunchEvent> {
        // Base64解码
        let data = general_purpose::STANDARD
            .decode(data_str)
            .map_err(|e| EventListenerError::EventParsing(format!("Base64解码失败: {}", e)))?;

        if data.len() < 8 {
            return Err(EventListenerError::EventParsing(
                "数据长度不足，无法包含discriminator".to_string(),
            ));
        }

        // 验证discriminator
        let discriminator = &data[0..8];
        if discriminator != self.discriminator {
            return Err(EventListenerError::DiscriminatorMismatch);
        }

        // Borsh反序列化事件数据
        let event_data = &data[8..];
        let event = LaunchEvent::try_from_slice(event_data)
            .map_err(|e| EventListenerError::EventParsing(format!("Borsh反序列化失败: {}", e)))?;

        info!(
            "✅ 成功解析LaunchEvent: meme={}, base={}, user={}",
            event.meme_token_mint, event.base_token_mint, event.user_wallet
        );

        Ok(event)
    }

    /// 将原始事件转换为ParsedEvent
    async fn convert_to_parsed_event(&self, event: LaunchEvent, signature: String, slot: u64) -> Result<ParsedEvent> {
        let data = LaunchEventData {
            meme_token_mint: event.meme_token_mint.to_string(),
            base_token_mint: event.base_token_mint.to_string(),
            user_wallet: event.user_wallet.to_string(),
            config_index: event.config_index,
            initial_price: event.initial_price,
            open_time: event.open_time,
            tick_lower_price: event.tick_lower_price,
            tick_upper_price: event.tick_upper_price,
            meme_token_amount: event.meme_token_amount,
            base_token_amount: event.base_token_amount,
            max_slippage_percent: event.max_slippage_percent,
            with_metadata: event.with_metadata.unwrap_or(false),
            signature,
            slot,
            processed_at: Utc::now().to_rfc3339(),
        };

        Ok(ParsedEvent::Launch(data))
    }

    /// 验证事件数据
    fn validate_launch_event(&self, event: &LaunchEventData) -> Result<bool> {
        // 验证代币地址
        if event.meme_token_mint == event.base_token_mint {
            warn!("❌ meme币和base币地址相同");
            return Ok(false);
        }

        // 验证价格参数
        if event.initial_price <= 0.0 {
            warn!("❌ 初始价格无效: {}", event.initial_price);
            return Ok(false);
        }

        if event.tick_lower_price >= event.tick_upper_price {
            warn!(
                "❌ 价格区间无效: lower={}, upper={}",
                event.tick_lower_price, event.tick_upper_price
            );
            return Ok(false);
        }

        // 验证数量
        if event.meme_token_amount == 0 || event.base_token_amount == 0 {
            warn!(
                "❌ 代币数量无效: meme={}, base={}",
                event.meme_token_amount, event.base_token_amount
            );
            return Ok(false);
        }

        // 验证滑点
        if event.max_slippage_percent < 0.0 || event.max_slippage_percent > 100.0 {
            warn!("❌ 滑点百分比无效: {}", event.max_slippage_percent);
            return Ok(false);
        }

        Ok(true)
    }
}

#[async_trait]
impl EventParser for LaunchEventParser {
    fn get_program_id(&self) -> Pubkey {
        self.target_program_id
    }

    fn get_discriminator(&self) -> [u8; 8] {
        self.discriminator
    }

    fn get_event_type(&self) -> &'static str {
        "launch"
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
                                "🎯 第{}行发现LaunchEvent: user={}, meme={}",
                                index + 1,
                                event.user_wallet,
                                event.meme_token_mint
                            );

                            // 转换为ParsedEvent
                            let parsed_event = self.convert_to_parsed_event(event, signature.to_string(), slot).await?;

                            return Ok(Some(parsed_event));
                        }
                        Err(EventListenerError::DiscriminatorMismatch) => {
                            // Discriminator不匹配是正常情况，继续尝试下一条日志
                            continue;
                        }
                        Err(e) => {
                            debug!("⚠️ 第{}行LaunchEvent解析失败: {}", index + 1, e);
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
            ParsedEvent::Launch(launch_event) => self.validate_launch_event(launch_event),
            _ => Ok(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_sdk::pubkey::Pubkey;

    fn create_test_config() -> EventListenerConfig {
        EventListenerConfig {
            solana: crate::config::settings::SolanaConfig {
                rpc_url: "https://api.devnet.solana.com".to_string(),
                ws_url: "wss://api.devnet.solana.com".to_string(),
                commitment: "confirmed".to_string(),
                program_ids: vec![Pubkey::new_unique()],
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

    #[test]
    fn test_launch_event_parser_creation() {
        let config = create_test_config();
        let parser = LaunchEventParser::new(&config, Pubkey::new_unique()).unwrap();

        assert_eq!(parser.get_event_type(), "launch");
        assert_eq!(parser.get_discriminator(), [27, 193, 47, 130, 115, 92, 239, 94]);
    }

    #[test]
    fn test_borsh_serialization() {
        let event = LaunchEvent {
            meme_token_mint: Pubkey::new_unique(),
            base_token_mint: Pubkey::new_unique(),
            user_wallet: Pubkey::new_unique(),
            config_index: 0,
            initial_price: 1.0,
            open_time: 0,
            tick_lower_price: 0.0001,
            tick_upper_price: 10000.0,
            meme_token_amount: 1000000,
            base_token_amount: 1000000,
            max_slippage_percent: 1.0,
            with_metadata: Some(true),
        };

        // 测试序列化
        let serialized = borsh::to_vec(&event).unwrap();
        assert!(!serialized.is_empty());

        // 测试反序列化
        let deserialized = LaunchEvent::try_from_slice(&serialized).unwrap();
        assert_eq!(deserialized.meme_token_mint, event.meme_token_mint);
        assert_eq!(deserialized.config_index, event.config_index);
    }

    #[tokio::test]
    async fn test_validate_launch_event() {
        let config = create_test_config();
        let parser = LaunchEventParser::new(&config, Pubkey::new_unique()).unwrap();

        // 有效的事件
        let valid_event = LaunchEventData {
            meme_token_mint: Pubkey::new_unique().to_string(),
            base_token_mint: Pubkey::new_unique().to_string(),
            user_wallet: Pubkey::new_unique().to_string(),
            config_index: 0,
            initial_price: 1.0,
            open_time: 0,
            tick_lower_price: 0.0001,
            tick_upper_price: 10000.0,
            meme_token_amount: 1000000,
            base_token_amount: 1000000,
            max_slippage_percent: 1.0,
            with_metadata: true,
            signature: "test_sig".to_string(),
            slot: 12345,
            processed_at: Utc::now().to_rfc3339(),
        };

        assert!(parser.validate_launch_event(&valid_event).unwrap());

        // 无效的事件（相同的代币）
        let invalid_event = LaunchEventData {
            base_token_mint: valid_event.meme_token_mint.clone(), // 相同的代币
            ..valid_event.clone()
        };

        assert!(!parser.validate_launch_event(&invalid_event).unwrap());
    }

    #[tokio::test]
    async fn test_validate_launch_event_invalid_price() {
        let config = create_test_config();
        let parser = LaunchEventParser::new(&config, Pubkey::new_unique()).unwrap();

        let base_event = LaunchEventData {
            meme_token_mint: Pubkey::new_unique().to_string(),
            base_token_mint: Pubkey::new_unique().to_string(),
            user_wallet: Pubkey::new_unique().to_string(),
            config_index: 0,
            initial_price: 1.0,
            open_time: 0,
            tick_lower_price: 0.0001,
            tick_upper_price: 10000.0,
            meme_token_amount: 1000000,
            base_token_amount: 1000000,
            max_slippage_percent: 1.0,
            with_metadata: true,
            signature: "test_sig".to_string(),
            slot: 12345,
            processed_at: Utc::now().to_rfc3339(),
        };

        // 无效初始价格
        let invalid_price_event = LaunchEventData {
            initial_price: -1.0,
            ..base_event.clone()
        };
        assert!(!parser.validate_launch_event(&invalid_price_event).unwrap());

        // 无效价格区间
        let invalid_range_event = LaunchEventData {
            tick_lower_price: 10000.0,
            tick_upper_price: 0.0001,
            ..base_event.clone()
        };
        assert!(!parser.validate_launch_event(&invalid_range_event).unwrap());

        // 无效代币数量
        let invalid_amount_event = LaunchEventData {
            meme_token_amount: 0,
            ..base_event.clone()
        };
        assert!(!parser.validate_launch_event(&invalid_amount_event).unwrap());

        // 无效滑点
        let invalid_slippage_event = LaunchEventData {
            max_slippage_percent: 150.0,
            ..base_event
        };
        assert!(!parser.validate_launch_event(&invalid_slippage_event).unwrap());
    }

    #[tokio::test]
    async fn test_convert_to_parsed_event() {
        let config = create_test_config();
        let parser = LaunchEventParser::new(&config, Pubkey::new_unique()).unwrap();

        let raw_event = LaunchEvent {
            meme_token_mint: Pubkey::new_unique(),
            base_token_mint: Pubkey::new_unique(),
            user_wallet: Pubkey::new_unique(),
            config_index: 0,
            initial_price: 1.0,
            open_time: 0,
            tick_lower_price: 0.0001,
            tick_upper_price: 10000.0,
            meme_token_amount: 1000000,
            base_token_amount: 1000000,
            max_slippage_percent: 1.0,
            with_metadata: Some(true),
        };

        let parsed = parser
            .convert_to_parsed_event(raw_event.clone(), "test_sig".to_string(), 12345)
            .await
            .unwrap();

        match parsed {
            ParsedEvent::Launch(data) => {
                assert_eq!(data.meme_token_mint, raw_event.meme_token_mint.to_string());
                assert_eq!(data.user_wallet, raw_event.user_wallet.to_string());
                assert_eq!(data.signature, "test_sig");
                assert_eq!(data.slot, 12345);
                assert!(data.with_metadata);
            }
            _ => panic!("转换的事件类型不正确"),
        }
    }

    #[test]
    fn test_parse_program_data_invalid_discriminator() {
        let config = create_test_config();
        let parser = LaunchEventParser::new(&config, Pubkey::new_unique()).unwrap();

        // 创建一个错误discriminator的数据
        let mut wrong_data = vec![0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]; // 错误的discriminator
        wrong_data.extend(vec![0; 100]); // 假数据

        let base64_data = general_purpose::STANDARD.encode(wrong_data);

        // 解析应该失败并返回DiscriminatorMismatch
        let result = parser.parse_program_data(&base64_data);
        assert!(matches!(result, Err(EventListenerError::DiscriminatorMismatch)));
    }

    #[test]
    fn test_parse_program_data_invalid_base64() {
        let config = create_test_config();
        let parser = LaunchEventParser::new(&config, Pubkey::new_unique()).unwrap();

        // 无效的base64数据
        let invalid_base64 = "invalid_base64_data!!!";
        let result = parser.parse_program_data(invalid_base64);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Base64解码失败"));
    }

    #[test]
    fn test_supports_program() {
        let config = create_test_config();
        let target_program = Pubkey::new_unique();
        let parser = LaunchEventParser::new(&config, target_program).unwrap();

        // 应该支持目标程序
        assert_eq!(parser.supports_program(&target_program), Some(true));

        // 不应该支持其他程序
        let other_program = Pubkey::new_unique();
        assert_eq!(parser.supports_program(&other_program), Some(false));
    }
}
