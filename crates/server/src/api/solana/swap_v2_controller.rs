use crate::{
    dtos::solana_dto::{ComputeSwapV2Request, RaydiumErrorResponse, RaydiumResponse, SwapComputeV2Data, TransactionData, TransactionSwapV2Request},
    extractors::validation_extractor::ValidationExtractor,
    services::Services,
};
use axum::{
    extract::{Extension, Query},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use tracing::{error, info};

pub struct SwapV2Controller;

impl SwapV2Controller {
    pub fn routes() -> Router {
        Router::new()
            // ============ SwapV2 API兼容路由（支持转账费） ============
            .route("/compute/swap-base-in", get(compute_swap_v2_base_in))
            .route("/compute/swap-base-out", get(compute_swap_v2_base_out))
            .route("/transaction/swap-base-in", post(transaction_swap_v2_base_in))
            .route("/transaction/swap-base-out", post(transaction_swap_v2_base_out))
    }
}

/// 计算swap-base-in交换数据
///
/// 基于固定输入金额计算输出金额和交换详情，支持转账费计算（SwapV2 API兼容）
///
/// # 查询参数
///
/// - inputMint: 输入代币mint地址
/// - outputMint: 输出代币mint地址
/// - amount: 输入金额（字符串形式的最小单位）
/// - slippageBps: 滑点容忍度（基点）
/// - limitPrice: 限价（可选）
/// - enableTransferFee: 是否启用转账费计算（默认为true）
/// - txVersion: 交易版本（V0或V1）
///
/// # 响应示例
///
/// ```json
/// {
///   "id": "uuid-here",
///   "success": true,
///   "version": "V1",
///   "data": {
///     "swapType": "BaseInV2",
///     "inputMint": "So11111111111111111111111111111111111111112",
///     "inputAmount": "1000000000",
///     "outputMint": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
///     "outputAmount": "100000000",
///     "otherAmountThreshold": "95000000",
///     "slippageBps": 50,
///     "priceImpactPct": 0.1,
///     "referrerAmount": "0",
///     "routePlan": [...],
///     "transferFeeInfo": {
///       "inputTransferFee": 5000,
///       "outputTransferFee": 0,
///       "inputMintDecimals": 9,
///       "outputMintDecimals": 6
///     },
///     "amountSpecified": "995000000",
///     "epoch": 543
///   }
/// }
/// ```
#[utoipa::path(
    get,
    path = "/api/v1/solana/compute/swap-base-in",
    params(
        ("inputMint" = String, Query, description = "输入代币mint地址"),
        ("outputMint" = String, Query, description = "输出代币mint地址"),
        ("amount" = String, Query, description = "输入金额"),
        ("slippageBps" = u16, Query, description = "滑点容忍度（基点）"),
        ("limitPrice" = Option<f64>, Query, description = "限价（可选）"),
        ("enableTransferFee" = Option<bool>, Query, description = "是否启用转账费计算"),
        ("txVersion" = String, Query, description = "交易版本")
    ),
    responses(
        (status = 200, description = "计算成功", body = RaydiumResponse<SwapComputeV2Data>),
        (status = 400, description = "参数错误", body = RaydiumErrorResponse),
        (status = 500, description = "计算失败", body = RaydiumErrorResponse)
    ),
    tag = "SwapV2兼容接口"
)]
pub async fn compute_swap_v2_base_in(
    Extension(services): Extension<Services>,
    Query(params): Query<ComputeSwapV2Request>,
) -> Result<Json<RaydiumResponse<SwapComputeV2Data>>, (StatusCode, Json<RaydiumErrorResponse>)> {
    info!(
        "📊 计算swap-base-in: {} {} -> {} (转账费: {:?})",
        params.amount, params.input_mint, params.output_mint, params.enable_transfer_fee
    );

    match services.solana.compute_swap_v2_base_in(params).await {
        Ok(compute_data) => {
            info!("✅ swap-base-in计算成功");
            Ok(Json(RaydiumResponse::success(compute_data)))
        }
        Err(e) => {
            error!("❌ swap-base-in计算失败: {:?}", e);
            let error_response = RaydiumErrorResponse::new(&format!("计算失败: {}", e));
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error_response)))
        }
    }
}

