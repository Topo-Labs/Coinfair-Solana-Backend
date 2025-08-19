use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use utoipa::ToSchema;

/// JWT Claims 结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// 用户ID
    pub sub: String,
    /// Solana钱包地址(可选)
    pub wallet: Option<String>,
    /// 权限列表
    pub permissions: Vec<String>,
    /// 用户等级
    pub tier: UserTier,
    /// 过期时间
    pub exp: u64,
    /// 签发时间
    pub iat: u64,
    /// 签发者
    pub iss: String,
}

/// 用户等级枚举
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum UserTier {
    Basic,
    Premium,
    VIP,
    Admin,
}

impl UserTier {
    /// 获取等级对应的速率限制倍数
    pub fn rate_limit_multiplier(&self) -> u32 {
        match self {
            UserTier::Basic => 1,
            UserTier::Premium => 5,
            UserTier::VIP => 20,
            UserTier::Admin => 100,
        }
    }
}

/// API权限枚举
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Permission {
    // 读取权限
    ReadUser,
    ReadPool,
    ReadPosition,
    ReadReward,

    // 写入权限
    CreateUser,
    CreatePool,
    CreatePosition,
    ManageReward,

    // 管理权限
    AdminConfig,
    SystemMonitor,
    UserManagement,
}

impl Permission {
    pub fn as_str(&self) -> &'static str {
        match self {
            Permission::ReadUser => "read:user",
            Permission::ReadPool => "read:pool",
            Permission::ReadPosition => "read:position",
            Permission::ReadReward => "read:reward",
            Permission::CreateUser => "create:user",
            Permission::CreatePool => "create:pool",
            Permission::CreatePosition => "create:position",
            Permission::ManageReward => "manage:reward",
            Permission::AdminConfig => "admin:config",
            Permission::SystemMonitor => "admin:monitor",
            Permission::UserManagement => "admin:users",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "read:user" => Some(Permission::ReadUser),
            "read:pool" => Some(Permission::ReadPool),
            "read:position" => Some(Permission::ReadPosition),
            "read:reward" => Some(Permission::ReadReward),
            "create:user" => Some(Permission::CreateUser),
            "create:pool" => Some(Permission::CreatePool),
            "create:position" => Some(Permission::CreatePosition),
            "manage:reward" => Some(Permission::ManageReward),
            "admin:config" => Some(Permission::AdminConfig),
            "admin:monitor" => Some(Permission::SystemMonitor),
            "admin:users" => Some(Permission::UserManagement),
            _ => None,
        }
    }
}

/// 认证用户信息
#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user_id: String,
    pub wallet_address: Option<String>,
    pub tier: UserTier,
    pub permissions: HashSet<Permission>,
}

impl AuthUser {
    pub fn has_permission(&self, permission: &Permission) -> bool {
        self.permissions.contains(permission)
    }

    pub fn has_any_permission(&self, permissions: &[Permission]) -> bool {
        permissions.iter().any(|p| self.permissions.contains(p))
    }

    pub fn is_admin(&self) -> bool {
        self.tier == UserTier::Admin
    }
}

/// Solana钱包登录请求
#[derive(Debug, Deserialize, ToSchema)]
pub struct SolanaLoginRequest {
    /// 钱包地址
    pub wallet_address: String,
    /// 签名的消息
    pub message: String,
    /// 消息签名
    pub signature: String,
}

/// 认证响应
#[derive(Debug, Serialize, ToSchema)]
pub struct AuthResponse {
    /// JWT访问令牌
    pub access_token: String,
    /// 令牌类型
    pub token_type: String,
    /// 过期时间(秒)
    pub expires_in: u64,
    /// 用户信息
    pub user: UserInfo,
}

/// 用户信息响应
#[derive(Debug, Serialize, ToSchema)]
pub struct UserInfo {
    pub user_id: String,
    pub wallet_address: Option<String>,
    pub tier: UserTier,
    pub permissions: Vec<String>,
}

/// API密钥配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyConfig {
    pub key_id: String,
    pub user_id: String,
    pub permissions: Vec<Permission>,
    pub rate_limits: RateLimitConfig,
    pub created_at: u64,
    pub expires_at: Option<u64>,
    pub is_active: bool,
}

/// 速率限制配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    /// 每分钟请求数
    pub requests_per_minute: u32,
    /// 每小时请求数
    pub requests_per_hour: u32,
    /// 每天请求数
    pub requests_per_day: u32,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            requests_per_minute: 100,
            requests_per_hour: 1000,
            requests_per_day: 10000,
        }
    }
}

/// 认证配置
#[derive(Debug, Clone)]
pub struct AuthConfig {
    pub jwt_secret: String,
    pub jwt_expires_in_hours: u64,
    pub solana_auth_message_ttl: u64,
    pub redis_url: Option<String>,
    pub rate_limit_redis_prefix: String,
    /// 认证开关：true时禁用认证，false时启用认证
    pub auth_disabled: bool,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            jwt_secret: std::env::var("JWT_SECRET").expect("JWT_SECRET environment variable is required"),
            jwt_expires_in_hours: std::env::var("JWT_EXPIRES_IN_HOURS")
                .unwrap_or_else(|_| "24".to_string())
                .parse()
                .unwrap_or(24),
            solana_auth_message_ttl: std::env::var("SOLANA_AUTH_MESSAGE_TTL")
                .unwrap_or_else(|_| "300".to_string())
                .parse()
                .unwrap_or(300),
            redis_url: std::env::var("REDIS_URL").ok(),
            rate_limit_redis_prefix: std::env::var("RATE_LIMIT_REDIS_PREFIX")
                .unwrap_or_else(|_| "coinfair:ratelimit".to_string()),
            auth_disabled: std::env::var("AUTH_DISABLED")
                .unwrap_or_else(|_| "false".to_string())
                .parse()
                .unwrap_or(false),
        }
    }
}
