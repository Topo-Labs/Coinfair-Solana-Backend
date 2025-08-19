use crate::auth::{AuthUser, UserTier};
use anyhow::Result;
use axum::{
    extract::{ConnectInfo, Request},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use redis::{AsyncCommands, Client};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::sync::RwLock;
use tracing::{debug, warn};

/// 速率限制配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    /// 每分钟请求数
    pub requests_per_minute: u32,
    /// 每小时请求数
    pub requests_per_hour: u32,
    /// 每天请求数
    pub requests_per_day: u32,
    /// 突发请求数（允许在短时间内超过分钟限制）
    pub burst_limit: u32,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            requests_per_minute: 60,
            requests_per_hour: 1000,
            requests_per_day: 10000,
            burst_limit: 100,
        }
    }
}

/// 速率限制键类型
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum RateLimitKey {
    /// IP地址限制
    Ip(String),
    /// 用户限制
    User(String),
    /// 端点限制
    Endpoint(String),
    /// 组合限制（用户+端点）
    UserEndpoint(String, String),
    /// API密钥限制
    ApiKey(String),
}

impl RateLimitKey {
    pub fn to_redis_key(&self, prefix: &str, window: &str) -> String {
        match self {
            RateLimitKey::Ip(ip) => format!("{}:ip:{}:{}", prefix, ip, window),
            RateLimitKey::User(user_id) => format!("{}:user:{}:{}", prefix, user_id, window),
            RateLimitKey::Endpoint(endpoint) => format!("{}:endpoint:{}:{}", prefix, endpoint, window),
            RateLimitKey::UserEndpoint(user_id, endpoint) => {
                format!("{}:user_endpoint:{}:{}:{}", prefix, user_id, endpoint, window)
            }
            RateLimitKey::ApiKey(key_id) => format!("{}:api_key:{}:{}", prefix, key_id, window),
        }
    }
}

/// 速率限制窗口
#[derive(Debug, Clone)]
pub enum TimeWindow {
    Minute,
    Hour,
    Day,
}

impl TimeWindow {
    pub fn as_str(&self) -> &'static str {
        match self {
            TimeWindow::Minute => "minute",
            TimeWindow::Hour => "hour",
            TimeWindow::Day => "day",
        }
    }

    pub fn duration_seconds(&self) -> u64 {
        match self {
            TimeWindow::Minute => 60,
            TimeWindow::Hour => 3600,
            TimeWindow::Day => 86400,
        }
    }

    pub fn get_window_start(&self, timestamp: u64) -> u64 {
        match self {
            TimeWindow::Minute => (timestamp / 60) * 60,
            TimeWindow::Hour => (timestamp / 3600) * 3600,
            TimeWindow::Day => (timestamp / 86400) * 86400,
        }
    }
}

/// 速率限制记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitRecord {
    pub count: u32,
    pub window_start: u64,
    pub last_request: u64,
}

/// 内存速率限制存储
type MemoryStore = Arc<RwLock<HashMap<String, RateLimitRecord>>>;

/// 速率限制服务
pub struct RateLimitService {
    redis_client: Option<Client>,
    memory_store: MemoryStore,
    redis_prefix: String,
}

impl RateLimitService {
    pub fn new(redis_url: Option<String>, redis_prefix: String) -> Result<Self> {
        let redis_client = if let Some(url) = redis_url {
            Some(Client::open(url)?)
        } else {
            None
        };

        let service = Self {
            redis_client,
            memory_store: Arc::new(RwLock::new(HashMap::new())),
            redis_prefix,
        };

        // 启动内存清理任务
        if service.redis_client.is_none() {
            service.start_memory_cleanup();
        }

        Ok(service)
    }

    /// 检查速率限制
    pub async fn check_rate_limit(&self, key: &RateLimitKey, config: &RateLimitConfig) -> Result<RateLimitResult> {
        let current_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();

        // 检查分钟限制
        let minute_result = self
            .check_window_limit(key, &TimeWindow::Minute, config.requests_per_minute, current_time)
            .await?;

        if !minute_result.allowed {
            return Ok(minute_result);
        }

        Ok(minute_result)
    }

    /// 检查单个时间窗口的限制
    async fn check_window_limit(
        &self,
        key: &RateLimitKey,
        window: &TimeWindow,
        limit: u32,
        current_time: u64,
    ) -> Result<RateLimitResult> {
        let window_start = window.get_window_start(current_time);
        let redis_key = key.to_redis_key(&self.redis_prefix, window.as_str());

        if let Some(client) = &self.redis_client {
            self.check_redis_limit(client, &redis_key, limit, window_start, current_time)
                .await
        } else {
            self.check_memory_limit(&redis_key, limit, window_start, current_time)
                .await
        }
    }