/// 计算swap-base-out交换数据
///
/// 基于固定输出金额计算所需输入金额和交换详情，支持转账费计算（SwapV2 API兼容）
///
/// # 查询参数
///
/// - inputMint: 输入代币mint地址
/// - outputMint: 输出代币mint地址
/// - amount: 期望输出金额（字符串形式的最小单位）
/// - slippageBps: 滑点容忍度（基点）
/// - limitPrice: 限价（可选）
/// - enableTransferFee: 是否启用转账费计算（默认为true）
/// - txVersion: 交易版本（V0或V1）
///
/// # 响应示例
///
/// ```json
/// {
///   "id": "uuid-here",
///   "success": true,
///   "version": "V1",
///   "data": {
///     "swapType": "BaseOutV2",
///     "inputMint": "So11111111111111111111111111111111111111112",
///     "inputAmount": "1050000000",
///     "outputMint": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
///     "outputAmount": "100000000",
///     "otherAmountThreshold": "1107500000",
///     "slippageBps": 50,
///     "priceImpactPct": 0.1,
///     "referrerAmount": "0",
///     "routePlan": [...],
///     "transferFeeInfo": {
///       "inputTransferFee": 5250,
///       "outputTransferFee": 0,
///       "inputMintDecimals": 9,
///       "outputMintDecimals": 6
///     },
///     "amountSpecified": "1050000000",
///     "epoch": 543
///   }
/// }
/// ```
#[utoipa::path(
    get,
    path = "/api/v1/solana/compute/swap-base-out",
    params(
        ("inputMint" = String, Query, description = "输入代币mint地址"),
        ("outputMint" = String, Query, description = "输出代币mint地址"),
        ("amount" = String, Query, description = "期望输出金额"),
        ("slippageBps" = u16, Query, description = "滑点容忍度（基点）"),
        ("limitPrice" = Option<f64>, Query, description = "限价（可选）"),
        ("enableTransferFee" = Option<bool>, Query, description = "是否启用转账费计算"),
        ("txVersion" = String, Query, description = "交易版本")
    ),
    responses(
        (status = 200, description = "计算成功", body = RaydiumResponse<SwapComputeV2Data>),
        (status = 400, description = "参数错误", body = RaydiumErrorResponse),
        (status = 500, description = "计算失败", body = RaydiumErrorResponse)
    ),
    tag = "SwapV2兼容接口"
)]
pub async fn compute_swap_v2_base_out(
    Extension(services): Extension<Services>,
    Query(params): Query<ComputeSwapV2Request>,
) -> Result<Json<RaydiumResponse<SwapComputeV2Data>>, (StatusCode, Json<RaydiumErrorResponse>)> {
    info!(
        "📊 计算swap-base-out: {} {} -> {} (转账费: {:?})",
        params.amount, params.input_mint, params.output_mint, params.enable_transfer_fee
    );

    match services.solana.compute_swap_v2_base_out(params).await {
        Ok(compute_data) => {
            info!("✅ swap-base-out计算成功");
            Ok(Json(RaydiumResponse::success(compute_data)))
        }
        Err(e) => {
            error!("❌ swap-base-out计算失败: {:?}", e);
            let error_response = RaydiumErrorResponse::new(&format!("计算失败: {}", e));
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error_response)))
        }
    }
}

