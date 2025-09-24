use crate::auth::{
    AuthResponse, AuthUser, GenerateAuthMessageRequest, JwtManager, SolanaAuthService, SolanaLoginRequest, UserInfo,
};
use axum::{
    extract::{Extension, Request},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde_json::json;
use std::sync::Arc;
use utoipa::OpenApi;

/// 认证控制器
pub struct AuthController;

impl AuthController {
    pub fn app() -> Router {
        Router::new()
            .route("/auth/generate-message", post(generate_auth_message))
            .route("/auth/login", post(solana_login))
            .route("/auth/refresh", post(refresh_token))
            .route("/auth/profile", get(get_user_profile))
            .route("/auth/logout", post(logout))
    }
}

/// 生成认证消息
#[utoipa::path(
    post,
    path = "/api/v1/auth/generate-message",
    tag = "authentication",
    request_body = GenerateAuthMessageRequest,
    responses(
        (status = 200, description = "成功生成认证消息", body = AuthMessageResponse),
        (status = 400, description = "请求参数错误")
    )
)]
pub async fn generate_auth_message(
    Extension(auth_service): Extension<Arc<SolanaAuthService>>,
    Json(request): Json<GenerateAuthMessageRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let response = auth_service
        .generate_auth_message(&request.wallet_address)
        .map_err(|e| {
            tracing::error!("Failed to generate auth message: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(json!(response)))
}

/// Solana钱包登录
#[utoipa::path(
    post,
    path = "/api/v1/auth/login",
    tag = "authentication",
    request_body = SolanaLoginRequest,
    responses(
        (status = 200, description = "登录成功", body = AuthResponse),
        (status = 400, description = "请求参数错误"),
        (status = 401, description = "认证失败")
    )
)]
pub async fn solana_login(
    Extension(auth_service): Extension<Arc<SolanaAuthService>>,
    Json(request): Json<SolanaLoginRequest>,
) -> Result<Json<AuthResponse>, StatusCode> {
    let response = auth_service.authenticate_wallet(request).await.map_err(|e| {
        tracing::warn!("Authentication failed: {}", e);
        StatusCode::UNAUTHORIZED
    })?;

    Ok(Json(response))
}

/// 刷新访问令牌
#[utoipa::path(
    post,
    path = "/api/v1/auth/refresh",
    tag = "authentication",
    security(
        ("Bearer" = [])
    ),
    responses(
        (status = 200, description = "令牌刷新成功", body = AuthResponse),
        (status = 401, description = "无效的刷新令牌")
    )
)]
pub async fn refresh_token(
    Extension(jwt_manager): Extension<Arc<JwtManager>>,
    request: Request,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // 从Authorization头部提取当前令牌
    let auth_header = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|header| header.strip_prefix("Bearer "))
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // 刷新令牌
    let new_token = jwt_manager.refresh_token(auth_header).map_err(|e| {
        tracing::warn!("Token refresh failed: {}", e);
        StatusCode::UNAUTHORIZED
    })?;

    // 从新令牌中提取用户信息
    let claims = jwt_manager.verify_token(&new_token).map_err(|_| {
        tracing::error!("Failed to verify refreshed token");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let response = AuthResponse {
        access_token: new_token,
        token_type: "Bearer".to_string(),
        expires_in: 24 * 3600, // 24小时
        user: UserInfo {
            user_id: claims.sub,
            wallet_address: claims.wallet,
            tier: claims.tier,
            permissions: claims.permissions,
        },
    };

    Ok(Json(json!(response)))
}

/// 获取用户个人信息
#[utoipa::path(
    get,
    path = "/api/v1/auth/profile",
    tag = "authentication",
    security(
        ("Bearer" = [])
    ),
    responses(
        (status = 200, description = "成功获取用户信息", body = UserInfo),
        (status = 401, description = "未认证")
    )
)]
pub async fn get_user_profile(request: Request) -> Result<Json<UserInfo>, StatusCode> {
    let auth_user = request.extensions().get::<AuthUser>().ok_or(StatusCode::UNAUTHORIZED)?;

    let user_info = UserInfo {
        user_id: auth_user.user_id.clone(),
        wallet_address: auth_user.wallet_address.clone(),
        tier: auth_user.tier.clone(),
        permissions: auth_user.permissions.iter().map(|p| p.as_str().to_string()).collect(),
    };

    Ok(Json(user_info))
}

/// 用户登出
#[utoipa::path(
    post,
    path = "/api/v1/auth/logout",
    tag = "authentication",
    security(
        ("Bearer" = [])
    ),
    responses(
        (status = 200, description = "登出成功"),
        (status = 401, description = "未认证")
    )
)]
pub async fn logout(request: Request) -> Result<Json<serde_json::Value>, StatusCode> {
    let auth_user = request.extensions().get::<AuthUser>().ok_or(StatusCode::UNAUTHORIZED)?;

    tracing::info!("User {} logged out", auth_user.user_id);

    // 在实际实现中，可以将令牌加入黑名单
    // 这里只是简单返回成功响应
    Ok(Json(json!({
        "message": "Successfully logged out",
        "timestamp": chrono::Utc::now().timestamp()
    })))
}

