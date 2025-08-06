use crate::error::{EventListenerError, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 解析后的事件数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ParsedEvent {
    /// 代币创建事件
    TokenCreation(TokenCreationEventData),
    /// 池子创建事件
    PoolCreation(PoolCreationEventData),
    /// NFT领取事件
    NftClaim(NftClaimEventData),
    /// 奖励分发事件
    RewardDistribution(RewardDistributionEventData),
}

impl ParsedEvent {
    /// 获取事件类型字符串
    pub fn event_type(&self) -> &'static str {
        match self {
            ParsedEvent::TokenCreation(_) => "token_creation",
            ParsedEvent::PoolCreation(_) => "pool_creation",
            ParsedEvent::NftClaim(_) => "nft_claim",
            ParsedEvent::RewardDistribution(_) => "reward_distribution",
        }
    }

    /// 获取事件的唯一标识符（用于去重）
    pub fn get_unique_id(&self) -> String {
        match self {
            ParsedEvent::TokenCreation(data) => data.mint_address.to_string(),
            ParsedEvent::PoolCreation(data) => data.pool_address.to_string(),
            ParsedEvent::NftClaim(data) => format!("{}_{}", data.nft_mint, data.signature),
            ParsedEvent::RewardDistribution(data) => format!("{}_{}", data.distribution_id, data.signature),
        }
    }
}

/// 代币创建事件数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenCreationEventData {
    /// 代币的 Mint 地址
    pub mint_address: solana_sdk::pubkey::Pubkey,
    /// 代币名称
    pub name: String,
    /// 代币符号
    pub symbol: String,
    /// 代币元数据的 URI（如 IPFS 链接）
    pub uri: String,
    /// 代币小数位数
    pub decimals: u8,
    /// 供应量（以最小单位计）
    pub supply: u64,
    /// 创建者的钱包地址
    pub creator: solana_sdk::pubkey::Pubkey,
    /// 是否支持白名单（true 表示有白名单机制）
    pub has_whitelist: bool,
    /// 白名单资格检查的时间戳（Unix 时间戳，0 表示无时间限制）
    pub whitelist_deadline: i64,
    /// 创建时间（Unix 时间戳）
    pub created_at: i64,
    /// 交易签名
    pub signature: String,
    /// 区块高度
    pub slot: u64,
}

/// 池子创建事件数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolCreationEventData {
    /// CLMM池子地址
    pub pool_address: solana_sdk::pubkey::Pubkey,
    /// 代币A的mint地址
    pub token_a_mint: solana_sdk::pubkey::Pubkey,
    /// 代币B的mint地址
    pub token_b_mint: solana_sdk::pubkey::Pubkey,
    /// 代币A的小数位数
    pub token_a_decimals: u8,
    /// 代币B的小数位数
    pub token_b_decimals: u8,
    /// 手续费率 (单位: 万分之一)
    pub fee_rate: u32,
    /// 手续费率百分比
    pub fee_rate_percentage: f64,
    /// 年化手续费率
    pub annual_fee_rate: f64,
    /// 池子类型
    pub pool_type: String,
    /// 初始sqrt价格
    pub sqrt_price_x64: u128,
    /// 初始价格比率
    pub initial_price: f64,
    /// 初始tick
    pub initial_tick: i32,
    /// 池子创建者
    pub creator: solana_sdk::pubkey::Pubkey,
    /// CLMM配置地址
    pub clmm_config: solana_sdk::pubkey::Pubkey,
    /// 是否为稳定币对
    pub is_stable_pair: bool,
    /// 预估流动性价值(USD)
    pub estimated_liquidity_usd: f64,
    /// 创建时间戳
    pub created_at: i64,
    /// 交易签名
    pub signature: String,
    /// 区块高度
    pub slot: u64,
    /// 处理时间
    pub processed_at: String,
}

/// NFT领取事件数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NftClaimEventData {
    /// NFT的mint地址
    pub nft_mint: solana_sdk::pubkey::Pubkey,
    /// 领取者钱包地址
    pub claimer: solana_sdk::pubkey::Pubkey,
    /// 推荐人地址（可选）
    pub referrer: Option<solana_sdk::pubkey::Pubkey>,
    /// NFT等级 (1-5级)
    pub tier: u8,
    /// 等级名称
    pub tier_name: String,
    /// 等级奖励倍率
    pub tier_bonus_rate: f64,
    /// 领取的代币数量
    pub claim_amount: u64,
    /// 代币mint地址
    pub token_mint: solana_sdk::pubkey::Pubkey,
    /// 奖励倍率 (基点)
    pub reward_multiplier: u16,
    /// 奖励倍率百分比
    pub reward_multiplier_percentage: f64,
    /// 实际奖励金额（包含倍率）
    pub bonus_amount: u64,
    /// 领取类型
    pub claim_type: u8,
    /// 领取类型名称
    pub claim_type_name: String,
    /// 累计领取量
    pub total_claimed: u64,
    /// 领取进度百分比
    pub claim_progress_percentage: f64,
    /// NFT所属的池子地址（可选）
    pub pool_address: Option<solana_sdk::pubkey::Pubkey>,
    /// 是否有推荐人
    pub has_referrer: bool,
    /// 是否为紧急领取
    pub is_emergency_claim: bool,
    /// 预估USD价值
    pub estimated_usd_value: f64,
    /// 领取时间戳
    pub claimed_at: i64,
    /// 交易签名
    pub signature: String,
    /// 区块高度
    pub slot: u64,
    /// 处理时间
    pub processed_at: String,
}

