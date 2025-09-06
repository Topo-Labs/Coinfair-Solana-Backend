use super::model::*;
use chrono::Utc;
use futures_util::TryStreamExt;
use mongodb::{
    bson::{doc, to_bson, DateTime as BsonDateTime},
    options::{FindOptions, IndexOptions, UpdateOptions},
    Collection, IndexModel,
};
use tracing::info;
use utils::AppResult;
use uuid::Uuid;

/// 事件扫描器检查点仓库
#[derive(Clone, Debug)]
pub struct EventScannerCheckpointRepository {
    collection: Collection<EventScannerCheckpoints>,
}

impl EventScannerCheckpointRepository {
    /// 创建新的仓库实例
    pub fn new(collection: Collection<EventScannerCheckpoints>) -> Self {
        Self { collection }
    }

    /// 获取集合引用（用于直接数据库操作）
    pub fn get_collection(&self) -> &Collection<EventScannerCheckpoints> {
        &self.collection
    }

    /// 初始化数据库索引
    pub async fn init_indexes(&self) -> AppResult<()> {
        let indexes = vec![
            // 程序ID + 事件名称复合唯一索引
            IndexModel::builder()
                .keys(doc! {
                    "program_id": 1,
                    "event_name": 1
                })
                .options(IndexOptions::builder().unique(true).build())
                .build(),
            // 程序ID索引 (查询特定程序的检查点)
            IndexModel::builder().keys(doc! { "program_id": 1 }).build(),
            // 事件名称索引 (查询特定事件的检查点)
            IndexModel::builder().keys(doc! { "event_name": 1 }).build(),
            // 更新时间索引 (按时间排序)
            IndexModel::builder().keys(doc! { "updated_at": -1 }).build(),
            // 槽位索引 (用于查询范围)
            IndexModel::builder().keys(doc! { "slot": -1 }).build(),
        ];

        self.collection.create_indexes(indexes, None).await?;
        info!("✅ EventScannerCheckpoints数据库索引初始化完成");
        Ok(())
    }

    /// 创建或更新检查点
    pub async fn upsert_checkpoint(&self, mut checkpoint: EventScannerCheckpoints) -> AppResult<String> {
        checkpoint.updated_at = Utc::now();

        let filter = doc! {
            "program_id": &checkpoint.program_id,
            "event_name": &checkpoint.event_name
        };

        let updated_at_bson = BsonDateTime::from_millis(checkpoint.updated_at.timestamp_millis());

        let update_doc = doc! {
            "$set": {
                "slot": to_bson(&checkpoint.slot)?,
                "last_signature": to_bson(&checkpoint.last_signature)?,
                "updated_at": updated_at_bson
            },
            "$setOnInsert": {
                "created_at": updated_at_bson
            }
        };

        let options = UpdateOptions::builder().upsert(true).build();
        let result = self.collection.update_one(filter, update_doc, options).await?;

        if let Some(upserted_id) = result.upserted_id {
            Ok(upserted_id.as_object_id().unwrap().to_hex())
        } else {
            // 更新情况，查找对应记录的ID
            let filter = doc! {
                "program_id": &checkpoint.program_id,
                "event_name": &checkpoint.event_name
            };
            if let Some(found) = self.collection.find_one(filter, None).await? {
                Ok(found.id.unwrap().to_hex())
            } else {
                Err(anyhow::anyhow!("无法获取检查点记录ID").into())
            }
        }
    }

    /// 根据程序ID和事件名称查找检查点
    pub async fn find_checkpoint(
        &self,
        program_id: &str,
        event_name: &str,
    ) -> AppResult<Option<EventScannerCheckpoints>> {
        let filter = doc! {
            "program_id": program_id,
            "event_name": event_name
        };
        let result = self.collection.find_one(filter, None).await?;
        Ok(result)
    }

    /// 根据程序ID查找所有检查点
    pub async fn find_checkpoints_by_program(&self, program_id: &str) -> AppResult<Vec<EventScannerCheckpoints>> {
        let filter = doc! { "program_id": program_id };
        let cursor = self.collection.find(filter, None).await?;
        let checkpoints: Vec<EventScannerCheckpoints> = cursor.try_collect().await?;
        Ok(checkpoints)
    }

    /// 查找所有检查点（按更新时间倒序）
    pub async fn find_all_checkpoints(&self) -> AppResult<Vec<EventScannerCheckpoints>> {
        let options = FindOptions::builder().sort(doc! { "updated_at": -1 }).build();
        let cursor = self.collection.find(None, options).await?;
        let checkpoints: Vec<EventScannerCheckpoints> = cursor.try_collect().await?;
        Ok(checkpoints)
    }

