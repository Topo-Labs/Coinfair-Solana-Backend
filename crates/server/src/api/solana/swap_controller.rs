use crate::{
    dtos::solana_dto::{
        ApiResponse, BalanceResponse, ErrorResponse, PriceQuoteRequest, PriceQuoteResponse, SwapRequest, SwapResponse, WalletInfo,
    },
    extractors::validation_extractor::ValidationExtractor,
    services::Services,
};
use axum::{
    extract::Extension,
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use tracing::{error, info};

pub struct SwapController;

impl SwapController {
    pub fn routes() -> Router {
        Router::new()
            .route("/swap", post(swap_tokens))
            .route("/balance", get(get_balance))
            .route("/quote", post(get_price_quote))
            .route("/wallet", get(get_wallet_info))
            .route("/health", get(health_check))
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