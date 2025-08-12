use crate::{
    config::EventListenerConfig,
    error::{EventListenerError, Result},
    parser::{event_parser::RewardDistributionEventData, EventParser, ParsedEvent},
};
use async_trait::async_trait;
use borsh::{BorshDeserialize, BorshSerialize};
use solana_sdk::pubkey::Pubkey;
use tracing::{debug, info, warn, error};

// 添加元数据相关的导入
use std::sync::Arc;
use solana_client::rpc_client::RpcClient;
use spl_token::state::Mint;
use solana_sdk::program_pack::Pack;
use database::{
    Database,
    token_info::{TokenPushRequest, DataSource},
};
use tokio::sync::RwLock;

/// 简化的代币元数据结构
#[derive(Debug, Clone)]
pub struct TokenMetadata {
    pub decimals: u8,
    pub name: Option<String>,
    pub symbol: Option<String>,
    pub logo_uri: Option<String>,
}

/// 推荐奖励分发事件的原始数据结构（与智能合约保持一致）
/// 新的ReferralRewardEvent结构体
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct ReferralRewardEvent {
    /// 付款人地址
    pub from: Pubkey,
    /// 接收者地址（上级或下级）
    pub to: Pubkey,
    /// 奖励的代币mint地址
    pub mint: Pubkey,
    /// 奖励数量
    pub amount: u64,
    /// 时间戳
    pub timestamp: i64,
}

/// 奖励发放事件解析器
pub struct RewardDistributionParser {
    /// 事件的discriminator
    discriminator: [u8; 8],
    /// 目标程序ID，指定此解析器处理哪个程序的事件
    target_program_id: Pubkey,
    /// RPC客户端，用于查询链上数据
    rpc_client: Option<Arc<RpcClient>>,
    /// 数据库连接，用于TokenInfo缓存
    database: Option<Arc<Database>>,
    /// 元数据缓存，避免重复查询
    metadata_cache: Arc<RwLock<std::collections::HashMap<String, TokenMetadata>>>,
}

impl RewardDistributionParser {
    /// 创建新的奖励发放事件解析器
    pub fn new(config: &EventListenerConfig, program_id: Pubkey) -> Result<Self> {
        // 奖励发放事件的discriminator
        // let discriminator = [178, 95, 213, 88, 42, 167, 129, 77];
        let discriminator = [88, 33, 159, 153, 151, 93, 111, 189];

        // 初始化RPC客户端
        let rpc_client = if !config.solana.rpc_url.is_empty() {
            let client = RpcClient::new(config.solana.rpc_url.clone());
            info!("✅ RPC客户端初始化成功: {}", config.solana.rpc_url);
            Some(Arc::new(client))
        } else {
            warn!("⚠️ 未配置RPC URL，代币元数据查询将被跳过");
            None
        };

        // 初始化数据库连接（如果需要）
        let database = None; // 暂时设为None，后续可以通过setter方法注入

        // 初始化元数据缓存
        let metadata_cache = Arc::new(RwLock::new(std::collections::HashMap::new()));

        Ok(Self {
            discriminator,
            target_program_id: program_id,
            rpc_client,
            database,
            metadata_cache,
        })
    }

    /// 从程序数据解析推荐奖励事件
    fn parse_program_data(&self, data_str: &str) -> Result<ReferralRewardEvent> {
        use base64::{engine::general_purpose, Engine as _};

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
        let event =
            ReferralRewardEvent::try_from_slice(event_data).map_err(|e| EventListenerError::EventParsing(format!("Borsh反序列化失败: {}", e)))?;

        debug!("✅ 成功解析推荐奖励事件: 从={}, 到={}, 数量={}", event.from, event.to, event.amount);
        Ok(event)
    }

    /// 生成唯一的分发ID（基于事件内容）
    fn generate_distribution_id(&self, event: &ReferralRewardEvent) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        event.from.hash(&mut hasher);
        event.to.hash(&mut hasher);
        event.mint.hash(&mut hasher);
        event.amount.hash(&mut hasher);
        event.timestamp.hash(&mut hasher);

