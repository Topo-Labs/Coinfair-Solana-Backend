use crate::dtos::solana::common::{ApiResponse, ErrorResponse};
use crate::dtos::solana::cpmm::lp_change_event::{
    CreateLpChangeEventRequest, LpChangeEventResponse, LpChangeEventsPageResponse, QueryLpChangeEventsRequest,
};
use crate::extractors::validation_extractor::ValidationExtractor;
use crate::services::Services;
use axum::extract::{Extension, Path};
use axum::http::StatusCode;
use axum::response::Json;
use axum::routing::{delete, get, post};
use axum::Router;
use serde_json::json;
use tracing::{error, info};

/// 构建LP变更事件相关的路由
pub fn lp_change_event_routes() -> Router {
    Router::new()
        // 基础CRUD操作
        .route("/lp-change-events", post(create_lp_change_event))
        .route("/lp-change-events/:id", get(get_lp_change_event))
        .route("/lp-change-events/:id", delete(delete_lp_change_event))
        // 查询接口
        .route("/lp-change-events/query", post(query_lp_change_events))
        // 根据signature查询（防重）
        .route("/lp-change-events/signature/:signature", get(get_event_by_signature))
        // 用户统计信息
        .route("/lp-change-events/stats/:user_wallet", get(get_user_event_stats))
}

/// 创建LP变更事件
#[utoipa::path(
    post,
    path = "/api/v1/solana/events/cpmm/lp-change-events",
    request_body = CreateLpChangeEventRequest,
    responses(
        (status = 201, description = "成功创建LP变更事件", body = LpChangeEventResponse),
        (status = 400, description = "请求参数错误"),
        (status = 409, description = "事件已存在（signature重复）"),
        (status = 500, description = "服务器错误")
    ),
    tag = "LP Change Events"
)]
pub async fn create_lp_change_event(
    Extension(services): Extension<Services>,
    ValidationExtractor(request): ValidationExtractor<CreateLpChangeEventRequest>,
) -> Result<(StatusCode, Json<ApiResponse<LpChangeEventResponse>>), (StatusCode, Json<ApiResponse<ErrorResponse>>)> {
    info!(
        "收到创建LP变更事件请求: user_wallet={}, signature={}",
        request.user_wallet, request.signature
    );

    match services.solana.create_lp_change_event(request).await {
        Ok(response) => {
            info!("LP变更事件创建成功: id={}", response.id);
            Ok((StatusCode::CREATED, Json(ApiResponse::success(response))))
        }
        Err(e) => {
            error!("创建LP变更事件失败: {}", e);
            let error_response = ErrorResponse::new("LP_CHANGE_EVENT_CREATE_FAILED", &format!("创建LP变更事件失败: {}", e));
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error(error_response))))
        }
    }
}

/// 根据ID获取LP变更事件
#[utoipa::path(
    get,
    path = "/api/v1/solana/events/cpmm/lp-change-events/{id}",
    params(
        ("id" = String, Path, description = "事件ID")
    ),
    responses(
        (status = 200, description = "成功获取LP变更事件", body = LpChangeEventResponse),
        (status = 404, description = "事件未找到"),
        (status = 400, description = "无效的事件ID"),
        (status = 500, description = "服务器错误")
    ),
    tag = "LP Change Events"
)]
pub async fn get_lp_change_event(
    Extension(services): Extension<Services>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<LpChangeEventResponse>>, (StatusCode, Json<ApiResponse<ErrorResponse>>)> {
    info!("收到获取LP变更事件请求: id={}", id);

    match services.solana.get_lp_change_event_by_id(&id).await {
        Ok(response) => {
            info!("LP变更事件查询成功: id={}", id);
            Ok(Json(ApiResponse::success(response)))
        }
        Err(e) => {
            error!("查询LP变更事件失败: {}", e);
            let error_response = ErrorResponse::new("LP_CHANGE_EVENT_NOT_FOUND", &format!("查询LP变更事件失败: {}", e));
            Err((StatusCode::NOT_FOUND, Json(ApiResponse::error(error_response))))
        }
    }
}

/// 根据signature获取LP变更事件（防重检查）
#[utoipa::path(
    get,
    path = "/api/v1/solana/events/cpmm/lp-change-events/signature/{signature}",
    params(
        ("signature" = String, Path, description = "交易签名")
    ),
    responses(
        (status = 200, description = "成功获取LP变更事件", body = LpChangeEventResponse),
        (status = 404, description = "事件未找到"),
        (status = 400, description = "signature参数错误"),
        (status = 500, description = "服务器错误")
    ),
    tag = "LP Change Events"
)]
pub async fn get_event_by_signature(
    Extension(services): Extension<Services>,
    Path(signature): Path<String>,
) -> Result<Json<ApiResponse<LpChangeEventResponse>>, (StatusCode, Json<ApiResponse<ErrorResponse>>)> {
    info!("收到根据signature查询LP变更事件请求: signature={}", signature);

    match services.solana.get_lp_change_event_by_signature(&signature).await {
        Ok(response) => {
            info!("LP变更事件查询成功: signature={}", signature);
            Ok(Json(ApiResponse::success(response)))
        }
        Err(e) => {
            error!("根据signature查询LP变更事件失败: {}", e);
            let error_response = ErrorResponse::new("LP_CHANGE_EVENT_NOT_FOUND", &format!("根据signature查询LP变更事件失败: {}", e));
            Err((StatusCode::NOT_FOUND, Json(ApiResponse::error(error_response))))
        }
    }
}

