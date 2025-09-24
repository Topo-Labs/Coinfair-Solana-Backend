use crate::extractors::validation_extractor::ValidationExtractor;
use crate::services::Services;

use crate::dtos::solana::common::{ApiResponse, ErrorResponse};
use crate::dtos::solana::clmm::nft::claim::{ClaimNftAndSendTransactionResponse, ClaimNftRequest, ClaimNftResponse};
use crate::dtos::solana::clmm::nft::mint::{MintNftAndSendTransactionResponse, MintNftRequest, MintNftResponse};
use axum::{extract::Extension, http::StatusCode, response::Json, routing::post, Router};
use tracing::{error, info};

pub struct NftController;

impl NftController {
    pub fn routes() -> Router {
        Router::new()
            // ============ MintNft API路由 ============
            .route("/mint-nft", post(mint_nft))
            // NFT铸造并发送交易, 用户本地测试使用，本地签名并发送交易
            .route("/mint-nft-and-send-transaction", post(mint_nft_and_send_transaction))
            // ============ ClaimNft API路由 ============
            .route("/claim-nft", post(claim_nft))
            // NFT领取并发送交易, 用户本地测试使用，本地签名并发送交易
            .route("/claim-nft-and-send-transaction", post(claim_nft_and_send_transaction))
    }
}

/// 铸造推荐NFT（构建交易但不签名）
///
/// 构建铸造推荐NFT的交易，但不签名，返回序列化的交易给前端进行签名和发送。
///
/// # 请求体
///
/// ```json
/// {
///   "user_wallet": "用户钱包地址",
///   "amount": 1
/// }
/// ```
///
/// # 响应示例
///
/// ```json
/// {
///   "id": "uuid-string",
///   "success": true,
///   "data": {
///     "signature": null,
///     "user_wallet": "用户钱包地址",
///     "amount": 1,
///     "nft_mint": "NFT mint地址",
///     "user_referral": "用户推荐账户地址",
///     "mint_counter": "mint计数器地址",
///     "nft_pool_authority": "NFT池子权限地址",
///     "nft_pool_account": "NFT池子账户地址",
///     "status": "Pending",
///     "explorer_url": null,
///     "timestamp": 1640995200,
///     "serialized_transaction": "base64编码的交易数据"
///   }
/// }
/// ```
#[utoipa::path(
    post,
    path = "/api/v1/solana/nft/mint-nft",
    request_body = MintNftRequest,
    responses(
        (status = 200, description = "NFT铸造交易构建成功", body = ApiResponse<MintNftResponse>),
        (status = 400, description = "请求参数错误", body = ApiResponse<ErrorResponse>),
        (status = 500, description = "服务器内部错误", body = ApiResponse<ErrorResponse>)
    ),
    tag = "Solana推荐NFT"
)]
pub async fn mint_nft(
    Extension(services): Extension<Services>,
    ValidationExtractor(request): ValidationExtractor<MintNftRequest>,
) -> Result<Json<ApiResponse<MintNftResponse>>, (StatusCode, Json<ApiResponse<ErrorResponse>>)> {
    info!("🎯 接收到铸造推荐NFT请求");
    info!("  用户钱包: {}", request.user_wallet);
    info!("  铸造数量: {}", request.amount);

    match services.solana.mint_nft(request).await {
        Ok(response) => {
            info!("✅ NFT铸造交易构建成功");
            Ok(Json(ApiResponse::success(response)))
        }
        Err(e) => {
            error!("❌ NFT铸造交易构建失败: {}", e);
            let error_response = ErrorResponse {
                code: "MINT_NFT_BUILD_FAILED".to_string(),
                message: format!("NFT铸造交易构建失败: {}", e),
                details: None,
                timestamp: chrono::Utc::now().timestamp(),
            };
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error(error_response)),
            ))
        }
    }
}

