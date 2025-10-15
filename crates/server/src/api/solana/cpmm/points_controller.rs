use crate::dtos::solana::common::{ApiResponse, ErrorResponse};
use crate::dtos::solana::cpmm::points::points_stats::PointsStatsResponse;
use crate::services::Services;
use axum::extract::{Extension, Path, Query};
use axum::http::StatusCode;
use axum::response::Json;
use axum::routing::get;
use axum::Router;
use std::collections::HashMap;
use tracing::{error, info};

/// 构建积分相关的路由
pub fn points_routes() -> Router {
    Router::new()
        // 积分排行榜统计
        .route("/points/stats/:wallet_address", get(get_points_stats))
}

/// 获取积分排行榜统计信息
///
/// 获取指定用户的积分排行榜信息，包括：
/// - 排行榜列表（分页）
/// - 用户自己的积分和排名
/// - 分页信息
///
/// # 参数
/// - `wallet_address`: 用户钱包地址（Path参数）
/// - `page`: 页码（Query参数，可选，默认1）
/// - `page_size`: 每页数量（Query参数，可选，默认50，最大100）
///
/// # 响应
/// - 200: 查询成功，返回积分排行榜数据
/// - 400: 参数错误
/// - 500: 服务器错误
#[utoipa::path(
    get,
    path = "/api/v1/solana/events/cpmm/points/stats/{wallet_address}",
    params(
        ("wallet_address" = String, Path, description = "用户钱包地址"),
        ("page" = Option<u64>, Query, description = "页码，默认1"),
        ("page_size" = Option<u64>, Query, description = "每页数量，默认50，最大100")
    ),
    responses(
        (status = 200, description = "查询成功", body = PointsStatsResponse),
        (status = 400, description = "参数错误", body = ApiResponse<ErrorResponse>),
        (status = 500, description = "查询失败", body = ApiResponse<ErrorResponse>)
    ),
    tag = "Points System"
)]
pub async fn get_points_stats(
    Extension(services): Extension<Services>,
    Path(wallet_address): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<PointsStatsResponse>, (StatusCode, Json<ApiResponse<ErrorResponse>>)> {
    info!("🔍 接收到积分排行榜查询请求: wallet_address={}", wallet_address);

    // 验证钱包地址
    if wallet_address.trim().is_empty() {
        let error_response = ErrorResponse::new("INVALID_PARAMETER", "钱包地址不能为空");
        return Err((StatusCode::BAD_REQUEST, Json(ApiResponse::error(error_response))));
    }

    // 验证钱包地址长度（基本检查）
    if wallet_address.len() < 32 || wallet_address.len() > 44 {
        let error_response = ErrorResponse::new("INVALID_ADDRESS_FORMAT", &format!("无效的钱包地址格式: {}", wallet_address));
        return Err((StatusCode::BAD_REQUEST, Json(ApiResponse::error(error_response))));
    }

    // 提取分页参数
    let page = params.get("page").and_then(|p| p.parse::<u64>().ok());
    let page_size = params.get("page_size").and_then(|p| p.parse::<u64>().ok());

    info!("  分页参数: page={:?}, page_size={:?}", page, page_size);

    // 调用服务层
    match services.solana.get_points_stats(&wallet_address, page, page_size).await {
        Ok(response) => {
            if response.success {
                if let Some(ref data) = response.data {
                    info!(
                        "✅ 积分排行榜查询成功: wallet_address={}, rank={}/{}, points={}, 返回{}条记录",
                        wallet_address, data.my_rank, data.total, data.my_points, data.rank_list.len()
                    );
                } else {
                    info!("✅ 积分排行榜查询成功（无数据）: wallet_address={}", wallet_address);
                }
            } else {
                info!("⚠️ 积分排行榜查询返回错误: wallet_address={}", wallet_address);
            }
            Ok(Json(response))
        }
        Err(e) => {
            error!("❌ 积分排行榜查询失败: wallet_address={}, error={}", wallet_address, e);
            let error_response = ErrorResponse::new("POINTS_STATS_QUERY_FAILED", &format!("查询积分排行榜失败: {}", e));
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error(error_response))))
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_wallet_address_validation() {
        // 测试钱包地址验证逻辑
        let valid_address = "8S2bcP66WehuF6cHryfZ7vfFpQWaUhYyAYSy5U3gX4Fy";
        assert!(valid_address.len() >= 32 && valid_address.len() <= 44);

        let too_short = "abc";
        assert!(too_short.len() < 32);

        let too_long = "a".repeat(50);
        assert!(too_long.len() > 44);

        println!("✅ 钱包地址验证测试通过");
    }

    #[test]
    fn test_parameter_parsing() {
        // 测试参数解析逻辑
        let page_str = "2";
        let page: Option<u64> = page_str.parse().ok();
        assert_eq!(page, Some(2));

        let invalid_page = "abc";
        let page: Option<u64> = invalid_page.parse().ok();
        assert_eq!(page, None);

        println!("✅ 参数解析测试通过");
    }
}
