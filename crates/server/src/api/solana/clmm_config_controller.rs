use crate::dtos::solana::common::ErrorResponse;
use crate::{dtos::static_dto::ApiResponse, extractors::validation_extractor::ValidationExtractor, services::Services};
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
        Router::new()
            .route("/", get(get_clmm_configs))
            .route("/save", post(save_clmm_config))
            .route("/create", post(create_amm_config))
            .route("/create-and-send", post(create_amm_config_and_send_transaction))
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
///   "id": "c8b629e0-68a2-4409-9773-a4914545dbce",
///   "success": true,
///   "data": [
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
        (status = 200, description = "CLMM配置获取成功", body = ApiResponse<static_dto::ClmmConfigResponse>),
        (status = 500, description = "内部服务器错误", body = ErrorResponse)
    ),
    tag = "CLMM配置管理"
)]
pub async fn get_clmm_configs(
    Extension(services): Extension<Services>,
) -> Result<Json<ApiResponse<crate::dtos::static_dto::ClmmConfigResponse>>, (StatusCode, Json<ErrorResponse>)> {
    info!("🔧 获取CLMM配置列表");

    match services.solana.get_clmm_configs().await {
        Ok(configs) => {
            info!("✅ CLMM配置获取成功，共{}个配置", configs.len());
            Ok(Json(ApiResponse::success(configs)))
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
///   "id": "c8b629e0-68a2-4409-9773-a4914545dbce",
///   "success": true,
///   "data": {
///     "id": "temp_config_20",
///     "created": true,
///     "message": "成功创建新的CLMM配置，索引: 20"
///   }
/// }
/// ```
#[utoipa::path(
    post,
    path = "/api/v1/solana/pool/clmm-config/save",
    request_body = static_dto::SaveClmmConfigRequest,
    responses(
        (status = 200, description = "CLMM配置保存成功", body = ApiResponse<static_dto::SaveClmmConfigResponse>),
        (status = 400, description = "请求参数错误", body = ErrorResponse),
        (status = 500, description = "内部服务器错误", body = ErrorResponse)
    ),
    tag = "CLMM配置管理"
)]
pub async fn save_clmm_config(
    Extension(services): Extension<Services>,
    ValidationExtractor(request): ValidationExtractor<crate::dtos::static_dto::SaveClmmConfigRequest>,
) -> Result<Json<ApiResponse<crate::dtos::static_dto::SaveClmmConfigResponse>>, (StatusCode, Json<ErrorResponse>)> {
    info!("💾 保存CLMM配置，索引: {}", request.index);

    match services.solana.save_clmm_config_from_request(request).await {
        Ok(response) => {
            info!("✅ CLMM配置保存成功: {}", response.message);
            Ok(Json(ApiResponse::success(response)))
        }
        Err(e) => {
            error!("❌ 保存CLMM配置失败: {:?}", e);
            let error_response = ErrorResponse::new("SAVE_CLMM_CONFIG_FAILED", &format!("保存CLMM配置失败: {}", e));
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error_response)))
        }
    }
}

/// 创建AMM配置
///
/// 在Solana链上创建新的AMM配置，设置交易费率、协议费率等参数。
/// 此操作需要管理员权限，会消耗链上交易费用。
///
/// # 参数说明
/// - `configIndex`: 配置索引，必须是未使用的唯一值 (0-65535)
/// - `tickSpacing`: tick间距，决定价格点之间的间隔 (1-1000)
/// - `tradeFeeRate`: 交易费率，以百万分之一为单位 (0-1000000)
/// - `protocolFeeRate`: 协议费率，以百万分之一为单位 (0-1000000)
/// - `fundFeeRate`: 基金费率，以百万分之一为单位 (0-1000000)
///
/// # 响应示例
/// ```json
/// {
///   "id": "c8b629e0-68a2-4409-9773-a4914545dbce",
///   "success": true,
///   "data": {
///     "signature": "3VbKy14uCGGBNGzCgZTSZwVvNGinVr8PWz1...",
///     "configAddress": "DzCP2QVgKD1jbN7X4qN8F5QjVQWbFT1qX9B...",
///     "configIndex": 100,
///     "tickSpacing": 60,
///     "tradeFeeRate": 2500,
///     "protocolFeeRate": 120000,
///     "fundFeeRate": 40000,
///     "timestamp": 1640995200
///   }
/// }
/// ```
#[utoipa::path(
    post,
    path = "/main/clmm-config/create",
    tag = "CLMM配置管理",
    request_body = crate::dtos::static_dto::CreateAmmConfigRequest,
    responses(
        (status = 200, description = "AMM配置创建成功", body = crate::dtos::static_dto::CreateAmmConfigResponse),
        (status = 400, description = "请求参数无效", body = ErrorResponse),
        (status = 409, description = "配置索引已存在", body = ErrorResponse),
        (status = 500, description = "内部服务器错误", body = ErrorResponse)
    )
)]
pub async fn create_amm_config(
    Extension(services): Extension<Services>,
    ValidationExtractor(request): ValidationExtractor<crate::dtos::static_dto::CreateAmmConfigRequest>,
) -> Result<Json<ApiResponse<crate::dtos::static_dto::CreateAmmConfigResponse>>, (StatusCode, Json<ErrorResponse>)> {
    info!("🔧 创建AMM配置，索引: {}", request.config_index);
    info!("  tick间距: {}", request.tick_spacing);
    info!("  交易费率: {}", request.trade_fee_rate);
    info!("  协议费率: {}", request.protocol_fee_rate);
    info!("  基金费率: {}", request.fund_fee_rate);

    match services.solana.create_amm_config(request).await {
        Ok(response) => {
            info!("✅ AMM配置交易构建成功");
            info!("  交易消息: {}", response.transaction_message);
            info!("  配置地址: {}", response.config_address);
            Ok(Json(ApiResponse::success(response)))
        }
        Err(e) => {
            error!("❌ 创建AMM配置失败: {:?}", e);

            // 根据错误类型返回不同的HTTP状态码
            let (status_code, error_code) = if e.to_string().contains("已存在") {
                (StatusCode::CONFLICT, "CONFIG_INDEX_EXISTS")
            } else if e.to_string().contains("获取") {
                (StatusCode::INTERNAL_SERVER_ERROR, "CONFIG_ERROR")
            } else {
                (StatusCode::INTERNAL_SERVER_ERROR, "CREATE_AMM_CONFIG_FAILED")
            };

            let error_response = ErrorResponse::new(error_code, &format!("创建AMM配置失败: {}", e));
            Err((status_code, Json(error_response)))
        }
    }
}

