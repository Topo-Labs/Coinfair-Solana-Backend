use anyhow::{Context, Result};
use axum::{routing::Router, serve};
use clap::Parser;
use database::Database;
use dotenvy::dotenv;
// use monitor::monitor::Monitor;  // 注释掉monitor的导入
use server::{app::ApplicationServer, services::Services};
use solana::{SolanaSwap, SwapConfig};
use std::sync::Arc;
// use telegram::HopeBot;  // 注释掉telegram的导入
// use timer::Timer;  // 注释掉timer的导入
use tokio::{signal, sync::Notify, task::JoinSet};
use tracing::info;
use utils::{logger::Logger, AppConfig};
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    dotenv().ok();

    let config = Arc::new(AppConfig::parse());

    // 直接启动ApplicationServer，不使用Hope结构体
    info!("🚀 启动Solana Controller服务...");
    ApplicationServer::serve(config)
        .await
        .context("🔴 Failed to start server")?;

    Ok(())
}

// 注释掉整个Hope结构体相关的代码，因为我们只需要ApplicationServer
/*
pub struct Hope {
    services: Services,
    monitor: Monitor,
    timer: Timer,
    telegram: HopeBot,
    config: Arc<AppConfig>,
}

impl Hope {
    pub async fn new() -> Self {
        let config = Hope::with_config();
        let services = Hope::with_service(config.clone()).await;
        let monitor = Hope::with_monitor(services.clone()).await;
        let telegram = Hope::with_telegram(services.clone());
        let timer = Hope::with_timer(services.clone(), telegram.clone());

        Self {
            services,
            monitor,
            timer,
            telegram,
            config,
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

        // 注释掉monitor服务的启动
        // set.spawn(async move {
        //     loop {
        //         info!("Starting monitor...");
        //         match self.monitor.run().await {
        //             Ok(_) => {
        //                 info!("Monitor exited normally, restarting...");
        //             }
        //             Err(e) => {
        //                 info!("🔴 Monitor crashed: {:?}. Restarting in 2 seconds...", e);
        //             }
        //         }
        //         sleep(Duration::from_secs(2)).await; // 等待2秒后重试
        //     }
        // });

        set.spawn(async move {
            ApplicationServer::serve(self.config.clone())
                .await
                .context("🔴 Failed to start server")
                .expect("🔴 Failed to start server");
        });

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

impl Hope {
    fn with_config() -> Arc<AppConfig> {
        dotenv().ok();
        let config = Arc::new(AppConfig::parse());
        //let _ = Logger::new(config.cargo_env);
        config
    }

    async fn with_service(config: Arc<AppConfig>) -> Services {
        let mongodb = Database::new(config.clone())
            .await
            .expect("mongodb wrong in hope/src/main.rs");

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
}
*/

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("🔴 Failed to install Ctrl+C handler");
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

    tokio::select! {
        _ = ctrl_c => {
            info!("🔔 Terminate signal received 1");
        },
        _ = terminate => {
            info!("🔔 Terminate signal received 2");
        },
    }

    tracing::warn!("❌ Signal received, starting graceful shutdown...");
}
