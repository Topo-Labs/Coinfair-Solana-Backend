use crate::error::{EventListenerError, Result};
use database::event_scanner::model::EventScannerCheckpoints;
use mongodb::{
    bson::{doc, from_document, to_document},
    options::UpdateOptions,
    Collection, Database,
};
use std::sync::Arc;
use tracing::{debug, error, info, warn};

/// 检查点持久化服务
///
/// 负责管理EventScannerCheckpoints的CRUD操作
/// 索引管理由database层的EventScannerCheckpointRepository处理
pub struct CheckpointPersistence {
    collection: Collection<mongodb::bson::Document>,
}

impl CheckpointPersistence {
    /// 创建新的检查点持久化服务
    pub async fn new(database: Arc<Database>) -> Result<Self> {
        let collection = database.collection("EventScannerCheckpoints");
        Ok(Self { collection })
    }

    /// 获取检查点
    pub async fn get_checkpoint(&self) -> Result<Option<EventScannerCheckpoints>> {
        // 查询最新的backfill检查点
        let filter = doc! {
            "event_name": "backfill"
        };

        let sort = doc! { "updated_at": -1 };

        let options = mongodb::options::FindOneOptions::builder().sort(sort).build();

        match self.collection.find_one(filter, options).await {
            Ok(Some(doc)) => {
                let checkpoint: EventScannerCheckpoints =
                    from_document(doc).map_err(|e| EventListenerError::Database(mongodb::error::Error::from(e)))?;

                debug!("📍 获取检查点成功: {:?}", checkpoint.last_signature);
                Ok(Some(checkpoint))
            }
            Ok(None) => {
                debug!("📍 没有找到检查点");
                Ok(None)
            }
            Err(e) => {
                error!("❌ 获取检查点失败: {}", e);
                Err(EventListenerError::Database(mongodb::error::Error::from(e)))
            }
        }
    }

    /// 根据程序ID和事件名获取检查点
    pub async fn get_checkpoint_by_program_and_event_name(
        &self,
        program_id: &str,
        event_name: &str,
    ) -> Result<Option<EventScannerCheckpoints>> {
        let filter = doc! {
            "program_id": program_id,
            "event_name": event_name
        };

        let sort = doc! { "updated_at": -1 };

        let options = mongodb::options::FindOneOptions::builder().sort(sort).build();

        match self.collection.find_one(filter, options).await {
            Ok(Some(doc)) => {
                let checkpoint: EventScannerCheckpoints =
                    from_document(doc).map_err(|e| EventListenerError::Database(mongodb::error::Error::from(e)))?;

                debug!(
                    "📍 获取检查点成功 {}[{}]: {:?}",
                    program_id, event_name, checkpoint.last_signature
                );
                Ok(Some(checkpoint))
            }
            Ok(None) => {
                debug!("📍 没有找到检查点: {}[{}]", program_id, event_name);
                Ok(None)
            }
            Err(e) => {
                error!("❌ 获取检查点失败: {}", e);
                Err(EventListenerError::Database(mongodb::error::Error::from(e)))
            }
        }
    }

    /// 更新或创建检查点
    pub async fn update_checkpoint(&self, checkpoint: &EventScannerCheckpoints) -> Result<()> {
        let mut doc =
            to_document(checkpoint).map_err(|e| EventListenerError::Unknown(format!("序列化检查点失败: {}", e)))?;

        let filter = match (&checkpoint.program_id, &checkpoint.event_name) {
            (Some(program_id), Some(event_name)) => doc! {
                "program_id": program_id,
                "event_name": event_name
            },
            _ => doc! {
                "event_name": "backfill"
            },
        };

        // 从$set文档中移除created_at字段，避免与$setOnInsert冲突
        doc.remove("created_at");

        let update = doc! {
            "$set": doc,
            "$setOnInsert": {
                "created_at": mongodb::bson::DateTime::now()
            }
        };

        let options = UpdateOptions::builder().upsert(true).build();

        match self.collection.update_one(filter, update, Some(options)).await {
            Ok(result) => {
                if result.upserted_id.is_some() {
                    info!("✅ 创建新检查点: {:?}", checkpoint.last_signature);
                } else {
                    info!("✅ 更新检查点: {:?}", checkpoint.last_signature);
                }
                Ok(())
            }
            Err(e) => {
                error!("❌ 更新检查点失败: {}", e);
                Err(EventListenerError::Unknown(format!("更新检查点失败: {}", e)))
            }
        }
    }

