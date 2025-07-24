//! CLMM池子存储服务
//! 
//! 负责将池子创建后的元数据存储到MongoDB数据库中

use crate::dtos::solana_dto::{CreatePoolRequest, CreatePoolResponse, CreatePoolAndSendTransactionResponse};
use database::clmm_pool::{
    ClmmPool, TokenInfo, PriceInfo, VaultInfo, ExtensionInfo, 
    TransactionInfo, TransactionStatus, PoolStatus, SyncStatus,
    ClmmPoolRepository
};
use mongodb::Collection;
use tracing::{info, error, warn};
use utils::AppResult;

/// CLMM池子存储服务
pub struct ClmmPoolStorageService {
    repository: ClmmPoolRepository,
}

impl ClmmPoolStorageService {
    /// 创建新的存储服务实例
    pub fn new(collection: Collection<ClmmPool>) -> Self {
        let repository = ClmmPoolRepository::new(collection);
        Self { repository }
    }

    /// 初始化数据库索引
    pub async fn init_indexes(&self) -> AppResult<()> {
        self.repository.init_indexes().await
    }

    /// 存储池子创建响应数据 (仅构建交易，未发送)
    pub async fn store_pool_creation(&self, 
        request: &CreatePoolRequest, 
        response: &CreatePoolResponse
    ) -> AppResult<String> {
        info!("💾 存储池子创建数据: {}", response.pool_address);

        let now = chrono::Utc::now().timestamp() as u64;

        // 解析mint地址，确保顺序正确
        let mut mint0_addr = request.mint0.clone();
        let mut mint1_addr = request.mint1.clone();
        // let mut price = request.price;

        // 如果mint0 > mint1，需要交换顺序
        if mint0_addr > mint1_addr {
            std::mem::swap(&mut mint0_addr, &mut mint1_addr);
            // price = 1.0 / price;
        }

        let pool = ClmmPool {
            id: None,
            pool_address: response.pool_address.clone(),
            amm_config_address: response.amm_config_address.clone(),
            config_index: request.config_index,
            
            mint0: TokenInfo {
                mint_address: mint0_addr,
                decimals: 0, // 需要从链上获取，暂时设为0
                owner: String::new(), // 需要从链上获取
                symbol: None,
                name: None,
            },
            
            mint1: TokenInfo {
                mint_address: mint1_addr,
                decimals: 0, // 需要从链上获取，暂时设为0
                owner: String::new(), // 需要从链上获取
                symbol: None,
                name: None,
            },
            
            price_info: PriceInfo {
                initial_price: response.initial_price,
                sqrt_price_x64: response.sqrt_price_x64.clone(),
                initial_tick: response.initial_tick,
                current_price: Some(response.initial_price),
                current_tick: Some(response.initial_tick),
            },
            
            vault_info: VaultInfo {
                token_vault_0: response.token_vault_0.clone(),
                token_vault_1: response.token_vault_1.clone(),
            },
            
            extension_info: ExtensionInfo {
                observation_address: response.observation_address.clone(),
                tickarray_bitmap_extension: response.tickarray_bitmap_extension.clone(),
            },
            
            creator_wallet: request.user_wallet.clone(),
            open_time: request.open_time,
            created_at: now,
            updated_at: now,
            transaction_info: None, // 仅构建交易时为空
            status: PoolStatus::Created,
            
            sync_status: SyncStatus {
                last_sync_at: now,
                sync_version: 1,
                needs_sync: true, // 新创建的池子需要同步链上数据
                sync_error: None,
            },
        };

        let pool_id = self.repository.create_pool(&pool).await?;
        info!("✅ 池子创建数据存储成功，ID: {}", pool_id);
        
        Ok(pool_id)
    }

    /// 存储池子创建并发送交易的响应数据
    pub async fn store_pool_creation_with_transaction(&self, 
        request: &CreatePoolRequest, 
        response: &CreatePoolAndSendTransactionResponse
    ) -> AppResult<String> {
        info!("💾 存储池子创建和交易数据: {}", response.pool_address);

        let now = chrono::Utc::now().timestamp() as u64;

        // 解析mint地址，确保顺序正确
        let mut mint0_addr = request.mint0.clone();
        let mut mint1_addr = request.mint1.clone();
        // let mut price = request.price;

        // 如果mint0 > mint1，需要交换顺序
        if mint0_addr > mint1_addr {
            std::mem::swap(&mut mint0_addr, &mut mint1_addr);
            // price = 1.0 / price;
        }

        let transaction_info = TransactionInfo {
            signature: response.signature.clone(),
            status: match response.status {
                crate::dtos::solana_dto::TransactionStatus::Finalized => TransactionStatus::Finalized,
                _ => TransactionStatus::Confirmed,
            },
            explorer_url: response.explorer_url.clone(),
            confirmed_at: now,
        };

        let pool = ClmmPool {
            id: None,
            pool_address: response.pool_address.clone(),
            amm_config_address: response.amm_config_address.clone(),
            config_index: request.config_index,
            
            mint0: TokenInfo {
                mint_address: mint0_addr,
                decimals: 0, // 需要从链上获取，暂时设为0
                owner: String::new(), // 需要从链上获取
                symbol: None,
                name: None,
            },
            
            mint1: TokenInfo {
                mint_address: mint1_addr,
                decimals: 0, // 需要从链上获取，暂时设为0
                owner: String::new(), // 需要从链上获取
                symbol: None,
                name: None,
            },
            
            price_info: PriceInfo {
                initial_price: response.initial_price,
                sqrt_price_x64: response.sqrt_price_x64.clone(),
                initial_tick: response.initial_tick,
                current_price: Some(response.initial_price),
                current_tick: Some(response.initial_tick),
            },
            
            vault_info: VaultInfo {
                token_vault_0: response.token_vault_0.clone(),
                token_vault_1: response.token_vault_1.clone(),
            },
            
            extension_info: ExtensionInfo {
                observation_address: response.observation_address.clone(),
                tickarray_bitmap_extension: response.tickarray_bitmap_extension.clone(),
            },
            
            creator_wallet: request.user_wallet.clone(),
            open_time: request.open_time,
            created_at: now,
            updated_at: now,
            transaction_info: Some(transaction_info),
            status: PoolStatus::Active, // 交易已确认，状态为活跃
            
            sync_status: SyncStatus {
                last_sync_at: now,
                sync_version: 1,
                needs_sync: true, // 需要同步完整的链上数据
                sync_error: None,
            },
        };

        let pool_id = self.repository.create_pool(&pool).await?;
        info!("✅ 池子创建和交易数据存储成功，ID: {}", pool_id);
        
        Ok(pool_id)
    }

