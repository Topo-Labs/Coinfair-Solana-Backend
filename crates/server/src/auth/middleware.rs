use crate::auth::{AuthUser, Claims, JwtManager, Permission, SolanaApiAction, TokenExtractor};
use crate::services::solana::auth::solana_permission_service::DynSolanaPermissionService;
use anyhow::Result as AnyhowResult;
use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use serde_json::json;
use std::sync::Arc;
use tracing::{info, warn};

/// 认证中间件状态
#[derive(Clone)]
pub struct AuthState {
    pub jwt_manager: Arc<JwtManager>,
    pub auth_config: Arc<crate::auth::models::AuthConfig>,
}

/// Solana 认证中间件状态（包含权限服务）
#[derive(Clone)]
pub struct SolanaAuthState {
    pub jwt_manager: Arc<JwtManager>,
    pub permission_service: DynSolanaPermissionService,
    pub auth_config: Arc<crate::auth::models::AuthConfig>,
}

impl AuthState {
    pub fn new(jwt_manager: JwtManager, auth_config: crate::auth::models::AuthConfig) -> Self {
        Self {
            jwt_manager: Arc::new(jwt_manager),
            auth_config: Arc::new(auth_config),
        }
    }
}

impl SolanaAuthState {
    pub fn new(
        jwt_manager: JwtManager,
        permission_service: DynSolanaPermissionService,
        auth_config: crate::auth::models::AuthConfig,
    ) -> Self {
        Self {
            jwt_manager: Arc::new(jwt_manager),
            permission_service,
            auth_config: Arc::new(auth_config),
        }
    }
}

/// JWT认证中间件
pub async fn jwt_auth_middleware(
    State(auth_state): State<AuthState>,
    mut request: Request,
    next: Next,
) -> AnyhowResult<Response, StatusCode> {
    // 检查认证开关
    if auth_state.auth_config.auth_disabled {
        tracing::info!("🔓 认证已禁用，创建匿名用户直接通过");

        // 创建匿名用户
        let anonymous_user = AuthUser {
            user_id: "anonymous".to_string(),
            wallet_address: None,
            tier: crate::auth::UserTier::Admin, // 给予管理员权限确保能访问所有资源
            permissions: std::collections::HashSet::new(),
        };

        // 将匿名用户信息添加到请求扩展中
        request.extensions_mut().insert(anonymous_user);
        return Ok(next.run(request).await);
    }

    let headers = request.headers();

    // 尝试从Authorization头部提取Bearer令牌
    let token = TokenExtractor::extract_bearer_token(headers.get("authorization").and_then(|v| v.to_str().ok()));

    // 如果没有Bearer令牌，尝试从X-API-Key头部提取API密钥
    let api_key_token = if token.is_none() {
        TokenExtractor::extract_api_key(headers.get("x-api-key").and_then(|v| v.to_str().ok()))
    } else {
        None
    };

    let final_token = token.or(api_key_token);

    match final_token {
        Some(token_str) => {
            match auth_state.jwt_manager.verify_token(&token_str) {
                Ok(claims) => {
                    let auth_user = create_auth_user_from_claims(claims);

                    // 将认证用户信息添加到请求扩展中
                    request.extensions_mut().insert(auth_user);
                    Ok(next.run(request).await)
                }
                Err(e) => {
                    tracing::warn!("Token verification failed: {}", e);
                    Err(StatusCode::UNAUTHORIZED)
                }
            }
        }
        None => {
            tracing::warn!("No authentication token provided");
            Err(StatusCode::UNAUTHORIZED)
        }
    }
}