    /// 删除检查点
    pub async fn delete_checkpoint(&self, program_id: Option<&str>, event_name: Option<&str>) -> Result<bool> {
        let filter = match (program_id, event_name) {
            (Some(pid), Some(name)) => doc! {
                "program_id": pid,
                "event_name": name
            },
            (Some(pid), None) => doc! {
                "program_id": pid
            },
            (None, Some(name)) => doc! {
                "event_name": name
            },
            (None, None) => doc! {
                "event_name": "backfill"
            },
        };

        match self.collection.delete_one(filter, None).await {
            Ok(result) => {
                let deleted = result.deleted_count > 0;
                if deleted {
                    info!("🗑️ 删除检查点成功");
                } else {
                    warn!("⚠️ 没有找到要删除的检查点");
                }
                Ok(deleted)
            }
            Err(e) => {
                error!("❌ 删除检查点失败: {}", e);
                Err(EventListenerError::Unknown(format!("删除检查点失败: {}", e)))
            }
        }
    }

    /// 列出所有检查点
    pub async fn list_checkpoints(&self) -> Result<Vec<EventScannerCheckpoints>> {
        let sort = doc! { "updated_at": -1 };

        let options = mongodb::options::FindOptions::builder().sort(sort).build();

        let mut cursor = self
            .collection
            .find(doc! {}, Some(options))
            .await
            .map_err(|e| EventListenerError::Unknown(format!("查询检查点失败: {}", e)))?;

        let mut checkpoints = Vec::new();

        while cursor
            .advance()
            .await
            .map_err(|e| EventListenerError::Unknown(format!("遍历检查点失败: {}", e)))?
        {
            let doc: mongodb::bson::Document = cursor
                .current()
                .try_into()
                .map_err(|e| EventListenerError::Unknown(format!("MongoDB文档转换失败: {}", e)))?;
            let checkpoint: EventScannerCheckpoints =
                from_document(doc).map_err(|e| EventListenerError::Database(mongodb::error::Error::from(e)))?;
            checkpoints.push(checkpoint);
        }

        debug!("📋 查询到 {} 个检查点", checkpoints.len());
        Ok(checkpoints)
    }

    /// 健康检查
    pub async fn is_healthy(&self) -> bool {
        match self.collection.find_one(doc! {}, None).await {
            Ok(_) => true,
            Err(e) => {
                error!("❌ 检查点持久化服务健康检查失败: {}", e);
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    async fn create_test_persistence() -> CheckpointPersistence {
        // 这里需要真实的MongoDB连接用于集成测试
        // 在单元测试中，应该使用mock
        todo!("需要MongoDB测试环境")
    }

    #[tokio::test]
    #[ignore] // 需要MongoDB连接
    async fn test_checkpoint_crud() {
        let persistence = create_test_persistence().await;

        // 创建测试检查点
        let checkpoint = EventScannerCheckpoints {
            id: None,
            program_id: Some("test_program".to_string()),
            event_name: Some("test_event".to_string()),
            slot: Some(123456),
            last_signature: Some("test_signature".to_string()),
            updated_at: Utc::now(),
            created_at: Utc::now(),
        };

        // 测试创建
        persistence.update_checkpoint(&checkpoint).await.unwrap();

        // 测试查询
        let retrieved = persistence
            .get_checkpoint_by_program_and_event_name("test_program", "test_event")
            .await
            .unwrap();

        assert!(retrieved.is_some());
        let retrieved_checkpoint = retrieved.unwrap();
        assert_eq!(retrieved_checkpoint.program_id, checkpoint.program_id);
        assert_eq!(retrieved_checkpoint.last_signature, checkpoint.last_signature);

        // 测试删除
        let deleted = persistence
            .delete_checkpoint(Some("test_program"), Some("test_event"))
            .await
            .unwrap();

        assert!(deleted);
    }
}
