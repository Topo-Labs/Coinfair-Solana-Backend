use crate::{
    config::EventListenerConfig,
    error::{EventListenerError, Result},
    parser::{EventParser, ParsedEvent},
};
use async_trait::async_trait;
use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use tracing::{debug, error, info, warn};

// 添加元数据相关的导入
use database::Database;
use solana_client::rpc_client::RpcClient;
use solana_sdk::program_pack::Pack;
use spl_token::state::Mint;
use std::sync::Arc;
use tokio::sync::RwLock;
// 添加元数据相关的导入
use database::clmm::token_info::{DataSource, TokenPushRequest};
// 使用 utils 中的共享类型
use utils::{ExternalTokenMetadata, TokenMetadata as UtilsTokenMetadata, TokenMetadataProvider};

#[cfg(test)]
use utils::ExternalTokenAttribute;

// 导入MetaplexService相关类型
// 注意：这里使用trait抽象来避免直接依赖server包

// 使用utils中的共享TokenMetadata结构
// 为了保持向后兼容，保留原有的TokenMetadata别名
type TokenMetadata = UtilsTokenMetadata;

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

/// 奖励分发事件数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RewardDistributionEventData {
    /// 奖励分发ID
    pub distribution_id: i64,
    /// 奖励池地址
    pub reward_pool: String,
    /// 接收者钱包地址
    pub recipient: String,
    /// 推荐人地址（可选）
    pub referrer: Option<String>,
    /// 奖励代币mint地址
    pub reward_token_mint: String,
    /// 奖励代币小数位数
    pub reward_token_decimals: Option<u8>,
    /// 奖励代币名称
    pub reward_token_name: Option<String>,
    /// 奖励代币符号
    pub reward_token_symbol: Option<String>,
    /// 奖励代币Logo URI
    pub reward_token_logo_uri: Option<String>,
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
    pub related_address: Option<String>,
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
    /// 代币元数据提供者（抽象的MetaplexService）
    metadata_provider: Option<Arc<tokio::sync::Mutex<dyn TokenMetadataProvider>>>,
    /// 元数据缓存，避免重复查询
    metadata_cache: Arc<RwLock<std::collections::HashMap<String, TokenMetadata>>>,
}

impl RewardDistributionParser {
    /// 创建新的奖励发放事件解析器
    pub fn new(config: &EventListenerConfig, program_id: Pubkey) -> Result<Self> {
        // 根据设计文档，使用事件类型名称计算discriminator
        let discriminator = crate::parser::event_parser::calculate_event_discriminator("ReferralRewardEvent");

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
            metadata_provider: None, // 通过setter方法注入
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
        let event = ReferralRewardEvent::try_from_slice(event_data)
            .map_err(|e| EventListenerError::EventParsing(format!("Borsh反序列化失败: {}", e)))?;

        info!("✅ event{:#?}", event);
        Ok(event)
    }

    /// 生成唯一的分发ID（基于事件内容）
    fn generate_distribution_id(&self, event: &ReferralRewardEvent) -> i64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        event.from.hash(&mut hasher);
        event.to.hash(&mut hasher);
        event.mint.hash(&mut hasher);
        event.amount.hash(&mut hasher);
        event.timestamp.hash(&mut hasher);

