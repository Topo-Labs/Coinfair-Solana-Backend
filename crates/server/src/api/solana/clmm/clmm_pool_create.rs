use crate::dtos::solana::common::{ApiResponse, ErrorResponse};
use crate::dtos::solana::clmm::pool::creation::{
    CreatePoolAndSendTransactionResponse, CreatePoolRequest, CreatePoolResponse,
};
use crate::{extractors::validation_extractor::ValidationExtractor, services::Services};
use axum::{extract::Extension, http::StatusCode, response::Json, routing::post, Router};
use tracing::{error, info, warn};

pub struct ClmmPoolCreateController;

impl ClmmPoolCreateController {
    pub fn routes() -> Router {
        Router::new()
            .route("/create", post(create_pool))
            .route("/create-and-send-transaction", post(create_pool_and_send_transaction))
    }
}

/// 创建池子（构建交易）
///
/// 在Raydium AMM V3中创建新的流动性池子，返回未签名的交易数据。
///
/// # 请求体
///
/// ```json
/// {
///   "config_index": 0,
///   "price": 1.5,
///   "mint0": "So11111111111111111111111111111111111111112",
///   "mint1": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
///   "open_time": 0,
///   "user_wallet": "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM"
/// }
/// ```
///
/// # 响应示例
///
/// ```json
/// {
///   "success": true,
///   "data": {
///     "transaction": "Base64编码的未签名交易数据",
///     "transaction_message": "创建池子交易 - 价格: 1.5",
///     "pool_address": "池子地址",
///     "amm_config_address": "AMM配置地址",
///     "token_vault_0": "Token0 Vault地址",
///     "token_vault_1": "Token1 Vault地址",
///     "observation_address": "观察状态地址",
///     "tickarray_bitmap_extension": "Tick Array Bitmap Extension地址",
///     "initial_price": 1.5,
///     "sqrt_price_x64": "价格的sqrt_price_x64表示",
///     "initial_tick": 1234,
///     "timestamp": 1640995200
///   }
/// }
/// ```
#[utoipa::path(
    post,
    path = "/api/v1/solana/pool/create",
    request_body = CreatePoolRequest,
    responses(
        (status = 200, description = "池子创建成功", body = ApiResponse<CreatePoolResponse>),
        (status = 400, description = "请求参数错误", body = ApiResponse<ErrorResponse>),
        (status = 409, description = "池子已存在", body = ApiResponse<ErrorResponse>),
        (status = 500, description = "服务器内部错误", body = ApiResponse<ErrorResponse>)
    ),
    tag = "Solana池子管理"
)]
pub async fn create_pool(
    Extension(services): Extension<Services>,
    ValidationExtractor(request): ValidationExtractor<CreatePoolRequest>,
) -> Result<Json<ApiResponse<CreatePoolResponse>>, (StatusCode, Json<ApiResponse<ErrorResponse>>)> {
    info!("🏗️ 接收到创建池子请求");
    info!("  配置索引: {}", request.config_index);
    info!("  初始价格: {}", request.price);
    info!("  Mint0: {}", request.mint0);
    info!("  Mint1: {}", request.mint1);
    info!("  开放时间: {}", request.open_time);
    info!("  用户钱包: {}", request.user_wallet);

    // 验证价格范围
    if request.price <= 0.0 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error(ErrorResponse::new("INVALID_PRICE", "价格必须大于0"))),
        ));
    }

    // 验证mint地址不能相同
    if request.mint0 == request.mint1 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error(ErrorResponse::new(
                "SAME_MINT_ERROR",
                "两个代币mint地址不能相同",
            ))),
        ));
    }

    match services.solana.create_pool(request).await {
        Ok(response) => {
            info!("✅ 创建池子交易构建成功: {}", response.transaction_message);
            info!("  池子地址: {}", response.pool_address);
            info!("  初始价格: {}", response.initial_price);
            info!("  初始tick: {}", response.initial_tick);
            Ok(Json(ApiResponse::success(response)))
        }
        Err(e) => {
            error!("❌ 创建池子失败: {:?}", e);
            let error_response = ErrorResponse::new("CREATE_POOL_ERROR", &format!("创建池子失败: {}", e));
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error(error_response)),
            ))
        }
    }
}