/// 奖励分发事件数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RewardDistributionEventData {
    /// 奖励分发ID
    pub distribution_id: u64,
    /// 奖励池地址
    pub reward_pool: solana_sdk::pubkey::Pubkey,
    /// 接收者钱包地址
    pub recipient: solana_sdk::pubkey::Pubkey,
    /// 推荐人地址（可选）
    pub referrer: Option<solana_sdk::pubkey::Pubkey>,
    /// 奖励代币mint地址
    pub reward_token_mint: solana_sdk::pubkey::Pubkey,
    /// 奖励数量
    pub reward_amount: u64,
    /// 基础奖励金额
    pub base_reward_amount: u64,
    /// 额外奖励金额
    pub bonus_amount: u64,
    /// 奖励类型
    pub reward_type: u8,
    /// 奖励类型名称
    pub reward_type_name: String,
    /// 奖励来源
    pub reward_source: u8,
    /// 奖励来源名称
    pub reward_source_name: String,
    /// 相关地址
    pub related_address: Option<solana_sdk::pubkey::Pubkey>,
    /// 奖励倍率 (基点)
    pub multiplier: u16,
    /// 奖励倍率百分比
    pub multiplier_percentage: f64,
    /// 是否已锁定
    pub is_locked: bool,
    /// 锁定期结束时间戳
    pub unlock_timestamp: Option<i64>,
    /// 锁定天数
    pub lock_days: u64,
    /// 是否有推荐人
    pub has_referrer: bool,
    /// 是否为推荐奖励
    pub is_referral_reward: bool,
    /// 是否为高价值奖励
    pub is_high_value_reward: bool,
    /// 预估USD价值
    pub estimated_usd_value: f64,
    /// 发放时间戳
    pub distributed_at: i64,
    /// 交易签名
    pub signature: String,
    /// 区块高度
    pub slot: u64,
    /// 处理时间
    pub processed_at: String,
}

/// 通用事件解析器接口
#[async_trait]
pub trait EventParser: Send + Sync {
    /// 获取此解析器处理的事件类型的discriminator
    fn get_discriminator(&self) -> [u8; 8];

    /// 获取事件类型名称
    fn get_event_type(&self) -> &'static str;

    /// 从日志数据中解析事件
    async fn parse_from_logs(&self, logs: &[String], signature: &str, slot: u64) -> Result<Option<ParsedEvent>>;

    /// 验证解析后的事件数据
    async fn validate_event(&self, event: &ParsedEvent) -> Result<bool>;
}

/// 事件解析器注册表
/// 
/// 管理所有已注册的事件解析器，并根据discriminator路由事件到对应的解析器
pub struct EventParserRegistry {
    parsers: HashMap<[u8; 8], Box<dyn EventParser>>,
}

impl EventParserRegistry {
    /// 创建新的解析器注册表
    pub fn new(config: &crate::config::EventListenerConfig) -> Result<Self> {
        let mut registry = Self {
            parsers: HashMap::new(),
        };

        // 注册代币创建事件解析器
        let token_creation_parser = Box::new(
            crate::parser::TokenCreationParser::new(config)?
        );
        registry.register_parser(token_creation_parser)?;

        // 注册池子创建事件解析器
        let pool_creation_parser = Box::new(
            crate::parser::PoolCreationParser::new(config)?
        );
        registry.register_parser(pool_creation_parser)?;

        // 注册NFT领取事件解析器
        let nft_claim_parser = Box::new(
            crate::parser::NftClaimParser::new(config)?
        );
        registry.register_parser(nft_claim_parser)?;

        // 注册奖励分发事件解析器
        let reward_distribution_parser = Box::new(
            crate::parser::RewardDistributionParser::new(config)?
        );
        registry.register_parser(reward_distribution_parser)?;

        Ok(registry)
    }

