use crate::dtos::static_dto::{ApiResponse, AutoFeeConfig, ChainTimeConfig, InfoResponse, MintListResponse, MintPriceResponse, PriceData, RpcConfig, VersionConfig};
use axum::{extract::Query, routing::get, Json, Router};
use serde::Deserialize;
use tracing::info;

pub struct StaticController;

impl StaticController {
    pub fn app() -> Router {
        Router::new()
            .route("/version", get(get_version))
            .route("/auto-fee", get(get_auto_fee))
            .route("/rpcs", get(get_rpcs))
            .route("/chain-time", get(get_chain_time))
            .route("/mint/list", get(get_mint_list))
            .route("/mint/price", get(get_mint_price))
            .route("/info", get(get_info))
    }
}

/// 获取版本信息
///
/// 返回系统当前版本信息
///
/// # 响应示例
///
/// ```json
/// {
///   "id": "b7348dae-a4e6-41c2-9d9c-4db2227e4656",
///   "success": true,
///   "data": {
///     "latest": "V3.0.1",
///     "least": "V3.0.1"
///   }
/// }
/// ```
#[utoipa::path(
    get,
    path = "/api/v1/main/version",
    responses(
        (status = 200, description = "版本信息获取成功", body = ApiResponse<VersionConfig>)
    ),
    tag = "系统配置"
)]
pub async fn get_version() -> Json<ApiResponse<VersionConfig>> {
    info!("📋 获取版本信息");

    let version_config = VersionConfig {
        latest: "V3.0.1".to_string(),
        least: "V3.0.1".to_string(),
    };

    Json(ApiResponse::success(version_config))
}

/// 获取自动费用配置
///
/// 返回系统的自动费用配置信息
///
/// # 响应示例
///
/// ```json
/// {
///   "id": "6c6fd9e7-caf0-40de-a75e-b8b2c3ce9012",
///   "success": true,
///   "data": {
///     "default": {
///       "vh": 25216,
///       "h": 18912,
///       "m": 10000
///     }
///   }
/// }
/// ```
#[utoipa::path(
    get,
    path = "/api/v1/main/auto-fee",
    responses(
        (status = 200, description = "自动费用配置获取成功", body = ApiResponse<AutoFeeConfig>)
    ),
    tag = "系统配置"
)]
pub async fn get_auto_fee() -> Json<ApiResponse<AutoFeeConfig>> {
    info!("💰 获取自动费用配置");

    let auto_fee_config = AutoFeeConfig {
        default: AutoFeeConfig::default_fees(),
    };

    Json(ApiResponse::success(auto_fee_config))
}

/// 获取RPC节点配置
///
/// 返回系统的RPC节点配置信息
///
/// # 响应示例
///
/// ```json
/// {
///   "id": "973437f0-af15-4b0e-b124-6d1d6120d0d0",
///   "success": true,
///   "data": {
///     "strategy": "weight",
///     "rpcs": [
///       {
///         "url": "https://api.mainnet-beta.solana.com",
///         "batch": true,
///         "name": "Mainnet",
///         "weight": 100
///       }
///     ]
///   }
/// }
/// ```
#[utoipa::path(
    get,
    path = "/api/v1/main/rpcs",
    responses(
        (status = 200, description = "RPC配置获取成功", body = ApiResponse<RpcConfig>)
    ),
    tag = "系统配置"
)]
pub async fn get_rpcs() -> Json<ApiResponse<RpcConfig>> {
    info!("🔗 获取RPC节点配置");

    let rpc_config = RpcConfig::default();

    Json(ApiResponse::success(rpc_config))
}

/// 获取链时间配置
///
/// 返回系统的链时间配置信息
///
/// # 响应示例
///
/// ```json
/// {
///   "id": "22b93c1f-dcf4-4910-8e7d-c56dbcfc6d95",
///   "success": true,
///   "data": "20"
/// }
/// ```
#[utoipa::path(
    get,
    path = "/api/v1/main/chain-time",
    responses(
        (status = 200, description = "链时间配置获取成功", body = ApiResponse<ChainTimeConfig>)
    ),
    tag = "系统配置"
)]
pub async fn get_chain_time() -> Json<ApiResponse<ChainTimeConfig>> {
    info!("⏰ 获取链时间配置");

    let chain_time_config = ChainTimeConfig { value: "20".to_string() };

    Json(ApiResponse::success(chain_time_config))
}

/// 获取代币列表
///
/// 返回系统支持的代币列表
///
/// # 响应示例
///
/// ```json
/// {
///   "id": "77937111-07f8-43b6-965e-53290f107404",
///   "success": true,
///   "data": {
///     "blacklist": [],
///     "mintList": [...],
///     "whiteList": []
///   }
/// }
/// ```
#[utoipa::path(
    get,
    path = "/api/v1/main/mint/list",
    responses(
        (status = 200, description = "代币列表获取成功", body = ApiResponse<MintListResponse>)
    ),
    tag = "代币信息"
)]
pub async fn get_mint_list() -> Json<ApiResponse<MintListResponse>> {
    info!("🪙 获取代币列表");

    let mint_list = MintListResponse::default();

    Json(ApiResponse::success(mint_list))
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
    path = "/api/v1/main/mint/price",
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

/// 获取系统信息
///
/// 返回系统的24小时交易量和总锁定价值信息
///
/// # 响应示例
///
/// ```json
/// {
///   "id": "3add9ed4-83a3-47c7-b10c-95ea7108b19a",
///   "success": true,
///   "data": {
///     "volume24": 1033122375.6490445,
///     "tvl": 2767700750.290236
///   }
/// }
/// ```
#[utoipa::path(
    get,
    path = "/api/v1/main/info",
    responses(
        (status = 200, description = "系统信息获取成功", body = ApiResponse<InfoResponse>)
    ),
    tag = "系统配置"
)]
pub async fn get_info() -> Json<ApiResponse<InfoResponse>> {
    info!("📊 获取系统信息");

    let info_response = InfoResponse {
        volume24: 1033122375.6490445,
        tvl: 2767700750.290236,
    };

    Json(ApiResponse::success(info_response))
}
