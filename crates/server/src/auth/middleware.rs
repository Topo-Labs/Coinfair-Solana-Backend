use crate::auth::{AuthUser, Claims, JwtManager, Permission, TokenExtractor};
use anyhow::Result as AnyhowResult;
use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use serde_json::json;
use std::sync::Arc;

/// 认证中间件状态
#[derive(Clone)]
pub struct AuthState {
    pub jwt_manager: Arc<JwtManager>,
}

impl AuthState {
    pub fn new(jwt_manager: JwtManager) -> Self {
        Self {
            jwt_manager: Arc::new(jwt_manager),
        }
    }
}

/// JWT认证中间件
pub async fn jwt_auth_middleware(State(auth_state): State<AuthState>, mut request: Request, next: Next) -> AnyhowResult<Response, StatusCode> {
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
pub async fn optional_auth_middleware(State(auth_state): State<AuthState>, mut request: Request, next: Next) -> AnyhowResult<Response, StatusCode> {
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
pub fn require_permission(required_permission: Permission) -> impl Fn(Request, Next) -> futures::future::BoxFuture<'static, AnyhowResult<Response, StatusCode>> + Clone {
    move |request: Request, next: Next| {
        let required_perm = required_permission.clone();
        Box::pin(async move {
            match request.extensions().get::<AuthUser>() {
                Some(auth_user) => {
                    if auth_user.has_permission(&required_perm) || auth_user.is_admin() {
                        Ok(next.run(request).await)
                    } else {
                        tracing::warn!("User {} lacks required permission: {:?}", auth_user.user_id, required_perm);
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
pub fn require_any_permission(required_permissions: Vec<Permission>) -> impl Fn(Request, Next) -> futures::future::BoxFuture<'static, AnyhowResult<Response, StatusCode>> + Clone {
    move |request: Request, next: Next| {
        let required_perms = required_permissions.clone();
        Box::pin(async move {
            match request.extensions().get::<AuthUser>() {
                Some(auth_user) => {
                    if auth_user.has_any_permission(&required_perms) || auth_user.is_admin() {
                        Ok(next.run(request).await)
                    } else {
                        tracing::warn!("User {} lacks any of required permissions: {:?}", auth_user.user_id, required_perms);
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
    let permissions: HashSet<Permission> = claims.permissions.iter().filter_map(|p| Permission::from_str(p)).collect();

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
        request.extensions().get::<AuthUser>().and_then(|user| user.wallet_address.as_ref())
    }
}

/// 中间件构建器
#[derive(Clone)]
pub struct MiddlewareBuilder {
    auth_state: AuthState,
}

impl MiddlewareBuilder {
    pub fn new(jwt_manager: JwtManager) -> Self {
        Self {
            auth_state: AuthState::new(jwt_manager),
        }
    }

    /// 构建JWT认证中间件
    pub fn jwt_auth(&self) -> impl Fn(Request, Next) -> futures::future::BoxFuture<'static, AnyhowResult<Response, StatusCode>> + Clone {
        let auth_state = self.auth_state.clone();
        move |request: Request, next: Next| {
            let auth_state = auth_state.clone();
            Box::pin(async move { jwt_auth_middleware(State(auth_state), request, next).await })
        }
    }

    /// 构建可选认证中间件
    pub fn optional_auth(&self) -> impl Fn(Request, Next) -> futures::future::BoxFuture<'static, AnyhowResult<Response, StatusCode>> + Clone {
        let auth_state = self.auth_state.clone();
        move |request: Request, next: Next| {
            let auth_state = auth_state.clone();
            Box::pin(async move { optional_auth_middleware(State(auth_state), request, next).await })
        }
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