    /// 注册事件解析器
    pub fn register_parser(&mut self, parser: Box<dyn EventParser>) -> Result<()> {
        let discriminator = parser.get_discriminator();
        let event_type = parser.get_event_type();

        if self.parsers.contains_key(&discriminator) {
            return Err(EventListenerError::EventParsing(
                format!("Discriminator {:?} already registered", discriminator)
            ));
        }

        self.parsers.insert(discriminator, parser);
        tracing::info!("✅ 注册事件解析器: {} ({:?})", event_type, discriminator);
        Ok(())
    }

    /// 从日志中解析事件
    pub async fn parse_event(&self, logs: &[String]) -> Result<Option<ParsedEvent>> {
        // 遍历所有日志，寻找程序数据日志
        for log in logs {
            if let Some(event) = self.try_parse_log(log, "", 0).await? {
                return Ok(Some(event));
            }
        }

        Ok(None)
    }

    /// 从单条日志和完整上下文解析事件
    pub async fn parse_event_with_context(&self, logs: &[String], signature: &str, slot: u64) -> Result<Option<ParsedEvent>> {
        // 首先尝试找到程序数据日志
        for log in logs {
            if log.starts_with("Program data: ") {
                if let Some(data_part) = log.strip_prefix("Program data: ") {
                    if let Some(event) = self.try_parse_program_data(data_part, signature, slot).await? {
                        return Ok(Some(event));
                    }
                }
            }
        }

        // 如果没有找到程序数据日志，尝试其他解析策略
        for parser in self.parsers.values() {
            if let Some(event) = parser.parse_from_logs(logs, signature, slot).await? {
                return Ok(Some(event));
            }
        }

        Ok(None)
    }

    /// 尝试从单条日志解析事件
    async fn try_parse_log(&self, log: &str, signature: &str, slot: u64) -> Result<Option<ParsedEvent>> {
        if log.starts_with("Program data: ") {
            if let Some(data_part) = log.strip_prefix("Program data: ") {
                return self.try_parse_program_data(data_part, signature, slot).await;
            }
        }
        Ok(None)
    }

    /// 尝试从程序数据解析事件
    async fn try_parse_program_data(&self, data_str: &str, signature: &str, slot: u64) -> Result<Option<ParsedEvent>> {
        // 解码Base64数据
        use base64::{Engine as _, engine::general_purpose};
        let data = general_purpose::STANDARD.decode(data_str)
            .map_err(|e| EventListenerError::EventParsing(format!("Base64解码失败: {}", e)))?;

        if data.len() < 8 {
            return Ok(None);
        }

        // 提取discriminator
        let discriminator: [u8; 8] = data[0..8].try_into()
            .map_err(|_| EventListenerError::EventParsing("无法提取discriminator".to_string()))?;

        // 查找对应的解析器
        if let Some(parser) = self.parsers.get(&discriminator) {
            tracing::debug!("🔍 找到匹配的解析器: {} ({:?})", parser.get_event_type(), discriminator);
            
            // 使用找到的解析器解析事件
            if let Some(event) = parser.parse_from_logs(&[format!("Program data: {}", data_str)], signature, slot).await? {
                // 验证解析后的事件
                if parser.validate_event(&event).await? {
                    return Ok(Some(event));
                } else {
                    tracing::warn!("⚠️ 事件验证失败: {}", signature);
                }
            }
        } else {
            tracing::debug!("🤷 未找到匹配的解析器: {:?}", discriminator);
        }

        Ok(None)
    }

    /// 获取所有已注册的解析器信息
    pub fn get_registered_parsers(&self) -> Vec<(String, [u8; 8])> {
        self.parsers
            .values()
            .map(|parser| (parser.get_event_type().to_string(), parser.get_discriminator()))
            .collect()
    }

    /// 获取注册的解析器数量
    pub fn parser_count(&self) -> usize {
        self.parsers.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Mock解析器用于测试
    struct MockParser {
        discriminator: [u8; 8],
        event_type: &'static str,
    }

    #[async_trait]
    impl EventParser for MockParser {
        fn get_discriminator(&self) -> [u8; 8] {
            self.discriminator
        }

        fn get_event_type(&self) -> &'static str {
            self.event_type
        }

        async fn parse_from_logs(&self, _logs: &[String], _signature: &str, _slot: u64) -> Result<Option<ParsedEvent>> {
            // Mock实现
            Ok(None)
        }

        async fn validate_event(&self, _event: &ParsedEvent) -> Result<bool> {
            Ok(true)
        }
    }

