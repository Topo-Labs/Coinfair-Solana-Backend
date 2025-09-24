use std::collections::HashMap;

use crate::dtos::solana::common::{ApiResponse, ErrorResponse};
use crate::dtos::solana::clmm::position::liquidity::{
    DecreaseLiquidityAndSendTransactionResponse, DecreaseLiquidityRequest, DecreaseLiquidityResponse,
    IncreaseLiquidityAndSendTransactionResponse, IncreaseLiquidityRequest, IncreaseLiquidityResponse,
};
use crate::dtos::solana::clmm::position::open_position::{
    CalculateLiquidityRequest, CalculateLiquidityResponse, GetUserPositionsRequest,
    OpenPositionAndSendTransactionResponse, OpenPositionRequest, OpenPositionResponse, PositionInfo,
    UserPositionsResponse,
};
use crate::{extractors::validation_extractor::ValidationExtractor, services::Services};
use axum::{
    extract::{Extension, Query},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use tracing::{error, info, warn};

pub struct PositionController;

impl PositionController {
    pub fn routes() -> Router {
        Router::new()
            // ============ OpenPosition API路由 ============
            .route("/open", post(open_position))
            // 开仓并发送交易, 用户本地测试使用，本地签名并发送交易
            .route("/open-and-send-transaction", post(open_position_and_send_transaction))
            .route("/calculate", post(calculate_liquidity))
            .route("/list", get(get_user_positions))
            .route("/info", get(get_position_info))
            .route("/check", get(check_position_exists))
            // ============ IncreaseLiquidity API路由 ============
            .route("/increase-liquidity", post(increase_liquidity))
            .route(
                "/increase-liquidity-and-send-transaction",
                post(increase_liquidity_and_send_transaction),
            )
            // ============ DecreaseLiquidity API路由 ============
            .route("/decrease-liquidity", post(decrease_liquidity))
            .route(
                "/decrease-liquidity-and-send-transaction",
                post(decrease_liquidity_and_send_transaction),
            )
    }
}

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

pub async fn open_position(
    Extension(services): Extension<Services>,
    ValidationExtractor(request): ValidationExtractor<OpenPositionRequest>,
) -> Result<Json<OpenPositionResponse>, (StatusCode, Json<ErrorResponse>)> {
    info!("🎯 接收到开仓请求");
    info!("  池子地址: {}", request.pool_address);
    info!("  用户钱包: {}", request.user_wallet);
    info!(
        "  价格范围: {} - {}",
        request.tick_lower_price, request.tick_upper_price
    );
    info!("  输入金额: {}", request.input_amount);

    // check if tick_lower_price is less than tick_upper_price
    if request.tick_lower_price >= request.tick_upper_price {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse::new(
                "TICK_PRICE_ERROR",
                "tick_lower_price must be less than tick_upper_price",
            )),
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
                let error_response = ErrorResponse::new(
                    "POSITION_ALREADY_EXISTS",
                    "相同价格范围的仓位已存在，请检查您的现有仓位或稍后重试",
                );
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

pub async fn open_position_and_send_transaction(
    Extension(services): Extension<Services>,
    ValidationExtractor(request): ValidationExtractor<OpenPositionRequest>,
) -> Result<Json<OpenPositionAndSendTransactionResponse>, (StatusCode, Json<ErrorResponse>)> {
    info!("🎯 接收到开仓请求");
    info!("  池子地址: {}", request.pool_address);
    info!(
        "  价格范围: {} - {}",
        request.tick_lower_price, request.tick_upper_price
    );

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

pub async fn calculate_liquidity(
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
pub async fn get_user_positions(
    Extension(services): Extension<Services>,
    Query(request): Query<GetUserPositionsRequest>,
) -> Result<Json<ApiResponse<UserPositionsResponse>>, (StatusCode, Json<ErrorResponse>)> {
    info!("📋 接收到获取用户仓位列表请求");

    match services.solana.get_user_positions(request).await {
        Ok(response) => {
            info!("✅ 获取用户仓位列表成功，共{}个仓位", response.total_count);
            Ok(Json(ApiResponse::success(response)))
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
pub async fn get_position_info(
    Extension(services): Extension<Services>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<ApiResponse<PositionInfo>>, (StatusCode, Json<ErrorResponse>)> {
    let position_key = params.get("position_key").ok_or_else(|| {
        let error_response = ErrorResponse::new("POSITION_INFO_ERROR", "缺少position_key参数");
        (StatusCode::BAD_REQUEST, Json(error_response))
    })?;

    info!("🔍 接收到获取仓位详情请求: {}", position_key);

    match services.solana.get_position_info(position_key.clone()).await {
        Ok(response) => {
            info!("✅ 获取仓位详情成功");
            Ok(Json(ApiResponse::success(response)))
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

pub async fn check_position_exists(
    Extension(services): Extension<Services>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<ApiResponse<Option<PositionInfo>>>, (StatusCode, Json<ErrorResponse>)> {
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
            Ok(Json(ApiResponse::success(response)))
        }
        Err(e) => {
            error!("❌ 检查仓位存在性失败: {:?}", e);
            let error_response = ErrorResponse::new("CHECK_POSITION_EXISTS_ERROR", &format!("检查仓位失败: {}", e));
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error_response)))
        }
    }
}

// ============ IncreaseLiquidity API处理函数 ============

/// 增加流动性（构建交易）
///
/// 向现有的流动性仓位增加更多流动性，需要先有相同价格范围的仓位。
///
/// # 请求体
///
/// ```json
/// {
///   "pool_address": "池子地址",
///   "user_wallet": "用户钱包地址",
///   "tick_lower_price": 1.2,
///   "tick_upper_price": 1.8,
///   "is_base_0": true,
///   "input_amount": 1000000,
///   "max_slippage_percent": 0.5
/// }
/// ```
///
/// # 响应示例
///
/// ```json
/// {
///   "transaction": "Base64编码的未签名交易数据",
///   "transaction_message": "增加流动性 - 池子: abc12345, 价格范围: 1.2000-1.8000, 新增流动性: 123456789",
///   "position_key": "现有仓位键值",
///   "liquidity_added": "123456789",
///   "amount_0": 1000000,
///   "amount_1": 500000,
///   "tick_lower_index": -1000,
///   "tick_upper_index": 1000,
///   "pool_address": "池子地址",
///   "timestamp": 1640995200
/// }
/// ```
#[utoipa::path(
    post,
    path = "/api/v1/solana/position/increase-liquidity",
    request_body = IncreaseLiquidityRequest,
    responses(
        (status = 200, description = "增加流动性交易构建成功", body = IncreaseLiquidityResponse),
        (status = 400, description = "请求参数错误", body = ErrorResponse),
        (status = 404, description = "未找到匹配的仓位", body = ErrorResponse),
        (status = 500, description = "服务器内部错误", body = ErrorResponse)
    ),
    tag = "Solana流动性"
)]

pub async fn increase_liquidity(
    Extension(services): Extension<Services>,
    ValidationExtractor(request): ValidationExtractor<IncreaseLiquidityRequest>,
) -> Result<Json<IncreaseLiquidityResponse>, (StatusCode, Json<ErrorResponse>)> {
    info!("🔧 接收到增加流动性请求");
    info!("  池子地址: {}", request.pool_address);
    info!("  用户钱包: {}", request.user_wallet);
    info!(
        "  价格范围: {} - {}",
        request.tick_lower_price, request.tick_upper_price
    );
    info!("  输入金额: {}", request.input_amount);

    // 验证价格范围
    if request.tick_lower_price >= request.tick_upper_price {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse::new("TICK_PRICE_ERROR", "下限价格必须小于上限价格")),
        ));
    }

    match services.solana.increase_liquidity(request).await {
        Ok(response) => {
            info!("✅ 增加流动性交易构建成功: {}", response.transaction_message);
            Ok(Json(response))
        }
        Err(e) => {
            error!("❌ 增加流动性失败: {:?}", e);

            // 检查是否是未找到匹配仓位的错误
            let error_msg = e.to_string();
            if error_msg.contains("未找到匹配的现有仓位") {
                warn!("🔄 检测到未找到匹配仓位的错误");
                let error_response = ErrorResponse::new(
                    "POSITION_NOT_FOUND",
                    "未找到匹配的现有仓位。增加流动性需要先有相同价格范围的仓位。",
                );
                Err((StatusCode::NOT_FOUND, Json(error_response)))
            } else {
                let error_response = ErrorResponse::new("INCREASE_LIQUIDITY_ERROR", &format!("增加流动性失败: {}", e));
                Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error_response)))
            }
        }
    }
}

