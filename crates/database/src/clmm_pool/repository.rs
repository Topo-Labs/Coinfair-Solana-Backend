use super::model::*;
use mongodb::{
    bson::{doc, Document},
    options::{FindOptions, IndexOptions},
    Collection, IndexModel,
};
use tracing::info;
use utils::AppResult;

/// CLMM池子数据库操作接口
pub struct ClmmPoolRepository {
    collection: Collection<ClmmPool>,
}

impl ClmmPoolRepository {
    /// 创建新的仓库实例
    pub fn new(collection: Collection<ClmmPool>) -> Self {
        Self { collection }
    }

    /// 初始化数据库索引
    pub async fn init_indexes(&self) -> AppResult<()> {
        info!("🔧 初始化CLMM池子数据库索引...");

        let indexes = vec![
            // 池子地址唯一索引
            IndexModel::builder()
                .keys(doc! { "pool_address": 1 })
                .options(IndexOptions::builder().unique(true).build())
                .build(),
            
            // 代币mint地址复合索引
            IndexModel::builder()
                .keys(doc! { 
                    "mint0.mint_address": 1,
                    "mint1.mint_address": 1
                })
                .build(),
            
            // 创建者索引
            IndexModel::builder()
                .keys(doc! { "creator_wallet": 1 })
                .build(),
            
            // 状态索引
            IndexModel::builder()
                .keys(doc! { "status": 1 })
                .build(),
            
            // 价格范围索引
            IndexModel::builder()
                .keys(doc! { "price_info.initial_price": 1 })
                .build(),
            
            // 时间索引
            IndexModel::builder()
                .keys(doc! { "created_at": -1 })
                .build(),
            
            // 开放时间索引
            IndexModel::builder()
                .keys(doc! { "open_time": 1 })
                .build(),
            
            // 同步状态复合索引
            IndexModel::builder()
                .keys(doc! { 
                    "sync_status.needs_sync": 1,
                    "sync_status.last_sync_at": 1
                })
                .build(),
            
            // 交易签名索引 (稀疏索引)
            IndexModel::builder()
                .keys(doc! { "transaction_info.signature": 1 })
                .options(IndexOptions::builder().sparse(true).build())
                .build(),
        ];

        self.collection.create_indexes(indexes, None).await?;
        info!("✅ CLMM池子数据库索引初始化完成");
        Ok(())
    }

    /// 创建新池子记录
    pub async fn create_pool(&self, pool: &ClmmPool) -> AppResult<String> {
        let result = self.collection.insert_one(pool, None).await?;
        Ok(result.inserted_id.to_string())
    }

    /// 根据池子地址查询
    pub async fn find_by_pool_address(&self, pool_address: &str) -> AppResult<Option<ClmmPool>> {
        let filter = doc! { "pool_address": pool_address };
        Ok(self.collection.find_one(filter, None).await?)
    }

    /// 根据代币mint地址查询池子列表
    pub async fn find_by_mint_address(&self, mint_address: &str, limit: Option<i64>) -> AppResult<Vec<ClmmPool>> {
        let filter = doc! {
            "$or": [
                { "mint0.mint_address": mint_address },
                { "mint1.mint_address": mint_address }
            ]
        };
        
        let options = FindOptions::builder()
            .limit(limit.unwrap_or(50))
            .sort(doc! { "created_at": -1 })
            .build();
            
        let mut cursor = self.collection.find(filter, options).await?;
        let mut pools = Vec::new();
        
        while cursor.advance().await? {
            pools.push(cursor.deserialize_current()?);
        }
        
        Ok(pools)
    }

    /// 根据创建者查询池子列表
    pub async fn find_by_creator(&self, creator_wallet: &str, limit: Option<i64>) -> AppResult<Vec<ClmmPool>> {
        let filter = doc! { "creator_wallet": creator_wallet };
        
        let options = FindOptions::builder()
            .limit(limit.unwrap_or(50))
            .sort(doc! { "created_at": -1 })
            .build();
            
        let mut cursor = self.collection.find(filter, options).await?;
        let mut pools = Vec::new();
        
        while cursor.advance().await? {
            pools.push(cursor.deserialize_current()?);
        }
        
        Ok(pools)
    }

