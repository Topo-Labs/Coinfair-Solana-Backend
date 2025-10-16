/// NFT 领取统计 Controller
///
/// 提供 NFT 领取统计相关的 Web API 接口
use crate::dtos::solana::common::{ApiResponse, ErrorResponse};
use crate::dtos::solana::cpmm::nft::{NftMintClaimStatsListResponse, NftMintClaimStatsResponse};
use crate::services::solana::cpmm::NftClaimStatsService;
use crate::services::Services;
use axum::{
    extract::{Extension, Path},
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
            .route("/claim-stats", get(get_all_nft_claim_stats))
            .route("/claim-stats/:nft_mint", get(get_nft_claim_stats_by_mint))
    }
}

/// 获取所有 NFT 的领取统计
///
/// GET /api/v1/solana/events/cpmm/nft/claim-stats
///
/// # 响应
/// - 200: 成功返回统计数据列表
/// - 500: 服务器内部错误
#[utoipa::path(
    get,
    path = "/api/v1/solana/events/cpmm/nft/claim-stats",
    responses(
        (status = 200, description = "成功获取所有NFT领取统计", body = ApiResponse<NftMintClaimStatsListResponse>),
        (status = 500, description = "服务器内部错误", body = ApiResponse<ErrorResponse>)
    ),
    tag = "CPMM NFT统计"
)]
pub async fn get_all_nft_claim_stats(
    Extension(services): Extension<Services>,
) -> Result<Json<ApiResponse<NftMintClaimStatsListResponse>>, (StatusCode, Json<ApiResponse<ErrorResponse>>)> {
    info!("📊 [API] 获取所有NFT领取统计");

    // 创建服务实例
    let service = NftClaimStatsService::new(services.database.clone());

    match service.get_all_nft_claim_stats().await {
        Ok(stats) => {
            info!("✅ [API] 成功获取 {} 个NFT的领取统计", stats.total_nfts);
            Ok(Json(ApiResponse::success(stats)))
        }
        Err(e) => {
            error!("❌ [API] 获取所有NFT领取统计失败: {}", e);
            let error_response =
                ErrorResponse::new("NFT_CLAIM_STATS_QUERY_FAILED", &format!("获取NFT领取统计失败: {}", e));
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error(error_response)),
            ))
        }
    }
}

/// 获取指定 NFT 的领取统计
///
/// GET /api/v1/solana/events/cpmm/nft/claim-stats/:nft_mint
///
/// # 参数
/// - `nft_mint`: NFT 地址
///
/// # 响应
/// - 200: 成功返回统计数据
/// - 404: NFT 不存在或没有领取记录
/// - 500: 服务器内部错误
#[utoipa::path(
    get,
    path = "/api/v1/solana/events/cpmm/nft/claim-stats/{nft_mint}",
    params(
        ("nft_mint" = String, Path, description = "NFT地址", example = "NFTaoszFxtEmGXvHcb8yfkGZxqLPAfwDqLN1mhrV2jM")
    ),
    responses(
        (status = 200, description = "成功获取NFT领取统计", body = ApiResponse<NftMintClaimStatsResponse>),
        (status = 404, description = "NFT不存在或没有领取记录", body = ApiResponse<ErrorResponse>),
        (status = 500, description = "服务器内部错误", body = ApiResponse<ErrorResponse>)
    ),
    tag = "CPMM NFT统计"
)]
pub async fn get_nft_claim_stats_by_mint(
    Extension(services): Extension<Services>,
    Path(nft_mint): Path<String>,
) -> Result<Json<ApiResponse<NftMintClaimStatsResponse>>, (StatusCode, Json<ApiResponse<ErrorResponse>>)> {
    info!("📊 [API] 获取NFT领取统计: {}", nft_mint);

    // 创建服务实例
    let service = NftClaimStatsService::new(services.database.clone());

    match service.get_nft_claim_stats_by_mint(&nft_mint).await {
        Ok(Some(stats)) => {
            info!("✅ [API] 成功获取NFT领取统计: {}", nft_mint);
            Ok(Json(ApiResponse::success(stats)))
        }
        Ok(None) => {
            info!("⚠️ [API] NFT {} 没有领取记录", nft_mint);
            let error_response = ErrorResponse::new("NFT_NOT_FOUND", &format!("NFT {} 没有领取记录", nft_mint));
            Err((StatusCode::NOT_FOUND, Json(ApiResponse::error(error_response))))
        }
        Err(e) => {
            error!("❌ [API] 获取NFT领取统计失败 {}: {}", nft_mint, e);
            let error_response =
                ErrorResponse::new("NFT_CLAIM_STATS_QUERY_FAILED", &format!("获取NFT领取统计失败: {}", e));
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
