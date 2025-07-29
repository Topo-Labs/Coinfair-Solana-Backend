use std::net::SocketAddr;

use crate::dtos::{
    solana_dto::ApiResponse,
    static_dto::{AutoFeeConfig, ChainTimeConfig, ClmmConfig, ClmmConfigResponse, InfoResponse, MintListResponse, RpcConfig, VersionConfig},
};
use axum::{extract::ConnectInfo, response::Json, routing::get, Router};
use tracing::info;

pub struct StaticConfigController;

impl StaticConfigController {
    pub fn routes() -> Router {
        Router::new()
            .route("/main/version", get(get_version))
            .route("/main/auto-fee", get(get_auto_fee))
            .route("/main/rpcs", get(get_rpcs))
            .route("/main/chain-time", get(get_chain_time))
            .route("/main/info", get(get_info))
            .route("/main/clmm-config", get(get_clmm_config))
            .route("/mint/list", get(get_mint_list))
    }
}

/// 获取版本信息
///
/// 返回系统当前版本和最低支持版本信息
///
/// # 响应示例
///
/// ```json
/// {
///   "id": "7c7e0c84-16f8-483a-b4c4-9f96f63c1c9d",
///   "success": true,
///   "data": {
///     "latest": "V3.0.1",
///     "least": "V3.0.1"
///   }
/// }
/// ```
#[utoipa::path(
    get,
    path = "/api/v1/solana/main/version",
    responses(
        (status = 200, description = "版本信息获取成功", body = ApiResponse<VersionConfig>)
    ),
    tag = "系统配置"
)]
pub async fn get_version(ConnectInfo(addr): ConnectInfo<SocketAddr>) -> Json<ApiResponse<VersionConfig>> {
    let client_ip = addr.ip();
    info!("📋 获取版本信息 - 来自IP: {}", client_ip);

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
    path = "/api/v1/solana/main/auto-fee",
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
    path = "/api/v1/solana/main/rpcs",
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
    path = "/api/v1/solana/main/chain-time",
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
    path = "/api/v1/solana/main/info",
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

/// 获取CLMM配置
///
/// 返回系统的CLMM（集中流动性做市商）配置信息
///
/// # 响应示例
///
/// ```json
/// {
///   "id": "61678e4c-9bf8-4924-99c8-0accc723bf5d",
///   "success": true,
///   "data": [
///     {
///       "id": "9iFER3bpjf1PTTCQCfTRu17EJgvsxo9pVyA9QWwEuX4x",
///       "index": 4,
///       "protocolFeeRate": 120000,
///       "tradeFeeRate": 100,
///       "tickSpacing": 1,
///       "fundFeeRate": 40000,
///       "defaultRange": 0.001,
///       "defaultRangePoint": [0.001, 0.003, 0.005, 0.008, 0.01]
///     }
///   ]
/// }
/// ```
#[utoipa::path(
    get,
    path = "/api/v1/solana/main/clmm-config",
    responses(
        (status = 200, description = "CLMM配置获取成功", body = ApiResponse<ClmmConfigResponse>)
    ),
    tag = "系统配置"
)]
pub async fn get_clmm_config() -> Json<ApiResponse<ClmmConfigResponse>> {
    info!("⚙️ 获取CLMM配置");

    let clmm_configs = ClmmConfig::default_configs();

    Json(ApiResponse::success(clmm_configs))
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
    path = "/api/v1/solana/mint/list",
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