    /// 复杂查询接口
    pub async fn query_pools(&self, params: &PoolQueryParams) -> AppResult<Vec<ClmmPool>> {
        let mut filter = Document::new();
        
        // 构建查询条件
        if let Some(pool_address) = &params.pool_address {
            filter.insert("pool_address", pool_address);
        }
        
        if let Some(mint_address) = &params.mint_address {
            filter.insert("$or", vec![
                doc! { "mint0.mint_address": mint_address },
                doc! { "mint1.mint_address": mint_address }
            ]);
        }
        
        if let Some(creator_wallet) = &params.creator_wallet {
            filter.insert("creator_wallet", creator_wallet);
        }
        
        if let Some(status) = &params.status {
            filter.insert("status", mongodb::bson::to_bson(status)?);
        }
        
        // 价格范围查询
        if params.min_price.is_some() || params.max_price.is_some() {
            let mut price_filter = Document::new();
            if let Some(min_price) = params.min_price {
                price_filter.insert("$gte", min_price);
            }
            if let Some(max_price) = params.max_price {
                price_filter.insert("$lte", max_price);
            }
            filter.insert("price_info.initial_price", price_filter);
        }
        
        // 时间范围查询
        if params.start_time.is_some() || params.end_time.is_some() {
            let mut time_filter = Document::new();
            if let Some(start_time) = params.start_time {
                time_filter.insert("$gte", start_time as f64);
            }
            if let Some(end_time) = params.end_time {
                time_filter.insert("$lte", end_time as f64);
            }
            filter.insert("created_at", time_filter);
        }
        
        // 构建查询选项
        let mut options = FindOptions::default();
        
        // 分页
        if let Some(page) = params.page {
            let limit = params.limit.unwrap_or(20);
            let skip = (page - 1) * limit;
            options.skip = Some(skip);
            options.limit = Some(limit as i64);
        } else if let Some(limit) = params.limit {
            options.limit = Some(limit as i64);
        }
        
        // 排序
        let sort_field = params.sort_by.as_deref().unwrap_or("created_at");
        let sort_order = if params.sort_order.as_deref() == Some("asc") { 1 } else { -1 };
        options.sort = Some(doc! { sort_field: sort_order });
        
        // 执行查询
        let mut cursor = self.collection.find(filter, options).await?;
        let mut pools = Vec::new();
        
        while cursor.advance().await? {
            pools.push(cursor.deserialize_current()?);
        }
        
        Ok(pools)
    }

    /// 更新池子信息
    pub async fn update_pool(&self, pool_address: &str, update_doc: Document) -> AppResult<bool> {
        let filter = doc! { "pool_address": pool_address };
        let mut update = update_doc;
        update.insert("updated_at", chrono::Utc::now().timestamp() as f64);
        
        let update_doc = doc! { "$set": update };
        let result = self.collection.update_one(filter, update_doc, None).await?;
        
        Ok(result.modified_count > 0)
    }

    /// 更新交易信息
    pub async fn update_transaction_info(&self, pool_address: &str, tx_info: &TransactionInfo) -> AppResult<bool> {
        let filter = doc! { "pool_address": pool_address };
        let update = doc! {
            "$set": {
                "transaction_info": mongodb::bson::to_bson(tx_info)?,
                "status": "Active", // 交易确认后状态变为活跃
                "updated_at": chrono::Utc::now().timestamp() as f64
            }
        };
        
        let result = self.collection.update_one(filter, update, None).await?;
        Ok(result.modified_count > 0)
    }

    /// 更新同步状态
    pub async fn update_sync_status(&self, pool_address: &str, sync_status: &SyncStatus) -> AppResult<bool> {
        let filter = doc! { "pool_address": pool_address };
        let update = doc! {
            "$set": {
                "sync_status": mongodb::bson::to_bson(sync_status)?,
                "updated_at": chrono::Utc::now().timestamp() as f64
            }
        };
        
        let result = self.collection.update_one(filter, update, None).await?;
        Ok(result.modified_count > 0)
    }

    /// 批量更新需要同步的池子
    pub async fn mark_pools_for_sync(&self, pool_addresses: &[String]) -> AppResult<u64> {
        let filter = doc! {
            "pool_address": { "$in": pool_addresses }
        };
        
        let update = doc! {
            "$set": {
                "sync_status.needs_sync": true,
                "updated_at": chrono::Utc::now().timestamp() as f64
            }
        };
        
        let result = self.collection.update_many(filter, update, None).await?;
        Ok(result.modified_count)
    }

