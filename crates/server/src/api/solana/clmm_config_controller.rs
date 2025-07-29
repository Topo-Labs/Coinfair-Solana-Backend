use crate::{dtos::solana_dto::ErrorResponse, extractors::validation_extractor::ValidationExtractor, services::Services};
use axum::{
    extract::Extension,
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use tracing::{error, info};

pub struct ClmmConfigController;

impl ClmmConfigController {
    pub fn routes() -> Router {
        Router::new().route("/", get(get_clmm_configs)).route("/save", post(save_clmm_config))
    }
}

/// 获取CLMM配置列表
///
/// 返回系统中所有可用的CLMM（集中流动性做市商）配置信息。
///
/// # 响应示例
///
/// ```json
/// {
///   "configs": [
///     {
///       "id": "config_0",
///       "index": 0,
///       "protocolFeeRate": 120000,
///       "tradeFeeRate": 100,
///       "tickSpacing": 1,
///       "fundFeeRate": 40000,
///       "description": "0.01% 费率，适合稳定币对",
///       "defaultRange": 0.01,
///       "defaultRangePoint": [0.001, 0.005, 0.01, 0.02, 0.05]
///     },
///     {
///       "id": "config_1",
///       "index": 1,
///       "protocolFeeRate": 120000,
///       "tradeFeeRate": 2500,
///       "tickSpacing": 60,
///       "fundFeeRate": 40000,
///       "description": "0.25% 费率，适合常规交易对",
///       "defaultRange": 0.1,
///       "defaultRangePoint": [0.01, 0.05, 0.1, 0.2, 0.5]
///     }
///   ]
/// }
/// ```
#[utoipa::path(
    get,
    path = "/api/v1/solana/pool/clmm-config",
    responses(
        (status = 200, description = "CLMM配置获取成功", body = static_dto::ClmmConfigResponse),
        (status = 500, description = "内部服务器错误", body = ErrorResponse)
    ),
    tag = "CLMM配置管理"
)]
pub async fn get_clmm_configs(Extension(services): Extension<Services>) -> Result<Json<crate::dtos::static_dto::ClmmConfigResponse>, (StatusCode, Json<ErrorResponse>)> {
    info!("🔧 获取CLMM配置列表");

    match services.solana.get_clmm_configs().await {
        Ok(configs) => {
            info!("✅ CLMM配置获取成功，共{}个配置", configs.len());
            Ok(Json(configs))
        }
        Err(e) => {
            error!("❌ 获取CLMM配置失败: {:?}", e);
            let error_response = ErrorResponse::new("GET_CLMM_CONFIGS_FAILED", &format!("获取CLMM配置失败: {}", e));
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error_response)))
        }
    }
}

/// 保存CLMM配置
///
/// 保存新的CLMM配置到数据库，用于UI创建新的AMM配置。
///
/// # 请求体
///
/// ```json
/// {
///   "index": 20,
///   "protocolFeeRate": 120000,
///   "tradeFeeRate": 5000,
///   "tickSpacing": 60,
///   "fundFeeRate": 40000,
///   "defaultRange": 0.1,
///   "defaultRangePoint": [0.01, 0.05, 0.1, 0.2, 0.5]
/// }
/// ```
///
/// # 响应示例
///
/// ```json
/// {
///   "id": "temp_config_20",
///   "created": true,
///   "message": "成功创建新的CLMM配置，索引: 20"
/// }
/// ```
#[utoipa::path(
    post,
    path = "/api/v1/solana/pool/clmm-config/save",
    request_body = static_dto::SaveClmmConfigRequest,
    responses(
        (status = 200, description = "CLMM配置保存成功", body = static_dto::SaveClmmConfigResponse),
        (status = 400, description = "请求参数错误", body = ErrorResponse),
        (status = 500, description = "内部服务器错误", body = ErrorResponse)
    ),
    tag = "CLMM配置管理"
)]
pub async fn save_clmm_config(
    Extension(services): Extension<Services>,
    ValidationExtractor(request): ValidationExtractor<crate::dtos::static_dto::SaveClmmConfigRequest>,
) -> Result<Json<crate::dtos::static_dto::SaveClmmConfigResponse>, (StatusCode, Json<ErrorResponse>)> {
    info!("💾 保存CLMM配置，索引: {}", request.index);

    match services.solana.save_clmm_config_from_request(request).await {
        Ok(response) => {
            info!("✅ CLMM配置保存成功: {}", response.message);
            Ok(Json(response))
        }
        Err(e) => {
            error!("❌ 保存CLMM配置失败: {:?}", e);
            let error_response = ErrorResponse::new("SAVE_CLMM_CONFIG_FAILED", &format!("保存CLMM配置失败: {}", e));
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error_response)))
        }
    }
}
