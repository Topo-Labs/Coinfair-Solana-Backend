use crate::error::{EventListenerError, Result};
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use std::{str::FromStr, time::Duration};
use tracing::info;

/// Event-Listener配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventListenerConfig {
    /// Solana配置
    pub solana: SolanaConfig,
    /// 数据库配置
    pub database: DatabaseConfig,
    /// 监听器配置
    pub listener: ListenerConfig,
    /// 监控配置
    pub monitoring: MonitoringConfig,
}

/// Solana网络配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolanaConfig {
    /// RPC URL
    pub rpc_url: String,
    /// WebSocket URL
    pub ws_url: String,
    /// Commitment level (confirmed, finalized)
    pub commitment: String,
    /// 目标程序ID列表 (要监听的合约地址)
    pub program_ids: Vec<Pubkey>,
    /// 签名者私钥 (可选，用于发送交易)
    pub private_key: Option<String>,
}

/// 数据库配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    /// MongoDB连接字符串
    pub uri: String,
    /// 数据库名称
    pub database_name: String,
    /// 最大连接数
    pub max_connections: u32,
    /// 最小连接数
    pub min_connections: u32,
}

/// 监听器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListenerConfig {
    /// 批量处理大小
    pub batch_size: usize,
    /// 同步间隔（秒）
    pub sync_interval_secs: u64,
    /// 最大重试次数
    pub max_retries: u32,
    /// 重试延迟（毫秒）
    pub retry_delay_ms: u64,
    /// 签名缓存大小
    pub signature_cache_size: usize,
    /// 检查点保存间隔（秒）
    pub checkpoint_save_interval_secs: u64,
    /// WebSocket重连退避配置
    pub backoff: BackoffConfig,
    /// 批量写入配置
    pub batch_write: BatchWriteConfig,
}

/// 监控配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    /// 指标收集间隔（秒）
    pub metrics_interval_secs: u64,
    /// 是否启用性能监控
    pub enable_performance_monitoring: bool,
    /// 健康检查间隔（秒）
    pub health_check_interval_secs: u64,
}

/// 退避重连配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackoffConfig {
    /// 初始延迟（毫秒）
    pub initial_delay_ms: u64,
    /// 最大延迟（毫秒）
    pub max_delay_ms: u64,
    /// 延迟倍数
    pub multiplier: f64,
    /// 最大重试次数
    pub max_retries: Option<u32>,
}

/// 批量写入配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchWriteConfig {
    /// 批量大小
    pub batch_size: usize,
    /// 最大等待时间（毫秒）
    pub max_wait_ms: u64,
    /// 缓冲区大小
    pub buffer_size: usize,
    /// 并发写入线程数
    pub concurrent_writers: usize,
}

