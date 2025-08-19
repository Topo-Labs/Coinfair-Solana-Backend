#[cfg(test)]
use crate::error::EventListenerError;
use crate::{config::EventListenerConfig, error::Result};
use backoff::{future::retry, ExponentialBackoff};
use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, error, info, warn};

/// 重试任务
#[derive(Debug, Clone)]
pub struct RetryTask<T> {
    /// 任务ID
    pub id: String,
    /// 任务数据
    pub data: T,
    /// 创建时间
    pub created_at: Instant,
    /// 重试次数
    pub retry_count: u32,
    /// 最大重试次数
    pub max_retries: u32,
    /// 下次重试时间
    pub next_retry_at: Instant,
    /// 错误历史
    pub error_history: Vec<String>,
}

impl<T> RetryTask<T> {
    /// 创建新的重试任务
    pub fn new(id: String, data: T, max_retries: u32) -> Self {
        let now = Instant::now();
        Self {
            id,
            data,
            created_at: now,
            retry_count: 0,
            max_retries,
            next_retry_at: now,
            error_history: Vec::new(),
        }
    }

    /// 记录失败并计算下次重试时间
    pub fn record_failure(&mut self, error: String, base_delay: Duration) {
        self.retry_count += 1;
        self.error_history.push(error);

        // 指数退避：base_delay * 2^retry_count
        let delay = base_delay * (2_u32.pow(self.retry_count.min(10))); // 限制最大指数
        self.next_retry_at = Instant::now() + delay;
    }

    /// 检查是否可以重试
    pub fn can_retry(&self) -> bool {
        self.retry_count < self.max_retries && Instant::now() >= self.next_retry_at
    }

    /// 检查是否已达到最大重试次数
    pub fn is_exhausted(&self) -> bool {
        self.retry_count >= self.max_retries
    }

    /// 获取任务年龄
    pub fn age(&self) -> Duration {
        Instant::now() - self.created_at
    }
}

/// 重试管理器
///
/// 负责:
/// - 管理失败任务的重试队列
/// - 实现指数退避重试策略
/// - 提供重试统计和监控
/// - 处理任务的生命周期管理
pub struct RetryManager<T: Clone + Send + Sync + 'static> {
    config: Arc<EventListenerConfig>,

    // 重试配置
    max_retries: u32,
    base_delay: Duration,
    max_queue_size: usize,

    // 重试队列
    retry_queue: Arc<Mutex<VecDeque<RetryTask<T>>>>,

    // 统计信息
    tasks_added: Arc<AtomicU64>,
    tasks_retried: Arc<AtomicU64>,
    tasks_succeeded: Arc<AtomicU64>,
    tasks_failed: Arc<AtomicU64>,
    tasks_expired: Arc<AtomicU64>,

    // 运行状态
    is_running: Arc<RwLock<bool>>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RetryStats {
    pub queue_size: usize,
    pub tasks_added: u64,
    pub tasks_retried: u64,
    pub tasks_succeeded: u64,
    pub tasks_failed: u64,
    pub tasks_expired: u64,
    pub success_rate: f64,
    pub is_running: bool,
}

