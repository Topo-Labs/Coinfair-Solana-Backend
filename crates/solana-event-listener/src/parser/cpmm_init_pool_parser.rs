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
use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use tracing::{debug, info, warn};

/// 池子初始化事件的原始数据结构（与CPMM智能合约保持一致）
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct InitPoolEvent {
    /// 池子ID
    pub pool_id: Pubkey,
    /// 池子创建者
    pub pool_creator: Pubkey,
    /// token_0的mint地址
    pub token_0_mint: Pubkey,
    /// token_1的mint地址
    pub token_1_mint: Pubkey,
    /// token_0的vault地址
    pub token_0_vault: Pubkey,
    /// token_1的vault地址
    pub token_1_vault: Pubkey,
    /// LP代币的程序ID
    pub lp_program_id: Pubkey,
    /// LP代币的mint地址
    pub lp_mint: Pubkey,
    /// LP代币精度
    pub decimals: u8,
}

/// 池子初始化事件数据（用于事件监听器解析结果）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitPoolEventData {
    // 池子信息
    pub pool_id: String,
    pub pool_creator: String,
    pub token_0_mint: String,
    pub token_1_mint: String,
    pub token_0_vault: String,
    pub token_1_vault: String,
    pub lp_mint: String,

    // 程序ID和精度信息
    pub lp_program_id: String,
    pub token_0_program_id: String, // 需要从链上获取
    pub token_1_program_id: String, // 需要从链上获取
    pub lp_mint_decimals: u8,       // 使用事件中的decimals
    pub token_0_decimals: u8,       // 需要从链上获取
    pub token_1_decimals: u8,       // 需要从链上获取

    // 交易信息
    pub signature: String,
    pub slot: u64,
    pub processed_at: String,
}

/// 池子初始化事件解析器
pub struct InitPoolParser {
    /// 事件的discriminator（需要从合约IDL获取）
    discriminator: [u8; 8],
    /// 目标程序ID
    target_program_id: Pubkey,
    /// RPC客户端（用于查询链上数据）
    rpc_client: RpcClient,
}

impl InitPoolParser {
    /// 创建新的池子初始化事件解析器
    pub fn new(config: &EventListenerConfig, program_id: Pubkey) -> Result<Self> {
        let discriminator = crate::parser::event_parser::calculate_event_discriminator("InitPoolEvent");

        // 初始化RPC客户端
        // let rpc_url = std::env::var("RPC_URL").unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".to_string());
        let rpc_client = RpcClient::new(config.solana.rpc_url.clone());

        info!(
            "✅ 创建InitPoolParser: 程序ID={}, discriminator={:?}",
            program_id, discriminator
        );

        Ok(Self {
            discriminator,
            target_program_id: program_id,
            rpc_client,
        })
    }

    /// 从程序数据解析池子初始化事件
    fn parse_program_data(&self, data_str: &str) -> Result<InitPoolEvent> {
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
        let event = InitPoolEvent::try_from_slice(event_data)
            .map_err(|e| EventListenerError::EventParsing(format!("Borsh反序列化失败: {}", e)))?;

        debug!(
            "✅ 成功解析池子初始化事件: pool_id={}, creator={}",
            event.pool_id, event.pool_creator
        );

        Ok(event)
    }

    /// 从链上获取缺失的信息（使用批量查询优化性能，带重试机制）
    async fn fetch_missing_info(
        &self,
        token_0_mint: &Pubkey,
        token_1_mint: &Pubkey,
    ) -> Result<(String, String, u8, u8)> {
        const MAX_RETRIES: u32 = 3;
        const RETRY_DELAY_MS: u64 = 2000;

        for attempt in 1..=MAX_RETRIES {
            match self.try_fetch_token_info(token_0_mint, token_1_mint).await {
                Ok(result) => {
                    debug!(
                        "✅ 第{}次尝试成功获取token信息: token_0_decimals={}, token_1_decimals={}",
                        attempt, result.2, result.3
                    );
                    return Ok(result);
                }
                Err(e) => {
                    warn!("⚠️ 第{}次尝试获取token信息失败: {}", attempt, e);

                    if attempt < MAX_RETRIES {
                        debug!("📡 {}ms后重试...", RETRY_DELAY_MS);
                        tokio::time::sleep(tokio::time::Duration::from_millis(RETRY_DELAY_MS)).await;
                    } else {
                        return Err(e);
                    }
                }
            }
        }

        Err(EventListenerError::SolanaRpc(
            "达到最大重试次数，仍无法获取token信息".to_string(),
        ))
    }

