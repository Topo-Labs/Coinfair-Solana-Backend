use crate::{
    config::EventListenerConfig,
    error::{EventListenerError, Result},
    parser::{
        event_parser::{
            DepositEventData, LaunchEventData, NftClaimEventData, PoolCreatedEventData, RewardDistributionEventData,
            SwapEventData, TokenCreationEventData,
        },
        ParsedEvent,
    },
    services::migration_client::MigrationClient,
};
use chrono::Utc;
use database::{
    clmm_pool::{
        ClmmPool, ClmmPoolRepository, DataSource, ExtensionInfo, PoolStatus, PriceInfo, SyncStatus, TokenInfo,
        TransactionInfo, TransactionStatus, VaultInfo,
    },
    event_model::{
        repository::TokenCreationEventRepository, ClmmPoolEvent, LaunchEvent, MigrationStatus, NftClaimEvent,
        RewardDistributionEvent, TokenCreationEvent,
    },
    token_info::{TokenInfoRepository, TokenPushRequest},
    Database,
};
use mongodb::bson::doc;
use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use std::sync::Arc;
use tracing::{debug, error, info, warn};
use utils::config::{AppConfig, EventListenerDbMode};
use utils::metaplex_service::{MetaplexConfig, MetaplexService};

/// 事件存储接口
///
/// 负责将解析后的事件持久化到数据库
/// 支持批量写入和事务处理
pub struct EventStorage {
    config: Arc<EventListenerConfig>,
    database: Arc<Database>,
    token_repository: Arc<TokenInfoRepository>,
    clmm_pool_repository: Arc<ClmmPoolRepository>,
    token_creation_event_repository: Arc<TokenCreationEventRepository>,
    app_config: Arc<AppConfig>,
    migration_client: Arc<MigrationClient>,
}

impl EventStorage {
    /// 合并/更新 TokenInfo.extensions（存款驱动的最小集字段）
    fn merge_extensions_for_deposit(
        mut extensions: serde_json::Value,
        project_config: &str,
        project_state: u8,
        total_raised: u64,
    ) -> serde_json::Value {
        if !extensions.is_object() {
            extensions = serde_json::json!({});
        }

        extensions["project_config"] = serde_json::Value::String(project_config.to_string());
        extensions["project_state"] = serde_json::Value::Number(serde_json::Number::from(project_state));
        extensions["total_raised"] = serde_json::Value::Number(serde_json::Number::from(total_raised));

        extensions
    }
    /// 创建新的事件存储
    pub async fn new(config: &EventListenerConfig) -> Result<Self> {
        let config = Arc::new(config.clone());

        // 创建数据库连接
        // 创建AppConfig（避免clap参数解析）
        let app_config = Arc::new(utils::config::AppConfig {
            cargo_env: utils::config::CargoEnv::Development, // 测试环境默认使用Development
            app_host: "0.0.0.0".to_string(),
            app_port: 8765,
            mongo_uri: config.database.uri.clone(),
            mongo_db: config.database.database_name.clone(),
            rpc_url: "https://api.devnet.solana.com".to_string(),
            private_key: None,
            raydium_program_id: "FA1RJDDXysgwg5Gm3fJXWxt26JQzPkAzhTA114miqNUX".to_string(),
            amm_config_index: 0,
            rust_log: "info".to_string(),
            // 读取环境变量
            enable_pool_event_insert: std::env::var("ENABLE_POOL_EVENT_INSERT")
                .unwrap_or_else(|_| "false".to_string())
                .parse()
                .unwrap_or(false),
            event_listener_db_mode: std::env::var("EVENT_LISTENER_DB_MODE")
                .unwrap_or_else(|_| "update_only".to_string()),
        });

        // 创建数据库实例
        let database = Arc::new(
            Database::new(app_config.clone())
                .await
                .map_err(|e| EventListenerError::Persistence(format!("数据库初始化失败: {}", e)))?,
        );

        // 创建代币信息仓库
        let token_repository = Arc::new(database.token_info_repository.clone());

        // 创建CLMM池子仓库
        let clmm_pool_repository = Arc::new(database.clmm_pool_repository.clone());

        // 创建代币创建事件仓库
        let token_creation_event_repository = Arc::new(database.token_creation_event_repository.clone());

        // 创建迁移客户端
        let migration_base_url =
            std::env::var("MIGRATION_BASE_URL").unwrap_or_else(|_| "http://localhost:8000".to_string());
        let migration_client = Arc::new(MigrationClient::new(migration_base_url));

        info!("✅ 事件存储初始化完成，数据库: {}", config.database.database_name);
        info!(
            "📊 事件监听器配置: enable_insert={}, mode={}",
            app_config.enable_pool_event_insert, app_config.event_listener_db_mode
        );

        Ok(Self {
            config,
            database,
            token_repository,
            clmm_pool_repository,
            token_creation_event_repository,
            app_config,
            migration_client,
        })
    }

    /// 批量写入事件
    pub async fn write_batch(&self, events: &[ParsedEvent]) -> Result<u64> {
        if events.is_empty() {
            return Ok(0);
        }

        debug!("💾 开始批量写入 {} 个事件", events.len());

        let mut written_count = 0u64;

        // 按事件类型分组处理
        let mut token_creation_events = Vec::new();
        let mut pool_creation_events = Vec::new();
        let mut nft_claim_events = Vec::new();
        let mut reward_distribution_events = Vec::new();
        let mut launch_events = Vec::new();
        let mut swap_events = Vec::new();
        let mut deposit_events = Vec::new();

        for event in events {
            match event {
                ParsedEvent::TokenCreation(token_event) => {
                    token_creation_events.push(token_event);
                }
                ParsedEvent::PoolCreation(pool_event) => {
                    pool_creation_events.push(pool_event);
                }
                ParsedEvent::NftClaim(nft_event) => {
                    nft_claim_events.push(nft_event);
                }
                ParsedEvent::RewardDistribution(reward_event) => {
                    reward_distribution_events.push(reward_event);
                }
                ParsedEvent::Launch(launch_event) => {
                    launch_events.push(launch_event);
                }
                ParsedEvent::Swap(swap_event) => {
                    swap_events.push(swap_event);
                }
                ParsedEvent::Deposit(deposit_event) => {
                    deposit_events.push(deposit_event);
                }
            }
        }

        // 批量处理代币创建事件
        if !token_creation_events.is_empty() {
            match self.write_token_creation_batch(&token_creation_events).await {
                Ok(count) => {
                    written_count += count;
                    info!("✅ 成功写入 {} 个代币创建事件", count);
                }
                Err(e) => {
                    error!("❌ 代币创建事件批量写入失败: {}", e);
                    return Err(e);
                }
            }
        }

        // 批量处理池子创建事件
        if !pool_creation_events.is_empty() {
            match self.write_pool_creation_batch(&pool_creation_events).await {
                Ok(count) => {
                    written_count += count;
                    info!("✅ 成功写入 {} 个池子创建事件", count);
                }
                Err(e) => {
                    error!("❌ 池子创建事件批量写入失败: {}", e);
                    return Err(e);
                }
            }
        }

        // 批量处理NFT领取事件
        if !nft_claim_events.is_empty() {
            match self.write_nft_claim_batch(&nft_claim_events).await {
                Ok(count) => {
                    written_count += count;
                    info!("✅ 成功写入 {} 个NFT领取事件", count);
                }
                Err(e) => {
                    error!("❌ NFT领取事件批量写入失败: {}", e);
                    return Err(e);
                }
            }
        }

        // 批量处理奖励分发事件
        if !reward_distribution_events.is_empty() {
            match self.write_reward_distribution_batch(&reward_distribution_events).await {
                Ok(count) => {
                    written_count += count;
                    info!("✅ 成功写入 {} 个奖励分发事件", count);
                }
                Err(e) => {
                    error!("❌ 奖励分发事件批量写入失败: {}", e);
                    return Err(e);
                }
            }
        }

        // 批量处理LaunchEvent
        if !launch_events.is_empty() {
            match self.write_launch_batch(&launch_events).await {
                Ok(count) => {
                    written_count += count;
                    info!("✅ 成功写入 {} 个Launch事件", count);
                }
                Err(e) => {
                    error!("❌ Launch事件批量写入失败: {}", e);
                    return Err(e);
                }
            }
        }

        // 批量处理交换事件
        if !swap_events.is_empty() {
            match self.write_swap_batch(&swap_events).await {
                Ok(count) => {
                    written_count += count;
                    info!("✅ 成功写入 {} 个交换事件", count);
                }
                Err(e) => {
                    error!("❌ 交换事件批量写入失败: {}", e);
                    return Err(e);
                }
            }
        }

        // 批量处理存款事件
        if !deposit_events.is_empty() {
            match self.write_deposit_batch(&deposit_events).await {
                Ok(count) => {
                    written_count += count;
                    info!("✅ 成功写入 {} 个存款事件", count);
                }
                Err(e) => {
                    error!("❌ 存款事件批量写入失败: {}", e);
                    return Err(e);
                }
            }
        }

        debug!("✅ 批量写入完成，总计写入: {} 个事件", written_count);
        Ok(written_count)
    }

