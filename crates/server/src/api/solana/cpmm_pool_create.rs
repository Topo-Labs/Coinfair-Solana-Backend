use crate::{
    dtos::solana_dto::{
        ApiResponse, CreateClassicAmmPoolAndSendTransactionResponse, CreateClassicAmmPoolRequest,
        CreateClassicAmmPoolResponse, ErrorResponse,
    },
    extractors::validation_extractor::ValidationExtractor,
    services::Services,
};
use axum::{extract::Extension, http::StatusCode, response::Json, routing::post, Router};
use tracing::{error, info, warn};

pub struct CpmmPoolCreateController;

impl CpmmPoolCreateController {
    pub fn routes() -> Router {
        Router::new().route("/create-amm", post(create_classic_amm_pool)).route(
            "/create-amm-and-send-transaction",
            post(create_classic_amm_pool_and_send_transaction),
        )
    }
}

/// 创建经典AMM池子
///
/// 创建基于Raydium V2 AMM的经典流动性池子，需要提供两种代币的初始流动性。
///
/// # 请求体
///
/// ```json
/// {
///   "mint0": "So11111111111111111111111111111111111111112",
///   "mint1": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
///   "init_amount_0": 1000000000,
///   "init_amount_1": 100000000,
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
///     "transaction_message": "创建经典AMM池子交易",
///     "pool_address": "池子地址",
///     "coin_mint": "Coin代币mint地址",
///     "pc_mint": "PC代币mint地址",
///     "coin_vault": "Coin代币账户地址",
///     "pc_vault": "PC代币账户地址",
///     "lp_mint": "LP代币mint地址",
///     "open_orders": "Open orders地址",
///     "target_orders": "Target orders地址",
///     "withdraw_queue": "Withdraw queue地址",
///     "init_coin_amount": 1000000000,
///     "init_pc_amount": 100000000,
///     "open_time": 0,
///     "timestamp": 1640995200
///   }
/// }
/// ```
#[utoipa::path(
    post,
    path = "/api/v1/solana/pool/create-amm",
    request_body = CreateClassicAmmPoolRequest,
    responses(
        (status = 200, description = "池子创建成功", body = ApiResponse<CreateClassicAmmPoolResponse>),
        (status = 400, description = "请求参数错误", body = ApiResponse<ErrorResponse>),
        (status = 409, description = "池子已存在", body = ApiResponse<ErrorResponse>),
        (status = 500, description = "服务器内部错误", body = ApiResponse<ErrorResponse>)
    ),
    tag = "Solana经典AMM"
)]
pub async fn create_classic_amm_pool(
    Extension(services): Extension<Services>,
    ValidationExtractor(request): ValidationExtractor<CreateClassicAmmPoolRequest>,
) -> Result<Json<ApiResponse<CreateClassicAmmPoolResponse>>, (StatusCode, Json<ApiResponse<ErrorResponse>>)> {
    info!("🏗️ 接收到创建经典AMM池子请求");
    info!("  Mint0: {}", request.mint0);
    info!("  Mint1: {}", request.mint1);
    info!("  初始数量0: {}", request.init_amount_0);
    info!("  初始数量1: {}", request.init_amount_1);
    info!("  开放时间: {}", request.open_time);
    info!("  用户钱包: {}", request.user_wallet);

    match services.solana.create_classic_amm_pool(request).await {
        Ok(response) => {
            info!("✅ 经典AMM池子创建交易构建成功");
            info!("  池子地址: {}", response.pool_address);
            info!("  Coin Mint: {}", response.coin_mint);
            info!("  PC Mint: {}", response.pc_mint);
            Ok(Json(ApiResponse::success(response)))
        }
        Err(e) => {
            error!("❌ 创建经典AMM池子失败: {:?}", e);

            // 检查是否是池子已存在的错误
            let error_msg = e.to_string();
            if error_msg.contains("already in use") || error_msg.contains("池子已存在") {
                warn!("🔄 检测到经典AMM池子已存在");
                let error_response = ErrorResponse::new(
                    "CLASSIC_AMM_POOL_ALREADY_EXISTS",
                    "该代币对的经典AMM池子已存在，请检查参数或使用现有池子",
                );
                Err((StatusCode::CONFLICT, Json(ApiResponse::error(error_response))))
            } else {
                let error_response =
                    ErrorResponse::new("CREATE_CLASSIC_AMM_POOL_ERROR", &format!("创建经典AMM池子失败: {}", e));
                Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiResponse::error(error_response)),
                ))
            }
        }
    }
}