impl EventListenerConfig {
    /// 从环境变量加载配置
    pub async fn from_env() -> Result<Self> {
        info!("🔧 从环境变量加载Event-Listener配置...");

        // 加载环境配置文件（避免clap参数解析冲突）
        Self::load_env_file_safe();

        // 加载Solana配置
        let solana = SolanaConfig {
            rpc_url: std::env::var("RPC_URL").unwrap_or_else(|_| "https://api.devnet.solana.com".to_string()),
            ws_url: Self::derive_ws_url(&std::env::var("RPC_URL").unwrap_or_else(|_| "https://api.devnet.solana.com".to_string()))?,
            commitment: std::env::var("SOLANA_COMMITMENT").unwrap_or_else(|_| "confirmed".to_string()),
            program_ids: Self::parse_program_ids()?,
            private_key: std::env::var("PRIVATE_KEY").ok(),
        };

        // 加载数据库配置
        let database = DatabaseConfig {
            uri: std::env::var("MONGO_URI").unwrap_or_else(|_| "mongodb://localhost:27017".to_string()),
            database_name: std::env::var("MONGO_DB").unwrap_or_else(|_| "coinfair_development".to_string()),
            max_connections: std::env::var("MONGO_MAX_CONNECTIONS").unwrap_or_else(|_| "10".to_string()).parse().unwrap_or(10),
            min_connections: std::env::var("MONGO_MIN_CONNECTIONS").unwrap_or_else(|_| "2".to_string()).parse().unwrap_or(2),
        };

        // 加载监听器配置
        let listener = ListenerConfig {
            batch_size: std::env::var("EVENT_BATCH_SIZE").unwrap_or_else(|_| "100".to_string()).parse().unwrap_or(100),
            sync_interval_secs: std::env::var("EVENT_SYNC_INTERVAL_SECS").unwrap_or_else(|_| "30".to_string()).parse().unwrap_or(30),
            max_retries: std::env::var("EVENT_MAX_RETRIES").unwrap_or_else(|_| "3".to_string()).parse().unwrap_or(3),
            retry_delay_ms: std::env::var("EVENT_RETRY_DELAY_MS").unwrap_or_else(|_| "1000".to_string()).parse().unwrap_or(1000),
            signature_cache_size: std::env::var("EVENT_SIGNATURE_CACHE_SIZE").unwrap_or_else(|_| "10000".to_string()).parse().unwrap_or(10000),
            checkpoint_save_interval_secs: std::env::var("EVENT_CHECKPOINT_INTERVAL_SECS").unwrap_or_else(|_| "60".to_string()).parse().unwrap_or(60),
            backoff: BackoffConfig {
                initial_delay_ms: std::env::var("EVENT_BACKOFF_INITIAL_MS").unwrap_or_else(|_| "1000".to_string()).parse().unwrap_or(1000),
                max_delay_ms: std::env::var("EVENT_BACKOFF_MAX_MS").unwrap_or_else(|_| "300000".to_string()).parse().unwrap_or(300000),
                multiplier: std::env::var("EVENT_BACKOFF_MULTIPLIER").unwrap_or_else(|_| "2.0".to_string()).parse().unwrap_or(2.0),
                max_retries: std::env::var("EVENT_BACKOFF_MAX_RETRIES").ok().and_then(|s| s.parse().ok()),
            },
            batch_write: BatchWriteConfig {
                batch_size: std::env::var("EVENT_BATCH_WRITE_SIZE").unwrap_or_else(|_| "50".to_string()).parse().unwrap_or(50),
                max_wait_ms: std::env::var("EVENT_BATCH_WRITE_WAIT_MS").unwrap_or_else(|_| "5000".to_string()).parse().unwrap_or(5000),
                buffer_size: std::env::var("EVENT_BATCH_WRITE_BUFFER_SIZE")
                    .unwrap_or_else(|_| "1000".to_string())
                    .parse()
                    .unwrap_or(1000),
                concurrent_writers: std::env::var("EVENT_BATCH_WRITE_CONCURRENT").unwrap_or_else(|_| "4".to_string()).parse().unwrap_or(4),
            },
        };

        // 加载监控配置
        let monitoring = MonitoringConfig {
            metrics_interval_secs: std::env::var("EVENT_METRICS_INTERVAL_SECS").unwrap_or_else(|_| "60".to_string()).parse().unwrap_or(60),
            enable_performance_monitoring: std::env::var("EVENT_ENABLE_PERFORMANCE_MONITORING")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .unwrap_or(true),
            health_check_interval_secs: std::env::var("EVENT_HEALTH_CHECK_INTERVAL_SECS").unwrap_or_else(|_| "30".to_string()).parse().unwrap_or(30),
        };

        let config = Self {
            solana,
            database,
            listener,
            monitoring,
        };

        info!("✅ Event-Listener配置加载完成");
        for (i, program_id) in config.solana.program_ids.iter().enumerate() {
            info!("🔗 监听程序 {}: {}", i + 1, program_id);
        }
        info!("🌐 RPC URL: {}", config.solana.rpc_url);
        info!("🔌 WebSocket URL: {}", config.solana.ws_url);
        info!("📊 数据库: {}", config.database.database_name);

        Ok(config)
    }