        hasher.finish()
    }

    /// 推断奖励来源（基于金额等特征）
    fn infer_reward_source(&self, _event: &ReferralRewardEvent) -> u8 {
        // 由于是ReferralRewardEvent，来源固定为推荐计划
        2 // 推荐计划
    }

    /// 推断奖励类型
    fn infer_reward_type(&self, _event: &ReferralRewardEvent) -> u8 {
        // 由于是ReferralRewardEvent，类型固定为推荐奖励
        1 // 推荐奖励
    }

    /// 计算默认倍率
    fn calculate_default_multiplier(&self, _event: &ReferralRewardEvent) -> u16 {
        // 默认1.0倍奖励
        10000
    }

    /// 获取奖励类型名称
    fn get_reward_type_name(&self, reward_type: u8) -> String {
        match reward_type {
            0 => "交易奖励".to_string(),
            1 => "推荐奖励".to_string(),
            2 => "流动性奖励".to_string(),
            3 => "治理奖励".to_string(),
            4 => "空投奖励".to_string(),
            _ => "未知奖励".to_string(),
        }
    }

    /// 获取奖励来源名称
    fn get_reward_source_name(&self, reward_source: u8) -> String {
        match reward_source {
            0 => "DEX交易".to_string(),
            1 => "流动性挖矿".to_string(),
            2 => "推荐计划".to_string(),
            3 => "治理投票".to_string(),
            4 => "特殊活动".to_string(),
            _ => "未知来源".to_string(),
        }
    }

    /// 计算奖励相关指标
    fn calculate_reward_metrics(&self, event: &ReferralRewardEvent) -> (f64, u64, u64, bool) {
        // 默认倍率 1.0x
        let multiplier_rate = 1.0;

        // 由于新结构没有base_reward_amount，假设全部为基础奖励，无额外奖励
        let bonus_amount = 0u64;

        // 新结构没有锁定信息，默认为0天
        let lock_days = 0u64;

        // 是否为高价值奖励（大于等价100 USDC）
        let is_high_value = event.amount >= 100_000_000; // 假设6位小数的代币

        (multiplier_rate, bonus_amount, lock_days, is_high_value)
    }

    /// 将原始事件转换为ParsedEvent（现在是异步方法，支持元数据查询）
    async fn convert_to_parsed_event(&self, event: ReferralRewardEvent, signature: String, slot: u64) -> ParsedEvent {
        let (multiplier_percentage, bonus_amount, lock_days, is_high_value) = self.calculate_reward_metrics(&event);
        let distribution_id = self.generate_distribution_id(&event);
        let reward_type = self.infer_reward_type(&event);
        let reward_source = self.infer_reward_source(&event);
        let multiplier = self.calculate_default_multiplier(&event);

        // 尝试获取代币元数据
        let (token_decimals, token_name, token_symbol, token_logo_uri) = match self.fetch_token_metadata(&event.mint).await {
            Ok(metadata) => {
                debug!("✅ 成功获取代币元数据: {} ({})", event.mint, metadata.symbol.as_deref().unwrap_or("UNK"));
                (
                    Some(metadata.decimals),
                    metadata.name,
                    metadata.symbol,
                    metadata.logo_uri,
                )
            }
            Err(e) => {
                warn!("⚠️ 获取代币元数据失败: {} - {}", event.mint, e);
                (None, None, None, None)
            }
        };

        ParsedEvent::RewardDistribution(RewardDistributionEventData {
            distribution_id,
            reward_pool: event.from.to_string(),    // 使用from作为奖励池地址
            recipient: event.to.to_string(),        // to对应recipient
            referrer: Some(event.from.to_string()), // from对应referrer
            reward_token_mint: event.mint.to_string(),      // mint对应reward_token_mint
            // 新增的代币元数据字段
            reward_token_decimals: token_decimals,
            reward_token_name: token_name,
            reward_token_symbol: token_symbol,
            reward_token_logo_uri: token_logo_uri,
            reward_amount: event.amount,        // amount对应reward_amount
            base_reward_amount: event.amount,   // 新结构没有base_reward，使用amount
            bonus_amount,
            reward_type,
            reward_type_name: self.get_reward_type_name(reward_type),
            reward_source,
            reward_source_name: self.get_reward_source_name(reward_source),
            related_address: None, // 新结构没有此字段
            multiplier,
            multiplier_percentage,
            is_locked: false, // 新结构没有锁定信息，默认不锁定
            unlock_timestamp: None,
            lock_days,
            has_referrer: true,       // 推荐奖励总是有推荐人
            is_referral_reward: true, // 固定为推荐奖励
            is_high_value_reward: is_high_value,
            estimated_usd_value: 0.0,        // 需要通过价格预言机获取
            distributed_at: event.timestamp, // timestamp对应distributed_at
            signature,
            slot,
            processed_at: chrono::Utc::now().to_rfc3339(),
        })
    }

    /// 验证奖励发放事件数据
    fn validate_reward_distribution(&self, event: &RewardDistributionEventData) -> Result<bool> {
        // 验证分发ID
        if event.distribution_id == 0 {
            warn!("❌ 分发ID不能为0");
            return Ok(false);
        }

        // 验证奖励池地址
        if event.reward_pool == Pubkey::default().to_string() {
            warn!("❌ 无效的奖励池地址");
            return Ok(false);
        }

        // 验证接收者地址
        if event.recipient == Pubkey::default().to_string() {
            warn!("❌ 无效的接收者地址");
            return Ok(false);
        }

        // 验证奖励代币地址
        if event.reward_token_mint == Pubkey::default().to_string() {
            warn!("❌ 无效的奖励代币地址");
            return Ok(false);
        }

        // 验证奖励数量
        if event.reward_amount == 0 {
            warn!("❌ 奖励数量不能为0");
            return Ok(false);
        }

        // 验证基础奖励数量
        if event.base_reward_amount == 0 {
            warn!("❌ 基础奖励数量不能为0");
            return Ok(false);
        }

        // 验证奖励数量与基础数量的关系
        if event.reward_amount < event.base_reward_amount {
            warn!(
                "❌ 奖励数量不能小于基础奖励数量: reward={}, base={}",
                event.reward_amount, event.base_reward_amount
            );
            return Ok(false);
        }

        // 验证奖励类型
        if event.reward_type > 4 {
            warn!("❌ 无效的奖励类型: {}", event.reward_type);
            return Ok(false);
        }

        // 验证奖励来源
        if event.reward_source > 4 {
            warn!("❌ 无效的奖励来源: {}", event.reward_source);
            return Ok(false);
        }

        // 验证倍率合理性 (0.1倍 - 6.5倍，因为u16最大值限制)
        if event.multiplier < 1000 {
            warn!("❌ 奖励倍率过低: {}", event.multiplier);
            return Ok(false);
        }

        // 验证锁定逻辑
        if event.is_locked && event.unlock_timestamp.is_none() {
            warn!("❌ 已锁定的奖励必须有解锁时间");
            return Ok(false);
        }

        // 验证解锁时间合理性
        if let Some(unlock_time) = event.unlock_timestamp {
            if unlock_time <= event.distributed_at {
                warn!(
                    "❌ 解锁时间不能早于或等于发放时间: unlock={}, distribute={}",
                    unlock_time, event.distributed_at
                );
                return Ok(false);
            }

            // 验证锁定期不能超过2年
            let max_lock_duration = 2 * 365 * 24 * 3600; // 2年的秒数
            if unlock_time - event.distributed_at > max_lock_duration {
                warn!("❌ 锁定期不能超过2年: {} 秒", unlock_time - event.distributed_at);
                return Ok(false);
            }
        }

        // 验证时间戳合理性
        let now = chrono::Utc::now().timestamp();
        if event.distributed_at > now || event.distributed_at < (now - 86400) {
            warn!("❌ 发放时间戳异常: {}", event.distributed_at);
            return Ok(false);
        }

        // 验证推荐人不能是自己
        if let Some(referrer) = &event.referrer {
            if referrer == &event.recipient {
                warn!("❌ 推荐人不能是自己: {}", event.recipient);
                return Ok(false);
            }
        }

        // 验证推荐奖励的逻辑一致性
        if event.is_referral_reward && event.referrer.is_none() {
            warn!("❌ 推荐奖励必须有推荐人");
            return Ok(false);
        }

        // 验证奖励金额的合理性（防止天文数字）
        let max_reasonable_amount = 1_000_000_000_000_000_000u64; // 10^18
        if event.reward_amount > max_reasonable_amount {
            warn!("❌ 奖励数量过大，可能有错误: {}", event.reward_amount);
            return Ok(false);
        }

        Ok(true)
    }

    /// 设置数据库连接（用于TokenInfo缓存）
    pub fn set_database(&mut self, database: Arc<Database>) {
        self.database = Some(database);
        info!("✅ RewardDistributionParser 数据库连接已设置");
    }

    /// 查询代币元数据（先查缓存，再查TokenInfo表，最后查链上）
    async fn fetch_token_metadata(&self, mint_address: &Pubkey) -> Result<TokenMetadata> {
        let mint_str = mint_address.to_string();
        
        // 1. 先检查内存缓存
        {
            let cache = self.metadata_cache.read().await;
            if let Some(metadata) = cache.get(&mint_str) {
                debug!("✅ 从内存缓存获取代币元数据: {}", mint_str);
                return Ok(metadata.clone());
            }
        }

        // 2. 查询TokenInfo表
        if let Some(db) = &self.database {
            debug!("🔍 从TokenInfo表查询代币元数据: {}", mint_str);
            match db.token_info_repository.find_by_address(&mint_str).await {
                Ok(Some(token_info)) => {
                    let metadata = TokenMetadata {
                        decimals: token_info.decimals,
                        name: Some(token_info.name.clone()),
                        symbol: Some(token_info.symbol.clone()),
                        logo_uri: if token_info.logo_uri.is_empty() { None } else { Some(token_info.logo_uri.clone()) },
                    };

                    // 更新内存缓存
                    {
                        let mut cache = self.metadata_cache.write().await;
                        cache.insert(mint_str.clone(), metadata.clone());
                    }

                    info!("✅ 从TokenInfo表获取代币元数据: {} ({})", token_info.symbol, mint_str);
                    return Ok(metadata);
                }
                Ok(None) => {
                    debug!("❌ TokenInfo表中未找到代币: {}", mint_str);
                }
                Err(e) => {
                    warn!("⚠️ 查询TokenInfo表失败: {} - {}", mint_str, e);
                }
            }
        }

        // 3. 查询链上数据
        let metadata = self.fetch_onchain_metadata(mint_address).await?;

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

    /// 从链上获取代币元数据
    async fn fetch_onchain_metadata(&self, mint_address: &Pubkey) -> Result<TokenMetadata> {
        let rpc_client = self.rpc_client.as_ref()
            .ok_or_else(|| EventListenerError::EventParsing("RPC客户端未初始化".to_string()))?;

        debug!("🔗 从链上获取代币元数据: {}", mint_address);

        // 获取mint账户数据
        let account_data = rpc_client.get_account_data(mint_address)
            .map_err(|e| EventListenerError::EventParsing(format!("获取mint账户数据失败: {} - {}", mint_address, e)))?;

        // 解析mint数据获取decimals
        let mint = Mint::unpack(&account_data)
            .map_err(|e| EventListenerError::EventParsing(format!("解析mint数据失败: {} - {}", mint_address, e)))?;

        let metadata = TokenMetadata {
            decimals: mint.decimals,
            name: None, // 链上mint账户不包含名称信息
            symbol: None, // 链上mint账户不包含符号信息
            logo_uri: None, // 链上mint账户不包含logo信息
        };

        info!("✅ 从链上获取代币元数据: {} (decimals: {})", mint_address, mint.decimals);
        Ok(metadata)
    }

    /// 异步保存代币元数据到TokenInfo表
    async fn save_to_token_info(
        database: Arc<Database>,
        mint_address: &str,
        metadata: &TokenMetadata,
    ) -> Result<()> {
        let push_request = TokenPushRequest {
            address: mint_address.to_string(),
            program_id: Some("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA".to_string()),
            name: metadata.name.clone().unwrap_or_else(|| "Unknown Token".to_string()),
            symbol: metadata.symbol.clone().unwrap_or_else(|| "UNK".to_string()),
            decimals: metadata.decimals,
            logo_uri: metadata.logo_uri.clone().unwrap_or_else(|| "".to_string()),
            tags: Some(vec!["event_listener".to_string()]),
            daily_volume: Some(0.0),
            freeze_authority: None,
            mint_authority: None,
            permanent_delegate: None,
            minted_at: None,
            extensions: None,
            source: Some(DataSource::OnchainSync),
        };

        match database.token_info_repository.push_token(push_request).await {
            Ok(response) => {
                if response.success {
                    debug!("✅ 代币元数据保存成功: {} ({})", mint_address, response.operation);
                } else {
                    warn!("⚠️ 代币元数据保存失败: {} - {}", mint_address, response.message);
                }
                Ok(())
            }
            Err(e) => {
                error!("❌ 保存代币元数据到TokenInfo失败: {} - {}", mint_address, e);
                Err(EventListenerError::EventParsing(format!("保存TokenInfo失败: {}", e)))
            }
        }
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
impl EventParser for RewardDistributionParser {
    fn get_program_id(&self) -> Pubkey {
        self.target_program_id
    }

    fn get_discriminator(&self) -> [u8; 8] {
        self.discriminator
    }

    fn get_event_type(&self) -> &'static str {
        "reward_distribution"
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
                                "💰 第{}行发现推荐奖励事件: 从 {} 向 {} 发放 {} {}",
                                index + 1,
                                event.from,
                                event.to,
                                event.amount,
                                "推荐奖励"
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
                            debug!("⚠️ 第{}行奖励发放事件解析失败: {}", index + 1, e);
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
            ParsedEvent::RewardDistribution(reward_event) => self.validate_reward_distribution(reward_event),
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
                program_ids: vec![Pubkey::from_str("FA1RJDDXysgwg5Gm3fJXWxt26JQzPkAzhTA114miqNUX").unwrap()],
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

    fn create_test_referral_reward_event() -> ReferralRewardEvent {
        ReferralRewardEvent {
            from: Pubkey::new_unique(), // 付款人
            to: Pubkey::new_unique(),   // 接收者
            mint: Pubkey::new_unique(), // 代币mint
            amount: 500000,             // 0.5 tokens with 6 decimals
            timestamp: chrono::Utc::now().timestamp(),
        }
    }

    #[test]
    fn test_reward_distribution_parser_creation() {
        let config = create_test_config();
        let parser = RewardDistributionParser::new(&config, Pubkey::new_unique()).unwrap();

        assert_eq!(parser.get_event_type(), "reward_distribution");
        assert_eq!(parser.get_discriminator(), [88, 33, 159, 153, 151, 93, 111, 189]);
    }

    #[test]
    fn test_reward_type_mapping() {
        let config = create_test_config();
        let parser = RewardDistributionParser::new(&config, Pubkey::new_unique()).unwrap();

        assert_eq!(parser.get_reward_type_name(0), "交易奖励");
        assert_eq!(parser.get_reward_type_name(1), "推荐奖励");
        assert_eq!(parser.get_reward_type_name(2), "流动性奖励");
        assert_eq!(parser.get_reward_type_name(3), "治理奖励");
        assert_eq!(parser.get_reward_type_name(4), "空投奖励");
        assert_eq!(parser.get_reward_type_name(99), "未知奖励");
    }

    #[test]
    fn test_reward_source_mapping() {
        let config = create_test_config();
        let parser = RewardDistributionParser::new(&config, Pubkey::new_unique()).unwrap();

        assert_eq!(parser.get_reward_source_name(0), "DEX交易");
        assert_eq!(parser.get_reward_source_name(1), "流动性挖矿");
        assert_eq!(parser.get_reward_source_name(2), "推荐计划");
        assert_eq!(parser.get_reward_source_name(3), "治理投票");
        assert_eq!(parser.get_reward_source_name(4), "特殊活动");
        assert_eq!(parser.get_reward_source_name(99), "未知来源");
    }

    #[tokio::test]
    async fn test_convert_to_parsed_event() {
        let config = create_test_config();
        let mut parser = RewardDistributionParser::new(&config, Pubkey::new_unique()).unwrap();
        
        // 不设置RPC客户端，避免实际的网络调用
        parser.rpc_client = None;
        
        let test_event = create_test_referral_reward_event();

        let parsed = parser.convert_to_parsed_event(test_event.clone(), "test_signature".to_string(), 12345).await;

        match parsed {
            ParsedEvent::RewardDistribution(data) => {
                assert_eq!(data.recipient, test_event.to.to_string());
                assert_eq!(data.referrer, Some(test_event.from.to_string()));
                assert_eq!(data.reward_token_mint, test_event.mint.to_string());
                assert_eq!(data.reward_amount, test_event.amount);
                assert_eq!(data.base_reward_amount, test_event.amount);
                assert_eq!(data.bonus_amount, 0); // 新结构默认无bonus
                assert_eq!(data.reward_type, 1); // 推荐奖励
                assert_eq!(data.reward_type_name, "推荐奖励");
                assert_eq!(data.reward_source, 2); // 推荐计划
                assert_eq!(data.reward_source_name, "推荐计划");
                assert_eq!(data.multiplier, 10000); // 1.0x
                assert_eq!(data.multiplier_percentage, 1.0);
                assert_eq!(data.is_locked, false); // 新结构默认不锁定
                assert_eq!(data.lock_days, 0);
                assert_eq!(data.has_referrer, true);
                assert_eq!(data.is_referral_reward, true);
                assert_eq!(data.distributed_at, test_event.timestamp);
                assert_eq!(data.signature, "test_signature");
                assert_eq!(data.slot, 12345);
                
                // 新的代币元数据字段（在没有RPC和数据库的情况下应该为None）
                assert_eq!(data.reward_token_decimals, None);
                assert_eq!(data.reward_token_name, None);
                assert_eq!(data.reward_token_symbol, None);
                assert_eq!(data.reward_token_logo_uri, None);
            }
            _ => panic!("期望RewardDistribution事件"),
        }
    }

    #[test]
    fn test_generate_distribution_id() {
        let config = create_test_config();
        let parser = RewardDistributionParser::new(&config, Pubkey::new_unique()).unwrap();
        let test_event = create_test_referral_reward_event();

        let id1 = parser.generate_distribution_id(&test_event);
        let id2 = parser.generate_distribution_id(&test_event);

        // 相同事件应该生成相同ID
        assert_eq!(id1, id2);

        // 不同事件应该生成不同ID
        let mut different_event = test_event.clone();
        different_event.amount = 999999;
        let id3 = parser.generate_distribution_id(&different_event);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_infer_reward_properties() {
        let config = create_test_config();
        let parser = RewardDistributionParser::new(&config, Pubkey::new_unique()).unwrap();
        let test_event = create_test_referral_reward_event();

        // 测试奖励类型推断
        assert_eq!(parser.infer_reward_type(&test_event), 1); // 推荐奖励

        // 测试奖励来源推断
        assert_eq!(parser.infer_reward_source(&test_event), 2); // 推荐计划

        // 测试默认倍率
        assert_eq!(parser.calculate_default_multiplier(&test_event), 10000); // 1.0x
    }

    #[tokio::test]
    async fn test_validate_reward_distribution() {
        let config = create_test_config();
        let parser = RewardDistributionParser::new(&config, Pubkey::new_unique()).unwrap();

        let valid_event = RewardDistributionEventData {
            distribution_id: 12345,
            reward_pool: Pubkey::new_unique().to_string(),
            recipient: Pubkey::new_unique().to_string(),
            referrer: Some(Pubkey::new_unique().to_string()),
            reward_token_mint: Pubkey::new_unique().to_string(),
            // 新增的代币元数据字段
            reward_token_decimals: Some(6),
            reward_token_name: Some("Test Token".to_string()),
            reward_token_symbol: Some("TEST".to_string()),
            reward_token_logo_uri: Some("https://example.com/logo.png".to_string()),
            reward_amount: 1500000,
            base_reward_amount: 1000000,
            bonus_amount: 500000,
            reward_type: 2,
            reward_type_name: "流动性奖励".to_string(),
            reward_source: 1,
            reward_source_name: "流动性挖矿".to_string(),
            related_address: Some(Pubkey::new_unique().to_string()),
            multiplier: 15000,
            multiplier_percentage: 1.5,
            is_locked: true,
            unlock_timestamp: Some(chrono::Utc::now().timestamp() + 7 * 24 * 3600),
            lock_days: 7,
            has_referrer: true,
            is_referral_reward: false,
            is_high_value_reward: false,
            estimated_usd_value: 0.0,
            distributed_at: chrono::Utc::now().timestamp(),
            signature: "test_sig".to_string(),
            slot: 12345,
            processed_at: chrono::Utc::now().to_rfc3339(),
        };

        assert!(parser.validate_reward_distribution(&valid_event).unwrap());

        // 测试无效事件（分发ID为0）
        let invalid_event = RewardDistributionEventData {
            distribution_id: 0, // 无效ID
            ..valid_event.clone()
        };

        assert!(!parser.validate_reward_distribution(&invalid_event).unwrap());

        // 测试推荐人是自己的情况
        let self_referrer_event = RewardDistributionEventData {
            referrer: Some(valid_event.recipient.clone()), // 推荐人是自己
            ..valid_event.clone()
        };

        assert!(!parser.validate_reward_distribution(&self_referrer_event).unwrap());

        // 测试锁定但没有解锁时间的情况
        let locked_no_unlock_event = RewardDistributionEventData {
            is_locked: true,
            unlock_timestamp: None, // 没有解锁时间
            ..valid_event.clone()
        };

        assert!(!parser.validate_reward_distribution(&locked_no_unlock_event).unwrap());
    }

    #[test]
    fn test_borsh_serialization() {
        let event = create_test_referral_reward_event();

        // 测试序列化
        let serialized = borsh::to_vec(&event).unwrap();
        assert!(!serialized.is_empty());

        // 测试反序列化
        let deserialized = ReferralRewardEvent::try_from_slice(&serialized).unwrap();
        assert_eq!(deserialized.from, event.from);
        assert_eq!(deserialized.to, event.to);
        assert_eq!(deserialized.amount, event.amount);
    }

    #[tokio::test]
    async fn test_parse_from_logs_no_program_data() {
        let config = create_test_config();
        let parser = RewardDistributionParser::new(&config, Pubkey::new_unique()).unwrap();

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
        let parser = RewardDistributionParser::new(&config, Pubkey::new_unique()).unwrap();

        let event = ParsedEvent::RewardDistribution(RewardDistributionEventData {
            distribution_id: 12345,
            reward_pool: Pubkey::new_unique().to_string(),
            recipient: Pubkey::new_unique().to_string(),
            referrer: Some(Pubkey::new_unique().to_string()),
            reward_token_mint: Pubkey::new_unique().to_string(),
            // 新增的代币元数据字段
            reward_token_decimals: Some(6),
            reward_token_name: Some("Test Token".to_string()),
            reward_token_symbol: Some("TEST".to_string()),
            reward_token_logo_uri: Some("https://example.com/logo.png".to_string()),
            reward_amount: 1500000,
            base_reward_amount: 1000000,
            bonus_amount: 500000,
            reward_type: 2,
            reward_type_name: "流动性奖励".to_string(),
            reward_source: 1,
            reward_source_name: "流动性挖矿".to_string(),
            related_address: Some(Pubkey::new_unique().to_string()),
            multiplier: 15000,
            multiplier_percentage: 1.5,
            is_locked: true,
            unlock_timestamp: Some(chrono::Utc::now().timestamp() + 7 * 24 * 3600),
            lock_days: 7,
            has_referrer: true,
            is_referral_reward: false,
            is_high_value_reward: false,
            estimated_usd_value: 0.0,
            distributed_at: chrono::Utc::now().timestamp(),
            signature: "test_sig".to_string(),
            slot: 12345,
            processed_at: chrono::Utc::now().to_rfc3339(),
        });

        assert!(parser.validate_event(&event).await.unwrap());
    }
}
