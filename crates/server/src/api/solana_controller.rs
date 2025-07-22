use std::collections::HashMap;

use crate::{
    dtos::solana_dto::{
        ApiResponse, BalanceResponse, CalculateLiquidityRequest, CalculateLiquidityResponse, ComputeSwapV2Request, CreatePoolAndSendTransactionResponse,
        CreatePoolRequest, CreatePoolResponse, ErrorResponse, GetUserPositionsRequest, OpenPositionAndSendTransactionResponse, OpenPositionRequest,
        OpenPositionResponse, PositionInfo, PriceQuoteRequest, PriceQuoteResponse, RaydiumErrorResponse, RaydiumResponse, SwapComputeV2Data, SwapRequest,
        SwapResponse, TransactionData, TransactionSwapV2Request, UserPositionsResponse, WalletInfo,
    },
    extractors::validation_extractor::ValidationExtractor,
    services::Services,
};
use axum::{
    extract::Query,
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Extension, Router,
};
use tracing::{error, info, warn};

pub struct SolanaController;

impl SolanaController {
    pub fn app() -> Router {
        Router::new()
            .route("/swap", post(swap_tokens))
            .route("/balance", get(get_balance))
            .route("/quote", post(get_price_quote))
            .route("/wallet", get(get_wallet_info))
            .route("/health", get(health_check))
            // ============ SwapV2 API兼容路由（支持转账费） ============
            .route("/compute/swap-base-in", get(compute_swap_v2_base_in))
            .route("/compute/swap-base-out", get(compute_swap_v2_base_out))
            .route("/transaction/swap-base-in", post(transaction_swap_v2_base_in))
            .route("/transaction/swap-base-out", post(transaction_swap_v2_base_out))
            // ============ OpenPosition API路由 ============
            .route("/position/open", post(open_position))
            // 开仓并发送交易, 用户本地测试使用，本地签名并发送交易
            .route("/position/open-and-send-transaction", post(open_position_and_send_transaction))
            .route("/position/calculate", post(calculate_liquidity))
            .route("/position/list", get(get_user_positions))
            .route("/position/info", get(get_position_info))
            .route("/position/check", get(check_position_exists))
            // ============ CreatePool API路由 ============
            .route("/pool/create", post(create_pool))
            .route("/pool/create-and-send-transaction", post(create_pool_and_send_transaction))
    }
}

/// 执行代币交换
///
/// 支持SOL和USDC之间的双向交换，基于Raydium AMM协议
///
/// # 请求体
///
/// ```json
/// {
///   "from_token": "SOL",
///   "to_token": "USDC",
///   "amount": 1000000000,
///   "minimum_amount_out": 95000000,
///   "max_slippage_percent": 5.0
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
///     "from_token": "SOL",
///     "to_token": "USDC",
///     "amount_in": 1000000000,
///     "amount_out_expected": 100000000,
///     "status": "Pending",
///     "explorer_url": "https://explorer.solana.com/tx/5VfYe...",
///     "timestamp": 1678901234
///   }
/// }
/// ```
#[utoipa::path(
    post,
    path = "/api/v1/solana/swap",
    request_body = SwapRequest,
    responses(
        (status = 200, description = "交换成功", body = ApiResponse<SwapResponse>),
        (status = 400, description = "请求参数错误", body = ApiResponse<ErrorResponse>),
        (status = 500, description = "内部服务器错误", body = ApiResponse<ErrorResponse>)
    ),
    tag = "Solana交换"
)]
pub async fn swap_tokens(
    Extension(services): Extension<Services>,
    ValidationExtractor(request): ValidationExtractor<SwapRequest>,
) -> Result<Json<ApiResponse<SwapResponse>>, (StatusCode, Json<ApiResponse<ErrorResponse>>)> {
    info!("🔄 收到交换请求: {} {} -> {}", request.amount, request.from_token, request.to_token);

    match services.solana.swap_tokens(request).await {
        Ok(response) => {
            info!("✅ 交换成功，交易签名: {}", response.signature);
            Ok(Json(ApiResponse::success(response)))
        }
        Err(e) => {
            error!("❌ 交换失败: {:?}", e);
            let error_response = ErrorResponse::new("SWAP_FAILED", &format!("交换失败: {}", e));
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error(error_response))))
        }
    }
}

