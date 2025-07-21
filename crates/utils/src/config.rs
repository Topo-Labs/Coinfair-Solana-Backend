use clap::Parser;

#[derive(clap::ValueEnum, Clone, Debug, Copy)]
pub enum CargoEnv {
    Development,
    Production,
}

/// 环境配置加载器
pub struct EnvLoader;

impl EnvLoader {
    /// 根据 CARGO_ENV 加载对应的环境配置文件
    pub fn load_env_file() -> Result<(), Box<dyn std::error::Error>> {
        use std::env;
        use std::path::Path;

        // 1. 获取环境变量 CARGO_ENV development
        let cargo_env = env::var("CARGO_ENV").unwrap_or_else(|_| "development".to_string());
        println!("cargo_env: {}", cargo_env);
        // 2. 构建配置文件路径
        let env_file = match cargo_env.as_str() {
            "production" | "prod" => ".env.production",
            "development" | "dev" => ".env.development",
            "test" => ".env.test",
            _ => {
                println!("⚠️  未知的 CARGO_ENV: {}，使用默认的 .env.development", cargo_env);
                ".env.development"
            }
        };
        println!("env_file: {}", env_file);
        // 3. 检查文件是否存在
        if !Path::new(env_file).exists() {
            eprintln!("⚠️  配置文件 {} 不存在，尝试加载默认的 .env 文件", env_file);
            // 回退到默认的 .env 文件
            if Path::new(".env").exists() {
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

#[derive(clap::Parser)]
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

    #[clap(long, env, default_value = "0")]
    pub amm_config_index: u8,

    #[clap(long, env, default_value = "info")]
    pub rust_log: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        EnvLoader::load_env_file().ok();
        AppConfig::parse()
    }
}