/// 铸造推荐NFT并发送交易（本地签名和发送）
///
/// 铸造推荐NFT并使用本地密钥签名发送交易。主要用于本地测试。
///
/// # 请求体
///
/// ```json
/// {
///   "user_wallet": "用户钱包地址",
///   "amount": 1
/// }
/// ```
///
/// # 响应示例
///
/// ```json
/// {
///   "id": "uuid-string",
///   "success": true,
///   "data": {
///     "signature": "交易签名",
///     "user_wallet": "用户钱包地址",
///     "amount": 1,
///     "nft_mint": "NFT mint地址",
///     "user_referral": "用户推荐账户地址",
///     "mint_counter": "mint计数器地址",
///     "nft_pool_authority": "NFT池子权限地址",
///     "nft_pool_account": "NFT池子账户地址",
///     "status": "Success",
///     "explorer_url": "https://explorer.solana.com/tx/...",
///     "timestamp": 1640995200
///   }
/// }
/// ```
#[utoipa::path(
    post,
    path = "/api/v1/solana/nft/mint-nft-and-send-transaction",
    request_body = MintNftRequest,
    responses(
        (status = 200, description = "NFT铸造交易成功", body = ApiResponse<MintNftAndSendTransactionResponse>),
        (status = 400, description = "请求参数错误", body = ApiResponse<ErrorResponse>),
        (status = 500, description = "服务器内部错误", body = ApiResponse<ErrorResponse>)
    ),
    tag = "Solana推荐NFT"
)]
pub async fn mint_nft_and_send_transaction(
    Extension(services): Extension<Services>,
    ValidationExtractor(request): ValidationExtractor<MintNftRequest>,
) -> Result<Json<ApiResponse<MintNftAndSendTransactionResponse>>, (StatusCode, Json<ApiResponse<ErrorResponse>>)> {
    info!("🎯 接收到铸造推荐NFT并发送交易请求");
    info!("  用户钱包: {}", request.user_wallet);
    info!("  铸造数量: {}", request.amount);

    match services.solana.mint_nft_and_send_transaction(request).await {
        Ok(response) => {
            info!("✅ NFT铸造交易成功，签名: {}", response.signature);
            Ok(Json(ApiResponse::success(response)))
        }
        Err(e) => {
            error!("❌ NFT铸造交易失败: {}", e);
            let error_response = ErrorResponse {
                code: "MINT_NFT_TRANSACTION_FAILED".to_string(),
                message: format!("NFT铸造交易失败: {}", e),
                details: None,
                timestamp: chrono::Utc::now().timestamp(),
            };
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error(error_response)),
            ))
        }
    }
}

/// 领取推荐NFT（构建交易但不签名）
///
/// 构建领取推荐NFT的交易，但不签名，返回序列化的交易给前端进行签名和发送。
///
/// # 请求体
///
/// ```json
/// {
///   "user_wallet": "下级用户钱包地址",
///   "upper": "上级用户钱包地址"
/// }
/// ```
///
/// # 响应示例
///
/// ```json
/// {
///   "id": "uuid-string",
///   "success": true,
///   "data": {
///     "signature": null,
///     "user_wallet": "下级用户钱包地址",
///     "upper": "上级用户钱包地址",
///     "nft_mint": "NFT mint地址",
///     "user_referral": "下级用户推荐账户地址",
///     "upper_referral": "上级用户推荐账户地址",
///     "upper_mint_counter": "上级用户mint计数器地址",
///     "nft_pool_authority": "NFT池子权限地址",
///     "nft_pool_account": "NFT池子账户地址",
///     "user_ata": "下级用户ATA账户地址",
///     "protocol_wallet": "协议钱包地址",
///     "referral_config": "推荐配置账户地址",
///     "status": "Pending",
///     "explorer_url": null,
///     "timestamp": 1640995200,
///     "serialized_transaction": "base64编码的交易数据"
///   }
/// }
/// ```
#[utoipa::path(
    post,
    path = "/api/v1/solana/nft/claim-nft",
    request_body = ClaimNftRequest,
    responses(
        (status = 200, description = "NFT领取交易构建成功", body = ApiResponse<ClaimNftResponse>),
        (status = 400, description = "请求参数错误", body = ApiResponse<ErrorResponse>),
        (status = 500, description = "服务器内部错误", body = ApiResponse<ErrorResponse>)
    ),
    tag = "Solana推荐NFT"
)]
pub async fn claim_nft(
    Extension(services): Extension<Services>,
    ValidationExtractor(request): ValidationExtractor<ClaimNftRequest>,
) -> Result<Json<ApiResponse<ClaimNftResponse>>, (StatusCode, Json<ApiResponse<ErrorResponse>>)> {
    info!("🎯 接收到领取推荐NFT请求");
    info!("  下级用户钱包: {}", request.user_wallet);
    info!("  上级用户钱包: {}", request.upper);

    match services.solana.claim_nft(request).await {
        Ok(response) => {
            info!("✅ NFT领取交易构建成功");
            Ok(Json(ApiResponse::success(response)))
        }
        Err(e) => {
            error!("❌ NFT领取交易构建失败: {}", e);
            let error_response = ErrorResponse {
                code: "CLAIM_NFT_BUILD_FAILED".to_string(),
                message: format!("NFT领取交易构建失败: {}", e),
                details: None,
                timestamp: chrono::Utc::now().timestamp(),
            };
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error(error_response)),
            ))
        }
    }
}