/// 创建池子并发送交易
///
/// 在Raydium AMM V3中创建新的流动性池子，并立即发送交易到区块链。
///
/// # 请求体
///
/// ```json
/// {
///   "config_index": 0,
///   "price": 1.5,
///   "mint0": "So11111111111111111111111111111111111111112",
///   "mint1": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
///   "open_time": 0,
///   "user_wallet": "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM"
/// }
/// ```
///
/// # 响应示例
///
/// ```json
/// {
///   "success": true,
///   "data": {
///     "signature": "交易签名",
///     "pool_address": "池子地址",
///     "amm_config_address": "AMM配置地址",
///     "token_vault_0": "Token0 Vault地址",
///     "token_vault_1": "Token1 Vault地址",
///     "observation_address": "观察状态地址",
///     "tickarray_bitmap_extension": "Tick Array Bitmap Extension地址",
///     "initial_price": 1.5,
///     "sqrt_price_x64": "价格的sqrt_price_x64表示",
///     "initial_tick": 1234,
///     "status": "Finalized",
///     "explorer_url": "https://explorer.solana.com/tx/...",
///     "timestamp": 1640995200
///   }
/// }
/// ```
#[utoipa::path(
    post,
    path = "/api/v1/solana/pool/create-and-send-transaction",
    request_body = CreatePoolRequest,
    responses(
        (status = 200, description = "创建池子成功", body = ApiResponse<CreatePoolAndSendTransactionResponse>),
        (status = 400, description = "请求参数错误", body = ApiResponse<ErrorResponse>),
        (status = 500, description = "服务器内部错误", body = ApiResponse<ErrorResponse>)
    ),
    tag = "Solana池子管理"
)]
pub async fn create_pool_and_send_transaction(
    Extension(services): Extension<Services>,
    ValidationExtractor(request): ValidationExtractor<CreatePoolRequest>,
) -> Result<Json<ApiResponse<CreatePoolAndSendTransactionResponse>>, (StatusCode, Json<ApiResponse<ErrorResponse>>)> {
    info!("🏗️ 接收到创建池子并发送交易请求");
    info!("  配置索引: {}", request.config_index);
    info!("  初始价格: {}", request.price);
    info!("  Mint0: {}", request.mint0);
    info!("  Mint1: {}", request.mint1);

    // 验证价格范围
    if request.price <= 0.0 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error(ErrorResponse::new("INVALID_PRICE", "价格必须大于0"))),
        ));
    }

    // 验证mint地址不能相同
    if request.mint0 == request.mint1 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error(ErrorResponse::new(
                "SAME_MINT_ERROR",
                "两个代币mint地址不能相同",
            ))),
        ));
    }

    match services.solana.create_pool_and_send_transaction(request).await {
        Ok(response) => {
            info!("✅ 创建池子成功: {}", response.signature);
            info!("  池子地址: {}", response.pool_address);
            info!("  交易状态: {:?}", response.status);
            Ok(Json(ApiResponse::success(response)))
        }
        Err(e) => {
            error!("❌ 创建池子并发送交易失败: {:?}", e);

            // 检查是否是池子已存在的错误
            let error_msg = e.to_string();
            if error_msg.contains("already in use") || error_msg.contains("池子已存在") {
                warn!("🔄 检测到池子已存在");
                let error_response = ErrorResponse::new(
                    "POOL_ALREADY_EXISTS",
                    "该配置和代币对的池子已存在，请检查参数或使用现有池子",
                );
                Err((StatusCode::CONFLICT, Json(ApiResponse::error(error_response))))
            } else {
                let error_response = ErrorResponse::new("CREATE_POOL_ERROR", &format!("创建池子失败: {}", e));
                Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiResponse::error(error_response)),
                ))
            }
        }
    }
}
