//! CLMM池子数据同步服务
//! 
//! 负责同步链上池子数据到本地数据库，确保数据一致性

use super::storage::ClmmPoolStorageService;
use super::super::shared::SharedContext;
use database::clmm_pool::{ClmmPool, SyncStatus};
use solana_sdk::{program_pack::Pack, pubkey::Pubkey};
use spl_token::state::Mint;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::{interval, sleep};
use tracing::{info, error, warn, debug};
use utils::AppResult;

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
            sync_interval: 300,    // 5分钟
            batch_size: 50,        // 每批次50个池子
            max_retries: 3,        // 最多重试3次
            retry_interval: 30,    // 重试间隔30秒
            auto_sync_enabled: true,
        }
    }
}

/// CLMM池子数据同步服务
pub struct ClmmPoolSyncService {
    shared: Arc<SharedContext>,
    storage: ClmmPoolStorageService,
    config: SyncConfig,
}

impl ClmmPoolSyncService {
    /// 创建新的同步服务实例
    pub fn new(
        shared: Arc<SharedContext>, 
        storage: ClmmPoolStorageService,
        config: Option<SyncConfig>
    ) -> Self {
        Self {
            shared,
            storage,
            config: config.unwrap_or_default(),
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
                        debug!("🔄 没有需要同步的池子");
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
            debug!("没有需要同步的池子");
            return Ok(0);
        }

        info!("📋 找到 {} 个需要同步的池子", pools_to_sync.len());
        
        let mut synced_count = 0u64;
        
        for pool in pools_to_sync {
            match self.sync_single_pool(&pool).await {
                Ok(true) => {
                    synced_count += 1;
                    debug!("✅ 池子同步成功: {}", pool.pool_address);
                }
                Ok(false) => {
                    debug!("⚠️ 池子无需更新: {}", pool.pool_address);
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
            sleep(Duration::from_millis(100)).await;
        }
        
        info!("🔄 批量同步完成，成功同步 {} 个池子", synced_count);
        Ok(synced_count)
    }

    /// 同步单个池子的数据
    pub async fn sync_single_pool(&self, pool: &ClmmPool) -> AppResult<bool> {
        debug!("🔄 同步池子数据: {}", pool.pool_address);
        
        let mut retry_count = 0;
        
        while retry_count < self.config.max_retries {
            match self.fetch_and_update_pool_data(pool).await {
                Ok(updated) => {
                    return Ok(updated);
                }
                Err(e) => {
                    retry_count += 1;
                    warn!("⚠️ 池子同步失败 (重试 {}/{}): {} - {}", 
                          retry_count, self.config.max_retries, pool.pool_address, e);
                    
                    if retry_count < self.config.max_retries {
                        sleep(Duration::from_secs(self.config.retry_interval)).await;
                    } else {
                        return Err(e);
                    }
                }
            }
        }
        
        Err(anyhow::anyhow!("同步重试次数已用完").into())
    }

    /// 从链上获取并更新池子数据
    async fn fetch_and_update_pool_data(&self, pool: &ClmmPool) -> AppResult<bool> {
        // 1. 获取mint信息
        let mint0_pubkey = Pubkey::from_str(&pool.mint0.mint_address)
            .map_err(|e| anyhow::anyhow!("无效的mint0地址: {}", e))?;
        let mint1_pubkey = Pubkey::from_str(&pool.mint1.mint_address)
            .map_err(|e| anyhow::anyhow!("无效的mint1地址: {}", e))?;
        
        let load_pubkeys = vec![mint0_pubkey, mint1_pubkey];
        let accounts = self.shared.rpc_client.get_multiple_accounts(&load_pubkeys)
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
                    debug!("🔄 Mint0信息需要更新: decimals={}, owner={}", decimals, mint0_account.owner);
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
                    debug!("🔄 Mint1信息需要更新: decimals={}, owner={}", decimals, mint1_account.owner);
                }
            }
        }
        
        // 4. 获取池子当前状态 (这里可以扩展获取更多链上数据)
        // TODO: 可以添加获取池子当前价格、流动性等信息的逻辑
        let current_price = None; // 暂时不更新价格
        let current_tick = None;  // 暂时不更新tick
        
        // 5. 如果有更新，则保存到数据库
        if has_updates {
            self.storage.update_pool_onchain_data(
                &pool.pool_address,
                mint0_info,
                mint1_info,
                current_price,
                current_tick
            ).await?;
            
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
            
            self.storage.update_sync_status(&pool.pool_address, &sync_status).await?;
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
        
        let stats = SyncStats {
            total_pools_need_sync: total_need_sync,
            last_sync_time: chrono::Utc::now().timestamp() as u64,
            sync_config: self.config.clone(),
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
}

/// 同步服务构建器
pub struct ClmmPoolSyncBuilder;

impl ClmmPoolSyncBuilder {
    /// 从共享上下文和存储服务创建同步服务
    pub fn from_context_and_storage(
        shared: Arc<SharedContext>,
        storage: ClmmPoolStorageService,
        config: Option<SyncConfig>
    ) -> ClmmPoolSyncService {
        ClmmPoolSyncService::new(shared, storage, config)
    }
}