/// 创建经典AMM池子并发送交易
///
/// 创建基于Raydium V2 AMM的经典流动性池子并立即发送交易到区块链。
///
/// # 请求体
///
/// ```json
/// {
///   "mint0": "So11111111111111111111111111111111111111112",
///   "mint1": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
///   "init_amount_0": 1000000000,
///   "init_amount_1": 100000000,
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
///     "coin_mint": "Coin代币mint地址",
///     "pc_mint": "PC代币mint地址",
///     "coin_vault": "Coin代币账户地址",
///     "pc_vault": "PC代币账户地址",
///     "lp_mint": "LP代币mint地址",
///     "open_orders": "Open orders地址",
///     "target_orders": "Target orders地址",
///     "withdraw_queue": "Withdraw queue地址",
///     "actual_coin_amount": 1000000000,
///     "actual_pc_amount": 100000000,
///     "open_time": 0,
///     "status": "Pending",
///     "explorer_url": "https://explorer.solana.com/tx/...",
///     "timestamp": 1640995200
///   }
/// }
/// ```
#[utoipa::path(
    post,
    path = "/api/v1/solana/pool/create-amm-and-send-transaction",
    request_body = CreateClassicAmmPoolRequest,
    responses(
        (status = 200, description = "池子创建并发送成功", body = ApiResponse<CreateClassicAmmPoolAndSendTransactionResponse>),
        (status = 400, description = "请求参数错误", body = ApiResponse<ErrorResponse>),
        (status = 409, description = "池子已存在", body = ApiResponse<ErrorResponse>),
        (status = 500, description = "服务器内部错误", body = ApiResponse<ErrorResponse>)
    ),
    tag = "Solana经典AMM"
)]
pub async fn create_classic_amm_pool_and_send_transaction(
    Extension(services): Extension<Services>,
    ValidationExtractor(request): ValidationExtractor<CreateClassicAmmPoolRequest>,
) -> Result<
    Json<ApiResponse<CreateClassicAmmPoolAndSendTransactionResponse>>,
    (StatusCode, Json<ApiResponse<ErrorResponse>>),
> {
    info!("🚀 接收到创建经典AMM池子并发送交易请求");
    info!("  Mint0: {}", request.mint0);
    info!("  Mint1: {}", request.mint1);
    info!("  初始数量0: {}", request.init_amount_0);
    info!("  初始数量1: {}", request.init_amount_1);
    info!("  开放时间: {}", request.open_time);
    info!("  用户钱包: {}", request.user_wallet);

    match services
        .solana
        .create_classic_amm_pool_and_send_transaction(request)
        .await
    {
        Ok(response) => {
            info!("✅ 经典AMM池子创建并发送交易成功: {}", response.signature);
            info!("  池子地址: {}", response.pool_address);
            info!("  交易状态: {:?}", response.status);
            Ok(Json(ApiResponse::success(response)))
        }
        Err(e) => {
            error!("❌ 创建经典AMM池子并发送交易失败: {:?}", e);

            // 检查是否是池子已存在的错误
            let error_msg = e.to_string();
            if error_msg.contains("already in use") || error_msg.contains("池子已存在") {
                warn!("🔄 检测到经典AMM池子已存在");
                let error_response = ErrorResponse::new(
                    "CLASSIC_AMM_POOL_ALREADY_EXISTS",
                    "该代币对的经典AMM池子已存在，请检查参数或使用现有池子",
                );
                Err((StatusCode::CONFLICT, Json(ApiResponse::error(error_response))))
            } else {
                let error_response =
                    ErrorResponse::new("CREATE_CLASSIC_AMM_POOL_ERROR", &format!("创建经典AMM池子失败: {}", e));
                Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiResponse::error(error_response)),
                ))
            }
        }
    }
}