/// 检查认证状态
#[utoipa::path(
    get,
    path = "/api/v1/auth/status",
    tag = "authentication",
    security(
        ("Bearer" = [])
    ),
    responses(
        (status = 200, description = "认证状态检查", body = serde_json::Value),
        (status = 401, description = "未认证")
    )
)]
pub async fn check_auth_status(
    Extension(jwt_manager): Extension<Arc<JwtManager>>,
    request: Request,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let auth_user = request.extensions().get::<AuthUser>().ok_or(StatusCode::UNAUTHORIZED)?;

    // 检查令牌是否即将过期
    let auth_header = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|header| header.strip_prefix("Bearer "))
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let is_expiring_soon = jwt_manager.is_token_expiring_soon(auth_header).unwrap_or(false);

    Ok(Json(json!({
        "authenticated": true,
        "user_id": auth_user.user_id,
        "tier": auth_user.tier,
        "permissions": auth_user.permissions.iter().map(|p| p.as_str()).collect::<Vec<_>>(),
        "token_expiring_soon": is_expiring_soon,
        "timestamp": chrono::Utc::now().timestamp()
    })))
}

/// 为管理员提供的用户管理端点
pub struct AdminAuthController;

impl AdminAuthController {
    pub fn app() -> Router {
        Router::new()
            .route("/admin/auth/users", get(list_authenticated_users))
            .route("/admin/auth/permissions/:user_id", get(get_user_permissions))
            .route("/admin/auth/permissions/:user_id", post(update_user_permissions))
    }
}

/// 列出已认证用户（管理员功能）
#[utoipa::path(
    get,
    path = "/api/v1/admin/auth/users",
    tag = "admin",
    security(
        ("Bearer" = [])
    ),
    responses(
        (status = 200, description = "成功获取用户列表"),
        (status = 403, description = "权限不足")
    )
)]
pub async fn list_authenticated_users(request: Request) -> Result<Json<serde_json::Value>, StatusCode> {
    let auth_user = request.extensions().get::<AuthUser>().ok_or(StatusCode::UNAUTHORIZED)?;

    if !auth_user.is_admin() {
        return Err(StatusCode::FORBIDDEN);
    }

    // 这里应该从数据库或缓存中获取活跃用户列表
    // 暂时返回模拟数据
    Ok(Json(json!({
        "users": [],
        "total": 0,
        "timestamp": chrono::Utc::now().timestamp()
    })))
}

/// 获取用户权限（管理员功能）
#[utoipa::path(
    get,
    path = "/api/v1/admin/auth/permissions/{user_id}",
    tag = "admin",
    params(
        ("user_id" = String, Path, description = "用户ID")
    ),
    security(
        ("Bearer" = [])
    ),
    responses(
        (status = 200, description = "成功获取用户权限"),
        (status = 403, description = "权限不足"),
        (status = 404, description = "用户不存在")
    )
)]
pub async fn get_user_permissions(request: Request) -> Result<Json<serde_json::Value>, StatusCode> {
    let auth_user = request.extensions().get::<AuthUser>().ok_or(StatusCode::UNAUTHORIZED)?;

    if !auth_user.is_admin() {
        return Err(StatusCode::FORBIDDEN);
    }

    // 这里应该从数据库中获取指定用户的权限
    // 暂时返回模拟数据
    Ok(Json(json!({
        "user_id": "example_user",
        "permissions": ["read:user", "read:pool"],
        "tier": "Basic",
        "timestamp": chrono::Utc::now().timestamp()
    })))
}

/// 更新用户权限（管理员功能）
#[utoipa::path(
    post,
    path = "/api/v1/admin/auth/permissions/{user_id}",
    tag = "admin",
    params(
        ("user_id" = String, Path, description = "用户ID")
    ),
    security(
        ("Bearer" = [])
    ),
    responses(
        (status = 200, description = "成功更新用户权限"),
        (status = 403, description = "权限不足"),
        (status = 404, description = "用户不存在")
    )
)]
pub async fn update_user_permissions(request: Request) -> Result<Json<serde_json::Value>, StatusCode> {
    let auth_user = request.extensions().get::<AuthUser>().ok_or(StatusCode::UNAUTHORIZED)?;

    if !auth_user.is_admin() {
        return Err(StatusCode::FORBIDDEN);
    }

    // 这里应该实现更新用户权限的逻辑
    // 暂时返回成功响应
    Ok(Json(json!({
        "message": "User permissions updated successfully",
        "timestamp": chrono::Utc::now().timestamp()
    })))
}

/// 认证相关的OpenAPI文档
#[derive(OpenApi)]
#[openapi(
    paths(
        generate_auth_message,
        solana_login,
        refresh_token,
        get_user_profile,
        logout,
        check_auth_status,
        list_authenticated_users,
        get_user_permissions,
        update_user_permissions,
    ),
    components(
        schemas(
            GenerateAuthMessageRequest,
            SolanaLoginRequest,
            AuthResponse,
            UserInfo,
        )
    ),
    tags(
        (name = "authentication", description = "用户认证相关API"),
        (name = "admin", description = "管理员功能API")
    )
)]
pub struct AuthApiDoc;
