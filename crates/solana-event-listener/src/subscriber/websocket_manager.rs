use crate::{
    config::EventListenerConfig,
    error::{EventListenerError, Result},
};
use backoff::{future::retry, ExponentialBackoff};
use futures::StreamExt;
use solana_client::{
    nonblocking::pubsub_client::PubsubClient,
    rpc_config::{RpcTransactionLogsConfig, RpcTransactionLogsFilter},
    rpc_response::RpcLogsResponse,
};
use solana_sdk::{commitment_config::CommitmentConfig, pubkey::Pubkey};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tokio::{
    sync::{broadcast, RwLock},
    time::sleep,
};
use tracing::{debug, error, info, warn};

/// WebSocket连接管理器
/// 
/// 负责:
/// - 维护与Solana WebSocket的持久连接
/// - 实现断线重连和指数退避
/// - 处理订阅和取消订阅
/// - 提供连接状态监控
pub struct WebSocketManager {
    config: Arc<EventListenerConfig>,
    program_id: Pubkey,
    is_connected: Arc<AtomicBool>,
    is_running: Arc<AtomicBool>,
    connection_count: Arc<RwLock<u64>>,
    last_connection_time: Arc<RwLock<Option<Instant>>>,
    event_sender: broadcast::Sender<RpcLogsResponse>,
    _event_receiver: broadcast::Receiver<RpcLogsResponse>,
}

#[derive(Debug, Clone)]
pub struct ConnectionStats {
    pub is_connected: bool,
    pub is_running: bool,
    pub connection_count: u64,
    pub last_connection_time: Option<Instant>,
    pub uptime_seconds: Option<u64>,
}

impl WebSocketManager {
    /// 创建新的WebSocket管理器
    pub fn new(config: Arc<EventListenerConfig>) -> Result<Self> {
        let program_id = config.solana.program_id;
        let (event_sender, event_receiver) = broadcast::channel(10240); // 增加到10倍缓冲区

        Ok(Self {
            config,
            program_id,
            is_connected: Arc::new(AtomicBool::new(false)),
            is_running: Arc::new(AtomicBool::new(false)),
            connection_count: Arc::new(RwLock::new(0)),
            last_connection_time: Arc::new(RwLock::new(None)),
            event_sender,
            _event_receiver: event_receiver,
        })
    }

    /// 启动WebSocket连接管理
    pub async fn start(&self) -> Result<()> {
        if self.is_running.load(Ordering::Relaxed) {
            warn!("WebSocket管理器已在运行中");
            return Ok(());
        }

        self.is_running.store(true, Ordering::Relaxed);
        info!("🔌 启动WebSocket连接管理器，目标程序: {}", self.program_id);

        // 使用指数退避重连策略
        let backoff = ExponentialBackoff {
            initial_interval: self.config.get_initial_backoff_delay(),
            max_interval: self.config.get_max_backoff_delay(),
            multiplier: self.config.listener.backoff.multiplier,
            max_elapsed_time: None,
            ..Default::default()
        };

        let manager = self.clone();
        retry(backoff, || async {
            if !manager.is_running.load(Ordering::Relaxed) {
                return Err(backoff::Error::permanent(EventListenerError::WebSocket(
                    "WebSocket管理器已停止".to_string(),
                )));
            }

            match manager.connect_and_subscribe().await {
                Ok(()) => {
                    info!("✅ WebSocket连接建立成功");
                    Ok(())
                }
                Err(e) => {
                    error!("❌ WebSocket连接失败: {}", e);
                    manager.is_connected.store(false, Ordering::Relaxed);
                    Err(backoff::Error::transient(e))
                }
            }
        })
        .await
        .map_err(|e| EventListenerError::WebSocket(format!("连接重试失败: {:?}", e)))?;

        Ok(())
    }

    /// 停止WebSocket连接管理
    pub async fn stop(&self) -> Result<()> {
        info!("🛑 停止WebSocket连接管理器");
        self.is_running.store(false, Ordering::Relaxed);
        self.is_connected.store(false, Ordering::Relaxed);
        Ok(())
    }