    /// 批量写入池子创建事件
    async fn write_pool_creation_batch(&self, events: &[&PoolCreatedEventData]) -> Result<u64> {
        let mut written_count = 0u64;

        for event in events {
            match self.write_single_pool_creation(event).await {
                Ok(true) => {
                    written_count += 1;
                    debug!("✅ 池子创建事件已写入: {}", event.pool_address);
                }
                Ok(false) => {
                    debug!("ℹ️ 池子创建事件已存在，跳过: {}", event.pool_address);
                }
                Err(e) => {
                    // 检查是否为重复键错误
                    if self.is_duplicate_key_error(&e) {
                        debug!("ℹ️ 池子创建事件已存在（重复键），跳过: {}", event.pool_address);
                        continue;
                    }

                    error!("❌ 池子创建事件写入失败: {} - {}", event.pool_address, e);

                    if self.is_fatal_error(&e) {
                        return Err(e);
                    }

                    warn!("⚠️ 跳过失败的事件: {}", event.pool_address);
                }
            }
        }

        Ok(written_count)
    }

    /// 批量写入NFT领取事件
    async fn write_nft_claim_batch(&self, events: &[&NftClaimEventData]) -> Result<u64> {
        let mut written_count = 0u64;

        for event in events {
            match self.write_single_nft_claim(event).await {
                Ok(true) => {
                    written_count += 1;
                    debug!("✅ NFT领取事件已写入: {} by {}", event.nft_mint, event.claimer);
                }
                Ok(false) => {
                    debug!("ℹ️ NFT领取事件已存在，跳过: {} by {}", event.nft_mint, event.claimer);
                }
                Err(e) => {
                    // 检查是否为重复键错误
                    if self.is_duplicate_key_error(&e) {
                        debug!(
                            "ℹ️ NFT领取事件已存在（重复键），跳过: {} by {}",
                            event.nft_mint, event.claimer
                        );
                        continue;
                    }

                    error!(
                        "❌ NFT领取事件写入失败: {} by {} - {}",
                        event.nft_mint, event.claimer, e
                    );

                    if self.is_fatal_error(&e) {
                        return Err(e);
                    }

                    warn!("⚠️ 跳过失败的事件: {} by {}", event.nft_mint, event.claimer);
                }
            }
        }

        Ok(written_count)
    }

    /// 批量写入奖励分发事件
    async fn write_reward_distribution_batch(&self, events: &[&RewardDistributionEventData]) -> Result<u64> {
        let mut written_count = 0u64;

        for event in events {
            match self.write_single_reward_distribution(event).await {
                Ok(true) => {
                    written_count += 1;
                    debug!(
                        "✅ 奖励分发事件已写入: {} to {}",
                        event.distribution_id, event.recipient
                    );
                }
                Ok(false) => {
                    debug!(
                        "ℹ️ 奖励分发事件已存在，跳过: {} to {}",
                        event.distribution_id, event.recipient
                    );
                }
                Err(e) => {
                    // 检查是否为重复键错误
                    if self.is_duplicate_key_error(&e) {
                        debug!(
                            "ℹ️ 奖励分发事件已存在（重复键），跳过: {} to {}",
                            event.distribution_id, event.recipient
                        );
                        continue;
                    }

                    error!(
                        "❌ 奖励分发事件写入失败: {} to {} - {}",
                        event.distribution_id, event.recipient, e
                    );

                    if self.is_fatal_error(&e) {
                        return Err(e);
                    }

                    warn!("⚠️ 跳过失败的事件: {} to {}", event.distribution_id, event.recipient);
                }
            }
        }

        Ok(written_count)
    }

    /// 批量写入交换事件
    async fn write_swap_batch(&self, events: &[&SwapEventData]) -> Result<u64> {
        let mut written_count = 0u64;

        for event in events {
            match self.write_single_swap(event).await {
                Ok(true) => {
                    written_count += 1;
                    debug!("✅ 交换事件已写入: {} in pool {}", event.signature, event.pool_address);
                }
                Ok(false) => {
                    debug!(
                        "ℹ️ 交换事件已存在，跳过: {} in pool {}",
                        event.signature, event.pool_address
                    );
                }
                Err(e) => {
                    // 检查是否为重复键错误
                    if self.is_duplicate_key_error(&e) {
                        debug!(
                            "ℹ️ 交换事件已存在（重复键），跳过: {} in pool {}",
                            event.signature, event.pool_address
                        );
                        continue;
                    }

                    error!(
                        "❌ 交换事件写入失败: {} in pool {} - {}",
                        event.signature, event.pool_address, e
                    );

                    if self.is_fatal_error(&e) {
                        return Err(e);
                    }

                    warn!("⚠️ 跳过失败的事件: {} in pool {}", event.signature, event.pool_address);
                }
            }
        }

        Ok(written_count)
    }

    /// 批量写入存款事件
    async fn write_deposit_batch(&self, events: &[&DepositEventData]) -> Result<u64> {
        let mut written_count = 0u64;

        for event in events {
            match self.write_single_deposit(event).await {
                Ok(true) => {
                    written_count += 1;
                    debug!("✅ 存款事件已写入: {} to {}", event.user, event.token_mint);
                }
                Ok(false) => {
                    debug!("ℹ️ 存款事件已存在，跳过: {} to {}", event.user, event.token_mint);
                }
                Err(e) => {
                    // 检查是否为重复键错误
                    if self.is_duplicate_key_error(&e) {
                        debug!(
                            "ℹ️ 存款事件已存在（重复键），跳过: {} to {}",
                            event.user, event.token_mint
                        );
                        continue;
                    }

                    error!("❌ 存款事件写入失败: {} to {} - {}", event.user, event.token_mint, e);

                    if self.is_fatal_error(&e) {
                        return Err(e);
                    }

                    warn!("⚠️ 跳过失败的事件: {} to {}", event.user, event.token_mint);
                }
            }
        }

        Ok(written_count)
    }

    async fn write_token_creation_batch(&self, events: &[&TokenCreationEventData]) -> Result<u64> {
        let mut written_count = 0u64;

        for event in events {
            match self.write_single_token_creation(event).await {
                Ok(true) => {
                    written_count += 1;
                    debug!("✅ 代币创建事件已写入: {} ({})", event.symbol, event.mint_address);
                }
                Ok(false) => {
                    debug!("ℹ️ 代币创建事件已存在，跳过: {} ({})", event.symbol, event.mint_address);
                }
                Err(e) => {
                    // 检查是否为重复键错误
                    if self.is_duplicate_key_error(&e) {
                        debug!(
                            "ℹ️ 代币创建事件已存在（重复键），跳过: {} ({})",
                            event.symbol, event.mint_address
                        );
                        continue;
                    }

                    error!(
                        "❌ 代币创建事件写入失败: {} ({}) - {}",
                        event.symbol, event.mint_address, e
                    );

                    // 根据错误类型决定是否继续
                    if self.is_fatal_error(&e) {
                        return Err(e);
                    }

                    // 非致命错误，记录但继续处理其他事件
                    warn!("⚠️ 跳过失败的事件: {} ({})", event.symbol, event.mint_address);
                }
            }
        }

        Ok(written_count)
    }

