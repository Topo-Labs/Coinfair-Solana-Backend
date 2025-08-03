use crate::dtos::solana_dto::{
    ApiResponse, ErrorResponse, GetUpperAndVerifyResponse, GetUpperRequest, GetUpperResponse,
    GetMintCounterRequest, GetMintCounterResponse, GetMintCounterAndVerifyResponse,
};
use crate::services::Services;
use crate::services::solana::SolanaService;

use axum::{
    extract::{Extension, Query},
    http::StatusCode,
    response::Json,
    routing::get,
    Router,
};
use tracing::{error, info};
use validator::Validate;

pub struct ReferralController;

impl ReferralController {
    pub fn routes() -> Router {
        Router::new()
            // ============ GetUpper API路由 ============
            .route("/get-upper", get(get_upper))
            // GetUpper查询并本地验证, 用户本地测试使用，包含完整验证信息
            .route("/get-upper-and-verify", get(get_upper_and_verify))
            // ============ GetMintCounter API路由 ============
            .route("/get-mint-counter", get(get_mint_counter))
            // GetMintCounter查询并本地验证, 用户本地测试使用，包含完整验证信息
            .route("/get-mint-counter-and-verify", get(get_mint_counter_and_verify))
    }
}

/// 获取用户的上级推荐人（查询但不签名）
///
/// 查询用户在推荐系统中的上级推荐人信息。100%复现CLI中GetUpper功能的业务逻辑。
///
/// # 查询参数
///
/// - `user_wallet`: 用户钱包地址
///
/// # 响应示例
///
/// ```json
/// {
///   "id": "uuid-string",
///   "success": true,
///   "data": {
///     "user_wallet": "用户钱包地址",
///     "upper": "上级钱包地址", // 如果存在
///     "referral_account": "推荐账户PDA地址",
///     "status": "Success",
///     "timestamp": 1640995200
///   }
/// }
/// ```
///
/// # 错误情况
///
/// - 如果用户还没有推荐关系，`upper`字段为`null`，`status`为"AccountNotFound"
/// - 如果钱包地址格式无效，返回400错误
#[utoipa::path(
    get,
    path = "/api/v1/solana/nft/referral/get-upper",
    params(GetUpperRequest),
    responses(
        (status = 200, description = "成功查询上级推荐人", body = ApiResponse<GetUpperResponse>),
        (status = 400, description = "请求参数错误", body = ApiResponse<ErrorResponse>),
        (status = 500, description = "服务器内部错误", body = ApiResponse<ErrorResponse>)
    ),
    tag = "Solana推荐系统"
)]
pub async fn get_upper(
    Extension(services): Extension<Services>,
    Query(request): Query<GetUpperRequest>,
) -> Result<Json<ApiResponse<GetUpperResponse>>, (StatusCode, Json<ApiResponse<ErrorResponse>>)> {
    info!("🎯 接收到查询上级推荐人请求");
    info!("  用户钱包: {}", request.user_wallet);

    // 验证请求参数
    if let Err(validation_errors) = request.validate() {
        let error_message = validation_errors
            .field_errors()
            .iter()
            .map(|(field, errors)| {
                let error_messages: Vec<String> = errors
                    .iter()
                    .map(|error| error.message.as_ref().unwrap_or(&std::borrow::Cow::Borrowed("Invalid value")).to_string())
                    .collect();
                format!("{}: {}", field, error_messages.join(", "))
            })
            .collect::<Vec<String>>()
            .join("; ");

        let error_response = ErrorResponse {
            code: "VALIDATION_ERROR".to_string(),
            message: format!("请求参数验证失败: {}", error_message),
            details: None,
            timestamp: chrono::Utc::now().timestamp(),
        };

        return Err((StatusCode::BAD_REQUEST, Json(ApiResponse::error(error_response))));
    }

    // 将DynSolanaService转换为具体的SolanaService以访问referral字段
    let concrete_service = services.solana
        .as_any()
        .downcast_ref::<SolanaService>()
        .ok_or_else(|| {
            error!("无法将SolanaService转换为具体类型");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error(ErrorResponse {
                    code: "SERVICE_TYPE_ERROR".to_string(),
                    message: "服务类型转换失败".to_string(),
                    details: None,
                    timestamp: chrono::Utc::now().timestamp(),
                })),
            )
        })?;

    match concrete_service.referral.get_upper(request).await {
        Ok(response) => {
            info!("✅ 成功查询上级推荐人");
            Ok(Json(ApiResponse::success(response)))
        }
        Err(e) => {
            error!("❌ 查询上级推荐人失败: {}", e);
            let error_response = ErrorResponse {
                code: "GET_UPPER_FAILED".to_string(),
                message: format!("查询上级推荐人失败: {}", e),
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

/// 获取用户的上级推荐人并进行本地验证（本地测试用）
///
/// 查询用户在推荐系统中的上级推荐人信息，并返回完整的验证数据。主要用于本地测试和调试。
///
/// # 查询参数
///
/// - `user_wallet`: 用户钱包地址
///
/// # 响应示例
///
/// ```json
/// {
///   "id": "uuid-string",
///   "success": true,
///   "data": {
///     "user_wallet": "用户钱包地址",
///     "upper": "上级钱包地址",
///     "referral_account": "推荐账户PDA地址",
///     "status": "Success",
///     "timestamp": 1640995200,
///     "account_exists": true,
///     "referral_account_data": {
///       "user": "用户地址",
///       "upper": "上级用户地址",
///       "upper_upper": "上上级用户地址",
///       "nft_mint": "绑定的NFT mint地址",
///       "bump": 254
///     }
///   }
/// }
/// ```
#[utoipa::path(
    get,
    path = "/api/v1/solana/nft/referral/get-upper-and-verify",
    params(GetUpperRequest),
    responses(
        (status = 200, description = "成功查询并验证上级推荐人", body = ApiResponse<GetUpperAndVerifyResponse>),
        (status = 400, description = "请求参数错误", body = ApiResponse<ErrorResponse>),
        (status = 500, description = "服务器内部错误", body = ApiResponse<ErrorResponse>)
    ),
    tag = "Solana推荐系统"
)]
pub async fn get_upper_and_verify(
    Extension(services): Extension<Services>,
    Query(request): Query<GetUpperRequest>,
) -> Result<Json<ApiResponse<GetUpperAndVerifyResponse>>, (StatusCode, Json<ApiResponse<ErrorResponse>>)> {
    info!("🎯 接收到查询并验证上级推荐人请求");
    info!("  用户钱包: {}", request.user_wallet);

    // 验证请求参数
    if let Err(validation_errors) = request.validate() {
        let error_message = validation_errors
            .field_errors()
            .iter()
            .map(|(field, errors)| {
                let error_messages: Vec<String> = errors
                    .iter()
                    .map(|error| error.message.as_ref().unwrap_or(&std::borrow::Cow::Borrowed("Invalid value")).to_string())
                    .collect();
                format!("{}: {}", field, error_messages.join(", "))
            })
            .collect::<Vec<String>>()
            .join("; ");

        let error_response = ErrorResponse {
            code: "VALIDATION_ERROR".to_string(),
            message: format!("请求参数验证失败: {}", error_message),
            details: None,
            timestamp: chrono::Utc::now().timestamp(),
        };

        return Err((StatusCode::BAD_REQUEST, Json(ApiResponse::error(error_response))));
    }

    // 将DynSolanaService转换为具体的SolanaService以访问referral字段
    let concrete_service = services.solana
        .as_any()
        .downcast_ref::<SolanaService>()
        .ok_or_else(|| {
            error!("无法将SolanaService转换为具体类型");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error(ErrorResponse {
                    code: "SERVICE_TYPE_ERROR".to_string(),
                    message: "服务类型转换失败".to_string(),
                    details: None,
                    timestamp: chrono::Utc::now().timestamp(),
                })),
            )
        })?;

    match concrete_service.referral.get_upper_and_verify(request).await {
        Ok(response) => {
            info!("✅ 成功查询并验证上级推荐人，账户存在: {}", response.account_exists);
            Ok(Json(ApiResponse::success(response)))
        }
        Err(e) => {
            error!("❌ 查询并验证上级推荐人失败: {}", e);
            let error_response = ErrorResponse {
                code: "GET_UPPER_VERIFY_FAILED".to_string(),
                message: format!("查询并验证上级推荐人失败: {}", e),
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

/// 获取用户的MintCounter信息（查询但不签名）
///
/// 查询用户在推荐系统中的mint数量统计信息。100%复现CLI中GetMintCounter功能的业务逻辑。
///
/// # 查询参数
///
/// - `user_wallet`: 用户钱包地址
///
/// # 响应示例
///
/// ```json
/// {
///   "id": "uuid-string",
///   "success": true,
///   "data": {
///     "user_wallet": "用户钱包地址",
///     "total_mint": 10,
///     "remain_mint": 5,
///     "mint_counter_account": "MintCounter账户PDA地址",
///     "status": "Success",
///     "timestamp": 1640995200
///   }
/// }
/// ```
///
/// # 错误情况
///
/// - 如果用户还没有mint过NFT，`total_mint`和`remain_mint`为0，`status`为"AccountNotFound"
/// - 如果钱包地址格式无效，返回400错误
#[utoipa::path(
    get,
    path = "/api/v1/solana/nft/referral/get-mint-counter",
    params(GetMintCounterRequest),
    responses(
        (status = 200, description = "成功查询MintCounter信息", body = ApiResponse<GetMintCounterResponse>),
        (status = 400, description = "请求参数错误", body = ApiResponse<ErrorResponse>),
        (status = 500, description = "服务器内部错误", body = ApiResponse<ErrorResponse>)
    ),
    tag = "Solana推荐系统"
)]
pub async fn get_mint_counter(
    Extension(services): Extension<Services>,
    Query(request): Query<GetMintCounterRequest>,
) -> Result<Json<ApiResponse<GetMintCounterResponse>>, (StatusCode, Json<ApiResponse<ErrorResponse>>)> {
    info!("🎯 接收到查询MintCounter请求");
    info!("  用户钱包: {}", request.user_wallet);

    // 验证请求参数
    if let Err(validation_errors) = request.validate() {
        let error_message = validation_errors
            .field_errors()
            .iter()
            .map(|(field, errors)| {
                let error_messages: Vec<String> = errors
                    .iter()
                    .map(|error| error.message.as_ref().unwrap_or(&std::borrow::Cow::Borrowed("Invalid value")).to_string())
                    .collect();
                format!("{}: {}", field, error_messages.join(", "))
            })
            .collect::<Vec<String>>()
            .join("; ");

        let error_response = ErrorResponse {
            code: "VALIDATION_ERROR".to_string(),
            message: format!("请求参数验证失败: {}", error_message),
            details: None,
            timestamp: chrono::Utc::now().timestamp(),
        };

        return Err((StatusCode::BAD_REQUEST, Json(ApiResponse::error(error_response))));
    }

    // 将DynSolanaService转换为具体的SolanaService以访问referral字段
    let concrete_service = services.solana
        .as_any()
        .downcast_ref::<SolanaService>()
        .ok_or_else(|| {
            error!("无法将SolanaService转换为具体类型");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error(ErrorResponse {
                    code: "SERVICE_TYPE_ERROR".to_string(),
                    message: "服务类型转换失败".to_string(),
                    details: None,
                    timestamp: chrono::Utc::now().timestamp(),
                })),
            )
        })?;

    match concrete_service.referral.get_mint_counter(request).await {
        Ok(response) => {
            info!("✅ 成功查询MintCounter信息: total_mint={}, remain_mint={}", response.total_mint, response.remain_mint);
            Ok(Json(ApiResponse::success(response)))
        }
        Err(e) => {
            error!("❌ 查询MintCounter信息失败: {}", e);
            let error_response = ErrorResponse {
                code: "GET_MINT_COUNTER_FAILED".to_string(),
                message: format!("查询MintCounter信息失败: {}", e),
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

/// 获取用户的MintCounter信息并进行本地验证（本地测试用）
///
/// 查询用户在推荐系统中的mint数量统计信息，并返回完整的验证数据。主要用于本地测试和调试。
///
/// # 查询参数
///
/// - `user_wallet`: 用户钱包地址
///
/// # 响应示例
///
/// ```json
/// {
///   "id": "uuid-string",
///   "success": true,
///   "data": {
///     "user_wallet": "用户钱包地址",
///     "total_mint": 10,
///     "remain_mint": 5,
///     "mint_counter_account": "MintCounter账户PDA地址",
///     "status": "Success",
///     "timestamp": 1640995200,
///     "account_exists": true,
///     "mint_counter_data": {
///       "minter": "用户地址",
///       "total_mint": 10,
///       "remain_mint": 5,
///       "bump": 254
///     }
///   }
/// }
/// ```
#[utoipa::path(
    get,
    path = "/api/v1/solana/nft/referral/get-mint-counter-and-verify",
    params(GetMintCounterRequest),
    responses(
        (status = 200, description = "成功查询并验证MintCounter信息", body = ApiResponse<GetMintCounterAndVerifyResponse>),
        (status = 400, description = "请求参数错误", body = ApiResponse<ErrorResponse>),
        (status = 500, description = "服务器内部错误", body = ApiResponse<ErrorResponse>)
    ),
    tag = "Solana推荐系统"
)]
pub async fn get_mint_counter_and_verify(
    Extension(services): Extension<Services>,
    Query(request): Query<GetMintCounterRequest>,
) -> Result<Json<ApiResponse<GetMintCounterAndVerifyResponse>>, (StatusCode, Json<ApiResponse<ErrorResponse>>)> {
    info!("🎯 接收到查询并验证MintCounter请求");
    info!("  用户钱包: {}", request.user_wallet);

    // 验证请求参数
    if let Err(validation_errors) = request.validate() {
        let error_message = validation_errors
            .field_errors()
            .iter()
            .map(|(field, errors)| {
                let error_messages: Vec<String> = errors
                    .iter()
                    .map(|error| error.message.as_ref().unwrap_or(&std::borrow::Cow::Borrowed("Invalid value")).to_string())
                    .collect();
                format!("{}: {}", field, error_messages.join(", "))
            })
            .collect::<Vec<String>>()
            .join("; ");

        let error_response = ErrorResponse {
            code: "VALIDATION_ERROR".to_string(),
            message: format!("请求参数验证失败: {}", error_message),
            details: None,
            timestamp: chrono::Utc::now().timestamp(),
        };

        return Err((StatusCode::BAD_REQUEST, Json(ApiResponse::error(error_response))));
    }

    // 将DynSolanaService转换为具体的SolanaService以访问referral字段
    let concrete_service = services.solana
        .as_any()
        .downcast_ref::<SolanaService>()
        .ok_or_else(|| {
            error!("无法将SolanaService转换为具体类型");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error(ErrorResponse {
                    code: "SERVICE_TYPE_ERROR".to_string(),
                    message: "服务类型转换失败".to_string(),
                    details: None,
                    timestamp: chrono::Utc::now().timestamp(),
                })),
            )
        })?;

    match concrete_service.referral.get_mint_counter_and_verify(request).await {
        Ok(response) => {
            info!("✅ 成功查询并验证MintCounter信息，账户存在: {}, total_mint={}, remain_mint={}", 
                  response.account_exists, response.base.total_mint, response.base.remain_mint);
            Ok(Json(ApiResponse::success(response)))
        }
        Err(e) => {
            error!("❌ 查询并验证MintCounter信息失败: {}", e);
            let error_response = ErrorResponse {
                code: "GET_MINT_COUNTER_VERIFY_FAILED".to_string(),
                message: format!("查询并验证MintCounter信息失败: {}", e),
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