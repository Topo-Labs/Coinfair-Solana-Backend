use crate::dtos::solana::common::{ApiResponse, ErrorResponse};
use crate::dtos::solana::cpmm::swap::{
    CpmmSwapBaseInCompute, CpmmSwapBaseInRequest, CpmmSwapBaseInResponse,
    CpmmSwapBaseInTransactionRequest, CpmmTransactionData,
};
use crate::{extractors::validation_extractor::ValidationExtractor, services::Services};
use axum::{
    extract::Extension,
    http::StatusCode,
    response::Json,
    routing::post,
    Router,
};
use tracing::{error, info};

/// CPMM交换控制器
pub struct CpmmSwapController;

impl CpmmSwapController {
    /// 创建CPMM交换相关路由
    pub fn routes() -> Router {
        Router::new()
            .route("/cpmm/swap/base-in", post(swap_base_in))
            .route("/cpmm/swap/base-in/compute", post(compute_swap_base_in))
            .route("/cpmm/swap/base-in/transaction", post(build_swap_base_in_transaction))
    }
}

/// 执行CPMM SwapBaseIn交换
///
/// 基于固定输入金额执行CPMM代币交换
///
/// # 请求体
///
/// ```json
/// {
///   "pool_id": "8k7F9Xb2wVxeJY4QcLrPz1cDEf3GhJ5mNvRtU6sB2W9A",
///   "user_input_token": "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM",
///   "user_input_amount": 1000000000,
///   "slippage": 0.5
/// }
/// ```
///
/// # 响应
///
/// ```json
/// {
///   "success": true,
///   "data": {
///     "signature": "5VfYe...transaction_signature",
///     "pool_id": "8k7F9Xb2wVxeJY4QcLrPz1cDEf3GhJ5mNvRtU6sB2W9A",
///     "input_token_mint": "So11111111111111111111111111111111111111112",
///     "output_token_mint": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
///     "actual_amount_in": 995000000,
///     "amount_out": 10500000,
///     "amount_received": 10450000,
///     "minimum_amount_out": 10400000,
///     "status": "Confirmed",
///     "explorer_url": "https://solscan.io/tx/5VfYe...",
///     "timestamp": 1678901234
///   }
/// }
/// ```
#[utoipa::path(
    post,
    path = "/api/v1/solana/cpmm/swap/base-in",
    request_body = CpmmSwapBaseInRequest,
    responses(
        (status = 200, description = "交换成功", body = ApiResponse<CpmmSwapBaseInResponse>),
        (status = 400, description = "请求参数错误", body = ApiResponse<ErrorResponse>),
        (status = 500, description = "内部服务器错误", body = ApiResponse<ErrorResponse>)
    ),
    tag = "CPMM交换"
)]
pub async fn swap_base_in(
    Extension(services): Extension<Services>,
    ValidationExtractor(request): ValidationExtractor<CpmmSwapBaseInRequest>,
) -> Result<Json<ApiResponse<CpmmSwapBaseInResponse>>, (StatusCode, Json<ApiResponse<ErrorResponse>>)> {
    info!(
        "🔄 收到CPMM SwapBaseIn请求: pool_id={}, user_input_token={}, amount={}",
        request.pool_id, request.user_input_token, request.user_input_amount
    );

    match services.solana.cpmm_swap_base_in(request).await {
        Ok(response) => {
            info!("✅ CPMM SwapBaseIn成功，交易签名: {}", response.signature);
            Ok(Json(ApiResponse::success(response)))
        }
        Err(e) => {
            error!("❌ CPMM SwapBaseIn失败: {:?}", e);
            let error_response = ErrorResponse::new("CPMM_SWAP_BASE_IN_FAILED", &format!("CPMM SwapBaseIn失败: {}", e));
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error(error_response)),
            ))
        }
    }
}

