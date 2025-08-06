use anyhow::{Context, Result};
use clap::Parser;
use database::Database;
use monitor::monitor::Monitor;
use server::{app::ApplicationServer, services::Services};
use solana_event_listener::{config::EventListenerConfig, EventListenerService};
use std::sync::Arc;
use telegram::HopeBot;
use timer::Timer;
use tokio::time::{sleep, Duration};
use tokio::{signal, sync::Notify, task::JoinSet};
use tracing::info;
use utils::{logger::Logger, AppConfig};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let coinfair = Coinfair::new().await;
    coinfair.run().await.expect("Coinfair backend error");

    Ok(())
}
#[allow(dead_code)]
pub struct Coinfair {
    services: Services,
    monitor: Monitor,
    timer: Timer,
    telegram: HopeBot,
    event_listener: Option<EventListenerService>,
    config: Arc<AppConfig>,
    _log_guard: tracing_appender::non_blocking::WorkerGuard,
}

impl Coinfair {
    pub async fn new() -> Self {
        let config = Coinfair::with_config();

        // 初始化日志系统 - 在这里统一管理
        let log_guard = Self::setup_logging(&config);

        let services = Coinfair::with_service(config.clone()).await;
        let monitor = Coinfair::with_monitor(services.clone()).await;
        let telegram = Coinfair::with_telegram(services.clone());
        let timer = Coinfair::with_timer(services.clone(), telegram.clone());
        let event_listener = Coinfair::with_event_listener(config.clone()).await;

        Self {
            services,
            monitor,
            timer,
            telegram,
            event_listener,
            config,
            _log_guard: log_guard,
        }
    }

    pub async fn run(self) -> Result<JoinSet<()>, Box<dyn std::error::Error>> {
        let shutdown_notify = Arc::new(Notify::new());
        let mut set = JoinSet::new();

        // 1. 启动api & services
        // 2. 启动telegram
        // 3. 启动Timer
        // 4. 启动Monitor

        // set.spawn(async move {
        //     run_telegram_with_poll(bot, state, services)
        //         .await
        //         .expect("telegram bot error");
        // });

        //set.spawn(async move {
        //  info!("Monitor is running...");
        //  self.monitor.run().await.expect("🔴 Failed to start monitor");
        // });

        set.spawn(async move {
            loop {
                info!("Starting monitor...");
                match self.monitor.run().await {
                    Ok(_) => {
                        info!("Monitor exited normally, restarting...");
                    }
                    Err(e) => {
                        info!("🔴 Monitor crashed: {:?}. Restarting in 2 seconds...", e);
                    }
                }
                sleep(Duration::from_secs(2)).await; // 等待2秒后重试
            }
        });

        set.spawn(async move {
            ApplicationServer::serve(self.config.clone())
                .await
                .context("🔴 Failed to start server")
                .expect("🔴 Failed to start server");
        });

        // 启动CLMM池子自动同步服务
        let services_for_sync = self.services.clone();
        set.spawn(async move {
            loop {
                info!("🔄 启动CLMM池子同步服务...");
                match services_for_sync.solana.start_clmm_pool_sync().await {
                    Ok(_) => {
                        info!("✅ CLMM池子同步服务正常退出，重启中...");
                    }
                    Err(e) => {
                        info!("❌ CLMM池子同步服务异常: {:?}，2秒后重启...", e);
                    }
                }
                sleep(Duration::from_secs(2)).await;
            }
        });

        // 启动事件监听服务
        if let Some(event_listener) = self.event_listener {
            set.spawn(async move {
                loop {
                    info!("🎯 启动Event-Listener服务...");
                    match event_listener.start().await {
                        Ok(_) => {
                            info!("✅ Event-Listener服务正常退出，重启中...");
                        }
                        Err(e) => {
                            info!("❌ Event-Listener服务异常: {:?}，2秒后重启...", e);
                        }
                    }
                    sleep(Duration::from_secs(2)).await;
                }
            });
        }

        tokio::select! {
            _ = async {
                while let Some(_) = set.join_next().await {
                    info!("🔔 Task completed");
                }
            } => {},
            _ = shutdown_signal() => {
                info!("🔔 Shutdown signal received, stopping all tasks...");
                shutdown_notify.notify_waiters(); // 通知所有等待的任务
            },
        }
        Ok(set)
    }
}

impl Coinfair {
    fn with_config() -> Arc<AppConfig> {
        // 根据 CARGO_ENV 加载对应的环境配置文件
        utils::EnvLoader::load_env_file().ok();
        let config = Arc::new(AppConfig::parse());
        config
    }

    fn setup_logging(config: &AppConfig) -> tracing_appender::non_blocking::WorkerGuard {
        // 获取可执行文件所在目录，构建日志路径
        let log_dir = if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                Some(exe_dir.join("logs"))
            } else {
                None
            }
        } else {
            None
        };

        println!("🚀 正在启动 Coinfair 后端服务...");
        if let Some(ref dir) = log_dir {
            println!("📁 日志目录: {:?}", dir);
        }

        Logger::new_with_log_dir(config.cargo_env, log_dir)
    }

    async fn with_service(config: Arc<AppConfig>) -> Services {
        let mongodb = Database::new(config.clone()).await.expect("mongodb wrong in coinfair/src/main.rs");

        let services = Services::new(mongodb);
        services
    }

    async fn with_monitor(services: Services) -> Monitor {
        let monitor = Monitor::default(services).await;
        monitor
    }

    fn with_telegram(services: Services) -> HopeBot {
        let telegram = HopeBot::default(services);
        telegram
    }

    fn with_timer(services: Services, telegram: HopeBot) -> Timer {
        let timer = Timer::new(None, services, telegram);
        timer
    }

    async fn with_event_listener(_config: Arc<AppConfig>) -> Option<EventListenerService> {
        // 尝试创建事件监听器配置
        match EventListenerConfig::from_env().await {
            Ok(event_config) => {
                info!("🎯 初始化Event-Listener配置...");

                match EventListenerService::new(event_config).await {
                    Ok(service) => {
                        info!("✅ Event-Listener服务初始化成功");
                        Some(service)
                    }
                    Err(e) => {
                        info!("❌ Event-Listener服务初始化失败: {}", e);
                        None
                    }
                }
            }
            Err(e) => {
                info!("❌ Event-Listener配置加载失败: {}", e);
                None
            }
        }
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c().await.expect("🔴 Failed to install Ctrl+C handler");
        info!("🔔 Ctrl+C received");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("🔴 Failed to install signal handler")
            .recv()
            .await;
        info!("🔔 Terminate signal received");
    };

    #[cfg(not(unix))]
    #[allow(dead_code)]
    let terminate = async {
        // Windows 系统下，我们只监听 Ctrl+C 信号
        // 这里创建一个永远不会完成的 future
        std::future::pending::<()>().await;
    };

    tokio::select! {
        _ = ctrl_c => {
            info!("🔔 Ctrl+C signal received");
        },
        _ = terminate => {
            info!("🔔 Terminate signal received");
        },
    }

    tracing::warn!("❌ Signal received, starting graceful shutdown...");
}
