use crate::dtos::solana_dto::{
    ApiResponse, ErrorResponse, EventPaginatedResponse, NftClaimEventQuery, NftClaimEventResponse, NftClaimStatsResponse, PaginationParams, RewardDistributionEventQuery,
    RewardDistributionEventResponse, RewardStatsResponse, RewardTypeDistribution, TierDistribution, UserNftClaimSummaryResponse, UserRewardSummaryResponse,
};
use crate::services::solana::event::EventService;
use crate::services::Services;

use axum::{
    extract::{Extension, Path, Query},
    http::StatusCode,
    response::Json,
    routing::get,
    Router,
};
use database::event_model::{NftClaimEvent, RewardDistributionEvent};
use tracing::{error, info};

pub struct EventController;

impl EventController {
    pub fn routes() -> Router {
        Router::new()
            // ============ NFT领取事件路由 ============
            .route("/nft-claims", get(get_nft_claim_events))
            .route("/nft-claims/stats", get(get_nft_claim_stats))
            .route("/nft-claims/by-claimer/:address", get(get_nft_claims_by_claimer))
            .route("/nft-claims/by-nft/:mint", get(get_nft_claims_by_nft))
            .route("/nft-claims/summary/:address", get(get_user_nft_claim_summary))
            // ============ 奖励分发事件路由 ============
            .route("/rewards", get(get_reward_events))
            .route("/rewards/stats", get(get_reward_stats))
            .route("/rewards/by-recipient/:address", get(get_rewards_by_recipient))
            .route("/rewards/by-id/:id", get(get_reward_by_distribution_id))
            .route("/rewards/summary/:address", get(get_user_reward_summary))
    }
}

// ==================== NFT领取事件接口 ====================

/// 查询NFT领取事件列表
///
/// 支持分页和多种过滤条件
///
/// # 请求参数
///
/// - `page`: 页码（默认1）
/// - `page_size`: 每页条数（默认20，最大100）
/// - `tier`: NFT等级过滤（1-5）
/// - `has_referrer`: 是否有推荐人
/// - `start_date`: 开始日期时间戳
/// - `end_date`: 结束日期时间戳
/// - `sort_by`: 排序字段
/// - `sort_order`: 排序方向（asc/desc）
#[utoipa::path(
    get,
    path = "/api/v1/solana/events/nft-claims",
    params(NftClaimEventQuery),
    responses(
        (status = 200, description = "查询成功", body = ApiResponse<EventPaginatedResponse<NftClaimEventResponse>>),
        (status = 400, description = "请求参数错误", body = ApiResponse<ErrorResponse>),
        (status = 500, description = "服务器内部错误", body = ApiResponse<ErrorResponse>)
    ),
    tag = "事件查询"
)]
pub async fn get_nft_claim_events(
    Extension(services): Extension<Services>,
    Query(params): Query<NftClaimEventQuery>,
) -> Result<Json<ApiResponse<EventPaginatedResponse<NftClaimEventResponse>>>, (StatusCode, Json<ApiResponse<ErrorResponse>>)> {
    info!("🔍 查询NFT领取事件列表");

    let event_service = EventService::new(services.database.clone());

    match event_service
        .get_nft_claim_events_paginated(
            Some(params.page),
            Some(params.page_size),
            params.tier,
            params.has_referrer,
            params.start_date,
            params.end_date,
            params.sort_by,
            params.sort_order,
        )
        .await
    {
        Ok(result) => {
            let response = convert_nft_claim_paginated_response(result);
            Ok(Json(ApiResponse::success(response)))
        }
        Err(e) => {
            error!("❌ 查询NFT领取事件失败: {}", e);
            let error_response = ErrorResponse {
                code: "QUERY_NFT_CLAIMS_FAILED".to_string(),
                message: format!("查询NFT领取事件失败: {}", e),
                details: None,
                timestamp: chrono::Utc::now().timestamp(),
            };
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error(error_response))))
        }
    }
}