    /// 实际获取token信息的核心逻辑
    async fn try_fetch_token_info(
        &self,
        token_0_mint: &Pubkey,
        token_1_mint: &Pubkey,
    ) -> Result<(String, String, u8, u8)> {
        // 批量获取两个token账户的信息
        let accounts = self
            .rpc_client
            .get_multiple_accounts(&[*token_0_mint, *token_1_mint])
            .map_err(|e| EventListenerError::SolanaRpc(format!("批量获取token账户失败: {}", e)))?;

        // 处理token_0账户
        let token_0_account = accounts[0]
            .as_ref()
            .ok_or_else(|| EventListenerError::SolanaRpc("token_0账户不存在".to_string()))?;

        // 处理token_1账户
        let token_1_account = accounts[1]
            .as_ref()
            .ok_or_else(|| EventListenerError::SolanaRpc("token_1账户不存在".to_string()))?;

        // 从账户数据中提取程序ID
        let token_0_program_id = token_0_account.owner.to_string();
        let token_1_program_id = token_1_account.owner.to_string();

        // 解析token精度信息
        let token_0_decimals = self.parse_token_decimals(token_0_account, token_0_mint)?;
        let token_1_decimals = self.parse_token_decimals(token_1_account, token_1_mint)?;

        // 验证程序ID是否为已知的Token程序
        self.validate_token_program(&token_0_program_id)?;
        self.validate_token_program(&token_1_program_id)?;

        Ok((
            token_0_program_id,
            token_1_program_id,
            token_0_decimals,
            token_1_decimals,
        ))
    }

    /// 解析token的精度信息
    fn parse_token_decimals(&self, account: &solana_sdk::account::Account, mint_pubkey: &Pubkey) -> Result<u8> {
        // SPL Token和Token-2022程序的Mint账户数据布局：
        // - 前36字节：供应量和其他字段
        // - 第36字节：mint_authority_option (1字节)
        // - 第37-68字节：mint_authority (32字节，如果存在)
        // - 第69字节：supply (8字节)
        // - 第77字节：decimals (1字节)
        //
        // 但实际上，decimals在第44字节的位置，这是经过验证的

        if account.data.len() < 45 {
            warn!(
                "⚠️ Token mint账户数据长度不足: {} bytes, mint: {}",
                account.data.len(),
                mint_pubkey
            );
            return Ok(9); // 默认精度
        }

        let decimals = account.data[44];

        // 验证精度值的合理性
        if decimals > 18 {
            warn!(
                "⚠️ Token精度值异常: {} decimals, mint: {}, 使用默认值9",
                decimals, mint_pubkey
            );
            return Ok(9);
        }

        debug!("✅ 解析token精度: {} decimals, mint: {}", decimals, mint_pubkey);
        Ok(decimals)
    }

    /// 验证token程序ID是否为已知的合法程序
    fn validate_token_program(&self, program_id: &str) -> Result<()> {
        const SPL_TOKEN_PROGRAM_ID: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
        const SPL_TOKEN_2022_PROGRAM_ID: &str = "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb";

        match program_id {
            SPL_TOKEN_PROGRAM_ID => {
                debug!("✅ 检测到SPL Token程序");
                Ok(())
            }
            SPL_TOKEN_2022_PROGRAM_ID => {
                debug!("✅ 检测到SPL Token 2022程序");
                Ok(())
            }
            _ => {
                warn!("⚠️ 未知的token程序ID: {}", program_id);
                // 不抛出错误，允许未知程序ID通过
                Ok(())
            }
        }
    }

    /// 将原始事件转换为ParsedEvent
    async fn convert_to_parsed_event(&self, event: InitPoolEvent, signature: String, slot: u64) -> Result<ParsedEvent> {
        // 从链上获取缺失的信息
        let (token_0_program_id, token_1_program_id, token_0_decimals, token_1_decimals) = self
            .fetch_missing_info(&event.token_0_mint, &event.token_1_mint)
            .await?;

        let init_pool_event = InitPoolEventData {
            pool_id: event.pool_id.to_string(),
            pool_creator: event.pool_creator.to_string(),
            token_0_mint: event.token_0_mint.to_string(),
            token_1_mint: event.token_1_mint.to_string(),
            token_0_vault: event.token_0_vault.to_string(),
            token_1_vault: event.token_1_vault.to_string(),
            lp_mint: event.lp_mint.to_string(),

            lp_program_id: event.lp_program_id.to_string(),
            token_0_program_id,
            token_1_program_id,
            lp_mint_decimals: event.decimals,
            token_0_decimals,
            token_1_decimals,

            signature,
            slot,
            processed_at: Utc::now().to_rfc3339(),
        };

        Ok(ParsedEvent::InitPool(init_pool_event))
    }