/// 查询LP变更事件列表（支持多个lp_mint）
#[utoipa::path(
    post,
    path = "/api/v1/solana/events/cpmm/lp-change-events/query",
    request_body = QueryLpChangeEventsRequest,
    responses(
        (status = 200, description = "成功查询LP变更事件", body = LpChangeEventsPageResponse),
        (status = 400, description = "请求参数错误"),
        (status = 500, description = "服务器错误")
    ),
    tag = "LP Change Events"
)]
pub async fn query_lp_change_events(
    Extension(services): Extension<Services>,
    ValidationExtractor(request): ValidationExtractor<QueryLpChangeEventsRequest>,
) -> Result<Json<ApiResponse<LpChangeEventsPageResponse>>, (StatusCode, Json<ApiResponse<ErrorResponse>>)> {
    info!(
        "收到查询LP变更事件请求: user_wallet={:?}, pool_id={:?}, page={:?}",
        request.user_wallet, request.pool_id, request.page
    );

    match services.solana.query_lp_change_events(request).await {
        Ok(response) => {
            info!("LP变更事件查询成功: 返回{}条记录", response.data.len());
            Ok(Json(ApiResponse::success(response)))
        }
        Err(e) => {
            error!("查询LP变更事件失败: {}", e);
            let error_response = ErrorResponse::new("LP_CHANGE_EVENT_QUERY_FAILED", &format!("查询LP变更事件失败: {}", e));
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error(error_response))))
        }
    }
}

/// 删除LP变更事件（管理员功能）
#[utoipa::path(
    delete,
    path = "/api/v1/solana/events/cpmm/lp-change-events/{id}",
    params(
        ("id" = String, Path, description = "事件ID")
    ),
    responses(
        (status = 200, description = "成功删除LP变更事件"),
        (status = 404, description = "事件未找到"),
        (status = 400, description = "无效的事件ID"),
        (status = 403, description = "权限不足"),
        (status = 500, description = "服务器错误")
    ),
    tag = "LP Change Events"
)]
pub async fn delete_lp_change_event(
    Extension(services): Extension<Services>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, Json<ApiResponse<ErrorResponse>>)> {
    info!("收到删除LP变更事件请求: id={}", id);

    match services.solana.delete_lp_change_event(&id).await {
        Ok(deleted) => {
            if deleted {
                info!("LP变更事件删除成功: id={}", id);
                let success_data = json!({
                    "message": "事件删除成功",
                    "id": id
                });
                Ok(Json(ApiResponse::success(success_data)))
            } else {
                error!("删除LP变更事件失败，事件可能不存在: id={}", id);
                let error_response = ErrorResponse::new("LP_CHANGE_EVENT_NOT_FOUND", &format!("LP变更事件未找到: {}", id));
                Err((StatusCode::NOT_FOUND, Json(ApiResponse::error(error_response))))
            }
        }
        Err(e) => {
            error!("删除LP变更事件失败: {}", e);
            let error_response = ErrorResponse::new("LP_CHANGE_EVENT_DELETE_FAILED", &format!("删除LP变更事件失败: {}", e));
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error(error_response))))
        }
    }
}

/// 获取用户事件统计信息
#[utoipa::path(
    get,
    path = "/api/v1/solana/events/cpmm/lp-change-events/stats/{user_wallet}",
    params(
        ("user_wallet" = String, Path, description = "用户钱包地址")
    ),
    responses(
        (status = 200, description = "成功获取用户事件统计"),
        (status = 400, description = "用户钱包地址参数错误"),
        (status = 500, description = "服务器错误")
    ),
    tag = "LP Change Events"
)]
pub async fn get_user_event_stats(
    Extension(services): Extension<Services>,
    Path(user_wallet): Path<String>,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, Json<ApiResponse<ErrorResponse>>)> {
    info!("收到获取用户事件统计请求: user_wallet={}", user_wallet);

    match services.solana.get_user_lp_change_event_stats(&user_wallet).await {
        Ok(stats) => {
            info!(
                "用户事件统计查询成功: user_wallet={}, total_events={}",
                user_wallet, stats.total_events
            );
            let stats_data = json!({
                "user_wallet": stats.user_wallet,
                "total_events": stats.total_events,
                "deposit_count": stats.deposit_count,
                "withdraw_count": stats.withdraw_count,
                "initialize_count": stats.initialize_count
            });
            Ok(Json(ApiResponse::success(stats_data)))
        }
        Err(e) => {
            error!("获取用户事件统计失败: {}", e);
            let error_response = ErrorResponse::new("LP_CHANGE_EVENT_STATS_FAILED", &format!("获取用户事件统计失败: {}", e));
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error(error_response))))
        }
    }
}