impl<T: Clone + Send + Sync + 'static> RetryManager<T> {
    /// 创建新的重试管理器
    pub fn new(config: &EventListenerConfig) -> Self {
        let config = Arc::new(config.clone());

        Self {
            config: Arc::clone(&config),
            max_retries: config.listener.max_retries,
            base_delay: Duration::from_millis(config.listener.retry_delay_ms),
            max_queue_size: 1000, // 默认最大队列大小
            retry_queue: Arc::new(Mutex::new(VecDeque::new())),
            tasks_added: Arc::new(AtomicU64::new(0)),
            tasks_retried: Arc::new(AtomicU64::new(0)),
            tasks_succeeded: Arc::new(AtomicU64::new(0)),
            tasks_failed: Arc::new(AtomicU64::new(0)),
            tasks_expired: Arc::new(AtomicU64::new(0)),
            is_running: Arc::new(RwLock::new(false)),
        }
    }

    /// 启动重试管理器
    pub async fn start<F, Fut>(&self, retry_handler: F) -> Result<()>
    where
        F: Fn(T) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<()>> + Send,
    {
        let mut is_running = self.is_running.write().await;
        if *is_running {
            warn!("重试管理器已在运行中");
            return Ok(());
        }
        *is_running = true;
        drop(is_running);

        info!("🔄 启动重试管理器");

        let manager = self.clone();
        let retry_handler = Arc::new(retry_handler);

        tokio::spawn(async move {
            manager.retry_loop(retry_handler).await;

            let mut is_running = manager.is_running.write().await;
            *is_running = false;

            info!("🔄 重试管理器已停止");
        });

        Ok(())
    }

    /// 停止重试管理器
    pub async fn stop(&self) -> Result<()> {
        info!("🛑 停止重试管理器");
        let mut is_running = self.is_running.write().await;
        *is_running = false;
        Ok(())
    }

    /// 添加任务到重试队列
    pub async fn add_task(&self, id: String, data: T) -> Result<()> {
        let mut queue = self.retry_queue.lock().await;

        // 检查队列大小
        if queue.len() >= self.max_queue_size {
            warn!("⚠️ 重试队列已满，移除最旧的任务");
            queue.pop_front();
            self.tasks_expired.fetch_add(1, Ordering::Relaxed);
        }

        let task = RetryTask::new(id.clone(), data, self.max_retries);
        queue.push_back(task);

        self.tasks_added.fetch_add(1, Ordering::Relaxed);
        debug!("📥 添加任务到重试队列: {}", id);

        Ok(())
    }

    /// 重试循环
    async fn retry_loop<F, Fut>(&self, retry_handler: Arc<F>)
    where
        F: Fn(T) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<()>> + Send,
    {
        let mut retry_interval = tokio::time::interval(Duration::from_millis(1000)); // 每秒检查一次

        while *self.is_running.read().await {
            retry_interval.tick().await;

            // 处理重试队列
            if let Err(e) = self.process_retry_queue(&retry_handler).await {
                error!("❌ 处理重试队列失败: {}", e);
            }

            // 清理过期任务
            self.cleanup_expired_tasks().await;
        }
    }

    /// 处理重试队列
    async fn process_retry_queue<F, Fut>(&self, retry_handler: &Arc<F>) -> Result<()>
    where
        F: Fn(T) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<()>> + Send,
    {
        let mut tasks_to_retry = Vec::new();
        let mut tasks_to_remove = Vec::new();

        // 收集需要重试的任务
        {
            let queue = self.retry_queue.lock().await;
            let len = queue.len();

            for i in 0..len {
                if let Some(task) = queue.get(i) {
                    if task.can_retry() {
                        tasks_to_retry.push((i, task.clone()));
                    } else if task.is_exhausted() {
                        tasks_to_remove.push(i);
                    }
                }
            }
        }

        // 处理需要移除的任务（从后往前移除，避免索引变化）
        if !tasks_to_remove.is_empty() {
            let mut queue = self.retry_queue.lock().await;
            for &index in tasks_to_remove.iter().rev() {
                if let Some(task) = queue.remove(index) {
                    warn!("❌ 任务重试次数已用完，移除: {} (重试{}次)", task.id, task.retry_count);
                    self.tasks_failed.fetch_add(1, Ordering::Relaxed);
                }
            }
        }

        // 执行重试任务
        for (_queue_index, mut task) in tasks_to_retry {
            debug!("🔄 重试任务: {} (第{}次)", task.id, task.retry_count + 1);

            let task_id = task.id.clone(); // 克隆task.id用于后续使用

            match retry_handler(task.data.clone()).await {
                Ok(()) => {
                    // 重试成功，从队列中移除
                    {
                        let mut queue = self.retry_queue.lock().await;
                        // 由于之前可能有移除操作，需要重新查找任务位置
                        if let Some(pos) = queue.iter().position(|t| t.id == task_id) {
                            queue.remove(pos);
                        }
                    }

                    self.tasks_succeeded.fetch_add(1, Ordering::Relaxed);
                    info!("✅ 任务重试成功: {}", task_id);
                }
                Err(e) => {
                    // 重试失败，更新任务状态
                    task.record_failure(e.to_string(), self.base_delay);

                    {
                        let mut queue = self.retry_queue.lock().await;
                        if let Some(pos) = queue.iter().position(|t| t.id == task_id) {
                            if let Some(queue_task) = queue.get_mut(pos) {
                                *queue_task = task;
                            }
                        }
                    }

                    self.tasks_retried.fetch_add(1, Ordering::Relaxed);
                    warn!("⚠️ 任务重试失败: {} - {}", task_id, e);
                }
            }
        }

        Ok(())
    }

    /// 清理过期任务
    async fn cleanup_expired_tasks(&self) {
        let max_age = Duration::from_secs(3600); // 1小时后过期
        let mut expired_count = 0;

        {
            let mut queue = self.retry_queue.lock().await;
            let _initial_len = queue.len();

            queue.retain(|task| {
                if task.age() > max_age {
                    expired_count += 1;
                    false
                } else {
                    true
                }
            });

            if expired_count > 0 {
                info!("🗑️ 清理了 {} 个过期任务", expired_count);
            }
        }

        if expired_count > 0 {
            self.tasks_expired.fetch_add(expired_count, Ordering::Relaxed);
        }
    }

    /// 使用指数退避重试策略执行操作
    pub async fn retry_with_backoff<F, Fut, R>(&self, operation: F) -> Result<R>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<R>>,
    {
        let backoff = ExponentialBackoff {
            initial_interval: self.base_delay,
            max_interval: Duration::from_secs(300), // 最大5分钟
            multiplier: 2.0,
            max_elapsed_time: Some(Duration::from_secs(1800)), // 总共最多30分钟
            ..Default::default()
        };

        retry(backoff, || async {
            match operation().await {
                Ok(result) => Ok(result),
                Err(e) => {
                    warn!("🔄 操作失败，将重试: {}", e);
                    Err(backoff::Error::transient(e))
                }
            }
        })
        .await
    }

    /// 获取统计信息
    pub async fn get_stats(&self) -> RetryStats {
        let queue_size = {
            let queue = self.retry_queue.lock().await;
            queue.len()
        };

        let tasks_added = self.tasks_added.load(Ordering::Relaxed);
        let tasks_succeeded = self.tasks_succeeded.load(Ordering::Relaxed);
        let tasks_failed = self.tasks_failed.load(Ordering::Relaxed);
        let total_completed = tasks_succeeded + tasks_failed;

        let success_rate = if total_completed > 0 {
            tasks_succeeded as f64 / total_completed as f64
        } else {
            1.0
        };

        RetryStats {
            queue_size,
            tasks_added,
            tasks_retried: self.tasks_retried.load(Ordering::Relaxed),
            tasks_succeeded,
            tasks_failed,
            tasks_expired: self.tasks_expired.load(Ordering::Relaxed),
            success_rate,
            is_running: *self.is_running.read().await,
        }
    }

    /// 清空重试队列
    pub async fn clear_queue(&self) -> usize {
        let mut queue = self.retry_queue.lock().await;
        let count = queue.len();
        queue.clear();

        info!("🗑️ 清空重试队列，移除了 {} 个任务", count);
        count
    }

    /// 获取队列中的任务列表（用于调试）
    pub async fn get_queue_tasks(&self) -> Vec<String> {
        let queue = self.retry_queue.lock().await;
        queue
            .iter()
            .map(|task| {
                format!(
                    "ID: {}, 重试次数: {}/{}, 年龄: {:?}",
                    task.id,
                    task.retry_count,
                    task.max_retries,
                    task.age()
                )
            })
            .collect()
    }

    /// 检查重试管理器是否健康
    pub async fn is_healthy(&self) -> bool {
        let is_running = *self.is_running.read().await;
        let queue_size = {
            let queue = self.retry_queue.lock().await;
            queue.len()
        };

        // 运行中且队列未过载
        is_running && queue_size < self.max_queue_size
    }
}