    /// 使用Redis检查限制
    async fn check_redis_limit(
        &self,
        client: &Client,
        key: &str,
        limit: u32,
        window_start: u64,
        _current_time: u64,
    ) -> Result<RateLimitResult> {
        let mut conn = client.get_multiplexed_async_connection().await?;

        // 简单的Redis计数实现
        let current_count: u32 = conn.incr(key, 1).await.unwrap_or(0);

        if current_count == 1 {
            // 设置过期时间
            let _: () = conn.expire(key, 60).await?;
        }

        Ok(RateLimitResult {
            allowed: current_count <= limit,
            count: current_count,
            limit,
            window_start,
            reset_time: window_start + 60,
        })
    }

    /// 使用内存检查限制
    async fn check_memory_limit(
        &self,
        key: &str,
        limit: u32,
        window_start: u64,
        current_time: u64,
    ) -> Result<RateLimitResult> {
        let mut store = self.memory_store.write().await;

        let record = store.entry(key.to_string()).or_insert(RateLimitRecord {
            count: 0,
            window_start,
            last_request: current_time,
        });

        // 如果窗口已经更新，重置计数
        if record.window_start < window_start {
            record.count = 0;
            record.window_start = window_start;
        }

        // 检查是否超过限制
        if record.count >= limit {
            return Ok(RateLimitResult {
                allowed: false,
                count: record.count,
                limit,
                window_start: record.window_start,
                reset_time: window_start + 60,
            });
        }

        // 增加计数
        record.count += 1;
        record.last_request = current_time;

        Ok(RateLimitResult {
            allowed: true,
            count: record.count,
            limit,
            window_start: record.window_start,
            reset_time: window_start + 60,
        })
    }

    /// 启动内存清理任务
    fn start_memory_cleanup(&self) {
        let store = Arc::clone(&self.memory_store);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(300));

            loop {
                interval.tick().await;
                let current_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();

                let mut store_guard = store.write().await;
                store_guard.retain(|_, record| current_time - record.last_request < 3600);
            }
        });
    }
}

/// 速率限制结果
#[derive(Debug, Clone)]
pub struct RateLimitResult {
    pub allowed: bool,
    pub count: u32,
    pub limit: u32,
    pub window_start: u64,
    pub reset_time: u64,
}

/// 多维度速率限制中间件
pub struct MultiDimensionalRateLimit {
    service: Arc<RateLimitService>,
    tier_configs: HashMap<UserTier, RateLimitConfig>,
    default_config: RateLimitConfig,
}

impl MultiDimensionalRateLimit {
    pub fn new(
        service: RateLimitService,
        tier_configs: Option<HashMap<UserTier, RateLimitConfig>>,
        _endpoint_configs: Option<HashMap<String, RateLimitConfig>>,
    ) -> Self {
        let default_tier_configs = HashMap::from([
            (
                UserTier::Basic,
                RateLimitConfig {
                    requests_per_minute: 30,
                    requests_per_hour: 500,
                    requests_per_day: 5000,
                    burst_limit: 50,
                },
            ),
            (
                UserTier::Premium,
                RateLimitConfig {
                    requests_per_minute: 100,
                    requests_per_hour: 2000,
                    requests_per_day: 20000,
                    burst_limit: 150,
                },
            ),
            (
                UserTier::VIP,
                RateLimitConfig {
                    requests_per_minute: 300,
                    requests_per_hour: 10000,
                    requests_per_day: 100000,
                    burst_limit: 500,
                },
            ),
            (
                UserTier::Admin,
                RateLimitConfig {
                    requests_per_minute: 1000,
                    requests_per_hour: 50000,
                    requests_per_day: 500000,
                    burst_limit: 2000,
                },
            ),
        ]);

        Self {
            service: Arc::new(service),
            tier_configs: tier_configs.unwrap_or(default_tier_configs),
            default_config: RateLimitConfig::default(),
        }
    }

