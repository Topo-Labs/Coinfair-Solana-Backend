use crate::{
    config::EventListenerConfig,
    error::{EventListenerError, Result},
};
use futures::stream::TryStreamExt;
use mongodb::{bson::doc, Client, Collection};
use serde::{Deserialize, Serialize};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tokio::{
    sync::{Mutex, RwLock},
    time::interval,
};
use tracing::{debug, error, info, warn};

/// 检查点复合主键
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CheckpointId {
    /// 程序ID（确保不同程序的检查点隔离）
    pub program_id: String,
    /// 检查点ID（固定为1，用于单例模式）
    pub checkpoint_id: i32,
}

/// 检查点记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointRecord {
    /// 复合主键：程序ID + 固定ID
    #[serde(rename = "_id")]
    pub id: CheckpointId,
    /// 最后处理的交易签名
    pub last_signature: Option<String>,
    /// 最后处理的区块高度
    pub last_slot: u64,
    /// 最后处理时间
    pub last_processed_at: chrono::DateTime<chrono::Utc>,
    /// 已处理的事件总数
    pub events_processed: u64,
    /// 更新时间
    pub updated_at: chrono::DateTime<chrono::Utc>,
    /// 检查点版本（用于兼容性检查）
    pub version: String,
    /// 程序ID（确保检查点对应正确的程序）
    pub program_id: String,
    /// 额外的元数据
    pub metadata: serde_json::Value,
}

impl Default for CheckpointRecord {
    fn default() -> Self {
        let now = chrono::Utc::now();
        Self {
            id: CheckpointId {
                program_id: String::new(),
                checkpoint_id: 1,
            },
            last_signature: None,
            last_slot: 0,
            last_processed_at: now,
            events_processed: 0,
            updated_at: now,
            version: "1.0.0".to_string(),
            program_id: String::new(),
            metadata: serde_json::Value::Null,
        }
    }
}

/// 检查点管理器
///
/// 负责:
/// - 维护事件处理的检查点
/// - 支持崩溃恢复和断点续传
/// - 定期保存检查点以确保数据不丢失
/// - 提供检查点查询和统计功能
pub struct CheckpointManager {
    config: Arc<EventListenerConfig>,
    collection: Collection<CheckpointRecord>,

    // 运行状态
    is_running: Arc<AtomicBool>,

    // 内存中的检查点缓存
    current_checkpoint: Arc<RwLock<Option<CheckpointRecord>>>,

    // 并发保存锁
    save_mutex: Arc<Mutex<()>>,

    // 统计信息
    save_count: Arc<RwLock<u64>>,
    last_save_time: Arc<RwLock<Option<Instant>>>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CheckpointStats {
    pub is_running: bool,
    pub last_signature: Option<String>,
    pub last_slot: u64,
    pub events_processed: u64,
    pub last_processed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub save_count: u64,
    #[serde(skip)]
    pub last_save_time: Option<Instant>,
    pub checkpoint_exists: bool,
}

impl CheckpointManager {
    /// 创建新的检查点管理器
    pub async fn new(config: &EventListenerConfig) -> Result<Self> {
        let config = Arc::new(config.clone());

        // 创建数据库连接
        let client = Client::with_uri_str(&config.database.uri).await.map_err(|e| EventListenerError::Database(e))?;

        let database = client.database(&config.database.database_name);
        let collection = database.collection::<CheckpointRecord>("event_listener_checkpoints");

        let manager = Self {
            config,
            collection,
            is_running: Arc::new(AtomicBool::new(false)),
            current_checkpoint: Arc::new(RwLock::new(None)),
            save_mutex: Arc::new(Mutex::new(())),
            save_count: Arc::new(RwLock::new(0)),
            last_save_time: Arc::new(RwLock::new(None)),
        };

        // 创建优化的索引
        manager.ensure_indexes().await?;

        // 加载现有检查点
        manager.load_checkpoint().await?;

        info!("✅ 检查点管理器初始化完成");
        Ok(manager)
    }