    /// 直接存储池子数据 (用于测试)
    pub async fn store_pool(&self, pool: &ClmmPool) -> AppResult<String> {
        info!("💾 直接存储池子数据: {}", pool.pool_address);
        let pool_id = self.repository.create_pool(pool).await?;
        info!("✅ 池子数据存储成功，ID: {}", pool_id);
        Ok(pool_id)
    }

    /// 更新池子的链上数据 (用于数据同步)
    pub async fn update_pool_onchain_data(&self, 
        pool_address: &str,
        mint0_info: Option<(u8, String)>, // (decimals, owner)
        mint1_info: Option<(u8, String)>, // (decimals, owner)
        current_price: Option<f64>,
        current_tick: Option<i32>
    ) -> AppResult<bool> {
        info!("🔄 更新池子链上数据: {}", pool_address);

        let mut update_doc = mongodb::bson::Document::new();

        // 更新mint0信息
        if let Some((decimals, owner)) = mint0_info {
            update_doc.insert("mint0.decimals", decimals as i32);
            update_doc.insert("mint0.owner", owner);
        }

        // 更新mint1信息
        if let Some((decimals, owner)) = mint1_info {
            update_doc.insert("mint1.decimals", decimals as i32);
            update_doc.insert("mint1.owner", owner);
        }

        // 更新当前价格信息
        if let Some(price) = current_price {
            update_doc.insert("price_info.current_price", price);
        }

        if let Some(tick) = current_tick {
            update_doc.insert("price_info.current_tick", tick);
        }

        // 更新同步状态
        let now = chrono::Utc::now().timestamp() as u64;
        update_doc.insert("sync_status.last_sync_at", now as f64);
        update_doc.insert("sync_status.needs_sync", false);
        update_doc.insert("sync_status.sync_error", mongodb::bson::Bson::Null);

        let updated = self.repository.update_pool(pool_address, update_doc).await?;
        
        if updated {
            info!("✅ 池子链上数据更新成功: {}", pool_address);
        } else {
            warn!("⚠️ 池子链上数据更新失败，池子不存在: {}", pool_address);
        }

        Ok(updated)
    }

    /// 标记池子同步失败
    pub async fn mark_sync_failed(&self, pool_address: &str, error_msg: &str) -> AppResult<bool> {
        error!("❌ 池子同步失败: {} - {}", pool_address, error_msg);

        let sync_status = SyncStatus {
            last_sync_at: chrono::Utc::now().timestamp() as u64,
            sync_version: 1,
            needs_sync: true, // 保持需要同步状态
            sync_error: Some(error_msg.to_string()),
        };

        self.repository.update_sync_status(pool_address, &sync_status).await
    }

    /// 获取需要同步的池子列表
    pub async fn get_pools_need_sync(&self, limit: Option<i64>) -> AppResult<Vec<ClmmPool>> {
        self.repository.get_pools_need_sync(limit).await
    }

    /// 获取池子信息 (对外查询接口)
    pub async fn get_pool_by_address(&self, pool_address: &str) -> AppResult<Option<ClmmPool>> {
        self.repository.find_by_pool_address(pool_address).await
    }

    /// 根据代币地址查询池子列表
    pub async fn get_pools_by_mint(&self, mint_address: &str, limit: Option<i64>) -> AppResult<Vec<ClmmPool>> {
        self.repository.find_by_mint_address(mint_address, limit).await
    }

    /// 根据创建者查询池子列表
    pub async fn get_pools_by_creator(&self, creator_wallet: &str, limit: Option<i64>) -> AppResult<Vec<ClmmPool>> {
        self.repository.find_by_creator(creator_wallet, limit).await
    }

    /// 获取池子统计信息
    pub async fn get_pool_statistics(&self) -> AppResult<database::clmm_pool::PoolStats> {
        self.repository.get_pool_stats().await
    }

    /// 复杂查询接口
    pub async fn query_pools(&self, params: &database::clmm_pool::PoolQueryParams) -> AppResult<Vec<ClmmPool>> {
        self.repository.query_pools(params).await
    }

    /// 更新同步状态
    pub async fn update_sync_status(&self, pool_address: &str, sync_status: &SyncStatus) -> AppResult<bool> {
        self.repository.update_sync_status(pool_address, sync_status).await
    }

    /// 批量标记池子需要同步
    pub async fn mark_pools_for_sync(&self, pool_addresses: &[String]) -> AppResult<u64> {
        self.repository.mark_pools_for_sync(pool_addresses).await
    }
}

/// 存储服务构建器
pub struct ClmmPoolStorageBuilder;

impl ClmmPoolStorageBuilder {
    /// 从数据库实例创建存储服务
    pub fn from_database(db: &database::Database) -> ClmmPoolStorageService {
        ClmmPoolStorageService::new(db.clmm_pools.clone())
    }
}