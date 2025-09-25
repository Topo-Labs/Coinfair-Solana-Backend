use clap::Parser;
use serde::{Deserialize, Serialize};

#[derive(clap::ValueEnum, Clone, Debug, Copy)]
#[clap(rename_all = "lowercase")]
pub enum CargoEnv {
    Development,
    Production,
}

/// 事件监听器数据库操作模式
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub enum EventListenerDbMode {
    #[serde(rename = "update_only")]
    UpdateOnly,
    #[serde(rename = "upsert")]
    Upsert,
}

impl Default for EventListenerDbMode {
    fn default() -> Self {
        EventListenerDbMode::UpdateOnly
    }
}

impl From<&str> for EventListenerDbMode {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "upsert" => EventListenerDbMode::Upsert,
            _ => EventListenerDbMode::UpdateOnly,
        }
    }
}

/// 环境配置加载器
pub struct EnvLoader;

impl EnvLoader {
    /// 根据 CARGO_ENV 加载对应的环境配置文件
    pub fn load_env_file() -> Result<(), Box<dyn std::error::Error>> {
        // 1. 获取环境变量 CARGO_ENV development
        let cargo_env = std::env::var("CARGO_ENV").unwrap_or_else(|_| "development".to_string());
        println!("cargo_env: {}", cargo_env);
        // 2. 构建配置文件路径
        let env_file = match cargo_env.as_str() {
            "production" | "Production" | "prod" => ".env.production",
            "development" | "Development" | "dev" => ".env.development",
            "test" | "Test" => ".env.test",
            _ => {
                println!("⚠️  未知的 CARGO_ENV: {}，使用默认的 .env.development", cargo_env);
                ".env.development"
            }
        };
        println!("env_file: {}", env_file);
        // 3. 检查文件是否存在
        if !std::path::Path::new(env_file).exists() {
            eprintln!("⚠️  配置文件 {} 不存在，尝试加载默认的 .env 文件", env_file);
            // 回退到默认的 .env 文件
            if std::path::Path::new(".env").exists() {
                dotenvy::from_filename(".env")?;
                println!("✅ 已加载默认配置文件: .env");
            } else {
                eprintln!("❌ 未找到任何配置文件，使用默认配置");
            }
            return Ok(());
        }

        // 4. 加载指定的环境配置文件
        dotenvy::from_filename(env_file)?;
        println!("✅ 已加载环境配置文件: {} (CARGO_ENV={})", env_file, cargo_env);

        Ok(())
    }
}

#[derive(clap::Parser, Clone)]
pub struct AppConfig {
    #[clap(long, env, value_enum)]
    pub cargo_env: CargoEnv,

    #[clap(long, env, default_value = "0.0.0.0")]
    pub app_host: String,

    #[clap(long, env, default_value = "8000")]
    pub app_port: u16,

    #[clap(long, env, default_value = "mongodb://localhost:27017")]
    pub mongo_uri: String,

    #[clap(long, env)]
    pub mongo_db: String,

    #[clap(long, env, default_value = "https://api.devnet.solana.com")]
    pub rpc_url: String,

    #[clap(long, env)]
    pub private_key: Option<String>,

    #[clap(long, env, default_value = "FA1RJDDXysgwg5Gm3fJXWxt26JQzPkAzhTA114miqNUX")]
    pub raydium_program_id: String,

    /// CPMM程序ID（Constant Product Market Maker）
    #[clap(long, env, default_value = "DRaycpLY18LhpbydsBWbVJtxpNv9oXPgjRSfpF2bWpYb")]
    pub raydium_cp_program_id: String,

    #[clap(long, env, default_value = "0")]
    pub amm_config_index: u8,

    #[clap(long, env, default_value = "info")]
    pub rust_log: String,

    /// 是否允许事件监听器插入新池子记录
    #[clap(long, env, default_value = "false")]
    pub enable_pool_event_insert: bool,

    /// 事件监听器数据库操作模式
    #[clap(long, env, default_value = "update_only")]
    pub event_listener_db_mode: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        EnvLoader::load_env_file().ok();
        AppConfig::parse()
    }
}
impl AppConfig {
    /// 手动创建配置实例（用于测试）
    pub fn new_for_test() -> Self {
        Self {
            cargo_env: CargoEnv::Development,
            app_host: "0.0.0.0".to_string(),
            app_port: 8765,
            mongo_uri: std::env::var("MONGO_URI").unwrap_or_else(|_| "mongodb://localhost:27017".to_string()),
            mongo_db: std::env::var("MONGO_DB").unwrap_or_else(|_| "test_db".to_string()),
            rpc_url: "https://api.devnet.solana.com".to_string(),
            private_key: None,
            raydium_program_id: "FA1RJDDXysgwg5Gm3fJXWxt26JQzPkAzhTA114miqNUX".to_string(),
            raydium_cp_program_id: std::env::var("RAYDIUM_CP_PROGRAM_ID")
                .unwrap_or_else(|_| "DRaycpLY18LhpbydsBWbVJtxpNv9oXPgjRSfpF2bWpYb".to_string()),
            amm_config_index: 0,
            rust_log: "info".to_string(),
            enable_pool_event_insert: std::env::var("ENABLE_POOL_EVENT_INSERT")
                .unwrap_or_else(|_| "false".to_string())
                .parse()
                .unwrap_or(false),
            event_listener_db_mode: std::env::var("EVENT_LISTENER_DB_MODE")
                .unwrap_or_else(|_| "update_only".to_string()),
        }
    }

    /// 获取事件监听器数据库操作模式
    pub fn get_event_listener_db_mode(&self) -> EventListenerDbMode {
        EventListenerDbMode::from(self.event_listener_db_mode.as_str())
    }
}