    /// 确保必要的数据库索引存在
    async fn ensure_indexes(&self) -> Result<()> {
        debug!("🔧 创建数据库索引...");

        // 主索引：基于复合主键的唯一索引
        let primary_index = mongodb::IndexModel::builder()
            .keys(doc! { "_id.program_id": 1, "_id.checkpoint_id": 1 })
            .options(mongodb::options::IndexOptions::builder().unique(true).name("checkpoint_primary_idx".to_string()).build())
            .build();

        // 查询优化索引：基于program_id的非唯一索引
        let query_index = mongodb::IndexModel::builder()
            .keys(doc! { "program_id": 1, "updated_at": -1 })
            .options(mongodb::options::IndexOptions::builder().name("checkpoint_query_idx".to_string()).build())
            .build();

        // 时间查询索引：用于监控和统计
        let time_index = mongodb::IndexModel::builder()
            .keys(doc! { "last_processed_at": -1 })
            .options(mongodb::options::IndexOptions::builder().name("checkpoint_time_idx".to_string()).build())
            .build();

        let indexes = vec![primary_index, query_index, time_index];

        match self.collection.create_indexes(indexes, None).await {
            Ok(result) => {
                info!("✅ 数据库索引创建成功: {:?}", result.index_names);
            }
            Err(e) => {
                // 索引可能已存在，这不是致命错误
                if e.to_string().contains("already exists") || e.to_string().contains("IndexOptionsConflict") {
                    debug!("ℹ️ 数据库索引已存在，跳过创建");
                } else {
                    warn!("⚠️ 数据库索引创建失败: {}", e);
                    return Err(EventListenerError::Database(e));
                }
            }
        }

        Ok(())
    }

    /// 启动定期保存任务
    pub async fn start_periodic_save(&self) -> Result<()> {
        if self.is_running.load(Ordering::Relaxed) {
            warn!("检查点管理器已在运行中");
            return Ok(());
        }

        self.is_running.store(true, Ordering::Relaxed);
        info!("🔄 启动检查点定期保存任务");

        let manager = self.clone();
        let save_interval = self.config.get_checkpoint_save_interval();

        tokio::spawn(async move {
            let mut interval = interval(save_interval);

            while manager.is_running.load(Ordering::Relaxed) {
                interval.tick().await;

                if let Err(e) = manager.save_checkpoint().await {
                    error!("❌ 定期保存检查点失败: {}", e);
                }
            }

            info!("🔄 检查点定期保存任务已停止");
        });

        Ok(())
    }

    /// 停止检查点管理器
    pub async fn stop(&self) -> Result<()> {
        info!("🛑 停止检查点管理器");
        self.is_running.store(false, Ordering::Relaxed);

        // 保存最终检查点
        self.save_checkpoint().await?;

        Ok(())
    }

    /// 加载现有检查点
    async fn load_checkpoint(&self) -> Result<()> {
        debug!("📥 加载检查点...");

        let checkpoint_id = CheckpointId {
            program_id: self.config.solana.program_id.to_string(),
            checkpoint_id: 1,
        };

        let filter = doc! {
            "_id": mongodb::bson::to_bson(&checkpoint_id)
                .map_err(|e| EventListenerError::Database(e.into()))?
        };

        match self.collection.find_one(filter, None).await {
            Ok(Some(checkpoint)) => {
                info!(
                    "✅ 加载到现有检查点: slot={}, events={}, signature={:?}",
                    checkpoint.last_slot, checkpoint.events_processed, checkpoint.last_signature
                );

                let mut current = self.current_checkpoint.write().await;
                *current = Some(checkpoint);
            }
            Ok(None) => {
                info!("ℹ️ 未找到现有检查点，将创建新的检查点");

                let new_checkpoint = CheckpointRecord {
                    id: CheckpointId {
                        program_id: self.config.solana.program_id.to_string(),
                        checkpoint_id: 1,
                    },
                    program_id: self.config.solana.program_id.to_string(),
                    ..Default::default()
                };

                let mut current = self.current_checkpoint.write().await;
                *current = Some(new_checkpoint);
            }
            Err(e) => {
                error!("❌ 加载检查点失败: {}", e);
                return Err(EventListenerError::Checkpoint(format!("加载检查点失败: {}", e)));
            }
        }

        Ok(())
    }

