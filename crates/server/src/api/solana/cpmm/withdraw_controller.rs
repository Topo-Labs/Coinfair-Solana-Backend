use axum::{
    extract::{Extension, Query},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::Deserialize;
use tracing::info;

use crate::{
    dtos::solana::common::{ApiResponse, ErrorResponse},
    dtos::solana::cpmm::withdraw::{
        CpmmWithdrawAndSendRequest, CpmmWithdrawAndSendResponse, CpmmWithdrawCompute, CpmmWithdrawRequest,
        CpmmWithdrawResponse,
    },
    extractors::validation_extractor::ValidationExtractor,
    services::Services,
};

/// CPMM Withdraw Controller
/// 处理CPMM池子的流动性提取操作
/// 基于CLI的Withdraw逻辑100%实现
pub struct CpmmWithdrawController;

/// 计算提取参数 - Query参数
#[derive(Debug, Deserialize)]
pub struct ComputeWithdrawQuery {
    /// 池子ID
    pub pool_id: String,
    /// LP代币数量
    pub lp_token_amount: u64,
    /// 滑点容忍度(百分比)
    pub slippage: Option<f64>,
}

impl CpmmWithdrawController {
    /// 创建路由
    pub fn routes() -> Router {
        Router::new()
            .route("/withdraw", post(withdraw_liquidity))
            .route("/withdraw-and-send", post(withdraw_liquidity_and_send))
            .route("/compute-withdraw", get(compute_withdraw))
    }
}

/// POST /api/v1/solana/liquidity/cpmm/withdraw
///
/// 100%忠实CLI的Withdraw逻辑：
/// 1. 获取池子状态和金库信息
/// 2. 使用CurveCalculator计算LP代币到基础代币的转换
/// 3. 应用滑点保护
/// 4. 计算并扣除transfer fee
/// 5. 创建用户接收代币的ATA账户
/// 6. 构建withdraw指令
/// 7. 返回未签名交易
pub async fn withdraw_liquidity(
    Extension(services): Extension<Services>,
    ValidationExtractor(request): ValidationExtractor<CpmmWithdrawRequest>,
) -> Result<Json<ApiResponse<CpmmWithdrawResponse>>, (StatusCode, Json<ApiResponse<ErrorResponse>>)> {
    info!("🏗️ 收到CPMM提取流动性请求: pool_id={}", request.pool_id);

    match services.solana.cpmm_withdraw_liquidity(request).await {
        Ok(response) => {
            info!("✅ CPMM提取流动性交易构建成功");
            Ok(Json(ApiResponse::success(response)))
        }
        Err(e) => {
            tracing::error!("❌ CPMM提取流动性失败: {:?}", e);
            let error_response = ErrorResponse::new("CPMM_WITHDRAW_FAILED", &format!("CPMM提取流动性失败: {}", e));
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error(error_response)),
            ))
        }
    }
}

/// 提取流动性并发送交易 - 使用服务端私钥签名并发送交易
/// POST /api/v1/solana/liquidity/cpmm/withdraw-and-send
///
/// 100%忠实CLI的Withdraw逻辑，但使用服务端私钥自动签名发送：
/// 1. 执行完整的withdraw计算流程
/// 2. 使用配置的私钥进行签名
/// 3. 发送交易到Solana网络
/// 4. 返回交易签名和结果
pub async fn withdraw_liquidity_and_send(
    Extension(services): Extension<Services>,
    ValidationExtractor(request): ValidationExtractor<CpmmWithdrawAndSendRequest>,
) -> Result<Json<ApiResponse<CpmmWithdrawAndSendResponse>>, (StatusCode, Json<ApiResponse<ErrorResponse>>)> {
    info!("🚀 收到CPMM提取流动性并发送交易请求: pool_id={}", request.pool_id);

    match services.solana.cpmm_withdraw_liquidity_and_send(request).await {
        Ok(response) => {
            info!("✅ CPMM提取流动性交易发送成功，签名: {}", response.signature);
            Ok(Json(ApiResponse::success(response)))
        }
        Err(e) => {
            tracing::error!("❌ CPMM提取流动性并发送交易失败: {:?}", e);
            let error_response = ErrorResponse::new(
                "CPMM_WITHDRAW_AND_SEND_FAILED",
                &format!("CPMM提取流动性并发送交易失败: {}", e),
            );
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error(error_response)),
            ))
        }
    }
}

/// 计算提取流动性结果 - 预览功能
/// GET /api/v1/solana/liquidity/cpmm/compute-withdraw?pool_id=xxx&lp_token_amount=xxx&slippage=0.5
///
/// 提供提取流动性的预计算功能，让用户了解：
/// 1. 可获得的基础代币数量
/// 2. 滑点影响后的数量
/// 3. 需要扣除的转账费
/// 4. 最终最小输出数量
pub async fn compute_withdraw(
    Extension(services): Extension<Services>,
    Query(query): Query<ComputeWithdrawQuery>,
) -> Result<Json<ApiResponse<CpmmWithdrawCompute>>, (StatusCode, Json<ApiResponse<ErrorResponse>>)> {
    info!("📊 收到CPMM提取流动性计算请求: {:?}", query);

    match services
        .solana
        .compute_cpmm_withdraw(&query.pool_id, query.lp_token_amount, query.slippage)
        .await
    {
        Ok(response) => {
            info!("✅ CPMM提取流动性计算成功");
            Ok(Json(ApiResponse::success(response)))
        }
        Err(e) => {
            tracing::error!("❌ CPMM提取流动性计算失败: {:?}", e);
            let error_response = ErrorResponse::new(
                "CPMM_COMPUTE_WITHDRAW_FAILED",
                &format!("CPMM提取流动性计算失败: {}", e),
            );
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error(error_response)),
            ))
        }
    }
}