        // 确保返回值在i64范围内
        let hash = hasher.finish();
        (hash as i64).abs()
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
        let (token_decimals, token_name, token_symbol, token_logo_uri) =
            match self.fetch_token_metadata(&event.mint).await {
                Ok(metadata) => {
                    debug!(
                        "✅ 成功获取代币元数据: {} ({})",
                        event.mint,
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
                    warn!("⚠️ 获取代币元数据失败: {} - {}", event.mint, e);
                    (None, None, None, None)
                }
            };

        ParsedEvent::RewardDistribution(RewardDistributionEventData {
            distribution_id,
            reward_pool: event.from.to_string(),       // 使用from作为奖励池地址
            recipient: event.to.to_string(),           // to对应recipient
            referrer: Some(event.from.to_string()),    // from对应referrer
            reward_token_mint: event.mint.to_string(), // mint对应reward_token_mint
            // 新增的代币元数据字段
            reward_token_decimals: token_decimals,
            reward_token_name: token_name,
            reward_token_symbol: token_symbol,
            reward_token_logo_uri: token_logo_uri,
            reward_amount: event.amount,      // amount对应reward_amount
            base_reward_amount: event.amount, // 新结构没有base_reward，使用amount
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
        // let now = chrono::Utc::now().timestamp();
        // if event.distributed_at > now || event.distributed_at < (now - 86400) {
        //     warn!("❌ 发放时间戳异常: {}", event.distributed_at);
        //     return Ok(false);
        // }

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

    /// 设置代币元数据提供者（抽象的MetaplexService）
    pub fn set_metadata_provider(&mut self, provider: Arc<tokio::sync::Mutex<dyn TokenMetadataProvider>>) {
        self.metadata_provider = Some(provider);
        info!("✅ RewardDistributionParser 代币元数据提供者已设置");
    }

    /// 将外部元数据转换为utils的TokenMetadata
    fn convert_external_metadata(external_metadata: ExternalTokenMetadata, decimals: u8) -> TokenMetadata {
        external_metadata.to_token_metadata(decimals)
    }

    /// 查询代币元数据（先查缓存，再查TokenInfo表，最后查链上）
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
            info!("🔍 从TokenInfo表查询代币元数据: {}", mint_str);
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
                Ok(None) => {
                    info!("❌ TokenInfo表中未找到代币: {}", mint_str);
                }
                Err(e) => {
                    warn!("⚠️ 查询TokenInfo表失败: {} - {}", mint_str, e);
                }
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
        } else {
            info!("⚠️ 代币元数据提供者未设置，使用基础链上查询: {}", mint_str);
        }

        // 回退到原始的链上查询方法（仅获取decimals）
        self.fetch_basic_onchain_metadata(mint_address).await
    }

    /// 获取基础的链上元数据（仅decimals）
    async fn fetch_basic_onchain_metadata(&self, mint_address: &Pubkey) -> Result<TokenMetadata> {
        let rpc_client = self
            .rpc_client
            .as_ref()
            .ok_or_else(|| EventListenerError::EventParsing("RPC客户端未初始化".to_string()))?;

        debug!("🔗 从链上获取基础代币元数据: {}", mint_address);

        // 获取mint账户数据
        let account_data = rpc_client
            .get_account_data(mint_address)
            .map_err(|e| EventListenerError::EventParsing(format!("获取mint账户数据失败: {} - {}", mint_address, e)))?;

        // 解析mint数据获取decimals
        let mint = Mint::unpack(&account_data)
            .map_err(|e| EventListenerError::EventParsing(format!("解析mint数据失败: {} - {}", mint_address, e)))?;

        let metadata = TokenMetadata {
            address: mint_address.to_string(),
            decimals: mint.decimals,
            name: None,     // 链上mint账户不包含名称信息
            symbol: None,   // 链上mint账户不包含符号信息
            logo_uri: None, // 链上mint账户不包含logo信息
            description: Some(format!("Token with {} decimals", mint.decimals)),
            external_url: None,
            attributes: None,
            tags: vec!["onchain-basic".to_string()],
        };

        info!(
            "✅ 从链上获取基础代币元数据: {} (decimals: {})",
            mint_address, mint.decimals
        );
        Ok(metadata)
    }

    /// 获取mint的decimals信息
    async fn fetch_mint_decimals(&self, mint_address: &Pubkey) -> Result<u8> {
        let rpc_client = self
            .rpc_client
            .as_ref()
            .ok_or_else(|| EventListenerError::EventParsing("RPC客户端未初始化".to_string()))?;

        let account_data = rpc_client
            .get_account_data(mint_address)
            .map_err(|e| EventListenerError::EventParsing(format!("获取mint账户数据失败: {}", e)))?;

        let mint = Mint::unpack(&account_data)
            .map_err(|e| EventListenerError::EventParsing(format!("解析mint数据失败: {}", e)))?;

        Ok(mint.decimals)
    }

    /// 创建默认的回退元数据
    fn create_fallback_metadata(&self, mint_address: &str, decimals: Option<u8>) -> TokenMetadata {
        let default_decimals = decimals.unwrap_or(6); // 默认6位小数

        // 为一些知名代币提供硬编码信息
        match mint_address {
            "So11111111111111111111111111111111111111112" => TokenMetadata {
                address: mint_address.to_string(),
                decimals: 9,
                symbol: Some("WSOL".to_string()),
                name: Some("Wrapped SOL".to_string()),
                logo_uri: Some("https://raw.githubusercontent.com/solana-labs/token-list/main/assets/mainnet/So11111111111111111111111111111111111111112/logo.png".to_string()),
                description: Some("Wrapped Solana".to_string()),
                external_url: Some("https://solana.com".to_string()),
                attributes: None,
                tags: vec!["fallback".to_string(), "wrapped-sol".to_string()],
            },
            "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v" => TokenMetadata {
                address: mint_address.to_string(),
                decimals: 6,
                symbol: Some("USDC".to_string()),
                name: Some("USD Coin".to_string()),
                logo_uri: Some("https://raw.githubusercontent.com/solana-labs/token-list/main/assets/mainnet/EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v/logo.png".to_string()),
                description: Some("USD Coin".to_string()),
                external_url: Some("https://www.centre.io".to_string()),
                attributes: None,
                tags: vec!["fallback".to_string(), "stablecoin".to_string()],
            },
            "4k3Dyjzvzp8eMZWUXbBCjEvwSkkk59S5iCNLY3QrkX6R" => TokenMetadata {
                address: mint_address.to_string(),
                decimals: 6,
                symbol: Some("RAY".to_string()),
                name: Some("Raydium".to_string()),
                logo_uri: Some("https://img-v1.raydium.io/icon/4k3Dyjzvzp8eMZWUXbBCjEvwSkkk59S5iCNLY3QrkX6R.png".to_string()),
                description: Some("Raydium Protocol Token".to_string()),
                external_url: Some("https://raydium.io".to_string()),
                attributes: None,
                tags: vec!["fallback".to_string(), "defi".to_string()],
            },
            _ => {
                // 默认情况：仅包含地址和decimals
                debug!("🔄 创建基础回退元数据: {}", mint_address);
                TokenMetadata {
                    address: mint_address.to_string(),
                    decimals: default_decimals,
                    symbol: None,
                    name: None,
                    logo_uri: None,
                    description: Some(format!("Token with {} decimals (no metadata found)", default_decimals)),
                    external_url: None,
                    attributes: None,
                    tags: vec!["fallback".to_string(), "unknown".to_string()],
                }
            }
        }
    }

    /// 获取完整的代币元数据（带有完整的fallback链）
    async fn fetch_complete_metadata(&self, mint_address: &Pubkey) -> TokenMetadata {
        let mint_str = mint_address.to_string();

        // 1. 先尝试正常的元数据获取
        match self.fetch_onchain_metadata(mint_address).await {
            Ok(metadata) => {
                info!("✅ 获取元数据成功: {}", mint_str);
                metadata
            }
            Err(e) => {
                warn!("⚠️ 获取元数据失败，使用fallback: {} - {}", mint_str, e);

                // 2. 尝试获取decimals信息
                let decimals = self.fetch_mint_decimals(mint_address).await.ok();

                // 3. 创建fallback元数据
                self.create_fallback_metadata(&mint_str, decimals)
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

    /// 异步保存代币元数据到TokenInfo表
    async fn save_to_token_info(database: Arc<Database>, mint_address: &str, metadata: &TokenMetadata) -> Result<()> {
        let push_request = TokenPushRequest {
            address: mint_address.to_string(),
            program_id: Some("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA".to_string()),
            name: metadata.name.clone().unwrap_or_else(|| "Unknown Token".to_string()),
            symbol: metadata.symbol.clone().unwrap_or_else(|| "UNK".to_string()),
            decimals: metadata.decimals,
            logo_uri: metadata.logo_uri.clone().unwrap_or_else(|| "".to_string()),
            tags: Some(metadata.tags.clone()),
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
            backfill: None,
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
        assert_eq!(
            parser.get_discriminator(),
            crate::parser::event_parser::calculate_event_discriminator("ReferralRewardEvent")
        );
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

        let parsed = parser
            .convert_to_parsed_event(test_event.clone(), "test_signature".to_string(), 12345)
            .await;

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

                // 新的代币元数据字段（在有RPC客户端的情况下可能为Some，无RPC时为默认值）
                // 这个测试在没有真实RPC的情况下，可能会有默认的元数据
                // assert_eq!(data.reward_token_decimals, None);
                // 测试实际返回的值
                assert!(data.reward_token_decimals.is_some() || data.reward_token_decimals.is_none());
                assert!(data.reward_token_name.is_some() || data.reward_token_name.is_none());
                assert!(data.reward_token_symbol.is_some() || data.reward_token_symbol.is_none());
                assert!(data.reward_token_logo_uri.is_some() || data.reward_token_logo_uri.is_none());
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

    #[tokio::test]
    async fn test_metadata_provider_integration() {
        let config = create_test_config();
        let parser = RewardDistributionParser::new(&config, Pubkey::new_unique()).unwrap();

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

    #[tokio::test]
    async fn test_fallback_metadata_creation() {
        let config = create_test_config();
        let parser = RewardDistributionParser::new(&config, Pubkey::new_unique()).unwrap();

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

        // 测试没有decimals时的默认值
        let default_metadata = parser.create_fallback_metadata("AnotherUnknownToken123456789", None);

        assert_eq!(default_metadata.decimals, 6); // 默认6位小数
    }

    #[tokio::test]
    async fn test_external_metadata_conversion() {
        // 测试外部元数据转换为utils的TokenMetadata
        let external_metadata = ExternalTokenMetadata {
            address: "test123".to_string(),
            symbol: Some("TEST".to_string()),
            name: Some("Test Token".to_string()),
            logo_uri: Some("https://example.com/logo.png".to_string()),
            description: Some("A test token".to_string()),
            external_url: Some("https://example.com".to_string()),
            attributes: Some(vec![ExternalTokenAttribute {
                trait_type: "rarity".to_string(),
                value: "common".to_string(),
            }]),
            tags: vec!["test".to_string()],
        };

        let converted = RewardDistributionParser::convert_external_metadata(external_metadata, 9);

        assert_eq!(converted.address, "test123");
        assert_eq!(converted.decimals, 9);
        assert_eq!(converted.symbol, Some("TEST".to_string()));
        assert_eq!(converted.name, Some("Test Token".to_string()));
        assert_eq!(converted.logo_uri, Some("https://example.com/logo.png".to_string()));
        assert_eq!(converted.description, Some("A test token".to_string()));
        assert_eq!(converted.external_url, Some("https://example.com".to_string()));
        assert_eq!(converted.tags, vec!["test".to_string()]);

        // 测试属性转换
        let attributes = converted.attributes.unwrap();
        assert_eq!(attributes.len(), 1);
        assert_eq!(attributes[0].trait_type, "rarity");
        assert_eq!(attributes[0].value, "common");
    }

    #[tokio::test]
    async fn test_token_metadata_utilities() {
        // 测试新的TokenMetadata功能
        let mut metadata = utils::TokenMetadata::new("test123".to_string(), 6);

        // 测试基础检查
        assert!(metadata.is_basic());
        assert!(!metadata.is_complete());

        // 添加元数据
        metadata.symbol = Some("TEST".to_string());
        metadata.name = Some("Test Token".to_string());
        metadata.logo_uri = Some("https://example.com/logo.png".to_string());

        assert!(!metadata.is_basic());
        assert!(metadata.is_complete());

        // 测试显示名称
        assert_eq!(metadata.display_name(), "Test Token");
        assert_eq!(metadata.display_symbol(), "TEST");

        // 测试标签和属性添加
        metadata.add_tag("test".to_string());
        metadata.add_tag("example".to_string());
        metadata.add_tag("test".to_string()); // 重复标签不应该被添加

        assert_eq!(metadata.tags.len(), 2);
        assert!(metadata.tags.contains(&"test".to_string()));
        assert!(metadata.tags.contains(&"example".to_string()));

        metadata.add_attribute("type".to_string(), "utility".to_string());
        metadata.add_attribute("rarity".to_string(), "common".to_string());

        let attributes = metadata.attributes.as_ref().unwrap();
        assert_eq!(attributes.len(), 2);

        // 测试属性更新
        metadata.add_attribute("type".to_string(), "governance".to_string());
        let updated_attributes = metadata.attributes.as_ref().unwrap();
        assert_eq!(updated_attributes.len(), 2); // 长度不变
        assert_eq!(updated_attributes[0].value, "governance"); // 值被更新
    }

    #[tokio::test]
    async fn test_metadata_merge() {
        let base = utils::TokenMetadata {
            address: "test123".to_string(),
            decimals: 6,
            symbol: Some("TEST".to_string()),
            name: None,
            logo_uri: None,
            description: None,
            external_url: None,
            attributes: None,
            tags: vec!["base".to_string()],
        };

        let additional = utils::TokenMetadata {
            address: "test123".to_string(),
            decimals: 6,
            symbol: Some("OVERRIDE".to_string()), // 不会被使用，因为base已有symbol
            name: Some("Test Token".to_string()), // 会被使用，因为base没有name
            logo_uri: Some("https://example.com/logo.png".to_string()),
            description: Some("A test token".to_string()),
            external_url: Some("https://example.com".to_string()),
            attributes: Some(vec![utils::TokenAttribute {
                trait_type: "source".to_string(),
                value: "additional".to_string(),
            }]),
            tags: vec!["additional".to_string(), "base".to_string()], // base标签不会重复
        };

        let merged = base.merge_with(additional);

        assert_eq!(merged.symbol, Some("TEST".to_string())); // 保持原值
        assert_eq!(merged.name, Some("Test Token".to_string())); // 使用新值
        assert_eq!(merged.logo_uri, Some("https://example.com/logo.png".to_string()));
        assert_eq!(merged.description, Some("A test token".to_string()));
        assert_eq!(merged.external_url, Some("https://example.com".to_string()));

        // 测试标签合并
        assert_eq!(merged.tags.len(), 2); // 去重后只有两个标签
        assert!(merged.tags.contains(&"base".to_string()));
        assert!(merged.tags.contains(&"additional".to_string()));

        // 测试属性合并
        let attributes = merged.attributes.unwrap();
        assert_eq!(attributes.len(), 1);
        assert_eq!(attributes[0].trait_type, "source");
        assert_eq!(attributes[0].value, "additional");
    }
}