    /// 写入单个代币创建事件
    async fn write_single_token_creation(&self, event: &TokenCreationEventData) -> Result<bool> {
        // 1. 检查事件表中是否已存在（防重复）
        let event_exists = self
            .token_creation_event_repository
            .exists_by_signature(&event.signature)
            .await
            .map_err(|e| EventListenerError::Persistence(format!("检查事件是否存在失败: {}", e)))?;

        if !event_exists {
            // 2. 写入到事件表
            let token_creation_event = self.convert_to_token_creation_event(event)?;
            let _event_id = self
                .token_creation_event_repository
                .insert_token_creation_event(token_creation_event)
                .await
                .map_err(|e| EventListenerError::Persistence(format!("插入代币创建事件失败: {}", e)))?;
            info!("写入到事件表成功: {}", event.signature);
        } else {
            info!("✅ 代币创建事件已存在，跳过: {}", event.signature);
        }

        // 3. 检查代币信息表中是否已存在
        let existing = self
            .token_repository
            .find_by_address(&event.mint_address.to_string())
            .await
            .map_err(|e| EventListenerError::Persistence(format!("查询现有代币失败: {}", e)))?;
        if existing.is_none() {
            // 4. 写入到代币信息表
            let push_request = self.convert_to_push_token_request(event)?;
            let response = self
                .token_repository
                .push_token(push_request)
                .await
                .map_err(|e| EventListenerError::Persistence(format!("推送代币信息失败: {}", e)))?;
            if !response.success {
                return Err(EventListenerError::Persistence(format!(
                    "代币信息推送失败: {}",
                    response.message
                )));
            }
            info!("✅ 代币信息写入到代币信息表成功: {}", event.mint_address);
            return Ok(true);
        } else {
            info!("代币信息已存在，跳过: {}", event.mint_address);
        }

        Ok(false)
    }

    /// 写入单个池子创建事件（改造版：更新ClmmPool表）
    async fn write_single_pool_creation(&self, event: &PoolCreatedEventData) -> Result<bool> {
        info!("🔄 处理链上池子创建事件: {}", event.pool_address);

        // 1. 查找是否有对应的API创建记录
        let existing_pool = self
            .clmm_pool_repository
            .find_by_pool_address(&event.pool_address)
            .await
            .map_err(|e| EventListenerError::Persistence(format!("查询池子失败: {}", e)))?;

        match existing_pool {
            Some(mut pool) => {
                // 2. 存在记录，执行智能更新
                info!("📝 找到已存在的池子记录，执行更新: {}", event.pool_address);
                self.smart_update_pool_from_event(&mut pool, event).await
            }
            None => {
                // 3. 不存在记录，检查是否允许插入
                if self.app_config.enable_pool_event_insert {
                    info!("🆕 池子不存在且允许插入，从链上事件创建新记录: {}", event.pool_address);
                    self.create_pool_from_chain_event(event).await
                } else {
                    warn!(
                        "⚠️ 池子不存在但禁止插入新记录(ENABLE_POOL_EVENT_INSERT=false): {}",
                        event.pool_address
                    );

                    // 仍然保存到ClmmPoolEvent作为审计记录
                    self.save_pool_event_as_audit_log(event).await?;

                    Ok(false)
                }
            }
        }
    }

    /// 写入单个NFT领取事件
    async fn write_single_nft_claim(&self, event: &NftClaimEventData) -> Result<bool> {
        // 转换为数据库模型
        let nft_event = self.convert_to_nft_claim_event(event)?;

        // 插入数据库
        self.database
            .nft_claim_event_repository
            .insert_nft_claim_event(nft_event)
            .await
            .map_err(|e| EventListenerError::Persistence(format!("插入NFT领取事件失败: {}", e)))?;

        Ok(true)
    }

    /// 写入单个奖励分发事件
    async fn write_single_reward_distribution(&self, event: &RewardDistributionEventData) -> Result<bool> {
        // 检查是否已存在
        let existing = self
            .database
            .reward_distribution_event_repository
            .find_by_distribution_id(event.distribution_id)
            .await
            .map_err(|e| EventListenerError::Persistence(format!("查询现有奖励事件失败: {}", e)))?;

        if existing.is_some() {
            debug!("奖励事件已存在，跳过: {}", event.distribution_id);
            return Ok(false);
        }

        // 转换为数据库模型
        let reward_event = self.convert_to_reward_distribution_event(event)?;

        // 插入数据库
        self.database
            .reward_distribution_event_repository
            .insert_reward_event(reward_event)
            .await
            .map_err(|e| EventListenerError::Persistence(format!("插入奖励分发事件失败: {}", e)))?;

        Ok(true)
    }

    /// 写入单个交换事件
    async fn write_single_swap(&self, event: &SwapEventData) -> Result<bool> {
        // 交换事件通常不需要去重（每个签名都是唯一的）
        // 但可以根据业务需求添加
        info!(
            "💱 记录交换事件: {} in pool {}, amount: {}→{}",
            event.signature, event.pool_address, event.amount_0, event.amount_1
        );

        // 目前只记录日志，可以根据需求添加数据库存储
        // 例如：存储到交易历史表、更新池子统计等

        // 这里可以添加实际的数据库写入逻辑
        // 例如：更新池子的交易量、价格等

        Ok(true)
    }

    /// 批量写入Launch事件
    async fn write_launch_batch(&self, events: &[&LaunchEventData]) -> Result<u64> {
        let mut written_count = 0u64;

        for event in events {
            match self.write_single_launch_event(event).await {
                Ok(true) => {
                    written_count += 1;
                    debug!(
                        "✅ Launch事件已写入: {} by {}",
                        event.meme_token_mint, event.user_wallet
                    );
                }
                Ok(false) => {
                    debug!(
                        "ℹ️ Launch事件已存在，跳过: {} by {}",
                        event.meme_token_mint, event.user_wallet
                    );
                }
                Err(e) => {
                    // 检查是否为重复键错误
                    if self.is_duplicate_key_error(&e) {
                        debug!(
                            "ℹ️ Launch事件已存在（重复键），跳过: {} by {}",
                            event.meme_token_mint, event.user_wallet
                        );
                        continue;
                    }

                    error!(
                        "❌ Launch事件写入失败: {} by {} - {}",
                        event.meme_token_mint, event.user_wallet, e
                    );

                    if self.is_fatal_error(&e) {
                        return Err(e);
                    }

                    warn!("⚠️ 跳过失败的事件: {} by {}", event.meme_token_mint, event.user_wallet);
                }
            }
        }

        Ok(written_count)
    }

    /// 写入单个Launch事件
    async fn write_single_launch_event(&self, event: &LaunchEventData) -> Result<bool> {
        // 检查是否已存在
        let existing = self
            .database
            .launch_event_repository
            .find_by_signature(&event.signature)
            .await
            .map_err(|e| EventListenerError::Persistence(format!("查询现有Launch事件失败: {}", e)))?;

        if existing.is_some() {
            debug!("Launch事件已存在，跳过: {}", event.signature);
            return Ok(false);
        }

        // 转换为数据库模型
        let launch_event = self.convert_to_launch_event(event)?;

        // 1. 立即插入数据库记录（状态：pending）
        let event_id = self
            .database
            .launch_event_repository
            .insert_launch_event(launch_event)
            .await
            .map_err(|e| EventListenerError::Persistence(format!("插入Launch事件失败: {}", e)))?;

        info!("✅ Launch事件已写入数据库: {} (id: {})", event.signature, event_id);

        // 2. 异步触发迁移操作（不阻塞当前流程）
        let event_data = event.clone();
        let database = Arc::clone(&self.database);
        let migration_client = Arc::clone(&self.migration_client);
        let signature = event.signature.clone();

        tokio::spawn(async move {
            info!("🚀 异步触发Launch事件迁移: {}", signature);

            // 调用真实的迁移API
            match migration_client.trigger_launch_migration(&event_data).await {
                Ok(response) => {
                    info!(
                        "✅ Launch迁移成功: signature={}, pool_address={}",
                        signature, response.pool_address
                    );

                    // 更新为成功状态
                    match database
                        .launch_event_repository
                        .update_migration_status(
                            &signature,
                            MigrationStatus::Success,
                            Some(response.pool_address),
                            None,
                        )
                        .await
                    {
                        Ok(_) => {
                            info!("✅ Launch事件迁移状态更新为成功: {}", signature);
                        }
                        Err(e) => {
                            error!("❌ Launch事件迁移状态更新失败: {} - {}", signature, e);
                        }
                    }
                }
                Err(e) => {
                    error!("❌ Launch迁移失败: {} - {}", signature, e);

                    // 更新为失败状态并记录错误信息
                    match database
                        .launch_event_repository
                        .update_migration_status(&signature, MigrationStatus::Failed, None, Some(e.to_string()))
                        .await
                    {
                        Ok(_) => {
                            info!("✅ Launch事件迁移状态更新为失败: {}", signature);
                        }
                        Err(update_e) => {
                            error!("❌ Launch事件迁移状态更新失败: {} - {}", signature, update_e);
                        }
                    }
                }
            }
        });

        Ok(true)
    }