    /// 解析程序ID列表从环境变量
    fn parse_program_ids() -> Result<Vec<Pubkey>> {
        use std::collections::HashSet;
        
        // 1. 优先使用新格式 SUBSCRIBED_PROGRAM_IDS（逗号分隔）
        if let Ok(ids_str) = std::env::var("SUBSCRIBED_PROGRAM_IDS") {
            let ids: std::result::Result<Vec<Pubkey>, solana_sdk::pubkey::ParsePubkeyError> = ids_str
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(|id| Pubkey::from_str(id))
                .collect();
            
            match ids {
                Ok(parsed_ids) => {
                    if parsed_ids.is_empty() {
                        return Err(EventListenerError::Config("SUBSCRIBED_PROGRAM_IDS不能为空".to_string()));
                    }
                    if parsed_ids.len() > 10 {
                        return Err(EventListenerError::Config("最多支持10个程序ID".to_string()));
                    }
                    
                    // 验证程序ID去重
                    let mut unique_ids = HashSet::new();
                    for id in &parsed_ids {
                        if !unique_ids.insert(*id) {
                            return Err(EventListenerError::Config(format!("程序ID重复: {}", id)));
                        }
                    }
                    
                    info!("📋 解析到{}个程序ID: {:?}", parsed_ids.len(), parsed_ids);
                    return Ok(parsed_ids);
                }
                Err(e) => return Err(EventListenerError::Config(format!("解析SUBSCRIBED_PROGRAM_IDS失败: {}", e))),
            }
        }
        
        // 2. 向后兼容：支持单个程序ID格式
        if let Ok(id_str) = std::env::var("SUBSCRIBED_PROGRAM_ID") {
            let id = Pubkey::from_str(&id_str)
                .map_err(|e| EventListenerError::Config(format!("解析SUBSCRIBED_PROGRAM_ID失败: {}", e)))?;
            info!("📋 使用单程序ID（兼容模式）: {}", id);
            return Ok(vec![id]);
        }
        
        Err(EventListenerError::Config(
            "必须设置SUBSCRIBED_PROGRAM_IDS（多个，逗号分隔）或SUBSCRIBED_PROGRAM_ID（单个）环境变量".to_string()
        ))
    }

    /// 安全地加载环境配置文件，避免clap参数解析冲突
    fn load_env_file_safe() {
        use std::env;
        use std::path::Path;

        // 1. 获取环境变量 CARGO_ENV
        let cargo_env = env::var("CARGO_ENV").unwrap_or_else(|_| "development".to_string());
        info!("cargo_env: {}", cargo_env);

        // 2. 构建配置文件路径
        let env_file = match cargo_env.as_str() {
            "production" | "Production" | "prod" => ".env.production",
            "development" | "Development" | "dev" => ".env.development",
            "test" | "Test" => ".env.test",
            _ => {
                info!("⚠️  未知的 CARGO_ENV: {}，使用默认的 .env.development", cargo_env);
                ".env.development"
            }
        };
        info!("env_file: {}", env_file);

        // 3. 检查文件是否存在
        if !Path::new(env_file).exists() {
            info!("⚠️  配置文件 {} 不存在，尝试加载默认的 .env 文件", env_file);
            // 回退到默认的 .env 文件
            if Path::new(".env").exists() {
                if let Err(e) = dotenvy::from_filename(".env") {
                    info!("⚠️  加载 .env 文件失败: {}", e);
                } else {
                    info!("✅ 已加载默认配置文件: .env");
                }
            } else {
                info!("❌ 未找到任何配置文件，使用默认配置");
            }
            return;
        }

        // 4. 加载指定的环境配置文件
        if let Err(e) = dotenvy::from_filename(env_file) {
            info!("⚠️  加载配置文件 {} 失败: {}", env_file, e);
        } else {
            info!("✅ 已加载环境配置文件: {} (CARGO_ENV={})", env_file, cargo_env);
        }
    }

    /// 从RPC URL推导WebSocket URL
    fn derive_ws_url(rpc_url: &str) -> Result<String> {
        let ws_url = rpc_url.replace("https://", "wss://").replace("http://", "ws://");
        Ok(ws_url)
    }

    /// 获取重连退避Duration
    pub fn get_initial_backoff_delay(&self) -> Duration {
        Duration::from_millis(self.listener.backoff.initial_delay_ms)
    }

    /// 获取最大退避延迟
    pub fn get_max_backoff_delay(&self) -> Duration {
        Duration::from_millis(self.listener.backoff.max_delay_ms)
    }

    /// 获取批量写入等待时间
    pub fn get_batch_write_wait_duration(&self) -> Duration {
        Duration::from_millis(self.listener.batch_write.max_wait_ms)
    }