/// 可选认证中间件（允许匿名访问但提取用户信息）
pub async fn optional_auth_middleware(
    State(auth_state): State<AuthState>,
    mut request: Request,
    next: Next,
) -> AnyhowResult<Response, StatusCode> {
    // 检查认证开关
    if auth_state.auth_config.auth_disabled {
        tracing::info!("🔓 认证已禁用，创建匿名用户直接通过");

        // 创建匿名用户
        let anonymous_user = AuthUser {
            user_id: "anonymous".to_string(),
            wallet_address: None,
            tier: crate::auth::UserTier::Admin, // 给予管理员权限确保能访问所有资源
            permissions: std::collections::HashSet::new(),
        };

        // 将匿名用户信息添加到请求扩展中
        request.extensions_mut().insert(anonymous_user);
        return Ok(next.run(request).await);
    }

    let headers = request.headers();

    let token = TokenExtractor::extract_bearer_token(headers.get("authorization").and_then(|v| v.to_str().ok()));

    let api_key_token = if token.is_none() {
        TokenExtractor::extract_api_key(headers.get("x-api-key").and_then(|v| v.to_str().ok()))
    } else {
        None
    };

    let final_token = token.or(api_key_token);

    if let Some(token_str) = final_token {
        if let Ok(claims) = auth_state.jwt_manager.verify_token(&token_str) {
            let auth_user = create_auth_user_from_claims(claims);
            request.extensions_mut().insert(auth_user);
        }
    }

    Ok(next.run(request).await)
}

/// 权限检查中间件
pub fn require_permission(
    required_permission: Permission,
) -> impl Fn(Request, Next) -> futures::future::BoxFuture<'static, AnyhowResult<Response, StatusCode>> + Clone {
    move |request: Request, next: Next| {
        let required_perm = required_permission.clone();
        Box::pin(async move {
            match request.extensions().get::<AuthUser>() {
                Some(auth_user) => {
                    if auth_user.has_permission(&required_perm) || auth_user.is_admin() {
                        Ok(next.run(request).await)
                    } else {
                        tracing::warn!(
                            "User {} lacks required permission: {:?}",
                            auth_user.user_id,
                            required_perm
                        );
                        Err(StatusCode::FORBIDDEN)
                    }
                }
                None => {
                    tracing::warn!("No authenticated user found for permission check");
                    Err(StatusCode::UNAUTHORIZED)
                }
            }
        })
    }
}

/// 权限检查中间件（需要任一权限）
pub fn require_any_permission(
    required_permissions: Vec<Permission>,
) -> impl Fn(Request, Next) -> futures::future::BoxFuture<'static, AnyhowResult<Response, StatusCode>> + Clone {
    move |request: Request, next: Next| {
        let required_perms = required_permissions.clone();
        Box::pin(async move {
            match request.extensions().get::<AuthUser>() {
                Some(auth_user) => {
                    if auth_user.has_any_permission(&required_perms) || auth_user.is_admin() {
                        Ok(next.run(request).await)
                    } else {
                        tracing::warn!(
                            "User {} lacks any of required permissions: {:?}",
                            auth_user.user_id,
                            required_perms
                        );
                        Err(StatusCode::FORBIDDEN)
                    }
                }
                None => {
                    tracing::warn!("No authenticated user found for permission check");
                    Err(StatusCode::UNAUTHORIZED)
                }
            }
        })
    }
}

/// 管理员权限检查中间件
pub async fn require_admin(request: Request, next: Next) -> AnyhowResult<Response, StatusCode> {
    match request.extensions().get::<AuthUser>() {
        Some(auth_user) => {
            if auth_user.is_admin() {
                Ok(next.run(request).await)
            } else {
                tracing::warn!("Non-admin user {} attempted admin operation", auth_user.user_id);
                Err(StatusCode::FORBIDDEN)
            }
        }
        None => {
            tracing::warn!("Unauthenticated request attempted admin operation");
            Err(StatusCode::UNAUTHORIZED)
        }
    }
}

/// 错误响应构造函数
pub fn create_auth_error_response(status: StatusCode, message: &str) -> Response {
    let error_json = json!({
        "error": {
            "code": status.as_u16(),
            "message": message,
            "timestamp": chrono::Utc::now().timestamp()
        }
    });

    Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .body(error_json.to_string().into())
        .unwrap_or_else(|_| Response::new(axum::body::Body::empty()))
}

/// 从JWT Claims创建AuthUser
fn create_auth_user_from_claims(claims: Claims) -> AuthUser {
    use std::collections::HashSet;
    let permissions: HashSet<Permission> = claims
        .permissions
        .iter()
        .filter_map(|p| Permission::from_str(p))
        .collect();

    AuthUser {
        user_id: claims.sub,
        wallet_address: claims.wallet,
        tier: claims.tier,
        permissions,
    }
}

