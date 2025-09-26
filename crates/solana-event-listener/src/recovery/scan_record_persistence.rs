use crate::error::{EventListenerError, Result};
use database::events::event_scanner::model::{ScanRecords, ScanStatus};
use mongodb::{
    bson::{doc, from_document, to_document},
    options::FindOptions,
    Collection, Database,
};
use std::sync::Arc;
use tracing::{debug, error, info, warn};

/// 扫描记录持久化服务
/// 
/// 负责管理ScanRecords的CRUD操作
/// 索引管理由database层的ScanRecordRepository处理
pub struct ScanRecordPersistence {
    collection: Collection<mongodb::bson::Document>,
}

impl ScanRecordPersistence {
    /// 创建新的扫描记录持久化服务
    pub async fn new(database: Arc<Database>) -> Result<Self> {
        let collection = database.collection("ScanRecords");
        Ok(Self { collection })
    }
    
    /// 创建扫描记录
    pub async fn create_scan_record(&self, scan_record: &ScanRecords) -> Result<()> {
        let doc = to_document(scan_record)
            .map_err(|e| EventListenerError::Unknown(format!("序列化扫描记录失败: {}", e)))?;
        
        match self.collection.insert_one(doc, None).await {
            Ok(result) => {
                info!("✅ 创建扫描记录成功: {} (ObjectId: {:?})", scan_record.scan_id, result.inserted_id);
                Ok(())
            }
            Err(e) => {
                error!("❌ 创建扫描记录失败: {}", e);
                Err(EventListenerError::Unknown(format!("创建扫描记录失败: {}", e)))
            }
        }
    }
    
    /// 根据scan_id获取扫描记录
    pub async fn get_scan_record(&self, scan_id: &str) -> Result<Option<ScanRecords>> {
        let filter = doc! { "scan_id": scan_id };
        
        match self.collection.find_one(filter, None).await {
            Ok(Some(doc)) => {
                let scan_record: ScanRecords = from_document(doc)
                    .map_err(|e| EventListenerError::Unknown(format!("反序列化扫描记录失败: {}", e)))?;
                
                debug!("📋 获取扫描记录成功: {}", scan_id);
                Ok(Some(scan_record))
            }
            Ok(None) => {
                debug!("📋 没有找到扫描记录: {}", scan_id);
                Ok(None)
            }
            Err(e) => {
                error!("❌ 获取扫描记录失败: {}", e);
                Err(EventListenerError::Unknown(format!("获取扫描记录失败: {}", e)))
            }
        }
    }
    
    /// 更新扫描记录
    pub async fn update_scan_record(&self, scan_record: &ScanRecords) -> Result<()> {
        let doc = to_document(scan_record)
            .map_err(|e| EventListenerError::Unknown(format!("序列化扫描记录失败: {}", e)))?;
        
        let filter = doc! { "scan_id": &scan_record.scan_id };
        let update = doc! { "$set": doc };
        
        match self.collection.update_one(filter, update, None).await {
            Ok(result) => {
                if result.matched_count > 0 {
                    info!("✅ 更新扫描记录成功: {}", scan_record.scan_id);
                } else {
                    warn!("⚠️ 没有找到要更新的扫描记录: {}", scan_record.scan_id);
                }
                Ok(())
            }
            Err(e) => {
                error!("❌ 更新扫描记录失败: {}", e);
                Err(EventListenerError::Unknown(format!("更新扫描记录失败: {}", e)))
            }
        }
    }
    
    /// 根据状态查询扫描记录
    pub async fn get_scan_records_by_status(&self, status: &ScanStatus) -> Result<Vec<ScanRecords>> {
        let status_str = match status {
            ScanStatus::Running => "Running",
            ScanStatus::Completed => "Completed",
            ScanStatus::Failed => "Failed",
            ScanStatus::Cancelled => "Cancelled",
        };
        
        let filter = doc! { "status": status_str };
        let sort = doc! { "started_at": -1 };
        
        let options = FindOptions::builder()
            .sort(sort)
            .build();
        
        let mut cursor = self.collection.find(filter, Some(options)).await
            .map_err(|e| EventListenerError::Unknown(format!("查询扫描记录失败: {}", e)))?;
        
        let mut scan_records = Vec::new();
        
        while cursor.advance().await.map_err(|e| EventListenerError::Unknown(format!("遍历扫描记录失败: {}", e)))? {
            let doc = cursor.current();
            let doc_parsed: mongodb::bson::Document = doc
                .try_into()
                .map_err(|e| EventListenerError::Unknown(format!("MongoDB文档转换失败: {}", e)))?;
            let scan_record: ScanRecords = from_document(doc_parsed)
                .map_err(|e| EventListenerError::Unknown(format!("反序列化扫描记录失败: {}", e)))?;
            scan_records.push(scan_record);
        }
        
        debug!("📋 查询到 {} 个{}状态的扫描记录", scan_records.len(), status_str);
        Ok(scan_records)
    }
    
