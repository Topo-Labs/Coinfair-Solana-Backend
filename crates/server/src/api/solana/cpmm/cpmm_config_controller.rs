use crate::dtos::solana::common::ErrorResponse;
use crate::services::Services;
use axum::{
    extract::Extension,
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use tracing::{error, info};
use crate::dtos::statics::static_dto::{ApiResponse, CreateCpmmConfigRequest, CpmmConfig};
use validator::Validate;

pub struct CpmmConfigController;

impl CpmmConfigController {
    pub fn routes() -> Router {
        Router::new()
            .route("/", get(get_cpmm_configs))
            .route("/", post(create_cpmm_config))
    }
}

/// 获取CPMM配置列表
///
/// 返回系统中所有可用的CPMM（恒定乘积做市商）配置信息。
///
/// # 响应示例
///
/// ```json
/// {
///   "id": "8b887691-05cf-4a63-ad02-56f9a90160df",
///   "success": true,
///   "data": [
///     {
///       "id": "D4FPEruKEHrG5TenZ2mpDGEfu1iUvTiqBxvpU8HLBvC2",
///       "index": 0,
///       "protocolFeeRate": 120000,
///       "tradeFeeRate": 2500,
///       "fundFeeRate": 40000,
///       "createPoolFee": 150000000,
///       "creatorFeeRate": 0
///     },
///     {
///       "id": "BgxH5ifebqHDuiADWKhLjXGP5hWZeZLoCdmeWJLkRqLP",
///       "index": 5,
///       "protocolFeeRate": 120000,
///       "tradeFeeRate": 3000,
///       "fundFeeRate": 40000,
///       "createPoolFee": 150000000,
///       "creatorFeeRate": 0
///     }
///   ]
/// }
/// ```
#[utoipa::path(
    get,
    path = "/api/v1/solana/main/cpmm-config",
    responses(
        (status = 200, description = "CPMM配置获取成功", body = ApiResponse<static_dto::CpmmConfigResponse>),
        (status = 500, description = "内部服务器错误", body = ErrorResponse)
    ),
    tag = "CPMM配置管理"
)]
pub async fn get_cpmm_configs(
    Extension(services): Extension<Services>,
) -> Result<Json<ApiResponse<crate::dtos::statics::static_dto::CpmmConfigResponse>>, (StatusCode, Json<ErrorResponse>)> {
    info!("🔧 获取CPMM配置列表");

    match services.solana.get_cpmm_configs().await {
        Ok(configs) => {
            info!("✅ CPMM配置获取成功，共{}个配置", configs.len());
            Ok(Json(ApiResponse::success(configs)))
        }
        Err(e) => {
            error!("❌ 获取CPMM配置失败: {:?}", e);
            let error_response = ErrorResponse::new("GET_CPMM_CONFIGS_FAILED", &format!("获取CPMM配置失败: {}", e));
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error_response)))
        }
    }
}