/// 认证用户信息提取器
pub struct AuthUserExtractor;

impl AuthUserExtractor {
    /// 从请求中提取认证用户信息
    pub fn extract_auth_user(request: &Request) -> Option<&AuthUser> {
        request.extensions().get::<AuthUser>()
    }

    /// 从请求中提取用户ID
    pub fn extract_user_id(request: &Request) -> Option<&String> {
        request.extensions().get::<AuthUser>().map(|user| &user.user_id)
    }

    /// 从请求中提取钱包地址
    pub fn extract_wallet_address(request: &Request) -> Option<&String> {
        request
            .extensions()
            .get::<AuthUser>()
            .and_then(|user| user.wallet_address.as_ref())
    }
}

/// 中间件构建器
#[derive(Clone)]
pub struct MiddlewareBuilder {
    auth_state: AuthState,
}

impl MiddlewareBuilder {
    pub fn new(jwt_manager: JwtManager, auth_config: crate::auth::models::AuthConfig) -> Self {
        Self {
            auth_state: AuthState::new(jwt_manager, auth_config),
        }
    }

    /// 构建JWT认证中间件
    pub fn jwt_auth(
        &self,
    ) -> impl Fn(Request, Next) -> futures::future::BoxFuture<'static, AnyhowResult<Response, StatusCode>> + Clone {
        let auth_state = self.auth_state.clone();
        move |request: Request, next: Next| {
            let auth_state = auth_state.clone();
            Box::pin(async move { jwt_auth_middleware(State(auth_state), request, next).await })
        }
    }

    /// 构建可选认证中间件
    pub fn optional_auth(
        &self,
    ) -> impl Fn(Request, Next) -> futures::future::BoxFuture<'static, AnyhowResult<Response, StatusCode>> + Clone {
        let auth_state = self.auth_state.clone();
        move |request: Request, next: Next| {
            let auth_state = auth_state.clone();
            Box::pin(async move { optional_auth_middleware(State(auth_state), request, next).await })
        }
    }
}