/// 获取账户余额
///
/// 返回当前钱包的SOL和USDC余额信息
///
/// # 响应
///
/// ```json
/// {
///   "success": true,
///   "data": {
///     "sol_balance_lamports": 2000000000,
///     "sol_balance": 2.0,
///     "usdc_balance_micro": 100000000,
///     "usdc_balance": 100.0,
///     "wallet_address": "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM",
///     "timestamp": 1678901234
///   }
/// }
/// ```
#[utoipa::path(
    get,
    path = "/api/v1/solana/balance",
    responses(
        (status = 200, description = "余额查询成功", body = ApiResponse<BalanceResponse>),
        (status = 500, description = "内部服务器错误", body = ApiResponse<ErrorResponse>)
    ),
    tag = "Solana交换"
)]
pub async fn get_balance(
    Extension(services): Extension<Services>,
) -> Result<Json<ApiResponse<BalanceResponse>>, (StatusCode, Json<ApiResponse<ErrorResponse>>)> {
    info!("📊 查询账户余额");

    match services.solana.get_balance().await {
        Ok(balance) => {
            info!("✅ 余额查询成功: SOL {:.6}, USDC {:.2}", balance.sol_balance, balance.usdc_balance);
            Ok(Json(ApiResponse::success(balance)))
        }
        Err(e) => {
            error!("❌ 余额查询失败: {:?}", e);
            let error_response = ErrorResponse::new("BALANCE_QUERY_FAILED", &format!("余额查询失败: {}", e));
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error(error_response))))
        }
    }
}

/// 获取价格报价
///
/// 获取指定金额的代币交换价格估算，不执行实际交换
///
/// # 请求体
///
/// ```json
/// {
///   "from_token": "SOL",
///   "to_token": "USDC",
///   "amount": 1000000000
/// }
/// ```
///
/// # 响应
///
/// ```json
/// {
///   "success": true,
///   "data": {
///     "from_token": "SOL",
///     "to_token": "USDC",
///     "amount_in": 1000000000,
///     "amount_out": 100000000,
///     "price": 0.1,
///     "price_impact_percent": 0.3,
///     "minimum_amount_out": 95000000,
///     "timestamp": 1678901234
///   }
/// }
/// ```
#[utoipa::path(
    post,
    path = "/api/v1/solana/quote",
    request_body = PriceQuoteRequest,
    responses(
        (status = 200, description = "价格查询成功", body = ApiResponse<PriceQuoteResponse>),
        (status = 400, description = "请求参数错误", body = ApiResponse<ErrorResponse>),
        (status = 500, description = "内部服务器错误", body = ApiResponse<ErrorResponse>)
    ),
    tag = "Solana交换"
)]
pub async fn get_price_quote(
    Extension(services): Extension<Services>,
    ValidationExtractor(request): ValidationExtractor<PriceQuoteRequest>,
) -> Result<Json<ApiResponse<PriceQuoteResponse>>, (StatusCode, Json<ApiResponse<ErrorResponse>>)> {
    info!("💰 获取价格报价: {} {} -> {}", request.amount, request.from_token, request.to_token);

    match services.solana.get_price_quote(request).await {
        Ok(quote) => {
            info!("✅ 价格查询成功: {} -> {}, 价格: {:.6}", quote.from_token, quote.to_token, quote.price);
            Ok(Json(ApiResponse::success(quote)))
        }
        Err(e) => {
            error!("❌ 价格查询失败: {:?}", e);
            let error_response = ErrorResponse::new("QUOTE_FAILED", &format!("价格查询失败: {}", e));
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error(error_response))))
        }
    }
}

