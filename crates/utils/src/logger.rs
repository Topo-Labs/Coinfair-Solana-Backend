// TODO: 需要移动到utils中（因为跟业务server无关）
// Replant from: ./crates/server/src/logger.rs

use std::path::PathBuf;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::CargoEnv;

pub struct Logger;
impl Logger {
    pub fn new(cargo_env: CargoEnv) -> WorkerGuard {
        Self::new_with_log_dir(cargo_env, None)
    }

    pub fn new_with_log_dir(cargo_env: CargoEnv, log_dir: Option<PathBuf>) -> WorkerGuard {
        let (non_blocking, guard) = match cargo_env {
            CargoEnv::Development => {
                let console_logger = std::io::stdout();
                tracing_appender::non_blocking(console_logger)
            }
            CargoEnv::Production => {
                let log_directory = Self::get_log_directory(log_dir);
                
                // 确保日志目录存在
                if let Err(e) = std::fs::create_dir_all(&log_directory) {
                    eprintln!("⚠️ 无法创建日志目录 {:?}: {}", log_directory, e);
                    eprintln!("回退到当前目录下的logs文件夹");
                    std::fs::create_dir_all("logs").ok();
                    let file_logger = tracing_appender::rolling::daily("logs", "log");
                    return tracing_appender::non_blocking(file_logger).1;
                }
                
                println!("✅ 日志将输出到目录: {:?}", log_directory);
                let file_logger = tracing_appender::rolling::daily(&log_directory, "log");
                tracing_appender::non_blocking(file_logger)
            }
        };

        // Set the default verbosity level for the root of the dependency graph.
        // env var: `RUST_LOG`
        let env_filter =
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| format!("{}=debug,tower_http=debug", env!("CARGO_PKG_NAME")).into());

        tracing_subscriber::registry()
            .with(env_filter)
            .with(
                tracing_subscriber::fmt::layer()
                    .with_writer(non_blocking)
                    .with_file(true) // 显示文件名
                    .with_line_number(true) // 显示行号
                    .with_target(false), // 隐藏target减少冗余
            )
            .init();

        guard
    }

    fn get_log_directory(log_dir: Option<PathBuf>) -> PathBuf {
        // 1. 优先使用传入的参数
        if let Some(dir) = log_dir {
            return dir;
        }

        // 2. 检查环境变量 LOG_DIR
        if let Ok(log_dir_env) = std::env::var("LOG_DIR") {
            return PathBuf::from(log_dir_env);
        }

        // 3. 尝试获取可执行文件目录
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                return exe_dir.join("logs");
            }
        }

        // 4. 回退到当前工作目录
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("logs")
    }
}