/// Solana API 权限检查中间件
pub async fn solana_permission_middleware(
    State(solana_auth_state): State<SolanaAuthState>,
    mut request: Request,
    next: Next,
) -> AnyhowResult<Response, StatusCode> {
    // 检查认证开关
    if solana_auth_state.auth_config.auth_disabled {
        tracing::info!("🔓 Solana认证已禁用，创建匿名用户直接通过");

        // 创建匿名用户
        let anonymous_user = AuthUser {
            user_id: "anonymous".to_string(),
            wallet_address: None,
            tier: crate::auth::UserTier::Admin, // 给予管理员权限确保能访问所有资源
            permissions: std::collections::HashSet::new(),
        };

        // 将匿名用户信息添加到请求扩展中
        request.extensions_mut().insert(anonymous_user);
        return Ok(next.run(request).await);
    } else {
        tracing::info!("🔒 Solana认证已启用，检查权限");
    }

    let headers = request.headers();

    // 尝试从Authorization头部提取Bearer令牌
    let token = TokenExtractor::extract_bearer_token(headers.get("authorization").and_then(|v| v.to_str().ok()));

    // 如果没有Bearer令牌，尝试从X-API-Key头部提取API密钥
    let api_key_token = if token.is_none() {
        TokenExtractor::extract_api_key(headers.get("x-api-key").and_then(|v| v.to_str().ok()))
    } else {
        None
    };

    let final_token = token.or(api_key_token);

    match final_token {
        Some(token_str) => {
            match solana_auth_state.jwt_manager.verify_token(&token_str) {
                Ok(claims) => {
                    let auth_user = create_auth_user_from_claims(claims);

                    // 🔧 修复：使用更智能的路径重建方法
                    let endpoint = {
                        let current_path = request.uri().path();

                        // 尝试从请求头中获取完整路径
                        if let Some(original_uri) = request.headers().get("x-original-uri") {
                            original_uri.to_str().unwrap_or(current_path).to_string()
                        } else if current_path.starts_with("/api/v1") {
                            // 已经是完整路径
                            current_path.to_string()
                        } else {
                            // 这是嵌套路由片段，需要从上下文重建路径
                            // 检查Axum的MatchedPath扩展
                            if let Some(matched_path) = request.extensions().get::<axum::extract::MatchedPath>() {
                                matched_path.as_str().to_string()
                            } else {
                                // 作为备用方案，我们需要手动重建路径
                                // 目前直接使用原始路径作为fallback
                                tracing::warn!("⚠️ 无法获取完整路径，使用原始路径: {}", current_path);
                                current_path.to_string()
                            }
                        }
                    };

                    tracing::debug!("🔍 路径重建: 原始路径={}, 重建路径={}", request.uri().path(), endpoint);
                    let method = request.method().as_str();

                    // 根据HTTP方法判断操作类型
                    let action = match method {
                        "GET" | "HEAD" | "OPTIONS" => SolanaApiAction::Read,
                        "POST" | "PUT" | "PATCH" | "DELETE" => SolanaApiAction::Write,
                        _ => SolanaApiAction::Read, // 默认为读取操作
                    };

                    // 检查权限
                    tracing::info!(
                        "🔍 开始Solana API权限检查: 用户={} 端点={} 操作={:?}",
                        auth_user.user_id,
                        endpoint,
                        action
                    );
                    match solana_auth_state
                        .permission_service
                        .check_api_permission(&endpoint, &action, &auth_user)
                        .await
                    {
                        Ok(_) => {
                            info!(
                                "✅ Solana API权限检查通过: 用户={} 端点={} 操作={:?}",
                                auth_user.user_id, endpoint, action
                            );
                            // 将认证用户信息添加到请求扩展中
                            request.extensions_mut().insert(auth_user);
                            Ok(next.run(request).await)
                        }
                        Err(permission_error) => {
                            warn!(
                                "❌ Solana API权限检查失败: 用户={} 端点={} 操作={:?} 原因={}",
                                auth_user.user_id, endpoint, action, permission_error
                            );
                            Err(StatusCode::FORBIDDEN)
                        }
                    }
                }
                Err(e) => {
                    warn!("Token verification failed: {}", e);
                    Err(StatusCode::UNAUTHORIZED)
                }
            }
        }
        None => {
            warn!("No authentication token provided for Solana API");
            Err(StatusCode::UNAUTHORIZED)
        }
    }
}