/// 增加流动性并发送交易
///
/// 向现有的流动性仓位增加更多流动性，并立即发送交易到区块链。
///
/// # 请求体
///
/// ```json
/// {
///   "pool_address": "池子地址",
///   "user_wallet": "用户钱包地址",
///   "tick_lower_price": 1.2,
///   "tick_upper_price": 1.8,
///   "is_base_0": true,
///   "input_amount": 1000000,
///   "max_slippage_percent": 0.5
/// }
/// ```
///
/// # 响应示例
///
/// ```json
/// {
///   "signature": "交易签名",
///   "position_key": "仓位键值",
///   "liquidity_added": "123456789",
///   "amount_0": 1000000,
///   "amount_1": 500000,
///   "tick_lower_index": -1000,
///   "tick_upper_index": 1000,
///   "pool_address": "池子地址",
///   "status": "Finalized",
///   "explorer_url": "https://explorer.solana.com/tx/...",
///   "timestamp": 1640995200
/// }
/// ```
#[utoipa::path(
    post,
    path = "/api/v1/solana/position/increase-liquidity-and-send-transaction",
    request_body = IncreaseLiquidityRequest,
    responses(
        (status = 200, description = "增加流动性成功", body = IncreaseLiquidityAndSendTransactionResponse),
        (status = 400, description = "请求参数错误", body = ErrorResponse),
        (status = 404, description = "未找到匹配的仓位", body = ErrorResponse),
        (status = 500, description = "服务器内部错误", body = ErrorResponse)
    ),
    tag = "Solana流动性"
)]