/// 获取NFT领取统计信息
#[utoipa::path(
    get,
    path = "/api/v1/solana/events/nft-claims/stats",
    responses(
        (status = 200, description = "查询成功", body = ApiResponse<NftClaimStatsResponse>),
        (status = 500, description = "服务器内部错误", body = ApiResponse<ErrorResponse>)
    ),
    tag = "事件查询"
)]
pub async fn get_nft_claim_stats(Extension(services): Extension<Services>) -> Result<Json<ApiResponse<NftClaimStatsResponse>>, (StatusCode, Json<ApiResponse<ErrorResponse>>)> {
    info!("📊 获取NFT领取统计信息");

    let event_service = EventService::new(services.database.clone());

    match event_service.get_nft_claim_stats().await {
        Ok(stats) => {
            let response = NftClaimStatsResponse {
                total_claims: stats.total_claims,
                today_claims: stats.today_claims,
                tier_distribution: stats
                    .tier_distribution
                    .into_iter()
                    .map(|(tier, count, amount)| TierDistribution {
                        tier,
                        count,
                        total_amount: amount,
                    })
                    .collect(),
            };
            Ok(Json(ApiResponse::success(response)))
        }
        Err(e) => {
            error!("❌ 获取NFT领取统计失败: {}", e);
            let error_response = ErrorResponse {
                code: "GET_NFT_STATS_FAILED".to_string(),
                message: format!("获取NFT领取统计失败: {}", e),
                details: None,
                timestamp: chrono::Utc::now().timestamp(),
            };
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error(error_response))))
        }
    }
}

/// 根据领取者地址查询NFT领取事件
#[utoipa::path(
    get,
    path = "/api/v1/solana/events/nft-claims/by-claimer/{address}",
    params(
        ("address" = String, Path, description = "领取者钱包地址"),
        PaginationParams
    ),
    responses(
        (status = 200, description = "查询成功", body = ApiResponse<EventPaginatedResponse<NftClaimEventResponse>>),
        (status = 400, description = "请求参数错误", body = ApiResponse<ErrorResponse>),
        (status = 500, description = "服务器内部错误", body = ApiResponse<ErrorResponse>)
    ),
    tag = "事件查询"
)]
pub async fn get_nft_claims_by_claimer(
    Extension(services): Extension<Services>,
    Path(address): Path<String>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<ApiResponse<EventPaginatedResponse<NftClaimEventResponse>>>, (StatusCode, Json<ApiResponse<ErrorResponse>>)> {
    info!("🔍 查询领取者 {} 的NFT领取事件", address);

    let event_service = EventService::new(services.database.clone());

    match event_service
        .get_nft_claim_events_by_claimer(&address, Some(params.page), Some(params.page_size), params.sort_by, params.sort_order)
        .await
    {
        Ok(result) => {
            let response = convert_nft_claim_paginated_response(result);
            Ok(Json(ApiResponse::success(response)))
        }
        Err(e) => {
            error!("❌ 查询领取者NFT事件失败: {}", e);
            let error_response = ErrorResponse {
                code: "QUERY_CLAIMER_NFT_FAILED".to_string(),
                message: format!("查询领取者NFT事件失败: {}", e),
                details: None,
                timestamp: chrono::Utc::now().timestamp(),
            };
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error(error_response))))
        }
    }
}

/// 根据NFT mint地址查询领取事件
#[utoipa::path(
    get,
    path = "/api/v1/solana/events/nft-claims/by-nft/{mint}",
    params(
        ("mint" = String, Path, description = "NFT mint地址"),
        PaginationParams
    ),
    responses(
        (status = 200, description = "查询成功", body = ApiResponse<EventPaginatedResponse<NftClaimEventResponse>>),
        (status = 400, description = "请求参数错误", body = ApiResponse<ErrorResponse>),
        (status = 500, description = "服务器内部错误", body = ApiResponse<ErrorResponse>)
    ),
    tag = "事件查询"
)]
pub async fn get_nft_claims_by_nft(
    Extension(services): Extension<Services>,
    Path(mint): Path<String>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<ApiResponse<EventPaginatedResponse<NftClaimEventResponse>>>, (StatusCode, Json<ApiResponse<ErrorResponse>>)> {
    info!("🔍 查询NFT {} 的领取事件", mint);

    let event_service = EventService::new(services.database.clone());

    match event_service.get_nft_claim_events_by_nft_mint(&mint, Some(params.page), Some(params.page_size)).await {
        Ok(result) => {
            let response = convert_nft_claim_paginated_response(result);
            Ok(Json(ApiResponse::success(response)))
        }
        Err(e) => {
            error!("❌ 查询NFT领取事件失败: {}", e);
            let error_response = ErrorResponse {
                code: "QUERY_NFT_EVENTS_FAILED".to_string(),
                message: format!("查询NFT领取事件失败: {}", e),
                details: None,
                timestamp: chrono::Utc::now().timestamp(),
            };
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error(error_response))))
        }
    }
}