/// 获取钱包信息
///
/// 返回当前配置的钱包基本信息
///
/// # 响应
///
/// ```json
/// {
///   "success": true,
///   "data": {
///     "address": "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM",
///     "network": "https://api.mainnet-beta.solana.com",
///     "connected": true
///   }
/// }
/// ```
#[utoipa::path(
    get,
    path = "/api/v1/solana/wallet",
    responses(
        (status = 200, description = "钱包信息查询成功", body = ApiResponse<WalletInfo>),
        (status = 500, description = "内部服务器错误", body = ApiResponse<ErrorResponse>)
    ),
    tag = "Solana交换"
)]
pub async fn get_wallet_info(
    Extension(services): Extension<Services>,
) -> Result<Json<ApiResponse<WalletInfo>>, (StatusCode, Json<ApiResponse<ErrorResponse>>)> {
    info!("🔍 查询钱包信息");

    match services.solana.get_wallet_info().await {
        Ok(wallet_info) => {
            info!(
                "✅ 钱包信息查询成功: {} ({})",
                wallet_info.address,
                if wallet_info.connected { "已连接" } else { "未连接" }
            );
            Ok(Json(ApiResponse::success(wallet_info)))
        }
        Err(e) => {
            error!("❌ 钱包信息查询失败: {:?}", e);
            let error_response = ErrorResponse::new("WALLET_INFO_FAILED", &format!("钱包信息查询失败: {}", e));
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error(error_response))))
        }
    }
}

/// 健康检查
///
/// 检查Solana服务的运行状态
///
/// # 响应
///
/// ```json
/// {
///   "success": true,
///   "data": "Solana服务运行正常"
/// }
/// ```
#[utoipa::path(
    get,
    path = "/api/v1/solana/health",
    responses(
        (status = 200, description = "服务正常", body = ApiResponse<String>),
        (status = 500, description = "服务异常", body = ApiResponse<ErrorResponse>)
    ),
    tag = "Solana交换"
)]
pub async fn health_check(Extension(services): Extension<Services>) -> Result<Json<ApiResponse<String>>, (StatusCode, Json<ApiResponse<ErrorResponse>>)> {
    match services.solana.health_check().await {
        Ok(status) => {
            info!("✅ Solana服务健康检查: {}", status);
            Ok(Json(ApiResponse::success(status)))
        }
        Err(e) => {
            error!("❌ Solana服务健康检查失败: {:?}", e);
            let error_response = ErrorResponse::new("HEALTH_CHECK_FAILED", &format!("健康检查失败: {}", e));
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error(error_response))))
        }
    }
}

// ============ SwapV2 API兼容接口（支持转账费） ============

/// 计算swap-v2-base-in交换数据
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
    path = "/api/v1/solana/compute/swap-v2-base-in",
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
        "📊 计算swap-v2-base-in: {} {} -> {} (转账费: {:?})",
        params.amount, params.input_mint, params.output_mint, params.enable_transfer_fee
    );

    match services.solana.compute_swap_v2_base_in(params).await {
        Ok(compute_data) => {
            info!("✅ swap-v2-base-in计算成功");
            Ok(Json(RaydiumResponse::success(compute_data)))
        }
        Err(e) => {
            error!("❌ swap-v2-base-in计算失败: {:?}", e);
            let error_response = RaydiumErrorResponse::new(&format!("计算失败: {}", e));
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error_response)))
        }
    }
}

/// 计算swap-v2-base-out交换数据
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
    path = "/api/v1/solana/compute/swap-v2-base-out",
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
        "📊 计算swap-v2-base-out: {} {} -> {} (转账费: {:?})",
        params.amount, params.input_mint, params.output_mint, params.enable_transfer_fee
    );

    match services.solana.compute_swap_v2_base_out(params).await {
        Ok(compute_data) => {
            info!("✅ swap-v2-base-out计算成功");
            Ok(Json(RaydiumResponse::success(compute_data)))
        }
        Err(e) => {
            error!("❌ swap-v2-base-out计算失败: {:?}", e);
            let error_response = RaydiumErrorResponse::new(&format!("计算失败: {}", e));
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error_response)))
        }
    }
}

