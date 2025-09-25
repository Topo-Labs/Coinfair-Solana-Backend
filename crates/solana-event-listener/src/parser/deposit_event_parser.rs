use crate::{
    config::EventListenerConfig,
    error::{EventListenerError, Result},
    parser::{event_parser::DepositEventData, EventParser, ParsedEvent},
};
use async_trait::async_trait;
use borsh::{BorshDeserialize, BorshSerialize};
use solana_sdk::pubkey::Pubkey;
use tracing::{debug, error, info, warn};

// 添加元数据相关的导入
use database::Database;
use mongodb::bson::doc;
use solana_client::rpc_client::RpcClient;
use solana_sdk::program_pack::Pack;
use spl_token::state::Mint;
use std::sync::Arc;
use tokio::sync::RwLock;
// 添加元数据相关的导入
use database::clmm::token_info::{DataSource, TokenPushRequest};
// 使用 utils 中的共享类型
use utils::{ExternalTokenMetadata, TokenMetadata as UtilsTokenMetadata, TokenMetadataProvider};
// use utils::metaplex_service::{MetaplexConfig, MetaplexService, UriMetadata};

// 使用utils中的共享TokenMetadata结构
// 为了保持向后兼容，保留原有的TokenMetadata别名
type TokenMetadata = UtilsTokenMetadata;

/// 存款事件的原始数据结构（与智能合约保持一致）
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct DepositEvent {
    /// 用户钱包地址
    pub user: Pubkey,
    /// 项目配置地址
    pub project_config: Pubkey,
    /// 项目状态
    pub project_state: u8,
    /// 存款代币mint地址
    pub token_mint: Pubkey,
    /// 存款数量
    pub amount: u64,
    /// 累计筹资总额
    pub total_raised: u64,
}

/// 存款事件解析器
pub struct DepositEventParser {
    /// 事件的discriminator
    discriminator: [u8; 8],
    /// 目标程序ID，指定此解析器处理哪个程序的事件
    target_program_id: Pubkey,
    /// RPC客户端，用于查询链上数据
    rpc_client: Option<Arc<RpcClient>>,
    /// 数据库连接，用于TokenInfo缓存
    database: Option<Arc<Database>>,
    /// 代币元数据提供者（抽象的TokenMetadataProvider）
    metadata_provider: Option<Arc<tokio::sync::Mutex<dyn TokenMetadataProvider>>>,
    /// 元数据缓存，避免重复查询
    metadata_cache: Arc<RwLock<std::collections::HashMap<String, TokenMetadata>>>,
}

impl DepositEventParser {
    /// 创建新的存款事件解析器
    pub fn new(config: &EventListenerConfig, program_id: Pubkey) -> Result<Self> {
        // 从环境变量或配置中获取discriminator，默认使用示例值
        let discriminator = [120, 248, 61, 83, 31, 142, 107, 144];

        // 初始化RPC客户端
        let rpc_client = if !config.solana.rpc_url.is_empty() {
            let client = RpcClient::new(config.solana.rpc_url.clone());
            info!("✅ RPC客户端初始化成功: {}", config.solana.rpc_url);
            Some(Arc::new(client))
        } else {
            warn!("⚠️ 未配置RPC URL，代币元数据查询将被跳过");
            None
        };

        // 初始化元数据缓存
        let metadata_cache = Arc::new(RwLock::new(std::collections::HashMap::new()));

        info!(
            "✅ DepositEventParser 初始化成功: program_id={}, discriminator={:?}",
            program_id, discriminator
        );

        Ok(Self {
            discriminator,
            target_program_id: program_id,
            rpc_client,
            database: None,          // 通过setter方法注入
            metadata_provider: None, // 通过setter方法注入
            metadata_cache,
        })
    }

    /// 设置数据库连接（用于TokenInfo缓存）
    pub fn set_database(&mut self, database: Arc<Database>) {
        self.database = Some(database);
        info!("✅ DepositEventParser 数据库连接已设置");
    }

    /// 设置代币元数据提供者（抽象的MetaplexService）
    pub fn set_metadata_provider(&mut self, provider: Arc<tokio::sync::Mutex<dyn TokenMetadataProvider>>) {
        self.metadata_provider = Some(provider);
        info!("✅ DepositEventParser 代币元数据提供者已设置");
    }

