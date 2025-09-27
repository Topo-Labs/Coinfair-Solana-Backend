use crate::{
    config::EventListenerConfig,
    error::{EventListenerError, Result},
    parser::{event_parser::TokenCreationEventData, EventParser, ParsedEvent},
};
use async_trait::async_trait;
use borsh::{BorshDeserialize, BorshSerialize};
use database::clmm::token_info::{DataSource, TokenInfo, TokenInfoRepository};
use mongodb::Client;
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use std::sync::Arc;
use tracing::{info, warn};

/// 代币创建事件的原始数据结构（与智能合约保持一致）
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
pub struct TokenCreationEvent {
    /// 项目配置地址
    pub project_config: Pubkey,
    /// 代币的 Mint 地址
    pub mint_address: Pubkey,
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
    pub creator: Pubkey,
    /// 是否支持白名单（true 表示有白名单机制）
    pub has_whitelist: bool,
    /// 白名单资格检查的时间戳（Unix 时间戳，0 表示无时间限制）
    pub whitelist_deadline: i64,
    /// 创建时间（Unix 时间戳）
    pub created_at: i64,
}

/// 代币创建事件解析器
pub struct TokenCreationParser {
    /// 事件的discriminator（8字节标识符）
    discriminator: [u8; 8],
    /// 目标程序ID，指定此解析器处理哪个程序的事件
    target_program_id: Pubkey,
    /// 数据库仓库
    token_repository: Option<Arc<TokenInfoRepository>>,
}

impl TokenCreationParser {
    /// 创建新的代币创建事件解析器
    pub fn new(_config: &EventListenerConfig, program_id: Pubkey) -> Result<Self> {
        // 根据设计文档，使用事件类型名称计算discriminator
        let discriminator = crate::parser::event_parser::calculate_event_discriminator("TokenCreationEvent");

        Ok(Self {
            discriminator,
            target_program_id: program_id,
            token_repository: None,
        })
    }

    /// 初始化数据库连接
    pub async fn init_database(&mut self, config: &EventListenerConfig) -> Result<()> {
        let client = Client::with_uri_str(&config.database.uri)
            .await
            .map_err(|e| EventListenerError::Database(e))?;

        let database = client.database(&config.database.database_name);
        let collection = database.collection::<TokenInfo>("token_info");
        let repository = Arc::new(TokenInfoRepository::new(collection));

        // 初始化数据库索引
        repository
            .init_indexes()
            .await
            .map_err(|e| EventListenerError::Persistence(format!("初始化数据库索引失败: {}", e)))?;

        self.token_repository = Some(repository);
        info!("✅ 代币创建解析器数据库初始化完成");
        Ok(())
    }

    /// 从程序数据解析代币创建事件
    fn parse_program_data(&self, data_str: &str) -> Result<TokenCreationEvent> {
        use tracing::info;

        info!("🔍 开始解析Program data: {}", &data_str[..50.min(data_str.len())]);

        // 解码Base64数据
        use base64::{engine::general_purpose, Engine as _};
        let data = general_purpose::STANDARD
            .decode(data_str)
            .map_err(|e| EventListenerError::EventParsing(format!("Base64解码失败: {}", e)))?;

        info!("🔍 解码后数据长度: {} bytes", data.len());

        if data.len() < 8 {
            info!("❌ 数据长度不足，无法包含discriminator: {} < 8", data.len());
            return Err(EventListenerError::EventParsing(
                "数据长度不足，无法包含discriminator".to_string(),
            ));
        }

        // 验证discriminator
        let discriminator = &data[0..8];
        if discriminator != self.discriminator {
            return Err(EventListenerError::DiscriminatorMismatch);
        }
        info!("✅ Discriminator匹配:{}，开始反序列化", self.get_event_type());

        // 反序列化事件数据
        let event_data = &data[8..];
        info!("🔍 事件数据长度: {} bytes", event_data.len());

        let token_create_event = TokenCreationEvent::try_from_slice(event_data)?;
        info!("🔍 token_create_event: {:?}", token_create_event);
        Ok(token_create_event)
    }