/// 构建swap-v2-base-in交易
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
    path = "/api/v1/solana/transaction/swap-v2-base-in",
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
    info!("🔨 构建swap-v2-base-in交易，钱包: {}", request.wallet);

    match services.solana.build_swap_v2_transaction_base_in(request).await {
        Ok(transaction_data) => {
            info!("✅ swap-v2-base-in交易构建成功");
            Ok(Json(RaydiumResponse::success(vec![transaction_data])))
        }
        Err(e) => {
            error!("❌ swap-v2-base-in交易构建失败: {:?}", e);
            let error_response = RaydiumErrorResponse::new(&format!("交易构建失败: {}", e));
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error_response)))
        }
    }
}

/// 构建swap-v2-base-out交易
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
    path = "/api/v1/solana/transaction/swap-v2-base-out",
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
    info!("🔨 构建swap-v2-base-out交易，钱包: {}", request.wallet);

    match services.solana.build_swap_v2_transaction_base_out(request).await {
        Ok(transaction_data) => {
            info!("✅ swap-v2-base-out交易构建成功");
            Ok(Json(RaydiumResponse::success(vec![transaction_data])))
        }
        Err(e) => {
            error!("❌ swap-v2-base-out交易构建失败: {:?}", e);
            let error_response = RaydiumErrorResponse::new(&format!("交易构建失败: {}", e));
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error_response)))
        }
    }
}

// ============ OpenPosition API处理函数 ============

/// 开仓（创建流动性仓位）
///
/// 在指定的池子中创建新的流动性仓位，提供流动性以获取手续费收益。
///
/// # 请求体
///
/// ```json
/// {
///   "pool_address": "池子地址",
///   "tick_lower_price": 1.2,
///   "tick_upper_price": 1.8,
///   "is_base_0": true,
///   "input_amount": 1000000,
///   "with_metadata": false,
///   "max_slippage_percent": 0.5
/// }
/// ```
///
/// # 响应示例
///
/// ```json
/// {
///   "signature": "交易签名",
///   "position_nft_mint": "仓位NFT地址",
///   "position_key": "仓位键值",
///   "tick_lower_index": -1000,
///   "tick_upper_index": 1000,
///   "liquidity": "123456789",
///   "amount_0": 1000000,
///   "amount_1": 500000,
///   "pool_address": "池子地址",
///   "status": "Success",
///   "explorer_url": "https://explorer.solana.com/tx/...",
///   "timestamp": 1640995200
/// }
/// ```
#[utoipa::path(
    post,
    path = "/api/v1/solana/position/open",
    request_body = OpenPositionRequest,
    responses(
        (status = 200, description = "开仓成功", body = OpenPositionResponse),
        (status = 400, description = "请求参数错误", body = ErrorResponse),
        (status = 500, description = "服务器内部错误", body = ErrorResponse)
    ),
    tag = "Solana流动性"
)]
async fn open_position(
    Extension(services): Extension<Services>,
    ValidationExtractor(request): ValidationExtractor<OpenPositionRequest>,
) -> Result<Json<OpenPositionResponse>, (StatusCode, Json<ErrorResponse>)> {
    info!("🎯 接收到开仓请求");
    info!("  池子地址: {}", request.pool_address);
    info!("  用户钱包: {}", request.user_wallet);
    info!("  价格范围: {} - {}", request.tick_lower_price, request.tick_upper_price);
    info!("  输入金额: {}", request.input_amount);

    // check if tick_lower_price is less than tick_upper_price
    if request.tick_lower_price >= request.tick_upper_price {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse::new("TICK_PRICE_ERROR", "tick_lower_price must be less than tick_upper_price")),
        ));
    }

    match services.solana.open_position(request).await {
        Ok(response) => {
            info!("✅ 开仓交易构建成功: {}", response.transaction_message);
            Ok(Json(response))
        }
        Err(e) => {
            error!("❌ 开仓失败: {:?}", e);

            // 检查是否是重复仓位错误
            let error_msg = e.to_string();
            if error_msg.contains("相同价格范围的仓位已存在") {
                warn!("🔄 检测到重复仓位创建尝试");
                let error_response = ErrorResponse::new("POSITION_ALREADY_EXISTS", "相同价格范围的仓位已存在，请检查您的现有仓位或稍后重试");
                Err((StatusCode::CONFLICT, Json(error_response)))
            } else {
                let error_response = ErrorResponse::new("OPEN_POSITION_ERROR", &format!("开仓失败: {}", e));
                Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error_response)))
            }
        }
    }
}

