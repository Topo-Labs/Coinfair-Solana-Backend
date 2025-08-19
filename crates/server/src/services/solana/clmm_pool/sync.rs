//! CLMM池子数据同步服务
//!
//! 负责同步链上池子数据到本地数据库，确保数据一致性

use super::super::shared::SharedContext;
use super::storage::ClmmPoolStorageService;
use database::clmm_pool::{ClmmPool, SyncStatus};
use solana_sdk::{program_pack::Pack, pubkey::Pubkey};
use spl_token::state::Mint;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::{interval, sleep};
use tracing::{debug, error, info, warn};
use utils::{AppResult, MetaplexService};

/// 数据同步服务配置
#[derive(Debug, Clone)]
pub struct SyncConfig {
    /// 同步间隔 (秒)
    pub sync_interval: u64,
    /// 每批次同步的池子数量
    pub batch_size: i64,
    /// 同步重试次数
    pub max_retries: u32,
    /// 重试间隔 (秒)
    pub retry_interval: u64,
    /// 是否启用自动同步
    pub auto_sync_enabled: bool,
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            sync_interval: std::env::var("CLMM_SYNC_INTERVAL")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(10), // 1分钟
            batch_size: std::env::var("CLMM_SYNC_BATCH_SIZE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(50), // 每批次50个池子
            max_retries: std::env::var("CLMM_SYNC_MAX_RETRIES")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(3), // 最多重试3次
            retry_interval: std::env::var("CLMM_SYNC_RETRY_INTERVAL")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(30), // 重试间隔30秒
            auto_sync_enabled: std::env::var("CLMM_AUTO_SYNC_ENABLED")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(true),
        }
    }
}

impl SyncConfig {
    /// 从环境变量创建配置
    pub fn from_env() -> Self {
        Self::default()
    }

    /// 验证配置参数
    pub fn validate(&self) -> Result<(), String> {
        if self.sync_interval < 10 {
            return Err("同步间隔不能小于10秒".to_string());
        }
        if self.batch_size < 1 || self.batch_size > 1000 {
            return Err("批次大小必须在1-1000之间".to_string());
        }
        if self.max_retries > 10 {
            return Err("最大重试次数不能超过10次".to_string());
        }
        if self.retry_interval < 1 {
            return Err("重试间隔不能小于1秒".to_string());
        }
        Ok(())
    }
}

/// CLMM池子数据同步服务
pub struct ClmmPoolSyncService {
    shared: Arc<SharedContext>,
    storage: ClmmPoolStorageService,
    config: SyncConfig,
    metaplex_service: tokio::sync::Mutex<MetaplexService>,
}

impl ClmmPoolSyncService {
    /// 创建新的同步服务实例
    pub fn new(shared: Arc<SharedContext>, storage: ClmmPoolStorageService, config: Option<SyncConfig>) -> Self {
        let metaplex_service = MetaplexService::new(None).expect("Failed to create MetaplexService");

        Self {
            shared,
            storage,
            config: config.unwrap_or_default(),
            metaplex_service: tokio::sync::Mutex::new(metaplex_service),
        }
    }

    /// 启动自动同步任务
    pub async fn start_auto_sync(&self) -> AppResult<()> {
        if !self.config.auto_sync_enabled {
            info!("🔄 自动同步已禁用");
            return Ok(());
        }

        info!("🔄 启动CLMM池子自动同步服务，间隔: {}秒", self.config.sync_interval);

        let mut interval = interval(Duration::from_secs(self.config.sync_interval));

        loop {
            interval.tick().await;

            match self.sync_pools_batch().await {
                Ok(synced_count) => {
                    if synced_count > 0 {
                        info!("✅ 批次同步完成，同步了 {} 个池子", synced_count);
                    } else {
                        info!("🔄 没有需要同步的池子");
                    }
                }
                Err(e) => {
                    error!("❌ 批次同步失败: {}", e);
                }
            }
        }
    }