pub async fn increase_liquidity_and_send_transaction(
    Extension(services): Extension<Services>,
    ValidationExtractor(request): ValidationExtractor<IncreaseLiquidityRequest>,
) -> Result<Json<IncreaseLiquidityAndSendTransactionResponse>, (StatusCode, Json<ErrorResponse>)> {
    info!("🚀 接收到增加流动性并发送交易请求");
    info!("  池子地址: {}", request.pool_address);
    info!("  用户钱包: {}", request.user_wallet);
    info!(
        "  价格范围: {} - {}",
        request.tick_lower_price, request.tick_upper_price
    );
    info!("  输入金额: {}", request.input_amount);

    // 验证价格范围
    if request.tick_lower_price >= request.tick_upper_price {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse::new("TICK_PRICE_ERROR", "下限价格必须小于上限价格")),
        ));
    }

    match services.solana.increase_liquidity_and_send_transaction(request).await {
        Ok(response) => {
            info!("✅ 增加流动性成功: {}", response.signature);
            Ok(Json(response))
        }
        Err(e) => {
            error!("❌ 增加流动性并发送交易失败: {:?}", e);

            // 检查是否是未找到匹配仓位的错误
            let error_msg = e.to_string();
            if error_msg.contains("未找到匹配的现有仓位") {
                warn!("🔄 检测到未找到匹配仓位的错误");
                let error_response = ErrorResponse::new(
                    "POSITION_NOT_FOUND",
                    "未找到匹配的现有仓位。增加流动性需要先有相同价格范围的仓位。",
                );
                Err((StatusCode::NOT_FOUND, Json(error_response)))
            } else if error_msg.contains("AccountOwnedByWrongProgram") {
                warn!("🔧 检测到Token Program不匹配错误，NFT可能使用Token-2022");
                let error_response = ErrorResponse::new(
                    "TOKEN_PROGRAM_MISMATCH",
                    "NFT账户使用了Token-2022程序，这个错误已在新版本中修复。请联系技术支持。",
                );
                Err((StatusCode::BAD_REQUEST, Json(error_response)))
            } else {
                let error_response = ErrorResponse::new("INCREASE_LIQUIDITY_ERROR", &format!("增加流动性失败: {}", e));
                Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error_response)))
            }
        }
    }
}

