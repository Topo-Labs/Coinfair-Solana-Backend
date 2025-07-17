use crate::{
    dtos::solana_dto::{
        ApiResponse, BalanceResponse, ComputeSwapRequest, ComputeSwapV2Request, ErrorResponse, PriceQuoteRequest, PriceQuoteResponse, RaydiumErrorResponse, RaydiumResponse, SwapComputeData, SwapComputeV2Data,
        SwapRequest, SwapResponse, TransactionData, TransactionSwapRequest, TransactionSwapV2Request, WalletInfo,
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
use tracing::{error, info};

pub struct SolanaController;

impl SolanaController {
    pub fn app() -> Router {
        Router::new()
            .route("/swap", post(swap_tokens))
            .route("/balance", get(get_balance))
            .route("/quote", post(get_price_quote))
            .route("/wallet", get(get_wallet_info))
            .route("/health", get(health_check))
            .route("/compute/swap-base-in", get(compute_swap_v2_base_in))
            .route("/compute/swap-base-out", get(compute_swap_v2_base_out))
            .route("/transaction/swap-base-in", post(transaction_swap_v2_base_in))
            .route("/transaction/swap-base-out", post(transaction_swap_v2_base_out))
        // ============ SwapV2 API兼容路由（支持转账费） ============
        // .route("/compute/swap-v2-base-in", get(compute_swap_v2_base_in))
        // .route("/compute/swap-v2-base-out", get(compute_swap_v2_base_out))
        // .route("/transaction/swap-v2-base-in", post(transaction_swap_v2_base_in))
        // .route("/transaction/swap-v2-base-out", post(transaction_swap_v2_base_out))
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
pub async fn get_balance(Extension(services): Extension<Services>) -> Result<Json<ApiResponse<BalanceResponse>>, (StatusCode, Json<ApiResponse<ErrorResponse>>)> {
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
pub async fn get_wallet_info(Extension(services): Extension<Services>) -> Result<Json<ApiResponse<WalletInfo>>, (StatusCode, Json<ApiResponse<ErrorResponse>>)> {
    info!("🔍 查询钱包信息");

    match services.solana.get_wallet_info().await {
        Ok(wallet_info) => {
            info!("✅ 钱包信息查询成功: {} ({})", wallet_info.address, if wallet_info.connected { "已连接" } else { "未连接" });
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
pub async fn compute_swap_v2_base_in(Extension(services): Extension<Services>, Query(params): Query<ComputeSwapV2Request>) -> Result<Json<RaydiumResponse<SwapComputeV2Data>>, (StatusCode, Json<RaydiumErrorResponse>)> {
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
pub async fn compute_swap_v2_base_out(Extension(services): Extension<Services>, Query(params): Query<ComputeSwapV2Request>) -> Result<Json<RaydiumResponse<SwapComputeV2Data>>, (StatusCode, Json<RaydiumErrorResponse>)> {
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