    /// 从程序数据解析DepositEvent
    fn parse_program_data(&self, data_str: &str) -> Result<DepositEvent> {
        use base64::{engine::general_purpose, Engine as _};

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
        let event = DepositEvent::try_from_slice(event_data)
            .map_err(|e| EventListenerError::EventParsing(format!("Borsh反序列化失败: {}", e)))?;

        info!(
            "✅ 成功解析DepositEvent: user={}, token={}, amount={}",
            event.user, event.token_mint, event.amount
        );

        Ok(event)
    }

    /// 将原始事件转换为ParsedEvent（异步方法，支持元数据查询）
    async fn convert_to_parsed_event(&self, event: DepositEvent, signature: String, slot: u64) -> ParsedEvent {
        // 尝试获取代币元数据
        let (token_decimals, token_name, token_symbol, token_logo_uri) =
            match self.fetch_token_metadata(&event.token_mint).await {
                Ok(metadata) => {
                    info!(
                        "✅ 成功获取代币元数据: {} ({})",
                        event.token_mint,
                        metadata.symbol.as_deref().unwrap_or("UNK")
                    );
                    (
                        Some(metadata.decimals),
                        metadata.name,
                        metadata.symbol,
                        metadata.logo_uri,
                    )
                }
                Err(e) => {
                    warn!("⚠️ 获取代币元数据失败: {} - {}", event.token_mint, e);
                    (None, None, None, None)
                }
            };

        // 计算实际金额和USD价值
        let actual_amount = if let Some(decimals) = token_decimals {
            (event.amount as f64) / 10_f64.powi(decimals as i32)
        } else {
            event.amount as f64
        };

        let actual_total_raised = if let Some(decimals) = token_decimals {
            (event.total_raised as f64) / 10_f64.powi(decimals as i32)
        } else {
            event.total_raised as f64
        };

        // 判断存款类型
        let deposit_type = self.infer_deposit_type(&event).await.unwrap_or(0);
        let deposit_type_name = self.get_deposit_type_name(deposit_type);

        // 判断是否为高价值存款
        let estimated_usd_value = 0.0; // TODO: 需要通过价格预言机获取
        let is_high_value_deposit = estimated_usd_value >= 10000.0;

        ParsedEvent::Deposit(DepositEventData {
            user: event.user.to_string(),
            project_config: event.project_config.to_string(),
            project_state: event.project_state,
            token_mint: event.token_mint.to_string(),
            amount: event.amount,
            total_raised: event.total_raised,
            // 新增的代币元数据字段
            token_decimals,
            token_name,
            token_symbol,
            token_logo_uri,
            // 扩展字段
            deposit_type,
            deposit_type_name,
            related_pool: None, // TODO: 需要查询关联池子
            is_high_value_deposit,
            estimated_usd_value,
            actual_amount,
            actual_total_raised,
            signature,
            slot,
            deposited_at: chrono::Utc::now().timestamp(),
            processed_at: chrono::Utc::now().to_rfc3339(),
        })
    }

    /// 验证存款事件数据
    fn validate_deposit_event(&self, event: &DepositEventData) -> Result<bool> {
        // 验证用户地址
        if event.user == Pubkey::default().to_string() {
            warn!("❌ 无效的用户地址");
            return Ok(false);
        }

        // 验证代币mint地址
        if event.token_mint == Pubkey::default().to_string() {
            warn!("❌ 无效的代币mint地址");
            return Ok(false);
        }

        // 验证存款金额
        if event.amount == 0 {
            warn!("❌ 存款金额不能为0");
            return Ok(false);
        }

        // 验证累计筹资额不能小于单次存款
        if event.total_raised < event.amount {
            warn!(
                "❌ 累计筹资额不能小于单次存款: total={}, amount={}",
                event.total_raised, event.amount
            );
            return Ok(false);
        }

        // 验证存款类型
        if event.deposit_type > 4 {
            warn!("❌ 无效的存款类型: {}", event.deposit_type);
            return Ok(false);
        }

        Ok(true)
    }

    /// 推断存款类型
    async fn infer_deposit_type(&self, _event: &DepositEvent) -> Result<u8> {
        // 简化逻辑：默认为初始存款
        // 实际实现中可以查询历史记录判断
        Ok(0) // 初始存款
    }

    /// 获取存款类型名称
    fn get_deposit_type_name(&self, deposit_type: u8) -> String {
        match deposit_type {
            0 => "初始存款".to_string(),
            1 => "追加存款".to_string(),
            2 => "应急存款".to_string(),
            _ => "未知类型".to_string(),
        }
    }