// ============ DecreaseLiquidity API处理函数 ============

/// 减少流动性（构建交易）
///
/// 减少现有流动性仓位的流动性数量，可以部分或全部减少。
///
/// # 请求体
///
/// ```json
/// {
///   "pool_address": "池子地址",
///   "user_wallet": "用户钱包地址",
///   "tick_lower_index": -1000,
///   "tick_upper_index": 1000,
///   "liquidity": "123456789", // 可选，如果为空则减少全部流动性
///   "max_slippage_percent": 0.5,
///   "simulate": false
/// }
/// ```
///
/// # 响应示例
///
/// ```json
/// {
///   "transaction": "Base64编码的未签名交易数据",
///   "transaction_message": "减少流动性 - 池子: abc12345, 仓位: def67890, 减少流动性: 123456789",
///   "position_key": "现有仓位键值",
///   "liquidity_removed": "123456789",
///   "amount_0_min": 950000,
///   "amount_1_min": 475000,
///   "amount_0_expected": 1000000,
///   "amount_1_expected": 500000,
///   "tick_lower_index": -1000,
///   "tick_upper_index": 1000,
///   "pool_address": "池子地址",
///   "will_close_position": false,
///   "timestamp": 1640995200
/// }
/// ```
#[utoipa::path(
    post,
    path = "/api/v1/solana/position/decrease-liquidity",
    request_body = DecreaseLiquidityRequest,
    responses(
        (status = 200, description = "减少流动性交易构建成功", body = DecreaseLiquidityResponse),
        (status = 400, description = "请求参数错误", body = ErrorResponse),
        (status = 404, description = "未找到匹配的仓位", body = ErrorResponse),
        (status = 500, description = "服务器内部错误", body = ErrorResponse)
    ),
    tag = "Solana流动性"
)]

pub async fn decrease_liquidity(
    Extension(services): Extension<Services>,
    ValidationExtractor(request): ValidationExtractor<DecreaseLiquidityRequest>,
) -> Result<Json<DecreaseLiquidityResponse>, (StatusCode, Json<ErrorResponse>)> {
    info!("🔧 接收到减少流动性请求");
    info!("  池子地址: {}", request.pool_address);
    info!("  用户钱包: {}", request.user_wallet);
    info!(
        "  Tick范围: {} - {}",
        request.tick_lower_index, request.tick_upper_index
    );
    info!("  减少流动性: {:?}", request.liquidity);

    // 验证tick范围
    if request.tick_lower_index >= request.tick_upper_index {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse::new(
                "TICK_INDEX_ERROR",
                "下限tick索引必须小于上限tick索引",
            )),
        ));
    }

    match services.solana.decrease_liquidity(request).await {
        Ok(response) => {
            info!("✅ 减少流动性交易构建成功: {}", response.transaction_message);
            Ok(Json(response))
        }
        Err(e) => {
            error!("❌ 减少流动性失败: {:?}", e);

            // 检查是否是未找到匹配仓位的错误
            let error_msg = e.to_string();
            if error_msg.contains("未找到匹配的仓位") {
                warn!("🔄 检测到未找到匹配仓位的错误");
                let error_response =
                    ErrorResponse::new("POSITION_NOT_FOUND", "未找到匹配的仓位。请检查tick索引范围和池子地址。");
                Err((StatusCode::NOT_FOUND, Json(error_response)))
            } else {
                let error_response = ErrorResponse::new("DECREASE_LIQUIDITY_ERROR", &format!("减少流动性失败: {}", e));
                Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error_response)))
            }
        }
    }
}