    /// 将池子创建事件转换为数据库模型
    fn convert_to_pool_event(&self, event: &PoolCreatedEventData) -> Result<ClmmPoolEvent> {
        let now = Utc::now().timestamp();

        Ok(ClmmPoolEvent {
            id: None,
            pool_address: event.pool_address.clone(),
            token_a_mint: event.token_a_mint.clone(),
            token_b_mint: event.token_b_mint.clone(),
            token_a_decimals: event.token_a_decimals,
            token_b_decimals: event.token_b_decimals,
            fee_rate: event.fee_rate,
            fee_rate_percentage: event.fee_rate_percentage,
            annual_fee_rate: event.annual_fee_rate,
            pool_type: event.pool_type.clone(),
            sqrt_price_x64: event.sqrt_price_x64.clone(),
            initial_price: event.initial_price,
            initial_tick: event.initial_tick,
            creator: event.creator.clone(),
            clmm_config: event.clmm_config.clone(),
            is_stable_pair: event.is_stable_pair,
            estimated_liquidity_usd: event.estimated_liquidity_usd,
            created_at: event.created_at,
            signature: event.signature.clone(),
            slot: event.slot,
            processed_at: now,
            updated_at: now,
        })
    }

    /// 将NFT领取事件转换为数据库模型
    fn convert_to_nft_claim_event(&self, event: &NftClaimEventData) -> Result<NftClaimEvent> {
        let now = Utc::now().timestamp();

        Ok(NftClaimEvent {
            id: None,
            nft_mint: event.nft_mint.clone(),
            claimer: event.claimer.clone(),
            referrer: event.referrer.clone(),
            tier: event.tier,
            tier_name: event.tier_name.clone(),
            tier_bonus_rate: event.tier_bonus_rate,
            claim_amount: event.claim_amount,
            token_mint: event.token_mint.clone(),
            reward_multiplier: event.reward_multiplier,
            reward_multiplier_percentage: event.reward_multiplier_percentage,
            bonus_amount: event.bonus_amount,
            claim_type: event.claim_type,
            claim_type_name: event.claim_type_name.clone(),
            total_claimed: event.total_claimed,
            claim_progress_percentage: event.claim_progress_percentage,
            pool_address: event.pool_address.clone(),
            has_referrer: event.has_referrer,
            is_emergency_claim: event.is_emergency_claim,
            estimated_usd_value: event.estimated_usd_value,
            claimed_at: event.claimed_at,
            signature: event.signature.clone(),
            slot: event.slot,
            processed_at: now,
            updated_at: now,
        })
    }

    /// 将奖励分发事件转换为数据库模型
    fn convert_to_reward_distribution_event(
        &self,
        event: &RewardDistributionEventData,
    ) -> Result<RewardDistributionEvent> {
        let now = Utc::now().timestamp();

        Ok(RewardDistributionEvent {
            id: None,
            distribution_id: event.distribution_id,
            reward_pool: event.reward_pool.clone(),
            recipient: event.recipient.clone(),
            referrer: event.referrer.clone(),
            reward_token_mint: event.reward_token_mint.clone(),
            // 新增的代币元数据字段
            reward_token_decimals: event.reward_token_decimals,
            reward_token_name: event.reward_token_name.clone(),
            reward_token_symbol: event.reward_token_symbol.clone(),
            reward_token_logo_uri: event.reward_token_logo_uri.clone(),
            reward_amount: event.reward_amount,
            base_reward_amount: event.base_reward_amount,
            bonus_amount: event.bonus_amount,
            reward_type: event.reward_type,
            reward_type_name: event.reward_type_name.clone(),
            reward_source: event.reward_source,
            reward_source_name: event.reward_source_name.clone(),
            related_address: event.related_address.clone(),
            multiplier: event.multiplier,
            multiplier_percentage: event.multiplier_percentage,
            is_locked: event.is_locked,
            unlock_timestamp: event.unlock_timestamp,
            lock_days: event.lock_days,
            has_referrer: event.has_referrer,
            is_referral_reward: event.is_referral_reward,
            is_high_value_reward: event.is_high_value_reward,
            estimated_usd_value: event.estimated_usd_value,
            distributed_at: event.distributed_at,
            signature: event.signature.clone(),
            slot: event.slot,
            processed_at: now,
            updated_at: now,
        })
    }

    /// 将Launch事件转换为数据库模型
    fn convert_to_launch_event(&self, event: &LaunchEventData) -> Result<LaunchEvent> {
        let now = Utc::now().timestamp();

        // 计算统计分析字段
        let total_liquidity_usd =
            (event.meme_token_amount as f64 * event.initial_price) + (event.base_token_amount as f64);

        let price_range_width_percent = if event.tick_lower_price > 0.0 {
            ((event.tick_upper_price - event.tick_lower_price) / event.tick_lower_price) * 100.0
        } else {
            0.0
        };

        // 判断代币对类型
        let pair_type = if event.base_token_mint == "So11111111111111111111111111111111111111112" {
            "MemeToSol"
        } else if event.base_token_mint == "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v" {
            "MemeToUsdc"
        } else if event.base_token_mint == "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB" {
            "MemeToUsdt"
        } else {
            "MemeToOther"
        };

        // 判断是否为高价值发射（基于流动性阈值）
        let is_high_value_launch = total_liquidity_usd >= 10000.0;

        Ok(LaunchEvent {
            id: None,

            // 核心业务字段
            meme_token_mint: event.meme_token_mint.clone(),
            base_token_mint: event.base_token_mint.clone(),
            user_wallet: event.user_wallet.clone(),

            // 价格和流动性参数
            config_index: event.config_index,
            initial_price: event.initial_price,
            tick_lower_price: event.tick_lower_price,
            tick_upper_price: event.tick_upper_price,

            // 代币数量
            meme_token_amount: event.meme_token_amount,
            base_token_amount: event.base_token_amount,

            // 交易参数
            max_slippage_percent: event.max_slippage_percent,
            with_metadata: event.with_metadata,

            // 时间字段
            open_time: event.open_time,
            launched_at: now,

            // 迁移状态跟踪 - 初始状态为pending
            migration_status: "pending".to_string(),
            migrated_pool_address: None,
            migration_completed_at: None,
            migration_error: None,
            migration_retry_count: 0,

            // 统计分析字段
            total_liquidity_usd,
            pair_type: pair_type.to_string(),
            price_range_width_percent,
            is_high_value_launch,

            // 区块链标准字段
            signature: event.signature.clone(),
            slot: event.slot,
            processed_at: now,
            updated_at: now,
        })
    }

    /// 将存款事件转换为数据库模型
    fn convert_to_deposit_event(&self, event: &DepositEventData) -> Result<database::event_model::DepositEvent> {
        use chrono::Utc;

        let now = Utc::now();

        Ok(database::event_model::DepositEvent {
            id: None,

            // 核心业务字段
            user: event.user.clone(),
            project_config: event.project_config.clone(),
            token_mint: event.token_mint.clone(),
            amount: event.amount,
            total_raised: event.total_raised,

            // 代币元数据字段
            token_decimals: event.token_decimals,
            token_name: event.token_name.clone(),
            token_symbol: event.token_symbol.clone(),
            token_logo_uri: event.token_logo_uri.clone(),

            // 业务扩展字段
            deposit_type: event.deposit_type,
            deposit_type_name: event.deposit_type_name.clone(),
            related_pool: event.related_pool.clone(),
            is_high_value_deposit: event.is_high_value_deposit,
            estimated_usd_value: event.estimated_usd_value,
            actual_amount: event.actual_amount,
            actual_total_raised: event.actual_total_raised,

            // 区块链标准字段
            signature: event.signature.clone(),
            slot: event.slot,
            deposited_at: event.deposited_at,
            processed_at: now.timestamp(),
            updated_at: now.timestamp(),
        })
    }

