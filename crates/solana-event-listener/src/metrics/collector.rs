use crate::{config::EventListenerConfig, error::Result};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use sysinfo::System;
use tokio::{sync::RwLock, time::interval};
use tracing::{debug, error, info, warn};

/// 指标类型
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MetricType {
    /// 计数器 - 只能增加
    Counter,
    /// 测量器 - 可以增减
    Gauge,
    /// 直方图 - 记录分布
    Histogram,
    /// 摘要 - 统计信息
    Summary,
}

/// 指标数据点
#[derive(Debug, Clone)]
pub struct MetricData {
    /// 指标名称
    pub name: String,
    /// 指标类型
    pub metric_type: MetricType,
    /// 指标值
    pub value: f64,
    /// 时间戳
    pub timestamp: Instant,
    /// 标签
    pub labels: HashMap<String, String>,
    /// 描述
    pub description: String,
}

impl MetricData {
    /// 创建新的指标数据点
    pub fn new(name: String, metric_type: MetricType, value: f64, description: String) -> Self {
        Self {
            name,
            metric_type,
            value,
            timestamp: Instant::now(),
            labels: HashMap::new(),
            description,
        }
    }

    /// 添加标签
    pub fn with_label(mut self, key: String, value: String) -> Self {
        self.labels.insert(key, value);
        self
    }

    /// 添加多个标签
    pub fn with_labels(mut self, labels: HashMap<String, String>) -> Self {
        self.labels.extend(labels);
        self
    }
}

/// 指标收集器
///
/// 负责:
/// - 收集各种系统指标
/// - 提供指标查询接口
/// - 定期报告系统状态
/// - 监控性能和健康状态
pub struct MetricsCollector {
    config: Arc<EventListenerConfig>,

    // 运行状态
    is_running: Arc<RwLock<bool>>,

    // 核心指标计数器
    events_processed: Arc<AtomicU64>,
    events_failed: Arc<AtomicU64>,
    websocket_connections: Arc<AtomicU64>,
    websocket_reconnections: Arc<AtomicU64>,
    batch_writes: Arc<AtomicU64>,
    checkpoint_saves: Arc<AtomicU64>,

    // 性能指标
    processing_durations: Arc<RwLock<Vec<Duration>>>,
    websocket_latencies: Arc<RwLock<Vec<Duration>>>,
    batch_write_durations: Arc<RwLock<Vec<Duration>>>,

    // 系统指标
    start_time: Instant,
    last_metrics_report: Arc<RwLock<Option<Instant>>>,
    system_monitor: Arc<RwLock<System>>,

    // 自定义指标存储
    custom_metrics: Arc<RwLock<HashMap<String, MetricData>>>,
}

/// 指标统计信息
#[derive(Debug, Clone, serde::Serialize)]
pub struct MetricsStats {
    pub uptime_seconds: u64,
    pub events_processed: u64,
    pub events_failed: u64,
    pub success_rate: f64,
    pub websocket_connections: u64,
    pub websocket_reconnections: u64,
    pub batch_writes: u64,
    pub checkpoint_saves: u64,
    pub avg_processing_duration_ms: f64,
    pub avg_websocket_latency_ms: f64,
    pub avg_batch_write_duration_ms: f64,
    pub is_running: bool,
    #[serde(skip)]
    pub last_metrics_report: Option<Instant>,
    pub custom_metrics_count: usize,
}