/// 减少流动性并发送交易
///
/// 减少现有流动性仓位的流动性数量，并立即发送交易到区块链。
///
/// # 请求体
///
/// ```json
/// {
///   "pool_address": "池子地址",
///   "user_wallet": "用户钱包地址",
///   "tick_lower_index": -1000,
///   "tick_upper_index": 1000,
///   "liquidity": "123456789", // 可选，如果为空则减少全部流动性
///   "max_slippage_percent": 0.5,
///   "simulate": false
/// }
/// ```
///
/// # 响应示例
///
/// ```json
/// {
///   "signature": "交易签名",
///   "position_key": "仓位键值",
///   "liquidity_removed": "123456789",
///   "amount_0_actual": 1000000,
///   "amount_1_actual": 500000,
///   "tick_lower_index": -1000,
///   "tick_upper_index": 1000,
///   "pool_address": "池子地址",
///   "position_closed": false,
///   "status": "Finalized",
///   "explorer_url": "https://explorer.solana.com/tx/...",
///   "timestamp": 1640995200
/// }
/// ```
#[utoipa::path(
    post,
    path = "/api/v1/solana/position/decrease-liquidity-and-send-transaction",
    request_body = DecreaseLiquidityRequest,
    responses(
        (status = 200, description = "减少流动性成功", body = DecreaseLiquidityAndSendTransactionResponse),
        (status = 400, description = "请求参数错误", body = ErrorResponse),
        (status = 404, description = "未找到匹配的仓位", body = ErrorResponse),
        (status = 500, description = "服务器内部错误", body = ErrorResponse)
    ),
    tag = "Solana流动性"
)]

pub async fn decrease_liquidity_and_send_transaction(
    Extension(services): Extension<Services>,
    ValidationExtractor(request): ValidationExtractor<DecreaseLiquidityRequest>,
) -> Result<Json<DecreaseLiquidityAndSendTransactionResponse>, (StatusCode, Json<ErrorResponse>)> {
    info!("🚀 接收到减少流动性并发送交易请求");
    info!("  池子地址: {}", request.pool_address);
    info!("  用户钱包: {}", request.user_wallet);
    info!(
        "  Tick范围: {} - {}",
        request.tick_lower_index, request.tick_upper_index
    );
    info!("  减少流动性: {:?}", request.liquidity);

    // 验证tick范围
    if request.tick_lower_index >= request.tick_upper_index {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse::new(
                "TICK_INDEX_ERROR",
                "下限tick索引必须小于上限tick索引",
            )),
        ));
    }

    match services.solana.decrease_liquidity_and_send_transaction(request).await {
        Ok(response) => {
            info!("✅ 减少流动性成功: {}", response.signature);
            Ok(Json(response))
        }
        Err(e) => {
            error!("❌ 减少流动性并发送交易失败: {:?}", e);

            // 检查是否是未找到匹配仓位的错误
            let error_msg = e.to_string();
            if error_msg.contains("未找到匹配的仓位") {
                warn!("🔄 检测到未找到匹配仓位的错误");
                let error_response =
                    ErrorResponse::new("POSITION_NOT_FOUND", "未找到匹配的仓位。请检查tick索引范围和池子地址。");
                Err((StatusCode::NOT_FOUND, Json(error_response)))
            } else if error_msg.contains("AccountOwnedByWrongProgram") {
                warn!("🔧 检测到Token Program不匹配错误，NFT可能使用Token-2022");
                let error_response = ErrorResponse::new(
                    "TOKEN_PROGRAM_MISMATCH",
                    "NFT账户使用了Token-2022程序，这个错误已在新版本中修复。请联系技术支持。",
                );
                Err((StatusCode::BAD_REQUEST, Json(error_response)))
            } else {
                let error_response = ErrorResponse::new("DECREASE_LIQUIDITY_ERROR", &format!("减少流动性失败: {}", e));
                Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error_response)))
            }
        }
    }
}
