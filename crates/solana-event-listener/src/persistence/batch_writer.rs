use crate::{
    config::EventListenerConfig,
    error::{EventListenerError, Result},
    parser::ParsedEvent,
    persistence::EventStorage,
};
use std::{
    collections::{HashMap, VecDeque},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tokio::{
    sync::{mpsc, Mutex, RwLock},
    time::{interval, timeout},
};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// 批量写入器
///
/// 负责:
/// - 收集事件到批量缓冲区
/// - 定期或达到阈值时批量写入数据库
/// - 提供写入性能监控
/// - 处理写入失败和重试
pub struct BatchWriter {
    config: Arc<EventListenerConfig>,
    event_storage: Arc<EventStorage>,

    // 批量写入配置
    batch_size: usize,
    max_wait_duration: Duration,
    buffer_size: usize,

    // 运行状态
    is_running: Arc<AtomicBool>,

    // 事件缓冲区
    event_buffer: Arc<Mutex<VecDeque<ParsedEvent>>>,

    // 统计信息
    events_queued: Arc<AtomicU64>,
    events_written: Arc<AtomicU64>,
    events_failed: Arc<AtomicU64>,
    batches_written: Arc<AtomicU64>,
    last_write_time: Arc<RwLock<Option<Instant>>>,

    // 事件提交通道
    event_sender: mpsc::UnboundedSender<ParsedEvent>,
    event_receiver: Arc<Mutex<mpsc::UnboundedReceiver<ParsedEvent>>>,

    // 重试管理 (测试可见)
    #[cfg(test)]
    pub retry_counts: Arc<Mutex<HashMap<String, u32>>>, // 批次ID -> 重试次数
    #[cfg(not(test))]
    retry_counts: Arc<Mutex<HashMap<String, u32>>>, // 批次ID -> 重试次数
    #[cfg(test)]
    pub max_retries: u32,
    #[cfg(not(test))]
    max_retries: u32,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct BatchWriterStats {
    pub is_running: bool,
    pub events_queued: u64,
    pub events_written: u64,
    pub events_failed: u64,
    pub batches_written: u64,
    pub buffer_size: usize,
    #[serde(skip)]
    pub last_write_time: Option<Instant>,
    pub success_rate: f64,
    pub average_batch_size: f64,
}

impl BatchWriter {
    /// 创建新的批量写入器
    pub async fn new(config: &EventListenerConfig) -> Result<Self> {
        let config = Arc::new(config.clone());
        let event_storage = Arc::new(EventStorage::new(&config).await?);

        let batch_size = config.listener.batch_write.batch_size;
        let max_wait_duration = Duration::from_millis(config.listener.batch_write.max_wait_ms);
        let buffer_size = config.listener.batch_write.buffer_size;

        let (event_sender, event_receiver) = mpsc::unbounded_channel::<ParsedEvent>();
        let event_receiver = Arc::new(Mutex::new(event_receiver));

        info!(
            "🔧 初始化批量写入器，batch_size: {}, max_wait: {:?}, buffer_size: {}",
            batch_size, max_wait_duration, buffer_size
        );

        let max_retries = config.listener.max_retries;

        Ok(Self {
            config,
            event_storage,
            batch_size,
            max_wait_duration,
            buffer_size,
            is_running: Arc::new(AtomicBool::new(false)),
            event_buffer: Arc::new(Mutex::new(VecDeque::with_capacity(buffer_size))),
            events_queued: Arc::new(AtomicU64::new(0)),
            events_written: Arc::new(AtomicU64::new(0)),
            events_failed: Arc::new(AtomicU64::new(0)),
            batches_written: Arc::new(AtomicU64::new(0)),
            last_write_time: Arc::new(RwLock::new(None)),
            event_sender,
            event_receiver,
            retry_counts: Arc::new(Mutex::new(HashMap::new())),
            max_retries,
        })
    }

    /// 启动批量处理
    pub async fn start_batch_processing(&self) -> Result<()> {
        if self.is_running.load(Ordering::Relaxed) {
            warn!("批量写入器已在运行中");
            return Ok(());
        }

        self.is_running.store(true, Ordering::Relaxed);
        info!("🚀 启动批量写入处理");

        // 启动事件收集任务
        let collection_task = {
            let writer = self.clone();
            tokio::spawn(async move {
                writer.event_collection_loop().await;
            })
        };

        // 启动批量写入任务
        let batch_write_task = {
            let writer = self.clone();
            tokio::spawn(async move {
                writer.batch_write_loop().await;
            })
        };

        // 等待任务完成
        tokio::select! {
            _ = collection_task => {
                warn!("事件收集任务完成");
            }
            _ = batch_write_task => {
                warn!("批量写入任务完成");
            }
        }

        Ok(())
    }

    /// 停止批量处理并刷新缓冲区
    pub async fn stop(&self) -> Result<()> {
        info!("🛑 停止批量写入器");
        self.is_running.store(false, Ordering::Relaxed);

        // 刷新剩余的事件
        self.flush().await?;

        Ok(())
    }

    /// 提交事件到批量写入队列
    pub async fn submit_event(&self, event: ParsedEvent) -> Result<()> {
        if !self.is_running.load(Ordering::Relaxed) {
            return Err(EventListenerError::Persistence("批量写入器未运行".to_string()));
        }

        self.event_sender
            .send(event)
            .map_err(|_| EventListenerError::Persistence("事件提交失败：通道已关闭".to_string()))?;

        self.events_queued.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    /// 批量提交多个事件到写入队列
    ///
    /// 这个方法比多次调用 submit_event 更高效，因为它减少了通道操作的开销
    ///
    /// # 参数
    /// * `events` - 要提交的事件向量
    ///
    /// # 返回值
    /// 如果所有事件都成功提交则返回 Ok(())，否则返回第一个遇到的错误
    ///
    /// # 注意
    /// 如果在提交过程中遇到错误，已提交的事件不会回滚
    pub async fn submit_events(&self, events: Vec<ParsedEvent>) -> Result<()> {
        if events.is_empty() {
            return Ok(());
        }

        if !self.is_running.load(Ordering::Relaxed) {
            return Err(EventListenerError::Persistence("批量写入器未运行".to_string()));
        }

        let event_count = events.len();

        // 批量发送所有事件
        for event in events {
            self.event_sender
                .send(event)
                .map_err(|_| EventListenerError::Persistence("批量事件提交失败：通道已关闭".to_string()))?;
        }

        // 更新统计计数器
        self.events_queued.fetch_add(event_count as u64, Ordering::Relaxed);

        debug!("📦 批量提交{}个事件到写入队列", event_count);
        Ok(())
    }

    /// 事件收集循环
    async fn event_collection_loop(&self) {
        info!("📥 启动事件收集循环");

        let mut receiver = self.event_receiver.lock().await;

        while self.is_running.load(Ordering::Relaxed) {
            match timeout(Duration::from_millis(100), receiver.recv()).await {
                Ok(Some(event)) => {
                    // 将事件添加到缓冲区
                    {
                        let mut buffer = self.event_buffer.lock().await;

                        // 检查缓冲区容量
                        if buffer.len() >= self.buffer_size {
                            warn!("⚠️ 事件缓冲区已满，丢弃最旧的事件");
                            buffer.pop_front();
                        }

                        buffer.push_back(event);
                    }

                    debug!("📦 事件已添加到缓冲区");
                }
                Ok(None) => {
                    warn!("事件接收通道已关闭");
                    break;
                }
                Err(_) => {
                    // 超时，继续循环
                    continue;
                }
            }
        }

        info!("📥 事件收集循环已停止");
    }

    /// 批量写入循环
    async fn batch_write_loop(&self) {
        info!("💾 启动批量写入循环");
        info!(
            "📊 批量配置 - batch_size: {}, max_wait: {:?}",
            self.batch_size, self.max_wait_duration
        );

        // 使用较短的检查间隔，但基于时间窗口决定是否写入
        let check_interval = Duration::from_millis(std::cmp::min(1000, self.max_wait_duration.as_millis() as u64));
        let mut write_interval = interval(check_interval);

        // 记录上次写入时间，用于时间窗口判断
        let mut last_write_time = Instant::now();

        while self.is_running.load(Ordering::Relaxed) {
            write_interval.tick().await;

            // 检查是否需要写入
            let buffer_size = {
                let buffer = self.event_buffer.lock().await;
                buffer.len()
            };

            if buffer_size == 0 {
                continue;
            }

            let time_since_last_write = last_write_time.elapsed();

            // 满足以下任一条件就触发批量写入：
            // 1. 缓冲区达到批量大小阈值
            // 2. 有事件且等待时间超过最大等待时间
            let should_write = buffer_size >= self.batch_size || time_since_last_write >= self.max_wait_duration;

            if should_write {
                debug!(
                    "🔍 触发批量写入 - 缓冲区: {}/{}, 等待时间: {:?}/{:?}",
                    buffer_size, self.batch_size, time_since_last_write, self.max_wait_duration
                );

                if let Err(e) = self.write_batch().await {
                    error!("❌ 批量写入失败: {}", e);
                } else {
                    // 成功写入后重置时间
                    last_write_time = Instant::now();
                }
            }
        }

        info!("💾 批量写入循环已停止");
    }

    /// 执行批量写入
    async fn write_batch(&self) -> Result<()> {
        let batch = {
            let mut buffer = self.event_buffer.lock().await;
            if buffer.is_empty() {
                return Ok(());
            }

            // 提取批量事件
            let batch_size = std::cmp::min(self.batch_size, buffer.len());
            let mut batch = Vec::with_capacity(batch_size);

            for _ in 0..batch_size {
                if let Some(event) = buffer.pop_front() {
                    batch.push(event);
                }
            }

            batch
        };

        if batch.is_empty() {
            return Ok(());
        }

        let batch_size = batch.len();
        info!("📦 批量写入开始 - 事件数量: {}", batch_size);

        let start_time = Instant::now();

        // 执行批量写入
        match self.event_storage.write_batch(&batch).await {
            Ok(written_count) => {
                let duration = start_time.elapsed();

                // 更新统计信息
                self.events_written.fetch_add(written_count, Ordering::Relaxed);
                self.batches_written.fetch_add(1, Ordering::Relaxed);
                {
                    let mut last_write = self.last_write_time.write().await;
                    *last_write = Some(Instant::now());
                }

                info!(
                    "✅ 批量写入完成，写入: {}/{} 事件，耗时: {:?}",
                    written_count, batch_size, duration
                );
            }
            Err(e) => {
                // 更新失败统计
                self.events_failed.fetch_add(batch_size as u64, Ordering::Relaxed);

                error!("❌ 批量写入失败: {}", e);

                // 生成批次ID用于重试跟踪
                let batch_id = Uuid::new_v4().to_string();

                // 将失败的事件重新加入缓冲区（可选择性重试）
                if self.should_retry_batch_internal(&batch, &e, &batch_id).await {
                    // 添加指数退避延迟
                    let retry_counts = self.retry_counts.lock().await;
                    let current_retries = retry_counts.get(&batch_id).copied().unwrap_or(0);
                    drop(retry_counts);

                    let delay_ms = self.config.listener.retry_delay_ms * (2_u64.pow(current_retries));
                    let delay = std::cmp::min(delay_ms, 30000); // 最大延迟30秒

                    tokio::time::sleep(Duration::from_millis(delay)).await;

                    self.requeue_batch(batch).await;
                } else {
                    warn!("🚫 批次重试已放弃，丢弃 {} 个事件", batch.len());
                }

                return Err(e);
            }
        }

        Ok(())
    }

    /// 判断是否应该重试批量写入
    async fn should_retry_batch_internal(
        &self,
        batch: &[ParsedEvent],
        error: &EventListenerError,
        batch_id: &str,
    ) -> bool {
        // 根据错误类型决定是否重试
        let is_retryable_error = match error {
            EventListenerError::Database(_) => true,            // 数据库连接问题可重试
            EventListenerError::IO(_) => true,                  // IO错误可重试
            EventListenerError::WebSocket(_) => true,           // WebSocket连接问题可重试
            EventListenerError::SolanaRpc(_) => true,           // Solana RPC错误可重试
            EventListenerError::Network(_) => true,             // 网络错误可重试
            EventListenerError::Persistence(_) => true,         // 持久化错误可重试
            EventListenerError::EventParsing(_) => false,       // 解析错误不重试
            EventListenerError::DiscriminatorMismatch => false, // Discriminator不匹配不重试
            EventListenerError::Config(_) => false,             // 配置错误不重试
            EventListenerError::Checkpoint(_) => false,         // 检查点错误不重试
            EventListenerError::Metrics(_) => false,            // 指标错误不重试
            EventListenerError::Serialization(_) => false,      // 序列化错误不重试
            EventListenerError::Base64Decode(_) => false,       // Base64解码错误不重试
            EventListenerError::SolanaSDK(_) => false,          // Solana SDK错误不重试
            EventListenerError::Unknown(_) => false,            // 未知错误不重试
        };

        if !is_retryable_error {
            debug!("❌ 错误类型不可重试: {}", error);
            return false;
        }

        // 检查重试次数
        let mut retry_counts = self.retry_counts.lock().await;
        let current_retries = retry_counts.get(batch_id).copied().unwrap_or(0);

        if current_retries >= self.max_retries {
            warn!("⚠️ 批次 {} 已达到最大重试次数 {}", batch_id, self.max_retries);
            retry_counts.remove(batch_id);
            return false;
        }

        // 增加重试计数
        retry_counts.insert(batch_id.to_string(), current_retries + 1);

        info!(
            "🔄 批次 {} 将进行第 {} 次重试（批次大小: {}）",
            batch_id,
            current_retries + 1,
            batch.len()
        );

        true
    }

    /// 判断是否应该重试批量写入 (测试可见)
    #[cfg(test)]
    pub async fn should_retry_batch(&self, batch: &[ParsedEvent], error: &EventListenerError, batch_id: &str) -> bool {
        // 调用内部方法
        self.should_retry_batch_internal(batch, error, batch_id).await
    }

    /// 将批量事件重新加入队列
    async fn requeue_batch(&self, batch: Vec<ParsedEvent>) {
        warn!("🔄 重新排队 {} 个失败的事件", batch.len());

        let mut buffer = self.event_buffer.lock().await;

        // 将失败的事件添加到缓冲区前部（优先处理）
        for event in batch.into_iter().rev() {
            if buffer.len() >= self.buffer_size {
                // 如果缓冲区满了，丢弃最旧的事件
                buffer.pop_back();
            }
            buffer.push_front(event);
        }
    }

    /// 刷新所有缓冲区中的事件
    pub async fn flush(&self) -> Result<()> {
        info!("🚿 刷新批量写入缓冲区");

        let mut attempts = 0;
        const MAX_ATTEMPTS: u32 = 3;

        while attempts < MAX_ATTEMPTS {
            let buffer_size = {
                let buffer = self.event_buffer.lock().await;
                buffer.len()
            };

            if buffer_size == 0 {
                info!("✅ 缓冲区已清空");
                break;
            }

            info!(
                "💾 刷新剩余 {} 个事件 (尝试 {}/{})",
                buffer_size,
                attempts + 1,
                MAX_ATTEMPTS
            );

            match self.write_batch().await {
                Ok(()) => {
                    info!("✅ 刷新批量写入成功");
                }
                Err(e) => {
                    error!("❌ 刷新批量写入失败: {}", e);
                    attempts += 1;

                    if attempts >= MAX_ATTEMPTS {
                        return Err(e);
                    }

                    // 等待一段时间再重试
                    tokio::time::sleep(Duration::from_millis(1000)).await;
                }
            }
        }

        Ok(())
    }

    /// 检查批量写入器是否健康
    pub async fn is_healthy(&self) -> bool {
        let is_running = self.is_running.load(Ordering::Relaxed);
        let buffer_size = {
            let buffer = self.event_buffer.lock().await;
            buffer.len()
        };

        // 检查是否运行正常且缓冲区未过载
        is_running && buffer_size < self.buffer_size
    }

    /// 获取统计信息
    pub async fn get_stats(&self) -> BatchWriterStats {
        let events_written = self.events_written.load(Ordering::Relaxed);
        let events_failed = self.events_failed.load(Ordering::Relaxed);
        let batches_written = self.batches_written.load(Ordering::Relaxed);
        let total_events = events_written + events_failed;

        let success_rate = if total_events > 0 {
            events_written as f64 / total_events as f64
        } else {
            1.0
        };

        let average_batch_size = if batches_written > 0 {
            events_written as f64 / batches_written as f64
        } else {
            0.0
        };

        let buffer_size = {
            let buffer = self.event_buffer.lock().await;
            buffer.len()
        };

        BatchWriterStats {
            is_running: self.is_running.load(Ordering::Relaxed),
            events_queued: self.events_queued.load(Ordering::Relaxed),
            events_written,
            events_failed,
            batches_written,
            buffer_size,
            last_write_time: *self.last_write_time.read().await,
            success_rate,
            average_batch_size,
        }
    }

    /// 重置统计信息
    pub async fn reset_stats(&self) {
        self.events_queued.store(0, Ordering::Relaxed);
        self.events_written.store(0, Ordering::Relaxed);
        self.events_failed.store(0, Ordering::Relaxed);
        self.batches_written.store(0, Ordering::Relaxed);
        {
            let mut last_write = self.last_write_time.write().await;
            *last_write = None;
        }
        info!("📊 批量写入器统计信息已重置");
    }
}

impl Clone for BatchWriter {
    fn clone(&self) -> Self {
        Self {
            config: Arc::clone(&self.config),
            event_storage: Arc::clone(&self.event_storage),
            batch_size: self.batch_size,
            max_wait_duration: self.max_wait_duration,
            buffer_size: self.buffer_size,
            is_running: Arc::clone(&self.is_running),
            event_buffer: Arc::clone(&self.event_buffer),
            events_queued: Arc::clone(&self.events_queued),
            events_written: Arc::clone(&self.events_written),
            events_failed: Arc::clone(&self.events_failed),
            batches_written: Arc::clone(&self.batches_written),
            last_write_time: Arc::clone(&self.last_write_time),
            event_sender: self.event_sender.clone(),
            event_receiver: Arc::clone(&self.event_receiver),
            retry_counts: Arc::clone(&self.retry_counts),
            max_retries: self.max_retries,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{event_parser::TokenCreationEventData, ParsedEvent};
    use solana_sdk::pubkey::Pubkey;

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
                retry_delay_ms: 1000,
                signature_cache_size: 10000,
                checkpoint_save_interval_secs: 60,
                backoff: crate::config::settings::BackoffConfig::default(),
                batch_write: crate::config::settings::BatchWriteConfig {
                    batch_size: 5,
                    max_wait_ms: 1000,
                    buffer_size: 100,
                    concurrent_writers: 2,
                },
            },
            monitoring: crate::config::settings::MonitoringConfig {
                metrics_interval_secs: 60,
                enable_performance_monitoring: true,
                health_check_interval_secs: 30,
            },
        }
    }

    fn create_test_event() -> ParsedEvent {
        ParsedEvent::TokenCreation(TokenCreationEventData {
            mint_address: Pubkey::new_unique().to_string(),
            name: "Test Token".to_string(),
            symbol: "TEST".to_string(),
            uri: "https://example.com/metadata.json".to_string(),
            decimals: 9,
            supply: 1000000,
            creator: Pubkey::new_unique().to_string(),
            has_whitelist: false,
            whitelist_deadline: 0,
            created_at: 1234567890,
            signature: "test_signature".to_string(),
            slot: 12345,
        })
    }

    #[tokio::test]
    async fn test_batch_writer_creation() {
        let config = create_test_config();
        let writer = BatchWriter::new(&config).await.unwrap();

        let stats = writer.get_stats().await;
        assert!(!stats.is_running);
        assert_eq!(stats.events_queued, 0);
        assert_eq!(stats.buffer_size, 0);
    }

    #[tokio::test]
    async fn test_submit_event() {
        let config = create_test_config();
        let writer = BatchWriter::new(&config).await.unwrap();

        // 启动批量写入器
        writer.is_running.store(true, Ordering::Relaxed);

        let event = create_test_event();
        writer.submit_event(event).await.unwrap();

        let stats = writer.get_stats().await;
        assert_eq!(stats.events_queued, 1);
    }

    #[tokio::test]
    async fn test_submit_events() {
        let config = create_test_config();
        let writer = BatchWriter::new(&config).await.unwrap();

        // 启动批量写入器
        writer.is_running.store(true, Ordering::Relaxed);

        // 创建多个测试事件
        let events = vec![create_test_event(), create_test_event(), create_test_event()];
        let event_count = events.len();

        writer.submit_events(events).await.unwrap();

        let stats = writer.get_stats().await;
        assert_eq!(stats.events_queued, event_count as u64);
    }

    #[tokio::test]
    async fn test_submit_empty_events() {
        let config = create_test_config();
        let writer = BatchWriter::new(&config).await.unwrap();

        // 启动批量写入器
        writer.is_running.store(true, Ordering::Relaxed);

        // 提交空的事件向量应该成功但不改变统计
        let events: Vec<ParsedEvent> = vec![];
        writer.submit_events(events).await.unwrap();

        let stats = writer.get_stats().await;
        assert_eq!(stats.events_queued, 0);
    }

    #[tokio::test]
    async fn test_retry_logic() {
        let config = create_test_config();
        let writer = BatchWriter::new(&config).await.unwrap();

        let test_batch = vec![create_test_event()];
        let batch_id = "test-batch-123";

        // 测试可重试错误
        let retryable_error = EventListenerError::Persistence("连接超时".to_string());
        assert!(writer.should_retry_batch(&test_batch, &retryable_error, batch_id).await);

        // 测试不可重试错误
        let non_retryable_error = EventListenerError::EventParsing("解析失败".to_string());
        assert!(
            !writer
                .should_retry_batch(&test_batch, &non_retryable_error, batch_id)
                .await
        );

        // 测试重试次数限制
        let database_error = EventListenerError::Persistence("连接失败".to_string());
        let batch_id_limit = "test-batch-limit";

        // 第一次重试应该成功
        assert!(
            writer
                .should_retry_batch(&test_batch, &database_error, batch_id_limit)
                .await
        );

        // 模拟达到最大重试次数
        {
            let mut retry_counts = writer.retry_counts.lock().await;
            retry_counts.insert(batch_id_limit.to_string(), writer.max_retries);
        }

        // 达到最大重试次数后应该拒绝重试
        assert!(
            !writer
                .should_retry_batch(&test_batch, &database_error, batch_id_limit)
                .await
        );
    }

    #[tokio::test]
    async fn test_requeue_batch() {
        let config = create_test_config();
        let writer = BatchWriter::new(&config).await.unwrap();

        let test_events = vec![create_test_event(), create_test_event(), create_test_event()];

        // 重新排队事件
        writer.requeue_batch(test_events.clone()).await;

        // 验证事件已添加到缓冲区
        let buffer_size = {
            let buffer = writer.event_buffer.lock().await;
            buffer.len()
        };
        assert_eq!(buffer_size, test_events.len());
    }

    #[tokio::test]
    async fn test_batch_writer_stats() {
        let config = create_test_config();
        let writer = BatchWriter::new(&config).await.unwrap();

        // 模拟一些统计数据
        writer.events_written.store(10, Ordering::Relaxed);
        writer.events_failed.store(2, Ordering::Relaxed);
        writer.batches_written.store(3, Ordering::Relaxed);

        let stats = writer.get_stats().await;
        assert_eq!(stats.events_written, 10);
        assert_eq!(stats.events_failed, 2);
        assert_eq!(stats.batches_written, 3);
        assert_eq!(stats.success_rate, 10.0 / 12.0);
        assert_eq!(stats.average_batch_size, 10.0 / 3.0);
    }

    #[tokio::test]
    async fn test_is_healthy() {
        let config = create_test_config();
        let writer = BatchWriter::new(&config).await.unwrap();

        // 未运行时不健康
        assert!(!writer.is_healthy().await);

        // 运行时健康
        writer.is_running.store(true, Ordering::Relaxed);
        assert!(writer.is_healthy().await);
    }

    #[tokio::test]
    async fn test_reset_stats() {
        let config = create_test_config();
        let writer = BatchWriter::new(&config).await.unwrap();

        // 设置一些统计数据
        writer.events_written.store(10, Ordering::Relaxed);
        writer.events_failed.store(2, Ordering::Relaxed);

        writer.reset_stats().await;

        let stats = writer.get_stats().await;
        assert_eq!(stats.events_written, 0);
        assert_eq!(stats.events_failed, 0);
    }
}