    /// 将原始事件转换为ParsedEvent
    async fn convert_to_parsed_event(
        &self,
        event: TokenCreationEvent,
        signature: String,
        slot: u64,
    ) -> Result<ParsedEvent> {
        // 从URI获取代币元数据
        let uri_metadata = self.fetch_uri_metadata(&event.uri).await?;
        // 构建extensions JSON，包含项目配置和URI元数据
        let mut extensions = serde_json::json!({
            "project_config": event.project_config.to_string(),
            "creator": event.creator.to_string(),
            "total_raised": 0u64,
            "project_state": 3,
        });
        let mut logo_uri = String::new();
        // 如果成功获取URI元数据，添加到extensions中
        if let Some(metadata) = &uri_metadata {
            if let Some(description) = &metadata.description {
                extensions["description"] = serde_json::Value::String(description.clone());
            }
            if let Some(log_url) = &metadata.avatar_url {
                logo_uri = log_url.clone();
                extensions["log_url"] = serde_json::Value::String(log_url.clone());
            }
            if let Some(social_links) = &metadata.social_links {
                extensions["social_links"] = serde_json::to_value(social_links).unwrap_or_default();
            }
            if let Some(whitelist) = &metadata.whitelist {
                extensions["whitelist"] = serde_json::to_value(whitelist).unwrap_or_default();
            }
            if let Some(crowdfunding) = &metadata.crowdfunding {
                extensions["crowdfunding"] = serde_json::to_value(crowdfunding).unwrap_or_default();
            }
        }

        Ok(ParsedEvent::TokenCreation(TokenCreationEventData {
            project_config: event.project_config.to_string(),
            mint_address: event.mint_address.to_string(),
            name: event.name,
            symbol: event.symbol,
            metadata_uri: event.uri,
            logo_uri,
            decimals: event.decimals,
            supply: event.supply,
            creator: event.creator.to_string(),
            has_whitelist: event.has_whitelist,
            whitelist_deadline: event.whitelist_deadline,
            created_at: event.created_at,
            signature,
            slot,
            extensions: Some(extensions),
            source: Some(DataSource::OnchainSync),
        }))
    }

    /// 验证代币创建事件数据
    fn validate_token_creation(&self, event: &TokenCreationEventData) -> Result<bool> {
        // 验证代币名称
        if event.name.trim().is_empty() {
            warn!("⚠️ 代币名称为空: {}", event.mint_address);
            return Ok(false);
        }

        // 验证代币符号
        if event.symbol.trim().is_empty() {
            warn!("⚠️ 代币符号为空: {}", event.mint_address);
            return Ok(false);
        }

        // 验证URI格式
        if !event.metadata_uri.starts_with("http")
            && !event.metadata_uri.starts_with("ipfs://")
            && !event.metadata_uri.starts_with("ar://")
        {
            warn!("⚠️ 无效的URI格式: {} ({})", event.metadata_uri, event.mint_address);
        }

        // 验证小数位数
        if event.decimals > 18 {
            warn!("⚠️ 小数位数过大: {} ({})", event.decimals, event.mint_address);
            return Ok(false);
        }

        // 验证供应量
        if event.supply == 0 {
            warn!("⚠️ 供应量为0: {}", event.mint_address);
        }

        // 验证时间戳
        if event.created_at <= 0 {
            warn!("⚠️ 无效的创建时间: {} ({})", event.created_at, event.mint_address);
            return Ok(false);
        }

        // 验证白名单截止时间
        if event.has_whitelist && event.whitelist_deadline <= 0 {
            warn!(
                "⚠️ 启用白名单但截止时间无效: {} ({})",
                event.whitelist_deadline, event.mint_address
            );
        }

        Ok(true)
    }

    /// 从URI获取代币元数据
    async fn fetch_uri_metadata(&self, uri: &str) -> Result<Option<utils::metaplex_service::UriMetadata>> {
        use utils::metaplex_service::{MetaplexConfig, MetaplexService};

        // 创建MetaplexService实例
        let config = MetaplexConfig::default();
        let metaplex_service = MetaplexService::new(Some(config))
            .map_err(|e| EventListenerError::Persistence(format!("创建MetaplexService失败: {}", e)))?;

        // 尝试从URI获取元数据
        match metaplex_service.fetch_metadata_from_uri(uri).await {
            Ok(metadata) => {
                // info!("🔍 metadata: {:?}", metadata);
                Ok(metadata)
            }
            Err(e) => {
                warn!("⚠️ 从URI获取元数据失败: {} - {}", uri, e);
                Ok(None)
            }
        }
    }
}