    /// 批量同步池子数据
    pub async fn sync_pools_batch(&self) -> AppResult<u64> {
        info!("🔄 开始批量同步池子数据...");

        // 获取需要同步的池子列表
        let pools_to_sync = self.storage.get_pools_need_sync(Some(self.config.batch_size)).await?;
        if pools_to_sync.is_empty() {
            info!("✅ 没有需要同步的池子");
            return Ok(0);
        }

        info!("📋 找到 {} 个需要同步的池子", pools_to_sync.len());

        // 批量获取mint信息以减少RPC调用
        let mint_addresses: Vec<_> = pools_to_sync
            .iter()
            .flat_map(|pool| vec![&pool.mint0.mint_address, &pool.mint1.mint_address])
            .collect();

        let mint_info_cache = match self.batch_fetch_mint_info(&mint_addresses).await {
            Ok(cache) => cache,
            Err(e) => {
                warn!("⚠️ 批量获取mint信息失败，将使用单独同步: {}", e);
                // 如果批量获取失败，回退到单独同步模式
                return self.sync_pools_batch_fallback(pools_to_sync).await;
            }
        };

        // 🔄 批量同步代币元数据
        let unique_mint_addresses: Vec<String> = mint_addresses.iter().map(|s| s.to_string()).collect();
        let unique_mints: std::collections::HashSet<String> = unique_mint_addresses.into_iter().collect();
        let mint_list: Vec<String> = unique_mints.into_iter().collect();

        info!("📦 开始同步 {} 个代币的元数据", mint_list.len());
        match self.sync_token_metadata_batch(&mint_list).await {
            Ok(updated_count) => {
                if updated_count > 0 {
                    info!("✅ 成功同步了 {} 个代币的元数据", updated_count);
                }
            }
            Err(e) => {
                warn!("⚠️ 批量同步代币元数据失败: {}", e);
            }
        }

        let mut synced_count = 0u64;
        let mut failed_pools = Vec::new();

        for pool in pools_to_sync {
            match self.sync_single_pool_with_cache(&pool, &mint_info_cache).await {
                Ok(true) => {
                    synced_count += 1;
                    info!("✅ 池子同步成功: {}", pool.pool_address);
                }
                Ok(false) => {
                    info!("⚠️ 池子无需更新: {}", pool.pool_address);
                }
                Err(e) => {
                    error!("❌ 池子同步失败: {} - {}", pool.pool_address, e);
                    failed_pools.push((pool.pool_address.clone(), e.to_string()));
                }
            }

            // 避免过于频繁的请求
            sleep(Duration::from_millis(50)).await;
        }

        // 批量标记失败的池子
        if !failed_pools.is_empty() {
            for (pool_address, error_msg) in failed_pools {
                if let Err(mark_err) = self.storage.mark_sync_failed(&pool_address, &error_msg).await {
                    error!("❌ 标记同步失败状态失败: {} - {}", pool_address, mark_err);
                }
            }
        }

        info!("🔄 批量同步完成，成功同步 {} 个池子", synced_count);
        Ok(synced_count)
    }

    /// 回退到单独同步模式（当批量获取mint信息失败时使用）
    async fn sync_pools_batch_fallback(&self, pools_to_sync: Vec<ClmmPool>) -> AppResult<u64> {
        info!("🔄 使用回退模式进行单独同步...");

        let mut synced_count = 0u64;

        for pool in pools_to_sync {
            match self.sync_single_pool(&pool).await {
                Ok(true) => {
                    synced_count += 1;
                    info!("✅ 池子同步成功: {}", pool.pool_address);
                }
                Ok(false) => {
                    info!("⚠️ 池子无需更新: {}", pool.pool_address);
                }
                Err(e) => {
                    info!("❌ 池子同步失败: {} - {}", pool.pool_address, e);

                    // 标记同步失败
                    if let Err(mark_err) = self.storage.mark_sync_failed(&pool.pool_address, &e.to_string()).await {
                        error!("❌ 标记同步失败状态失败: {} - {}", pool.pool_address, mark_err);
                    }
                }
            }

            // 避免过于频繁的请求
            sleep(Duration::from_millis(100)).await;
        }

        info!("🔄 回退模式同步完成，成功同步 {} 个池子", synced_count);
        Ok(synced_count)
    }

    /// 批量获取mint信息以优化RPC调用
    async fn batch_fetch_mint_info(
        &self,
        mint_addresses: &[&String],
    ) -> AppResult<std::collections::HashMap<String, (u8, String)>> {
        use std::collections::HashMap;
        use std::str::FromStr;

        let mut cache = HashMap::new();

        // 去重mint地址
        let unique_mints: std::collections::HashSet<_> = mint_addresses.iter().cloned().collect();
        let pubkeys: Result<Vec<_>, _> = unique_mints.iter().map(|addr| Pubkey::from_str(addr)).collect();

        let pubkeys = pubkeys.map_err(|e| anyhow::anyhow!("无效的mint地址: {}", e))?;

        if pubkeys.is_empty() {
            return Ok(cache);
        }

        // 批量获取账户信息
        let accounts = self
            .shared
            .rpc_client
            .get_multiple_accounts(&pubkeys)
            .map_err(|e| anyhow::anyhow!("批量获取mint账户失败: {}", e))?;

        for (i, account_opt) in accounts.iter().enumerate() {
            if let Some(account) = account_opt {
                if let Ok(mint_state) = Mint::unpack(&account.data) {
                    let mint_address = unique_mints.iter().nth(i).unwrap().to_string();
                    cache.insert(mint_address, (mint_state.decimals, account.owner.to_string()));
                }
            }
        }

        debug!("📦 批量获取了 {} 个mint信息", cache.len());
        Ok(cache)
    }