/// 构建swap-base-in交易
///
/// 使用compute-v2接口的结果构建可执行的交易数据，支持转账费（SwapV2 API兼容）
///
/// # 请求体
///
/// ```json
/// {
///   "wallet": "用户钱包地址",
///   "computeUnitPriceMicroLamports": "15000",
///   "swapResponse": { /* compute-v2接口的完整响应 */ },
///   "txVersion": "V0",
///   "wrapSol": false,
///   "unwrapSol": false,
///   "inputAccount": "输入代币账户地址（可选）",
///   "outputAccount": "输出代币账户地址（可选）"
/// }
/// ```
///
/// # 响应示例
///
/// ```json
/// {
///   "id": "uuid-here",
///   "success": true,
///   "version": "V1",
///   "data": [
///     {
///       "transaction": "Base64编码的序列化交易数据"
///     }
///   ]
/// }
/// ```
#[utoipa::path(
    post,
    path = "/api/v1/solana/transaction/swap-base-in",
    request_body = TransactionSwapV2Request,
    responses(
        (status = 200, description = "交易构建成功", body = RaydiumResponse<Vec<TransactionData>>),
        (status = 400, description = "参数错误", body = RaydiumErrorResponse),
        (status = 500, description = "交易构建失败", body = RaydiumErrorResponse)
    ),
    tag = "SwapV2兼容接口"
)]
pub async fn transaction_swap_v2_base_in(
    Extension(services): Extension<Services>,
    ValidationExtractor(request): ValidationExtractor<TransactionSwapV2Request>,
) -> Result<Json<RaydiumResponse<Vec<TransactionData>>>, (StatusCode, Json<RaydiumErrorResponse>)> {
    info!("🔨 构建swap-base-in交易，钱包: {}", request.wallet);

    match services.solana.build_swap_v2_transaction_base_in(request).await {
        Ok(transaction_data) => {
            info!("✅ swap-base-in交易构建成功");
            Ok(Json(RaydiumResponse::success(vec![transaction_data])))
        }
        Err(e) => {
            error!("❌ swap-base-in交易构建失败: {:?}", e);
            let error_response = RaydiumErrorResponse::new(&format!("交易构建失败: {}", e));
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error_response)))
        }
    }
}

/// 构建swap-base-out交易
///
/// 使用compute-v2接口的结果构建可执行的交易数据，支持转账费（SwapV2 API兼容）
///
/// # 请求体
///
/// ```json
/// {
///   "wallet": "用户钱包地址",
///   "computeUnitPriceMicroLamports": "15000",
///   "swapResponse": { /* compute-v2接口的完整响应 */ },
///   "txVersion": "V0",
///   "wrapSol": false,
///   "unwrapSol": false,
///   "inputAccount": "输入代币账户地址（可选）",
///   "outputAccount": "输出代币账户地址（可选）"
/// }
/// ```
///
/// # 响应示例
///
/// ```json
/// {
///   "id": "uuid-here",
///   "success": true,
///   "version": "V1",
///   "data": [
///     {
///       "transaction": "Base64编码的序列化交易数据"
///     }
///   ]
/// }
/// ```
#[utoipa::path(
    post,
    path = "/api/v1/solana/transaction/swap-base-out",
    request_body = TransactionSwapV2Request,
    responses(
        (status = 200, description = "交易构建成功", body = RaydiumResponse<Vec<TransactionData>>),
        (status = 400, description = "参数错误", body = RaydiumErrorResponse),
        (status = 500, description = "交易构建失败", body = RaydiumErrorResponse)
    ),
    tag = "SwapV2兼容接口"
)]
pub async fn transaction_swap_v2_base_out(
    Extension(services): Extension<Services>,
    ValidationExtractor(request): ValidationExtractor<TransactionSwapV2Request>,
) -> Result<Json<RaydiumResponse<Vec<TransactionData>>>, (StatusCode, Json<RaydiumErrorResponse>)> {
    info!("🔨 构建swap-base-out交易，钱包: {}", request.wallet);

    match services.solana.build_swap_v2_transaction_base_out(request).await {
        Ok(transaction_data) => {
            info!("✅ swap-base-out交易构建成功");
            Ok(Json(RaydiumResponse::success(vec![transaction_data])))
        }
        Err(e) => {
            error!("❌ swap-base-out交易构建失败: {:?}", e);
            let error_response = RaydiumErrorResponse::new(&format!("交易构建失败: {}", e));
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error_response)))
        }
    }
}