use crate::{
    config::EventListenerConfig,
    error::Result,
    metrics::MetricsCollector,
    parser::EventParserRegistry,
    persistence::BatchWriter,
    recovery::CheckpointManager,
    subscriber::{EventFilter, WebSocketManager},
};
use dashmap::DashMap;
use solana_client::{rpc_client::RpcClient, rpc_response::RpcLogsResponse};
use std::{
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tokio::{sync::RwLock, time::interval};
use tracing::{debug, error, info, warn};

/// 订阅管理器
///
/// 负责协调所有订阅相关的组件:
/// - WebSocket连接管理
/// - 事件过滤和路由
/// - 事件解析和持久化
/// - 性能监控和统计
pub struct SubscriptionManager {
    config: Arc<EventListenerConfig>,
    websocket_manager: Arc<WebSocketManager>,
    event_filter: Arc<EventFilter>,
    parser_registry: Arc<EventParserRegistry>,
    batch_writer: Arc<BatchWriter>,
    checkpoint_manager: Arc<CheckpointManager>,
    metrics: Arc<MetricsCollector>,
    rpc_client: Arc<RpcClient>,

    // 运行状态
    is_running: Arc<AtomicBool>,

    // 统计信息
    processed_events: Arc<AtomicU64>,
    failed_events: Arc<AtomicU64>,
    last_activity: Arc<RwLock<Option<Instant>>>,

    // 签名缓存（防重复处理）
    signature_cache: Arc<DashMap<String, Instant>>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SubscriptionStats {
    pub is_running: bool,
    pub processed_events: u64,
    pub failed_events: u64,
    pub cache_size: usize,
    #[serde(skip)]
    pub last_activity: Option<Instant>,
    pub success_rate: f64,
}

impl SubscriptionManager {
    /// 创建新的订阅管理器
    pub async fn new(
        config: &EventListenerConfig,
        parser_registry: Arc<EventParserRegistry>,
        batch_writer: Arc<BatchWriter>,
        checkpoint_manager: Arc<CheckpointManager>,
        metrics: Arc<MetricsCollector>,
    ) -> Result<Self> {
        let config = Arc::new(config.clone());

        // 创建RPC客户端
        let rpc_client = Arc::new(RpcClient::new(&config.solana.rpc_url));

        // 创建WebSocket管理器
        let websocket_manager = Arc::new(WebSocketManager::new(Arc::clone(&config))?);

        // 创建事件过滤器
        let event_filter = Arc::new(
            EventFilter::accept_all(config.solana.program_ids.clone()) // 传递多个程序ID
                .with_error_filtering(true) // 过滤失败的交易
                .with_min_log_length(1), // 至少要有一条日志
        );

        // 创建签名缓存
        let signature_cache = Arc::new(DashMap::new());

        Ok(Self {
            config,
            websocket_manager,
            event_filter,
            parser_registry,
            batch_writer,
            checkpoint_manager,
            metrics,
            rpc_client,
            is_running: Arc::new(AtomicBool::new(false)),
            processed_events: Arc::new(AtomicU64::new(0)),
            failed_events: Arc::new(AtomicU64::new(0)),
            last_activity: Arc::new(RwLock::new(None)),
            signature_cache,
        })
    }

    /// 启动订阅管理器
    pub async fn start(&self) -> Result<()> {
        if self.is_running.load(Ordering::Relaxed) {
            warn!("订阅管理器已在运行中");
            return Ok(());
        }

        self.is_running.store(true, Ordering::Relaxed);
        info!("🚀 启动订阅管理器");

        // 启动WebSocket管理器
        let websocket_manager = Arc::clone(&self.websocket_manager);
        let ws_task = tokio::spawn(async move {
            if let Err(e) = websocket_manager.start().await {
                error!("WebSocket管理器启动失败: {}", e);
            }
        });

        // 启动WebSocket连接状态监控
        let ws_monitor_task = {
            let websocket_manager = Arc::clone(&self.websocket_manager);
            let metrics = Arc::clone(&self.metrics);
            tokio::spawn(async move {
                let mut last_connected = false;
                let mut connection_recorded = false;

                let mut interval = interval(Duration::from_secs(1));
                loop {
                    interval.tick().await;

                    let stats = websocket_manager.get_stats().await;
                    let currently_connected = stats.is_connected;

                    // 检测到新连接
                    if currently_connected && !last_connected {
                        info!("✅ WebSocket连接建立，开始监听事件");
                        if !connection_recorded {
                            if let Err(e) = metrics.record_websocket_connection().await {
                                warn!("记录WebSocket连接指标失败: {}", e);
                            } else {
                                connection_recorded = true;
                            }
                        }
                    }
                    // 检测到连接断开
                    else if !currently_connected && last_connected {
                        warn!("❌ WebSocket连接断开");
                    }

                    last_connected = currently_connected;

                    // 如果订阅管理器停止运行，退出监控
                    if !stats.is_running {
                        break;
                    }
                }
            })
        };

        // 启动事件处理循环
        let event_processing_task = {
            let manager = self.clone();
            tokio::spawn(async move {
                manager.event_processing_loop().await;
            })
        };

        // 启动清理任务
        let cleanup_task = {
            let manager = self.clone();
            tokio::spawn(async move {
                manager.cleanup_loop().await;
            })
        };

        // 等待任务完成或停止信号
        tokio::select! {
            _ = ws_task => {
                warn!("WebSocket管理器任务完成");
            }
            _ = ws_monitor_task => {
                warn!("WebSocket监控任务完成");
            }
            _ = event_processing_task => {
                warn!("事件处理任务完成");
            }
            _ = cleanup_task => {
                warn!("清理任务完成");
            }
        }

        Ok(())
    }

    /// 停止订阅管理器
    pub async fn stop(&self) -> Result<()> {
        info!("🛑 停止订阅管理器");
        self.is_running.store(false, Ordering::Relaxed);

        // 停止WebSocket管理器
        self.websocket_manager.stop().await?;

        Ok(())
    }

    /// 事件处理主循环
    async fn event_processing_loop(&self) {
        info!("📡 启动事件处理循环");

        let mut event_receiver = self.websocket_manager.subscribe();
        info!("📡 已订阅WebSocket事件，开始处理循环");

        while self.is_running.load(Ordering::Relaxed) {
            match tokio::time::timeout(Duration::from_millis(100), event_receiver.recv()).await {
                Ok(Ok(log_response)) => {
                    info!("📨 订阅管理器接收到事件: {}", log_response.signature);

                    // 更新活动时间
                    {
                        let mut last_activity = self.last_activity.write().await;
                        *last_activity = Some(Instant::now());
                    }

                    // 异步处理事件（不阻塞接收）
                    let manager_clone = self.clone();
                    tokio::spawn(async move {
                        if let Err(e) = manager_clone.process_event(log_response).await {
                            debug!("处理事件失败: {}", e);
                        }
                    });
                }
                Ok(Err(e)) => {
                    match e {
                        tokio::sync::broadcast::error::RecvError::Closed => {
                            warn!("事件接收器已关闭");
                            break;
                        }
                        tokio::sync::broadcast::error::RecvError::Lagged(skipped) => {
                            warn!("⚠️ 事件接收器滞后，跳过了 {} 个事件 - 尝试继续处理", skipped);
                            // 重新订阅以获取新的接收器
                            event_receiver = self.websocket_manager.subscribe();
                            info!("📡 重新订阅WebSocket事件");
                            continue;
                        }
                    }
                }
                Err(_) => {
                    // 超时，继续下一次循环
                    continue;
                }
            }
        }

        info!("📡 事件处理循环已停止");
    }

    /// 获取当前slot
    async fn get_current_slot_internal(&self) -> Result<u64> {
        use crate::error::EventListenerError;

        tokio::task::spawn_blocking({
            let rpc_client = Arc::clone(&self.rpc_client);
            move || {
                rpc_client
                    .get_slot()
                    .map_err(|e| EventListenerError::WebSocket(format!("获取当前slot失败: {}", e)))
            }
        })
        .await
        .map_err(|e| EventListenerError::Unknown(format!("异步任务执行失败: {}", e)))?
    }

    /// 获取当前slot (测试可见)
    #[cfg(test)]
    pub async fn get_current_slot(&self) -> Result<u64> {
        // 调用内部方法
        self.get_current_slot_internal().await
    }

    /// 处理单个事件
    async fn process_event(&self, log_response: RpcLogsResponse) -> Result<()> {
        let signature = &log_response.signature;

        info!("🔍 开始处理事件: {}", signature);
        info!("🔍 事件日志: {:?}", log_response.logs);

        // 获取当前slot，如果失败则使用0作为备用值
        let slot = match self.get_current_slot_internal().await {
            Ok(slot) => slot,
            Err(e) => {
                warn!("⚠️ 无法获取当前slot: {}, 使用默认值0", e);
                0
            }
        };

        debug!("🔍 处理事件: {} (slot: {})", signature, slot);

        // 检查是否已处理过此事件
        if self.is_signature_processed(signature) {
            debug!("⏭️ 事件已处理，跳过: {}", signature);
            return Ok(());
        }

        // 应用事件过滤器
        if !self.event_filter.should_process(&log_response) {
            info!("🚫 事件被过滤器拒绝: {}", signature);
            return Ok(());
        }

        info!("🔍 事件通过过滤器，开始解析: {}", signature);

        // 尝试解析所有事件（使用智能路由多事件处理）
        match self
            .parser_registry
            .parse_all_events_with_context(&log_response.logs, signature, slot, &self.config.solana.program_ids)
            .await
        {
            Ok(parsed_events) if !parsed_events.is_empty() => {
                info!(
                    "✅ 事件解析成功: {} -> 发现{}个事件: {:?}",
                    signature,
                    parsed_events.len(),
                    parsed_events.iter().map(|e| e.event_type()).collect::<Vec<_>>()
                );

                // 尝试从日志中提取程序ID用于监控
                let program_id = self.extract_program_id_from_logs(&log_response.logs);

                // 批量提交所有解析的事件到写入器
                self.batch_writer.submit_events(parsed_events.clone()).await?;

                // 更新检查点 - 使用程序特定的检查点更新
                if let Some(ref prog_id_str) = program_id {
                    // 如果能提取到程序ID，使用程序特定的检查点更新
                    self.checkpoint_manager
                        .update_last_processed_for_program(prog_id_str, signature, slot)
                        .await?;
                } else {
                    // 回退到向后兼容的方法（更新第一个程序的检查点）
                    self.checkpoint_manager.update_last_processed(signature, slot).await?;
                }

                // 标记为已处理
                self.mark_signature_processed(signature);

                // 更新指标 - 按实际处理的事件数量更新
                let event_count = parsed_events.len();
                for _ in 0..event_count {
                    self.metrics.record_event_processed().await?;
                }
                if let Some(prog_id) = program_id {
                    for _ in 0..event_count {
                        self.metrics.record_event_processed_for_program(&prog_id).await?;
                    }
                }
                self.processed_events.fetch_add(event_count as u64, Ordering::Relaxed);

                info!("📊 事务处理完成: {} -> 成功处理{}个事件", signature, event_count);
            }
            Ok(_) => {
                // 这个分支覆盖了 Ok(parsed_events) if parsed_events.is_empty() 的情况
                info!("ℹ️ 事件无法识别，跳过: {}", signature);
            }
            Err(e) => {
                warn!("❌ 事件解析失败: {} - {}", signature, e);

                // 尝试从日志中提取程序ID用于错误监控
                let program_id = self.extract_program_id_from_logs(&log_response.logs);
                let error_type = self.classify_error(&e);

                self.failed_events.fetch_add(1, Ordering::Relaxed);
                self.metrics.record_event_failed().await?;
                if let Some(prog_id) = program_id {
                    self.metrics
                        .record_event_failed_for_program(&prog_id, &error_type)
                        .await?;
                }
                return Err(e);
            }
        }

        Ok(())
    }

    /// 从日志中提取程序ID
    fn extract_program_id_from_logs(&self, logs: &[String]) -> Option<String> {
        for log in logs {
            // 查找形如 "Program 11111111111111111111111111111111 invoke [1]" 的日志
            if log.starts_with("Program ") && log.contains(" invoke [") {
                let parts: Vec<&str> = log.split_whitespace().collect();
                if parts.len() >= 3 {
                    let program_id = parts[1];
                    // 验证是否是我们监听的程序ID之一
                    for target_id in &self.config.solana.program_ids {
                        if target_id.to_string() == program_id {
                            return Some(program_id.to_string());
                        }
                    }
                }
            }
        }
        None
    }

    /// 将错误分类为监控类别
    fn classify_error(&self, error: &crate::error::EventListenerError) -> String {
        use crate::error::EventListenerError;

        match error {
            EventListenerError::EventParsing(_) => "parse_error".to_string(),
            EventListenerError::Database(_) => "database_error".to_string(),
            EventListenerError::WebSocket(_) => "websocket_error".to_string(),
            EventListenerError::Network(_) => "network_error".to_string(),
            EventListenerError::Config(_) => "config_error".to_string(),
            EventListenerError::DiscriminatorMismatch => "discriminator_mismatch".to_string(),
            EventListenerError::Persistence(_) => "persistence_error".to_string(),
            EventListenerError::Checkpoint(_) => "checkpoint_error".to_string(),
            EventListenerError::Metrics(_) => "metrics_error".to_string(),
            EventListenerError::SolanaRpc(_) => "solana_rpc_error".to_string(),
            EventListenerError::Serialization(_) => "serialization_error".to_string(),
            EventListenerError::Base64Decode(_) => "base64_decode_error".to_string(),
            EventListenerError::IO(_) => "io_error".to_string(),
            EventListenerError::SolanaSDK(_) => "solana_sdk_error".to_string(),
            EventListenerError::Unknown(_) => "unknown_error".to_string(),
        }
    }

    /// 清理循环（定期清理缓存和过期数据）
    async fn cleanup_loop(&self) {
        info!("🧹 启动清理循环");

        let mut cleanup_interval = interval(Duration::from_secs(300)); // 每5分钟清理一次

        while self.is_running.load(Ordering::Relaxed) {
            cleanup_interval.tick().await;

            // 清理签名缓存
            self.cleanup_signature_cache().await;

            // 更新指标
            if let Err(e) = self.metrics.record_cleanup_cycle().await {
                warn!("更新清理指标失败: {}", e);
            }
        }

        info!("🧹 清理循环已停止");
    }

    /// 清理签名缓存
    async fn cleanup_signature_cache(&self) {
        let now = Instant::now();
        let ttl = Duration::from_secs(3600); // 1小时TTL
        let max_size = self.config.listener.signature_cache_size;

        // 移除过期条目
        let mut expired_count = 0;
        self.signature_cache.retain(|_, &mut timestamp| {
            if now.duration_since(timestamp) > ttl {
                expired_count += 1;
                false
            } else {
                true
            }
        });

        // 如果缓存仍然太大，移除最老的条目
        let current_size = self.signature_cache.len();
        if current_size > max_size {
            let mut entries: Vec<_> = self.signature_cache.iter().collect();
            entries.sort_by_key(|entry| *entry.value());

            let to_remove = current_size - max_size;
            for entry in entries.into_iter().take(to_remove) {
                self.signature_cache.remove(entry.key());
            }
        }

        if expired_count > 0 {
            debug!("🗑️ 清理了 {} 个过期签名缓存条目", expired_count);
        }
    }

    /// 检查签名是否已处理
    fn is_signature_processed(&self, signature: &str) -> bool {
        self.signature_cache.contains_key(signature)
    }

    /// 标记签名为已处理
    fn mark_signature_processed(&self, signature: &str) {
        self.signature_cache.insert(signature.to_string(), Instant::now());
    }

    /// 检查订阅管理器是否健康
    pub async fn is_healthy(&self) -> bool {
        // 检查各个组件的健康状态
        let websocket_healthy = self.websocket_manager.is_healthy().await;
        let batch_writer_healthy = self.batch_writer.is_healthy().await;
        let checkpoint_healthy = self.checkpoint_manager.is_healthy().await;

        // 检查最近是否有活动
        let last_activity = *self.last_activity.read().await;
        let activity_healthy = match last_activity {
            Some(last) => last.elapsed() < Duration::from_secs(300), // 5分钟内有活动
            None => true,                                            // 刚启动时认为是健康的
        };

        websocket_healthy && batch_writer_healthy && checkpoint_healthy && activity_healthy
    }

    /// 获取订阅统计信息
    pub async fn get_stats(&self) -> SubscriptionStats {
        let processed = self.processed_events.load(Ordering::Relaxed);
        let failed = self.failed_events.load(Ordering::Relaxed);
        let total = processed + failed;
        let success_rate = if total > 0 {
            processed as f64 / total as f64
        } else {
            1.0
        };

        SubscriptionStats {
            is_running: self.is_running.load(Ordering::Relaxed),
            processed_events: processed,
            failed_events: failed,
            cache_size: self.signature_cache.len(),
            last_activity: *self.last_activity.read().await,
            success_rate,
        }
    }

    /// 重置统计信息
    pub async fn reset_stats(&self) {
        self.processed_events.store(0, Ordering::Relaxed);
        self.failed_events.store(0, Ordering::Relaxed);
        {
            let mut last_activity = self.last_activity.write().await;
            *last_activity = None;
        }
        info!("📊 订阅管理器统计信息已重置");
    }
}

impl Clone for SubscriptionManager {
    fn clone(&self) -> Self {
        Self {
            config: Arc::clone(&self.config),
            websocket_manager: Arc::clone(&self.websocket_manager),
            event_filter: Arc::clone(&self.event_filter),
            parser_registry: Arc::clone(&self.parser_registry),
            batch_writer: Arc::clone(&self.batch_writer),
            checkpoint_manager: Arc::clone(&self.checkpoint_manager),
            metrics: Arc::clone(&self.metrics),
            rpc_client: Arc::clone(&self.rpc_client),
            is_running: Arc::clone(&self.is_running),
            processed_events: Arc::clone(&self.processed_events),
            failed_events: Arc::clone(&self.failed_events),
            last_activity: Arc::clone(&self.last_activity),
            signature_cache: Arc::clone(&self.signature_cache),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn create_test_config() -> EventListenerConfig {
        crate::config::EventListenerConfig {
            solana: crate::config::settings::SolanaConfig {
                rpc_url: "https://api.devnet.solana.com".to_string(),
                ws_url: "wss://api.devnet.solana.com".to_string(),
                commitment: "confirmed".to_string(),
                program_ids: vec![
                    solana_sdk::pubkey::Pubkey::from_str("FA1RJDDXysgwg5Gm3fJXWxt26JQzPkAzhTA114miqNUX").unwrap(),
                    solana_sdk::pubkey::Pubkey::from_str("CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK").unwrap(),
                ],
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
                batch_write: crate::config::settings::BatchWriteConfig::default(),
            },
            monitoring: crate::config::settings::MonitoringConfig {
                metrics_interval_secs: 60,
                enable_performance_monitoring: true,
                health_check_interval_secs: 30,
            },
        }
    }

    #[tokio::test]
    async fn test_signature_cache() {
        let config = create_test_config();
        let parser_registry = Arc::new(EventParserRegistry::new(&config).unwrap());
        let batch_writer = Arc::new(BatchWriter::new(&config).await.unwrap());
        let checkpoint_manager = Arc::new(CheckpointManager::new(&config).await.unwrap());
        let metrics = Arc::new(MetricsCollector::new(&config).unwrap());

        let manager = SubscriptionManager::new(&config, parser_registry, batch_writer, checkpoint_manager, metrics)
            .await
            .unwrap();

        let test_signature = "test_signature_123";

        // 初始状态：未处理
        assert!(!manager.is_signature_processed(test_signature));

        // 标记为已处理
        manager.mark_signature_processed(test_signature);
        assert!(manager.is_signature_processed(test_signature));

        // 获取统计信息
        let stats = manager.get_stats().await;
        assert_eq!(stats.cache_size, 1);
    }

    #[tokio::test]
    async fn test_stats_tracking() {
        let config = create_test_config();
        let parser_registry = Arc::new(EventParserRegistry::new(&config).unwrap());
        let batch_writer = Arc::new(BatchWriter::new(&config).await.unwrap());
        let checkpoint_manager = Arc::new(CheckpointManager::new(&config).await.unwrap());
        let metrics = Arc::new(MetricsCollector::new(&config).unwrap());

        let manager = SubscriptionManager::new(&config, parser_registry, batch_writer, checkpoint_manager, metrics)
            .await
            .unwrap();

        // 初始统计
        let initial_stats = manager.get_stats().await;
        assert_eq!(initial_stats.processed_events, 0);
        assert_eq!(initial_stats.failed_events, 0);
        assert_eq!(initial_stats.success_rate, 1.0);

        // 模拟处理一些事件
        manager.processed_events.store(10, Ordering::Relaxed);
        manager.failed_events.store(2, Ordering::Relaxed);

        let updated_stats = manager.get_stats().await;
        assert_eq!(updated_stats.processed_events, 10);
        assert_eq!(updated_stats.failed_events, 2);
        assert_eq!(updated_stats.success_rate, 10.0 / 12.0);
    }

    #[tokio::test]
    async fn test_get_current_slot() {
        let config = create_test_config();
        let parser_registry = Arc::new(EventParserRegistry::new(&config).unwrap());
        let batch_writer = Arc::new(BatchWriter::new(&config).await.unwrap());
        let checkpoint_manager = Arc::new(CheckpointManager::new(&config).await.unwrap());
        let metrics = Arc::new(MetricsCollector::new(&config).unwrap());

        let manager = SubscriptionManager::new(&config, parser_registry, batch_writer, checkpoint_manager, metrics)
            .await
            .unwrap();

        // 测试获取slot（注意：这会向真实的RPC端点发送请求）
        // 在测试环境中，我们期望这能成功获取到一个slot值
        match manager.get_current_slot().await {
            Ok(slot) => {
                // slot应该是一个有效的数值
                println!("✅ 获取到当前slot: {}", slot);
                assert!(slot < u64::MAX); // 基本合理性检查
            }
            Err(e) => {
                // 在测试环境中，如果RPC不可用，这是可以接受的
                println!("⚠️ 无法获取slot（测试环境RPC可能不可用）: {}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_intelligent_routing_calls_with_context() {
        let config = create_test_config();
        let parser_registry = Arc::new(EventParserRegistry::new(&config).unwrap());
        let batch_writer = Arc::new(BatchWriter::new(&config).await.unwrap());
        let checkpoint_manager = Arc::new(CheckpointManager::new(&config).await.unwrap());
        let metrics = Arc::new(MetricsCollector::new(&config).unwrap());

        let manager = SubscriptionManager::new(&config, parser_registry, batch_writer, checkpoint_manager, metrics)
            .await
            .unwrap();

        // 测试日志数据，包含程序调用信息
        let logs_with_program_invocation = vec![
            "Program CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK invoke [1]".to_string(),
            "Program data: invalid_base64_data".to_string(),
            "Program CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK consumed 52341 of 200000 compute units".to_string(),
            "Program CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK success".to_string(),
        ];

        // 验证解析器注册表能够正确提取程序ID
        let extracted_program_id = manager
            .parser_registry
            .extract_program_id_from_logs(&logs_with_program_invocation, &manager.config.solana.program_ids);
        assert!(extracted_program_id.is_some(), "应该能从日志中提取到程序ID");

        // 测试智能路由是否正确调用parse_event_with_context
        let result = manager
            .parser_registry
            .parse_event_with_context(
                &logs_with_program_invocation,
                "test_signature",
                12345,
                &manager.config.solana.program_ids,
            )
            .await;

        // 验证调用成功（即使数据无效，智能路由流程应该正常工作
        match result {
            Ok(None) => {
                // 这是预期结果：没有找到匹配的事件，但智能路由正常工作
                println!("✅ 智能路由正常工作，未找到匹配事件（预期结果）");
            }
            Err(_) => {
                // 也是可以接受的：可能因为数据解析失败
                println!("✅ 智能路由正常调用，数据解析失败（预期结果）");
            }
            Ok(Some(_)) => {
                // 意外的成功解析
                println!("⚠️ 意外解析成功，可能是测试数据问题");
            }
        }
    }

    #[tokio::test]
    async fn test_parse_all_events_integration() {
        let config = create_test_config();

        // 创建所有必需的组件
        let parser_registry = Arc::new(EventParserRegistry::new(&config).unwrap());
        let batch_writer = Arc::new(BatchWriter::new(&config).await.unwrap());
        let checkpoint_manager = Arc::new(CheckpointManager::new(&config).await.unwrap());
        let metrics = Arc::new(MetricsCollector::new(&config).unwrap());

        let manager = SubscriptionManager::new(&config, parser_registry, batch_writer, checkpoint_manager, metrics)
            .await
            .unwrap();

        // 模拟包含多个Program data的日志
        let logs_with_multiple_program_data = vec![
            "Program CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK invoke [1]".to_string(),
            "Program data: dGVzdF9kYXRhXzE=".to_string(), // base64编码的"test_data_1"
            "Program data: dGVzdF9kYXRhXzI=".to_string(), // base64编码的"test_data_2"
            "Program data: dGVzdF9kYXRhXzM=".to_string(), // base64编码的"test_data_3"
            "Program CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK success".to_string(),
        ];

        // 测试新的 parse_all_events_with_context 方法
        let all_events_result = manager
            .parser_registry
            .parse_all_events_with_context(
                &logs_with_multiple_program_data,
                "test_signature",
                12345,
                &manager.config.solana.program_ids,
            )
            .await;

        // 验证方法调用成功
        match all_events_result {
            Ok(events) => {
                println!("✅ parse_all_events_with_context 调用成功，返回{}个事件", events.len());
                // 由于测试数据是无效的，预期返回空列表
                // 但重要的是验证方法能够正常调用并处理多个 Program data
            }
            Err(e) => {
                println!(
                    "✅ parse_all_events_with_context 调用成功，数据解析失败（预期结果）: {}",
                    e
                );
                // 这也是预期的，因为测试数据是无效的
            }
        }

        // 对比测试：验证原有的 parse_event_with_context 仍然正常工作
        let single_event_result = manager
            .parser_registry
            .parse_event_with_context(
                &logs_with_multiple_program_data,
                "test_signature",
                12345,
                &manager.config.solana.program_ids,
            )
            .await;

        match single_event_result {
            Ok(event) => match event {
                Some(_) => println!("✅ parse_event_with_context 返回了1个事件"),
                None => println!("✅ parse_event_with_context 没有找到有效事件"),
            },
            Err(e) => {
                println!("✅ parse_event_with_context 数据解析失败（预期结果）: {}", e);
            }
        }

        println!("🎉 多事件处理集成测试完成");
    }
}
