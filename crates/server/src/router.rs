use super::services::Services;
use crate::{
    api,
    auth::{
        AuthConfig, JwtManager, MultiDimensionalRateLimit, PermissionManager, RateLimitService, SolanaAuthService,
        SolanaMiddlewareBuilder,
    },
};
use axum::{
    error_handling::HandleErrorLayer,
    extract::State,
    http::{Method, Request, StatusCode},
    middleware::{self as axum_middleware, Next},
    response::{IntoResponse, Response},
    routing::get,
    BoxError, Extension, Json, Router,
};
use lazy_static::lazy_static;
use serde_json::json;
use std::{sync::Arc, time::Duration};
use tower::{buffer::BufferLayer, ServiceBuilder};
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;
use crate::services::docs::docs;
use crate::services::middleware::middleware;

lazy_static! {
    static ref HTTP_TIMEOUT: u64 = 30;
}

pub struct AppRouter;

impl AppRouter {
    pub fn new(services: Services) -> Router {
        // 确保加载环境变量
        utils::config::EnvLoader::load_env_file().expect("Failed to load environment variables");

        // 初始化认证配置
        let auth_config = AuthConfig::default();
        let jwt_manager = JwtManager::new(auth_config.clone());
        let solana_auth_service = SolanaAuthService::new(jwt_manager.clone(), auth_config.clone());
        let permission_manager = PermissionManager::new();

        // 创建 Solana 权限中间件构建器
        let solana_middleware_builder = SolanaMiddlewareBuilder::new(
            jwt_manager.clone(),
            services.solana_permission.clone(),
            auth_config.clone(),
        );

        // 初始化速率限制服务
        let rate_limit_service = RateLimitService::new(
            auth_config.redis_url.clone(),
            auth_config.rate_limit_redis_prefix.clone(),
        )
        .expect("Failed to initialize rate limit service");

        let multi_rate_limiter = MultiDimensionalRateLimit::new(
            rate_limit_service,
            None, // 使用默认用户等级配置
            None, // 使用默认端点配置
        );

        // 克隆用于中间件
        let rate_limiter_for_middleware = Arc::new(MultiDimensionalRateLimit::new(
            RateLimitService::new(
                auth_config.redis_url.clone(),
                auth_config.rate_limit_redis_prefix.clone(),
            )
            .expect("Failed to initialize rate limit service for middleware"),
            None,
            None,
        ));

        let cors = CorsLayer::new()
            .allow_origin(Any)
            .allow_methods([
                Method::GET,
                Method::POST,
                Method::DELETE,
                Method::PUT,
                Method::PATCH,
                Method::OPTIONS,
            ])
            .allow_headers([
                axum::http::header::CONTENT_TYPE,
                axum::http::header::AUTHORIZATION,
                axum::http::header::ACCEPT,
                axum::http::header::USER_AGENT,
                axum::http::HeaderName::from_static("x-api-key"),
            ]);

        let router = Router::new()
            // API 路由
            .nest("/api/v1", api::app())
            // API 文档说明页面
            .route("/api-docs", get(api_docs_info))
            // 添加速率限制中间件
            .layer(axum_middleware::from_fn_with_state(
                rate_limiter_for_middleware,
                Self::rate_limit_middleware,
            ))
            // 添加IP日志中间件
            .layer(axum_middleware::from_fn(middleware::simple_ip_logger))
            .layer(cors)
            .layer(
                ServiceBuilder::new()
                    .layer(Extension(services))
                    .layer(Extension(Arc::new(jwt_manager)))
                    .layer(Extension(Arc::new(solana_auth_service)))
                    .layer(Extension(Arc::new(permission_manager)))
                    .layer(Extension(Arc::new(multi_rate_limiter)))
                    .layer(Extension(Arc::new(solana_middleware_builder)))
                    .layer(TraceLayer::new_for_http())
                    .layer(HandleErrorLayer::new(Self::handle_timeout_error))
                    .timeout(Duration::from_secs(*HTTP_TIMEOUT))
                    .layer(BufferLayer::new(1024)),
            )
            // Swagger UI 路由 - 包含 OpenAPI JSON 端点
            .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", docs::ApiDoc::openapi()))
            .fallback(Self::handle_404);

        router
    }

    async fn handle_404() -> impl IntoResponse {
        (
            StatusCode::NOT_FOUND,
            axum::response::Json(serde_json::json!({
            "errors":{
            "message": vec!(String::from("The requested resource does not exist on this server!")),}
            })),
        )
    }

    async fn handle_timeout_error(err: BoxError) -> (StatusCode, Json<serde_json::Value>) {
        if err.is::<tower::timeout::error::Elapsed>() {
            (
                StatusCode::REQUEST_TIMEOUT,
                Json(json!({
                    "error": {
                        "code": "TIMEOUT",
                        "message": format!(
                            "Request took longer than the configured {} second timeout",
                            *HTTP_TIMEOUT
                        ),
                        "timestamp": chrono::Utc::now().timestamp()
                    }
                })),
            )
        } else {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": {
                        "code": "INTERNAL_ERROR",
                        "message": format!("Unhandled internal error: {}", err),
                        "timestamp": chrono::Utc::now().timestamp()
                    }
                })),
            )
        }
    }

    /// 速率限制中间件
    async fn rate_limit_middleware(
        State(rate_limiter): State<Arc<MultiDimensionalRateLimit>>,
        request: Request<axum::body::Body>,
        next: Next,
    ) -> Result<Response, StatusCode> {
        rate_limiter.middleware(request, next).await
    }
}

/// API 文档说明页面
async fn api_docs_info() -> impl IntoResponse {
    Json(json!({
        "message": "Coinfair Solana Backend API Documentation",
        "version": "1.0.0",
        "openapi_spec": "/api-docs/openapi.json",
        "swagger_ui": "/swagger-ui",
        "description": "访问 /swagger-ui 查看交互式 API 文档"
    }))
}
