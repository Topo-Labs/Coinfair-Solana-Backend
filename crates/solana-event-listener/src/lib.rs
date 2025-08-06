pub mod config;
pub mod error;
pub mod metrics;
pub mod parser;
pub mod persistence;
pub mod recovery;
pub mod subscriber;

#[cfg(test)]
pub mod tests;

pub use error::{EventListenerError, Result};

use crate::{
    config::EventListenerConfig,
    metrics::MetricsCollector,
    parser::EventParserRegistry,
    persistence::BatchWriter,
    recovery::CheckpointManager,
    subscriber::SubscriptionManager,
};
use std::sync::Arc;
use tokio::signal;
use tracing::{error, info, warn};

/// Event-Listener 主服务
/// 
/// 负责协调所有子模块运行:
/// - WebSocket订阅管理
/// - 事件解析和路由
/// - 批量持久化
/// - 检查点管理
/// - 监控指标收集
#[derive(Clone)]
pub struct EventListenerService {
    config: Arc<EventListenerConfig>,
    subscription_manager: Arc<SubscriptionManager>,
    parser_registry: Arc<EventParserRegistry>,
    batch_writer: Arc<BatchWriter>,
    checkpoint_manager: Arc<CheckpointManager>,
    metrics: Arc<MetricsCollector>,
}

impl EventListenerService {
    /// 创建新的Event-Listener服务实例
    pub async fn new(config: EventListenerConfig) -> Result<Self> {
        let config = Arc::new(config);
        
        info!("🚀 初始化Event-Listener服务...");
        
        // 初始化各个组件
        let metrics = Arc::new(MetricsCollector::new(&config)?);
        let checkpoint_manager = Arc::new(CheckpointManager::new(&config).await?);
        let batch_writer = Arc::new(BatchWriter::new(&config).await?);
        let parser_registry = Arc::new(EventParserRegistry::new(&config)?);
        let subscription_manager = Arc::new(
            SubscriptionManager::new(
                &config,
                Arc::clone(&parser_registry),
                Arc::clone(&batch_writer),
                Arc::clone(&checkpoint_manager),
                Arc::clone(&metrics),
            ).await?
        );

        info!("✅ Event-Listener服务初始化完成");

        Ok(Self {
            config,
            subscription_manager,
            parser_registry,
            batch_writer,
            checkpoint_manager,
            metrics,
        })
    }

    /// 启动Event-Listener服务
    pub async fn start(&self) -> Result<()> {
        info!("🎯 启动Event-Listener服务...");

        // 启动各个子服务
        let subscription_task = {
            let manager = Arc::clone(&self.subscription_manager);
            tokio::spawn(async move {
                if let Err(e) = manager.start().await {
                    error!("订阅管理器启动失败: {}", e);
                }
            })
        };

        let checkpoint_task = {
            let manager = Arc::clone(&self.checkpoint_manager);
            tokio::spawn(async move {
                if let Err(e) = manager.start_periodic_save().await {
                    error!("检查点管理器启动失败: {}", e);
                }
            })
        };

        let batch_writer_task = {
            let writer = Arc::clone(&self.batch_writer);
            tokio::spawn(async move {
                if let Err(e) = writer.start_batch_processing().await {
                    error!("批量写入器启动失败: {}", e);
                }
            })
        };

        let metrics_task = {
            let metrics = Arc::clone(&self.metrics);
            tokio::spawn(async move {
                if let Err(e) = metrics.start_collection().await {
                    error!("指标收集器启动失败: {}", e);
                }
            })
        };

        info!("✅ Event-Listener服务启动完成");

        // 等待关闭信号
        self.wait_for_shutdown_signal().await;

        info!("🛑 接收到关闭信号，开始优雅关闭...");

        // 停止所有任务
        subscription_task.abort();
        checkpoint_task.abort();
        batch_writer_task.abort();
        metrics_task.abort();

        // 执行清理工作
        self.shutdown().await?;

        info!("✅ Event-Listener服务已优雅关闭");
        Ok(())
    }

    /// 等待关闭信号
    async fn wait_for_shutdown_signal(&self) {
        let ctrl_c = async {
            signal::ctrl_c()
                .await
                .expect("failed to install Ctrl+C handler");
        };

        #[cfg(unix)]
        let terminate = async {
            signal::unix::signal(signal::unix::SignalKind::terminate())
                .expect("failed to install signal handler")
                .recv()
                .await;
        };

        #[cfg(not(unix))]
        let terminate = std::future::pending::<()>();

        tokio::select! {
            _ = ctrl_c => {
                info!("接收到Ctrl+C信号");
            },
            _ = terminate => {
                info!("接收到TERM信号");
            },
        }
    }

    /// 执行优雅关闭
    async fn shutdown(&self) -> Result<()> {
        // 停止订阅
        if let Err(e) = self.subscription_manager.stop().await {
            warn!("停止订阅管理器时出错: {}", e);
        }

        // 刷新批量写入缓冲区
        if let Err(e) = self.batch_writer.flush().await {
            warn!("刷新批量写入缓冲区时出错: {}", e);
        }

        // 保存最终检查点
        if let Err(e) = self.checkpoint_manager.save_checkpoint().await {
            warn!("保存最终检查点时出错: {}", e);
        }

        // 停止指标收集
        if let Err(e) = self.metrics.stop().await {
            warn!("停止指标收集时出错: {}", e);
        }

        Ok(())
    }

    /// 获取配置信息（用于健康检查和调试）
    pub fn get_config(&self) -> Arc<EventListenerConfig> {
        Arc::clone(&self.config)
    }

    /// 获取解析器注册表（用于运行时查询）
    pub fn get_parser_registry(&self) -> Arc<EventParserRegistry> {
        Arc::clone(&self.parser_registry)
    }

    /// 获取服务健康状态
    pub async fn health_check(&self) -> Result<HealthStatus> {
        let subscription_healthy = self.subscription_manager.is_healthy().await;
        let batch_writer_healthy = self.batch_writer.is_healthy().await;
        let checkpoint_healthy = self.checkpoint_manager.is_healthy().await;

        Ok(HealthStatus {
            overall_healthy: subscription_healthy && batch_writer_healthy && checkpoint_healthy,
            subscription_manager: subscription_healthy,
            batch_writer: batch_writer_healthy,
            checkpoint_manager: checkpoint_healthy,
            metrics: self.metrics.get_stats().await?,
        })
    }
}

/// 服务健康状态
#[derive(Debug, serde::Serialize)]
pub struct HealthStatus {
    pub overall_healthy: bool,
    pub subscription_manager: bool,
    pub batch_writer: bool,
    pub checkpoint_manager: bool,
    pub metrics: crate::metrics::MetricsStats,
}