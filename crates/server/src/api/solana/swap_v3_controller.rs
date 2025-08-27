use crate::dtos::solana::common::TransactionData;
use crate::dtos::solana::swap::raydium::RaydiumResponse;
use crate::dtos::solana::swap::swap_v3::{
    ComputeSwapV3Request, SwapComputeV3Data, SwapV3AndSendTransactionResponse, TransactionSwapV3Request,
};
use crate::services::Services;
use axum::{
    extract::{Extension, Json, Query},
    http::StatusCode,
    routing::{get, post},
    Router,
};
use tracing::{error, info};

pub struct SwapV3Controller;

impl SwapV3Controller {
    pub fn routes() -> Router {
        Router::new()
            // ============ SwapV3 API路由（支持推荐系统） ============
            .route("/compute/swap-v3-base-in", get(compute_swap_v3_base_in))
            .route("/compute/swap-v3-base-out", get(compute_swap_v3_base_out))
            .route("/transaction/swap-v3-base-in", post(transaction_swap_v3_base_in))
            .route("/transaction/swap-v3-base-out", post(transaction_swap_v3_base_out))
            // ============ SwapV3 测试API路由（本地签名） ============
            .route(
                "/transaction/swap-v3-base-in-and-send",
                post(test_swap_v3_base_in_and_send),
            )
            .route(
                "/transaction/swap-v3-base-out-and-send",
                post(test_swap_v3_base_out_and_send),
            )
    }
}

