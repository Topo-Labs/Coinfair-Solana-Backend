use crate::cpmm::swap_event::model::{PoolSwapStats, SwapEventModel, UserSwapStats};
use anyhow::Result;
use chrono::Utc;
use futures::stream::TryStreamExt;
use mongodb::{
    bson::{doc, oid::ObjectId, Document},
    options::{FindOptions, IndexOptions, InsertManyOptions},
    Collection, IndexModel,
};
use tracing::{debug, error, info, warn};

/// SwapEvent仓储接口
#[derive(Clone, Debug)]
pub struct SwapEventRepository {
    collection: Collection<SwapEventModel>,
}

impl SwapEventRepository {
    /// 创建新的SwapEvent仓储
    pub fn new(collection: Collection<SwapEventModel>) -> Self {
        Self { collection }
    }

    /// 初始化数据库索引
    pub async fn init_indexes(&self) -> Result<()> {
        info!("🔧 初始化SwapEvent集合索引...");

        let indexes = vec![
            // signature唯一索引（确保一个交易只记录一次）
            IndexModel::builder()
                .keys(doc! { "signature": 1 })
                .options(
                    IndexOptions::builder()
                        .unique(true)
                        .name("idx_signature_unique".to_string())
                        .build(),
                )
                .build(),
            // 用户交换历史查询索引
            IndexModel::builder()
                .keys(doc! {
                    "payer": 1,
                    "created_at": -1
                })
                .options(
                    IndexOptions::builder()
                        .name("idx_payer_created_at".to_string())
                        .build(),
                )
                .build(),
            // 池子交换历史查询索引
            IndexModel::builder()
                .keys(doc! {
                    "pool_id": 1,
                    "created_at": -1
                })
                .options(
                    IndexOptions::builder()
                        .name("idx_pool_id_created_at".to_string())
                        .build(),
                )
                .build(),
            // 输入代币查询索引
            IndexModel::builder()
                .keys(doc! { "input_mint": 1, "created_at": -1 })
                .options(
                    IndexOptions::builder()
                        .name("idx_input_mint_created_at".to_string())
                        .build(),
                )
                .build(),
            // 输出代币查询索引
            IndexModel::builder()
                .keys(doc! { "output_mint": 1, "created_at": -1 })
                .options(
                    IndexOptions::builder()
                        .name("idx_output_mint_created_at".to_string())
                        .build(),
                )
                .build(),
            // 代币对查询索引
            IndexModel::builder()
                .keys(doc! {
                    "input_mint": 1,
                    "output_mint": 1,
                    "created_at": -1
                })
                .options(
                    IndexOptions::builder()
                        .name("idx_token_pair_created_at".to_string())
                        .build(),
                )
                .build(),
            // 区块高度查询索引
            IndexModel::builder()
                .keys(doc! { "slot": -1 })
                .options(IndexOptions::builder().name("idx_slot".to_string()).build())
                .build(),
            // 时间范围查询索引
            IndexModel::builder()
                .keys(doc! { "created_at": -1 })
                .options(IndexOptions::builder().name("idx_created_at".to_string()).build())
                .build(),
            // 交换方向查询索引
            IndexModel::builder()
                .keys(doc! { "base_input": 1, "created_at": -1 })
                .options(
                    IndexOptions::builder()
                        .name("idx_base_input_created_at".to_string())
                        .build(),
                )
                .build(),
        ];

        match self.collection.create_indexes(indexes, None).await {
            Ok(_) => {
                info!("✅ SwapEvent索引创建成功");
                Ok(())
            }
            Err(e) => {
                error!("❌ SwapEvent索引创建失败: {}", e);
                Err(e.into())
            }
        }
    }

    /// 插入单个交换事件
    pub async fn insert(&self, mut event: SwapEventModel) -> Result<SwapEventModel> {
        event.created_at = Utc::now();

        // 验证数据
        if let Err(e) = event.validate() {
            error!("❌ SwapEvent数据验证失败: {}", e);
            return Err(anyhow::anyhow!("数据验证失败: {}", e));
        }

        match self.collection.insert_one(&event, None).await {
            Ok(result) => {
                info!(
                    "✅ 交换事件插入成功: signature={}, pool={}",
                    event.signature, event.pool_id
                );
                if let Some(id) = result.inserted_id.as_object_id() {
                    event.id = Some(id);
                }
                Ok(event)
            }
            Err(e) => {
                if e.to_string().contains("duplicate key") {
                    warn!("⚠️ 交换事件已存在，signature重复: {}", event.signature);
                    return Err(anyhow::anyhow!("交换事件已存在，signature重复: {}", event.signature));
                }
                error!("❌ 交换事件插入失败: {}", e);
                Err(e.into())
            }
        }
    }

