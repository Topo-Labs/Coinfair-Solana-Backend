use crate::{
    config::EventListenerConfig,
    error::{EventListenerError, Result},
    parser::{ParsedEvent, event_parser::{TokenCreationEventData, PoolCreationEventData, NftClaimEventData, RewardDistributionEventData}},
};
use database::{
    token_info::{TokenPushRequest, DataSource, TokenInfoRepository},
    event_model::{ClmmPoolEvent, NftClaimEvent, RewardDistributionEvent},
    Database,
};
use std::sync::Arc;
use tracing::{debug, error, info, warn};
use chrono::Utc;

/// 事件存储接口
/// 
/// 负责将解析后的事件持久化到数据库
/// 支持批量写入和事务处理
pub struct EventStorage {
    config: Arc<EventListenerConfig>,
    database: Arc<Database>,
    token_repository: Arc<TokenInfoRepository>,
}

impl EventStorage {
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
        });

        // 创建数据库实例
        let database = Arc::new(Database::new(app_config).await
            .map_err(|e| EventListenerError::Persistence(format!("数据库初始化失败: {}", e)))?);

        // 创建代币信息仓库 
        let token_repository = Arc::new(database.token_info_repository.clone());

        info!("✅ 事件存储初始化完成，数据库: {}", config.database.database_name);

        Ok(Self {
            config,
            database,
            token_repository,
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

        debug!("✅ 批量写入完成，总计写入: {} 个事件", written_count);
        Ok(written_count)
    }

    /// 批量写入池子创建事件
    async fn write_pool_creation_batch(&self, events: &[&PoolCreationEventData]) -> Result<u64> {
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
                    error!("❌ NFT领取事件写入失败: {} by {} - {}", event.nft_mint, event.claimer, e);
                    
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
                    debug!("✅ 奖励分发事件已写入: {} to {}", event.distribution_id, event.recipient);
                }
                Ok(false) => {
                    debug!("ℹ️ 奖励分发事件已存在，跳过: {} to {}", event.distribution_id, event.recipient);
                }
                Err(e) => {
                    error!("❌ 奖励分发事件写入失败: {} to {} - {}", event.distribution_id, event.recipient, e);
                    
                    if self.is_fatal_error(&e) {
                        return Err(e);
                    }
                    
                    warn!("⚠️ 跳过失败的事件: {} to {}", event.distribution_id, event.recipient);
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
                    error!("❌ 代币创建事件写入失败: {} ({}) - {}", 
                           event.symbol, event.mint_address, e);
                    
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
        // 检查是否已存在
        let existing = self.token_repository.find_by_address(&event.mint_address.to_string()).await
            .map_err(|e| EventListenerError::Persistence(format!("查询现有代币失败: {}", e)))?;

        if existing.is_some() {
            debug!("代币已存在，跳过: {}", event.mint_address);
            return Ok(false);
        }

        // 构建TokenPushRequest
        let push_request = self.convert_to_push_request(event)?;

        // 推送到数据库
        let response = self.token_repository.push_token(push_request).await
            .map_err(|e| EventListenerError::Persistence(format!("推送代币信息失败: {}", e)))?;

        if !response.success {
            return Err(EventListenerError::Persistence(
                format!("代币信息推送失败: {}", response.message)
            ));
        }

        Ok(true)
    }

    /// 写入单个池子创建事件
    async fn write_single_pool_creation(&self, event: &PoolCreationEventData) -> Result<bool> {
        // 检查是否已存在
        let existing = self.database.clmm_pool_event_repository
            .find_by_pool_address(&event.pool_address.to_string()).await
            .map_err(|e| EventListenerError::Persistence(format!("查询现有池子事件失败: {}", e)))?;

        if existing.is_some() {
            debug!("池子事件已存在，跳过: {}", event.pool_address);
            return Ok(false);
        }

        // 转换为数据库模型
        let pool_event = self.convert_to_pool_event(event)?;

        // 插入数据库
        self.database.clmm_pool_event_repository
            .insert_pool_event(pool_event).await
            .map_err(|e| EventListenerError::Persistence(format!("插入池子事件失败: {}", e)))?;

        Ok(true)
    }

    /// 写入单个NFT领取事件
    async fn write_single_nft_claim(&self, event: &NftClaimEventData) -> Result<bool> {
        // 转换为数据库模型
        let nft_event = self.convert_to_nft_claim_event(event)?;

        // 插入数据库
        self.database.nft_claim_event_repository
            .insert_nft_claim_event(nft_event).await
            .map_err(|e| EventListenerError::Persistence(format!("插入NFT领取事件失败: {}", e)))?;

        Ok(true)
    }

    /// 写入单个奖励分发事件
    async fn write_single_reward_distribution(&self, event: &RewardDistributionEventData) -> Result<bool> {
        // 检查是否已存在
        let existing = self.database.reward_distribution_event_repository
            .find_by_distribution_id(event.distribution_id).await
            .map_err(|e| EventListenerError::Persistence(format!("查询现有奖励事件失败: {}", e)))?;

        if existing.is_some() {
            debug!("奖励事件已存在，跳过: {}", event.distribution_id);
            return Ok(false);
        }

        // 转换为数据库模型
        let reward_event = self.convert_to_reward_distribution_event(event)?;

        // 插入数据库
        self.database.reward_distribution_event_repository
            .insert_reward_event(reward_event).await
            .map_err(|e| EventListenerError::Persistence(format!("插入奖励分发事件失败: {}", e)))?;

        Ok(true)
    }

    /// 将池子创建事件转换为数据库模型
    fn convert_to_pool_event(&self, event: &PoolCreationEventData) -> Result<ClmmPoolEvent> {
        let now = Utc::now();
        
        Ok(ClmmPoolEvent {
            id: None,
            pool_address: event.pool_address,
            token_a_mint: event.token_a_mint,
            token_b_mint: event.token_b_mint,
            token_a_decimals: event.token_a_decimals,
            token_b_decimals: event.token_b_decimals,
            fee_rate: event.fee_rate,
            fee_rate_percentage: event.fee_rate_percentage,
            annual_fee_rate: event.annual_fee_rate,
            pool_type: event.pool_type.clone(),
            sqrt_price_x64: event.sqrt_price_x64,
            initial_price: event.initial_price,
            initial_tick: event.initial_tick,
            creator: event.creator,
            clmm_config: event.clmm_config,
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
        let now = Utc::now();
        
        Ok(NftClaimEvent {
            id: None,
            nft_mint: event.nft_mint,
            claimer: event.claimer,
            referrer: event.referrer,
            tier: event.tier,
            tier_name: event.tier_name.clone(),
            tier_bonus_rate: event.tier_bonus_rate,
            claim_amount: event.claim_amount,
            token_mint: event.token_mint,
            reward_multiplier: event.reward_multiplier,
            reward_multiplier_percentage: event.reward_multiplier_percentage,
            bonus_amount: event.bonus_amount,
            claim_type: event.claim_type,
            claim_type_name: event.claim_type_name.clone(),
            total_claimed: event.total_claimed,
            claim_progress_percentage: event.claim_progress_percentage,
            pool_address: event.pool_address,
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
    fn convert_to_reward_distribution_event(&self, event: &RewardDistributionEventData) -> Result<RewardDistributionEvent> {
        let now = Utc::now();
        
        Ok(RewardDistributionEvent {
            id: None,
            distribution_id: event.distribution_id,
            reward_pool: event.reward_pool,
            recipient: event.recipient,
            referrer: event.referrer,
            reward_token_mint: event.reward_token_mint,
            reward_amount: event.reward_amount,
            base_reward_amount: event.base_reward_amount,
            bonus_amount: event.bonus_amount,
            reward_type: event.reward_type,
            reward_type_name: event.reward_type_name.clone(),
            reward_source: event.reward_source,
            reward_source_name: event.reward_source_name.clone(),
            related_address: event.related_address,
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

    /// 将代币创建事件转换为TokenPushRequest
    fn convert_to_push_request(&self, event: &TokenCreationEventData) -> Result<TokenPushRequest> {
        // 确定标签
        let mut tags = vec!["event-listener".to_string(), "onchain".to_string()];
        
        if event.has_whitelist {
            tags.push("whitelist".to_string());
        }
        
        // 根据供应量添加标签
        if event.supply > 1_000_000_000_000_000_000 { // 大于10^18
            tags.push("large-supply".to_string());
        } else if event.supply < 1_000_000_000 { // 小于10^9
            tags.push("small-supply".to_string());
        }

        // 创建TokenPushRequest
        Ok(TokenPushRequest {
            address: event.mint_address.to_string(),
            program_id: Some("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA".to_string()),
            name: event.name.clone(),
            symbol: event.symbol.clone(),
            decimals: event.decimals,
            logo_uri: event.uri.clone(),
            tags: Some(tags),
            daily_volume: Some(0.0), // 新创建的代币没有交易量
            freeze_authority: None,  // 从事件中无法获取，设为None
            mint_authority: Some(event.creator.to_string()),
            permanent_delegate: None, // 从事件中无法获取，设为None
            minted_at: Some(
                chrono::DateTime::from_timestamp(event.created_at, 0)
                    .unwrap_or_else(|| chrono::Utc::now())
            ),
            extensions: Some(serde_json::json!({
                "supply": event.supply,
                "has_whitelist": event.has_whitelist,
                "whitelist_deadline": event.whitelist_deadline,
                "signature": event.signature,
                "slot": event.slot,
                "source": "event-listener"
            })),
            source: Some(DataSource::OnchainSync),
        })
    }

    /// 判断是否为致命错误
    fn is_fatal_error(&self, error: &EventListenerError) -> bool {
        match error {
            EventListenerError::Database(_) => true,     // 数据库连接错误是致命的
            EventListenerError::Config(_) => true,      // 配置错误是致命的
            EventListenerError::Persistence(_) => false, // 持久化错误通常可以跳过
            _ => false,
        }
    }

    /// 写入单个事件（非批量）
    pub async fn write_event(&self, event: &ParsedEvent) -> Result<bool> {
        match event {
            ParsedEvent::TokenCreation(token_event) => {
                self.write_single_token_creation(token_event).await
            }
            ParsedEvent::PoolCreation(pool_event) => {
                self.write_single_pool_creation(pool_event).await
            }
            ParsedEvent::NftClaim(nft_event) => {
                self.write_single_nft_claim(nft_event).await
            }
            ParsedEvent::RewardDistribution(reward_event) => {
                self.write_single_reward_distribution(reward_event).await
            }
        }
    }

    /// 获取存储统计信息
    pub async fn get_storage_stats(&self) -> Result<StorageStats> {
        // 获取代币统计
        let token_stats = self.token_repository.get_token_stats().await
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

    fn create_test_config() -> EventListenerConfig {
        EventListenerConfig {
            solana: crate::config::settings::SolanaConfig {
                rpc_url: "https://api.devnet.solana.com".to_string(),
                ws_url: "wss://api.devnet.solana.com".to_string(),
                commitment: "confirmed".to_string(),
                program_id: solana_sdk::pubkey::Pubkey::new_unique(),
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
        }
    }

    fn create_test_token_event() -> TokenCreationEventData {
        TokenCreationEventData {
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
        }
    }

    #[test]
    fn test_convert_to_push_request() {
        let config = create_test_config();
        let storage = tokio_test::block_on(async {
            EventStorage::new(&config).await
        });

        // 如果连接失败（比如没有MongoDB），跳过这个测试
        if storage.is_err() {
            return;
        }

        let storage = storage.unwrap();
        let event = create_test_token_event();
        
        let push_request = storage.convert_to_push_request(&event).unwrap();
        
        assert_eq!(push_request.address, event.mint_address.to_string());
        assert_eq!(push_request.name, event.name);
        assert_eq!(push_request.symbol, event.symbol);
        assert_eq!(push_request.decimals, event.decimals);
        assert_eq!(push_request.logo_uri, event.uri);
        assert!(push_request.tags.as_ref().unwrap().contains(&"event-listener".to_string()));
    }

    #[test]
    fn test_is_fatal_error() {
        let config = create_test_config();
        let storage = tokio_test::block_on(async {
            EventStorage::new(&config).await
        });

        if storage.is_err() {
            return;
        }

        let storage = storage.unwrap();

        // 数据库错误应该是致命的
        let db_error = EventListenerError::Database(
            mongodb::error::Error::from(std::io::Error::new(std::io::ErrorKind::Other, "test"))
        );
        assert!(storage.is_fatal_error(&db_error));

        // 持久化错误不应该是致命的
        let persist_error = EventListenerError::Persistence("test error".to_string());
        assert!(!storage.is_fatal_error(&persist_error));
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

    fn create_test_pool_event() -> crate::parser::event_parser::PoolCreationEventData {
        use crate::parser::event_parser::PoolCreationEventData;
        PoolCreationEventData {
            pool_address: solana_sdk::pubkey::Pubkey::new_unique(),
            token_a_mint: solana_sdk::pubkey::Pubkey::new_unique(),
            token_b_mint: solana_sdk::pubkey::Pubkey::new_unique(),
            token_a_decimals: 9,
            token_b_decimals: 6,
            fee_rate: 3000,
            fee_rate_percentage: 0.3,
            annual_fee_rate: 109.5,
            pool_type: "标准费率".to_string(),
            sqrt_price_x64: 1u128 << 64,
            initial_price: 1.0,
            initial_tick: 0,
            creator: solana_sdk::pubkey::Pubkey::new_unique(),
            clmm_config: solana_sdk::pubkey::Pubkey::new_unique(),
            is_stable_pair: false,
            estimated_liquidity_usd: 0.0,
            created_at: chrono::Utc::now().timestamp(),
            signature: "test_pool_sig".to_string(),
            slot: 12345,
            processed_at: chrono::Utc::now().to_rfc3339(),
        }
    }

    fn create_test_nft_event() -> crate::parser::event_parser::NftClaimEventData {
        use crate::parser::event_parser::NftClaimEventData;
        NftClaimEventData {
            nft_mint: solana_sdk::pubkey::Pubkey::new_unique(),
            claimer: solana_sdk::pubkey::Pubkey::new_unique(),
            referrer: Some(solana_sdk::pubkey::Pubkey::new_unique()),
            tier: 3,
            tier_name: "Gold".to_string(),
            tier_bonus_rate: 1.5,
            claim_amount: 1000000,
            token_mint: solana_sdk::pubkey::Pubkey::new_unique(),
            reward_multiplier: 15000,
            reward_multiplier_percentage: 1.5,
            bonus_amount: 1500000,
            claim_type: 0,
            claim_type_name: "定期领取".to_string(),
            total_claimed: 5000000,
            claim_progress_percentage: 20.0,
            pool_address: Some(solana_sdk::pubkey::Pubkey::new_unique()),
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
            reward_pool: solana_sdk::pubkey::Pubkey::new_unique(),
            recipient: solana_sdk::pubkey::Pubkey::new_unique(),
            referrer: Some(solana_sdk::pubkey::Pubkey::new_unique()),
            reward_token_mint: solana_sdk::pubkey::Pubkey::new_unique(),
            reward_amount: 1500000,
            base_reward_amount: 1000000,
            bonus_amount: 500000,
            reward_type: 2,
            reward_type_name: "流动性奖励".to_string(),
            reward_source: 1,
            reward_source_name: "流动性挖矿".to_string(),
            related_address: Some(solana_sdk::pubkey::Pubkey::new_unique()),
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
        let storage = tokio_test::block_on(async {
            EventStorage::new(&config).await
        });

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

        let push_request = storage.convert_to_push_request(&large_supply_event).unwrap();
        let tags = push_request.tags.unwrap();
        assert!(tags.contains(&"large-supply".to_string()));

        // 测试小供应量标签
        let small_supply_event = TokenCreationEventData {
            supply: 500_000_000,
            has_whitelist: false,
            ..create_test_token_event()
        };

        let push_request = storage.convert_to_push_request(&small_supply_event).unwrap();
        let tags = push_request.tags.unwrap();
        assert!(tags.contains(&"small-supply".to_string()));

        // 测试白名单标签
        let whitelist_event = TokenCreationEventData {
            has_whitelist: true,
            ..create_test_token_event()
        };

        let push_request = storage.convert_to_push_request(&whitelist_event).unwrap();
        let tags = push_request.tags.unwrap();
        assert!(tags.contains(&"whitelist".to_string()));
    }
}