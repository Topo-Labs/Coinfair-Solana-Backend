use crate::dtos::solana::common::{ApiResponse, ErrorResponse};
use crate::dtos::solana::launch::{
    LaunchMigrationAndSendTransactionResponse, LaunchMigrationRequest, LaunchMigrationResponse,
    UserLaunchHistoryParams, UserLaunchHistoryResponse, LaunchMigrationStatsResponse,
    PaginationInfo,
};
use crate::{extractors::validation_extractor::ValidationExtractor, services::Services};
use axum::{
    extract::{Extension, Query},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use tracing::{error, info};

pub struct LaunchMigrationController;

impl LaunchMigrationController {
    pub fn routes() -> Router {
        Router::new()
            // 构建发射迁移交易（不签名不发送）
            .route("/launch", post(launch_migration))
            // 构建并发送发射迁移交易（用于测试）
            .route(
                "/launch-and-send-transaction",
                post(launch_migration_and_send_transaction),
            )
            // 专门用于事件监听器调用的端点（简化路径）
            .route("/send", post(launch_migration_and_send_transaction))
            // 查询用户Launch Migration历史
            .route("/history", get(get_user_launch_history))
            // 获取Launch Migration统计信息
            .route("/stats", get(get_launch_stats))
    }
}

/// 构建发射迁移交易
async fn launch_migration(
    Extension(services): Extension<Services>,
    ValidationExtractor(request): ValidationExtractor<LaunchMigrationRequest>,
) -> Result<Json<ApiResponse<LaunchMigrationResponse>>, (StatusCode, Json<ErrorResponse>)> {
    info!(
        "🚀 收到发射迁移请求: user_wallet={}, meme_token={}",
        request.user_wallet, request.meme_token_mint
    );

    match services.solana.launch_migration(request).await {
        Ok(response) => {
            info!("✅ 发射迁移交易构建成功");
            Ok(Json(ApiResponse::success(response)))
        }
        Err(e) => {
            error!("❌ 发射迁移交易构建失败: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(
                    "LAUNCH_MIGRATION_FAILED",
                    &format!("发射迁移交易构建失败: {}", e),
                )),
            ))
        }
    }
}

/// 构建并发送发射迁移交易
async fn launch_migration_and_send_transaction(
    Extension(services): Extension<Services>,
    ValidationExtractor(request): ValidationExtractor<LaunchMigrationRequest>,
) -> Result<Json<ApiResponse<LaunchMigrationAndSendTransactionResponse>>, (StatusCode, Json<ErrorResponse>)> {
    info!(
        "🚀 收到发射迁移并发送交易请求: user_wallet={}, meme_token={}",
        request.user_wallet, request.meme_token_mint
    );

    match services.solana.launch_migration_and_send_transaction(request).await {
        Ok(response) => {
            info!("✅ 发射迁移交易发送成功，签名: {}", response.signature);
            Ok(Json(ApiResponse::success(response)))
        }
        Err(e) => {
            error!("❌ 发射迁移交易发送失败: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(
                    "LAUNCH_MIGRATION_SEND_FAILED",
                    &format!("发射迁移交易发送失败: {}", e),
                )),
            ))
        }
    }
}

/// 查询用户Launch Migration历史
async fn get_user_launch_history(
    Extension(services): Extension<Services>,
    Query(params): Query<UserLaunchHistoryParams>,
) -> Result<Json<ApiResponse<UserLaunchHistoryResponse>>, (StatusCode, Json<ErrorResponse>)> {
    info!(
        "🔍 收到用户Launch Migration历史查询请求: creator_wallet={}",
        params.creator_wallet
    );

    // 参数验证和默认值处理
    let page = params.page.unwrap_or(1);
    let limit = match params.limit.unwrap_or(10) {
        0 => {
            // GitHub风格：返回错误
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse::new(
                    "INVALID_LIMIT",
                    "limit 参数必须大于 0",
                )),
            ));
        }
        l if l > 100 => {
            // 限制最大值，防止滥用
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse::new(
                    "LIMIT_TOO_LARGE",
                    "limit 参数不能超过 100",
                )),
            ));
        }
        l => l,
    };

    match tokio::try_join!(
        services.solana.get_user_launch_history(&params.creator_wallet, page, limit),
        services.solana.get_user_launch_history_count(&params.creator_wallet)
    ) {
        Ok((launches, total_count)) => {
            let total_pages = if total_count > 0 { (total_count + limit - 1) / limit } else { 0 };
            
            // 当没有数据时，导航逻辑应该都为false
            let (has_next, has_prev) = if total_count == 0 {
                (false, false)
            } else {
                // 更智能的导航逻辑：即使页面无效，也提供导航到有效页面的选项
                let is_valid_page = page > 0 && page <= total_pages;
                match (page, total_pages) {
                    // 有效页面：正常导航逻辑
                    _ if is_valid_page => (page < total_pages, page > 1),
                    // page=0或负数：可以去第1页
                    (0, _) => (total_pages > 0, false),
                    // 超出最大页面：可以回到最后一页
                    (p, tp) if p > tp => (false, tp > 0),
                    // 其他异常情况
                    _ => (false, false),
                }
            };
            
            let response = UserLaunchHistoryResponse {
                launches,
                total_count,
                pagination: PaginationInfo {
                    current_page: page,
                    page_size: limit,
                    total_count,
                    total_pages,
                    has_next,
                    has_prev,
                },
            };

            info!("✅ 用户Launch Migration历史查询成功，找到 {} 条记录（总共 {} 条）", response.launches.len(), total_count);
            Ok(Json(ApiResponse::success(response)))
        }
        Err(e) => {
            error!("❌ 用户Launch Migration历史查询失败: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(
                    "LAUNCH_HISTORY_QUERY_FAILED",
                    &format!("Launch Migration历史查询失败: {}", e),
                )),
            ))
        }
    }
}

/// 获取Launch Migration统计信息
async fn get_launch_stats(
    Extension(services): Extension<Services>,
) -> Result<Json<ApiResponse<LaunchMigrationStatsResponse>>, (StatusCode, Json<ErrorResponse>)> {
    info!("📊 收到Launch Migration统计信息查询请求");

    match services.solana.get_launch_stats().await {
        Ok(stats) => {
            let response = LaunchMigrationStatsResponse { stats };
            
            info!(
                "✅ Launch Migration统计查询成功: 总数={}, 成功数={}, 成功率={:.2}%",
                response.stats.total_launches,
                response.stats.successful_launches,
                response.stats.success_rate
            );
            Ok(Json(ApiResponse::success(response)))
        }
        Err(e) => {
            error!("❌ Launch Migration统计查询失败: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(
                    "LAUNCH_STATS_QUERY_FAILED",
                    &format!("Launch Migration统计查询失败: {}", e),
                )),
            ))
        }
    }
}