    /// 获取正在运行的扫描记录
    pub async fn get_running_scans(&self) -> Result<Vec<ScanRecords>> {
        self.get_scan_records_by_status(&ScanStatus::Running).await
    }
    
    /// 获取最近的扫描记录
    pub async fn get_recent_scan_records(&self, limit: i64) -> Result<Vec<ScanRecords>> {
        let sort = doc! { "started_at": -1 };
        
        let options = FindOptions::builder()
            .sort(sort)
            .limit(limit)
            .build();
        
        let mut cursor = self.collection.find(doc! {}, Some(options)).await
            .map_err(|e| EventListenerError::Unknown(format!("查询最近扫描记录失败: {}", e)))?;
        
        let mut scan_records = Vec::new();
        
        while cursor.advance().await.map_err(|e| EventListenerError::Unknown(format!("遍历扫描记录失败: {}", e)))? {
            let doc = cursor.current();
            let doc_parsed: mongodb::bson::Document = doc
                .try_into()
                .map_err(|e| EventListenerError::Unknown(format!("MongoDB文档转换失败: {}", e)))?;
            let scan_record: ScanRecords = from_document(doc_parsed)
                .map_err(|e| EventListenerError::Unknown(format!("反序列化扫描记录失败: {}", e)))?;
            scan_records.push(scan_record);
        }
        
        debug!("📋 查询到 {} 个最近的扫描记录", scan_records.len());
        Ok(scan_records)
    }
    
    /// 根据程序过滤器查询扫描记录
    pub async fn get_scan_records_by_program(&self, program_id: &str) -> Result<Vec<ScanRecords>> {
        let filter = doc! {
            "program_filters": {
                "$in": [program_id]
            }
        };
        
        let sort = doc! { "started_at": -1 };
        
        let options = FindOptions::builder()
            .sort(sort)
            .build();
        
        let mut cursor = self.collection.find(filter, Some(options)).await
            .map_err(|e| EventListenerError::Unknown(format!("查询程序扫描记录失败: {}", e)))?;
        
        let mut scan_records = Vec::new();
        
        while cursor.advance().await.map_err(|e| EventListenerError::Unknown(format!("遍历扫描记录失败: {}", e)))? {
            let doc = cursor.current();
            let doc_parsed: mongodb::bson::Document = doc
                .try_into()
                .map_err(|e| EventListenerError::Unknown(format!("MongoDB文档转换失败: {}", e)))?;
            let scan_record: ScanRecords = from_document(doc_parsed)
                .map_err(|e| EventListenerError::Unknown(format!("反序列化扫描记录失败: {}", e)))?;
            scan_records.push(scan_record);
        }
        
        debug!("📋 查询到 {} 个程序({})的扫描记录", scan_records.len(), program_id);
        Ok(scan_records)
    }
    
    /// 删除扫描记录
    pub async fn delete_scan_record(&self, scan_id: &str) -> Result<bool> {
        let filter = doc! { "scan_id": scan_id };
        
        match self.collection.delete_one(filter, None).await {
            Ok(result) => {
                let deleted = result.deleted_count > 0;
                if deleted {
                    info!("🗑️ 删除扫描记录成功: {}", scan_id);
                } else {
                    warn!("⚠️ 没有找到要删除的扫描记录: {}", scan_id);
                }
                Ok(deleted)
            }
            Err(e) => {
                error!("❌ 删除扫描记录失败: {}", e);
                Err(EventListenerError::Unknown(format!("删除扫描记录失败: {}", e)))
            }
        }
    }
    