/// 领取推荐NFT并发送交易（本地签名和发送）
///
/// 领取推荐NFT并使用本地密钥签名发送交易。主要用于本地测试。
///
/// # 请求体
///
/// ```json
/// {
///   "user_wallet": "下级用户钱包地址",
///   "upper": "上级用户钱包地址"
/// }
/// ```
///
/// # 响应示例
///
/// ```json
/// {
///   "id": "uuid-string",
///   "success": true,
///   "data": {
///     "signature": "交易签名",
///     "user_wallet": "下级用户钱包地址",
///     "upper": "上级用户钱包地址",
///     "nft_mint": "NFT mint地址",
///     "user_referral": "下级用户推荐账户地址",
///     "upper_referral": "上级用户推荐账户地址",
///     "upper_mint_counter": "上级用户mint计数器地址",
///     "nft_pool_authority": "NFT池子权限地址",
///     "nft_pool_account": "NFT池子账户地址",
///     "user_ata": "下级用户ATA账户地址",
///     "protocol_wallet": "协议钱包地址",
///     "referral_config": "推荐配置账户地址",
///     "status": "Success",
///     "explorer_url": "https://explorer.solana.com/tx/...",
///     "timestamp": 1640995200
///   }
/// }
/// ```
#[utoipa::path(
    post,
    path = "/api/v1/solana/nft/claim-nft-and-send-transaction",
    request_body = ClaimNftRequest,
    responses(
        (status = 200, description = "NFT领取交易成功", body = ApiResponse<ClaimNftAndSendTransactionResponse>),
        (status = 400, description = "请求参数错误", body = ApiResponse<ErrorResponse>),
        (status = 500, description = "服务器内部错误", body = ApiResponse<ErrorResponse>)
    ),
    tag = "Solana推荐NFT"
)]
pub async fn claim_nft_and_send_transaction(
    Extension(services): Extension<Services>,
    ValidationExtractor(request): ValidationExtractor<ClaimNftRequest>,
) -> Result<Json<ApiResponse<ClaimNftAndSendTransactionResponse>>, (StatusCode, Json<ApiResponse<ErrorResponse>>)> {
    info!("🎯 接收到领取推荐NFT并发送交易请求");
    info!("  下级用户钱包: {}", request.user_wallet);
    info!("  上级用户钱包: {}", request.upper);

    match services.solana.claim_nft_and_send_transaction(request).await {
        Ok(response) => {
            info!("✅ NFT领取交易成功，签名: {}", response.signature);
            Ok(Json(ApiResponse::success(response)))
        }
        Err(e) => {
            error!("❌ NFT领取交易失败: {}", e);
            let error_response = ErrorResponse {
                code: "CLAIM_NFT_TRANSACTION_FAILED".to_string(),
                message: format!("NFT领取交易失败: {}", e),
                details: None,
                timestamp: chrono::Utc::now().timestamp(),
            };
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error(error_response)),
            ))
        }
    }
}