    /// 查询代币元数据（四级回退策略，完全复用reward_distribution_parser）
    async fn fetch_token_metadata(&self, mint_address: &Pubkey) -> Result<TokenMetadata> {
        let mint_str = mint_address.to_string();

        // 1. 先检查内存缓存
        {
            let cache = self.metadata_cache.read().await;
            if let Some(metadata) = cache.get(&mint_str) {
                info!("✅ 从内存缓存获取代币元数据: {}", mint_str);
                return Ok(metadata.clone());
            }
        }

        // 2. 查询TokenInfo表
        if let Some(db) = &self.database {
            match db.token_info_repository.find_by_address(&mint_str).await {
                Ok(Some(token_info)) => {
                    let metadata = TokenMetadata {
                        address: mint_str.clone(),
                        decimals: token_info.decimals,
                        name: Some(token_info.name.clone()),
                        symbol: Some(token_info.symbol.clone()),
                        logo_uri: if token_info.logo_uri.is_empty() {
                            None
                        } else {
                            Some(token_info.logo_uri.clone())
                        },
                        description: None,
                        external_url: None,
                        attributes: None,
                        tags: vec!["database".to_string()],
                    };

                    // 更新内存缓存
                    {
                        let mut cache = self.metadata_cache.write().await;
                        cache.insert(mint_str.clone(), metadata.clone());
                    }

                    info!("✅ 从TokenInfo表获取代币元数据: {} ({})", token_info.symbol, mint_str);
                    return Ok(metadata);
                }
                _ => {} // 继续下一级查询
            }
        }

        // 3. 查询链上数据（带有完整的fallback链）
        let metadata = self.fetch_complete_metadata(mint_address).await;

        // 4. 异步保存到TokenInfo表
        if let Some(db) = &self.database {
            let db_clone = db.clone();
            let mint_clone = mint_str.clone();
            let metadata_clone = metadata.clone();

            tokio::spawn(async move {
                match Self::save_to_token_info(db_clone, &mint_clone, &metadata_clone).await {
                    Ok(_) => {
                        info!("✅ 代币元数据已异步保存到TokenInfo: {}", mint_clone);
                    }
                    Err(e) => {
                        warn!("⚠️ 异步保存代币元数据失败: {} - {}", mint_clone, e);
                    }
                }
            });
        }

        // 5. 更新内存缓存
        {
            let mut cache = self.metadata_cache.write().await;
            cache.insert(mint_str, metadata.clone());
        }

        Ok(metadata)
    }

    /// 获取完整的代币元数据（带有完整的fallback链）
    async fn fetch_complete_metadata(&self, mint_address: &Pubkey) -> TokenMetadata {
        let mint_str = mint_address.to_string();

        // 先尝试正常的元数据获取
        match self.fetch_onchain_metadata(mint_address).await {
            Ok(metadata) => {
                info!("✅ 获取元数据成功: {}", mint_str);
                metadata
            }
            Err(e) => {
                warn!("⚠️ 获取元数据失败，使用fallback: {} - {}", mint_str, e);

                // 尝试获取decimals信息
                let decimals = self.fetch_mint_decimals(mint_address).await.ok();

                // 创建fallback元数据
                self.create_fallback_metadata(&mint_str, decimals)
            }
        }
    }

    /// 从链上获取代币元数据（集成MetaplexService）
    async fn fetch_onchain_metadata(&self, mint_address: &Pubkey) -> Result<TokenMetadata> {
        let mint_str = mint_address.to_string();

        // 优先尝试使用代币元数据提供者获取完整元数据
        if let Some(metadata_provider) = &self.metadata_provider {
            info!("🔍 使用代币元数据提供者获取代币元数据: {}", mint_str);

            let mut provider = metadata_provider.lock().await;
            match provider.get_token_metadata(&mint_str).await {
                Ok(Some(external_metadata)) => {
                    info!(
                        "✅ 代币元数据提供者成功获取元数据: {} ({})",
                        mint_str,
                        external_metadata.symbol.as_deref().unwrap_or("UNK")
                    );

                    // 需要获取decimals信息（外部元数据可能没有decimals）
                    let decimals = self.fetch_mint_decimals(mint_address).await.unwrap_or(6);
                    let converted_metadata = Self::convert_external_metadata(external_metadata, decimals);

                    return Ok(converted_metadata);
                }
                Ok(None) => {
                    info!("⚠️ 代币元数据提供者未找到元数据，回退到链上查询: {}", mint_str);
                }
                Err(e) => {
                    warn!("⚠️ 代币元数据提供者查询失败，回退到链上查询: {} - {}", mint_str, e);
                }
            }
        }

        // 回退到原始的链上查询方法（仅获取decimals）
        self.fetch_basic_onchain_metadata(mint_address).await
    }

