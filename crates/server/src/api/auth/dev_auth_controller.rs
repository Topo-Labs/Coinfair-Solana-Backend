use crate::auth::{JwtManager, Permission, UserInfo, UserTier};
use axum::{
    extract::Extension,
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{info, warn};
use utoipa::ToSchema;

/// 开发者认证控制器 - 仅用于开发和测试环境
pub struct DevAuthController;

impl DevAuthController {
    /// 创建开发者认证路由
    /// 注意：这些接口仅在开发环境中可用
    pub fn routes() -> Router {
        Router::new()
            .route("/dev/admin-token", post(generate_admin_token))
            .route("/dev/user-token", post(generate_user_token))
            .route("/dev/token-info", get(get_token_info))
    }
}

/// 管理员令牌生成请求
#[derive(Debug, Deserialize, ToSchema)]
pub struct AdminTokenRequest {
    /// 可选的用户ID，默认为 "dev_admin"
    pub user_id: Option<String>,
    /// 可选的钱包地址
    pub wallet_address: Option<String>,
    /// 令牌有效期（小时），默认24小时
    pub expires_in_hours: Option<u64>,
}

/// 用户令牌生成请求
#[derive(Debug, Deserialize, ToSchema)]
pub struct UserTokenRequest {
    /// 用户ID
    pub user_id: String,
    /// 用户等级
    pub tier: UserTier,
    /// 权限列表
    pub permissions: Vec<String>,
    /// 可选的钱包地址
    pub wallet_address: Option<String>,
    /// 令牌有效期（小时），默认24小时
    pub expires_in_hours: Option<u64>,
}

/// 开发令牌响应
#[derive(Debug, Serialize, ToSchema)]
pub struct DevTokenResponse {
    /// JWT访问令牌
    pub access_token: String,
    /// 令牌类型
    pub token_type: String,
    /// 过期时间(秒)
    pub expires_in: u64,
    /// 用户信息
    pub user: UserInfo,
    /// 使用说明
    pub usage: String,
}

/// 令牌信息响应
#[derive(Debug, Serialize, ToSchema)]
pub struct TokenInfoResponse {
    /// 用户ID
    pub user_id: String,
    /// 钱包地址
    pub wallet_address: Option<String>,
    /// 用户等级
    pub tier: UserTier,
    /// 权限列表
    pub permissions: Vec<String>,
    /// 过期时间
    pub expires_at: u64,
    /// 签发时间
    pub issued_at: u64,
    /// 是否为管理员
    pub is_admin: bool,
}

/// 生成管理员令牌
///
/// 为开发和测试环境生成具有完整管理员权限的JWT令牌
///
/// **注意：此接口仅在开发环境中可用，生产环境将被禁用**
#[utoipa::path(
    post,
    path = "/api/v1/dev/admin-token",
    tag = "development",
    request_body = AdminTokenRequest,
    responses(
        (status = 200, description = "成功生成管理员令牌", body = DevTokenResponse),
        (status = 403, description = "生产环境禁止使用"),
        (status = 500, description = "令牌生成失败")
    )
)]
pub async fn generate_admin_token(
    Extension(jwt_manager): Extension<Arc<JwtManager>>,
    Json(request): Json<AdminTokenRequest>,
) -> Result<Json<DevTokenResponse>, StatusCode> {
    // 检查是否为开发环境
    if !is_development_environment() {
        warn!("🚫 Admin token generation blocked in production environment");
        return Err(StatusCode::FORBIDDEN);
    }

    let user_id = request.user_id.unwrap_or_else(|| "dev_admin".to_string());
    let expires_in_hours = request.expires_in_hours.unwrap_or(24);

    // 创建管理员权限集合
    let admin_permissions = vec![
        Permission::ReadUser.as_str().to_string(),
        Permission::ReadPool.as_str().to_string(),
        Permission::ReadPosition.as_str().to_string(),
        Permission::ReadReward.as_str().to_string(),
        Permission::CreateUser.as_str().to_string(),
        Permission::CreatePool.as_str().to_string(),
        Permission::CreatePosition.as_str().to_string(),
        Permission::ManageReward.as_str().to_string(),
        Permission::AdminConfig.as_str().to_string(),
        Permission::SystemMonitor.as_str().to_string(),
        Permission::UserManagement.as_str().to_string(),
    ];

    // 生成JWT令牌
    let token = jwt_manager
        .generate_token(
            &user_id,
            request.wallet_address.as_deref(),
            admin_permissions.clone(),
            UserTier::Admin,
        )
        .map_err(|e| {
            tracing::error!("Failed to generate admin token: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let user_info = UserInfo {
        user_id: user_id.clone(),
        wallet_address: request.wallet_address.clone(),
        tier: UserTier::Admin,
        permissions: admin_permissions,
    };

    let response = DevTokenResponse {
        access_token: token,
        token_type: "Bearer".to_string(),
        expires_in: expires_in_hours * 3600,
        user: user_info,
        usage: format!(
            "在请求头中添加: Authorization: Bearer YOUR_TOKEN\n示例: curl -H \"Authorization: Bearer YOUR_TOKEN\" http://localhost:8765/api/v1/admin/permissions/global/config"
        ),
    };

    info!("🔓 Generated admin token for user: {} (development only)", user_id);

    Ok(Json(response))
}

/// 生成指定权限的用户令牌
///
/// 为开发和测试环境生成具有指定权限的JWT令牌
///
/// **注意：此接口仅在开发环境中可用，生产环境将被禁用**
#[utoipa::path(
    post,
    path = "/api/v1/dev/user-token",
    tag = "development",
    request_body = UserTokenRequest,
    responses(
        (status = 200, description = "成功生成用户令牌", body = DevTokenResponse),
        (status = 400, description = "请求参数错误"),
        (status = 403, description = "生产环境禁止使用"),
        (status = 500, description = "令牌生成失败")
    )
)]
pub async fn generate_user_token(
    Extension(jwt_manager): Extension<Arc<JwtManager>>,
    Json(request): Json<UserTokenRequest>,
) -> Result<Json<DevTokenResponse>, StatusCode> {
    // 检查是否为开发环境
    if !is_development_environment() {
        warn!("🚫 User token generation blocked in production environment");
        return Err(StatusCode::FORBIDDEN);
    }

    let expires_in_hours = request.expires_in_hours.unwrap_or(24);

    // 验证权限格式
    let mut valid_permissions = Vec::new();
    for perm_str in &request.permissions {
        if Permission::from_str(perm_str).is_some() {
            valid_permissions.push(perm_str.clone());
        } else {
            warn!("Invalid permission ignored: {}", perm_str);
        }
    }

    // 生成JWT令牌
    let token = jwt_manager
        .generate_token(
            &request.user_id,
            request.wallet_address.as_deref(),
            valid_permissions.clone(),
            request.tier.clone(),
        )
        .map_err(|e| {
            tracing::error!("Failed to generate user token: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let user_info = UserInfo {
        user_id: request.user_id.clone(),
        wallet_address: request.wallet_address.clone(),
        tier: request.tier.clone(),
        permissions: valid_permissions,
    };

    let response = DevTokenResponse {
        access_token: token,
        token_type: "Bearer".to_string(),
        expires_in: expires_in_hours * 3600,
        user: user_info,
        usage: format!("在请求头中添加: Authorization: Bearer YOUR_TOKEN\n示例: curl -H \"Authorization: Bearer YOUR_TOKEN\" http://localhost:8000/api/v1/solana/main/version"),
    };

    info!(
        "🔓 Generated user token for: {} with tier: {:?} (development only)",
        request.user_id, request.tier
    );

    Ok(Json(response))
}

/// 获取令牌信息
///
/// 解析并显示JWT令牌中的用户信息
#[utoipa::path(
    get,
    path = "/api/v1/dev/token-info",
    tag = "development",
    security(
        ("Bearer" = [])
    ),
    responses(
        (status = 200, description = "成功获取令牌信息", body = TokenInfoResponse),
        (status = 401, description = "无效的令牌"),
        (status = 403, description = "生产环境禁止使用")
    )
)]
pub async fn get_token_info(
    Extension(jwt_manager): Extension<Arc<JwtManager>>,
    request: axum::extract::Request,
) -> Result<Json<TokenInfoResponse>, StatusCode> {
    // 检查是否为开发环境
    if !is_development_environment() {
        warn!("🚫 Token info blocked in production environment");
        return Err(StatusCode::FORBIDDEN);
    }

    // 从Authorization头部提取令牌
    let token = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|header| header.strip_prefix("Bearer "))
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // 验证令牌并提取信息
    let claims = jwt_manager.verify_token(token).map_err(|e| {
        warn!("Invalid token: {}", e);
        StatusCode::UNAUTHORIZED
    })?;

    let response = TokenInfoResponse {
        user_id: claims.sub,
        wallet_address: claims.wallet,
        tier: claims.tier.clone(),
        permissions: claims.permissions,
        expires_at: claims.exp,
        issued_at: claims.iat,
        is_admin: claims.tier == UserTier::Admin,
    };

    Ok(Json(response))
}