    /// 验证池子初始化事件数据（全面验证所有字段）
    fn validate_init_pool_event(&self, event: &InitPoolEventData) -> Result<bool> {
        let mut validation_errors = Vec::new();

        // 验证所有Pubkey格式的字段
        let pubkey_fields = vec![
            ("pool_id", &event.pool_id),
            ("pool_creator", &event.pool_creator),
            ("token_0_mint", &event.token_0_mint),
            ("token_1_mint", &event.token_1_mint),
            ("token_0_vault", &event.token_0_vault),
            ("token_1_vault", &event.token_1_vault),
            ("lp_mint", &event.lp_mint),
            ("lp_program_id", &event.lp_program_id),
            ("token_0_program_id", &event.token_0_program_id),
            ("token_1_program_id", &event.token_1_program_id),
        ];

        for (field_name, field_value) in pubkey_fields {
            if field_value.trim().is_empty() {
                validation_errors.push(format!("{} 字段为空", field_name));
                continue;
            }

            // 验证Pubkey格式
            if let Err(_) = field_value.parse::<Pubkey>() {
                validation_errors.push(format!("{} 不是有效的Pubkey格式: {}", field_name, field_value));
            }
        }

        // 验证精度范围
        let decimals_fields = vec![
            ("lp_mint_decimals", event.lp_mint_decimals),
            ("token_0_decimals", event.token_0_decimals),
            ("token_1_decimals", event.token_1_decimals),
        ];

        for (field_name, decimals) in decimals_fields {
            if decimals > 18 {
                validation_errors.push(format!("{} 超出合理范围(0-18): {}", field_name, decimals));
            }
        }

        // 验证交易签名格式（Base58格式，长度应为88）
        if event.signature.trim().is_empty() {
            validation_errors.push("交易签名为空".to_string());
        } else if event.signature.len() != 88 {
            validation_errors.push(format!(
                "交易签名长度异常: 期望88字符，实际{}字符",
                event.signature.len()
            ));
        } else {
            // 验证Base58格式
            if let Err(_) = bs58::decode(&event.signature).into_vec() {
                validation_errors.push("交易签名不是有效的Base58格式".to_string());
            }
        }

        // 验证slot值
        if event.slot == 0 {
            validation_errors.push(format!("无效的slot值: {}", event.slot));
        }

        // 验证token不能相同
        if event.token_0_mint == event.token_1_mint {
            validation_errors.push("token_0_mint和token_1_mint不能相同".to_string());
        }

        // 验证vault不能相同
        if event.token_0_vault == event.token_1_vault {
            validation_errors.push("token_0_vault和token_1_vault不能相同".to_string());
        }

        // 验证processed_at时间戳格式
        if let Err(_) = chrono::DateTime::parse_from_rfc3339(&event.processed_at) {
            validation_errors.push("processed_at 不是有效的RFC3339时间格式".to_string());
        }

        // 输出验证结果
        if validation_errors.is_empty() {
            debug!("✅ 池子初始化事件验证通过: pool_id={}", event.pool_id);
            Ok(true)
        } else {
            warn!(
                "❌ 池子初始化事件验证失败: pool_id={}, 错误: {:?}",
                event.pool_id, validation_errors
            );
            Ok(false)
        }
    }

    /// 验证原始事件数据的业务逻辑
    fn validate_raw_event(&self, event: &InitPoolEvent) -> Result<bool> {
        // 验证LP mint不能与token mint相同
        if event.lp_mint == event.token_0_mint || event.lp_mint == event.token_1_mint {
            warn!(
                "❌ LP mint不能与token mint相同: lp_mint={}, token_0={}, token_1={}",
                event.lp_mint, event.token_0_mint, event.token_1_mint
            );
            return Ok(false);
        }

        // 验证token mint顺序（CPMM池子通常需要mint地址排序）
        if event.token_0_mint >= event.token_1_mint {
            warn!(
                "⚠️ Token mint顺序可能不正确: token_0={} >= token_1={}",
                event.token_0_mint, event.token_1_mint
            );
            // 这里不返回false，只是警告，因为不同的CPMM实现可能有不同的排序规则
        }

        debug!("✅ 原始事件业务逻辑验证通过: pool_id={}", event.pool_id);
        Ok(true)
    }
}