    /// 从链上获取基础代币元数据（仅获取decimals等基本信息）
    async fn fetch_basic_onchain_metadata(&self, mint_address: &Pubkey) -> Result<TokenMetadata> {
        let mint_str = mint_address.to_string();

        if let Some(rpc_client) = &self.rpc_client {
            info!("🔍 从链上获取基础代币元数据: {}", mint_str);

            match rpc_client.get_account(mint_address) {
                Ok(account) => {
                    if let Ok(mint) = Mint::unpack(&account.data) {
                        info!("✅ 成功从链上获取代币信息: {} (decimals: {})", mint_str, mint.decimals);

                        let metadata = TokenMetadata {
                            address: mint_str,
                            decimals: mint.decimals,
                            name: None,
                            symbol: None,
                            logo_uri: None,
                            description: None,
                            external_url: None,
                            attributes: None,
                            tags: vec!["onchain-basic".to_string()],
                        };

                        return Ok(metadata);
                    }
                }
                Err(e) => {
                    warn!("⚠️ 从链上获取账户信息失败: {} - {}", mint_str, e);
                }
            }
        }

        // 如果所有方法都失败，返回fallback元数据
        Ok(self.create_fallback_metadata(&mint_str, None))
    }

    /// 仅获取代币的decimals信息
    async fn fetch_mint_decimals(&self, mint_address: &Pubkey) -> Result<u8> {
        if let Some(rpc_client) = &self.rpc_client {
            match rpc_client.get_account(mint_address) {
                Ok(account) => {
                    if let Ok(mint) = Mint::unpack(&account.data) {
                        return Ok(mint.decimals);
                    }
                }
                Err(_) => {}
            }
        }

        Err(EventListenerError::EventParsing("无法获取代币decimals信息".to_string()))
    }

    /// 创建fallback元数据
    fn create_fallback_metadata(&self, mint_str: &str, decimals: Option<u8>) -> TokenMetadata {
        let mut tags = vec!["fallback".to_string()];

        // 检查是否为知名代币
        let (name, symbol, additional_tags) = match mint_str {
            "So11111111111111111111111111111111111111112" => (
                Some("Wrapped SOL".to_string()),
                Some("WSOL".to_string()),
                vec!["wrapped-sol".to_string()],
            ),
            "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v" => (
                Some("USD Coin".to_string()),
                Some("USDC".to_string()),
                vec!["stablecoin".to_string()],
            ),
            _ => {
                tags.push("unknown".to_string());
                (None, None, vec![])
            }
        };

        tags.extend(additional_tags);

        TokenMetadata {
            address: mint_str.to_string(),
            decimals: decimals.unwrap_or(6),
            name,
            symbol,
            logo_uri: None,
            description: None,
            external_url: None,
            attributes: None,
            tags,
        }
    }

    /// 转换外部元数据格式
    fn convert_external_metadata(external: ExternalTokenMetadata, decimals: u8) -> TokenMetadata {
        TokenMetadata {
            address: external.address,
            decimals,
            name: external.name,
            symbol: external.symbol,
            logo_uri: external.logo_uri,
            description: external.description,
            external_url: external.external_url,
            attributes: external.attributes.map(|attrs| {
                attrs
                    .into_iter()
                    .map(|attr| utils::TokenAttribute {
                        trait_type: attr.trait_type,
                        value: attr.value,
                    })
                    .collect()
            }),
            tags: external.tags,
        }
    }

    /// 异步保存代币元数据到TokenInfo表
    async fn save_to_token_info(database: Arc<Database>, mint: &str, metadata: &TokenMetadata) -> Result<()> {
        // 构造TokenInfo请求
        let request = TokenPushRequest {
            address: mint.to_string(),
            program_id: Some("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb".to_string()),
            name: metadata.name.clone().unwrap_or_else(|| "Unknown".to_string()),
            symbol: metadata.symbol.clone().unwrap_or_else(|| "UNK".to_string()),
            decimals: metadata.decimals,
            logo_uri: metadata.logo_uri.clone().unwrap_or_default(),
            tags: Some(metadata.tags.clone()),
            daily_volume: Some(0.0),
            freeze_authority: None,
            mint_authority: None,
            permanent_delegate: None,
            minted_at: None,
            extensions: None,
            source: Some(DataSource::ExternalPush),
        };

        // 尝试保存或更新
        match database.token_info_repository.push_token(request).await {
            Ok(_) => {
                debug!("✅ TokenInfo保存成功: {}", mint);
                Ok(())
            }
            Err(e) => {
                error!("❌ TokenInfo保存失败: {} - {}", mint, e);
                Err(EventListenerError::EventParsing(format!("保存TokenInfo失败: {}", e)))
            }
        }
    }