    /// 根据ID查找交换事件
    pub async fn find_by_id(&self, id: &ObjectId) -> Result<Option<SwapEventModel>> {
        let filter = doc! { "_id": id };

        match self.collection.find_one(filter, None).await {
            Ok(result) => {
                if result.is_some() {
                    debug!("✅ 根据ID查找交换事件成功: {}", id);
                } else {
                    debug!("📭 根据ID未找到交换事件: {}", id);
                }
                Ok(result)
            }
            Err(e) => {
                error!("❌ 根据ID查找交换事件失败: {}", e);
                Err(e.into())
            }
        }
    }

    /// 根据signature查找交换事件
    pub async fn find_by_signature(&self, signature: &str) -> Result<Option<SwapEventModel>> {
        let filter = doc! { "signature": signature };

        match self.collection.find_one(filter, None).await {
            Ok(result) => {
                if result.is_some() {
                    debug!("✅ 根据signature查找交换事件成功: {}", signature);
                } else {
                    debug!("📭 根据signature未找到交换事件: {}", signature);
                }
                Ok(result)
            }
            Err(e) => {
                error!("❌ 根据signature查找交换事件失败: {}", e);
                Err(e.into())
            }
        }
    }

    /// 根据用户查找交换事件
    pub async fn find_by_payer(&self, payer: &str, limit: Option<i64>) -> Result<Vec<SwapEventModel>> {
        let filter = doc! { "payer": payer };
        let options = FindOptions::builder()
            .sort(doc! { "created_at": -1 })
            .limit(limit.unwrap_or(100))
            .build();

        match self.collection.find(filter, options).await {
            Ok(cursor) => {
                let events: Vec<SwapEventModel> = cursor.try_collect().await?;
                debug!("✅ 根据payer查找交换事件成功，查询到{}条记录", events.len());
                Ok(events)
            }
            Err(e) => {
                error!("❌ 根据payer查找交换事件失败: {}", e);
                Err(e.into())
            }
        }
    }

    /// 根据池子查找交换事件
    pub async fn find_by_pool(&self, pool_id: &str, limit: Option<i64>) -> Result<Vec<SwapEventModel>> {
        let filter = doc! { "pool_id": pool_id };
        let options = FindOptions::builder()
            .sort(doc! { "created_at": -1 })
            .limit(limit.unwrap_or(100))
            .build();

        match self.collection.find(filter, options).await {
            Ok(cursor) => {
                let events: Vec<SwapEventModel> = cursor.try_collect().await?;
                debug!("✅ 根据pool_id查找交换事件成功，查询到{}条记录", events.len());
                Ok(events)
            }
            Err(e) => {
                error!("❌ 根据pool_id查找交换事件失败: {}", e);
                Err(e.into())
            }
        }
    }

    /// 根据代币对查找交换事件
    pub async fn find_by_token_pair(&self, input_mint: &str, output_mint: &str, limit: Option<i64>) -> Result<Vec<SwapEventModel>> {
        let filter = doc! {
            "input_mint": input_mint,
            "output_mint": output_mint
        };
        let options = FindOptions::builder()
            .sort(doc! { "created_at": -1 })
            .limit(limit.unwrap_or(100))
            .build();

        match self.collection.find(filter, options).await {
            Ok(cursor) => {
                let events: Vec<SwapEventModel> = cursor.try_collect().await?;
                debug!(
                    "✅ 根据代币对查找交换事件成功，查询到{}条记录",
                    events.len()
                );
                Ok(events)
            }
            Err(e) => {
                error!("❌ 根据代币对查找交换事件失败: {}", e);
                Err(e.into())
            }
        }
    }

    /// 根据过滤条件查找交换事件
    pub async fn find_with_filter(&self, filter: Document, options: FindOptions) -> Result<Vec<SwapEventModel>> {
        let cursor = self.collection.find(filter.clone(), options).await?;
        let events: Vec<SwapEventModel> = cursor.try_collect().await?;

        debug!("✅ 带过滤条件查询交换事件成功，查询到{}条记录", events.len());
        Ok(events)
    }