    /// 建立连接并订阅事件
    async fn connect_and_subscribe(&self) -> Result<()> {
        debug!("🔗 尝试连接到WebSocket: {}", self.config.solana.ws_url);

        // 创建PubSub客户端
        let pubsub_client = PubsubClient::new(&self.config.solana.ws_url)
            .await
            .map_err(|e| EventListenerError::WebSocket(format!("创建PubSub客户端失败: {}", e)))?;

        // 配置日志订阅
        let config = RpcTransactionLogsConfig {
            commitment: Some(parse_commitment_config(&self.config.solana.commitment)),
        };

        // 订阅程序日志
        let (mut logs_subscription, _logs_unsubscribe) = pubsub_client
            .logs_subscribe(
                RpcTransactionLogsFilter::Mentions(vec![self.program_id.to_string()]),
                config,
            )
            .await
            .map_err(|e| EventListenerError::WebSocket(format!("订阅日志失败: {}", e)))?;

        // 更新连接状态
        self.is_connected.store(true, Ordering::Relaxed);
        {
            let mut count = self.connection_count.write().await;
            *count += 1;
        }
        {
            let mut last_time = self.last_connection_time.write().await;
            *last_time = Some(Instant::now());
        }

        info!("✅ WebSocket连接建立，开始监听事件");

        // 处理传入的日志事件
        while self.is_running.load(Ordering::Relaxed) {
            match logs_subscription.next().await {
                Some(log_response) => {
                    debug!("📨 接收到日志事件: {}", log_response.value.signature);

                    // 广播事件给所有订阅者
                    match self.event_sender.send(log_response.value) {
                        Ok(receiver_count) => {
                            debug!("✅ 事件广播成功，接收者数量: {}", receiver_count);
                        }
                        Err(e) => {
                            warn!("❌ 广播事件失败: {} - 可能没有活跃的接收者", e);
                        }
                    }
                }
                None => {
                    warn!("📡 WebSocket连接意外断开");
                    self.is_connected.store(false, Ordering::Relaxed);
                    return Err(EventListenerError::WebSocket(
                        "WebSocket连接意外断开".to_string(),
                    ));
                }
            }
        }

        Ok(())
    }

    /// 获取事件接收器
    pub fn subscribe(&self) -> broadcast::Receiver<RpcLogsResponse> {
        self.event_sender.subscribe()
    }

    /// 检查连接状态
    pub async fn is_healthy(&self) -> bool {
        self.is_connected.load(Ordering::Relaxed) && self.is_running.load(Ordering::Relaxed)
    }

    /// 获取连接统计信息
    pub async fn get_stats(&self) -> ConnectionStats {
        let connection_count = *self.connection_count.read().await;
        let last_connection_time = *self.last_connection_time.read().await;
        let uptime_seconds = last_connection_time.map(|time| time.elapsed().as_secs());

        ConnectionStats {
            is_connected: self.is_connected.load(Ordering::Relaxed),
            is_running: self.is_running.load(Ordering::Relaxed),
            connection_count,
            last_connection_time,
            uptime_seconds,
        }
    }

    /// 手动重连
    pub async fn reconnect(&self) -> Result<()> {
        info!("🔄 手动重连WebSocket");
        self.is_connected.store(false, Ordering::Relaxed);
        
        // 等待一段时间再重连
        sleep(Duration::from_millis(1000)).await;
        
        self.connect_and_subscribe().await
    }
}

impl Clone for WebSocketManager {
    fn clone(&self) -> Self {
        Self {
            config: Arc::clone(&self.config),
            program_id: self.program_id,
            is_connected: Arc::clone(&self.is_connected),
            is_running: Arc::clone(&self.is_running),
            connection_count: Arc::clone(&self.connection_count),
            last_connection_time: Arc::clone(&self.last_connection_time),
            event_sender: self.event_sender.clone(),
            _event_receiver: self.event_sender.subscribe(),
        }
    }
}

// Helper function to parse commitment config from string
fn parse_commitment_config(s: &str) -> CommitmentConfig {
    match s.to_lowercase().as_str() {
        "processed" => CommitmentConfig::processed(),
        "confirmed" => CommitmentConfig::confirmed(),
        "finalized" => CommitmentConfig::finalized(),
        _ => CommitmentConfig::confirmed(), // default
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::settings::SolanaConfig;
    use std::str::FromStr;

    fn create_test_config() -> EventListenerConfig {
        EventListenerConfig {
            solana: SolanaConfig {
                rpc_url: "https://api.devnet.solana.com".to_string(),
                ws_url: "wss://api.devnet.solana.com".to_string(),
                commitment: "confirmed".to_string(),
                program_id: Pubkey::from_str("FA1RJDDXysgwg5Gm3fJXWxt26JQzPkAzhTA114miqNUX").unwrap(),
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
    async fn test_websocket_manager_creation() {
        let config = Arc::new(create_test_config());
        let manager = WebSocketManager::new(config).unwrap();
        
        assert!(!manager.is_connected.load(Ordering::Relaxed));
        assert!(!manager.is_running.load(Ordering::Relaxed));
    }

    #[tokio::test]
    async fn test_websocket_manager_stats() {
        let config = Arc::new(create_test_config());
        let manager = WebSocketManager::new(config).unwrap();
        
        let stats = manager.get_stats().await;
        assert!(!stats.is_connected);
        assert!(!stats.is_running);
        assert_eq!(stats.connection_count, 0);
        assert!(stats.last_connection_time.is_none());
    }

    #[test]
    fn test_commitment_config_parsing() {
        // 测试我们的parse_commitment_config函数
        let processed = parse_commitment_config("processed");
        let confirmed = parse_commitment_config("confirmed");
        let finalized = parse_commitment_config("finalized");
        let _invalid = parse_commitment_config("invalid");
        
        // 验证它们不相等（这样测试不同的配置产生不同的结果）
        assert!(processed.commitment != finalized.commitment);
        assert!(confirmed.commitment != processed.commitment);
    }
}