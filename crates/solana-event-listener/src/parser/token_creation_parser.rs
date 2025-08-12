use crate::{
    config::EventListenerConfig,
    error::{EventListenerError, Result},
    parser::{event_parser::TokenCreationEventData, EventParser, ParsedEvent},
};
use async_trait::async_trait;
use borsh::{BorshDeserialize, BorshSerialize};
use database::token_info::{DataSource, TokenInfo, TokenInfoRepository, TokenPushRequest};
use mongodb::Client;
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

/// 代币创建事件的原始数据结构（与智能合约保持一致）
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
pub struct TokenCreationEvent {
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
        // 代币创建事件的discriminator
        let discriminator = [142, 175, 175, 21, 74, 229, 126, 116];

        Ok(Self {
            discriminator,
            target_program_id: program_id,
            token_repository: None,
        })
    }

    /// 初始化数据库连接
    pub async fn init_database(&mut self, config: &EventListenerConfig) -> Result<()> {
        let client = Client::with_uri_str(&config.database.uri).await.map_err(|e| EventListenerError::Database(e))?;

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
            return Err(EventListenerError::EventParsing("数据长度不足，无法包含discriminator".to_string()));
        }

        // 验证discriminator
        let discriminator = &data[0..8];
        // info!("🔍 实际discriminator: {:?}", discriminator);
        // info!("🔍 期望discriminator: {:?}", self.discriminator);

        // // 将discriminator信息写入文件，便于调试
        // if let Err(e) = std::fs::write(
        //     "/tmp/discriminator_debug.txt",
        //     format!("实际discriminator: {:?}\n期望discriminator: {:?}\n", discriminator, self.discriminator),
        // ) {
        //     warn!("写入调试文件失败: {}", e);
        // }

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
    fn convert_to_parsed_event(&self, event: TokenCreationEvent, signature: String, slot: u64) -> ParsedEvent {
        ParsedEvent::TokenCreation(TokenCreationEventData {
            mint_address: event.mint_address.to_string(),
            name: event.name,
            symbol: event.symbol,
            uri: event.uri,
            decimals: event.decimals,
            supply: event.supply,
            creator: event.creator.to_string(),
            has_whitelist: event.has_whitelist,
            whitelist_deadline: event.whitelist_deadline,
            created_at: event.created_at,
            signature,
            slot,
        })
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
        if !event.uri.starts_with("http") && !event.uri.starts_with("ipfs://") && !event.uri.starts_with("ar://") {
            warn!("⚠️ 无效的URI格式: {} ({})", event.uri, event.mint_address);
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
            warn!("⚠️ 启用白名单但截止时间无效: {} ({})", event.whitelist_deadline, event.mint_address);
        }

        Ok(true)
    }

    /// 持久化代币创建事件到数据库
    pub async fn persist_token_creation(&self, event: &TokenCreationEventData) -> Result<()> {
        let repository = self
            .token_repository
            .as_ref()
            .ok_or_else(|| EventListenerError::Persistence("数据库未初始化".to_string()))?;

        // 构建TokenPushRequest
        let push_request = TokenPushRequest {
            address: event.mint_address.to_string(),
            program_id: Some("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA".to_string()),
            name: event.name.clone(),
            symbol: event.symbol.clone(),
            decimals: event.decimals,
            logo_uri: event.uri.clone(),
            tags: Some(vec!["meme".to_string(), "new".to_string()]),
            daily_volume: Some(0.0),
            freeze_authority: None,
            mint_authority: Some(event.creator.to_string()),
            permanent_delegate: None,
            minted_at: Some(chrono::DateTime::from_timestamp(event.created_at, 0).unwrap_or_else(|| chrono::Utc::now())),
            extensions: Some(serde_json::json!({
                "supply": event.supply,
                "has_whitelist": event.has_whitelist,
                "whitelist_deadline": event.whitelist_deadline,
                "signature": event.signature,
                "slot": event.slot
            })),
            source: Some(DataSource::OnchainSync),
        };

        // 推送到数据库
        let response = repository
            .push_token(push_request)
            .await
            .map_err(|e| EventListenerError::Persistence(format!("推送代币信息失败: {}", e)))?;

        if response.success {
            info!("✅ 代币创建事件已持久化: {} ({}) - {}", event.symbol, event.mint_address, response.operation);
        } else {
            error!("❌ 代币创建事件持久化失败: {} ({})", event.symbol, event.mint_address);
            return Err(EventListenerError::Persistence(response.message));
        }

        Ok(())
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
                            let parsed_event = self.convert_to_parsed_event(event, signature.to_string(), slot);
                            return Ok(Some(parsed_event));
                        }
                        Err(EventListenerError::DiscriminatorMismatch) => {
                            // Discriminator不匹配是正常情况，继续尝试下一条日志
                            continue;
                        }
                        Err(e) => {
                            // 其他错误需要记录
                            debug!("解析程序数据失败: {}", e);
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
        }
    }

    fn create_test_token_creation_event() -> TokenCreationEvent {
        TokenCreationEvent {
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
        assert_eq!(parser.get_discriminator(), [142, 175, 175, 21, 74, 229, 126, 116]);
    }

    #[test]
    fn test_convert_to_parsed_event() {
        let config = create_test_config();
        let parser = TokenCreationParser::new(&config, Pubkey::new_unique()).unwrap();
        let test_event = create_test_token_creation_event();

        let parsed = parser.convert_to_parsed_event(test_event.clone(), "test_signature".to_string(), 12345);

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
            mint_address: Pubkey::new_unique().to_string(),
            name: "Valid Token".to_string(),
            symbol: "VALID".to_string(),
            uri: "https://example.com/metadata.json".to_string(),
            decimals: 9,
            supply: 1000000,
            creator: Pubkey::new_unique().to_string(),
            has_whitelist: false,
            whitelist_deadline: 0,
            created_at: 1234567890,
            signature: "test_sig".to_string(),
            slot: 12345,
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
