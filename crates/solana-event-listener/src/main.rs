use solana_event_listener::{config::EventListenerConfig, EventListenerService};
use tracing::{error, info};
use tracing_subscriber::{EnvFilter, FmtSubscriber};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 加载环境配置文件
    if let Err(e) = utils::config::EnvLoader::load_env_file() {
        eprintln!("⚠️ 加载环境配置文件失败: {}", e);
    }

    // 初始化日志系统
    let subscriber = FmtSubscriber::builder()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with_target(false)
        .with_thread_ids(true)
        .with_thread_names(true)
        .with_file(true)
        .with_line_number(true)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("Failed to set tracing subscriber");

    info!("🎯 启动Coinfair Event-Listener服务");

    // 加载配置
    let config = match EventListenerConfig::from_env().await {
        Ok(config) => {
            info!("✅ 配置加载成功");
            config
        }
        Err(e) => {
            error!("❌ 配置加载失败: {}", e);
            std::process::exit(1);
        }
    };

    // 创建并启动服务
    match EventListenerService::new(config).await {
        Ok(service) => {
            info!("✅ Event-Listener服务创建成功");

            if let Err(e) = service.start().await {
                error!("❌ Event-Listener服务运行失败: {}", e);
                std::process::exit(1);
            }
        }
        Err(e) => {
            error!("❌ Event-Listener服务创建失败: {}", e);
            std::process::exit(1);
        }
    }

    Ok(())
}