    /// 速率限制中间件函数
    pub async fn middleware(&self, request: Request, next: Next) -> Result<Response, StatusCode> {
        let uri = request.uri().path().to_string();
        let client_ip = self.extract_client_ip(&request);
        let auth_user = request.extensions().get::<AuthUser>().cloned();

        debug!("🔒 Rate limit middleware called for IP: {} | Path: {}", client_ip, uri);

        // 基础IP限制（防止滥用）
        let ip_config = RateLimitConfig {
            requests_per_minute: 200,
            requests_per_hour: 5000,
            requests_per_day: 50000,
            burst_limit: 300,
        };

        let ip_result = self
            .service
            .check_rate_limit(&RateLimitKey::Ip(client_ip.clone()), &ip_config)
            .await
            .map_err(|e| {
                warn!("Rate limit check failed for IP {}: {}", client_ip, e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        if !ip_result.allowed {
            debug!("Rate limit exceeded for IP: {}", client_ip);
            return Err(StatusCode::TOO_MANY_REQUESTS);
        }

        // 如果用户已认证，应用用户级别的限制
        if let Some(user) = &auth_user {
            let user_config = self
                .tier_configs
                .get(&user.tier)
                .unwrap_or(&self.default_config)
                .clone();

            // 用户总体限制
            let user_result = self
                .service
                .check_rate_limit(&RateLimitKey::User(user.user_id.clone()), &user_config)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

            if !user_result.allowed {
                debug!("Rate limit exceeded for user: {}", user.user_id);
                return Err(StatusCode::TOO_MANY_REQUESTS);
            }
        }

        Ok(next.run(request).await)
    }

    /// 提取客户端IP地址
    fn extract_client_ip(&self, request: &Request) -> String {
        // 尝试从X-Forwarded-For头部获取真实IP
        if let Some(forwarded_for) = request.headers().get("x-forwarded-for") {
            if let Ok(forwarded_str) = forwarded_for.to_str() {
                if let Some(first_ip) = forwarded_str.split(',').next() {
                    return first_ip.trim().to_string();
                }
            }
        }

        // 尝试从X-Real-IP头部获取
        if let Some(real_ip) = request.headers().get("x-real-ip") {
            if let Ok(ip_str) = real_ip.to_str() {
                return ip_str.to_string();
            }
        }

        // 使用连接信息中的IP
        request
            .extensions()
            .get::<ConnectInfo<SocketAddr>>()
            .map(|connect_info| connect_info.0.ip().to_string())
            .unwrap_or_else(|| "unknown".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_memory_rate_limit_service() {
        let service = RateLimitService::new(None, "test".to_string()).unwrap();
        let key = RateLimitKey::Ip("127.0.0.1".to_string());
        let config = RateLimitConfig {
            requests_per_minute: 2,
            requests_per_hour: 10,
            requests_per_day: 100,
            burst_limit: 5,
        };

        // 第一次请求应该通过
        let result1 = service.check_rate_limit(&key, &config).await.unwrap();
        assert!(result1.allowed);
        assert_eq!(result1.count, 1);

        // 第二次请求应该通过
        let result2 = service.check_rate_limit(&key, &config).await.unwrap();
        assert!(result2.allowed);
        assert_eq!(result2.count, 2);

        // 第三次请求应该被限制
        let result3 = service.check_rate_limit(&key, &config).await.unwrap();
        assert!(!result3.allowed);
        assert_eq!(result3.count, 2);
    }

    #[test]
    fn test_rate_limit_key_generation() {
        let key = RateLimitKey::UserEndpoint("user123".to_string(), "/api/v1/swap".to_string());
        let redis_key = key.to_redis_key("coinfair", "minute");
        assert_eq!(redis_key, "coinfair:user_endpoint:user123:/api/v1/swap:minute");
    }

    #[test]
    fn test_time_window_calculations() {
        let window = TimeWindow::Minute;
        let timestamp = 1640995200; // 2022-01-01 00:00:00 UTC
        let window_start = window.get_window_start(timestamp + 30); // 30 seconds later
        assert_eq!(window_start, timestamp);
    }

    #[tokio::test]
    async fn test_multi_dimensional_rate_limit() {
        let service = RateLimitService::new(None, "test".to_string()).unwrap();
        let rate_limiter = MultiDimensionalRateLimit::new(service, None, None);

        // 测试用户配置
        let basic_config = rate_limiter.tier_configs.get(&UserTier::Basic).unwrap();
        assert_eq!(basic_config.requests_per_minute, 30);

        let vip_config = rate_limiter.tier_configs.get(&UserTier::VIP).unwrap();
        assert_eq!(vip_config.requests_per_minute, 300);
    }
}