    /// 批量同步代币元数据到数据库
    async fn sync_token_metadata_batch(&self, mint_addresses: &[String]) -> AppResult<u64> {
        if mint_addresses.is_empty() {
            return Ok(0);
        }

        info!("🔄 开始批量同步 {} 个代币的元数据", mint_addresses.len());

        // 批量获取代币元数据
        let metadata_map = {
            let mut metaplex_service = self.metaplex_service.lock().await;
            match metaplex_service.get_tokens_metadata(mint_addresses).await {
                Ok(map) => map,
                Err(e) => {
                    warn!("⚠️ 批量获取代币元数据失败: {}", e);
                    return Ok(0);
                }
            }
        };

        let mut updated_count = 0u64;

        // 逐个更新数据库中的代币信息
        for (mint_address, metadata) in metadata_map.iter() {
            match self.storage.update_token_metadata(mint_address, metadata).await {
                Ok(true) => {
                    updated_count += 1;
                    debug!(
                        "✅ 代币元数据已更新: {} - {}",
                        mint_address,
                        metadata.symbol.as_deref().unwrap_or("Unknown")
                    );
                }
                Ok(false) => {
                    debug!("ℹ️ 代币元数据无需更新: {}", mint_address);
                }
                Err(e) => {
                    warn!("⚠️ 更新代币元数据失败: {} - {}", mint_address, e);
                }
            }

            // 避免过于频繁的数据库写入
            if updated_count % 10 == 0 {
                sleep(Duration::from_millis(10)).await;
            }
        }

        info!("📦 批量同步代币元数据完成，更新了 {} 个代币", updated_count);
        Ok(updated_count)
    }

    /// 使用缓存同步单个池子以减少RPC调用
    async fn sync_single_pool_with_cache(
        &self,
        pool: &ClmmPool,
        mint_cache: &std::collections::HashMap<String, (u8, String)>,
    ) -> AppResult<bool> {
        let mut mint0_info: Option<(u8, String)> = None;
        let mut mint1_info: Option<(u8, String)> = None;
        let mut has_updates = false;

        // 从缓存获取mint0信息
        if let Some((decimals, owner)) = mint_cache.get(&pool.mint0.mint_address) {
            if pool.mint0.decimals != *decimals || pool.mint0.owner != *owner {
                mint0_info = Some((*decimals, owner.clone()));
                has_updates = true;
            }
        }

        // 从缓存获取mint1信息
        if let Some((decimals, owner)) = mint_cache.get(&pool.mint1.mint_address) {
            if pool.mint1.decimals != *decimals || pool.mint1.owner != *owner {
                mint1_info = Some((*decimals, owner.clone()));
                has_updates = true;
            }
        }

        // TODO: 这里可以添加获取池子当前价格、流动性等信息的逻辑
        let current_price = None;
        let current_tick = None;

        if has_updates {
            self.storage
                .update_pool_onchain_data(&pool.pool_address, mint0_info, mint1_info, current_price, current_tick)
                .await?;

            debug!("✅ 池子数据更新完成: {}", pool.pool_address);
            Ok(true)
        } else {
            // 即使没有数据更新，也要更新同步状态
            let sync_status = SyncStatus {
                last_sync_at: chrono::Utc::now().timestamp() as u64,
                sync_version: pool.sync_status.sync_version + 1,
                needs_sync: false,
                sync_error: None,
            };

            self.storage
                .update_sync_status(&pool.pool_address, &sync_status)
                .await?;
            debug!("🔄 池子同步状态已更新: {}", pool.pool_address);
            Ok(false)
        }
    }

