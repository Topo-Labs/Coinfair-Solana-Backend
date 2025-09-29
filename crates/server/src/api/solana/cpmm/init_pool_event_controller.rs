use crate::dtos::solana::common::{ApiResponse, ErrorResponse};
use crate::dtos::solana::cpmm::pool::init_pool_event::{
    CreateInitPoolEventRequest, InitPoolEventResponse, InitPoolEventsPageResponse, QueryInitPoolEventsRequest,
    UserPoolStats,
};
use crate::extractors::validation_extractor::ValidationExtractor;
use crate::services::Services;
use axum::extract::{Extension, Path};
use axum::http::StatusCode;
use axum::response::Json;
use axum::routing::{delete, get, post};
use axum::Router;
use tracing::{error, info};

/// 构建池子初始化事件相关的路由
pub fn init_pool_event_routes() -> Router {
    Router::new()
        // 基础CRUD操作
        .route("/init-pool-events", post(create_init_pool_event))
        .route("/init-pool-events/:id", get(get_init_pool_event))
        .route("/init-pool-events/:id", delete(delete_init_pool_event))
        // 查询接口（支持多个pool_id）
        .route("/init-pool-events/query", post(query_init_pool_events))
        // 根据pool_id查询
        .route("/init-pool-events/pool/:pool_id", get(get_event_by_pool_id))
        // 根据signature查询（防重）
        .route(
            "/init-pool-events/signature/:signature",
            get(get_initialize_pool_event_by_signature),
        )
        // 用户统计接口
        .route("/init-pool-events/stats/:pool_creator", get(get_user_pool_stats))
}

/// 创建池子初始化事件
#[utoipa::path(
    post,
    path = "/api/v1/solana/events/cpmm/init-pool-events",
    request_body = CreateInitPoolEventRequest,
    responses(
        (status = 201, description = "成功创建池子初始化事件", body = InitPoolEventResponse),
        (status = 400, description = "请求参数错误"),
        (status = 409, description = "事件已存在（pool_id或signature重复）"),
        (status = 500, description = "服务器错误")
    ),
    tag = "Init Pool Events"
)]
pub async fn create_init_pool_event(
    Extension(services): Extension<Services>,
    ValidationExtractor(request): ValidationExtractor<CreateInitPoolEventRequest>,
) -> Result<(StatusCode, Json<ApiResponse<InitPoolEventResponse>>), (StatusCode, Json<ApiResponse<ErrorResponse>>)> {
    info!(
        "收到创建池子初始化事件请求: pool_id={}, signature={}",
        request.pool_id, request.signature
    );

    match services.solana.create_init_pool_event(request).await {
        Ok(response) => {
            info!("池子初始化事件创建成功: id={}", response.id);
            Ok((StatusCode::CREATED, Json(ApiResponse::success(response))))
        }
        Err(e) => {
            error!("池子初始化事件创建失败: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error(ErrorResponse::new(
                    "CREATE_INIT_POOL_EVENT_FAILED",
                    &format!("池子初始化事件创建失败: {}", e),
                ))),
            ))
        }
    }
}

/// 获取池子初始化事件
#[utoipa::path(
    get,
    path = "/api/v1/solana/events/cpmm/init-pool-events/{id}",
    params(
        ("id" = String, Path, description = "事件ID")
    ),
    responses(
        (status = 200, description = "成功获取池子初始化事件", body = InitPoolEventResponse),
        (status = 404, description = "事件不存在"),
        (status = 500, description = "服务器错误")
    ),
    tag = "Init Pool Events"
)]
pub async fn get_init_pool_event(
    Extension(services): Extension<Services>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<InitPoolEventResponse>>, (StatusCode, Json<ApiResponse<ErrorResponse>>)> {
    info!("收到获取池子初始化事件请求: id={}", id);

    match services.solana.get_init_pool_event_by_id(&id).await {
        Ok(response) => {
            info!("池子初始化事件获取成功: id={}", id);
            Ok(Json(ApiResponse::success(response)))
        }
        Err(e) => {
            error!("池子初始化事件获取失败: {}", e);
            let status_code = if e.to_string().contains("not found") {
                StatusCode::NOT_FOUND
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            };

            let error_response = ErrorResponse::new("INIT_POOL_EVENT_GET_FAILED", &format!("获取失败: {}", e));
            Err((status_code, Json(ApiResponse::error(error_response))))
        }
    }
}

/// 删除池子初始化事件
#[utoipa::path(
    delete,
    path = "/api/v1/solana/events/cpmm/init-pool-events/{id}",
    params(
        ("id" = String, Path, description = "事件ID")
    ),
    responses(
        (status = 200, description = "成功删除池子初始化事件"),
        (status = 404, description = "事件不存在"),
        (status = 500, description = "服务器错误")
    ),
    tag = "Init Pool Events"
)]
pub async fn delete_init_pool_event(
    Extension(services): Extension<Services>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<bool>>, (StatusCode, Json<ApiResponse<ErrorResponse>>)> {
    info!("收到删除池子初始化事件请求: id={}", id);

    match services.solana.delete_init_pool_event(&id).await {
        Ok(deleted) => {
            if deleted {
                info!("池子初始化事件删除成功: id={}", id);
                Ok(Json(ApiResponse::success(true)))
            } else {
                error!("池子初始化事件不存在: id={}", id);
                let error_response = ErrorResponse::new("DELETE_POOL_EVENT_GET_FAILED", "获取失败");
                Err((StatusCode::NOT_FOUND, Json(ApiResponse::error(error_response))))
            }
        }
        Err(e) => {
            error!("池子初始化事件删除失败: {}", e);
            let error_response = ErrorResponse::new("DELETE_POOL_EVENT_GET_FAILED", &format!("获取失败: {}", e));
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error(error_response)),
            ))
        }
    }
}