#[async_trait]
impl EventParser for TokenCreationParser {
    fn get_program_id(&self) -> Pubkey {
        self.target_program_id
    }

    fn get_discriminator(&self) -> [u8; 8] {
        self.discriminator
    }

    fn get_event_type(&self) -> &'static str {
        "token_creation"
    }

    fn supports_program(&self, program_id: &Pubkey) -> Option<bool> {
        Some(*program_id == self.target_program_id)
    }

    async fn parse_from_logs(&self, logs: &[String], signature: &str, slot: u64) -> Result<Option<ParsedEvent>> {
        for log in logs {
            if log.starts_with("Program data: ") {
                if let Some(data_part) = log.strip_prefix("Program data: ") {
                    match self.parse_program_data(data_part) {
                        Ok(event) => {
                            let parsed_event = self.convert_to_parsed_event(event, signature.to_string(), slot).await?;
                            return Ok(Some(parsed_event));
                        }
                        Err(EventListenerError::DiscriminatorMismatch) => {
                            // Discriminator不匹配是正常情况，继续尝试下一条日志
                            continue;
                        }
                        Err(e) => {
                            // 其他错误需要记录
                            warn!("❌ 解析程序数据失败: {}", e);
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
            ParsedEvent::TokenCreation(token_event) => self.validate_token_creation(token_event),
            _ => Ok(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_sdk::pubkey;
    use solana_sdk::pubkey::Pubkey;

    fn create_test_config() -> EventListenerConfig {
        EventListenerConfig {
            solana: crate::config::settings::SolanaConfig {
                rpc_url: "https://api.devnet.solana.com".to_string(),
                ws_url: "wss://api.devnet.solana.com".to_string(),
                commitment: "confirmed".to_string(),
                program_ids: vec![pubkey!("FA1RJDDXysgwg5Gm3fJXWxt26JQzPkAzhTA114miqNUX")],
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
            backfill: None,
        }
    }

    fn create_test_token_creation_event() -> TokenCreationEvent {
        TokenCreationEvent {
            project_config: Pubkey::new_unique(),
            mint_address: Pubkey::new_unique(),
            name: "Test Token".to_string(),
            symbol: "TEST".to_string(),
            uri: "https://example.com/metadata.json".to_string(),
            decimals: 9,
            supply: 1000000,
            creator: Pubkey::new_unique(),
            has_whitelist: false,
            whitelist_deadline: 0,
            created_at: 1234567890,
        }
    }

    #[test]
    fn test_token_creation_parser_creation() {
        let config = create_test_config();
        let parser = TokenCreationParser::new(&config, Pubkey::new_unique()).unwrap();

        assert_eq!(parser.get_event_type(), "token_creation");
        // assert_eq!(parser.get_discriminator(), [142, 175, 175, 21, 74, 229, 126, 116]);
        assert_eq!(
            parser.get_discriminator(),
            crate::parser::event_parser::calculate_event_discriminator("TokenCreationEvent")
        );
    }

    #[tokio::test]
    async fn test_convert_to_parsed_event() {
        let config = create_test_config();
        let parser = TokenCreationParser::new(&config, Pubkey::new_unique()).unwrap();
        let test_event = create_test_token_creation_event();

        let parsed = parser
            .convert_to_parsed_event(test_event.clone(), "test_signature".to_string(), 12345)
            .await
            .unwrap();

        match parsed {
            ParsedEvent::TokenCreation(data) => {
                assert_eq!(data.mint_address, test_event.mint_address.to_string());
                assert_eq!(data.name, test_event.name);
                assert_eq!(data.symbol, test_event.symbol);
                assert_eq!(data.signature, "test_signature");
                assert_eq!(data.slot, 12345);
            }
            _ => panic!("期望TokenCreation事件"),
        }
    }

    #[tokio::test]
    async fn test_validate_token_creation() {
        let config = create_test_config();
        let parser = TokenCreationParser::new(&config, Pubkey::new_unique()).unwrap();

        let valid_event = TokenCreationEventData {
            project_config: Pubkey::new_unique().to_string(),
            mint_address: Pubkey::new_unique().to_string(),
            name: "Valid Token".to_string(),
            symbol: "VALID".to_string(),
            metadata_uri: "https://example.com/metadata.json".to_string(),
            logo_uri: "https://example.com/logo.png".to_string(),
            decimals: 9,
            supply: 1000000,
            creator: Pubkey::new_unique().to_string(),
            has_whitelist: false,
            whitelist_deadline: 0,
            created_at: 1234567890,
            signature: "test_sig".to_string(),
            slot: 12345,
            extensions: None,
            source: None,
        };

        assert!(parser.validate_token_creation(&valid_event).unwrap());

        // 测试无效事件（空名称）
        let invalid_event = TokenCreationEventData {
            name: "".to_string(),
            ..valid_event.clone()
        };

        assert!(!parser.validate_token_creation(&invalid_event).unwrap());
    }

    #[test]
    fn test_project_config_field() {
        let test_event = create_test_token_creation_event();

        // 验证project_config字段存在且不为空
        assert_ne!(test_event.project_config, Pubkey::default());
    }

    #[tokio::test]
    async fn test_uri_metadata_fetch() {
        let config = create_test_config();
        let parser = TokenCreationParser::new(&config, Pubkey::new_unique()).unwrap();

        // 测试有效的HTTP URI
        let http_uri = "https://raw.githubusercontent.com/solana-labs/token-list/main/assets/mainnet/EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v/logo.png";
        let result = parser.fetch_uri_metadata(http_uri).await;
        // 这个测试可能因为网络原因失败，但不应该导致程序崩溃
        assert!(result.is_ok());

        // 测试无效的URI
        let invalid_uri = "invalid-uri";
        let result = parser.fetch_uri_metadata(invalid_uri).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    // #[tokio::test]
    // async fn test_enhanced_persist_token_creation() {
    //     let config = create_test_config();
    //     let mut parser = TokenCreationParser::new(&config, Pubkey::new_unique()).unwrap();

    //     // 尝试初始化数据库（如果失败就跳过测试）
    //     if parser.init_database(&config).await.is_err() {
    //         return; // 跳过测试，因为没有数据库连接
    //     }

    //     let test_event = TokenCreationEventData {
    //         project_config: Pubkey::new_unique().to_string(),
    //         mint_address: Pubkey::new_unique().to_string(),
    //         name: "Enhanced Test Token".to_string(),
    //         symbol: "ENHANCED".to_string(),
    //         uri: "https://example.com/metadata.json".to_string(),
    //         decimals: 9,
    //         supply: 1000000000,
    //         creator: Pubkey::new_unique().to_string(),
    //         has_whitelist: true,
    //         whitelist_deadline: 1700000000,
    //         created_at: 1234567890,
    //         signature: "enhanced_test_signature".to_string(),
    //         slot: 54321,
    //         extensions: None,
    //         source: None,
    //     };

    //     // 测试持久化过程
    //     let result = parser.persist_token_creation(&test_event).await;
    //     match result {
    //         Ok(_) => {
    //             println!("✅ 增强的持久化测试成功");
    //         }
    //         Err(e) => {
    //             println!("⚠️ 持久化测试失败，可能是数据库连接问题: {}", e);
    //         }
    //     }
    // }

    #[test]
    fn test_borsh_serialization() {
        let event = create_test_token_creation_event();

        // 测试序列化
        let serialized = borsh::to_vec(&event).unwrap();
        assert!(!serialized.is_empty());

        // 测试反序列化
        let deserialized = TokenCreationEvent::try_from_slice(&serialized).unwrap();
        assert_eq!(deserialized.mint_address, event.mint_address);
        assert_eq!(deserialized.name, event.name);
        assert_eq!(deserialized.symbol, event.symbol);
    }

    #[tokio::test]
    async fn test_parse_from_logs_no_program_data() {
        let config = create_test_config();
        let parser = TokenCreationParser::new(&config, Pubkey::new_unique()).unwrap();

        let logs = vec![
            "Program 11111111111111111111111111111111 invoke [1]".to_string(),
            "Program 11111111111111111111111111111111 success".to_string(),
        ];

        let result = parser.parse_from_logs(&logs, "test_sig", 12345).await.unwrap();
        assert!(result.is_none());
    }
}