    /// 将TokenCreationEventData转换为TokenCreationEvent
    fn convert_to_token_creation_event(&self, event: &TokenCreationEventData) -> Result<TokenCreationEvent> {
        let now = Utc::now().timestamp();

        // 处理 extensions 字段，将 Option<serde_json::Value> 转换为 Option<mongodb::bson::Document>
        let extensions_doc = if let Some(ref extensions) = event.extensions {
            // 将 serde_json::Value 转换为 mongodb::bson::Document
            Some(
                mongodb::bson::to_document(extensions)
                    .map_err(|e| EventListenerError::Persistence(format!("转换extensions到BSON失败: {}", e)))?,
            )
        } else {
            None
        };

        Ok(TokenCreationEvent {
            id: None,
            project_config: event.project_config.clone(),
            mint_address: event.mint_address.clone(),
            name: event.name.clone(),
            symbol: event.symbol.clone(),
            metadata_uri: event.metadata_uri.clone(),
            logo_uri: event.logo_uri.clone(),
            decimals: event.decimals,
            supply: event.supply,
            creator: event.creator.clone(),
            has_whitelist: event.has_whitelist,
            whitelist_deadline: event.whitelist_deadline,
            extensions: extensions_doc,
            source: event.source.as_ref().map(|s| format!("{:?}", s)),
            signature: event.signature.clone(),
            slot: event.slot,
            created_at: event.created_at,
            processed_at: now,
            updated_at: now,
        })
    }

    fn convert_to_push_token_request(&self, event: &TokenCreationEventData) -> Result<TokenPushRequest> {
        // 确定标签
        let mut tags = vec!["event-listener".to_string(), "onchain".to_string()];

        if event.has_whitelist {
            tags.push("whitelist".to_string());
        }

        // 根据供应量添加标签
        if event.supply > 1_000_000_000_000_000_000 {
            // 大于10^18
            tags.push("large-supply".to_string());
        } else if event.supply < 1_000_000_000 {
            // 小于10^9
            tags.push("small-supply".to_string());
        }

        // 创建TokenPushRequest
        Ok(TokenPushRequest {
            address: event.mint_address.to_string(),
            program_id: Some("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA".to_string()),
            name: event.name.clone(),
            symbol: event.symbol.clone(),
            decimals: event.decimals,
            logo_uri: event.logo_uri.clone(),
            tags: Some(tags),
            daily_volume: Some(0.0), // 新创建的代币没有交易量
            freeze_authority: None,  // 从事件中无法获取，设为None
            mint_authority: Some(event.creator.to_string()),
            permanent_delegate: None, // 从事件中无法获取，设为None
            minted_at: Some(
                chrono::DateTime::from_timestamp(event.created_at, 0).unwrap_or_else(|| chrono::Utc::now()),
            ),
            extensions: event.extensions.clone(),
            source: event.source.clone(),
        })
    }

    /// 判断是否为致命错误
    fn is_fatal_error(&self, error: &EventListenerError) -> bool {
        match error {
            EventListenerError::Database(_) => true,     // 数据库连接错误是致命的
            EventListenerError::Config(_) => true,       // 配置错误是致命的
            EventListenerError::Persistence(_) => false, // 持久化错误通常可以跳过
            _ => false,
        }
    }

    /// 检查是否为MongoDB重复键错误
    fn is_duplicate_key_error(&self, error: &EventListenerError) -> bool {
        if let EventListenerError::Persistence(msg) = error {
            // 检查错误消息中是否包含MongoDB重复键错误标识
            msg.contains("E11000") && msg.contains("duplicate key error")
        } else {
            false
        }
    }

    /// 写入单个存款事件
    async fn write_single_deposit(&self, event: &DepositEventData) -> Result<bool> {
        // 检查是否已存在（根据交易签名去重）
        let existing = self
            .database
            .deposit_event_repository
            .find_by_signature(&event.signature)
            .await
            .map_err(|e| EventListenerError::Persistence(format!("查询现有存款事件失败: {}", e)))?;

        if existing.is_some() {
            debug!("存款事件已存在，跳过: {}", event.signature);
            return Ok(false);
        }

        // 转换为数据库模型
        let deposit_event = self.convert_to_deposit_event(event)?;

        // 插入数据库
        self.database
            .deposit_event_repository
            .insert_deposit_event(deposit_event)
            .await
            .map_err(|e| EventListenerError::Persistence(format!("插入存款事件失败: {}", e)))?;

        info!(
            "✅ 存款事件已写入: {} 存款 {} 到项目 {}",
            event.user, event.amount, event.token_mint
        );

        // 异步触发 TokenInfo.extensions 维护
        // 通过解析器上的方法更新，这里复用数据库引用自行实现
        let db = Arc::clone(&self.database);
        let cfg = Arc::clone(&self.config);
        let event_clone = event.clone();
        tokio::spawn(async move {
            info!("异步触发 TokenInfo.extensions 维护: {}", event_clone.token_mint);
            // 查找 TokenInfo 并最小化更新 extensions
            if let Ok(Some(existing)) = db.token_info_repository.find_by_address(&event_clone.token_mint).await {
                // 现有 extensions
                let mut extensions = if existing.extensions.is_null() {
                    info!("TokenInfo.extensions 为空，创建空对象");
                    serde_json::json!({})
                } else {
                    info!("TokenInfo.extensions 不为空:{}，复制现有对象", existing.extensions);
                    existing.extensions.clone()
                };

                // 合并最小必需字段
                extensions = EventStorage::merge_extensions_for_deposit(
                    extensions,
                    &event_clone.project_config,
                    event_clone.project_state,
                    event_clone.total_raised,
                );

                // 若 extensions 为空或缺关键字段，则尝试补齐：creator、description、log_url、social_links、whitelist、crowdfunding
                let need_enrich = extensions.get("description").is_none()
                    || extensions.get("log_url").is_none()
                    || extensions.get("social_links").is_none()
                    || extensions.get("whitelist").is_none()
                    || extensions.get("crowdfunding").is_none()
                    || extensions.get("creator").is_none();

                if need_enrich {
                    info!("TokenInfo.extensions 需要补齐:{}，从链上获取", event_clone.token_mint);
                    // 读取链上 ProjectConfig 的 project_wallet 与 meta_uri
                    let rpc_url = cfg.solana.rpc_url.clone();
                    if !rpc_url.is_empty() {
                        if let Ok(proj_key) = Pubkey::from_str(&event_clone.project_config) {
                            let rpc = RpcClient::new(rpc_url);
                            if let Ok(data) = rpc.get_account_data(&proj_key) {
                                // 解析：authority(32) + project_wallet(32) + meta_uri(String)
                                if data.len() >= 64 + 4 {
                                    let mut offset = 0usize;
                                    offset += 32; // skip authority
                                    let wallet_bytes = &data[offset..offset + 32];
                                    if let Ok(wallet) = <[u8; 32]>::try_from(wallet_bytes) {
                                        let wallet_pk = Pubkey::new_from_array(wallet);
                                        extensions["creator"] = serde_json::Value::String(wallet_pk.to_string());
                                    }
                                    offset += 32;
                                    if offset + 4 <= data.len() {
                                        let len_bytes: [u8; 4] = match data[offset..offset + 4].try_into() {
                                            Ok(b) => b,
                                            Err(_) => [0u8; 4],
                                        };
                                        let str_len = u32::from_le_bytes(len_bytes) as usize;
                                        offset += 4;
                                        if offset + str_len <= data.len() {
                                            let uri_bytes = &data[offset..offset + str_len];
                                            if let Ok(uri) = String::from_utf8(uri_bytes.to_vec()) {
                                                // 拉取 URI 元数据
                                                if uri.starts_with("http") {
                                                    info!("拉取 URI 元数据: {}", uri);
                                                    if let Ok(svc) =
                                                        MetaplexService::new(Some(MetaplexConfig::default()))
                                                    {
                                                        if let Ok(meta_opt) = svc.fetch_metadata_from_uri(&uri).await {
                                                            if let Some(meta) = meta_opt {
                                                                info!("拉取 URI 元数据成功: {:#?}", meta);
                                                                if extensions.get("description").is_none() {
                                                                    if let Some(desc) = meta.description {
                                                                        extensions["description"] =
                                                                            serde_json::Value::String(desc);
                                                                    }
                                                                }
                                                                if extensions.get("log_url").is_none() {
                                                                    if let Some(url) = meta.avatar_url {
                                                                        extensions["log_url"] =
                                                                            serde_json::Value::String(url);
                                                                    }
                                                                }
                                                                if extensions.get("social_links").is_none() {
                                                                    if let Ok(v) =
                                                                        serde_json::to_value(meta.social_links)
                                                                    {
                                                                        if !v.is_null() {
                                                                            extensions["social_links"] = v;
                                                                        }
                                                                    }
                                                                }
                                                                if extensions.get("whitelist").is_none() {
                                                                    if let Ok(v) = serde_json::to_value(meta.whitelist)
                                                                    {
                                                                        if !v.is_null() {
                                                                            extensions["whitelist"] = v;
                                                                        }
                                                                    }
                                                                }
                                                                if extensions.get("crowdfunding").is_none() {
                                                                    if let Ok(v) =
                                                                        serde_json::to_value(meta.crowdfunding)
                                                                    {
                                                                        if !v.is_null() {
                                                                            extensions["crowdfunding"] = v;
                                                                        }
                                                                    }
                                                                }
                                                                info!(
                                                                    "✅ TokenInfo.extensions 补齐完成: {:#?}",
                                                                    extensions
                                                                );
                                                            } else {
                                                                warn!("⚠️ 拉取 URI 元数据失败: metadata is None");
                                                            }
                                                        } else {
                                                            warn!(
                                                                "⚠️ 拉取 URI 元数据失败: call fetch_metadata_from_uri failed"
                                                            );
                                                        }
                                                    } else {
                                                        warn!("⚠️ 拉取 URI 元数据失败: MetaplexService initialization failed");
                                                    }
                                                } else {
                                                    warn!("⚠️ 拉取 URI 元数据失败: uri is not valid");
                                                }
                                            } else {
                                                warn!("⚠️ 拉取 URI 元数据失败: String::from_utf8 解析urifailed");
                                            }
                                        }
                                    } else {
                                        warn!("⚠️ 拉取 URI 元数据失败: offset + str_len > data.len()");
                                    }
                                } else {
                                    warn!("⚠️ 拉取 URI 元数据失败: data.len() < 64 + 4");
                                }
                            } else {
                                warn!("⚠️ 拉取 URI 元数据失败: get_account_data failed");
                            }
                        } else {
                            warn!("⚠️ 拉取 URI 元数据失败: project_config is invalid");
                        }
                    } else {
                        warn!("⚠️ 拉取 URI 元数据失败: rpc_url is empty");
                    }
                }

                // 写回
                if let Ok(bson) = mongodb::bson::to_bson(&extensions) {
                    let _ = db
                        .token_info_repository
                        .update_token(&event_clone.token_mint, mongodb::bson::doc! {"extensions": bson})
                        .await;
                    info!("TokenInfo.extensions 写回成功!");
                }
            } else {
                info!(
                    "异步触发 TokenInfo.extensions 维护完成: {}，但TokenInfo不存在",
                    event_clone.token_mint
                );
            }
        });

        Ok(true)
    }