    /// 同步单个池子的数据
    pub async fn sync_single_pool(&self, pool: &ClmmPool) -> AppResult<bool> {
        debug!("🔄 同步池子数据: {}", pool.pool_address);

        let mut retry_count = 0;
        let mut last_error: Option<utils::AppError> = None;

        while retry_count < self.config.max_retries {
            match self.fetch_and_update_pool_data(pool).await {
                Ok(updated) => {
                    if retry_count > 0 {
                        info!(
                            "✅ 池子同步重试成功: {} (尝试次数: {})",
                            pool.pool_address,
                            retry_count + 1
                        );
                    }
                    return Ok(updated);
                }
                Err(e) => {
                    // 检查是否为不可重试的错误
                    let error_msg = e.to_string().to_lowercase();
                    let is_retryable = !error_msg.contains("invalid")
                        && !error_msg.contains("not found")
                        && !error_msg.contains("parse");

                    if retry_count < self.config.max_retries && is_retryable {
                        retry_count += 1;
                        let delay = Duration::from_secs(self.config.retry_interval * retry_count as u64);
                        warn!(
                            "⚠️ 池子同步失败，将重试: {} (尝试 {}/{}) - {} (延迟: {:?})",
                            pool.pool_address, retry_count, self.config.max_retries, e, delay
                        );
                        sleep(delay).await;
                    } else {
                        error!("❌ 池子同步最终失败: {} - {}", pool.pool_address, e);
                        return Err(e);
                    }
                    last_error = Some(e);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("同步重试次数已用完").into()))
    }

    /// 从链上获取并更新池子数据
    async fn fetch_and_update_pool_data(&self, pool: &ClmmPool) -> AppResult<bool> {
        // 使用错误处理器进行重试
        use super::error_handler::{ErrorHandler, RetryConfig};

        let retry_config = RetryConfig {
            max_retries: self.config.max_retries,
            base_delay_ms: self.config.retry_interval * 1000,
            backoff_multiplier: 1.5,
            max_delay_ms: 60000,
            jitter_factor: 0.1,
        };

        let error_handler = ErrorHandler::new(Some(retry_config));
        let pool_address = pool.pool_address.clone();

        error_handler
            .execute_with_retry(&format!("同步池子数据: {}", pool_address), || {
                self.fetch_pool_data_once(pool)
            })
            .await
    }

    /// 单次获取池子数据（不包含重试逻辑）
    async fn fetch_pool_data_once(&self, pool: &ClmmPool) -> AppResult<bool> {
        // 1. 获取mint信息
        let mint0_pubkey =
            Pubkey::from_str(&pool.mint0.mint_address).map_err(|e| anyhow::anyhow!("无效的mint0地址: {}", e))?;
        let mint1_pubkey =
            Pubkey::from_str(&pool.mint1.mint_address).map_err(|e| anyhow::anyhow!("无效的mint1地址: {}", e))?;

        let load_pubkeys = vec![mint0_pubkey, mint1_pubkey];
        let accounts = self
            .shared
            .rpc_client
            .get_multiple_accounts(&load_pubkeys)
            .map_err(|e| anyhow::anyhow!("获取账户信息失败: {}", e))?;

        let mut mint0_info: Option<(u8, String)> = None;
        let mut mint1_info: Option<(u8, String)> = None;
        let mut has_updates = false;

        // 2. 解析mint0信息
        if let Some(mint0_account) = &accounts[0] {
            if let Ok(mint0_state) = Mint::unpack(&mint0_account.data) {
                let decimals = mint0_state.decimals;
                let owner = mint0_account.owner.to_string();

                // 检查是否需要更新
                if pool.mint0.decimals != decimals || pool.mint0.owner != owner {
                    mint0_info = Some((decimals, owner));
                    has_updates = true;
                    debug!(
                        "🔄 Mint0信息需要更新: decimals={}, owner={}",
                        decimals, mint0_account.owner
                    );
                }
            }
        }

        // 3. 解析mint1信息
        if let Some(mint1_account) = &accounts[1] {
            if let Ok(mint1_state) = Mint::unpack(&mint1_account.data) {
                let decimals = mint1_state.decimals;
                let owner = mint1_account.owner.to_string();

                // 检查是否需要更新
                if pool.mint1.decimals != decimals || pool.mint1.owner != owner {
                    mint1_info = Some((decimals, owner));
                    has_updates = true;
                    debug!(
                        "🔄 Mint1信息需要更新: decimals={}, owner={}",
                        decimals, mint1_account.owner
                    );
                }
            }
        }

        // 4. 获取池子当前状态 (这里可以扩展获取更多链上数据)
        // TODO: 可以添加获取池子当前价格、流动性等信息的逻辑
        let current_price = None; // 暂时不更新价格
        let current_tick = None; // 暂时不更新tick

        // 5. 如果有更新，则保存到数据库
        if has_updates {
            self.storage
                .update_pool_onchain_data(&pool.pool_address, mint0_info, mint1_info, current_price, current_tick)
                .await?;

            debug!("✅ 池子数据更新完成: {}", pool.pool_address);
            Ok(true)
        } else {
            // 即使没有数据更新，也要更新同步状态
            let sync_status = SyncStatus {
                last_sync_at: chrono::Utc::now().timestamp() as u64,
                sync_version: pool.sync_status.sync_version + 1,
                needs_sync: false,
                sync_error: None,
            };

            self.storage
                .update_sync_status(&pool.pool_address, &sync_status)
                .await?;
            debug!("🔄 池子同步状态已更新: {}", pool.pool_address);
            Ok(false)
        }
    }

    /// 手动触发全量同步
    pub async fn trigger_full_sync(&self) -> AppResult<u64> {
        info!("🔄 开始全量同步所有池子...");

        let mut total_synced = 0u64;

        loop {
            // 分批获取需要同步的池子
            let pools = self.storage.get_pools_need_sync(Some(self.config.batch_size)).await?;

            if pools.is_empty() {
                break;
            }

            info!("🔄 同步批次 - {} 个池子", pools.len());

            for pool in pools {
                match self.sync_single_pool(&pool).await {
                    Ok(true) => {
                        total_synced += 1;
                    }
                    Ok(false) => {
                        // 无需更新，但也算作处理成功
                    }
                    Err(e) => {
                        error!("❌ 池子同步失败: {} - {}", pool.pool_address, e);

                        // 标记同步失败
                        if let Err(mark_err) = self.storage.mark_sync_failed(&pool.pool_address, &e.to_string()).await {
                            error!("❌ 标记同步失败状态失败: {} - {}", pool.pool_address, mark_err);
                        }
                    }
                }

                // 避免过于频繁的请求
                sleep(Duration::from_millis(200)).await;
            }
        }

        info!("✅ 全量同步完成，总共同步了 {} 个池子", total_synced);
        Ok(total_synced)
    }

    /// 标记指定池子需要同步
    pub async fn mark_pools_for_sync(&self, pool_addresses: &[String]) -> AppResult<u64> {
        info!("🔄 标记 {} 个池子需要同步", pool_addresses.len());

        let marked_count = self.storage.mark_pools_for_sync(pool_addresses).await?;

        info!("✅ 成功标记 {} 个池子需要同步", marked_count);
        Ok(marked_count)
    }

    /// 获取同步统计信息
    pub async fn get_sync_stats(&self) -> AppResult<SyncStats> {
        info!("📊 获取同步统计信息");

        // 这里可以扩展更详细的统计信息
        let pools_need_sync = self.storage.get_pools_need_sync(Some(1000)).await?;
        let total_need_sync = pools_need_sync.len() as u64;

        // TODO: 实际项目中应该从持久化存储中获取这些指标
        let performance_metrics = SyncPerformanceMetrics {
            total_sync_count: 0,
            success_sync_count: 0,
            failed_sync_count: 0,
            avg_sync_time_ms: 0.0,
            last_sync_duration_ms: 0,
            rpc_call_count: 0,
            rpc_failure_count: 0,
        };

        let stats = SyncStats {
            total_pools_need_sync: total_need_sync,
            last_sync_time: chrono::Utc::now().timestamp() as u64,
            sync_config: self.config.clone(),
            performance_metrics,
        };

        Ok(stats)
    }
}

/// 同步统计信息
#[derive(Debug, Clone)]
pub struct SyncStats {
    /// 需要同步的池子总数
    pub total_pools_need_sync: u64,
    /// 最后同步时间
    pub last_sync_time: u64,
    /// 同步配置
    pub sync_config: SyncConfig,
    /// 同步性能指标
    pub performance_metrics: SyncPerformanceMetrics,
}

/// 同步性能指标
#[derive(Debug, Clone)]
pub struct SyncPerformanceMetrics {
    /// 总同步次数
    pub total_sync_count: u64,
    /// 成功同步次数
    pub success_sync_count: u64,
    /// 失败同步次数
    pub failed_sync_count: u64,
    /// 平均同步时间 (毫秒)
    pub avg_sync_time_ms: f64,
    /// 最后一次同步耗时 (毫秒)
    pub last_sync_duration_ms: u64,
    /// RPC调用次数
    pub rpc_call_count: u64,
    /// RPC调用失败次数
    pub rpc_failure_count: u64,
}

/// 同步服务构建器
pub struct ClmmPoolSyncBuilder;

impl ClmmPoolSyncBuilder {
    /// 从共享上下文和存储服务创建同步服务
    pub fn from_context_and_storage(
        shared: Arc<SharedContext>,
        storage: ClmmPoolStorageService,
        config: Option<SyncConfig>,
    ) -> ClmmPoolSyncService {
        ClmmPoolSyncService::new(shared, storage, config)
    }
}