/// 计算swap-v3-base-in交换数据
///
/// 基于固定输入金额计算输出金额和交换详情，支持推荐系统和转账费计算（SwapV3 API）
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
/// - referralAccount: 推荐账户地址（可选）
/// - upperAccount: 上级地址（可选）
/// - enableReferralRewards: 是否启用推荐奖励（默认为true）
///
/// # 响应示例
///
/// ```json
/// {
///   "id": "uuid-here",
///   "success": true,
///   "version": "V1",
///   "data": {
///     "swapType": "BaseInV3",
///     "inputMint": "So11111111111111111111111111111111111111112",
///     "inputAmount": "1000000000",
///     "outputMint": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
///     "outputAmount": "1500000",
///     "otherAmountThreshold": "1485000",
///     "slippageBps": 100,
///     "priceImpactPct": 0.02,
///     "referrerAmount": "0",
///     "routePlan": [...],
///     "transferFeeInfo": {
///       "inputTransferFee": 0,
///       "outputTransferFee": 0,
///       "inputMintDecimals": 9,
///       "outputMintDecimals": 6
///     },
///     "amountSpecified": "1000000000",
///     "epoch": 500,
///     "referralInfo": {
///       "upper": "8S2bcP66WehuF6cHryfZ7vfFpQWaUhYyAYSy5U3gX4Fy",
///       "upperUpper": null,
///       "projectAccount": "8S2bcP66WehuF6cHryfZ7vfFpQWaUhYyAYSy5U3gX4Fy",
///       "referralProgram": "REFRpo1ievaQhpSLR8uDwCzjfUDJ8xGsBmNn8J5fC2q",
///       "payerReferral": "9X...ABC",
///       "upperReferral": "7Y...DEF"
///     },
///     "rewardDistribution": {
///       "totalRewardFee": 2500,
///       "projectReward": 1250,
///       "upperReward": 1042,
///       "upperUpperReward": 208,
///       "distributionRatios": {
///         "projectRatio": 50.0,
///         "upperRatio": 41.67,
///         "upperUpperRatio": 8.33
///       }
///     }
///   }
/// }
/// ```
#[utoipa::path(
    get,
    path = "/api/v1/solana/compute/swap-v3-base-in",
    params(ComputeSwapV3Request),
    responses(
        (status = 200, description = "交换计算成功", body = RaydiumResponse<SwapComputeV3Data>),
        (status = 400, description = "请求参数无效", body = RaydiumErrorResponse),
        (status = 500, description = "内部服务器错误", body = RaydiumErrorResponse)
    ),
    tag = "SwapV3"
)]
pub async fn compute_swap_v3_base_in(
    Extension(services): Extension<Services>,
    Query(params): Query<ComputeSwapV3Request>,
) -> Result<Json<RaydiumResponse<SwapComputeV3Data>>, StatusCode> {
    info!(
        "🔄 计算SwapV3 BaseIn交换: {} -> {} (金额: {})",
        params.input_mint, params.output_mint, params.amount
    );

    match services.solana.compute_swap_v3_base_in(params).await {
        Ok(data) => {
            info!("✅ SwapV3 BaseIn计算成功");
            let response = RaydiumResponse::success(data);
            Ok(Json(response))
        }
        Err(e) => {
            error!("❌ SwapV3 BaseIn计算失败: {:?}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// 计算swap-v3-base-out交换数据
///
/// 基于固定输出金额计算所需输入金额和交换详情，支持推荐系统和转账费计算（SwapV3 API）
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
/// - referralAccount: 推荐账户地址（可选）
/// - upperAccount: 上级地址（可选）
/// - enableReferralRewards: 是否启用推荐奖励（默认为true）
///
/// # 响应示例
///
/// ```json
/// {
///   "id": "uuid-here",
///   "success": true,
///   "version": "V1",
///   "data": {
///     "swapType": "BaseOutV3",
///     "inputMint": "So11111111111111111111111111111111111111112",
///     "inputAmount": "1005000000",
///     "outputMint": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
///     "outputAmount": "1500000",
///     "otherAmountThreshold": "1015000000",
///     "slippageBps": 100,
///     "priceImpactPct": 0.02,
///     "referrerAmount": "0",
///     "routePlan": [...],
///     "transferFeeInfo": null,
///     "amountSpecified": "1005000000",
///     "epoch": null,
///     "referralInfo": {
///       "upper": "8S2bcP66WehuF6cHryfZ7vfFpQWaUhYyAYSy5U3gX4Fy",
///       "upperUpper": null,
///       "projectAccount": "8S2bcP66WehuF6cHryfZ7vfFpQWaUhYyAYSy5U3gX4Fy",
///       "referralProgram": "REFRpo1ievaQhpSLR8uDwCzjfUDJ8xGsBmNn8J5fC2q",
///       "payerReferral": "9X...ABC",
///       "upperReferral": null
///     },
///     "rewardDistribution": {
///       "totalRewardFee": 2513,
///       "projectReward": 1256,
///       "upperReward": 1047,
///       "upperUpperReward": 210,
///       "distributionRatios": {
///         "projectRatio": 50.0,
///         "upperRatio": 41.67,
///         "upperUpperRatio": 8.33
///       }
///     }
///   }
/// }
/// ```
#[utoipa::path(
    get,
    path = "/api/v1/solana/compute/swap-v3-base-out",
    params(ComputeSwapV3Request),
    responses(
        (status = 200, description = "交换计算成功", body = RaydiumResponse<SwapComputeV3Data>),
        (status = 400, description = "请求参数无效", body = RaydiumErrorResponse),
        (status = 500, description = "内部服务器错误", body = RaydiumErrorResponse)
    ),
    tag = "SwapV3"
)]
pub async fn compute_swap_v3_base_out(
    Extension(services): Extension<Services>,
    Query(params): Query<ComputeSwapV3Request>,
) -> Result<Json<RaydiumResponse<SwapComputeV3Data>>, StatusCode> {
    info!(
        "🔄 计算SwapV3 BaseOut交换: {} -> {} (期望输出: {})",
        params.input_mint, params.output_mint, params.amount
    );

    match services.solana.compute_swap_v3_base_out(params).await {
        Ok(data) => {
            info!("✅ SwapV3 BaseOut计算成功");
            let response = RaydiumResponse::success(data);
            Ok(Json(response))
        }
        Err(e) => {
            error!("❌ SwapV3 BaseOut计算失败: {:?}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// 构建swap-v3-base-in交易数据
///
/// 为SwapV3 BaseIn交换构建未签名的交易数据，支持推荐系统
///
/// # 请求体
///
/// ```json
/// {
///   "wallet": "8S2bcP66WehuF6cHryfZ7vfFpQWaUhYyAYSy5U3gX4Fy",
///   "computeUnitPriceMicroLamports": "1000000",
///   "swapResponse": {
///     "id": "uuid-here",
///     "success": true,
///     "version": "V1",
///     "data": {
///       "swapType": "BaseInV3",
///       "inputMint": "So11111111111111111111111111111111111111112",
///       "inputAmount": "1000000000",
///       "outputMint": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
///       "outputAmount": "1500000",
///       "otherAmountThreshold": "1485000",
///       "slippageBps": 100,
///       "referralInfo": {...},
///       "rewardDistribution": {...}
///     }
///   },
///   "txVersion": "V0",
///   "wrapSol": true
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
///   "data": {
///     "transaction": "base64-encoded-transaction-data"
///   }
/// }
/// ```
#[utoipa::path(
    post,
    path = "/api/v1/solana/transaction/swap-v3-base-in",
    request_body = TransactionSwapV3Request,
    responses(
        (status = 200, description = "交易构建成功", body = RaydiumResponse<TransactionData>),
        (status = 400, description = "请求参数无效", body = RaydiumErrorResponse),
        (status = 500, description = "内部服务器错误", body = RaydiumErrorResponse)
    ),
    tag = "SwapV3"
)]
pub async fn transaction_swap_v3_base_in(
    Extension(services): Extension<Services>,
    Json(request): Json<TransactionSwapV3Request>,
) -> Result<Json<RaydiumResponse<TransactionData>>, StatusCode> {
    info!("🔨 构建SwapV3 BaseIn交易: 钱包={}", request.wallet);

    match services.solana.build_swap_v3_transaction_base_in(request).await {
        Ok(data) => {
            info!("✅ SwapV3 BaseIn交易构建成功");
            let response = RaydiumResponse::success(data);
            Ok(Json(response))
        }
        Err(e) => {
            error!("❌ SwapV3 BaseIn交易构建失败: {:?}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// 构建swap-v3-base-out交易数据
///
/// 为SwapV3 BaseOut交换构建未签名的交易数据，支持推荐系统
///
/// # 请求体
///
/// ```json
/// {
///   "wallet": "8S2bcP66WehuF6cHryfZ7vfFpQWaUhYyAYSy5U3gX4Fy",
///   "computeUnitPriceMicroLamports": "1000000",
///   "swapResponse": {
///     "id": "uuid-here",
///     "success": true,
///     "version": "V1",
///     "data": {
///       "swapType": "BaseOutV3",
///       "inputMint": "So11111111111111111111111111111111111111112",
///       "inputAmount": "1005000000",
///       "outputMint": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
///       "outputAmount": "1500000",
///       "otherAmountThreshold": "1015000000",
///       "slippageBps": 100,
///       "referralInfo": {...},
///       "rewardDistribution": {...}
///     }
///   },
///   "txVersion": "V0",
///   "wrapSol": true
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
///   "data": {
///     "transaction": "base64-encoded-transaction-data"
///   }
/// }
/// ```
#[utoipa::path(
    post,
    path = "/api/v1/solana/transaction/swap-v3-base-out",
    request_body = TransactionSwapV3Request,
    responses(
        (status = 200, description = "交易构建成功", body = RaydiumResponse<TransactionData>),
        (status = 400, description = "请求参数无效", body = RaydiumErrorResponse),
        (status = 500, description = "内部服务器错误", body = RaydiumErrorResponse)
    ),
    tag = "SwapV3"
)]
pub async fn transaction_swap_v3_base_out(
    Extension(services): Extension<Services>,
    Json(request): Json<TransactionSwapV3Request>,
) -> Result<Json<RaydiumResponse<TransactionData>>, StatusCode> {
    info!("🔨 构建SwapV3 BaseOut交易: 钱包={}", request.wallet);

    match services.solana.build_swap_v3_transaction_base_out(request).await {
        Ok(data) => {
            info!("✅ SwapV3 BaseOut交易构建成功");
            let response = RaydiumResponse::success(data);
            Ok(Json(response))
        }
        Err(e) => {
            error!("❌ SwapV3 BaseOut交易构建失败: {:?}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// 测试SwapV3 BaseIn交换并发送交易（本地签名）
///
/// 构建SwapV3 BaseIn交换交易并使用配置的私钥签名发送到链上（仅用于本地测试）
///
/// # 请求体
///
/// ```json
/// {
///   "wallet": "8S2bcP66WehuF6cHryfZ7vfFpQWaUhYyAYSy5U3gX4Fy",
///   "computeUnitPriceMicroLamports": "1000000",
///   "swapResponse": {
///     "id": "uuid-here",
///     "success": true,
///     "version": "V1",
///     "data": {
///       "swapType": "BaseInV3",
///       "inputMint": "So11111111111111111111111111111111111111112",
///       "inputAmount": "1000000000",
///       "outputMint": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
///       "outputAmount": "1500000",
///       "otherAmountThreshold": "1485000",
///       "slippageBps": 100,
///       "referralInfo": {...},
///       "rewardDistribution": {...}
///     }
///   },
///   "txVersion": "V0",
///   "wrapSol": true
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
///   "data": {
///     "signature": "5VB...ABC123",
///     "userWallet": "8S2bcP66WehuF6cHryfZ7vfFpQWaUhYyAYSy5U3gX4Fy",
///     "inputMint": "So11111111111111111111111111111111111111112",
///     "outputMint": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
///     "inputAmount": "1000000000",
///     "outputAmount": "1500000",
///     "minimumAmountOut": "1485000",
///     "poolAddress": "8XY...DEF456",
///     "referralInfo": {...},
///     "status": "Confirmed",
///     "explorerUrl": "https://explorer.solana.com/tx/5VB...ABC123",
///     "timestamp": 1641234567
///   }
/// }
/// ```
#[utoipa::path(
    post,
    path = "/api/v1/solana/transaction/swap-v3-base-in-and-send",
    request_body = TransactionSwapV3Request,
    responses(
        (status = 200, description = "SwapV3交换交易成功", body = RaydiumResponse<SwapV3AndSendTransactionResponse>),
        (status = 400, description = "请求参数无效", body = RaydiumErrorResponse),
        (status = 500, description = "内部服务器错误", body = RaydiumErrorResponse)
    ),
    tag = "SwapV3-Test"
)]
pub async fn test_swap_v3_base_in_and_send(
    Extension(services): Extension<Services>,
    Json(request): Json<TransactionSwapV3Request>,
) -> Result<Json<RaydiumResponse<SwapV3AndSendTransactionResponse>>, StatusCode> {
    info!("🧪 测试SwapV3 BaseIn交换并发送: 钱包={}", request.wallet);

    match services
        .solana
        .build_and_send_transaction_swap_v3_transaction_base_in(request)
        .await
    {
        Ok(data) => {
            info!("✅ SwapV3 BaseIn测试交易成功，签名: {}", data.signature);
            let response = RaydiumResponse::success(data);
            Ok(Json(response))
        }
        Err(e) => {
            error!("❌ SwapV3 BaseIn测试交易失败: {:?}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// 测试SwapV3 BaseOut交换并发送交易（本地签名）
///
/// 构建SwapV3 BaseOut交换交易并使用配置的私钥签名发送到链上（仅用于本地测试）
///
/// # 请求体
///
/// ```json
/// {
///   "wallet": "8S2bcP66WehuF6cHryfZ7vfFpQWaUhYyAYSy5U3gX4Fy",
///   "computeUnitPriceMicroLamports": "1000000",
///   "swapResponse": {
///     "id": "uuid-here",
///     "success": true,
///     "version": "V1",
///     "data": {
///       "swapType": "BaseOutV3",
///       "inputMint": "So11111111111111111111111111111111111111112",
///       "inputAmount": "1005000000",
///       "outputMint": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
///       "outputAmount": "1500000",
///       "otherAmountThreshold": "1015000000",
///       "slippageBps": 100,
///       "referralInfo": {...},
///       "rewardDistribution": {...}
///     }
///   },
///   "txVersion": "V0",
///   "wrapSol": true
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
///   "data": {
///     "signature": "5VB...ABC123",
///     "userWallet": "8S2bcP66WehuF6cHryfZ7vfFpQWaUhYyAYSy5U3gX4Fy",
///     "inputMint": "So11111111111111111111111111111111111111112",
///     "outputMint": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
///     "inputAmount": "1015000000",
///     "outputAmount": "1500000",
///     "minimumAmountOut": "1500000",
///     "poolAddress": "8XY...DEF456",
///     "referralInfo": {...},
///     "status": "Confirmed",
///     "explorerUrl": "https://explorer.solana.com/tx/5VB...ABC123",
///     "timestamp": 1641234567
///   }
/// }
/// ```
#[utoipa::path(
    post,
    path = "/api/v1/solana/transaction/swap-v3-base-out-and-send",
    request_body = TransactionSwapV3Request,
    responses(
        (status = 200, description = "SwapV3交换交易成功", body = RaydiumResponse<SwapV3AndSendTransactionResponse>),
        (status = 400, description = "请求参数无效", body = RaydiumErrorResponse),
        (status = 500, description = "内部服务器错误", body = RaydiumErrorResponse)
    ),
    tag = "SwapV3-Test"
)]
pub async fn test_swap_v3_base_out_and_send(
    Extension(services): Extension<Services>,
    Json(request): Json<TransactionSwapV3Request>,
) -> Result<Json<RaydiumResponse<SwapV3AndSendTransactionResponse>>, StatusCode> {
    info!("🧪 测试SwapV3 BaseOut交换并发送: 钱包={}", request.wallet);

    match services
        .solana
        .build_and_send_transaction_swap_v3_transaction_base_out(request)
        .await
    {
        Ok(data) => {
            info!("✅ SwapV3 BaseOut测试交易成功，签名: {}", data.signature);
            let response = RaydiumResponse::success(data);
            Ok(Json(response))
        }
        Err(e) => {
            error!("❌ SwapV3 BaseOut测试交易失败: {:?}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}