    /// 写入单个事件（非批量）
    pub async fn write_event(&self, event: &ParsedEvent) -> Result<bool> {
        match event {
            ParsedEvent::TokenCreation(token_event) => self.write_single_token_creation(token_event).await,
            ParsedEvent::PoolCreation(pool_event) => self.write_single_pool_creation(pool_event).await,
            ParsedEvent::NftClaim(nft_event) => self.write_single_nft_claim(nft_event).await,
            ParsedEvent::RewardDistribution(reward_event) => self.write_single_reward_distribution(reward_event).await,
            ParsedEvent::Launch(launch_event) => self.write_single_launch_event(launch_event).await,
            ParsedEvent::Swap(swap_event) => self.write_single_swap(swap_event).await,
            ParsedEvent::Deposit(deposit_event) => self.write_single_deposit(deposit_event).await,
        }
    }

    /// 智能更新池子（防止覆盖）
    async fn smart_update_pool_from_event(&self, pool: &mut ClmmPool, event: &PoolCreatedEventData) -> Result<bool> {
        // 版本控制：检查slot防止旧事件覆盖新数据
        if let Some(api_slot) = pool.api_created_slot {
            if event.slot < api_slot {
                warn!(
                    "⚠️ 忽略旧事件: event_slot({}) < api_slot({}), pool: {}",
                    event.slot, api_slot, pool.pool_address
                );
                return Ok(false);
            }
        }

        // 如果已经被链上确认，检查是否需要更新
        if pool.chain_confirmed {
            if let Some(event_slot) = pool.event_updated_slot {
                if event.slot <= event_slot {
                    debug!(
                        "ℹ️ 池子已有更新的链上数据，跳过: {} (existing_slot: {}, new_slot: {})",
                        pool.pool_address, event_slot, event.slot
                    );
                    return Ok(false);
                }
            }
        }

        let now = chrono::Utc::now().timestamp() as u64;

        // 根据操作模式决定更新策略
        let update_strategy = match self.app_config.get_event_listener_db_mode() {
            EventListenerDbMode::UpdateOnly => {
                // 仅更新模式：只更新必要字段
                doc! {
                    "$set": {
                        // 更新链上事件信息
                        "event_signature": &event.signature,
                        "event_updated_slot": event.slot as i64,
                        "event_confirmed_at": event.created_at,
                        "event_updated_at": now as i64,

                        // 更新状态
                        "status": "Active",
                        "chain_confirmed": true,
                        "data_source": if pool.data_source == DataSource::ApiCreated {
                            "api_chain_confirmed"
                        } else {
                            "chain"
                        },

                        // 更新价格信息（链上数据更准确）
                        "price_info.current_price": event.initial_price,
                        "price_info.current_tick": event.initial_tick,

                        // 更新时间戳
                        "updated_at": now as i64,
                    },

                    // 仅在字段不存在时设置（保护已有数据）
                    "$setOnInsert": {
                        "mint0.decimals": event.token_a_decimals as i32,
                        "mint1.decimals": event.token_b_decimals as i32,
                    }
                }
            }
            EventListenerDbMode::Upsert => {
                // Upsert模式：可以覆盖更多字段
                doc! {
                    "$set": {
                        // 更新链上事件信息
                        "event_signature": &event.signature,
                        "event_updated_slot": event.slot as i64,
                        "event_confirmed_at": event.created_at,
                        "event_updated_at": now as i64,

                        // 更新状态
                        "status": "Active",
                        "chain_confirmed": true,
                        "data_source": "api_chain_confirmed",

                        // 更新价格信息
                        "price_info.current_price": event.initial_price,
                        "price_info.current_tick": event.initial_tick,
                        "price_info.sqrt_price_x64": &event.sqrt_price_x64,

                        // 更新代币信息
                        "mint0.decimals": event.token_a_decimals as i32,
                        "mint1.decimals": event.token_b_decimals as i32,

                        // 更新费率信息
                        "fee_rate": event.fee_rate,

                        // 更新时间戳
                        "updated_at": now as i64,
                    }
                }
            }
        };

        // 执行更新
        let updated = self
            .clmm_pool_repository
            .update_pool_with_version_check(&pool.pool_address, update_strategy, Some(event.slot))
            .await
            .map_err(|e| EventListenerError::Persistence(format!("更新池子失败: {}", e)))?;

        if updated {
            info!(
                "✅ 池子已通过链上事件更新: {} (slot: {}, mode: {:?})",
                pool.pool_address,
                event.slot,
                self.app_config.get_event_listener_db_mode()
            );
        } else {
            warn!("⚠️ 池子更新被版本控制拒绝: {} (可能是并发更新)", pool.pool_address);
        }

        // 同时保存到ClmmPoolEvent作为审计日志
        self.save_pool_event_as_audit_log(event).await?;

        Ok(updated)
    }