    /// 检查是否有代币元数据提供者可用
    pub fn has_metadata_provider(&self) -> bool {
        self.metadata_provider.is_some()
    }

    /// 检查是否有RPC客户端可用
    pub fn has_rpc_client(&self) -> bool {
        self.rpc_client.is_some()
    }

    /// 检查是否有数据库连接可用
    pub fn has_database(&self) -> bool {
        self.database.is_some()
    }

    /// 获取当前支持的元数据源列表
    pub fn get_available_metadata_sources(&self) -> Vec<&'static str> {
        let mut sources = Vec::new();

        if self.has_metadata_provider() {
            sources.extend_from_slice(&[
                "external-provider",
                "token-2022",
                "jupiter-token-list",
                "solana-token-list",
            ]);
        }

        if self.has_database() {
            sources.push("database");
        }

        if self.has_rpc_client() {
            sources.push("onchain-basic");
        }

        sources.push("fallback");
        sources.push("cache");

        sources
    }

    /// 清理元数据缓存（避免内存泄漏）
    pub async fn clear_metadata_cache(&self) {
        let mut cache = self.metadata_cache.write().await;
        let cache_size = cache.len();
        cache.clear();
        info!("🗑️ 清理代币元数据缓存: {} 个条目", cache_size);
    }

    /// 获取缓存统计信息
    pub async fn get_cache_stats(&self) -> (usize, Vec<String>) {
        let cache = self.metadata_cache.read().await;
        let size = cache.len();
        let keys: Vec<String> = cache.keys().cloned().collect();
        (size, keys)
    }
}

#[async_trait]
impl EventParser for DepositEventParser {
    /// 获取程序ID
    fn get_program_id(&self) -> Pubkey {
        self.target_program_id
    }

    /// 获取discriminator
    fn get_discriminator(&self) -> [u8; 8] {
        self.discriminator
    }