    /// 统计交换事件数量
    pub async fn count_with_filter(&self, filter: Document) -> Result<u64> {
        match self.collection.count_documents(filter, None).await {
            Ok(count) => {
                debug!("✅ 统计交换事件数量成功: {}", count);
                Ok(count)
            }
            Err(e) => {
                error!("❌ 统计交换事件数量失败: {}", e);
                Err(e.into())
            }
        }
    }

    /// 批量插入交换事件
    pub async fn bulk_insert(&self, mut events: Vec<SwapEventModel>) -> Result<usize> {
        if events.is_empty() {
            return Ok(0);
        }

        let now = Utc::now();
        for event in &mut events {
            event.created_at = now;
            // 验证每个事件
            if let Err(e) = event.validate() {
                error!("❌ 批量插入时发现无效数据: {}", e);
                return Err(anyhow::anyhow!("批量插入时发现无效数据: {}", e));
            }
        }

        let options = InsertManyOptions::builder().ordered(false).build();

        match self.collection.insert_many(&events, Some(options)).await {
            Ok(result) => {
                let inserted_count = result.inserted_ids.len();
                info!("✅ 批量插入交换事件成功: {}", inserted_count);
                Ok(inserted_count)
            }
            Err(e) => {
                if e.to_string().contains("duplicate key") {
                    warn!("⚠️ 批量插入时部分事件重复，已跳过重复项");
                    // 在批量插入模式下，重复是预期的，返回0表示没有新插入
                    Ok(0)
                } else {
                    error!("❌ 批量插入交换事件失败: {}", e);
                    Err(e.into())
                }
            }
        }
    }

    /// 获取用户交换统计信息（使用聚合管道优化性能）
    pub async fn get_user_swap_stats(&self, payer: &str) -> Result<UserSwapStats> {
        debug!("📊 使用聚合管道获取用户交换统计: {}", payer);

        let pipeline = vec![
            // 第1步：筛选指定用户
            doc! {
                "$match": {
                    "payer": payer
                }
            },
            // 第2步：聚合统计数据
            doc! {
                "$group": {
                    "_id": null,
                    "total": { "$sum": 1 },
                    "total_input": { "$sum": "$input_amount" },
                    "total_output": { "$sum": "$output_amount" },
                    "total_fees": { "$sum": { "$add": ["$trade_fee", "$creator_fee"] } },
                    "first_swap": { "$min": "$created_at" },
                    "latest_swap": { "$max": "$created_at" }
                }
            },
        ];

        let mut cursor = self.collection.aggregate(pipeline, None).await?;

        if let Some(result) = cursor.try_next().await? {
            // 解析聚合结果
            let total = result
                .get_i32("total")
                .map(|v| v as u64)
                .or_else(|_| result.get_i64("total").map(|v| v as u64))
                .unwrap_or(0);

            let total_input = result
                .get_i32("total_input")
                .map(|v| v as u64)
                .or_else(|_| result.get_i64("total_input").map(|v| v as u64))
                .unwrap_or(0);

            let total_output = result
                .get_i32("total_output")
                .map(|v| v as u64)
                .or_else(|_| result.get_i64("total_output").map(|v| v as u64))
                .unwrap_or(0);

            let total_fees = result
                .get_i32("total_fees")
                .map(|v| v as u64)
                .or_else(|_| result.get_i64("total_fees").map(|v| v as u64))
                .unwrap_or(0);

            let first_swap = result
                .get_datetime("first_swap")
                .ok()
                .map(|dt| chrono::DateTime::from_timestamp_millis(dt.timestamp_millis()))
                .flatten();

            let latest_swap = result
                .get_datetime("latest_swap")
                .ok()
                .map(|dt| chrono::DateTime::from_timestamp_millis(dt.timestamp_millis()))
                .flatten();

            debug!(
                "✅ 用户交换统计查询成功: payer={}, swaps={}, input={}, output={}",
                payer, total, total_input, total_output
            );

            Ok(UserSwapStats {
                user_wallet: payer.to_string(),
                total_swaps: total,
                total_input_amount: total_input,
                total_output_amount: total_output,
                total_fees,
                first_swap_time: first_swap,
                latest_swap_time: latest_swap,
            })
        } else {
            debug!("📭 用户 {} 没有交换记录", payer);
            Ok(UserSwapStats {
                user_wallet: payer.to_string(),
                total_swaps: 0,
                total_input_amount: 0,
                total_output_amount: 0,
                total_fees: 0,
                first_swap_time: None,
                latest_swap_time: None,
            })
        }
    }