    /// 更新最后处理的事件信息
    pub async fn update_last_processed(&self, signature: &str, slot: u64) -> Result<()> {
        let mut current = self.current_checkpoint.write().await;

        if let Some(ref mut checkpoint) = *current {
            checkpoint.last_signature = Some(signature.to_string());
            checkpoint.last_slot = slot;
            checkpoint.events_processed += 1;
            checkpoint.last_processed_at = chrono::Utc::now();
            checkpoint.updated_at = chrono::Utc::now();

            debug!("📝 更新检查点: signature={}, slot={}, events={}", signature, slot, checkpoint.events_processed);
        } else {
            warn!("⚠️ 检查点未初始化，无法更新");
            return Err(EventListenerError::Checkpoint("检查点未初始化".to_string()));
        }

        Ok(())
    }

    /// 保存检查点到数据库（带并发控制和重试机制）
    pub async fn save_checkpoint(&self) -> Result<()> {
        // 获取保存锁，防止并发保存
        let _lock = self.save_mutex.lock().await;

        let checkpoint = {
            let current = self.current_checkpoint.read().await;
            match current.as_ref() {
                Some(cp) => cp.clone(),
                None => {
                    debug!("ℹ️ 没有检查点需要保存");
                    return Ok(());
                }
            }
        };

        debug!("💾 保存检查点到数据库: slot={}, events={}", checkpoint.last_slot, checkpoint.events_processed);

        let filter = doc! {
            "_id": mongodb::bson::to_bson(&checkpoint.id)
                .map_err(|e| EventListenerError::Database(e.into()))?
        };
        let options = mongodb::options::ReplaceOptions::builder().upsert(true).build();

        // 重试机制处理并发冲突
        let mut retries = 0;
        const MAX_RETRIES: u32 = 3;

        loop {
            match self.collection.replace_one(filter.clone(), &checkpoint, options.clone()).await {
                Ok(_) => {
                    // 更新统计信息
                    {
                        let mut save_count = self.save_count.write().await;
                        *save_count += 1;
                    }
                    {
                        let mut last_save = self.last_save_time.write().await;
                        *last_save = Some(Instant::now());
                    }

                    debug!("✅ 检查点保存成功");
                    break;
                }
                Err(e) => {
                    // 特殊处理重复键错误
                    if e.to_string().contains("E11000") || e.to_string().contains("duplicate key") {
                        retries += 1;
                        if retries >= MAX_RETRIES {
                            error!("❌ 检查点保存失败，重试次数已用完: {}", e);
                            return Err(EventListenerError::Checkpoint(format!("保存检查点失败，重试次数已用完: {}", e)));
                        }
                        warn!("⚠️ 检查点保存遇到重复键错误，第{}次重试", retries);

                        // 指数退避
                        tokio::time::sleep(Duration::from_millis(100 * (2_u64.pow(retries)))).await;
                        continue;
                    } else {
                        error!("❌ 检查点保存失败: {}", e);
                        return Err(EventListenerError::Checkpoint(format!("保存检查点失败: {}", e)));
                    }
                }
            }
        }

        Ok(())
    }

    /// 获取当前检查点
    pub async fn get_current_checkpoint(&self) -> Option<CheckpointRecord> {
        let current = self.current_checkpoint.read().await;
        current.clone()
    }

    /// 获取最后处理的签名
    pub async fn get_last_signature(&self) -> Option<String> {
        let current = self.current_checkpoint.read().await;
        current.as_ref().and_then(|cp| cp.last_signature.clone())
    }

    /// 获取最后处理的区块高度
    pub async fn get_last_slot(&self) -> u64 {
        let current = self.current_checkpoint.read().await;
        current.as_ref().map(|cp| cp.last_slot).unwrap_or(0)
    }

    /// 获取已处理的事件总数
    pub async fn get_events_processed(&self) -> u64 {
        let current = self.current_checkpoint.read().await;
        current.as_ref().map(|cp| cp.events_processed).unwrap_or(0)
    }

    /// 重置检查点（谨慎使用）
    pub async fn reset_checkpoint(&self) -> Result<()> {
        warn!("⚠️ 重置检查点");

        let new_checkpoint = CheckpointRecord {
            id: CheckpointId {
                program_id: self.config.solana.program_id.to_string(),
                checkpoint_id: 1,
            },
            program_id: self.config.solana.program_id.to_string(),
            ..Default::default()
        };

        {
            let mut current = self.current_checkpoint.write().await;
            *current = Some(new_checkpoint);
        }

        self.save_checkpoint().await?;
        info!("✅ 检查点已重置");
        Ok(())
    }