    /// 删除特定检查点
    pub async fn delete_checkpoint(&self, program_id: &str, event_name: &str) -> AppResult<bool> {
        let filter = doc! {
            "program_id": program_id,
            "event_name": event_name
        };
        let result = self.collection.delete_one(filter, None).await?;
        Ok(result.deleted_count > 0)
    }
}

/// 扫描记录仓库
#[derive(Clone, Debug)]
pub struct ScanRecordRepository {
    collection: Collection<ScanRecords>,
}

impl ScanRecordRepository {
    /// 创建新的仓库实例
    pub fn new(collection: Collection<ScanRecords>) -> Self {
        Self { collection }
    }

    /// 获取集合引用（用于直接数据库操作）
    pub fn get_collection(&self) -> &Collection<ScanRecords> {
        &self.collection
    }

    /// 初始化数据库索引
    pub async fn init_indexes(&self) -> AppResult<()> {
        let indexes = vec![
            // 扫描ID唯一索引
            IndexModel::builder()
                .keys(doc! { "scan_id": 1 })
                .options(IndexOptions::builder().unique(true).build())
                .build(),
            // 扫描状态索引
            IndexModel::builder().keys(doc! { "status": 1 }).build(),
            // 开始时间索引 (按时间排序)
            IndexModel::builder().keys(doc! { "started_at": -1 }).build(),
            // 完成时间索引
            IndexModel::builder().keys(doc! { "completed_at": -1 }).build(),
            // 起始槽位索引
            IndexModel::builder().keys(doc! { "until_slot": -1 }).build(),
            // 结束槽位索引
            IndexModel::builder().keys(doc! { "before_slot": -1 }).build(),
            // 程序过滤器索引
            IndexModel::builder().keys(doc! { "program_filters": 1 }).build(),
            // 复合索引：状态 + 开始时间
            IndexModel::builder()
                .keys(doc! {
                    "status": 1,
                    "started_at": -1
                })
                .build(),
        ];

        self.collection.create_indexes(indexes, None).await?;
        info!("✅ ScanRecords数据库索引初始化完成");
        Ok(())
    }

    /// 创建扫描记录
    ///
    /// 自动生成UUID作为scan_id，设置开始时间为当前时间
    ///
    /// # 参数
    /// - `record`: 扫描记录（scan_id字段会被自动生成的UUID覆盖）
    ///
    /// # 返回
    /// - 生成的scan_id (UUID字符串)
    pub async fn create_scan_record(&self, mut record: ScanRecords) -> AppResult<String> {
        // 自动生成UUID作为scan_id
        let scan_id = Uuid::new_v4().to_string();
        record.scan_id = scan_id.clone();

        // 设置开始时间为当前时间
        record.started_at = Utc::now();

        let _result = self.collection.insert_one(record, None).await?;

        // 返回生成的scan_id而不是MongoDB的ObjectId
        Ok(scan_id)
    }

    /// 创建新的扫描记录（便利方法）
    ///
    /// 提供一个简化的接口来创建扫描记录，只需要提供核心参数
    ///
    /// # 参数
    /// - `until_slot`: 起始槽位
    /// - `before_slot`: 结束槽位  
    /// - `until_signature`: 起始签名
    /// - `before_signature`: 结束签名
    /// - `program_filters`: 程序过滤器列表
    ///
    /// # 返回
    /// - 生成的scan_id (UUID字符串)
    pub async fn create_new_scan(
        &self,
        until_slot: Option<u64>,
        before_slot: Option<u64>,
        until_signature: String,
        before_signature: String,
        program_filters: Vec<String>,
    ) -> AppResult<String> {
        let record = ScanRecords {
            id: None,
            scan_id: String::new(), // 将被自动生成
            until_slot,
            before_slot,
            until_signature,
            before_signature,
            status: ScanStatus::Running,
            events_found: 0,
            events_backfilled_count: 0,
            events_backfilled_signatures: vec![],
            started_at: Utc::now(), // 将被Repository重写
            completed_at: None,
            error_message: None,
            program_filters,
        };

        self.create_scan_record(record).await
    }

    /// 根据扫描ID查找记录
    pub async fn find_by_scan_id(&self, scan_id: &str) -> AppResult<Option<ScanRecords>> {
        let filter = doc! { "scan_id": scan_id };
        let result = self.collection.find_one(filter, None).await?;
        Ok(result)
    }