    /// 获取扫描统计信息
    pub async fn get_scan_statistics(&self) -> Result<ScanStatistics> {
        use mongodb::bson::doc;
        
        let pipeline = vec![
            doc! {
                "$group": {
                    "_id": "$status",
                    "count": { "$sum": 1 },
                    "total_events_found": { "$sum": "$events_found" },
                    "total_events_backfilled": { "$sum": "$events_backfilled_count" }
                }
            }
        ];
        
        let mut cursor = self.collection.aggregate(pipeline, None).await
            .map_err(|e| EventListenerError::Unknown(format!("获取扫描统计失败: {}", e)))?;
        
        let mut statistics = ScanStatistics::default();
        
        while cursor.advance().await.map_err(|e| EventListenerError::Unknown(format!("遍历统计结果失败: {}", e)))? {
            let doc = cursor.current();
            
            if let Ok(status) = doc.get_str("_id") {
                let count = doc.get_i32("count").unwrap_or(0) as u64;
                let events_found = doc.get_i64("total_events_found").unwrap_or(0) as u64;
                let events_backfilled = doc.get_i64("total_events_backfilled").unwrap_or(0) as u64;
                
                match status {
                    "Running" => {
                        statistics.running_count = count;
                        statistics.running_events_found = events_found;
                        statistics.running_events_backfilled = events_backfilled;
                    }
                    "Completed" => {
                        statistics.completed_count = count;
                        statistics.completed_events_found = events_found;
                        statistics.completed_events_backfilled = events_backfilled;
                    }
                    "Failed" => {
                        statistics.failed_count = count;
                        statistics.failed_events_found = events_found;
                    }
                    "Cancelled" => {
                        statistics.cancelled_count = count;
                    }
                    _ => {}
                }
            }
        }
        
        statistics.total_scans = statistics.running_count + statistics.completed_count + statistics.failed_count + statistics.cancelled_count;
        statistics.total_events_found = statistics.running_events_found + statistics.completed_events_found + statistics.failed_events_found;
        statistics.total_events_backfilled = statistics.running_events_backfilled + statistics.completed_events_backfilled;
        
        debug!("📊 扫描统计: 总扫描{}, 总发现{}, 总回填{}", 
            statistics.total_scans, 
            statistics.total_events_found, 
            statistics.total_events_backfilled);
        
        Ok(statistics)
    }
    
    /// 健康检查
    pub async fn is_healthy(&self) -> bool {
        match self.collection.find_one(doc! {}, None).await {
            Ok(_) => true,
            Err(e) => {
                error!("❌ 扫描记录持久化服务健康检查失败: {}", e);
                false
            }
        }
    }
}

/// 扫描统计信息
#[derive(Debug, Clone, Default)]
pub struct ScanStatistics {
    pub total_scans: u64,
    pub running_count: u64,
    pub completed_count: u64,
    pub failed_count: u64,
    pub cancelled_count: u64,
    
    pub total_events_found: u64,
    pub total_events_backfilled: u64,
    
    pub running_events_found: u64,
    pub running_events_backfilled: u64,
    pub completed_events_found: u64,
    pub completed_events_backfilled: u64,
    pub failed_events_found: u64,
}

impl ScanStatistics {
    /// 获取成功率
    pub fn success_rate(&self) -> f64 {
        if self.total_scans == 0 {
            0.0
        } else {
            self.completed_count as f64 / self.total_scans as f64
        }
    }
    
    /// 获取回填效率
    pub fn backfill_efficiency(&self) -> f64 {
        if self.total_events_found == 0 {
            0.0
        } else {
            self.total_events_backfilled as f64 / self.total_events_found as f64
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    
    async fn create_test_persistence() -> ScanRecordPersistence {
        // 这里需要真实的MongoDB连接用于集成测试
        // 在单元测试中，应该使用mock
        todo!("需要MongoDB测试环境")
    }
    
    #[tokio::test]
    #[ignore] // 需要MongoDB连接
    async fn test_scan_record_crud() {
        let persistence = create_test_persistence().await;
        
        // 创建测试扫描记录
        let scan_record = ScanRecords {
            id: None,
            scan_id: "test-scan-001".to_string(),
            until_slot: Some(100),
            before_slot: Some(200),
            until_signature: "sig1".to_string(),
            before_signature: "sig2".to_string(),
            status: ScanStatus::Running,
            events_found: 10,
            events_backfilled_count: 8,
            events_backfilled_signatures: vec!["sig1".to_string(), "sig2".to_string()],
            started_at: Utc::now(),
            completed_at: None,
            error_message: None,
            program_filters: vec!["test_program".to_string()],
            program_id: Some("test_program".to_string()),
            event_name: Some("test_event".to_string()),
        };
        
        // 测试创建
        persistence.create_scan_record(&scan_record).await.unwrap();
        
        // 测试查询
        let retrieved = persistence
            .get_scan_record("test-scan-001")
            .await
            .unwrap();
        
        assert!(retrieved.is_some());
        let retrieved_record = retrieved.unwrap();
        assert_eq!(retrieved_record.scan_id, scan_record.scan_id);
        assert_eq!(retrieved_record.events_found, scan_record.events_found);
        
        // 测试删除
        let deleted = persistence
            .delete_scan_record("test-scan-001")
            .await
            .unwrap();
        
        assert!(deleted);
    }
}