    #[test]
    fn test_parsed_event_types() {
        let event = ParsedEvent::TokenCreation(TokenCreationEventData {
            mint_address: solana_sdk::pubkey::Pubkey::new_unique(),
            name: "Test Token".to_string(),
            symbol: "TEST".to_string(),
            uri: "https://example.com/metadata.json".to_string(),
            decimals: 9,
            supply: 1000000,
            creator: solana_sdk::pubkey::Pubkey::new_unique(),
            has_whitelist: false,
            whitelist_deadline: 0,
            created_at: 1234567890,
            signature: "test_signature".to_string(),
            slot: 12345,
        });

        assert_eq!(event.event_type(), "token_creation");
    }

    #[tokio::test]
    async fn test_registry_creation() {
        let config = crate::config::EventListenerConfig {
            solana: crate::config::settings::SolanaConfig {
                rpc_url: "https://api.devnet.solana.com".to_string(),
                ws_url: "wss://api.devnet.solana.com".to_string(),
                commitment: "confirmed".to_string(),
                program_id: solana_sdk::pubkey::Pubkey::new_unique(),
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
        };

        let registry = EventParserRegistry::new(&config).unwrap();
        assert!(registry.parser_count() > 0);
        
        let parsers = registry.get_registered_parsers();
        assert!(!parsers.is_empty());
    }

    #[tokio::test]
    async fn test_parser_registration() {
        let config = crate::config::EventListenerConfig {
            solana: crate::config::settings::SolanaConfig {
                rpc_url: "https://api.devnet.solana.com".to_string(),
                ws_url: "wss://api.devnet.solana.com".to_string(),
                commitment: "confirmed".to_string(),
                program_id: solana_sdk::pubkey::Pubkey::new_unique(),
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
        };

        let mut registry = EventParserRegistry::new(&config).unwrap();
        let initial_count = registry.parser_count();

        // 注册新的mock解析器
        let mock_parser = Box::new(MockParser {
            discriminator: [1, 2, 3, 4, 5, 6, 7, 8],
            event_type: "mock_event",
        });

        registry.register_parser(mock_parser).unwrap();
        assert_eq!(registry.parser_count(), initial_count + 1);

        // 尝试注册相同discriminator的解析器应该失败
        let duplicate_parser = Box::new(MockParser {
            discriminator: [1, 2, 3, 4, 5, 6, 7, 8],
            event_type: "duplicate_event",
        });

        assert!(registry.register_parser(duplicate_parser).is_err());
    }

    #[tokio::test]
    async fn test_registry_with_all_parsers() {
        let config = crate::config::EventListenerConfig {
            solana: crate::config::settings::SolanaConfig {
                rpc_url: "https://api.devnet.solana.com".to_string(),
                ws_url: "wss://api.devnet.solana.com".to_string(),
                commitment: "confirmed".to_string(),
                program_id: solana_sdk::pubkey::Pubkey::new_unique(),
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
        };

        let registry = EventParserRegistry::new(&config).unwrap();
        
        // 应该有四个解析器：token_creation、pool_creation、nft_claim、reward_distribution
        assert_eq!(registry.parser_count(), 4);
        
        let parsers = registry.get_registered_parsers();
        let parser_types: Vec<String> = parsers.iter().map(|(name, _)| name.clone()).collect();
        
        assert!(parser_types.contains(&"token_creation".to_string()));
        assert!(parser_types.contains(&"pool_creation".to_string()));
        assert!(parser_types.contains(&"nft_claim".to_string()));
        assert!(parser_types.contains(&"reward_distribution".to_string()));
    }

    #[tokio::test]
    async fn test_parse_event_with_context() {
        let config = crate::config::EventListenerConfig {
            solana: crate::config::settings::SolanaConfig {
                rpc_url: "https://api.devnet.solana.com".to_string(),
                ws_url: "wss://api.devnet.solana.com".to_string(),
                commitment: "confirmed".to_string(),
                program_id: solana_sdk::pubkey::Pubkey::new_unique(),
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
        };

        let registry = EventParserRegistry::new(&config).unwrap();
        
        // 测试无程序数据日志的情况
        let logs = vec![
            "Program 11111111111111111111111111111111 invoke [1]".to_string(),
            "Program 11111111111111111111111111111111 success".to_string(),
        ];
        
        let result = registry.parse_event_with_context(&logs, "test_sig", 12345).await.unwrap();
        assert!(result.is_none());
        
        // 测试无效的程序数据
        let logs_with_invalid_data = vec![
            "Program data: invalid_base64_data".to_string(),
        ];
        
        let result = registry.parse_event_with_context(&logs_with_invalid_data, "test_sig", 12345).await;
        // 应该失败或者返回 None
        match result {
            Ok(None) => {}, // 正常情况
            Err(_) => {}, // 也可能失败
            _ => panic!("应该返回None或错误"),
        }
    }
}