/// 创建AMM配置并发送交易
///
/// 在Solana链上创建新的AMM配置并直接发送交易，同时保存配置到数据库。
/// 此接口主要用于测试目的，不依赖前端钱包签名。
///
/// # 参数说明
/// - `configIndex`: 配置索引，必须是未使用的唯一值 (0-65535)
/// - `tickSpacing`: tick间距，决定价格点之间的间隔 (1-1000)
/// - `tradeFeeRate`: 交易费率，以百万分之一为单位 (0-1000000)
/// - `protocolFeeRate`: 协议费率，以百万分之一为单位 (0-1000000)
/// - `fundFeeRate`: 基金费率，以百万分之一为单位 (0-1000000)
///
/// # 响应示例
/// ```json
/// {
///   "id": "c8b629e0-68a2-4409-9773-a4914545dbce",
///   "success": true,
///   "data": {
///     "signature": "3VbKy14uCGGBNGzCgZTSZwVvNGinVr8PWz1...",
///     "configAddress": "DzCP2QVgKD1jbN7X4qN8F5QjVQWbFT1qX9B...",
///     "configIndex": 100,
///     "tickSpacing": 60,
///     "tradeFeeRate": 2500,
///     "protocolFeeRate": 120000,
///     "fundFeeRate": 40000,
///     "explorerUrl": "https://explorer.solana.com/tx/...",
///     "dbSaveResponse": {
///       "id": "config_pda_address",
///       "created": true,
///       "message": "成功创建新的CLMM配置，索引: 100"
///     },
///     "timestamp": 1640995200
///   }
/// }
/// ```
#[utoipa::path(
    post,
    path = "/main/clmm-config/create-and-send",
    tag = "CLMM配置管理",
    request_body = crate::dtos::static_dto::CreateAmmConfigRequest,
    responses(
        (status = 200, description = "AMM配置创建并发送成功", body = crate::dtos::static_dto::CreateAmmConfigAndSendTransactionResponse),
        (status = 400, description = "请求参数无效", body = ErrorResponse),
        (status = 409, description = "配置索引已存在", body = ErrorResponse),
        (status = 500, description = "内部服务器错误", body = ErrorResponse)
    )
)]
pub async fn create_amm_config_and_send_transaction(
    Extension(services): Extension<Services>,
    ValidationExtractor(request): ValidationExtractor<crate::dtos::static_dto::CreateAmmConfigRequest>,
) -> Result<
    Json<ApiResponse<crate::dtos::static_dto::CreateAmmConfigAndSendTransactionResponse>>,
    (StatusCode, Json<ErrorResponse>),
> {
    info!("🚀 创建AMM配置并发送交易，索引: {}", request.config_index);
    info!("  tick间距: {}", request.tick_spacing);
    info!("  交易费率: {}", request.trade_fee_rate);
    info!("  协议费率: {}", request.protocol_fee_rate);
    info!("  基金费率: {}", request.fund_fee_rate);

    match services.solana.create_amm_config_and_send_transaction(request).await {
        Ok(response) => {
            info!("✅ AMM配置创建并发送交易成功");
            info!("  交易签名: {}", response.signature);
            info!("  配置地址: {}", response.config_address);
            info!("  数据库保存: {}", response.db_save_response.message);
            Ok(Json(ApiResponse::success(response)))
        }
        Err(e) => {
            error!("❌ 创建AMM配置并发送交易失败: {:?}", e);

            // 根据错误类型返回不同的HTTP状态码
            let (status_code, error_code) = if e.to_string().contains("已存在") {
                (StatusCode::CONFLICT, "CONFIG_INDEX_EXISTS")
            } else if e.to_string().contains("获取") || e.to_string().contains("私钥") {
                (StatusCode::INTERNAL_SERVER_ERROR, "CONFIG_ERROR")
            } else {
                (StatusCode::INTERNAL_SERVER_ERROR, "CREATE_AMM_CONFIG_AND_SEND_FAILED")
            };

            let error_response = ErrorResponse::new(error_code, &format!("创建AMM配置并发送交易失败: {}", e));
            Err((status_code, Json(error_response)))
        }
    }
}