/// 获取用户NFT领取汇总信息
#[utoipa::path(
    get,
    path = "/api/v1/solana/events/nft-claims/summary/{address}",
    params(
        ("address" = String, Path, description = "用户钱包地址")
    ),
    responses(
        (status = 200, description = "查询成功", body = ApiResponse<UserNftClaimSummaryResponse>),
        (status = 500, description = "服务器内部错误", body = ApiResponse<ErrorResponse>)
    ),
    tag = "事件查询"
)]
pub async fn get_user_nft_claim_summary(
    Extension(services): Extension<Services>,
    Path(address): Path<String>,
) -> Result<Json<ApiResponse<UserNftClaimSummaryResponse>>, (StatusCode, Json<ApiResponse<ErrorResponse>>)> {
    info!("📊 获取用户 {} 的NFT领取汇总", address);

    let event_service = EventService::new(services.database.clone());

    match event_service.get_user_nft_claim_summary(&address).await {
        Ok(summary) => {
            let response = UserNftClaimSummaryResponse {
                claimer: summary.claimer,
                total_claims: summary.total_claims,
                total_claim_amount: summary.total_claim_amount,
                total_bonus_amount: summary.total_bonus_amount,
                claims_with_referrer: summary.claims_with_referrer,
                tier_distribution: summary.tier_distribution,
            };
            Ok(Json(ApiResponse::success(response)))
        }
        Err(e) => {
            error!("❌ 获取用户NFT领取汇总失败: {}", e);
            let error_response = ErrorResponse {
                code: "GET_NFT_SUMMARY_FAILED".to_string(),
                message: format!("获取用户NFT领取汇总失败: {}", e),
                details: None,
                timestamp: chrono::Utc::now().timestamp(),
            };
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error(error_response))))
        }
    }
}

// ==================== 奖励分发事件接口 ====================

/// 查询奖励分发事件列表
#[utoipa::path(
    get,
    path = "/api/v1/solana/events/rewards",
    params(RewardDistributionEventQuery),
    responses(
        (status = 200, description = "查询成功", body = ApiResponse<EventPaginatedResponse<RewardDistributionEventResponse>>),
        (status = 400, description = "请求参数错误", body = ApiResponse<ErrorResponse>),
        (status = 500, description = "服务器内部错误", body = ApiResponse<ErrorResponse>)
    ),
    tag = "事件查询"
)]
pub async fn get_reward_events(
    Extension(services): Extension<Services>,
    Query(params): Query<RewardDistributionEventQuery>,
) -> Result<Json<ApiResponse<EventPaginatedResponse<RewardDistributionEventResponse>>>, (StatusCode, Json<ApiResponse<ErrorResponse>>)> {
    info!("🔍 查询奖励分发事件列表");

    let event_service = EventService::new(services.database.clone());

    match event_service
        .get_reward_events_paginated(
            Some(params.page),
            Some(params.page_size),
            params.is_locked,
            params.reward_type,
            params.reward_source,
            params.is_referral_reward,
            params.start_date,
            params.end_date,
            params.sort_by,
            params.sort_order,
        )
        .await
    {
        Ok(result) => {
            let response = convert_reward_paginated_response(result);
            Ok(Json(ApiResponse::success(response)))
        }
        Err(e) => {
            error!("❌ 查询奖励分发事件失败: {}", e);
            let error_response = ErrorResponse {
                code: "QUERY_REWARDS_FAILED".to_string(),
                message: format!("查询奖励分发事件失败: {}", e),
                details: None,
                timestamp: chrono::Utc::now().timestamp(),
            };
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error(error_response))))
        }
    }
}