/// 开仓并发送交易（创建流动性位置）
///
/// 在指定的池子中创建新的流动性位置，提供流动性以获取手续费收益。
///
/// # 请求体
///
/// ```json
/// {
///   "pool_address": "池子地址",
///   "tick_lower_price": 1.2,
///   "tick_upper_price": 1.8,
///   "is_base_0": true,
///   "input_amount": 1000000,
///   "with_metadata": false,
///   "max_slippage_percent": 0.5
/// }
/// ```
///
/// # 响应示例
///
/// ```json
/// {
///   "signature": "交易签名",
///   "position_nft_mint": "位置NFT地址",
///   "position_key": "位置键值",
///   "tick_lower_index": -1000,
///   "tick_upper_index": 1000,
///   "liquidity": "123456789",
///   "amount_0": 1000000,
///   "amount_1": 500000,
///   "pool_address": "池子地址",
///   "status": "Success",
///   "explorer_url": "https://explorer.solana.com/tx/...",
///   "timestamp": 1640995200
/// }
/// ```
#[utoipa::path(
    post,
    path = "/api/v1/solana/position/open-and-send-transaction",
    request_body = OpenPositionRequest,
    responses(
        (status = 200, description = "开仓成功", body = OpenPositionResponse),
        (status = 400, description = "请求参数错误", body = ErrorResponse),
        (status = 500, description = "服务器内部错误", body = ErrorResponse)
    ),
    tag = "Solana流动性"
)]
async fn open_position_and_send_transaction(
    Extension(services): Extension<Services>,
    ValidationExtractor(request): ValidationExtractor<OpenPositionRequest>,
) -> Result<Json<OpenPositionAndSendTransactionResponse>, (StatusCode, Json<ErrorResponse>)> {
    info!("🎯 接收到开仓请求");
    info!("  池子地址: {}", request.pool_address);
    info!("  价格范围: {} - {}", request.tick_lower_price, request.tick_upper_price);

    match services.solana.open_position_and_send_transaction(request).await {
        Ok(response) => {
            info!("✅ 开仓成功: {}", response.signature);
            Ok(Json(response))
        }
        Err(e) => {
            error!("❌ 开仓失败: {:?}", e);
            let error_response = ErrorResponse::new("OPEN_POSITION_ERROR", &format!("开仓失败: {}", e));
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error_response)))
        }
    }
}

/// 计算流动性参数
///
/// 根据价格范围和输入金额计算所需的流动性和代币数量。
///
/// # 请求体
///
/// ```json
/// {
///   "pool_address": "池子地址",
///   "tick_lower_price": 1.2,
///   "tick_upper_price": 1.8,
///   "is_base_0": true,
///   "input_amount": 1000000
/// }
/// ```
#[utoipa::path(
    post,
    path = "/api/v1/solana/position/calculate",
    request_body = CalculateLiquidityRequest,
    responses(
        (status = 200, description = "计算成功", body = CalculateLiquidityResponse),
        (status = 400, description = "请求参数错误", body = ErrorResponse),
        (status = 500, description = "服务器内部错误", body = ErrorResponse)
    ),
    tag = "Solana流动性"
)]
async fn calculate_liquidity(
    Extension(services): Extension<Services>,
    ValidationExtractor(request): ValidationExtractor<CalculateLiquidityRequest>,
) -> Result<Json<CalculateLiquidityResponse>, (StatusCode, Json<ErrorResponse>)> {
    info!("🧮 接收到流动性计算请求");

    match services.solana.calculate_liquidity(request).await {
        Ok(response) => {
            info!("✅ 流动性计算成功");
            Ok(Json(response))
        }
        Err(e) => {
            error!("❌ 流动性计算失败: {:?}", e);
            let error_response = ErrorResponse::new("CALCULATE_LIQUIDITY_ERROR", &format!("流动性计算失败: {}", e));
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error_response)))
        }
    }
}