/// 创建CPMM配置
///
/// 创建新的CPMM（恒定乘积做市商）配置。
///
/// # 请求体示例
///
/// ```json
/// {
///   "id": "D4FPEruKEHrG5TenZ2mpDGEfu1iUvTiqBxvpU8HLBvC2",
///   "index": 0,
///   "protocolFeeRate": 120000,
///   "tradeFeeRate": 2500,
///   "fundFeeRate": 40000,
///   "createPoolFee": 150000000,
///   "creatorFeeRate": 0
/// }
/// ```
///
/// # 响应示例
///
/// ```json
/// {
///   "id": "8b887691-05cf-4a63-ad02-56f9a90160df",
///   "success": true,
///   "data": {
///     "configId": "D4FPEruKEHrG5TenZ2mpDGEfu1iUvTiqBxvpU8HLBvC2",
///     "saved": true
///   }
/// }
/// ```
#[utoipa::path(
    post,
    path = "/api/v1/solana/main/cpmm-config",
    request_body = CreateCpmmConfigRequest,
    responses(
        (status = 200, description = "CPMM配置创建成功", body = ApiResponse<String>),
        (status = 400, description = "请求参数验证失败", body = ErrorResponse),
        (status = 500, description = "内部服务器错误", body = ErrorResponse)
    ),
    tag = "CPMM配置管理"
)]
pub async fn create_cpmm_config(
    Extension(services): Extension<Services>,
    Json(request): Json<CreateCpmmConfigRequest>,
) -> Result<Json<ApiResponse<String>>, (StatusCode, Json<ErrorResponse>)> {
    info!("🔧 创建CPMM配置，ID: {}, 索引: {}", request.id, request.index);

    // 验证请求参数
    if let Err(validation_errors) = request.validate() {
        error!("❌ CPMM配置参数验证失败: {:?}", validation_errors);
        let error_response = ErrorResponse::new("VALIDATION_FAILED", &format!("参数验证失败: {}", validation_errors));
        return Err((StatusCode::BAD_REQUEST, Json(error_response)));
    }

    // 转换为CpmmConfig
    let config = CpmmConfig {
        id: request.id,
        index: request.index,
        protocol_fee_rate: request.protocol_fee_rate,
        trade_fee_rate: request.trade_fee_rate,
        fund_fee_rate: request.fund_fee_rate,
        create_pool_fee: request.create_pool_fee,
        creator_fee_rate: request.creator_fee_rate,
    };

    match services.solana.save_cpmm_config(config.clone()).await {
        Ok(saved_id) => {
            info!("✅ CPMM配置创建成功，ID: {}, 保存ID: {}", config.id, saved_id);
            Ok(Json(ApiResponse::success(saved_id)))
        }
        Err(e) => {
            error!("❌ 创建CPMM配置失败: {:?}", e);
            let error_response = ErrorResponse::new("CREATE_CPMM_CONFIG_FAILED", &format!("创建CPMM配置失败: {}", e));
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error_response)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dtos::statics::static_dto::CreateCpmmConfigRequest;

    #[test]
    fn test_create_cpmm_config_request_validation() {
        // 测试有效的请求
        let valid_request = CreateCpmmConfigRequest {
            id: "D4FPEruKEHrG5TenZ2mpDGEfu1iUvTiqBxvpU8HLBvC2".to_string(),
            index: 0,
            protocol_fee_rate: 120000,
            trade_fee_rate: 2500,
            fund_fee_rate: 40000,
            create_pool_fee: 150000000,
            creator_fee_rate: 0,
        };

        assert!(valid_request.validate().is_ok());

        // 测试无效的请求 - ID太短
        let invalid_request = CreateCpmmConfigRequest {
            id: "".to_string(),
            index: 0,
            protocol_fee_rate: 120000,
            trade_fee_rate: 2500,
            fund_fee_rate: 40000,
            create_pool_fee: 150000000,
            creator_fee_rate: 0,
        };

        assert!(invalid_request.validate().is_err());

        // 测试无效的请求 - fee rate超出范围
        let invalid_request2 = CreateCpmmConfigRequest {
            id: "valid_id".to_string(),
            index: 0,
            protocol_fee_rate: 2000000, // 超出最大值1000000
            trade_fee_rate: 2500,
            fund_fee_rate: 40000,
            create_pool_fee: 150000000,
            creator_fee_rate: 0,
        };

        assert!(invalid_request2.validate().is_err());

        println!("✅ CPMM配置请求验证测试通过");
    }

    #[test]
    fn test_cpmm_config_conversion() {
        let request = CreateCpmmConfigRequest {
            id: "D4FPEruKEHrG5TenZ2mpDGEfu1iUvTiqBxvpU8HLBvC2".to_string(),
            index: 0,
            protocol_fee_rate: 120000,
            trade_fee_rate: 2500,
            fund_fee_rate: 40000,
            create_pool_fee: 150000000,
            creator_fee_rate: 0,
        };

        let config = CpmmConfig {
            id: request.id.clone(),
            index: request.index,
            protocol_fee_rate: request.protocol_fee_rate,
            trade_fee_rate: request.trade_fee_rate,
            fund_fee_rate: request.fund_fee_rate,
            create_pool_fee: request.create_pool_fee,
            creator_fee_rate: request.creator_fee_rate,
        };

        assert_eq!(config.id, request.id);
        assert_eq!(config.index, request.index);
        assert_eq!(config.protocol_fee_rate, request.protocol_fee_rate);
        assert_eq!(config.trade_fee_rate, request.trade_fee_rate);
        assert_eq!(config.fund_fee_rate, request.fund_fee_rate);
        assert_eq!(config.create_pool_fee, request.create_pool_fee);
        assert_eq!(config.creator_fee_rate, request.creator_fee_rate);

        println!("✅ CPMM配置转换测试通过");
    }
}