    /// 获取事件类型
    fn get_event_type(&self) -> &'static str {
        "deposit"
    }

    /// 检查是否支持该程序
    fn supports_program(&self, program_id: &Pubkey) -> Option<bool> {
        Some(*program_id == self.target_program_id)
    }

    /// 从交易日志中解析事件（返回单个事件）
    async fn parse_from_logs(&self, logs: &[String], signature: &str, slot: u64) -> Result<Option<ParsedEvent>> {
        for (index, log) in logs.iter().enumerate() {
            if log.starts_with("Program data: ") {
                if let Some(data_part) = log.strip_prefix("Program data: ") {
                    match self.parse_program_data(data_part) {
                        Ok(event) => {
                            info!(
                                "💰 第{}行发现DepositEvent: 用户={} 代币={} 数量={}",
                                index + 1,
                                event.user,
                                event.token_mint,
                                event.amount
                            );
                            // 使用异步方法转换事件
                            let parsed_event = self.convert_to_parsed_event(event, signature.to_string(), slot).await;
                            return Ok(Some(parsed_event));
                        }
                        Err(EventListenerError::DiscriminatorMismatch) => {
                            // Discriminator不匹配是正常情况，继续尝试下一条日志
                            continue;
                        }
                        Err(e) => {
                            debug!("⚠️ 第{}行DepositEvent解析失败: {}", index + 1, e);
                            continue;
                        }
                    }
                }
            }
        }
        Ok(None)
    }

    /// 验证事件数据的有效性
    async fn validate_event(&self, event: &ParsedEvent) -> Result<bool> {
        match event {
            ParsedEvent::Deposit(deposit_event) => self.validate_deposit_event(deposit_event),
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
                program_ids: vec![Pubkey::from_str("11111111111111111111111111111112").unwrap()],
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

    fn create_test_deposit_event() -> DepositEvent {
        DepositEvent {
            user: Pubkey::new_unique(),
            project_config: Pubkey::new_unique(),
            project_state: 3,
            token_mint: Pubkey::new_unique(),
            amount: 1000000,       // 1 token with 6 decimals
            total_raised: 5000000, // 5 tokens
        }
    }

    #[test]
    fn test_deposit_event_parser_creation() {
        let config = create_test_config();
        let parser = DepositEventParser::new(&config, Pubkey::new_unique()).unwrap();

        assert_eq!(parser.get_event_type(), "deposit");
        // 测试discriminator（需要从实际IDL获取）
        assert_eq!(parser.get_discriminator(), [120, 248, 61, 83, 31, 142, 107, 144]);
    }

    #[test]
    fn test_borsh_serialization() {
        let event = create_test_deposit_event();

        // 测试序列化
        let serialized = borsh::to_vec(&event).unwrap();
        assert!(!serialized.is_empty());

        // 测试反序列化
        let deserialized = DepositEvent::try_from_slice(&serialized).unwrap();
        assert_eq!(deserialized.user, event.user);
        assert_eq!(deserialized.token_mint, event.token_mint);
        assert_eq!(deserialized.amount, event.amount);
    }

    #[tokio::test]
    async fn test_convert_to_parsed_event() {
        let config = create_test_config();
        let mut parser = DepositEventParser::new(&config, Pubkey::new_unique()).unwrap();

        // 不设置RPC客户端，避免实际的网络调用
        parser.rpc_client = None;

        let test_event = create_test_deposit_event();

        let parsed = parser
            .convert_to_parsed_event(test_event.clone(), "test_signature".to_string(), 12345)
            .await;

        match parsed {
            ParsedEvent::Deposit(data) => {
                assert_eq!(data.user, test_event.user.to_string());
                assert_eq!(data.token_mint, test_event.token_mint.to_string());
                assert_eq!(data.amount, test_event.amount);
                assert_eq!(data.signature, "test_signature");
                assert_eq!(data.slot, 12345);
            }
            _ => panic!("期望Deposit事件"),
        }
    }

    #[tokio::test]
    async fn test_parse_from_logs_no_program_data() {
        let config = create_test_config();
        let parser = DepositEventParser::new(&config, Pubkey::new_unique()).unwrap();

        let logs = vec![
            "Program 11111111111111111111111111111111 invoke [1]".to_string(),
            "Program 11111111111111111111111111111111 success".to_string(),
        ];

        let result = parser.parse_from_logs(&logs, "test_sig", 12345).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_fallback_metadata_creation() {
        let config = create_test_config();
        let parser = DepositEventParser::new(&config, Pubkey::new_unique()).unwrap();

        // 测试知名代币的fallback元数据
        let wsol_metadata = parser.create_fallback_metadata("So11111111111111111111111111111111111111112", Some(9));

        assert_eq!(wsol_metadata.symbol, Some("WSOL".to_string()));
        assert_eq!(wsol_metadata.name, Some("Wrapped SOL".to_string()));
        assert_eq!(wsol_metadata.decimals, 9);
        assert!(wsol_metadata.tags.contains(&"fallback".to_string()));
        assert!(wsol_metadata.tags.contains(&"wrapped-sol".to_string()));

        // 测试未知代币的fallback元数据
        let unknown_metadata = parser.create_fallback_metadata("UnknownTokenAddress123456789", Some(6));

        assert_eq!(unknown_metadata.symbol, None);
        assert_eq!(unknown_metadata.name, None);
        assert_eq!(unknown_metadata.decimals, 6);
        assert!(unknown_metadata.tags.contains(&"fallback".to_string()));
        assert!(unknown_metadata.tags.contains(&"unknown".to_string()));
    }

    #[tokio::test]
    async fn test_metadata_provider_integration() {
        let config = create_test_config();
        let parser = DepositEventParser::new(&config, Pubkey::new_unique()).unwrap();

        // 初始状态：没有代币元数据提供者
        assert!(!parser.has_metadata_provider());
        assert!(parser.has_rpc_client()); // 应该有RPC客户端

        // 测试支持的元数据源
        let sources = parser.get_available_metadata_sources();
        println!("支持的元数据源: {:?}", sources);

        // 没有代币元数据提供者时，应该有这些源
        assert!(sources.contains(&"onchain-basic"));
        assert!(sources.contains(&"fallback"));
        assert!(sources.contains(&"cache"));
        assert!(!sources.contains(&"external-provider"));
    }
}