/// 获取用户仓位列表
///
/// 查询用户的所有流动性仓位。
///
/// # 查询参数
///
/// - `wallet_address` (可选): 钱包地址，默认使用配置的钱包
/// - `pool_address` (可选): 池子地址过滤
#[utoipa::path(
    get,
    path = "/api/v1/solana/position/list",
    params(
        ("wallet_address" = Option<String>, Query, description = "钱包地址"),
        ("pool_address" = Option<String>, Query, description = "池子地址过滤")
    ),
    responses(
        (status = 200, description = "查询成功", body = UserPositionsResponse),
        (status = 400, description = "请求参数错误", body = ErrorResponse),
        (status = 500, description = "服务器内部错误", body = ErrorResponse)
    ),
    tag = "Solana流动性"
)]
async fn get_user_positions(
    Extension(services): Extension<Services>,
    Query(request): Query<GetUserPositionsRequest>,
) -> Result<Json<UserPositionsResponse>, (StatusCode, Json<ErrorResponse>)> {
    info!("📋 接收到获取用户仓位列表请求");

    match services.solana.get_user_positions(request).await {
        Ok(response) => {
            info!("✅ 获取用户仓位列表成功，共{}个仓位", response.total_count);
            Ok(Json(response))
        }
        Err(e) => {
            error!("❌ 获取用户仓位列表失败: {:?}", e);
            let error_response = ErrorResponse::new("GET_USER_POSITIONS_ERROR", &format!("获取仓位列表失败: {}", e));
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error_response)))
        }
    }
}

/// 获取仓位详情
///
/// 根据仓位键值获取仓位的详细信息。
///
/// # 查询参数
///
/// - `position_key`: 仓位键值
#[utoipa::path(
    get,
    path = "/api/v1/solana/position/info",
    params(
        ("position_key" = String, Query, description = "仓位键值")
    ),
    responses(
        (status = 200, description = "查询成功", body = PositionInfo),
        (status = 400, description = "请求参数错误", body = ErrorResponse),
        (status = 404, description = "仓位不存在", body = ErrorResponse),
        (status = 500, description = "服务器内部错误", body = ErrorResponse)
    ),
    tag = "Solana流动性"
)]
async fn get_position_info(
    Extension(services): Extension<Services>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<PositionInfo>, (StatusCode, Json<ErrorResponse>)> {
    let position_key = params.get("position_key").ok_or_else(|| {
        let error_response = ErrorResponse::new("POSITION_INFO_ERROR", "缺少position_key参数");
        (StatusCode::BAD_REQUEST, Json(error_response))
    })?;

    info!("🔍 接收到获取仓位详情请求: {}", position_key);

    match services.solana.get_position_info(position_key.clone()).await {
        Ok(response) => {
            info!("✅ 获取仓位详情成功");
            Ok(Json(response))
        }
        Err(e) => {
            error!("❌ 获取仓位详情失败: {:?}", e);
            let error_response = ErrorResponse::new("GET_POSITION_INFO_ERROR", &format!("获取仓位详情失败: {}", e));
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error_response)))
        }
    }
}

