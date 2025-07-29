use crate::dtos::static_dto::{ApiResponse, MintPriceResponse, PriceData};
use axum::{extract::Query, routing::get, Json, Router};
use serde::Deserialize;
use tracing::info;

pub struct StaticController;

impl StaticController {
    pub fn app() -> Router {
        Router::new().route("/price", get(get_mint_price))
    }
}

/// 查询参数结构体
#[derive(Debug, Deserialize)]
pub struct MintPriceQuery {
    pub mints: String,
}

/// 获取代币价格
///
/// 根据提供的代币mint地址列表查询价格
///
/// # 查询参数
///
/// - mints: 代币mint地址列表，用逗号分隔
///
/// # 响应示例
///
/// ```json
/// {
///   "id": "fe1955f5-91ba-43c6-8d14-cc0588bb71db",
///   "success": true,
///   "data": {
///     "data": [
///       {
///         "mint": "So11111111111111111111111111111111111111112",
///         "price": "0"
///       }
///     ]
///   }
/// }
/// ```
#[utoipa::path(
    get,
    path = "/api/v1/solana/mint/price",
    params(
        ("mints" = String, Query, description = "代币mint地址列表，用逗号分隔")
    ),
    responses(
        (status = 200, description = "代币价格查询成功", body = ApiResponse<MintPriceResponse>)
    ),
    tag = "代币信息"
)]
pub async fn get_mint_price(Query(params): Query<MintPriceQuery>) -> Json<ApiResponse<MintPriceResponse>> {
    info!("💰 获取代币价格，mints: {}", params.mints);

    let mint_addresses: Vec<&str> = params.mints.split(',').collect();

    let mut price_data = Vec::new();
    for mint in mint_addresses {
        price_data.push(PriceData {
            mint: mint.to_string(),
            price: "0".to_string(), // 按照文档要求，全部返回0
        });
    }

    let response = MintPriceResponse { data: price_data };

    Json(ApiResponse::success(response))
}
