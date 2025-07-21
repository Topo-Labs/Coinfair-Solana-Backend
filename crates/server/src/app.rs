use crate::{router::AppRouter, services::Services};
use anyhow::Context;
use axum::serve;
use database::Database;
use std::sync::Arc;
use tokio::signal;
use tracing::info;
use utils::{logger::Logger, AppConfig};

pub struct ApplicationServer;

impl ApplicationServer {
    pub async fn serve(config: Arc<AppConfig>) -> anyhow::Result<()> {
        // 根据 CARGO_ENV 加载对应的环境配置文件
        // if let Err(e) = utils::EnvLoader::load_env_file() {
        //     tracing::warn!("Failed to load environment file: {}", e);
        // }

        let _guard = Logger::new(config.cargo_env);

        let address = format!("{}:{}", config.app_host, config.app_port);
        let tcp_listener = tokio::net::TcpListener::bind(address).await.context("🔴 Failed to bind TCP listener")?;

        let local_addr = tcp_listener.local_addr().context("🔴 Failed to get local address")?;

        // 构建一个内置了多种"集合"对应的底层数据库操作的Database
        let db = Database::new(config.clone()).await?;
        let services = Services::new(db);
        let router = AppRouter::new(services);

        info!("🟢 server:referring_reward has launched on {local_addr} 🚀");

        serve(tcp_listener, router)
            .with_graceful_shutdown(Self::shutdown_signal())
            .await
            .context("🔴 Failed to start server")?;

        Ok(())
    }

    async fn shutdown_signal() {
        let ctrl_c = async {
            signal::ctrl_c().await.expect("🔴 Failed to install Ctrl+C handler");
        };

        #[cfg(unix)]
        let terminate = async {
            signal::unix::signal(signal::unix::SignalKind::terminate())
                .expect("🔴 Failed to install signal handler")
                .recv()
                .await;
        };

        #[cfg(not(unix))]
        let terminate = std::future::pending::<()>();

        tokio::select! {
            _ = ctrl_c => {},
            _ = terminate => {},
        }

        tracing::warn!("❌ Signal received, starting graceful shutdown...");
    }
}