    /// 获取需要同步的池子列表
    pub async fn get_pools_need_sync(&self, limit: Option<i64>) -> AppResult<Vec<ClmmPool>> {
        let filter = doc! { "sync_status.needs_sync": true };
        let options = FindOptions::builder()
            .limit(limit.unwrap_or(100))
            .sort(doc! { "sync_status.last_sync_at": 1 })
            .build();
            
        let mut cursor = self.collection.find(filter, options).await?;
        let mut pools = Vec::new();
        
        while cursor.advance().await? {
            pools.push(cursor.deserialize_current()?);
        }
        
        Ok(pools)
    }

    /// 获取池子统计信息
    pub async fn get_pool_stats(&self) -> AppResult<PoolStats> {
        // 总池子数量
        let total_pools = self.collection.count_documents(doc! {}, None).await?;
        
        // 活跃池子数量
        let active_pools = self.collection.count_documents(
            doc! { "status": "Active" }, 
            None
        ).await?;
        
        // 今日新增池子数量
        let today_start = chrono::Utc::now().date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp() as f64;
        let today_new_pools = self.collection.count_documents(
            doc! { "created_at": { "$gte": today_start } },
            None
        ).await?;
        
        // 按状态分组统计 (使用聚合管道)
        let status_pipeline = vec![
            doc! {
                "$group": {
                    "_id": "$status",
                    "count": { "$sum": 1 }
                }
            }
        ];
        
        let mut status_cursor = self.collection.aggregate(status_pipeline, None).await?;
        let mut status_stats = Vec::new();
        
        while status_cursor.advance().await? {
            let doc = status_cursor.current();
            if let (Ok(status), Ok(count)) = (
                doc.get_str("_id"),
                doc.get_i64("count")
            ) {
                // 这里需要根据实际的状态字符串转换
                let pool_status = match status {
                    "Created" => PoolStatus::Created,
                    "Pending" => PoolStatus::Pending,
                    "Active" => PoolStatus::Active,
                    "Paused" => PoolStatus::Paused,
                    "Closed" => PoolStatus::Closed,
                    _ => continue,
                };
                
                status_stats.push(StatusStat {
                    status: pool_status,
                    count: count as u64,
                });
            }
        }
        
        // 按代币分组统计 (Top 10)
        let token_pipeline = vec![
            doc! {
                "$facet": {
                    "mint0": [
                        { "$group": { "_id": "$mint0.mint_address", "count": { "$sum": 1 } } }
                    ],
                    "mint1": [
                        { "$group": { "_id": "$mint1.mint_address", "count": { "$sum": 1 } } }
                    ]
                }
            },
            doc! {
                "$project": {
                    "combined": { "$concatArrays": ["$mint0", "$mint1"] }
                }
            },
            doc! { "$unwind": "$combined" },
            doc! {
                "$group": {
                    "_id": "$combined._id",
                    "pool_count": { "$sum": "$combined.count" }
                }
            },
            doc! { "$sort": { "pool_count": -1 } },
            doc! { "$limit": 10 }
        ];
        
        let mut token_cursor = self.collection.aggregate(token_pipeline, None).await?;
        let mut token_stats = Vec::new();
        
        while token_cursor.advance().await? {
            let doc = token_cursor.current();
            if let (Ok(mint_address), Ok(pool_count)) = (
                doc.get_str("_id"),
                doc.get_i64("pool_count")
            ) {
                token_stats.push(TokenStat {
                    mint_address: mint_address.to_string(),
                    symbol: None, // 可以后续通过代币信息服务补充
                    pool_count: pool_count as u64,
                });
            }
        }
        
        Ok(PoolStats {
            total_pools,
            active_pools,
            today_new_pools,
            status_stats,
            token_stats,
        })
    }

    /// 删除池子记录 (谨慎使用)
    pub async fn delete_pool(&self, pool_address: &str) -> AppResult<bool> {
        let filter = doc! { "pool_address": pool_address };
        let result = self.collection.delete_one(filter, None).await?;
        Ok(result.deleted_count > 0)
    }
}