use crate::dtos::solana::common::{ApiResponse, ErrorResponse};
use crate::dtos::solana::cpmm::deposit::{
    CpmmDepositAndSendRequest, CpmmDepositAndSendResponse, CpmmDepositCompute, CpmmDepositRequest, CpmmDepositResponse,
};
use crate::{extractors::validation_extractor::ValidationExtractor, services::Services};
use axum::{
    extract::{Extension, Query},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::Deserialize;
use tracing::{error, info};

/// CPMM存款控制器
///
/// 提供CPMM流动性存款相关的HTTP API端点
/// 基于CLI的Deposit逻辑100%实现
pub struct CpmmDepositController;

/// 计算存款参数 - Query参数
#[derive(Debug, Deserialize)]
pub struct ComputeDepositQuery {
    /// 池子ID
    pub pool_id: String,
    /// LP代币数量
    pub lp_token_amount: u64,
    /// 滑点容忍度(百分比)
    pub slippage: Option<f64>,
}

impl CpmmDepositController {
    /// 创建路由配置
    pub fn routes() -> Router {
        Router::new()
            .route("/deposit", post(deposit_liquidity))
            .route("/deposit-and-send", post(deposit_liquidity_and_send))
            .route("/compute-deposit", get(compute_deposit))
    }
}

/// POST /api/v1/solana/liquidity/cpmm/deposit
///
/// 100%忠实CLI的Deposit逻辑：
/// 1. 获取池子状态和金库信息
/// 2. 使用CurveCalculator计算LP代币到基础代币的转换
/// 3. 应用滑点保护
/// 4. 计算并添加transfer fee
/// 5. 创建用户ATA账户
/// 6. 构建deposit指令
/// 7. 返回未签名交易
pub async fn deposit_liquidity(
    Extension(services): Extension<Services>,
    ValidationExtractor(request): ValidationExtractor<CpmmDepositRequest>,
) -> Result<Json<ApiResponse<CpmmDepositResponse>>, (StatusCode, Json<ApiResponse<ErrorResponse>>)> {
    info!("🏗️ 收到CPMM存入流动性请求: pool_id={}", request.pool_id);

    match services.solana.cpmm_deposit_liquidity(request).await {
        Ok(response) => {
            info!("✅ CPMM存入流动性交易构建成功");
            Ok(Json(ApiResponse::success(response)))
        }
        Err(e) => {
            error!("❌ CPMM存入流动性失败: {:?}", e);
            let error_response = ErrorResponse::new("CPMM_DEPOSIT_FAILED", &format!("CPMM存入流动性失败: {}", e));
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error(error_response)),
            ))
        }
    }
}

/// 存入流动性并发送交易 - 使用服务端私钥签名并发送交易
/// POST /api/v1/solana/liquidity/cpmm/deposit-and-send
///
/// 100%忠实CLI的Deposit逻辑，但使用服务端私钥自动签名发送：
/// 1. 执行完整的deposit计算流程
/// 2. 使用配置的私钥进行签名
/// 3. 发送交易到Solana网络
/// 4. 返回交易签名和结果
pub async fn deposit_liquidity_and_send(
    Extension(services): Extension<Services>,
    ValidationExtractor(request): ValidationExtractor<CpmmDepositAndSendRequest>,
) -> Result<Json<ApiResponse<CpmmDepositAndSendResponse>>, (StatusCode, Json<ApiResponse<ErrorResponse>>)> {
    info!("🚀 收到CPMM存入流动性并发送交易请求: pool_id={}", request.pool_id);

    match services.solana.cpmm_deposit_liquidity_and_send(request).await {
        Ok(response) => {
            info!("✅ CPMM存入流动性交易发送成功，签名: {}", response.signature);
            Ok(Json(ApiResponse::success(response)))
        }
        Err(e) => {
            error!("❌ CPMM存入流动性并发送交易失败: {:?}", e);
            let error_response = ErrorResponse::new(
                "CPMM_DEPOSIT_AND_SEND_FAILED",
                &format!("CPMM存入流动性并发送交易失败: {}", e),
            );
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error(error_response)),
            ))
        }
    }
}

/// 计算存入流动性结果 - 预览功能
/// GET /api/v1/solana/liquidity/cpmm/compute-deposit?pool_id=xxx&lp_token_amount=xxx&slippage=0.5
///
/// 提供存入流动性的预计算功能，让用户了解：
/// 1. 需要存入的基础代币数量
/// 2. 滑点影响后的数量
/// 3. 需要支付的转账费
/// 4. 最终最大输入数量
pub async fn compute_deposit(
    Extension(services): Extension<Services>,
    Query(query): Query<ComputeDepositQuery>,
) -> Result<Json<ApiResponse<CpmmDepositCompute>>, (StatusCode, Json<ApiResponse<ErrorResponse>>)> {
    info!("📊 收到CPMM存入流动性计算请求: {:?}", query);

    match services
        .solana
        .compute_cpmm_deposit(&query.pool_id, query.lp_token_amount, query.slippage)
        .await
    {
        Ok(response) => {
            info!("✅ CPMM存入流动性计算成功");
            Ok(Json(ApiResponse::success(response)))
        }
        Err(e) => {
            error!("❌ CPMM存入流动性计算失败: {:?}", e);
            let error_response =
                ErrorResponse::new("CPMM_COMPUTE_DEPOSIT_FAILED", &format!("CPMM存入流动性计算失败: {}", e));
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error(error_response)),
            ))
        }
    }
}