    /// 获取池子交换统计信息（使用聚合管道）
    pub async fn get_pool_swap_stats(&self, pool_id: &str) -> Result<PoolSwapStats> {
        debug!("📊 使用聚合管道获取池子交换统计: {}", pool_id);

        let pipeline = vec![
            // 第1步：筛选指定池子
            doc! {
                "$match": {
                    "pool_id": pool_id
                }
            },
            // 第2步：聚合统计数据
            doc! {
                "$group": {
                    "_id": null,
                    "total": { "$sum": 1 },
                    "total_volume_input": { "$sum": "$input_amount" },
                    "total_volume_output": { "$sum": "$output_amount" },
                    "total_fees": { "$sum": { "$add": ["$trade_fee", "$creator_fee"] } },
                    "unique_traders": { "$addToSet": "$payer" },
                    "first_swap": { "$min": "$created_at" },
                    "latest_swap": { "$max": "$created_at" }
                }
            },
        ];

        let mut cursor = self.collection.aggregate(pipeline, None).await?;

        if let Some(result) = cursor.try_next().await? {
            let total = result
                .get_i32("total")
                .map(|v| v as u64)
                .or_else(|_| result.get_i64("total").map(|v| v as u64))
                .unwrap_or(0);

            let total_volume_input = result
                .get_i32("total_volume_input")
                .map(|v| v as u64)
                .or_else(|_| result.get_i64("total_volume_input").map(|v| v as u64))
                .unwrap_or(0);

            let total_volume_output = result
                .get_i32("total_volume_output")
                .map(|v| v as u64)
                .or_else(|_| result.get_i64("total_volume_output").map(|v| v as u64))
                .unwrap_or(0);

            let total_fees = result
                .get_i32("total_fees")
                .map(|v| v as u64)
                .or_else(|_| result.get_i64("total_fees").map(|v| v as u64))
                .unwrap_or(0);

            let unique_traders = result.get_array("unique_traders").map(|arr| arr.len() as u64).unwrap_or(0);

            let first_swap = result
                .get_datetime("first_swap")
                .ok()
                .map(|dt| chrono::DateTime::from_timestamp_millis(dt.timestamp_millis()))
                .flatten();

            let latest_swap = result
                .get_datetime("latest_swap")
                .ok()
                .map(|dt| chrono::DateTime::from_timestamp_millis(dt.timestamp_millis()))
                .flatten();

            debug!(
                "✅ 池子交换统计查询成功: pool={}, swaps={}, traders={}",
                pool_id, total, unique_traders
            );

            Ok(PoolSwapStats {
                pool_id: pool_id.to_string(),
                total_swaps: total,
                total_volume_input,
                total_volume_output,
                total_fees_collected: total_fees,
                unique_traders,
                first_swap_time: first_swap,
                latest_swap_time: latest_swap,
            })
        } else {
            debug!("📭 池子 {} 没有交换记录", pool_id);
            Ok(PoolSwapStats {
                pool_id: pool_id.to_string(),
                total_swaps: 0,
                total_volume_input: 0,
                total_volume_output: 0,
                total_fees_collected: 0,
                unique_traders: 0,
                first_swap_time: None,
                latest_swap_time: None,
            })
        }
    }

    /// 根据ID删除交换事件
    pub async fn delete_by_id(&self, id: &ObjectId) -> Result<bool> {
        let filter = doc! { "_id": id };

        match self.collection.delete_one(filter, None).await {
            Ok(result) => {
                let deleted = result.deleted_count > 0;
                if deleted {
                    info!("✅ 删除交换事件成功: {}", id);
                } else {
                    warn!("⚠️ 交换事件不存在，无法删除: {}", id);
                }
                Ok(deleted)
            }
            Err(e) => {
                error!("❌ 删除交换事件失败: {}", e);
                Err(e.into())
            }
        }
    }
}