    /// 更新扫描状态
    pub async fn update_scan_status(
        &self,
        scan_id: &str,
        status: ScanStatus,
        error_message: Option<&str>,
    ) -> AppResult<bool> {
        let now = Utc::now();
        let now_bson = BsonDateTime::from_millis(now.timestamp_millis());

        let mut update_doc = doc! {
            "status": to_bson(&status)?,
            "updated_at": now_bson
        };

        // 如果是完成或失败状态，设置完成时间
        if matches!(
            status,
            ScanStatus::Completed | ScanStatus::Failed | ScanStatus::Cancelled
        ) {
            update_doc.insert("completed_at", now_bson);
        }

        // 如果有错误信息，添加到更新文档中
        if let Some(error) = error_message {
            update_doc.insert("error_message", error);
        }

        let filter = doc! { "scan_id": scan_id };
        let update = doc! { "$set": update_doc };

        let result = self.collection.update_one(filter, update, None).await?;
        Ok(result.modified_count > 0)
    }

    /// 更新事件统计信息
    pub async fn update_event_stats(
        &self,
        scan_id: &str,
        events_found: u64,
        events_backfilled_count: u64,
        events_backfilled_signatures: Vec<String>,
    ) -> AppResult<bool> {
        let filter = doc! { "scan_id": scan_id };
        let now_bson = BsonDateTime::from_millis(Utc::now().timestamp_millis());

        let update = doc! {
            "$set": {
                "events_found": events_found as i64,
                "events_backfilled_count": events_backfilled_count as i64,
                "events_backfilled_signatures": events_backfilled_signatures,
                "updated_at": now_bson
            }
        };

        let result = self.collection.update_one(filter, update, None).await?;
        Ok(result.modified_count > 0)
    }

    /// 查找指定状态的扫描记录
    pub async fn find_by_status(&self, status: ScanStatus) -> AppResult<Vec<ScanRecords>> {
        let filter = doc! { "status": to_bson(&status)? };
        let options = FindOptions::builder().sort(doc! { "started_at": -1 }).build();
        let cursor = self.collection.find(filter, options).await?;
        let records: Vec<ScanRecords> = cursor.try_collect().await?;
        Ok(records)
    }

    /// 查找运行中的扫描记录
    pub async fn find_running_scans(&self) -> AppResult<Vec<ScanRecords>> {
        self.find_by_status(ScanStatus::Running).await
    }

    /// 查找已完成的扫描记录
    pub async fn find_completed_scans(&self) -> AppResult<Vec<ScanRecords>> {
        self.find_by_status(ScanStatus::Completed).await
    }

    /// 查找失败的扫描记录
    pub async fn find_failed_scans(&self) -> AppResult<Vec<ScanRecords>> {
        self.find_by_status(ScanStatus::Failed).await
    }

    /// 分页查询扫描记录（按开始时间倒序）
    pub async fn find_records_paginated(&self, skip: u64, limit: i64) -> AppResult<Vec<ScanRecords>> {
        let options = FindOptions::builder()
            .sort(doc! { "started_at": -1 })
            .skip(skip)
            .limit(limit)
            .build();

        let cursor = self.collection.find(None, options).await?;
        let records: Vec<ScanRecords> = cursor.try_collect().await?;
        Ok(records)
    }

    /// 统计扫描记录总数
    pub async fn count_total_records(&self) -> AppResult<u64> {
        let count = self.collection.count_documents(None, None).await?;
        Ok(count)
    }

    /// 统计特定状态的记录数量
    pub async fn count_records_by_status(&self, status: ScanStatus) -> AppResult<u64> {
        let filter = doc! { "status": to_bson(&status)? };
        let count = self.collection.count_documents(filter, None).await?;
        Ok(count)
    }

    /// 删除扫描记录
    pub async fn delete_scan_record(&self, scan_id: &str) -> AppResult<bool> {
        let filter = doc! { "scan_id": scan_id };
        let result = self.collection.delete_one(filter, None).await?;
        Ok(result.deleted_count > 0)
    }

