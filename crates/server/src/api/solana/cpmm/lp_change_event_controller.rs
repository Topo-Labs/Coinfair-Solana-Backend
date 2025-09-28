use crate::dtos::solana::common::{ApiResponse, ErrorResponse};
use crate::dtos::solana::cpmm::lp::lp_change_event::{
    CreateLpChangeEventRequest, LpChangeEventResponse, LpChangeEventsPageResponse, QueryLpChangeEventsRequest,
};
use crate::dtos::solana::cpmm::lp::query_lp_mint::{LpMintPoolInfo, QueryLpMintRequest};
use crate::extractors::validation_extractor::ValidationExtractor;
use crate::services::Services;
use axum::extract::{Extension, Path, Query};
use axum::http::StatusCode;
use axum::response::Json;
use axum::routing::{delete, get, post};
use axum::Router;
use serde_json::json;
use std::collections::HashMap;
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
        // 新的LP mint查询接口
        .route("/query-lp-mint/query", get(query_lp_mint_pools))
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
            let error_response =
                ErrorResponse::new("LP_CHANGE_EVENT_CREATE_FAILED", &format!("创建LP变更事件失败: {}", e));
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error(error_response)),
            ))
        }
    }
}

/// 根据LP mint查询池子信息
#[utoipa::path(
    get,
    path = "/api/v1/solana/events/cpmm/query-lp-mint/query",
    params(
        ("lps" = String, Query, description = "多个LP mint地址，用英文逗号分隔"),
        ("page" = Option<u64>, Query, description = "页码，默认1"),
        ("page_size" = Option<u64>, Query, description = "每页数量，默认20，最大100")
    ),
    responses(
        (status = 200, description = "查询成功", body = ApiResponse<Vec<LpMintPoolInfo>>),
        (status = 400, description = "参数错误", body = ApiResponse<ErrorResponse>),
        (status = 500, description = "查询失败", body = ApiResponse<ErrorResponse>)
    ),
    tag = "LP Change Events"
)]
pub async fn query_lp_mint_pools(
    Extension(services): Extension<Services>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<ApiResponse<Vec<Option<LpMintPoolInfo>>>>, (StatusCode, Json<ApiResponse<ErrorResponse>>)> {
    info!("🔍 接收到LP mint池子查询请求");

    // 提取并验证参数
    let lps = params.get("lps").ok_or_else(|| {
        let error_response = ErrorResponse::new("MISSING_PARAMETER", "缺少lps参数");
        (StatusCode::BAD_REQUEST, Json(ApiResponse::error(error_response)))
    })?;

    if lps.trim().is_empty() {
        let error_response = ErrorResponse::new("INVALID_PARAMETER", "lps参数不能为空");
        return Err((StatusCode::BAD_REQUEST, Json(ApiResponse::error(error_response))));
    }

    info!("  LP地址: {}", lps);

    // 构建请求对象
    let request = QueryLpMintRequest {
        lps: lps.clone(),
        page: params.get("page").and_then(|p| p.parse::<u64>().ok()),
        page_size: params.get("page_size").and_then(|p| p.parse::<u64>().ok()),
    };

    // 验证LP地址格式
    let lp_addresses: Vec<&str> = lps.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();

    if lp_addresses.is_empty() {
        let error_response = ErrorResponse::new("INVALID_PARAMETER", "解析到的LP地址为空");
        return Err((StatusCode::BAD_REQUEST, Json(ApiResponse::error(error_response))));
    }

    // 限制一次查询的LP数量
    if lp_addresses.len() > 100 {
        let error_response = ErrorResponse::new("PARAMETER_LIMIT_EXCEEDED", "一次查询的LP地址数量不能超过100个");
        return Err((StatusCode::BAD_REQUEST, Json(ApiResponse::error(error_response))));
    }

    // 验证每个地址的格式（基本长度检查）
    for addr in &lp_addresses {
        if addr.len() < 32 || addr.len() > 44 {
            let error_response = ErrorResponse::new("INVALID_ADDRESS_FORMAT", &format!("无效的LP地址格式: {}", addr));
            return Err((StatusCode::BAD_REQUEST, Json(ApiResponse::error(error_response))));
        }
    }

    // 调用服务层
    match services.solana.query_lp_mint_pools(request).await {
        Ok(pool_infos) => {
            info!("✅ LP mint池子查询成功，返回{}个池子", pool_infos.len());
            Ok(Json(ApiResponse::success(pool_infos)))
        }
        Err(e) => {
            error!("❌ LP mint池子查询失败: {:?}", e);
            let error_response =
                ErrorResponse::new("QUERY_LP_MINT_POOLS_FAILED", &format!("查询LP mint池子失败: {}", e));
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error(error_response)),
            ))
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
            let error_response = ErrorResponse::new(
                "LP_CHANGE_EVENT_NOT_FOUND",
                &format!("根据signature查询LP变更事件失败: {}", e),
            );
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
            let error_response =
                ErrorResponse::new("LP_CHANGE_EVENT_QUERY_FAILED", &format!("查询LP变更事件失败: {}", e));
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error(error_response)),
            ))
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
                let error_response =
                    ErrorResponse::new("LP_CHANGE_EVENT_NOT_FOUND", &format!("LP变更事件未找到: {}", id));
                Err((StatusCode::NOT_FOUND, Json(ApiResponse::error(error_response))))
            }
        }
        Err(e) => {
            error!("删除LP变更事件失败: {}", e);
            let error_response =
                ErrorResponse::new("LP_CHANGE_EVENT_DELETE_FAILED", &format!("删除LP变更事件失败: {}", e));
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error(error_response)),
            ))
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
            let error_response =
                ErrorResponse::new("LP_CHANGE_EVENT_STATS_FAILED", &format!("获取用户事件统计失败: {}", e));
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error(error_response)),
            ))
        }
    }
}