    /// 检查管理器是否健康
    pub async fn is_healthy(&self) -> bool {
        // 检查检查点是否存在
        let has_checkpoint = {
            let current = self.current_checkpoint.read().await;
            current.is_some()
        };

        // 检查最近是否有保存活动
        let recent_save = {
            let last_save = self.last_save_time.read().await;
            match *last_save {
                Some(time) => time.elapsed() < Duration::from_secs(300), // 5分钟内有保存
                None => true,                                            // 如果从未保存，认为是健康的（刚启动）
            }
        };

        has_checkpoint && recent_save
    }

    /// 获取统计信息
    pub async fn get_stats(&self) -> CheckpointStats {
        let checkpoint = {
            let current = self.current_checkpoint.read().await;
            current.clone()
        };

        let save_count = *self.save_count.read().await;
        let last_save_time = *self.last_save_time.read().await;

        CheckpointStats {
            is_running: self.is_running.load(Ordering::Relaxed),
            last_signature: checkpoint.as_ref().and_then(|cp| cp.last_signature.clone()),
            last_slot: checkpoint.as_ref().map(|cp| cp.last_slot).unwrap_or(0),
            events_processed: checkpoint.as_ref().map(|cp| cp.events_processed).unwrap_or(0),
            last_processed_at: checkpoint.as_ref().map(|cp| cp.last_processed_at),
            save_count,
            last_save_time,
            checkpoint_exists: checkpoint.is_some(),
        }
    }

    /// 强制保存检查点
    pub async fn force_save(&self) -> Result<()> {
        info!("🔧 强制保存检查点");
        self.save_checkpoint().await
    }

    /// 更新检查点元数据
    pub async fn update_metadata(&self, metadata: serde_json::Value) -> Result<()> {
        let mut current = self.current_checkpoint.write().await;

        if let Some(ref mut checkpoint) = *current {
            checkpoint.metadata = metadata;
            checkpoint.updated_at = chrono::Utc::now();
            debug!("📝 更新检查点元数据");
        } else {
            return Err(EventListenerError::Checkpoint("检查点未初始化".to_string()));
        }

        Ok(())
    }

    /// 获取检查点年龄（距离上次更新的时间）
    pub async fn get_checkpoint_age(&self) -> Option<Duration> {
        let current = self.current_checkpoint.read().await;
        current.as_ref().map(|cp| {
            let now = chrono::Utc::now();
            let duration = now - cp.updated_at;
            Duration::from_secs(duration.num_seconds() as u64)
        })
    }