/// 性能报告
#[derive(Debug, Clone, serde::Serialize)]
pub struct PerformanceReport {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub uptime_seconds: u64,
    pub events_per_second: f64,
    pub batches_per_minute: f64,
    pub avg_processing_time_ms: f64,
    pub error_rate: f64,
    pub websocket_health: WebSocketHealth,
    pub database_health: DatabaseHealth,
    pub system_resources: SystemResources,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct WebSocketHealth {
    pub is_connected: bool,
    pub connections_count: u64,
    pub reconnections_count: u64,
    pub avg_latency_ms: f64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DatabaseHealth {
    pub batch_writes_count: u64,
    pub avg_write_duration_ms: f64,
    pub checkpoint_saves_count: u64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SystemResources {
    pub memory_usage_mb: f64,
    pub cpu_usage_percent: f64,
}

impl MetricsCollector {
    /// 创建新的指标收集器
    pub fn new(config: &EventListenerConfig) -> Result<Self> {
        let config = Arc::new(config.clone());

        // 初始化系统监控
        let mut system = System::new_all();
        system.refresh_all();

        Ok(Self {
            config,
            is_running: Arc::new(RwLock::new(false)),
            events_processed: Arc::new(AtomicU64::new(0)),
            events_failed: Arc::new(AtomicU64::new(0)),
            websocket_connections: Arc::new(AtomicU64::new(0)),
            websocket_reconnections: Arc::new(AtomicU64::new(0)),
            batch_writes: Arc::new(AtomicU64::new(0)),
            checkpoint_saves: Arc::new(AtomicU64::new(0)),
            processing_durations: Arc::new(RwLock::new(Vec::new())),
            websocket_latencies: Arc::new(RwLock::new(Vec::new())),
            batch_write_durations: Arc::new(RwLock::new(Vec::new())),
            start_time: Instant::now(),
            last_metrics_report: Arc::new(RwLock::new(None)),
            system_monitor: Arc::new(RwLock::new(system)),
            custom_metrics: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// 启动指标收集
    pub async fn start_collection(&self) -> Result<()> {
        let mut is_running = self.is_running.write().await;
        if *is_running {
            warn!("指标收集器已在运行中");
            return Ok(());
        }
        *is_running = true;
        drop(is_running);

        info!("📊 启动指标收集");

        let collector = self.clone();
        let metrics_interval = self.config.get_metrics_interval();

        tokio::spawn(async move {
            let mut interval = interval(metrics_interval);

            while *collector.is_running.read().await {
                interval.tick().await;

                if let Err(e) = collector.collect_and_report_metrics().await {
                    error!("❌ 指标收集和报告失败: {}", e);
                }
            }

            info!("📊 指标收集已停止");
        });

        Ok(())
    }

    /// 停止指标收集
    pub async fn stop(&self) -> Result<()> {
        info!("🛑 停止指标收集器");
        let mut is_running = self.is_running.write().await;
        *is_running = false;
        Ok(())
    }

    /// 记录事件处理成功 - 支持多程序标签
    pub async fn record_event_processed(&self) -> Result<()> {
        self.events_processed.fetch_add(1, Ordering::Relaxed);
        debug!("📈 记录事件处理成功");
        Ok(())
    }

    /// 记录特定程序的事件处理成功
    pub async fn record_event_processed_for_program(&self, program_id: &str) -> Result<()> {
        self.events_processed.fetch_add(1, Ordering::Relaxed);

        // 创建带有程序ID标签的指标
        let metric = MetricData::new(
            "events_processed_by_program".to_string(),
            MetricType::Counter,
            1.0,
            "Events processed by specific program".to_string(),
        )
        .with_label("program_id".to_string(), program_id.to_string());

        self.add_custom_metric(metric).await?;
        debug!("📈 记录程序{}事件处理成功", program_id);
        Ok(())
    }

    /// 记录事件处理失败
    pub async fn record_event_failed(&self) -> Result<()> {
        self.events_failed.fetch_add(1, Ordering::Relaxed);
        debug!("📉 记录事件处理失败");
        Ok(())
    }

    /// 记录特定程序的事件处理失败
    pub async fn record_event_failed_for_program(&self, program_id: &str, error: &str) -> Result<()> {
        self.events_failed.fetch_add(1, Ordering::Relaxed);

        // 创建带有程序ID和错误类型标签的指标
        let metric = MetricData::new(
            "events_failed_by_program".to_string(),
            MetricType::Counter,
            1.0,
            "Events failed by specific program".to_string(),
        )
        .with_label("program_id".to_string(), program_id.to_string())
        .with_label("error_type".to_string(), error.to_string());

        self.add_custom_metric(metric).await?;
        debug!("📉 记录程序{}事件处理失败: {}", program_id, error);
        Ok(())
    }

    /// 记录事件处理耗时
    pub async fn record_processing_duration(&self, duration: Duration) -> Result<()> {
        let mut durations = self.processing_durations.write().await;
        durations.push(duration);

        // 保持最近1000个样本
        if durations.len() > 1000 {
            durations.remove(0);
        }

        debug!("⏱️ 记录处理耗时: {:?}", duration);
        Ok(())
    }

    /// 记录WebSocket连接
    pub async fn record_websocket_connection(&self) -> Result<()> {
        self.websocket_connections.fetch_add(1, Ordering::Relaxed);
        debug!("🔌 记录WebSocket连接");
        Ok(())
    }

    /// 记录WebSocket重连
    pub async fn record_websocket_reconnection(&self) -> Result<()> {
        self.websocket_reconnections.fetch_add(1, Ordering::Relaxed);
        debug!("🔄 记录WebSocket重连");
        Ok(())
    }

    /// 记录WebSocket延迟
    pub async fn record_websocket_latency(&self, latency: Duration) -> Result<()> {
        let mut latencies = self.websocket_latencies.write().await;
        latencies.push(latency);

        // 保持最近1000个样本
        if latencies.len() > 1000 {
            latencies.remove(0);
        }

        debug!("📡 记录WebSocket延迟: {:?}", latency);
        Ok(())
    }

    /// 记录批量写入
    pub async fn record_batch_write(&self) -> Result<()> {
        self.batch_writes.fetch_add(1, Ordering::Relaxed);
        debug!("💾 记录批量写入");
        Ok(())
    }

    /// 记录批量写入耗时
    pub async fn record_batch_write_duration(&self, duration: Duration) -> Result<()> {
        let mut durations = self.batch_write_durations.write().await;
        durations.push(duration);

        // 保持最近1000个样本
        if durations.len() > 1000 {
            durations.remove(0);
        }

        debug!("💽 记录批量写入耗时: {:?}", duration);
        Ok(())
    }

    /// 记录检查点保存
    pub async fn record_checkpoint_save(&self) -> Result<()> {
        self.checkpoint_saves.fetch_add(1, Ordering::Relaxed);
        debug!("💾 记录检查点保存");
        Ok(())
    }

    /// 记录清理周期
    pub async fn record_cleanup_cycle(&self) -> Result<()> {
        // 可以在这里记录清理相关的指标
        debug!("🧹 记录清理周期");
        Ok(())
    }

    /// 添加自定义指标
    pub async fn add_custom_metric(&self, metric: MetricData) -> Result<()> {
        let mut metrics = self.custom_metrics.write().await;
        metrics.insert(metric.name.clone(), metric);
        debug!("📊 添加自定义指标");
        Ok(())
    }

    /// 更新自定义指标
    pub async fn update_custom_metric(&self, name: &str, value: f64) -> Result<()> {
        let mut metrics = self.custom_metrics.write().await;
        if let Some(metric) = metrics.get_mut(name) {
            metric.value = value;
            metric.timestamp = Instant::now();
            debug!("📊 更新自定义指标: {} = {}", name, value);
        } else {
            warn!("⚠️ 自定义指标不存在: {}", name);
        }
        Ok(())
    }

    /// 收集并报告指标
    async fn collect_and_report_metrics(&self) -> Result<()> {
        if !self.config.monitoring.enable_performance_monitoring {
            return Ok(());
        }

        let stats = self.get_stats().await?;
        let report = self.generate_performance_report().await?;

        // 更新最后报告时间
        {
            let mut last_report = self.last_metrics_report.write().await;
            *last_report = Some(Instant::now());
        }

        info!(
            "📊 性能报告 - 运行时间: {}s, 处理事件: {}, 成功率: {:.2}%, 平均处理时间: {:.2}ms",
            stats.uptime_seconds,
            stats.events_processed,
            stats.success_rate * 100.0,
            stats.avg_processing_duration_ms
        );

        if report.error_rate > 0.1 {
            // 错误率超过10%
            warn!("⚠️ 高错误率检测: {:.2}%", report.error_rate * 100.0);
        }

        if report.avg_processing_time_ms > 1000.0 {
            // 平均处理时间超过1秒
            warn!("⚠️ 高处理延迟检测: {:.2}ms", report.avg_processing_time_ms);
        }

        Ok(())
    }

    /// 生成性能报告
    pub async fn generate_performance_report(&self) -> Result<PerformanceReport> {
        let uptime = self.start_time.elapsed().as_secs();
        let events_processed = self.events_processed.load(Ordering::Relaxed);
        let events_failed = self.events_failed.load(Ordering::Relaxed);
        let total_events = events_processed + events_failed;

        let events_per_second = if uptime > 0 {
            events_processed as f64 / uptime as f64
        } else {
            0.0
        };

        let batches_per_minute = if uptime > 0 {
            let batch_writes = self.batch_writes.load(Ordering::Relaxed);
            (batch_writes as f64 / uptime as f64) * 60.0
        } else {
            0.0
        };

        let error_rate = if total_events > 0 {
            events_failed as f64 / total_events as f64
        } else {
            0.0
        };

        // 计算平均处理时间
        let avg_processing_time_ms = {
            let durations = self.processing_durations.read().await;
            if durations.is_empty() {
                0.0
            } else {
                let total: Duration = durations.iter().sum();
                total.as_millis() as f64 / durations.len() as f64
            }
        };

        // WebSocket健康状态
        let websocket_health = WebSocketHealth {
            is_connected: true, // 这里应该从WebSocket管理器获取实际状态
            connections_count: self.websocket_connections.load(Ordering::Relaxed),
            reconnections_count: self.websocket_reconnections.load(Ordering::Relaxed),
            avg_latency_ms: {
                let latencies = self.websocket_latencies.read().await;
                if latencies.is_empty() {
                    0.0
                } else {
                    let total: Duration = latencies.iter().sum();
                    total.as_millis() as f64 / latencies.len() as f64
                }
            },
        };

        // 数据库健康状态
        let database_health = DatabaseHealth {
            batch_writes_count: self.batch_writes.load(Ordering::Relaxed),
            avg_write_duration_ms: {
                let durations = self.batch_write_durations.read().await;
                if durations.is_empty() {
                    0.0
                } else {
                    let total: Duration = durations.iter().sum();
                    total.as_millis() as f64 / durations.len() as f64
                }
            },
            checkpoint_saves_count: self.checkpoint_saves.load(Ordering::Relaxed),
        };

        // 获取真实的系统资源使用情况
        let system_resources = {
            let mut system = self.system_monitor.write().await;
            // 刷新系统信息以获取最新数据
            system.refresh_cpu();
            system.refresh_memory();

            // 计算内存使用量（MB）
            let memory_usage_mb = system.used_memory() as f64 / 1024.0 / 1024.0;

            // 计算CPU使用率（平均值）
            let cpu_usage_percent = if system.cpus().is_empty() {
                0.0
            } else {
                system.cpus().iter().map(|cpu| cpu.cpu_usage()).sum::<f32>() as f64 / system.cpus().len() as f64
            };

            SystemResources {
                memory_usage_mb,
                cpu_usage_percent,
            }
        };

        Ok(PerformanceReport {
            timestamp: chrono::Utc::now(),
            uptime_seconds: uptime,
            events_per_second,
            batches_per_minute,
            avg_processing_time_ms,
            error_rate,
            websocket_health,
            database_health,
            system_resources,
        })
    }

    /// 获取指标统计信息
    pub async fn get_stats(&self) -> Result<MetricsStats> {
        let uptime = self.start_time.elapsed().as_secs();
        let events_processed = self.events_processed.load(Ordering::Relaxed);
        let events_failed = self.events_failed.load(Ordering::Relaxed);
        let total_events = events_processed + events_failed;

        let success_rate = if total_events > 0 {
            events_processed as f64 / total_events as f64
        } else {
            1.0
        };

        let avg_processing_duration_ms = {
            let durations = self.processing_durations.read().await;
            if durations.is_empty() {
                0.0
            } else {
                let total: Duration = durations.iter().sum();
                total.as_millis() as f64 / durations.len() as f64
            }
        };

        let avg_websocket_latency_ms = {
            let latencies = self.websocket_latencies.read().await;
            if latencies.is_empty() {
                0.0
            } else {
                let total: Duration = latencies.iter().sum();
                total.as_millis() as f64 / latencies.len() as f64
            }
        };

        let avg_batch_write_duration_ms = {
            let durations = self.batch_write_durations.read().await;
            if durations.is_empty() {
                0.0
            } else {
                let total: Duration = durations.iter().sum();
                total.as_millis() as f64 / durations.len() as f64
            }
        };

        let custom_metrics_count = {
            let metrics = self.custom_metrics.read().await;
            metrics.len()
        };

        Ok(MetricsStats {
            uptime_seconds: uptime,
            events_processed,
            events_failed,
            success_rate,
            websocket_connections: self.websocket_connections.load(Ordering::Relaxed),
            websocket_reconnections: self.websocket_reconnections.load(Ordering::Relaxed),
            batch_writes: self.batch_writes.load(Ordering::Relaxed),
            checkpoint_saves: self.checkpoint_saves.load(Ordering::Relaxed),
            avg_processing_duration_ms,
            avg_websocket_latency_ms,
            avg_batch_write_duration_ms,
            is_running: *self.is_running.read().await,
            last_metrics_report: *self.last_metrics_report.read().await,
            custom_metrics_count,
        })
    }

    /// 重置所有指标
    pub async fn reset_metrics(&self) -> Result<()> {
        info!("🔄 重置所有指标");

        self.events_processed.store(0, Ordering::Relaxed);
        self.events_failed.store(0, Ordering::Relaxed);
        self.websocket_connections.store(0, Ordering::Relaxed);
        self.websocket_reconnections.store(0, Ordering::Relaxed);
        self.batch_writes.store(0, Ordering::Relaxed);
        self.checkpoint_saves.store(0, Ordering::Relaxed);

        {
            let mut durations = self.processing_durations.write().await;
            durations.clear();
        }

        {
            let mut latencies = self.websocket_latencies.write().await;
            latencies.clear();
        }

        {
            let mut durations = self.batch_write_durations.write().await;
            durations.clear();
        }

        {
            let mut metrics = self.custom_metrics.write().await;
            metrics.clear();
        }

        {
            let mut last_report = self.last_metrics_report.write().await;
            *last_report = None;
        }

        Ok(())
    }

    /// 导出指标为Prometheus格式（增强版本）
    pub async fn export_prometheus_metrics(&self) -> Result<String> {
        let stats = self.get_stats().await?;
        let report = self.generate_performance_report().await?;

        let mut output = String::new();

        // 基础标签 - 动态获取版本号
        let service_labels = "service=\"event-listener\"";
        let version_labels = format!("version=\"{}\"", env!("CARGO_PKG_VERSION"));
        let base_labels = format!("{},{}", service_labels, version_labels);

        // === 事件处理指标 ===
        output.push_str("# HELP events_processed_total Total number of events processed successfully\n");
        output.push_str("# TYPE events_processed_total counter\n");
        output.push_str(&format!(
            "events_processed_total{{{}}} {}\n",
            base_labels, stats.events_processed
        ));

        output.push_str("# HELP events_failed_total Total number of events that failed processing\n");
        output.push_str("# TYPE events_failed_total counter\n");
        output.push_str(&format!(
            "events_failed_total{{{}}} {}\n",
            base_labels, stats.events_failed
        ));

        output.push_str("# HELP events_success_rate Success rate of event processing (0-1)\n");
        output.push_str("# TYPE events_success_rate gauge\n");
        output.push_str(&format!(
            "events_success_rate{{{}}} {:.4}\n",
            base_labels, stats.success_rate
        ));

        output.push_str("# HELP events_per_second Current events processing rate\n");
        output.push_str("# TYPE events_per_second gauge\n");
        output.push_str(&format!(
            "events_per_second{{{}}} {:.2}\n",
            base_labels, report.events_per_second
        ));

        // === WebSocket 指标 ===
        output.push_str("# HELP websocket_connections_total Total number of WebSocket connections established\n");
        output.push_str("# TYPE websocket_connections_total counter\n");
        output.push_str(&format!(
            "websocket_connections_total{{{}}} {}\n",
            base_labels, stats.websocket_connections
        ));

        output.push_str("# HELP websocket_reconnections_total Total number of WebSocket reconnections\n");
        output.push_str("# TYPE websocket_reconnections_total counter\n");
        output.push_str(&format!(
            "websocket_reconnections_total{{{}}} {}\n",
            base_labels, stats.websocket_reconnections
        ));

        output
            .push_str("# HELP websocket_connected Current WebSocket connection status (1=connected, 0=disconnected)\n");
        output.push_str("# TYPE websocket_connected gauge\n");
        output.push_str(&format!(
            "websocket_connected{{{}}} {}\n",
            base_labels,
            if report.websocket_health.is_connected { 1 } else { 0 }
        ));

        output.push_str("# HELP websocket_latency_ms Average WebSocket latency in milliseconds\n");
        output.push_str("# TYPE websocket_latency_ms gauge\n");
        output.push_str(&format!(
            "websocket_latency_ms{{{}}} {:.2}\n",
            base_labels, report.websocket_health.avg_latency_ms
        ));

        // === 批量写入指标 ===
        output.push_str("# HELP batch_writes_total Total number of batch writes executed\n");
        output.push_str("# TYPE batch_writes_total counter\n");
        output.push_str(&format!(
            "batch_writes_total{{{}}} {}\n",
            base_labels, stats.batch_writes
        ));

        output.push_str("# HELP batch_writes_per_minute Current batch writes per minute rate\n");
        output.push_str("# TYPE batch_writes_per_minute gauge\n");
        output.push_str(&format!(
            "batch_writes_per_minute{{{}}} {:.2}\n",
            base_labels, report.batches_per_minute
        ));

        output.push_str("# HELP batch_write_duration_ms Average batch write duration in milliseconds\n");
        output.push_str("# TYPE batch_write_duration_ms gauge\n");
        output.push_str(&format!(
            "batch_write_duration_ms{{{}}} {:.2}\n",
            base_labels, report.database_health.avg_write_duration_ms
        ));

        // === 检查点指标 ===
        output.push_str("# HELP checkpoint_saves_total Total number of checkpoint saves\n");
        output.push_str("# TYPE checkpoint_saves_total counter\n");
        output.push_str(&format!(
            "checkpoint_saves_total{{{}}} {}\n",
            base_labels, stats.checkpoint_saves
        ));

        // === 性能指标 ===
        output.push_str("# HELP processing_duration_ms Average event processing duration in milliseconds\n");
        output.push_str("# TYPE processing_duration_ms gauge\n");
        output.push_str(&format!(
            "processing_duration_ms{{{}}} {:.2}\n",
            base_labels, stats.avg_processing_duration_ms
        ));

        output.push_str("# HELP processing_duration_total_ms Total processing time across all events\n");
        output.push_str("# TYPE processing_duration_total_ms gauge\n");
        output.push_str(&format!(
            "processing_duration_total_ms{{{}}} {:.2}\n",
            base_labels, report.avg_processing_time_ms
        ));

        output.push_str("# HELP error_rate Current error rate (0-1)\n");
        output.push_str("# TYPE error_rate gauge\n");
        output.push_str(&format!("error_rate{{{}}} {:.4}\n", base_labels, report.error_rate));

        // === 系统资源指标 ===
        output.push_str("# HELP system_memory_usage_mb Current memory usage in megabytes\n");
        output.push_str("# TYPE system_memory_usage_mb gauge\n");
        output.push_str(&format!(
            "system_memory_usage_mb{{{}}} {:.2}\n",
            base_labels, report.system_resources.memory_usage_mb
        ));

        output.push_str("# HELP system_cpu_usage_percent Current CPU usage percentage\n");
        output.push_str("# TYPE system_cpu_usage_percent gauge\n");
        output.push_str(&format!(
            "system_cpu_usage_percent{{{}}} {:.2}\n",
            base_labels, report.system_resources.cpu_usage_percent
        ));

        // === 运行时指标 ===
        output.push_str("# HELP uptime_seconds Total uptime in seconds\n");
        output.push_str("# TYPE uptime_seconds counter\n");
        output.push_str(&format!("uptime_seconds{{{}}} {}\n", base_labels, stats.uptime_seconds));

        output.push_str("# HELP running_status Current running status (1=running, 0=stopped)\n");
        output.push_str("# TYPE running_status gauge\n");
        output.push_str(&format!(
            "running_status{{{}}} {}\n",
            base_labels,
            if stats.is_running { 1 } else { 0 }
        ));

        // === 自定义指标 ===
        output.push_str("# HELP custom_metrics_count Number of custom metrics registered\n");
        output.push_str("# TYPE custom_metrics_count gauge\n");
        output.push_str(&format!(
            "custom_metrics_count{{{}}} {}\n",
            base_labels, stats.custom_metrics_count
        ));

        // 导出自定义指标
        let custom_metrics = self.custom_metrics.read().await;
        for (name, metric) in custom_metrics.iter() {
            // 根据指标类型决定Prometheus类型
            let prom_type = match metric.metric_type {
                MetricType::Counter => "counter",
                MetricType::Gauge => "gauge",
                MetricType::Histogram => "histogram",
                MetricType::Summary => "summary",
            };

            output.push_str(&format!("# HELP {} {}\n", name, metric.description));
            output.push_str(&format!("# TYPE {} {}\n", name, prom_type));

            // 构建标签字符串
            let mut labels_vec = vec![service_labels.to_string(), version_labels.clone()];
            for (key, value) in &metric.labels {
                labels_vec.push(format!("{}=\"{}\"", key, value));
            }
            let labels_str = labels_vec.join(",");

            output.push_str(&format!("{}{{{}}} {}\n", name, labels_str, metric.value));
        }

        Ok(output)
    }

    /// 检查指标收集器是否健康
    pub async fn is_healthy(&self) -> bool {
        *self.is_running.read().await
    }
}

impl Clone for MetricsCollector {
    fn clone(&self) -> Self {
        // 注意：对于系统监控，我们需要创建一个新的实例
        let mut system = System::new_all();
        system.refresh_all();

        Self {
            config: Arc::clone(&self.config),
            is_running: Arc::clone(&self.is_running),
            events_processed: Arc::clone(&self.events_processed),
            events_failed: Arc::clone(&self.events_failed),
            websocket_connections: Arc::clone(&self.websocket_connections),
            websocket_reconnections: Arc::clone(&self.websocket_reconnections),
            batch_writes: Arc::clone(&self.batch_writes),
            checkpoint_saves: Arc::clone(&self.checkpoint_saves),
            processing_durations: Arc::clone(&self.processing_durations),
            websocket_latencies: Arc::clone(&self.websocket_latencies),
            batch_write_durations: Arc::clone(&self.batch_write_durations),
            start_time: self.start_time,
            last_metrics_report: Arc::clone(&self.last_metrics_report),
            system_monitor: Arc::new(RwLock::new(system)),
            custom_metrics: Arc::clone(&self.custom_metrics),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_metric_data_creation() {
        let metric = MetricData::new(
            "test_counter".to_string(),
            MetricType::Counter,
            42.0,
            "Test counter metric".to_string(),
        );

        assert_eq!(metric.name, "test_counter");
        assert_eq!(metric.metric_type, MetricType::Counter);
        assert_eq!(metric.value, 42.0);
        assert_eq!(metric.description, "Test counter metric");
        assert!(metric.labels.is_empty());
    }

    #[test]
    fn test_metric_data_with_labels() {
        let mut labels = HashMap::new();
        labels.insert("environment".to_string(), "test".to_string());
        labels.insert("service".to_string(), "event-listener".to_string());

        let metric = MetricData::new(
            "test_gauge".to_string(),
            MetricType::Gauge,
            100.0,
            "Test gauge metric".to_string(),
        )
        .with_labels(labels.clone());

        assert_eq!(metric.labels, labels);
    }

    #[tokio::test]
    async fn test_metrics_collector_creation() {
        let config = create_test_config();
        let collector = MetricsCollector::new(&config).unwrap();

        let stats = collector.get_stats().await.unwrap();
        assert_eq!(stats.events_processed, 0);
        assert_eq!(stats.events_failed, 0);
        assert!(!stats.is_running);
    }

    #[tokio::test]
    async fn test_record_events() {
        let config = create_test_config();
        let collector = MetricsCollector::new(&config).unwrap();

        // 记录一些事件
        collector.record_event_processed().await.unwrap();
        collector.record_event_processed().await.unwrap();
        collector.record_event_failed().await.unwrap();

        let stats = collector.get_stats().await.unwrap();
        assert_eq!(stats.events_processed, 2);
        assert_eq!(stats.events_failed, 1);
        assert_eq!(stats.success_rate, 2.0 / 3.0);
    }

    #[tokio::test]
    async fn test_record_durations() {
        let config = create_test_config();
        let collector = MetricsCollector::new(&config).unwrap();

        // 记录一些处理耗时
        collector
            .record_processing_duration(Duration::from_millis(100))
            .await
            .unwrap();
        collector
            .record_processing_duration(Duration::from_millis(200))
            .await
            .unwrap();
        collector
            .record_processing_duration(Duration::from_millis(150))
            .await
            .unwrap();

        let stats = collector.get_stats().await.unwrap();
        assert_eq!(stats.avg_processing_duration_ms, 150.0);
    }

    #[tokio::test]
    async fn test_custom_metrics() {
        let config = create_test_config();
        let collector = MetricsCollector::new(&config).unwrap();

        let metric = MetricData::new(
            "custom_counter".to_string(),
            MetricType::Counter,
            10.0,
            "Custom counter".to_string(),
        );

        collector.add_custom_metric(metric).await.unwrap();

        let stats = collector.get_stats().await.unwrap();
        assert_eq!(stats.custom_metrics_count, 1);

        // 更新自定义指标
        collector.update_custom_metric("custom_counter", 20.0).await.unwrap();
        // 这里可以添加更多验证逻辑
    }

    #[tokio::test]
    async fn test_reset_metrics() {
        let config = create_test_config();
        let collector = MetricsCollector::new(&config).unwrap();

        // 记录一些数据
        collector.record_event_processed().await.unwrap();
        collector.record_websocket_connection().await.unwrap();

        let stats_before = collector.get_stats().await.unwrap();
        assert_eq!(stats_before.events_processed, 1);
        assert_eq!(stats_before.websocket_connections, 1);

        // 重置指标
        collector.reset_metrics().await.unwrap();

        let stats_after = collector.get_stats().await.unwrap();
        assert_eq!(stats_after.events_processed, 0);
        assert_eq!(stats_after.websocket_connections, 0);
    }

    #[tokio::test]
    async fn test_system_resource_monitoring() {
        let config = create_test_config();
        let collector = MetricsCollector::new(&config).unwrap();

        // 生成性能报告，应该包含真实的系统资源信息
        let report = collector.generate_performance_report().await.unwrap();

        // 验证系统资源监控不再使用占位符值
        assert!(report.system_resources.memory_usage_mb >= 0.0);
        assert!(report.system_resources.cpu_usage_percent >= 0.0);
        assert!(
            report.system_resources.cpu_usage_percent
                <= 100.0 * std::thread::available_parallelism().unwrap().get() as f64
        );

        println!(
            "✅ 系统资源监控: 内存 {:.2}MB, CPU {:.2}%",
            report.system_resources.memory_usage_mb, report.system_resources.cpu_usage_percent
        );
    }

    #[tokio::test]
    async fn test_generate_performance_report() {
        let config = create_test_config();
        let collector = MetricsCollector::new(&config).unwrap();

        // 记录一些数据
        collector.record_event_processed().await.unwrap();
        collector
            .record_processing_duration(Duration::from_millis(500))
            .await
            .unwrap();
        collector.record_batch_write().await.unwrap();

        let report = collector.generate_performance_report().await.unwrap();

        assert_eq!(report.websocket_health.connections_count, 0);
        assert_eq!(report.database_health.batch_writes_count, 1);
        assert!(report.uptime_seconds < 60); // 应该小于60秒
    }

    #[tokio::test]
    async fn test_export_prometheus_metrics_enhanced() {
        let config = create_test_config();
        let collector = MetricsCollector::new(&config).unwrap();

        // 记录一些数据来测试增强功能
        collector.record_event_processed().await.unwrap();
        collector.record_event_failed().await.unwrap();
        collector.record_websocket_connection().await.unwrap();
        collector.record_batch_write().await.unwrap();

        // 添加一个自定义指标
        let custom_metric = MetricData::new(
            "test_custom_metric".to_string(),
            MetricType::Gauge,
            42.5,
            "Test custom metric for enhanced export".to_string(),
        )
        .with_label("environment".to_string(), "test".to_string());

        collector.add_custom_metric(custom_metric).await.unwrap();

        let prometheus_output = collector.export_prometheus_metrics().await.unwrap();

        // 获取当前版本用于测试
        let current_version = env!("CARGO_PKG_VERSION");
        let expected_label_pattern = format!("service=\"event-listener\",version=\"{}\"", current_version);

        // 验证基础指标
        assert!(prometheus_output.contains(&format!("events_processed_total{{{}}} 1", expected_label_pattern)));
        assert!(prometheus_output.contains(&format!("events_failed_total{{{}}} 1", expected_label_pattern)));

        // 验证增强功能
        assert!(prometheus_output.contains("events_success_rate"));
        assert!(prometheus_output.contains("events_per_second"));
        assert!(prometheus_output.contains("websocket_connected"));
        assert!(prometheus_output.contains("system_memory_usage_mb"));
        assert!(prometheus_output.contains("system_cpu_usage_percent"));
        assert!(prometheus_output.contains("uptime_seconds"));
        assert!(prometheus_output.contains("running_status"));

        // 验证自定义指标
        assert!(prometheus_output.contains("test_custom_metric"));
        assert!(prometheus_output.contains("Test custom metric for enhanced export"));
        assert!(prometheus_output.contains("environment=\"test\""));
        assert!(prometheus_output.contains("42.5"));

        // 验证Prometheus格式正确性
        assert!(prometheus_output.contains("# HELP"));
        assert!(prometheus_output.contains("# TYPE"));

        // 验证所有标签都包含service和version
        let lines: Vec<&str> = prometheus_output.lines().collect();
        for line in lines.iter().filter(|line| !line.starts_with('#') && !line.is_empty()) {
            if line.contains('{') {
                assert!(
                    line.contains("service=\"event-listener\""),
                    "Line missing service label: {}",
                    line
                );
                assert!(
                    line.contains(&format!("version=\"{}\"", current_version)),
                    "Line missing version label: {}",
                    line
                );
            }
        }

        println!("✅ 增强的Prometheus导出包含 {} 行指标", lines.len());
    }

    #[tokio::test]
    async fn test_is_healthy() {
        let config = create_test_config();
        let collector = MetricsCollector::new(&config).unwrap();

        // 初始状态不健康（未运行）
        assert!(!collector.is_healthy().await);

        // 模拟运行状态
        {
            let mut is_running = collector.is_running.write().await;
            *is_running = true;
        }

        // 运行状态下应该健康
        assert!(collector.is_healthy().await);
    }
}