    /// 获取同步间隔Duration
    pub fn get_sync_interval(&self) -> Duration {
        Duration::from_secs(self.listener.sync_interval_secs)
    }

    /// 获取检查点保存间隔
    pub fn get_checkpoint_save_interval(&self) -> Duration {
        Duration::from_secs(self.listener.checkpoint_save_interval_secs)
    }

    /// 获取指标收集间隔
    pub fn get_metrics_interval(&self) -> Duration {
        Duration::from_secs(self.monitoring.metrics_interval_secs)
    }

    /// 获取健康检查间隔
    pub fn get_health_check_interval(&self) -> Duration {
        Duration::from_secs(self.monitoring.health_check_interval_secs)
    }

    /// 验证配置的有效性
    pub fn validate(&self) -> Result<()> {
        // 验证URL格式
        if !self.solana.rpc_url.starts_with("http") {
            return Err(EventListenerError::Config("RPC URL必须以http或https开头".to_string()));
        }

        if !self.solana.ws_url.starts_with("ws") {
            return Err(EventListenerError::Config("WebSocket URL必须以ws或wss开头".to_string()));
        }

        // 验证程序ID列表
        if self.solana.program_ids.is_empty() {
            return Err(EventListenerError::Config("至少需要配置一个程序ID".to_string()));
        }
        
        if self.solana.program_ids.len() > 10 {
            return Err(EventListenerError::Config("最多支持10个程序ID".to_string()));
        }
        
        // 验证程序ID去重（双重保险）
        use std::collections::HashSet;
        let mut unique_ids = HashSet::new();
        for id in &self.solana.program_ids {
            if !unique_ids.insert(*id) {
                return Err(EventListenerError::Config(format!("程序ID重复: {}", id)));
            }
        }

        // 验证批量配置
        if self.listener.batch_size == 0 {
            return Err(EventListenerError::Config("批量大小必须大于0".to_string()));
        }

        if self.listener.batch_write.batch_size == 0 {
            return Err(EventListenerError::Config("批量写入大小必须大于0".to_string()));
        }

        // 验证连接池配置
        if self.database.max_connections <= self.database.min_connections {
            return Err(EventListenerError::Config("最大连接数必须大于最小连接数".to_string()));
        }

        Ok(())
    }
}

impl Default for BackoffConfig {
    fn default() -> Self {
        Self {
            initial_delay_ms: 1000,
            max_delay_ms: 300000,
            multiplier: 2.0,
            max_retries: None,
        }
    }
}

impl Default for BatchWriteConfig {
    fn default() -> Self {
        Self {
            batch_size: 50,
            max_wait_ms: 5000,
            buffer_size: 1000,
            concurrent_writers: 4,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_derive_ws_url() {
        assert_eq!(EventListenerConfig::derive_ws_url("https://api.devnet.solana.com").unwrap(), "wss://api.devnet.solana.com");
        assert_eq!(EventListenerConfig::derive_ws_url("http://localhost:8899").unwrap(), "ws://localhost:8899");
    }

    #[test]
    fn test_backoff_config_default() {
        let config = BackoffConfig::default();
        assert_eq!(config.initial_delay_ms, 1000);
        assert_eq!(config.max_delay_ms, 300000);
        assert_eq!(config.multiplier, 2.0);
        assert_eq!(config.max_retries, None);
    }

    #[test]
    fn test_batch_write_config_default() {
        let config = BatchWriteConfig::default();
        assert_eq!(config.batch_size, 50);
        assert_eq!(config.max_wait_ms, 5000);
        assert_eq!(config.buffer_size, 1000);
        assert_eq!(config.concurrent_writers, 4);
    }

    #[tokio::test]
    async fn test_config_validation() {
        // 设置测试环境变量
        env::set_var("RAYDIUM_PROGRAM_ID", "FA1RJDDXysgwg5Gm3fJXWxt26JQzPkAzhTA114miqNUX");

        let config = EventListenerConfig::from_env().await.unwrap();
        assert!(config.validate().is_ok());

        // 清理环境变量
        env::remove_var("RAYDIUM_PROGRAM_ID");
    }
}
