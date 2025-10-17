/// NFT 领取统计 Controller
///
/// 提供 NFT 领取统计相关的 Web API 接口
/// 注意：统计维度为按推荐人（referrer）统计
use crate::dtos::solana::common::{ApiResponse, ErrorResponse};
use crate::dtos::solana::cpmm::nft::{PaginatedReferrerStatsResponse, ReferrerStatsQuery, ReferrerStatsResponse};
use crate::services::solana::cpmm::NftClaimStatsService;
use crate::services::Services;
use axum::{
    extract::{Extension, Path, Query},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use tracing::{error, info};

/// NFT 领取统计 Controller
pub struct NftClaimStatsController;

impl NftClaimStatsController {
    /// 创建路由
    pub fn routes() -> Router {
        Router::new()
            .route("/claim-stats/by-referrer", get(get_all_claimer_stats))
            .route("/claim-stats/by-referrer/:referrer", get(get_claimer_stats_by_address))
    }
}

/// 获取所有推荐人的统计（分页版本）
///
/// GET /api/v1/solana/events/cpmm/nft/claim-stats/by-referrer
///
/// # 查询参数
/// - `page`: 页码（默认：1）
/// - `page_size`: 每页条数（默认：20）
/// - `sort_by`: 排序字段（默认：referred_count）
/// - `sort_order`: 排序方向（asc/desc，默认：desc）
///
/// # 响应
/// - 200: 成功返回分页统计数据
/// - 500: 服务器内部错误
#[utoipa::path(
    get,
    path = "/api/v1/solana/events/cpmm/nft/claim-stats/by-referrer",
    params(
        ReferrerStatsQuery
    ),
    responses(
        (status = 200, description = "成功获取所有推荐人统计（分页）", body = ApiResponse<PaginatedReferrerStatsResponse>),
        (status = 500, description = "服务器内部错误", body = ApiResponse<ErrorResponse>)
    ),
    tag = "CPMM NFT统计"
)]
pub async fn get_all_claimer_stats(
    Extension(services): Extension<Services>,
    Query(query): Query<ReferrerStatsQuery>,
) -> Result<Json<ApiResponse<PaginatedReferrerStatsResponse>>, (StatusCode, Json<ApiResponse<ErrorResponse>>)> {
    info!(
        "📊 [API] 获取所有推荐人统计（分页）: page={}, page_size={}, sort_by={:?}, sort_order={:?}",
        query.page, query.page_size, query.sort_by, query.sort_order
    );

    // 创建服务实例
    let service = NftClaimStatsService::new(services.database.clone());

    match service
        .get_all_claimer_stats_paginated(query.page, query.page_size, query.sort_by, query.sort_order)
        .await
    {
        Ok(stats) => {
            info!(
                "✅ [API] 成功获取推荐人统计（分页）: 返回 {} 条记录，总共 {} 条，共 {} 页",
                stats.items.len(),
                stats.total,
                stats.total_pages
            );
            Ok(Json(ApiResponse::success(stats)))
        }
        Err(e) => {
            error!("❌ [API] 获取推荐人统计（分页）失败: {}", e);
            let error_response = ErrorResponse::new("REFERRER_STATS_QUERY_FAILED", &format!("获取推荐人统计失败: {}", e));
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error(error_response)),
            ))
        }
    }
}

/// 获取指定推荐人的统计
///
/// GET /api/v1/solana/events/cpmm/nft/claim-stats/by-referrer/:referrer
///
/// # 参数
/// - `referrer`: 推荐人地址
///
/// # 响应
/// - 200: 成功返回统计数据
/// - 404: 推荐人不存在或没有推荐记录
/// - 500: 服务器内部错误
#[utoipa::path(
    get,
    path = "/api/v1/solana/events/cpmm/nft/claim-stats/by-referrer/{referrer}",
    params(
        ("referrer" = String, Path, description = "推荐人地址", example = "9ZNTfG4NyQgxy2SWjSiQoUyBPEvXT2xo7fKc5hPYYJ7b")
    ),
    responses(
        (status = 200, description = "成功获取推荐人统计", body = ApiResponse<ReferrerStatsResponse>),
        (status = 404, description = "推荐人不存在或没有推荐记录", body = ApiResponse<ErrorResponse>),
        (status = 500, description = "服务器内部错误", body = ApiResponse<ErrorResponse>)
    ),
    tag = "CPMM NFT统计"
)]
pub async fn get_claimer_stats_by_address(
    Extension(services): Extension<Services>,
    Path(referrer): Path<String>,
) -> Result<Json<ApiResponse<ReferrerStatsResponse>>, (StatusCode, Json<ApiResponse<ErrorResponse>>)> {
    info!("📊 [API] 获取推荐人统计: {}", referrer);

    // 创建服务实例
    let service = NftClaimStatsService::new(services.database.clone());

    match service.get_claimer_stats_by_address(&referrer).await {
        Ok(Some(stats)) => {
            info!("✅ [API] 成功获取推荐人统计: {}", referrer);
            Ok(Json(ApiResponse::success(stats)))
        }
        Ok(None) => {
            info!("⚠️ [API] 推荐人 {} 没有推荐记录", referrer);
            let error_response = ErrorResponse::new("REFERRER_NOT_FOUND", &format!("推荐人 {} 没有推荐记录", referrer));
            Err((StatusCode::NOT_FOUND, Json(ApiResponse::error(error_response))))
        }
        Err(e) => {
            error!("❌ [API] 获取推荐人统计失败 {}: {}", referrer, e);
            let error_response =
                ErrorResponse::new("REFERRER_STATS_QUERY_FAILED", &format!("获取推荐人统计失败: {}", e));
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error(error_response)),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nft_claim_stats_controller_routes() {
        // 测试路由创建
        let _router = NftClaimStatsController::routes();
        assert!(true, "路由创建测试通过");
    }
}
