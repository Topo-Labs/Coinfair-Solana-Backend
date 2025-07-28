use super::services::Services;
use crate::{api, docs, middleware};
use axum::{
    error_handling::HandleErrorLayer,
    http::{Method, StatusCode},
    middleware as axum_middleware,
    response::IntoResponse,
    routing::get,
    BoxError, Extension, Json, Router,
};
use lazy_static::lazy_static;
use serde_json::json;
use std::time::Duration;
use tower::{buffer::BufferLayer, limit::RateLimitLayer, ServiceBuilder};
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

lazy_static! {
    static ref HTTP_TIMEOUT: u64 = 30;
}

pub struct AppRouter;

impl AppRouter {
    pub fn new(services: Services) -> Router {
        let cors = CorsLayer::new()
            .allow_origin(Any)
            .allow_methods([
                Method::GET,
                Method::POST,
                Method::DELETE,
                Method::PUT,
                Method::PATCH,
                Method::OPTIONS, // 添加OPTIONS方法支持
            ])
            .allow_headers(Any) // 允许所有头部
            .allow_credentials(false); // 明确设置credentials

        let router = Router::new()
            // API 路由
            .nest("/api/v1", api::app())
            // API 文档说明页面
            .route("/api-docs", get(api_docs_info))
            // 添加IP日志中间件
            .layer(axum_middleware::from_fn(middleware::simple_ip_logger))
            .layer(cors)
            .layer(
                ServiceBuilder::new()
                    .layer(Extension(services))
                    .layer(TraceLayer::new_for_http())
                    .layer(HandleErrorLayer::new(Self::handle_timeout_error))
                    .timeout(Duration::from_secs(*HTTP_TIMEOUT))
                    .layer(BufferLayer::new(1024))
                    .layer(RateLimitLayer::new(50, Duration::from_secs(1))), // 修改为每秒50个请求
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
                    "error":
                        format!(
                            "request took longer than the configured {} second timeout",
                            *HTTP_TIMEOUT
                        )
                })),
            )
        } else {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": format!("unhandled internal error: {}", err)
                })),
            )
        }
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