/// 检查仓位是否存在
///
/// 检查指定价格范围的仓位是否已经存在。
///
/// # 查询参数
///
/// - `pool_address`: 池子地址
/// - `tick_lower`: 下限tick
/// - `tick_upper`: 上限tick
/// - `wallet_address` (可选): 钱包地址
#[utoipa::path(
    get,
    path = "/api/v1/solana/position/check",
    params(
        ("pool_address" = String, Query, description = "池子地址"),
        ("tick_lower" = i32, Query, description = "下限tick"),
        ("tick_upper" = i32, Query, description = "上限tick"),
        ("wallet_address" = Option<String>, Query, description = "钱包地址")
    ),
    responses(
        (status = 200, description = "检查完成", body = Option<PositionInfo>),
        (status = 400, description = "请求参数错误", body = ErrorResponse),
        (status = 500, description = "服务器内部错误", body = ErrorResponse)
    ),
    tag = "Solana流动性"
)]
async fn check_position_exists(
    Extension(services): Extension<Services>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Option<PositionInfo>>, (StatusCode, Json<ErrorResponse>)> {
    let pool_address = params
        .get("pool_address")
        .ok_or_else(|| {
            let error_response = ErrorResponse::new("CHECK_POSITION_EXISTS_ERROR", "缺少pool_address参数");
            (StatusCode::BAD_REQUEST, Json(error_response))
        })?
        .clone();

    let tick_lower = params
        .get("tick_lower")
        .ok_or_else(|| {
            let error_response = ErrorResponse::new("CHECK_POSITION_EXISTS_ERROR", "缺少tick_lower参数");
            (StatusCode::BAD_REQUEST, Json(error_response))
        })?
        .parse::<i32>()
        .map_err(|_| {
            let error_response = ErrorResponse::new("CHECK_POSITION_EXISTS_ERROR", "tick_lower参数格式错误");
            (StatusCode::BAD_REQUEST, Json(error_response))
        })?;

    let tick_upper = params
        .get("tick_upper")
        .ok_or_else(|| {
            let error_response = ErrorResponse::new("CHECK_POSITION_EXISTS_ERROR", "缺少tick_upper参数");
            (StatusCode::BAD_REQUEST, Json(error_response))
        })?
        .parse::<i32>()
        .map_err(|_| {
            let error_response = ErrorResponse::new("CHECK_POSITION_EXISTS_ERROR", "tick_upper参数格式错误");
            (StatusCode::BAD_REQUEST, Json(error_response))
        })?;

    let wallet_address = params.get("wallet_address").cloned();

    info!("🔍 检查仓位是否存在");
    info!("  池子: {}", pool_address);
    info!("  Tick范围: {} - {}", tick_lower, tick_upper);

    match services
        .solana
        .check_position_exists(pool_address, tick_lower, tick_upper, wallet_address)
        .await
    {
        Ok(response) => {
            if response.is_some() {
                info!("✅ 找到相同范围的仓位");
            } else {
                info!("✅ 没有找到相同范围的仓位");
            }
            Ok(Json(response))
        }
        Err(e) => {
            error!("❌ 检查仓位存在性失败: {:?}", e);
            let error_response = ErrorResponse::new("CHECK_POSITION_EXISTS_ERROR", &format!("检查仓位失败: {}", e));
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error_response)))
        }
    }
}

// ============ CreatePool API处理函数 ============

/// 创建池子
///
/// 在Raydium AMM V3中创建新的流动性池子。
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
///     "transaction_message": "创建池子 - 配置索引: 0, 价格: 1.500000",
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
        (status = 200, description = "创建池子交易构建成功", body = ApiResponse<CreatePoolResponse>),
        (status = 400, description = "请求参数错误", body = ApiResponse<ErrorResponse>),
        (status = 500, description = "服务器内部错误", body = ApiResponse<ErrorResponse>)
    ),
    tag = "Solana池子管理"
)]
async fn create_pool(
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
            Json(ApiResponse::error(ErrorResponse::new("SAME_MINT_ERROR", "两个代币mint地址不能相同"))),
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
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error(error_response))))
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
async fn create_pool_and_send_transaction(
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
            Json(ApiResponse::error(ErrorResponse::new("SAME_MINT_ERROR", "两个代币mint地址不能相同"))),
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
                let error_response = ErrorResponse::new("POOL_ALREADY_EXISTS", "该配置和代币对的池子已存在，请检查参数或使用现有池子");
                Err((StatusCode::CONFLICT, Json(ApiResponse::error(error_response))))
            } else {
                let error_response = ErrorResponse::new("CREATE_POOL_ERROR", &format!("创建池子失败: {}", e));
                Err((StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error(error_response))))
            }
        }
    }
}