/// Solana API 可选权限检查中间件（允许匿名访问但检查权限）
pub async fn solana_optional_permission_middleware(
    State(solana_auth_state): State<SolanaAuthState>,
    mut request: Request,
    next: Next,
) -> AnyhowResult<Response, StatusCode> {
    // 检查认证开关
    if solana_auth_state.auth_config.auth_disabled {
        tracing::info!("🔓 Solana认证已禁用，创建匿名用户直接通过");

        // 创建匿名用户
        let anonymous_user = AuthUser {
            user_id: "anonymous".to_string(),
            wallet_address: None,
            tier: crate::auth::UserTier::Admin, // 给予管理员权限确保能访问所有资源
            permissions: std::collections::HashSet::new(),
        };

        // 将匿名用户信息添加到请求扩展中
        request.extensions_mut().insert(anonymous_user);
        return Ok(next.run(request).await);
    } else {
        tracing::info!("🔒 Solana认证已启用，检查权限");
    }

    let headers = request.headers();

    // 🔧 修复：使用更智能的路径重建方法
    let endpoint = {
        let current_path = request.uri().path();

        // 尝试从请求头中获取完整路径
        if let Some(original_uri) = request.headers().get("x-original-uri") {
            original_uri.to_str().unwrap_or(current_path).to_string()
        } else if current_path.starts_with("/api/v1") {
            // 已经是完整路径
            current_path.to_string()
        } else {
            // 这是嵌套路由片段，需要从上下文重建路径
            // 检查Axum的MatchedPath扩展
            if let Some(matched_path) = request.extensions().get::<axum::extract::MatchedPath>() {
                matched_path.as_str().to_string()
            } else {
                // 作为备用方案，我们需要手动重建路径
                // 目前直接使用原始路径作为fallback
                tracing::warn!("⚠️ 无法获取完整路径，使用原始路径: {}", current_path);
                current_path.to_string()
            }
        }
    };

    tracing::debug!(
        "🔍 可选权限检查路径重建: 原始路径={}, 重建路径={}",
        request.uri().path(),
        endpoint
    );

    let method = request.method().as_str();

    // 根据HTTP方法判断操作类型
    let action = match method {
        "GET" | "HEAD" | "OPTIONS" => SolanaApiAction::Read,
        "POST" | "PUT" | "PATCH" | "DELETE" => SolanaApiAction::Write,
        _ => SolanaApiAction::Read,
    };

    let token = TokenExtractor::extract_bearer_token(headers.get("authorization").and_then(|v| v.to_str().ok()));

    let api_key_token = if token.is_none() {
        TokenExtractor::extract_api_key(headers.get("x-api-key").and_then(|v| v.to_str().ok()))
    } else {
        None
    };

    let final_token = token.or(api_key_token);

    if let Some(token_str) = final_token {
        if let Ok(claims) = solana_auth_state.jwt_manager.verify_token(&token_str) {
            let auth_user = create_auth_user_from_claims(claims);

            // 检查权限
            tracing::info!(
                "🔍 开始Solana API可选权限检查: 用户={} 端点={} 操作={:?}",
                auth_user.user_id,
                endpoint,
                action
            );
            match solana_auth_state
                .permission_service
                .check_api_permission(&endpoint, &action, &auth_user)
                .await
            {
                Ok(_) => {
                    info!(
                        "✅ Solana API可选权限检查通过: 用户={} 端点={} 操作={:?}",
                        auth_user.user_id, endpoint, action
                    );
                    request.extensions_mut().insert(auth_user);
                }
                Err(permission_error) => {
                    warn!(
                        "❌ Solana API可选权限检查失败: 用户={} 端点={} 操作={:?} 原因={}",
                        auth_user.user_id, endpoint, action, permission_error
                    );
                    // 对于可选中间件，权限失败时不直接拒绝，而是不添加用户信息
                }
            }
        }
    } else {
        // 没有认证信息，检查是否允许匿名访问
        use crate::auth::{AuthUser, UserTier};
        use std::collections::HashSet;

        let anonymous_user = AuthUser {
            user_id: "anonymous".to_string(),
            wallet_address: None,
            tier: UserTier::Basic,
            permissions: HashSet::new(),
        };

        match solana_auth_state
            .permission_service
            .check_api_permission(&endpoint, &action, &anonymous_user)
            .await
        {
            Ok(_) => {
                info!("✅ Solana API匿名访问允许: 端点={} 操作={:?}", endpoint, action);
                // 不添加用户信息到扩展中，表示匿名访问
            }
            Err(permission_error) => {
                warn!(
                    "❌ Solana API匿名访问被拒绝: 端点={} 操作={:?} 原因={}",
                    endpoint, action, permission_error
                );
                return Err(StatusCode::UNAUTHORIZED);
            }
        }
    }

    Ok(next.run(request).await)
}

/// Solana 特定权限检查中间件（需要特定权限）
pub fn solana_require_permission(
    required_permission: Permission,
) -> impl Fn(Request, Next) -> futures::future::BoxFuture<'static, AnyhowResult<Response, StatusCode>> + Clone {
    move |request: Request, next: Next| {
        let required_perm = required_permission.clone();
        Box::pin(async move {
            match request.extensions().get::<AuthUser>() {
                Some(auth_user) => {
                    if auth_user.has_permission(&required_perm) || auth_user.is_admin() {
                        info!(
                            "✅ Solana特定权限检查通过: 用户={} 权限={:?}",
                            auth_user.user_id, required_perm
                        );
                        Ok(next.run(request).await)
                    } else {
                        warn!(
                            "❌ Solana特定权限检查失败: 用户={} 缺少权限={:?}",
                            auth_user.user_id, required_perm
                        );
                        Err(StatusCode::FORBIDDEN)
                    }
                }
                None => {
                    warn!("No authenticated user found for Solana specific permission check");
                    Err(StatusCode::UNAUTHORIZED)
                }
            }
        })
    }
}