    /// 清理指定天数之前的已完成记录
    pub async fn cleanup_old_completed_records(&self, days_ago: i64) -> AppResult<u64> {
        let cutoff_time = Utc::now() - chrono::Duration::days(days_ago);

        let cutoff_bson = BsonDateTime::from_millis(cutoff_time.timestamp_millis());

        let filter = doc! {
            "status": to_bson(&ScanStatus::Completed)?,
            "completed_at": {
                "$lt": cutoff_bson
            }
        };

        let result = self.collection.delete_many(filter, None).await?;
        Ok(result.deleted_count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mongodb::Client;

    async fn setup_test_collections() -> (EventScannerCheckpointRepository, ScanRecordRepository) {
        let client = Client::with_uri_str("mongodb://localhost:27017").await.unwrap();
        let db = client.database("coinfair_test");

        let checkpoint_collection = db.collection::<EventScannerCheckpoints>("EventScannerCheckpoints");
        let scan_record_collection = db.collection::<ScanRecords>("ScanRecords");

        // 清理测试数据
        let _ = checkpoint_collection.drop(None).await;
        let _ = scan_record_collection.drop(None).await;

        let checkpoint_repo = EventScannerCheckpointRepository::new(checkpoint_collection);
        let scan_record_repo = ScanRecordRepository::new(scan_record_collection);

        // 初始化索引
        checkpoint_repo.init_indexes().await.unwrap();
        scan_record_repo.init_indexes().await.unwrap();

        (checkpoint_repo, scan_record_repo)
    }

    #[tokio::test]
    async fn test_checkpoint_operations() {
        let (checkpoint_repo, _) = setup_test_collections().await;

        let checkpoint = EventScannerCheckpoints {
            id: None,
            program_id: Some("test_program".to_string()),
            event_name: Some("test_event".to_string()),
            slot: Some(12345),
            last_signature: Some("test_signature".to_string()),
            updated_at: Utc::now(),
            created_at: Utc::now(),
        };

        // 测试创建检查点
        let id = checkpoint_repo.upsert_checkpoint(checkpoint.clone()).await.unwrap();
        assert!(!id.is_empty());

        // 测试查找检查点
        let found = checkpoint_repo
            .find_checkpoint("test_program", "test_event")
            .await
            .unwrap();

        assert!(found.is_some());
        let found_checkpoint = found.unwrap();
        assert_eq!(found_checkpoint.program_id, Some("test_program".to_string()));
        assert_eq!(found_checkpoint.event_name, Some("test_event".to_string()));
        assert_eq!(found_checkpoint.slot, Some(12345));

        // 清理测试数据
        checkpoint_repo
            .delete_checkpoint("test_program", "test_event")
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_scan_record_operations() {
        let (_, scan_record_repo) = setup_test_collections().await;

        let scan_record = ScanRecords {
            id: None,
            scan_id: String::new(), // 将被Repository自动生成
            until_slot: Some(100),
            before_slot: Some(200),
            until_signature: "sig_start".to_string(),
            before_signature: "sig_end".to_string(),
            status: ScanStatus::Running,
            events_found: 0,
            events_backfilled_count: 0,
            events_backfilled_signatures: vec![],
            started_at: Utc::now(),
            completed_at: None,
            error_message: None,
            program_filters: vec!["program1".to_string(), "program2".to_string()],
        };

        // 测试创建扫描记录
        let generated_scan_id = scan_record_repo.create_scan_record(scan_record).await.unwrap();
        assert!(!generated_scan_id.is_empty());
        // 验证生成的是有效的UUID格式
        assert!(Uuid::parse_str(&generated_scan_id).is_ok());

        // 测试查找记录
        let found = scan_record_repo.find_by_scan_id(&generated_scan_id).await.unwrap();
        assert!(found.is_some());
        let found_record = found.unwrap();
        assert_eq!(found_record.scan_id, generated_scan_id);
        assert_eq!(found_record.status, ScanStatus::Running);

        // 测试更新状态
        let updated = scan_record_repo
            .update_scan_status(&generated_scan_id, ScanStatus::Completed, None)
            .await
            .unwrap();
        assert!(updated);

        // 重新查询记录验证状态更新
        let updated_record = scan_record_repo.find_by_scan_id(&generated_scan_id).await.unwrap();
        assert!(updated_record.is_some());
        let updated_record = updated_record.unwrap();
        assert_eq!(updated_record.status, ScanStatus::Completed);
        assert!(updated_record.completed_at.is_some());

        // 测试更新事件统计
        let updated_stats = scan_record_repo
            .update_event_stats(&generated_scan_id, 10, 5, vec!["sig1".to_string(), "sig2".to_string()])
            .await
            .unwrap();
        assert!(updated_stats);

        // 清理测试数据
        scan_record_repo.delete_scan_record(&generated_scan_id).await.unwrap();
    }
}