/// 获取奖励分发统计信息
#[utoipa::path(
    get,
    path = "/api/v1/solana/events/rewards/stats",
    responses(
        (status = 200, description = "查询成功", body = ApiResponse<RewardStatsResponse>),
        (status = 500, description = "服务器内部错误", body = ApiResponse<ErrorResponse>)
    ),
    tag = "事件查询"
)]
pub async fn get_reward_stats(Extension(services): Extension<Services>) -> Result<Json<ApiResponse<RewardStatsResponse>>, (StatusCode, Json<ApiResponse<ErrorResponse>>)> {
    info!("📊 获取奖励分发统计信息");

    let event_service = EventService::new(services.database.clone());

    match event_service.get_reward_stats().await {
        Ok(stats) => {
            let response = RewardStatsResponse {
                total_distributions: stats.total_distributions,
                today_distributions: stats.today_distributions,
                locked_rewards: stats.locked_rewards,
                reward_type_distribution: stats
                    .reward_type_distribution
                    .into_iter()
                    .map(|(reward_type, count, amount)| RewardTypeDistribution {
                        reward_type,
                        count,
                        total_amount: amount,
                    })
                    .collect(),
            };
            Ok(Json(ApiResponse::success(response)))
        }
        Err(e) => {
            error!("❌ 获取奖励分发统计失败: {}", e);
            let error_response = ErrorResponse {
                code: "GET_REWARD_STATS_FAILED".to_string(),
                message: format!("获取奖励分发统计失败: {}", e),
                details: None,
                timestamp: chrono::Utc::now().timestamp(),
            };
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error(error_response))))
        }
    }
}

/// 根据接收者地址查询奖励分发事件
#[utoipa::path(
    get,
    path = "/api/v1/solana/events/rewards/by-recipient/{address}",
    params(
        ("address" = String, Path, description = "接收者钱包地址"),
        PaginationParams
    ),
    responses(
        (status = 200, description = "查询成功", body = ApiResponse<EventPaginatedResponse<RewardDistributionEventResponse>>),
        (status = 400, description = "请求参数错误", body = ApiResponse<ErrorResponse>),
        (status = 500, description = "服务器内部错误", body = ApiResponse<ErrorResponse>)
    ),
    tag = "事件查询"
)]
pub async fn get_rewards_by_recipient(
    Extension(services): Extension<Services>,
    Path(address): Path<String>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<ApiResponse<EventPaginatedResponse<RewardDistributionEventResponse>>>, (StatusCode, Json<ApiResponse<ErrorResponse>>)> {
    info!("🔍 查询接收者 {} 的奖励分发事件", address);

    let event_service = EventService::new(services.database.clone());

    match event_service
        .get_reward_events_by_recipient(&address, Some(params.page), Some(params.page_size), None, None, params.sort_by, params.sort_order)
        .await
    {
        Ok(result) => {
            let response = convert_reward_paginated_response(result);
            Ok(Json(ApiResponse::success(response)))
        }
        Err(e) => {
            error!("❌ 查询接收者奖励事件失败: {}", e);
            let error_response = ErrorResponse {
                code: "QUERY_RECIPIENT_REWARDS_FAILED".to_string(),
                message: format!("查询接收者奖励事件失败: {}", e),
                details: None,
                timestamp: chrono::Utc::now().timestamp(),
            };
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error(error_response))))
        }
    }
}

/// 根据分发ID查询奖励事件
#[utoipa::path(
    get,
    path = "/api/v1/solana/events/rewards/by-id/{id}",
    params(
        ("id" = u64, Path, description = "奖励分发ID")
    ),
    responses(
        (status = 200, description = "查询成功", body = ApiResponse<RewardDistributionEventResponse>),
        (status = 404, description = "事件不存在", body = ApiResponse<ErrorResponse>),
        (status = 500, description = "服务器内部错误", body = ApiResponse<ErrorResponse>)
    ),
    tag = "事件查询"
)]
pub async fn get_reward_by_distribution_id(
    Extension(services): Extension<Services>,
    Path(id): Path<u64>,
) -> Result<Json<ApiResponse<RewardDistributionEventResponse>>, (StatusCode, Json<ApiResponse<ErrorResponse>>)> {
    info!("🔍 查询分发ID {} 的奖励事件", id);

    let event_service = EventService::new(services.database.clone());

    match event_service.get_reward_event_by_distribution_id(id).await {
        Ok(Some(event)) => {
            let response = convert_reward_event_to_response(event);
            Ok(Json(ApiResponse::success(response)))
        }
        Ok(None) => {
            let error_response = ErrorResponse {
                code: "REWARD_NOT_FOUND".to_string(),
                message: format!("奖励分发事件 {} 不存在", id),
                details: None,
                timestamp: chrono::Utc::now().timestamp(),
            };
            Err((StatusCode::NOT_FOUND, Json(ApiResponse::error(error_response))))
        }
        Err(e) => {
            error!("❌ 查询奖励事件失败: {}", e);
            let error_response = ErrorResponse {
                code: "QUERY_REWARD_FAILED".to_string(),
                message: format!("查询奖励事件失败: {}", e),
                details: None,
                timestamp: chrono::Utc::now().timestamp(),
            };
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error(error_response))))
        }
    }
}