/// 计算CPMM SwapBaseIn交换结果
///
/// 预计算交换结果，不执行实际交换，用于获取报价
///
/// # 请求体
///
/// ```json
/// {
///   "pool_id": "8k7F9Xb2wVxeJY4QcLrPz1cDEf3GhJ5mNvRtU6sB2W9A",
///   "user_input_token": "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM",
///   "user_input_amount": 1000000000,
///   "slippage": 0.5
/// }
/// ```
///
/// # 响应
///
/// ```json
/// {
///   "success": true,
///   "data": {
///     "pool_id": "8k7F9Xb2wVxeJY4QcLrPz1cDEf3GhJ5mNvRtU6sB2W9A",
///     "input_token_mint": "So11111111111111111111111111111111111111112",
///     "output_token_mint": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
///     "actual_amount_in": 995000000,
///     "amount_out": 10500000,
///     "amount_received": 10450000,
///     "minimum_amount_out": 10400000,
///     "price_ratio": 0.0105,
///     "price_impact_percent": 0.15,
///     "trade_fee": 250000,
///     "slippage": 0.5,
///     "pool_info": {
///       "total_token_0_amount": 100000000000,
///       "total_token_1_amount": 1000000000000,
///       "token_0_mint": "So11111111111111111111111111111111111111112",
///       "token_1_mint": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
///       "trade_direction": "ZeroForOne"
///     }
///   }
/// }
/// ```
#[utoipa::path(
    post,
    path = "/api/v1/solana/cpmm/swap/base-in/compute",
    request_body = CpmmSwapBaseInRequest,
    responses(
        (status = 200, description = "计算成功", body = ApiResponse<CpmmSwapBaseInCompute>),
        (status = 400, description = "请求参数错误", body = ApiResponse<ErrorResponse>),
        (status = 500, description = "内部服务器错误", body = ApiResponse<ErrorResponse>)
    ),
    tag = "CPMM交换"
)]
pub async fn compute_swap_base_in(
    Extension(services): Extension<Services>,
    ValidationExtractor(request): ValidationExtractor<CpmmSwapBaseInRequest>,
) -> Result<Json<ApiResponse<CpmmSwapBaseInCompute>>, (StatusCode, Json<ApiResponse<ErrorResponse>>)> {
    info!(
        "💰 收到CPMM SwapBaseIn计算请求: pool_id={}, amount={}",
        request.pool_id, request.user_input_amount
    );

    match services.solana.compute_cpmm_swap_base_in(request).await {
        Ok(compute_result) => {
            info!(
                "✅ CPMM SwapBaseIn计算成功: 输入{} -> 输出{}",
                compute_result.actual_amount_in, compute_result.amount_received
            );
            Ok(Json(ApiResponse::success(compute_result)))
        }
        Err(e) => {
            error!("❌ CPMM SwapBaseIn计算失败: {:?}", e);
            let error_response = ErrorResponse::new("CPMM_COMPUTE_FAILED", &format!("CPMM SwapBaseIn计算失败: {}", e));
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error(error_response)),
            ))
        }
    }
}

/// 构建CPMM SwapBaseIn交易
///
/// 基于计算结果构建交易数据，供客户端签名和发送
///
/// # 请求体
///
/// ```json
/// {
///   "wallet": "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM",
///   "tx_version": "0",
///   "swap_compute": {
///     "pool_id": "8k7F9Xb2wVxeJY4QcLrPz1cDEf3GhJ5mNvRtU6sB2W9A",
///     ...
///   }
/// }
/// ```
///
/// # 响应
///
/// ```json
/// {
///   "success": true,
///   "data": {
///     "transaction": "AAAHdGVzdCBkYXRh...base64交易数据",
///     "transaction_size": 412,
///     "description": "CPMM SwapBaseIn交易"
///   }
/// }
/// ```
#[utoipa::path(
    post,
    path = "/api/v1/solana/cpmm/swap/base-in/transaction",
    request_body = CpmmSwapBaseInTransactionRequest,
    responses(
        (status = 200, description = "交易构建成功", body = ApiResponse<CpmmTransactionData>),
        (status = 400, description = "请求参数错误", body = ApiResponse<ErrorResponse>),
        (status = 500, description = "内部服务器错误", body = ApiResponse<ErrorResponse>)
    ),
    tag = "CPMM交换"
)]
pub async fn build_swap_base_in_transaction(
    Extension(services): Extension<Services>,
    ValidationExtractor(request): ValidationExtractor<CpmmSwapBaseInTransactionRequest>,
) -> Result<Json<ApiResponse<CpmmTransactionData>>, (StatusCode, Json<ApiResponse<ErrorResponse>>)> {
    info!(
        "🔨 收到CPMM SwapBaseIn交易构建请求: wallet={}, pool_id={}",
        request.wallet, request.swap_compute.pool_id
    );

    match services.solana.build_cpmm_swap_base_in_transaction(request).await {
        Ok(transaction_data) => {
            info!(
                "✅ CPMM SwapBaseIn交易构建成功: 大小{}字节",
                transaction_data.transaction_size
            );
            Ok(Json(ApiResponse::success(transaction_data)))
        }
        Err(e) => {
            error!("❌ CPMM SwapBaseIn交易构建失败: {:?}", e);
            let error_response = ErrorResponse::new("CPMM_TRANSACTION_BUILD_FAILED", &format!("CPMM SwapBaseIn交易构建失败: {}", e));
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error(error_response)),
            ))
        }
    }
}