    /// 诊断检查点冲突问题
    pub async fn diagnose_conflicts(&self) -> Result<serde_json::Value> {
        let checkpoint_id = CheckpointId {
            program_id: self.config.solana.program_id.to_string(),
            checkpoint_id: 1,
        };

        // 查询所有相关的检查点记录
        let filter = doc! {};
        let cursor = self.collection.find(filter, None).await.map_err(|e| EventListenerError::Database(e))?;

        let mut all_records = Vec::new();
        let records: Vec<CheckpointRecord> = cursor.try_collect().await.map_err(|e| EventListenerError::Database(e))?;

        for record in records {
            all_records.push(serde_json::json!({
                "id": record.id,
                "program_id": record.program_id,
                "last_slot": record.last_slot,
                "events_processed": record.events_processed,
                "updated_at": record.updated_at
            }));
        }

        let diagnostic = serde_json::json!({
            "target_checkpoint_id": checkpoint_id,
            "current_program_id": self.config.solana.program_id.to_string(),
            "all_checkpoint_records": all_records,
            "total_records_found": all_records.len(),
            "timestamp": chrono::Utc::now()
        });

        info!("🔍 检查点冲突诊断: {}", diagnostic);
        Ok(diagnostic)
    }
}

impl Clone for CheckpointManager {
    fn clone(&self) -> Self {
        Self {
            config: Arc::clone(&self.config),
            collection: self.collection.clone(),
            is_running: Arc::clone(&self.is_running),
            current_checkpoint: Arc::clone(&self.current_checkpoint),
            save_mutex: Arc::clone(&self.save_mutex),
            save_count: Arc::clone(&self.save_count),
            last_save_time: Arc::clone(&self.last_save_time),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn create_test_config() -> EventListenerConfig {
        EventListenerConfig {
            solana: crate::config::settings::SolanaConfig {
                rpc_url: "https://api.devnet.solana.com".to_string(),
                ws_url: "wss://api.devnet.solana.com".to_string(),
                commitment: "confirmed".to_string(),
                program_id: solana_sdk::pubkey::Pubkey::from_str("FA1RJDDXysgwg5Gm3fJXWxt26JQzPkAzhTA114miqNUX").unwrap(),
                private_key: None,
            },
            database: crate::config::settings::DatabaseConfig {
                uri: "mongodb://localhost:27017".to_string(),
                database_name: "test_event_listener".to_string(),
                max_connections: 10,
                min_connections: 2,
            },
            listener: crate::config::settings::ListenerConfig {
                batch_size: 100,
                sync_interval_secs: 30,
                max_retries: 3,
                retry_delay_ms: 1000,
                signature_cache_size: 10000,
                checkpoint_save_interval_secs: 60,
                backoff: crate::config::settings::BackoffConfig::default(),
                batch_write: crate::config::settings::BatchWriteConfig::default(),
            },
            monitoring: crate::config::settings::MonitoringConfig {
                metrics_interval_secs: 60,
                enable_performance_monitoring: true,
                health_check_interval_secs: 30,
            },
        }
    }

    #[test]
    fn test_checkpoint_record_default() {
        let checkpoint = CheckpointRecord::default();
        assert_eq!(checkpoint.id.checkpoint_id, 1);
        assert_eq!(checkpoint.last_slot, 0);
        assert_eq!(checkpoint.events_processed, 0);
        assert_eq!(checkpoint.version, "1.0.0");
    }

    #[tokio::test]
    async fn test_checkpoint_manager_creation() {
        let config = create_test_config();

        // 如果无法连接MongoDB，跳过测试
        if let Ok(manager) = CheckpointManager::new(&config).await {
            let stats = manager.get_stats().await;
            assert!(!stats.is_running);
            assert_eq!(stats.save_count, 0);
        }
    }

    #[tokio::test]
    async fn test_update_last_processed() {
        let config = create_test_config();

        if let Ok(manager) = CheckpointManager::new(&config).await {
            let result = manager.update_last_processed("test_signature", 12345).await;

            if result.is_ok() {
                let stats = manager.get_stats().await;
                assert_eq!(stats.last_signature, Some("test_signature".to_string()));
                assert_eq!(stats.last_slot, 12345);
                assert_eq!(stats.events_processed, 1);
            }
        }
    }

    #[tokio::test]
    async fn test_checkpoint_accessors() {
        let config = create_test_config();

        if let Ok(manager) = CheckpointManager::new(&config).await {
            // 测试初始状态
            assert_eq!(manager.get_last_slot().await, 0);
            assert_eq!(manager.get_events_processed().await, 0);
            assert!(manager.get_last_signature().await.is_none());

            // 更新后测试
            if manager.update_last_processed("test_sig", 100).await.is_ok() {
                assert_eq!(manager.get_last_slot().await, 100);
                assert_eq!(manager.get_events_processed().await, 1);
                assert_eq!(manager.get_last_signature().await, Some("test_sig".to_string()));
            }
        }
    }

    #[tokio::test]
    async fn test_metadata_update() {
        let config = create_test_config();

        if let Ok(manager) = CheckpointManager::new(&config).await {
            let metadata = serde_json::json!({
                "version": "test",
                "custom_field": "value"
            });

            let result = manager.update_metadata(metadata.clone()).await;

            if result.is_ok() {
                let checkpoint = manager.get_current_checkpoint().await;
                if let Some(cp) = checkpoint {
                    assert_eq!(cp.metadata, metadata);
                }
            }
        }
    }

    #[tokio::test]
    async fn test_is_healthy() {
        let config = create_test_config();

        if let Ok(manager) = CheckpointManager::new(&config).await {
            // 初始状态应该是健康的
            assert!(manager.is_healthy().await);
        }
    }
}