/// 检查是否为开发环境
fn is_development_environment() -> bool {
    // 检查环境变量
    match std::env::var("CARGO_ENV") {
        Ok(env) => env.to_lowercase() == "development",
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::AuthConfig;

    #[tokio::test]
    async fn test_is_development_environment() {
        // 设置测试环境变量
        std::env::set_var("CARGO_ENV", "development");
        assert!(is_development_environment());

        std::env::set_var("CARGO_ENV", "production");
        assert!(!is_development_environment());

        std::env::remove_var("CARGO_ENV");
        assert!(!is_development_environment());
    }

    #[tokio::test]
    async fn test_admin_token_generation() {
        std::env::set_var("CARGO_ENV", "development");

        let config = AuthConfig::default();
        let jwt_manager = Arc::new(JwtManager::new(config));

        let _request = AdminTokenRequest {
            user_id: Some("test_admin".to_string()),
            wallet_address: None,
            expires_in_hours: Some(1),
        };

        // 这里我们无法直接测试HTTP处理函数，但可以测试JWT生成逻辑
        let token = jwt_manager.generate_token("test_admin", None, vec!["admin:config".to_string()], UserTier::Admin);

        assert!(token.is_ok());

        // 验证生成的令牌
        let claims = jwt_manager.verify_token(&token.unwrap());
        assert!(claims.is_ok());
        let claims = claims.unwrap();
        assert_eq!(claims.sub, "test_admin");
        assert_eq!(claims.tier, UserTier::Admin);
    }
}