/// Solana 中间件构建器
#[derive(Clone)]
pub struct SolanaMiddlewareBuilder {
    solana_auth_state: SolanaAuthState,
}

impl SolanaMiddlewareBuilder {
    pub fn new(
        jwt_manager: JwtManager,
        permission_service: DynSolanaPermissionService,
        auth_config: crate::auth::models::AuthConfig,
    ) -> Self {
        Self {
            solana_auth_state: SolanaAuthState::new(jwt_manager, permission_service, auth_config),
        }
    }

    /// 构建Solana权限检查中间件
    pub fn solana_auth(
        &self,
    ) -> impl Fn(Request, Next) -> futures::future::BoxFuture<'static, AnyhowResult<Response, StatusCode>> + Clone {
        let auth_state = self.solana_auth_state.clone();
        move |request: Request, next: Next| {
            let auth_state = auth_state.clone();
            Box::pin(async move { solana_permission_middleware(State(auth_state), request, next).await })
        }
    }

    /// 构建Solana可选权限检查中间件
    pub fn solana_optional_auth(
        &self,
    ) -> impl Fn(Request, Next) -> futures::future::BoxFuture<'static, AnyhowResult<Response, StatusCode>> + Clone {
        let auth_state = self.solana_auth_state.clone();
        move |request: Request, next: Next| {
            let auth_state = auth_state.clone();
            Box::pin(async move { solana_optional_permission_middleware(State(auth_state), request, next).await })
        }
    }

    /// 获取权限服务引用（用于调试和管理）
    pub fn get_permission_service(&self) -> &DynSolanaPermissionService {
        &self.solana_auth_state.permission_service
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::{AuthConfig, JwtManager, UserTier};
    #[allow(dead_code)]
    fn create_test_jwt_manager() -> JwtManager {
        let config = AuthConfig {
            jwt_secret: "test_secret_key_for_testing_only".to_string(),
            jwt_expires_in_hours: 24,
            solana_auth_message_ttl: 300,
            redis_url: None,
            rate_limit_redis_prefix: "test:ratelimit".to_string(),
            auth_disabled: false,
        };
        JwtManager::new(config)
    }

    #[tokio::test]
    async fn test_token_extractor() {
        // 测试Bearer令牌提取
        let bearer_header = "Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...";
        let token = TokenExtractor::extract_bearer_token(Some(bearer_header));
        assert_eq!(token, Some("eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...".to_string()));

        // 测试API密钥提取
        let api_key = "ak_test_key_123456";
        let extracted_key = TokenExtractor::extract_api_key(Some(api_key));
        assert_eq!(extracted_key, Some("ak_test_key_123456".to_string()));
    }

    #[test]
    fn test_create_auth_user_from_claims() {
        let claims = Claims {
            sub: "test_user".to_string(),
            wallet: Some("test_wallet".to_string()),
            permissions: vec!["read:user".to_string(), "create:pool".to_string()],
            tier: UserTier::Premium,
            exp: 1234567890,
            iat: 1234567890,
            iss: "test".to_string(),
        };

        let auth_user = create_auth_user_from_claims(claims);
        assert_eq!(auth_user.user_id, "test_user");
        assert_eq!(auth_user.wallet_address, Some("test_wallet".to_string()));
        assert_eq!(auth_user.tier, UserTier::Premium);
        assert!(auth_user.has_permission(&Permission::ReadUser));
        assert!(auth_user.has_permission(&Permission::CreatePool));
    }

    #[test]
    fn test_user_tier_rate_limits() {
        assert_eq!(UserTier::Basic.rate_limit_multiplier(), 1);
        assert_eq!(UserTier::Premium.rate_limit_multiplier(), 5);
        assert_eq!(UserTier::VIP.rate_limit_multiplier(), 20);
        assert_eq!(UserTier::Admin.rate_limit_multiplier(), 100);
    }
}