/// 获取用户奖励汇总信息
#[utoipa::path(
    get,
    path = "/api/v1/solana/events/rewards/summary/{address}",
    params(
        ("address" = String, Path, description = "用户钱包地址")
    ),
    responses(
        (status = 200, description = "查询成功", body = ApiResponse<UserRewardSummaryResponse>),
        (status = 500, description = "服务器内部错误", body = ApiResponse<ErrorResponse>)
    ),
    tag = "事件查询"
)]
pub async fn get_user_reward_summary(
    Extension(services): Extension<Services>,
    Path(address): Path<String>,
) -> Result<Json<ApiResponse<UserRewardSummaryResponse>>, (StatusCode, Json<ApiResponse<ErrorResponse>>)> {
    info!("📊 获取用户 {} 的奖励汇总", address);

    let event_service = EventService::new(services.database.clone());

    match event_service.get_user_reward_summary(&address).await {
        Ok(summary) => {
            let response = UserRewardSummaryResponse {
                recipient: summary.recipient,
                total_rewards: summary.total_rewards,
                total_amount: summary.total_amount,
                locked_amount: summary.locked_amount,
                unlocked_amount: summary.unlocked_amount,
                referral_rewards: summary.referral_rewards,
                referral_amount: summary.referral_amount,
            };
            Ok(Json(ApiResponse::success(response)))
        }
        Err(e) => {
            error!("❌ 获取用户奖励汇总失败: {}", e);
            let error_response = ErrorResponse {
                code: "GET_REWARD_SUMMARY_FAILED".to_string(),
                message: format!("获取用户奖励汇总失败: {}", e),
                details: None,
                timestamp: chrono::Utc::now().timestamp(),
            };
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error(error_response))))
        }
    }
}

// ==================== 辅助函数 ====================

/// 转换NFT领取事件分页响应
fn convert_nft_claim_paginated_response(result: crate::services::solana::event::service::PaginatedResponse<NftClaimEvent>) -> EventPaginatedResponse<NftClaimEventResponse> {
    EventPaginatedResponse {
        items: result.items.into_iter().map(convert_nft_claim_to_response).collect(),
        total: result.total,
        page: result.page,
        page_size: result.page_size,
        total_pages: result.total_pages,
    }
}

/// 转换单个NFT领取事件到响应
fn convert_nft_claim_to_response(event: NftClaimEvent) -> NftClaimEventResponse {
    NftClaimEventResponse {
        nft_mint: event.nft_mint.to_string(),
        claimer: event.claimer.to_string(),
        referrer: event.referrer.map(|r| r.to_string()),
        tier: event.tier,
        tier_name: event.tier_name,
        claim_amount: event.claim_amount,
        bonus_amount: event.bonus_amount,
        has_referrer: event.has_referrer,
        estimated_usd_value: event.estimated_usd_value,
        claimed_at: event.claimed_at.to_string(),
        signature: event.signature,
    }
}

/// 转换奖励分发事件分页响应
fn convert_reward_paginated_response(
    result: crate::services::solana::event::service::PaginatedResponse<RewardDistributionEvent>,
) -> EventPaginatedResponse<RewardDistributionEventResponse> {
    EventPaginatedResponse {
        items: result.items.into_iter().map(convert_reward_event_to_response).collect(),
        total: result.total,
        page: result.page,
        page_size: result.page_size,
        total_pages: result.total_pages,
    }
}

/// 转换单个奖励分发事件到响应
fn convert_reward_event_to_response(event: RewardDistributionEvent) -> RewardDistributionEventResponse {
    RewardDistributionEventResponse {
        distribution_id: event.distribution_id,
        recipient: event.recipient.to_string(),
        referrer: event.referrer.map(|r| r.to_string()),
        reward_token_mint: event.reward_token_mint.to_string(),
        reward_amount: event.reward_amount,
        reward_type_name: event.reward_type_name,
        is_locked: event.is_locked,
        unlock_timestamp: event.unlock_timestamp.map(|t| t.to_string()),
        is_referral_reward: event.is_referral_reward,
        estimated_usd_value: event.estimated_usd_value,
        distributed_at: event.distributed_at.to_string(),
        signature: event.signature,
    }
}