impl<T: Clone + Send + Sync + 'static> Clone for RetryManager<T> {
    fn clone(&self) -> Self {
        Self {
            config: Arc::clone(&self.config),
            max_retries: self.max_retries,
            base_delay: self.base_delay,
            max_queue_size: self.max_queue_size,
            retry_queue: Arc::clone(&self.retry_queue),
            tasks_added: Arc::clone(&self.tasks_added),
            tasks_retried: Arc::clone(&self.tasks_retried),
            tasks_succeeded: Arc::clone(&self.tasks_succeeded),
            tasks_failed: Arc::clone(&self.tasks_failed),
            tasks_expired: Arc::clone(&self.tasks_expired),
            is_running: Arc::clone(&self.is_running),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicU32;

    fn create_test_config() -> EventListenerConfig {
        EventListenerConfig {
            solana: crate::config::settings::SolanaConfig {
                rpc_url: "https://api.devnet.solana.com".to_string(),
                ws_url: "wss://api.devnet.solana.com".to_string(),
                commitment: "confirmed".to_string(),
                program_ids: vec![solana_sdk::pubkey::Pubkey::new_unique()],
                private_key: None,
            },
            database: crate::config::settings::DatabaseConfig {
                uri: "mongodb://localhost:27017".to_string(),
                database_name: "test".to_string(),
                max_connections: 10,
                min_connections: 2,
            },
            listener: crate::config::settings::ListenerConfig {
                batch_size: 100,
                sync_interval_secs: 30,
                max_retries: 3,
                retry_delay_ms: 100,
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
    fn test_retry_task_creation() {
        let task = RetryTask::new("test_task".to_string(), "test_data".to_string(), 3);

        assert_eq!(task.id, "test_task");
        assert_eq!(task.data, "test_data");
        assert_eq!(task.retry_count, 0);
        assert_eq!(task.max_retries, 3);
        assert!(task.can_retry());
        assert!(!task.is_exhausted());
    }

    #[test]
    fn test_retry_task_failure_recording() {
        let mut task = RetryTask::new("test_task".to_string(), "test_data".to_string(), 3);
        let base_delay = Duration::from_millis(100);

        // 记录第一次失败
        task.record_failure("Error 1".to_string(), base_delay);
        assert_eq!(task.retry_count, 1);
        assert_eq!(task.error_history.len(), 1);
        assert!(!task.is_exhausted());

        // 记录第二次失败
        task.record_failure("Error 2".to_string(), base_delay);
        assert_eq!(task.retry_count, 2);
        assert_eq!(task.error_history.len(), 2);

        // 记录第三次失败
        task.record_failure("Error 3".to_string(), base_delay);
        assert_eq!(task.retry_count, 3);
        assert!(task.is_exhausted());
    }

    #[tokio::test]
    async fn test_retry_manager_creation() {
        let config = create_test_config();
        let manager: RetryManager<String> = RetryManager::new(&config);

        let stats = manager.get_stats().await;
        assert_eq!(stats.queue_size, 0);
        assert_eq!(stats.tasks_added, 0);
        assert!(!stats.is_running);
    }

    #[tokio::test]
    async fn test_add_task() {
        let config = create_test_config();
        let manager: RetryManager<String> = RetryManager::new(&config);

        manager
            .add_task("task1".to_string(), "data1".to_string())
            .await
            .unwrap();

        let stats = manager.get_stats().await;
        assert_eq!(stats.queue_size, 1);
        assert_eq!(stats.tasks_added, 1);
    }

    #[tokio::test]
    async fn test_retry_with_backoff_success() {
        let config = create_test_config();
        let manager: RetryManager<String> = RetryManager::new(&config);

        let result = manager.retry_with_backoff(|| async { Ok("success".to_string()) }).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success");
    }

    #[tokio::test]
    async fn test_retry_with_backoff_failure() {
        let config = create_test_config();
        let manager: RetryManager<String> = RetryManager::new(&config);

        let attempt_count = Arc::new(AtomicU32::new(0));
        let attempt_count_clone = Arc::clone(&attempt_count);

        let result = manager
            .retry_with_backoff(|| {
                let count = Arc::clone(&attempt_count_clone);
                async move {
                    let current = count.fetch_add(1, Ordering::Relaxed);
                    if current < 2 {
                        Err(EventListenerError::Unknown("Temporary failure".to_string()))
                    } else {
                        Ok("success after retries".to_string())
                    }
                }
            })
            .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success after retries");
        assert!(attempt_count.load(Ordering::Relaxed) >= 3); // 至少重试了2次
    }

    #[tokio::test]
    async fn test_clear_queue() {
        let config = create_test_config();
        let manager: RetryManager<String> = RetryManager::new(&config);

        // 添加一些任务
        manager
            .add_task("task1".to_string(), "data1".to_string())
            .await
            .unwrap();
        manager
            .add_task("task2".to_string(), "data2".to_string())
            .await
            .unwrap();

        let initial_stats = manager.get_stats().await;
        assert_eq!(initial_stats.queue_size, 2);

        // 清空队列
        let cleared_count = manager.clear_queue().await;
        assert_eq!(cleared_count, 2);

        let final_stats = manager.get_stats().await;
        assert_eq!(final_stats.queue_size, 0);
    }

    #[tokio::test]
    async fn test_is_healthy() {
        let config = create_test_config();
        let manager: RetryManager<String> = RetryManager::new(&config);

        // 初始状态不健康（未运行）
        assert!(!manager.is_healthy().await);

        // 模拟运行状态
        {
            let mut is_running = manager.is_running.write().await;
            *is_running = true;
        }

        // 运行状态下应该健康
        assert!(manager.is_healthy().await);
    }
}
