pub mod config;
pub mod error;
pub mod metrics;
pub mod parser;
pub mod persistence;
pub mod recovery;
pub mod services;
pub mod subscriber;

#[cfg(test)]
pub mod tests;

pub use error::{EventListenerError, Result};

use crate::{
    config::EventListenerConfig, metrics::MetricsCollector, parser::EventParserRegistry, persistence::BatchWriter,
    recovery::{BackfillManager, CheckpointPersistence, ScanRecordPersistence},
    subscriber::SubscriptionManager,
};
use std::sync::Arc;
use tokio::signal;
use tracing::{error, info, warn};
use utils::{MetaplexService, TokenMetadataProvider};

/// Event-Listener 主服务
///
/// 负责协调所有子模块运行:
/// - WebSocket订阅管理
/// - 事件解析和路由
/// - 批量持久化
/// - 监控指标收集
/// - 历史事件回填
#[derive(Clone)]
pub struct EventListenerService {
    config: Arc<EventListenerConfig>,
    subscription_manager: Arc<SubscriptionManager>,
    parser_registry: Arc<EventParserRegistry>,
    batch_writer: Arc<BatchWriter>,
    metrics: Arc<MetricsCollector>,
    backfill_manager: Option<Arc<BackfillManager>>,
}

impl EventListenerService {
    /// 创建新的Event-Listener服务实例
    pub async fn new(config: EventListenerConfig) -> Result<Self> {
        let config = Arc::new(config);

        info!("🚀 初始化Event-Listener服务...");

        // 初始化各个组件
        let metrics = Arc::new(MetricsCollector::new(&config)?);
        let batch_writer = Arc::new(BatchWriter::new(&config).await?);

        // 创建 MetaplexService 作为代币元数据提供者
        let metadata_provider = match MetaplexService::new(None) {
            Ok(service) => {
                info!("✅ 成功创建代币元数据提供者");
                let provider: Arc<tokio::sync::Mutex<dyn TokenMetadataProvider>> =
                    Arc::new(tokio::sync::Mutex::new(service));
                Some(provider)
            }
            Err(e) => {
                warn!("⚠️ 创建代币元数据提供者失败: {}, 将使用基础链上查询", e);
                None
            }
        };

        // 预计算回填配置（如果启用回填功能）
        info!("🔍 检查回填配置状态: backfill={:?}", config.backfill.as_ref().map(|c| c.enabled));
        let backfill_parser_keys = if let Some(backfill_config) = &config.backfill {
            info!("📋 发现回填配置，enabled={}", backfill_config.enabled);
            if backfill_config.enabled {
                info!("🔄 预计算回填ParserKey配置...");
                let event_configs = config.get_backfill_event_configs()?;
                
                let mut keys = std::collections::HashSet::new();
                for event_config in &event_configs {
                    if event_config.enabled {
                        let discriminator = crate::parser::event_parser::calculate_event_discriminator(&event_config.event_type);
                        let parser_key = crate::parser::event_parser::ParserKey::for_program(event_config.program_id, discriminator);
                        keys.insert(parser_key);
                        
                        info!("🔑 预计算回填事件 {} 的ParserKey: program={}, discriminator={:?}", 
                            event_config.event_type, event_config.program_id, discriminator);
                    }
                }
                info!("✅ 预计算完成，共 {} 个回填ParserKey", keys.len());
                Some(keys)
            } else {
                info!("⚠️ 回填配置被禁用，跳过预计算");
                None
            }
        } else {
            info!("⚠️ 未找到回填配置，跳过预计算");
            None
        };

        // 使用带有元数据提供者和回填配置的EventParserRegistry
        let parser_registry = Arc::new(EventParserRegistry::new_with_metadata_provider_and_backfill(
            &config,
            metadata_provider,
            backfill_parser_keys,
        )?);

        let subscription_manager = Arc::new(
            SubscriptionManager::new(
                &config,
                Arc::clone(&parser_registry),
                Arc::clone(&batch_writer),
                Arc::clone(&metrics),
            )
            .await?,
        );

        // 初始化回填管理器（可选功能）
        let backfill_manager = if let Some(backfill_config) = &config.backfill {
            if backfill_config.enabled {
                info!("🔄 初始化历史事件回填管理器...");
                
                // 创建数据库连接用于持久化组件
                let client = mongodb::Client::with_uri_str(&config.database.uri)
                    .await
                    .map_err(|e| EventListenerError::Database(e))?;
                let database = Arc::new(client.database(&config.database.database_name));
                
                // 创建持久化组件
                let checkpoint_persistence = Arc::new(CheckpointPersistence::new(database.clone()).await?);
                let scan_record_persistence = Arc::new(ScanRecordPersistence::new(database.clone()).await?);
                
                // 获取回填事件配置
                let event_configs = config.get_backfill_event_configs()?;
                let default_check_interval = backfill_config.default_check_interval_secs.unwrap_or(300);
                
                let manager = BackfillManager::new(
                    &config,
                    Arc::clone(&parser_registry),
                    Arc::clone(&batch_writer),
                    Arc::clone(&metrics),
                    checkpoint_persistence,
                    scan_record_persistence,
                    event_configs,
                    default_check_interval,
                );
                
                info!("✅ 回填管理器初始化完成");
                Some(Arc::new(manager))
            } else {
                info!("⚠️ 回填功能已禁用");
                None
            }
        } else {
            info!("⚠️ 未配置回填功能");
            None
        };

        info!("✅ Event-Listener服务初始化完成");

        Ok(Self {
            config,
            subscription_manager,
            parser_registry,
            batch_writer,
            metrics,
            backfill_manager,
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

        // 启动回填管理器（如果启用）
        let backfill_task = if let Some(backfill_manager) = &self.backfill_manager {
            let manager = Arc::clone(backfill_manager);
            Some(tokio::spawn(async move {
                if let Err(e) = manager.start().await {
                    error!("回填管理器启动失败: {}", e);
                }
            }))
        } else {
            None
        };

        info!("✅ Event-Listener服务启动完成");

        // 等待关闭信号
        self.wait_for_shutdown_signal().await;

        info!("🛑 接收到关闭信号，开始优雅关闭...");

        // 停止所有任务
        subscription_task.abort();
        batch_writer_task.abort();
        metrics_task.abort();
        
        // 停止回填任务（如果存在）
        if let Some(task) = backfill_task {
            task.abort();
        }

        // 执行清理工作
        self.shutdown().await?;

        info!("✅ Event-Listener服务已优雅关闭");
        Ok(())
    }

    /// 等待关闭信号
    async fn wait_for_shutdown_signal(&self) {
        let ctrl_c = async {
            signal::ctrl_c().await.expect("failed to install Ctrl+C handler");
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

        // 注意：不再保存订阅服务检查点，回填服务会自动处理检查点

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

        // 如果启用回填服务，检查其健康状态，否则默认为健康
        let backfill_healthy = if self.backfill_manager.is_some() {
            // 回填服务暂时假设总是健康的，因为它是周期性任务
            // 可以在后续添加更复杂的健康检查逻辑
            true
        } else {
            true // 未启用时视为健康
        };

        Ok(HealthStatus {
            overall_healthy: subscription_healthy && batch_writer_healthy && backfill_healthy,
            subscription_manager: subscription_healthy,
            batch_writer: batch_writer_healthy,
            backfill_manager: backfill_healthy,
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
    pub backfill_manager: bool,
    pub metrics: crate::metrics::MetricsStats,
}