    /// 从链上事件创建新池子记录（仅在允许时调用）
    async fn create_pool_from_chain_event(&self, event: &PoolCreatedEventData) -> Result<bool> {
        // 再次检查开关（双重保险）
        if !self.app_config.enable_pool_event_insert {
            warn!("❌ 尝试从事件创建池子但开关已关闭: {}", event.pool_address);
            return Ok(false);
        }

        info!("🆕 从链上事件创建新池子: {}", event.pool_address);

        let now = chrono::Utc::now().timestamp() as u64;

        // 构建完整的池子记录
        let pool = ClmmPool {
            id: None,
            pool_address: event.pool_address.clone(),
            amm_config_address: event.clmm_config.clone(),
            config_index: 0, // 默认值，需要后续同步

            mint0: TokenInfo {
                mint_address: event.token_a_mint.clone(),
                decimals: event.token_a_decimals,
                owner: String::new(), // 需要后续同步
                symbol: None,
                name: None,
                log_uri: None,
                description: None,
                external_url: None,
                tags: None,
                attributes: None,
            },

            mint1: TokenInfo {
                mint_address: event.token_b_mint.clone(),
                decimals: event.token_b_decimals,
                owner: String::new(),
                symbol: None,
                name: None,
                log_uri: None,
                description: None,
                external_url: None,
                tags: None,
                attributes: None,
            },

            price_info: PriceInfo {
                initial_price: event.initial_price,
                sqrt_price_x64: event.sqrt_price_x64.clone(),
                initial_tick: event.initial_tick,
                current_price: Some(event.initial_price),
                current_tick: Some(event.initial_tick),
            },

            vault_info: VaultInfo {
                token_vault_0: String::new(), // 需要后续同步
                token_vault_1: String::new(),
            },

            extension_info: ExtensionInfo {
                observation_address: String::new(),
                tickarray_bitmap_extension: String::new(),
            },

            creator_wallet: event.creator.clone(),
            open_time: event.created_at as u64,

            // 时间戳字段
            api_created_at: event.created_at as u64, // 使用事件时间
            api_created_slot: None,                  // 纯链上创建，无API slot
            updated_at: now,

            // 链上事件信息
            event_signature: Some(event.signature.clone()),
            event_updated_slot: Some(event.slot),
            event_confirmed_at: Some(event.created_at as u64),
            event_updated_at: Some(now),

            // 状态管理
            status: PoolStatus::Active,
            data_source: DataSource::ChainEvent,
            chain_confirmed: true,

            transaction_info: Some(TransactionInfo {
                signature: event.signature.clone(),
                status: TransactionStatus::Confirmed,
                explorer_url: format!("https://explorer.solana.com/tx/{}", event.signature),
                confirmed_at: event.created_at as u64,
            }),

            sync_status: SyncStatus {
                last_sync_at: now,
                sync_version: 1,
                needs_sync: true, // 标记需要同步完整信息
                sync_error: None,
            },

            pool_type: database::clmm_pool::PoolType::Concentrated,
        };

        // 插入新记录
        self.clmm_pool_repository
            .insert_pool(pool)
            .await
            .map_err(|e| EventListenerError::Persistence(format!("插入池子失败: {}", e)))?;

        info!("✅ 成功从链上事件创建池子记录: {}", event.pool_address);

        // 保存审计日志
        self.save_pool_event_as_audit_log(event).await?;

        Ok(true)
    }

    /// 保存池子事件作为审计日志
    async fn save_pool_event_as_audit_log(&self, event: &PoolCreatedEventData) -> Result<()> {
        // 转换为ClmmPoolEvent用于审计
        let pool_event = self.convert_to_pool_event(event)?;

        // 插入到事件表（作为审计日志）
        self.database
            .clmm_pool_event_repository
            .insert_pool_event(pool_event)
            .await
            .map_err(|e| EventListenerError::Persistence(format!("保存审计日志失败: {}", e)))?;

        debug!("📝 池子事件已保存为审计日志: {}", event.pool_address);
        Ok(())
    }

    /// 获取存储统计信息
    pub async fn get_storage_stats(&self) -> Result<StorageStats> {
        // 获取代币统计
        let token_stats = self
            .token_repository
            .get_token_stats()
            .await
            .map_err(|e| EventListenerError::Persistence(format!("获取代币统计失败: {}", e)))?;

        Ok(StorageStats {
            total_tokens: token_stats.total_tokens,
            active_tokens: token_stats.active_tokens,
            verified_tokens: token_stats.verified_tokens,
            today_new_tokens: token_stats.today_new_tokens,
        })
    }

    /// 检查存储健康状态
    pub async fn health_check(&self) -> Result<bool> {
        // 尝试执行一个简单的数据库操作来检查连接
        match self.token_repository.get_token_stats().await {
            Ok(_) => Ok(true),
            Err(e) => {
                warn!("存储健康检查失败: {}", e);
                Ok(false)
            }
        }
    }

    /// 获取配置信息（用于调试和健康检查）
    pub fn get_config(&self) -> Arc<EventListenerConfig> {
        Arc::clone(&self.config)
    }

    /// 获取数据库引用（用于高级操作）
    pub fn get_database(&self) -> &Arc<Database> {
        &self.database
    }

    /// 获取代币仓库引用
    pub fn get_token_repository(&self) -> &Arc<TokenInfoRepository> {
        &self.token_repository
    }
}

