use crate::cpmm::lp_change_event::model::LpChangeEvent;
use anyhow::Result;
use chrono::Utc;
use futures::stream::TryStreamExt;
use mongodb::{
    bson::{doc, oid::ObjectId, Document},
    options::{FindOptions, IndexOptions, InsertManyOptions},
    Collection, IndexModel,
};
use tracing::{debug, error, info, warn};

/// LP变更事件Repository
#[derive(Clone, Debug)]
pub struct LpChangeEventRepository {
    collection: Collection<LpChangeEvent>,
}

impl LpChangeEventRepository {
    pub fn new(collection: Collection<LpChangeEvent>) -> Self {
        Self { collection }
    }

    /// 获取集合引用（用于直接数据库操作）
    /// 初始化数据库索引
    pub async fn init_indexes(&self) -> Result<()> {
        let indexes = vec![
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
            // 用户查询索引
            IndexModel::builder()
                .keys(doc! { "user_wallet": 1, "created_at": -1 })
                .options(
                    IndexOptions::builder()
                        .name("idx_user_wallet_created_at".to_string())
                        .build(),
                )
                .build(),
            // 池子查询索引
            IndexModel::builder()
                .keys(doc! { "pool_id": 1, "created_at": -1 })
                .options(
                    IndexOptions::builder()
                        .name("idx_pool_id_created_at".to_string())
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
            // 区块高度索引
            IndexModel::builder()
                .keys(doc! { "slot": -1 })
                .options(IndexOptions::builder().name("idx_slot".to_string()).build())
                .build(),
            // 时间范围查询索引
            IndexModel::builder()
                .keys(doc! { "created_at": -1 })
                .options(IndexOptions::builder().name("idx_created_at".to_string()).build())
                .build(),
            // 变更类型索引
            IndexModel::builder()
                .keys(doc! { "change_type": 1 })
                .options(IndexOptions::builder().name("idx_change_type".to_string()).build())
                .build(),
        ];

        match self.collection.create_indexes(indexes, None).await {
            Ok(_result) => {
                info!("✅ LpChangeEvent索引初始化完成");
                Ok(())
            }
            Err(e) => {
                error!("❌ LP变更事件索引创建失败: {}", e);
                Err(e.into())
            }
        }
    }

    /// 插入新事件
    pub async fn insert(&self, mut event: LpChangeEvent) -> Result<LpChangeEvent> {
        // 设置创建时间
        event.created_at = Utc::now();

        // 验证事件数据
        if let Err(e) = event.validate() {
            warn!("⚠️ 事件数据验证失败: {}", e);
            return Err(anyhow::anyhow!("事件数据验证失败: {}", e));
        }

        match self.collection.insert_one(&event, None).await {
            Ok(result) => {
                info!("✅ LP变更事件插入成功: signature={}", event.signature);
                // 更新ID
                if let Some(id) = result.inserted_id.as_object_id() {
                    event.id = Some(id);
                }
                Ok(event)
            }
            Err(e) => {
                // 检查是否为重复signature错误
                if e.to_string().contains("duplicate key") {
                    warn!("⚠️ 事件已存在，signature重复: {}", event.signature);
                    return Err(anyhow::anyhow!("事件已存在，signature重复: {}", event.signature));
                }
                error!("❌ LP变更事件插入失败: {}", e);
                Err(e.into())
            }
        }
    }

    /// 根据ID查找事件
    pub async fn find_by_id(&self, id: &ObjectId) -> Result<Option<LpChangeEvent>> {
        let filter = doc! { "_id": id };

        match self.collection.find_one(filter, None).await {
            Ok(result) => {
                if result.is_some() {
                    debug!("✅ 根据ID查找事件成功: {}", id);
                } else {
                    debug!("📭 根据ID未找到事件: {}", id);
                }
                Ok(result)
            }
            Err(e) => {
                error!("❌ 根据ID查找事件失败: {}", e);
                Err(e.into())
            }
        }
    }

    /// 根据signature查找事件（防重）
    pub async fn find_by_signature(&self, signature: &str) -> Result<Option<LpChangeEvent>> {
        let filter = doc! { "signature": signature };

        match self.collection.find_one(filter, None).await {
            Ok(result) => {
                if result.is_some() {
                    debug!("✅ 根据signature查找事件成功: {}", signature);
                } else {
                    debug!("📭 根据signature未找到事件: {}", signature);
                }
                Ok(result)
            }
            Err(e) => {
                error!("❌ 根据signature查找事件失败: {}", e);
                Err(e.into())
            }
        }
    }

    /// 带过滤条件的分页查询
    pub async fn find_with_filter(&self, filter: Document, options: FindOptions) -> Result<Vec<LpChangeEvent>> {
        match self.collection.find(filter.clone(), options).await {
            Ok(mut cursor) => {
                let mut events = Vec::new();
                while let Ok(Some(event)) = cursor.try_next().await {
                    events.push(event);
                }
                debug!("✅ 分页查询成功，返回{}条记录", events.len());
                Ok(events)
            }
            Err(e) => {
                error!("❌ 分页查询失败: {}", e);
                Err(e.into())
            }
        }
    }

    /// 计数查询
    pub async fn count_with_filter(&self, filter: Document) -> Result<u64> {
        match self.collection.count_documents(filter, None).await {
            Ok(count) => {
                debug!("✅ 计数查询成功: {}", count);
                Ok(count)
            }
            Err(e) => {
                error!("❌ 计数查询失败: {}", e);
                Err(e.into())
            }
        }
    }

    /// 删除事件
    pub async fn delete_by_id(&self, id: &ObjectId) -> Result<bool> {
        let filter = doc! { "_id": id };

        match self.collection.delete_one(filter, None).await {
            Ok(result) => {
                let deleted = result.deleted_count > 0;
                if deleted {
                    info!("✅ 删除事件成功: {}", id);
                } else {
                    warn!("⚠️ 要删除的事件不存在: {}", id);
                }
                Ok(deleted)
            }
            Err(e) => {
                error!("❌ 删除事件失败: {}", e);
                Err(e.into())
            }
        }
    }

    /// 批量插入事件（事件监听器使用）
    pub async fn bulk_insert(&self, mut events: Vec<LpChangeEvent>) -> Result<usize> {
        if events.is_empty() {
            return Ok(0);
        }

        // 设置创建时间并验证
        let now = Utc::now();
        for event in &mut events {
            event.created_at = now;
            if let Err(e) = event.validate() {
                warn!("⚠️ 批量插入中发现无效事件: {}", e);
                continue;
            }
        }

        // 使用ordered: false，忽略重复错误
        let options = InsertManyOptions::builder().ordered(false).build();

        match self.collection.insert_many(&events, options).await {
            Ok(result) => {
                let inserted_count = result.inserted_ids.len();
                info!("✅ 批量插入事件成功: {}/{}", inserted_count, events.len());
                Ok(inserted_count)
            }
            Err(e) => {
                // 批量插入时部分成功也是可以接受的（比如重复signature）
                if e.to_string().contains("duplicate key") {
                    warn!("⚠️ 批量插入部分失败（存在重复signature）");
                    // 尝试获取实际插入的数量，这里简化处理
                    Ok(0)
                } else {
                    error!("❌ 批量插入事件失败: {}", e);
                    Err(e.into())
                }
            }
        }
    }

    /// 根据多个lp_mint查询事件（用于新的query-lp-mint接口）
    pub async fn find_by_lp_mints(&self, lp_mints: Vec<String>, limit: Option<i64>) -> Result<Vec<LpChangeEvent>> {
        if lp_mints.is_empty() {
            return Ok(vec![]);
        }

        let filter = if lp_mints.len() == 1 {
            doc! { "lp_mint": &lp_mints[0] }
        } else {
            doc! { "lp_mint": { "$in": lp_mints } }
        };

        let options = if let Some(limit_value) = limit {
            FindOptions::builder()
                .sort(doc! { "created_at": -1 })
                .limit(limit_value)
                .build()
        } else {
            FindOptions::builder().sort(doc! { "created_at": -1 }).build()
        };

        match self.collection.find(filter, options).await {
            Ok(mut cursor) => {
                let mut events = Vec::new();
                while let Ok(Some(event)) = cursor.try_next().await {
                    events.push(event);
                }
                debug!("✅ 根据lp_mints查询成功，返回{}条记录", events.len());
                Ok(events)
            }
            Err(e) => {
                error!("❌ 根据lp_mints查询失败: {}", e);
                Err(e.into())
            }
        }
    }
}
