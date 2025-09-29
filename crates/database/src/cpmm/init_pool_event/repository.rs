use crate::cpmm::init_pool_event::model::InitPoolEvent;
use crate::cpmm::init_pool_event::model::UserPoolStats;
use anyhow::Result;
use chrono::Utc;
use futures::stream::TryStreamExt;
use mongodb::{
    bson::{doc, oid::ObjectId, Document},
    options::{FindOptions, IndexOptions, InsertManyOptions},
    Collection, IndexModel,
};
// use serde::{Deserialize, Serialize};
use tracing::{debug, error, info, warn};

#[derive(Clone, Debug)]
pub struct InitPoolEventRepository {
    collection: Collection<InitPoolEvent>,
}

impl InitPoolEventRepository {
    pub fn new(collection: Collection<InitPoolEvent>) -> Self {
        Self { collection }
    }

    pub async fn init_indexes(&self) -> Result<()> {
        let indexes = vec![
            // pool_id唯一索引（确保一个池子只记录一次）
            IndexModel::builder()
                .keys(doc! { "pool_id": 1 })
                .options(
                    IndexOptions::builder()
                        .unique(true)
                        .name("idx_pool_id_unique".to_string())
                        .build(),
                )
                .build(),
            // signature唯一索引（防重）
            IndexModel::builder()
                .keys(doc! { "signature": 1 })
                .options(
                    IndexOptions::builder()
                        .unique(true)
                        .name("idx_signature_unique".to_string())
                        .build(),
                )
                .build(),
            // 用户创建的池子查询索引
            IndexModel::builder()
                .keys(doc! { "pool_creator": 1, "created_at": -1 })
                .options(
                    IndexOptions::builder()
                        .name("idx_pool_creator_created_at".to_string())
                        .build(),
                )
                .build(),
            // LP代币查询索引
            IndexModel::builder()
                .keys(doc! { "lp_mint": 1, "created_at": -1 })
                .options(
                    IndexOptions::builder()
                        .name("idx_lp_mint_created_at".to_string())
                        .build(),
                )
                .build(),
            // Token0查询索引
            IndexModel::builder()
                .keys(doc! { "token_0_mint": 1, "created_at": -1 })
                .options(
                    IndexOptions::builder()
                        .name("idx_token_0_mint_created_at".to_string())
                        .build(),
                )
                .build(),
            // Token1查询索引
            IndexModel::builder()
                .keys(doc! { "token_1_mint": 1, "created_at": -1 })
                .options(
                    IndexOptions::builder()
                        .name("idx_token_1_mint_created_at".to_string())
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
        ];

        match self.collection.create_indexes(indexes, None).await {
            Ok(_) => {
                info!("✅ InitPoolEvent索引创建成功");
                Ok(())
            }
            Err(e) => {
                error!("❌ InitPoolEvent索引创建失败: {}", e);
                Err(e.into())
            }
        }
    }

    pub async fn insert(&self, mut event: InitPoolEvent) -> Result<InitPoolEvent> {
        event.created_at = Utc::now();

        match self.collection.insert_one(&event, None).await {
            Ok(result) => {
                info!("✅ 池子初始化事件插入成功: pool_id={}", event.pool_id);
                if let Some(id) = result.inserted_id.as_object_id() {
                    event.id = Some(id);
                }
                Ok(event)
            }
            Err(e) => {
                if e.to_string().contains("duplicate key") {
                    if e.to_string().contains("idx_pool_id_unique") {
                        warn!("⚠️ 池子已存在，pool_id重复: {}", event.pool_id);
                        return Err(anyhow::anyhow!("池子已存在，pool_id重复: {}", event.pool_id));
                    } else if e.to_string().contains("idx_signature_unique") {
                        warn!("⚠️ 事件已存在，signature重复: {}", event.signature);
                        return Err(anyhow::anyhow!("事件已存在，signature重复: {}", event.signature));
                    }
                }
                error!("❌ 池子初始化事件插入失败: {}", e);
                Err(e.into())
            }
        }
    }

    pub async fn find_by_id(&self, id: &ObjectId) -> Result<Option<InitPoolEvent>> {
        let filter = doc! { "_id": id };

        match self.collection.find_one(filter, None).await {
            Ok(result) => {
                if result.is_some() {
                    debug!("✅ 根据ID查找池子初始化事件成功: {}", id);
                } else {
                    debug!("📭 根据ID未找到池子初始化事件: {}", id);
                }
                Ok(result)
            }
            Err(e) => {
                error!("❌ 根据ID查找池子初始化事件失败: {}", e);
                Err(e.into())
            }
        }
    }

    pub async fn find_by_pool_id(&self, pool_id: &str) -> Result<Option<InitPoolEvent>> {
        let filter = doc! { "pool_id": pool_id };

        match self.collection.find_one(filter, None).await {
            Ok(result) => {
                if result.is_some() {
                    debug!("✅ 根据pool_id查找池子初始化事件成功: {}", pool_id);
                } else {
                    debug!("📭 根据pool_id未找到池子初始化事件: {}", pool_id);
                }
                Ok(result)
            }
            Err(e) => {
                error!("❌ 根据pool_id查找池子初始化事件失败: {}", e);
                Err(e.into())
            }
        }
    }

    pub async fn find_by_signature(&self, signature: &str) -> Result<Option<InitPoolEvent>> {
        let filter = doc! { "signature": signature };

        match self.collection.find_one(filter, None).await {
            Ok(result) => {
                if result.is_some() {
                    debug!("✅ 根据signature查找池子初始化事件成功: {}", signature);
                } else {
                    debug!("📭 根据signature未找到池子初始化事件: {}", signature);
                }
                Ok(result)
            }
            Err(e) => {
                error!("❌ 根据signature查找池子初始化事件失败: {}", e);
                Err(e.into())
            }
        }
    }

    pub async fn find_with_filter(&self, filter: Document, options: FindOptions) -> Result<Vec<InitPoolEvent>> {
        let cursor = self.collection.find(filter.clone(), options).await?;
        let events: Vec<InitPoolEvent> = cursor.try_collect().await?;

        debug!("✅ 带过滤条件查询池子初始化事件成功，查询到{}条记录", events.len());
        Ok(events)
    }

    pub async fn count_with_filter(&self, filter: Document) -> Result<u64> {
        match self.collection.count_documents(filter, None).await {
            Ok(count) => {
                debug!("✅ 统计池子初始化事件数量成功: {}", count);
                Ok(count)
            }
            Err(e) => {
                error!("❌ 统计池子初始化事件数量失败: {}", e);
                Err(e.into())
            }
        }
    }

    pub async fn delete_by_id(&self, id: &ObjectId) -> Result<bool> {
        let filter = doc! { "_id": id };

        match self.collection.delete_one(filter, None).await {
            Ok(result) => {
                let deleted = result.deleted_count > 0;
                if deleted {
                    info!("✅ 删除池子初始化事件成功: {}", id);
                } else {
                    warn!("⚠️ 池子初始化事件不存在，无法删除: {}", id);
                }
                Ok(deleted)
            }
            Err(e) => {
                error!("❌ 删除池子初始化事件失败: {}", e);
                Err(e.into())
            }
        }
    }

    pub async fn bulk_insert(&self, mut events: Vec<InitPoolEvent>) -> Result<usize> {
        if events.is_empty() {
            return Ok(0);
        }

        let now = Utc::now();
        for event in &mut events {
            event.created_at = now;
        }

        let options = InsertManyOptions::builder().ordered(false).build();

        match self.collection.insert_many(&events, Some(options)).await {
            Ok(result) => {
                let inserted_count = result.inserted_ids.len();
                info!("✅ 批量插入池子初始化事件成功: {}", inserted_count);
                Ok(inserted_count)
            }
            Err(e) => {
                if e.to_string().contains("duplicate key") {
                    warn!("⚠️ 批量插入时部分事件重复，已跳过重复项");
                    // 在批量插入模式下，重复是预期的，返回0表示没有新插入
                    Ok(0)
                } else {
                    error!("❌ 批量插入池子初始化事件失败: {}", e);
                    Err(e.into())
                }
            }
        }
    }

    /// 获取用户池子创建统计（使用聚合管道优化性能）
    pub async fn get_user_pool_stats(&self, pool_creator: &str) -> Result<UserPoolStats> {
        debug!("📊 使用聚合管道获取用户池子创建统计: {}", pool_creator);

        // 构建聚合管道
        let pipeline = vec![
            // 第1步：筛选指定用户的池子
            doc! {
                "$match": {
                    "pool_creator": pool_creator
                }
            },
            // 第2步：聚合统计数据
            doc! {
                "$group": {
                    "_id": null,
                    "total": { "$sum": 1 },
                    "first_created_at": { "$min": "$created_at" },
                    "latest_created_at": { "$max": "$created_at" }
                }
            },
        ];

        // 执行聚合查询
        let mut cursor = self.collection.aggregate(pipeline, None).await?;

        // 获取结果
        if let Some(result) = cursor.try_next().await? {
            // 解析聚合结果
            // MongoDB $sum 可能返回 i32 或 i64，先尝试 i32
            let total = result
                .get_i32("total")
                .map(|v| v as u64)
                .or_else(|_| result.get_i64("total").map(|v| v as u64))
                .unwrap_or(0);

            // created_at 在数据库中是字符串格式，不是DateTime类型
            let first_created_at = result.get_str("first_created_at").ok().map(|s| s.to_string());

            let latest_created_at = result.get_str("latest_created_at").ok().map(|s| s.to_string());

            debug!(
                "✅ 聚合查询成功: total={}, first={:?}, latest={:?}",
                total, first_created_at, latest_created_at
            );

            Ok(UserPoolStats {
                total_pools_created: total,
                first_pool_created_at: first_created_at,
                latest_pool_created_at: latest_created_at,
            })
        } else {
            // 没有数据时返回空统计
            debug!("📭 用户 {} 没有创建任何池子", pool_creator);
            Ok(UserPoolStats {
                total_pools_created: 0,
                first_pool_created_at: None,
                latest_pool_created_at: None,
            })
        }
    }
}