/// 查询池子初始化事件列表（支持多个pool_id）
#[utoipa::path(
    post,
    path = "/api/v1/solana/events/cpmm/init-pool-events/query",
    request_body = QueryInitPoolEventsRequest,
    responses(
        (status = 200, description = "成功查询池子初始化事件", body = InitPoolEventsPageResponse),
        (status = 400, description = "请求参数错误"),
        (status = 500, description = "服务器错误")
    ),
    tag = "Init Pool Events"
)]
pub async fn query_init_pool_events(
    Extension(services): Extension<Services>,
    ValidationExtractor(request): ValidationExtractor<QueryInitPoolEventsRequest>,
) -> Result<Json<ApiResponse<InitPoolEventsPageResponse>>, (StatusCode, Json<ApiResponse<ErrorResponse>>)> {
    info!("收到查询池子初始化事件请求");

    match services.solana.query_init_pool_events(request).await {
        Ok(response) => {
            info!("池子初始化事件查询成功: 共{}条", response.total);
            Ok(Json(ApiResponse::success(response)))
        }
        Err(e) => {
            error!("池子初始化事件查询失败: {}", e);
            let error_response = ErrorResponse::new("INIT_POOL_EVENT_QUERY_FAILED", &format!("获取失败: {}", e));
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error(error_response)),
            ))
        }
    }
}

/// 根据池子ID获取初始化事件
#[utoipa::path(
    get,
    path = "/api/v1/solana/events/cpmm/init-pool-events/pool/{pool_id}",
    params(
        ("pool_id" = String, Path, description = "池子地址")
    ),
    responses(
        (status = 200, description = "成功获取池子初始化事件", body = InitPoolEventResponse),
        (status = 404, description = "事件不存在"),
        (status = 500, description = "服务器错误")
    ),
    tag = "Init Pool Events"
)]
pub async fn get_event_by_pool_id(
    Extension(services): Extension<Services>,
    Path(pool_id): Path<String>,
) -> Result<Json<ApiResponse<InitPoolEventResponse>>, (StatusCode, Json<ApiResponse<ErrorResponse>>)> {
    info!("收到根据pool_id获取池子初始化事件请求: pool_id={}", pool_id);

    match services.solana.get_init_pool_event_by_pool_id(&pool_id).await {
        Ok(response) => {
            info!("池子初始化事件获取成功: pool_id={}", pool_id);
            Ok(Json(ApiResponse::success(response)))
        }
        Err(e) => {
            error!("池子初始化事件获取失败: {}", e);
            let status_code = if e.to_string().contains("not found") {
                StatusCode::NOT_FOUND
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            };
            let error_response = ErrorResponse::new("INIT_POOL_EVENT_GET_FAILED", &format!("获取失败: {}", e));
            Err((status_code, Json(ApiResponse::error(error_response))))
        }
    }
}

/// 根据signature获取初始化事件
#[utoipa::path(
    get,
    path = "/api/v1/solana/events/cpmm/init-pool-events/signature/{signature}",
    params(
        ("signature" = String, Path, description = "交易签名")
    ),
    responses(
        (status = 200, description = "成功获取池子初始化事件", body = InitPoolEventResponse),
        (status = 404, description = "事件不存在"),
        (status = 500, description = "服务器错误")
    ),
    tag = "Init Pool Events"
)]
pub async fn get_initialize_pool_event_by_signature(
    Extension(services): Extension<Services>,
    Path(signature): Path<String>,
) -> Result<Json<ApiResponse<InitPoolEventResponse>>, (StatusCode, Json<ApiResponse<ErrorResponse>>)> {
    info!("收到根据signature获取池子初始化事件请求: signature={}", signature);

    match services.solana.get_init_pool_event_by_signature(&signature).await {
        Ok(response) => {
            info!("池子初始化事件获取成功: signature={}", signature);
            Ok(Json(ApiResponse::success(response)))
        }
        Err(e) => {
            error!("池子初始化事件获取失败: {}", e);
            let status_code = if e.to_string().contains("not found") {
                StatusCode::NOT_FOUND
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            };
            let error_response = ErrorResponse::new("INIT_POOL_EVENT_GET_FAILED", &format!("获取失败: {}", e));
            Err((status_code, Json(ApiResponse::error(error_response))))
        }
    }
}

/// 获取用户池子创建统计
#[utoipa::path(
    get,
    path = "/api/v1/solana/events/cpmm/init-pool-events/stats/{pool_creator}",
    params(
        ("pool_creator" = String, Path, description = "池子创建者地址")
    ),
    responses(
        (status = 200, description = "成功获取用户统计", body = UserPoolStats),
        (status = 500, description = "服务器错误")
    ),
    tag = "Init Pool Events"
)]
pub async fn get_user_pool_stats(
    Extension(services): Extension<Services>,
    Path(pool_creator): Path<String>,
) -> Result<Json<ApiResponse<UserPoolStats>>, (StatusCode, Json<ApiResponse<ErrorResponse>>)> {
    info!("收到获取用户池子统计请求: pool_creator={}", pool_creator);

    match services.solana.get_user_pool_stats(&pool_creator).await {
        Ok(response) => {
            info!("用户池子统计获取成功: pool_creator={}", pool_creator);
            Ok(Json(ApiResponse::success(response)))
        }
        Err(e) => {
            error!("用户池子统计获取失败: {}", e);
            let error_response = ErrorResponse::new("INIT_POOL_EVENT_GET_FAILED", &format!("获取失败: {}", e));
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error(error_response)),
            ))
        }
    }
}