/// 存储统计信息
#[derive(Debug, Clone, serde::Serialize)]
pub struct StorageStats {
    pub total_tokens: u64,
    pub active_tokens: u64,
    pub verified_tokens: u64,
    pub today_new_tokens: u64,
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
                program_ids: vec![solana_sdk::pubkey::Pubkey::new_unique()],
                private_key: None,
            },
            database: crate::config::settings::DatabaseConfig {
                uri: "mongodb://localhost:27017".to_string(),
                database_name: "test_event_listener".to_string(),
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

    fn create_test_token_event() -> TokenCreationEventData {
        TokenCreationEventData {
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
        }
    }

    #[test]
    fn test_convert_to_push_request() {
        let config = create_test_config();
        let storage = tokio_test::block_on(async { EventStorage::new(&config).await });

        // 如果连接失败（比如没有MongoDB），跳过这个测试
        if storage.is_err() {
            return;
        }

        let storage = storage.unwrap();
        let event = create_test_token_event();

        let push_request = storage.convert_to_push_token_request(&event).unwrap();

        assert_eq!(push_request.address, event.mint_address.to_string());
        assert_eq!(push_request.name, event.name);
        assert_eq!(push_request.symbol, event.symbol);
        assert_eq!(push_request.decimals, event.decimals);
        assert_eq!(push_request.logo_uri, event.logo_uri);
        assert!(push_request
            .tags
            .as_ref()
            .unwrap()
            .contains(&"event-listener".to_string()));
    }

    #[test]
    fn test_is_fatal_error() {
        let config = create_test_config();
        let storage = tokio_test::block_on(async { EventStorage::new(&config).await });

        if storage.is_err() {
            return;
        }

        let storage = storage.unwrap();

        // 数据库错误应该是致命的
        let db_error = EventListenerError::Database(mongodb::error::Error::from(std::io::Error::new(
            std::io::ErrorKind::Other,
            "test",
        )));
        assert!(storage.is_fatal_error(&db_error));

        // 持久化错误不应该是致命的
        let persist_error = EventListenerError::Persistence("test error".to_string());
        assert!(!storage.is_fatal_error(&persist_error));
    }

    #[test]
    fn test_is_duplicate_key_error() {
        let config = create_test_config();
        let storage = tokio_test::block_on(async { EventStorage::new(&config).await });

        if storage.is_err() {
            return;
        }

        let storage = storage.unwrap();

        // 测试重复键错误检测
        let duplicate_error = EventListenerError::Persistence(
            "保存审计日志失败: Kind: An error occurred when trying to execute a write operation: WriteError(WriteError { code: 11000, code_name: None, message: \"E11000 duplicate key error collection: coinfair_development.ClmmPoolEvent index: pool_address_1_signature_1 dup key: { pool_address: \\\"test\\\", signature: \\\"test\\\" }\", details: None })".to_string()
        );
        assert!(storage.is_duplicate_key_error(&duplicate_error));

        // 测试非重复键错误
        let normal_error = EventListenerError::Persistence("normal error".to_string());
        assert!(!storage.is_duplicate_key_error(&normal_error));

        // 测试其他类型错误
        let db_error = EventListenerError::Database(mongodb::error::Error::from(std::io::Error::new(
            std::io::ErrorKind::Other,
            "database error",
        )));
        assert!(!storage.is_duplicate_key_error(&db_error));
    }

    #[tokio::test]
    async fn test_write_batch_empty() {
        let config = create_test_config();

        // 如果无法连接数据库，跳过测试
        if let Ok(storage) = EventStorage::new(&config).await {
            let result = storage.write_batch(&[]).await.unwrap();
            assert_eq!(result, 0);
        }
    }

    #[tokio::test]
    async fn test_write_batch_with_new_event_types() {
        let config = create_test_config();

        // 如果无法连接数据库，跳过测试
        if let Ok(storage) = EventStorage::new(&config).await {
            // 创建各种类型的事件
            let token_event = ParsedEvent::TokenCreation(create_test_token_event());
            let pool_event = ParsedEvent::PoolCreation(create_test_pool_event());
            let nft_event = ParsedEvent::NftClaim(create_test_nft_event());
            let reward_event = ParsedEvent::RewardDistribution(create_test_reward_event());

            let events = vec![token_event, pool_event, nft_event, reward_event];

            // 这个测试可能会失败，因为需要实际的数据库连接
            // 但它验证了接口的正确性
            let result = storage.write_batch(&events).await;
            match result {
                Ok(written_count) => {
                    // 如果成功，应该写入了一些事件
                    println!("成功写入 {} 个事件", written_count);
                }
                Err(e) => {
                    // 如果失败，可能是数据库连接问题
                    println!("写入失败，可能是数据库连接问题: {}", e);
                }
            }
        }
    }

    #[tokio::test]
    async fn test_write_single_events() {
        let config = create_test_config();

        if let Ok(storage) = EventStorage::new(&config).await {
            // 测试写入单个池子创建事件
            let pool_event = ParsedEvent::PoolCreation(create_test_pool_event());
            let _result = storage.write_event(&pool_event).await;

            // 测试写入单个NFT领取事件
            let nft_event = ParsedEvent::NftClaim(create_test_nft_event());
            let _result = storage.write_event(&nft_event).await;

            // 测试写入单个奖励分发事件
            let reward_event = ParsedEvent::RewardDistribution(create_test_reward_event());
            let _result = storage.write_event(&reward_event).await;
        }
    }

    fn create_test_pool_event() -> crate::parser::event_parser::PoolCreatedEventData {
        use crate::parser::event_parser::PoolCreatedEventData;
        PoolCreatedEventData {
            pool_address: Pubkey::new_unique().to_string(),
            token_a_mint: Pubkey::new_unique().to_string(),
            token_b_mint: Pubkey::new_unique().to_string(),
            token_a_decimals: 9,
            token_b_decimals: 6,
            fee_rate: 3000,
            fee_rate_percentage: 0.3,
            annual_fee_rate: 109.5,
            pool_type: "标准费率".to_string(),
            sqrt_price_x64: (1u128 << 64).to_string(),
            initial_price: 1.0,
            initial_tick: 0,
            creator: Pubkey::new_unique().to_string(),
            clmm_config: Pubkey::new_unique().to_string(),
            is_stable_pair: false,
            estimated_liquidity_usd: 0.0,
            created_at: chrono::Utc::now().timestamp(),
            signature: "test_pool_sig".to_string(),
            slot: 12345,
            processed_at: chrono::Utc::now().to_rfc3339(),
        }
    }

    #[test]
    fn test_merge_extensions_for_deposit_minimal() {
        let existing = serde_json::json!({});
        let merged = EventStorage::merge_extensions_for_deposit(existing, "proj_cfg", 3, 12345);
        assert_eq!(
            merged["project_config"],
            serde_json::Value::String("proj_cfg".to_string())
        );
        assert_eq!(
            merged["project_state"],
            serde_json::Value::Number(serde_json::Number::from(3u8))
        );
        assert_eq!(
            merged["total_raised"],
            serde_json::Value::Number(serde_json::Number::from(12345u64))
        );
    }

    #[test]
    fn test_merge_extensions_for_deposit_preserve_other_fields() {
        let existing = serde_json::json!({
            "description": "keep me",
            "social_links": {"twitter": "x"}
        });
        let merged = EventStorage::merge_extensions_for_deposit(existing.clone(), "cfg", 4, 1);
        assert_eq!(merged["description"], existing["description"]);
        assert_eq!(merged["social_links"], existing["social_links"]);
        assert_eq!(merged["project_config"], serde_json::Value::String("cfg".to_string()));
        assert_eq!(
            merged["project_state"],
            serde_json::Value::Number(serde_json::Number::from(4u8))
        );
        assert_eq!(
            merged["total_raised"],
            serde_json::Value::Number(serde_json::Number::from(1u64))
        );
    }

    fn create_test_nft_event() -> crate::parser::event_parser::NftClaimEventData {
        use crate::parser::event_parser::NftClaimEventData;
        NftClaimEventData {
            nft_mint: Pubkey::new_unique().to_string(),
            claimer: Pubkey::new_unique().to_string(),
            referrer: Some(Pubkey::new_unique().to_string()),
            tier: 3,
            tier_name: "Gold".to_string(),
            tier_bonus_rate: 1.5,
            claim_amount: 1000000,
            token_mint: Pubkey::new_unique().to_string(),
            reward_multiplier: 15000,
            reward_multiplier_percentage: 1.5,
            bonus_amount: 1500000,
            claim_type: 0,
            claim_type_name: "定期领取".to_string(),
            total_claimed: 5000000,
            claim_progress_percentage: 20.0,
            pool_address: Some(Pubkey::new_unique().to_string()),
            has_referrer: true,
            is_emergency_claim: false,
            estimated_usd_value: 0.0,
            claimed_at: chrono::Utc::now().timestamp(),
            signature: "test_nft_sig".to_string(),
            slot: 23456,
            processed_at: chrono::Utc::now().to_rfc3339(),
        }
    }

    fn create_test_reward_event() -> crate::parser::event_parser::RewardDistributionEventData {
        use crate::parser::event_parser::RewardDistributionEventData;
        RewardDistributionEventData {
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
            signature: "test_reward_sig".to_string(),
            slot: 34567,
            processed_at: chrono::Utc::now().to_rfc3339(),
        }
    }

    #[test]
    fn test_tag_generation() {
        let config = create_test_config();
        let storage = tokio_test::block_on(async { EventStorage::new(&config).await });

        if storage.is_err() {
            return;
        }

        let storage = storage.unwrap();

        // 测试大供应量标签
        let large_supply_event = TokenCreationEventData {
            supply: 2_000_000_000_000_000_000,
            has_whitelist: false,
            ..create_test_token_event()
        };

        let push_request = storage.convert_to_push_token_request(&large_supply_event).unwrap();
        let tags = push_request.tags.unwrap();
        assert!(tags.contains(&"large-supply".to_string()));

        // 测试小供应量标签
        let small_supply_event = TokenCreationEventData {
            supply: 500_000_000,
            has_whitelist: false,
            ..create_test_token_event()
        };

        let push_request = storage.convert_to_push_token_request(&small_supply_event).unwrap();
        let tags = push_request.tags.unwrap();
        assert!(tags.contains(&"small-supply".to_string()));

        // 测试白名单标签
        let whitelist_event = TokenCreationEventData {
            has_whitelist: true,
            ..create_test_token_event()
        };

        let push_request = storage.convert_to_push_token_request(&whitelist_event).unwrap();
        let tags = push_request.tags.unwrap();
        assert!(tags.contains(&"whitelist".to_string()));
    }
}