#[async_trait]
impl EventParser for InitPoolParser {
    fn get_program_id(&self) -> Pubkey {
        self.target_program_id
    }

    fn get_discriminator(&self) -> [u8; 8] {
        self.discriminator
    }

    fn get_event_type(&self) -> &'static str {
        "init_pool"
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
                                "🏊 第{}行发现池子初始化事件: pool_id={}, creator={}",
                                index + 1,
                                event.pool_id,
                                event.pool_creator
                            );

                            // 验证原始事件的业务逻辑
                            if !self.validate_raw_event(&event)? {
                                warn!("⚠️ 池子初始化事件未通过业务逻辑验证，跳过: pool_id={}", event.pool_id);
                                continue;
                            }

                            // 转换为ParsedEvent
                            let parsed_event = self.convert_to_parsed_event(event, signature.to_string(), slot).await?;

                            return Ok(Some(parsed_event));
                        }
                        Err(EventListenerError::DiscriminatorMismatch) => {
                            // Discriminator不匹配是正常情况，继续尝试下一条日志
                            continue;
                        }
                        Err(e) => {
                            debug!("⚠️ 第{}行池子初始化事件解析失败: {}", index + 1, e);
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
            ParsedEvent::InitPool(init_event) => self.validate_init_pool_event(init_event),
            _ => Ok(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::parser::token_creation_parser::TokenCreationEventData;

    use super::*;
    use solana_sdk::pubkey::Pubkey;

    fn create_test_config() -> crate::config::EventListenerConfig {
        crate::config::EventListenerConfig {
            solana: crate::config::settings::SolanaConfig {
                rpc_url: "https://api.devnet.solana.com".to_string(),
                ws_url: "wss://api.devnet.solana.com".to_string(),
                commitment: "confirmed".to_string(),
                program_ids: vec![],
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

    fn create_test_init_pool_event() -> InitPoolEvent {
        InitPoolEvent {
            pool_id: Pubkey::new_unique(),
            pool_creator: Pubkey::new_unique(),
            token_0_mint: Pubkey::new_unique(),
            token_1_mint: Pubkey::new_unique(),
            token_0_vault: Pubkey::new_unique(),
            token_1_vault: Pubkey::new_unique(),
            lp_program_id: Pubkey::new_unique(),
            lp_mint: Pubkey::new_unique(),
            decimals: 9,
        }
    }

    fn create_test_init_pool_event_data() -> InitPoolEventData {
        InitPoolEventData {
            pool_id: Pubkey::new_unique().to_string(),
            pool_creator: Pubkey::new_unique().to_string(),
            token_0_mint: Pubkey::new_unique().to_string(),
            token_1_mint: Pubkey::new_unique().to_string(),
            token_0_vault: Pubkey::new_unique().to_string(),
            token_1_vault: Pubkey::new_unique().to_string(),
            lp_mint: Pubkey::new_unique().to_string(),
            lp_program_id: Pubkey::new_unique().to_string(),
            token_0_program_id: "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA".to_string(),
            token_1_program_id: "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb".to_string(),
            lp_mint_decimals: 9,
            token_0_decimals: 6,
            token_1_decimals: 9,
            signature: "3PGKKiYqS6KJNcvS5KvHTZMiKF7RPTJdGXHFDwMHhJf5tDn1Zj4BhM5XgRcvNsF2kL6pYzCH8qR7eB9J3VfGKdAt"
                .to_string(),
            slot: 12345,
            processed_at: chrono::Utc::now().to_rfc3339(),
        }
    }

    #[test]
    fn test_init_pool_parser_creation() {
        let config = create_test_config();
        let program_id = Pubkey::new_unique();

        let parser = InitPoolParser::new(&config, program_id);
        assert!(parser.is_ok(), "InitPoolParser创建应该成功");

        let parser = parser.unwrap();
        assert_eq!(parser.get_program_id(), program_id);
        assert_eq!(parser.get_event_type(), "init_pool");

        let expected_discriminator = crate::parser::event_parser::calculate_event_discriminator("InitPoolEvent");
        assert_eq!(parser.get_discriminator(), expected_discriminator);

        println!("✅ InitPoolParser创建测试通过");
    }

    #[test]
    fn test_validate_init_pool_event() {
        let config = create_test_config();
        let program_id = Pubkey::new_unique();
        let parser = InitPoolParser::new(&config, program_id).unwrap();

        // 测试有效的事件数据
        let valid_event = create_test_init_pool_event_data();
        let result = parser.validate_init_pool_event(&valid_event);
        assert!(result.is_ok(), "有效的InitPoolEventData验证应该通过");
        assert!(result.unwrap(), "有效的InitPoolEventData应该返回true");

        // 测试无效的事件数据 - 空的pool_id
        let mut invalid_event = create_test_init_pool_event_data();
        invalid_event.pool_id = "".to_string();
        let result = parser.validate_init_pool_event(&invalid_event);
        assert!(result.is_ok(), "验证方法不应该抛出错误");
        assert!(!result.unwrap(), "无效的pool_id应该返回false");

        // 测试无效的事件数据 - 非法的Pubkey格式
        let mut invalid_event = create_test_init_pool_event_data();
        invalid_event.token_0_mint = "invalid_pubkey".to_string();
        let result = parser.validate_init_pool_event(&invalid_event);
        assert!(result.is_ok(), "验证方法不应该抛出错误");
        assert!(!result.unwrap(), "无效的Pubkey格式应该返回false");

        // 测试相同的token mint
        let mut invalid_event = create_test_init_pool_event_data();
        invalid_event.token_1_mint = invalid_event.token_0_mint.clone();
        let result = parser.validate_init_pool_event(&invalid_event);
        assert!(result.is_ok(), "验证方法不应该抛出错误");
        assert!(!result.unwrap(), "相同的token mint应该返回false");

        println!("✅ InitPoolEvent验证测试通过");
    }

    #[test]
    fn test_validate_raw_event() {
        let config = create_test_config();
        let program_id = Pubkey::new_unique();
        let parser = InitPoolParser::new(&config, program_id).unwrap();

        // 测试有效的原始事件
        let valid_event = create_test_init_pool_event();
        let result = parser.validate_raw_event(&valid_event);
        assert!(result.is_ok(), "有效的原始事件验证应该通过");
        assert!(result.unwrap(), "有效的原始事件应该返回true");

        // 测试LP mint与token mint相同的情况
        let mut invalid_event = create_test_init_pool_event();
        invalid_event.lp_mint = invalid_event.token_0_mint;
        let result = parser.validate_raw_event(&invalid_event);
        assert!(result.is_ok(), "验证方法不应该抛出错误");
        assert!(!result.unwrap(), "LP mint与token mint相同应该返回false");

        println!("✅ 原始事件验证测试通过");
    }

    #[test]
    fn test_validate_token_program() {
        let config = create_test_config();
        let program_id = Pubkey::new_unique();
        let parser = InitPoolParser::new(&config, program_id).unwrap();

        // 测试SPL Token程序
        let result = parser.validate_token_program("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
        assert!(result.is_ok(), "SPL Token程序验证应该通过");

        // 测试SPL Token 2022程序
        let result = parser.validate_token_program("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb");
        assert!(result.is_ok(), "SPL Token 2022程序验证应该通过");

        // 测试未知程序（应该允许）
        let result = parser.validate_token_program("AZxHQhxgjENmx8x9CQ8r86Eodo8Qg6H9wYiuRqbonaoH");
        assert!(result.is_ok(), "未知程序应该被允许");

        println!("✅ Token程序验证测试通过");
    }

    #[test]
    fn test_parse_token_decimals() {
        let config = create_test_config();
        let program_id = Pubkey::new_unique();
        let parser = InitPoolParser::new(&config, program_id).unwrap();

        // 创建一个模拟的token mint账户数据
        let mut account_data = vec![0u8; 82]; // SPL Token mint账户的标准大小
        account_data[44] = 6; // 在第44字节设置decimals为6

        let account = solana_sdk::account::Account {
            lamports: 1000000,
            data: account_data,
            owner: solana_sdk::system_program::ID,
            executable: false,
            rent_epoch: 0,
        };

        let mint_pubkey = Pubkey::new_unique();
        let result = parser.parse_token_decimals(&account, &mint_pubkey);
        assert!(result.is_ok(), "解析token decimals应该成功");
        assert_eq!(result.unwrap(), 6, "解析的decimals应该正确");

        // 测试数据长度不足的情况
        let short_data = vec![0u8; 40]; // 长度不足的数据
        let short_account = solana_sdk::account::Account {
            lamports: 1000000,
            data: short_data,
            owner: solana_sdk::system_program::ID,
            executable: false,
            rent_epoch: 0,
        };

        let result = parser.parse_token_decimals(&short_account, &mint_pubkey);
        assert!(result.is_ok(), "数据长度不足时应该返回默认值");
        assert_eq!(result.unwrap(), 9, "数据长度不足时应该返回默认值9");

        // 测试异常的decimals值
        let mut invalid_data = vec![0u8; 82];
        invalid_data[44] = 20; // 设置一个异常的decimals值

        let invalid_account = solana_sdk::account::Account {
            lamports: 1000000,
            data: invalid_data,
            owner: solana_sdk::system_program::ID,
            executable: false,
            rent_epoch: 0,
        };

        let result = parser.parse_token_decimals(&invalid_account, &mint_pubkey);
        assert!(result.is_ok(), "异常decimals值应该返回默认值");
        assert_eq!(result.unwrap(), 9, "异常decimals值应该返回默认值9");

        println!("✅ Token decimals解析测试通过");
    }

    #[tokio::test]
    async fn test_parse_from_logs_discriminator_mismatch() {
        let config = create_test_config();
        let program_id = Pubkey::new_unique();
        let parser = InitPoolParser::new(&config, program_id).unwrap();

        // 创建一个包含无效discriminator的日志
        let logs = vec![
            "Program data: aW52YWxpZF9kYXRhX3dpdGhfaW52YWxpZF9kaXNjcmltaW5hdG9y".to_string(), // 无效的discriminator
        ];

        let result = parser.parse_from_logs(&logs, "test_signature", 12345).await;
        assert!(result.is_ok(), "解析日志不应该出错");
        assert!(result.unwrap().is_none(), "discriminator不匹配时应该返回None");

        println!("✅ discriminator不匹配测试通过");
    }

    #[tokio::test]
    async fn test_validate_event_with_parsed_event() {
        let config = create_test_config();
        let program_id = Pubkey::new_unique();
        let parser = InitPoolParser::new(&config, program_id).unwrap();

        // 创建一个有效的ParsedEvent::InitPool
        let init_pool_data = create_test_init_pool_event_data();
        let parsed_event = ParsedEvent::InitPool(init_pool_data);

        let result = parser.validate_event(&parsed_event).await;
        assert!(result.is_ok(), "验证ParsedEvent应该成功");
        assert!(result.unwrap(), "有效的ParsedEvent::InitPool应该通过验证");

        // 测试其他类型的ParsedEvent
        let token_creation_data = TokenCreationEventData {
            project_config: Pubkey::new_unique().to_string(),
            mint_address: Pubkey::new_unique().to_string(),
            name: "Test Token".to_string(),
            symbol: "TEST".to_string(),
            metadata_uri: "https://example.com/metadata.json".to_string(),
            logo_uri: "https://example.com/logo.png".to_string(),
            decimals: 9,
            supply: 1000000,
            creator: Pubkey::new_unique().to_string(),
            has_whitelist: false,
            whitelist_deadline: 0,
            created_at: 1234567890,
            signature: "test_signature".to_string(),
            slot: 12345,
            extensions: None,
            source: None,
        };
        let other_event = ParsedEvent::TokenCreation(token_creation_data);

        let result = parser.validate_event(&other_event).await;
        assert!(result.is_ok(), "验证其他类型的ParsedEvent应该成功");
        assert!(!result.unwrap(), "其他类型的ParsedEvent应该返回false");

        println!("✅ ParsedEvent验证测试通过");
    }

    #[test]
    fn test_discriminator_calculation() {
        // 验证discriminator计算的一致性
        let config = create_test_config();
        let program_id = Pubkey::new_unique();
        let parser = InitPoolParser::new(&config, program_id).unwrap();

        let discriminator1 = parser.get_discriminator();
        let discriminator2 = crate::parser::event_parser::calculate_event_discriminator("InitPoolEvent");

        assert_eq!(discriminator1, discriminator2, "discriminator应该一致");

        println!("✅ Discriminator计算测试通过");
        println!("   - Discriminator: {:?}", discriminator1);
    }
}
