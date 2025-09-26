use crate::services::Services;
use axum::{
    extract::{Extension, Path},
    response::Json,
    routing::get,
    Router,
};
use tracing::{debug, error, info};
use utils::AppError;
use crate::dtos::solana::clmm::events::launch_event::{LaunchEventResponse, LaunchEventStatsResponse};

/// LaunchEvent控制器
pub struct LaunchEventController;

impl LaunchEventController {
    /// 创建LaunchEvent路由
    pub fn routes() -> Router {
        Router::new()
            .route("/:signature", get(get_launch_event_by_signature))
            .route("/stats", get(get_launch_event_stats))
            .route("/pending", get(get_pending_migrations))
            .route("/failed-retry", get(get_failed_migrations_for_retry))
    }
}

/// 根据签名获取Launch事件详情
///
/// 根据交易签名查询特定的Launch事件详细信息，包括迁移状态、流动性信息等。
#[utoipa::path(
    get,
    path = "/api/v1/solana/events/launch/{signature}",
    params(
        ("signature" = String, Path, description = "交易签名", example = "5VfYuQwKwqSL8yVv4DjKQAXZEh7mEDTLmBVrCfPdNwvS5YnkFqHqhg9J8QkZsHxN2WqV")
    ),
    responses(
        (status = 200, description = "成功获取Launch事件详情", body = LaunchEventResponse),
        (status = 404, description = "未找到指定签名的Launch事件"),
        (status = 500, description = "服务器内部错误")
    ),
    tag = "LaunchEvent"
)]
pub async fn get_launch_event_by_signature(
        Extension(services): Extension<Services>,
        Path(signature): Path<String>,
    ) -> Result<Json<LaunchEventResponse>, AppError> {
        debug!("API请求: 查询Launch事件详情 signature={}", signature);

        match services
            .launch_event
            .get_launch_event_by_signature(&signature)
            .await
        {
            Ok(Some(event)) => {
                info!("成功获取Launch事件详情: signature={}", signature);
                Ok(Json(event))
            }
            Ok(None) => {
                info!("未找到Launch事件: signature={}", signature);
                Err(AppError::NotFound(format!(
                    "未找到签名为 {} 的Launch事件",
                    signature
                )))
            }
            Err(e) => {
                error!("获取Launch事件详情失败: signature={}, error={}", signature, e);
                Err(e)
            }
        }
    }

/// 获取Launch事件统计信息
///
/// 获取系统中所有Launch事件的统计信息，包括总数、成功率、各状态数量等。
#[utoipa::path(
    get,
    path = "/api/v1/solana/events/launch/stats",
    responses(
        (status = 200, description = "成功获取Launch事件统计信息", body = LaunchEventStatsResponse),
        (status = 500, description = "服务器内部错误")
    ),
    tag = "LaunchEvent"
)]
pub async fn get_launch_event_stats(
        Extension(services): Extension<Services>,
    ) -> Result<Json<LaunchEventStatsResponse>, AppError> {
        debug!("API请求: 获取Launch事件统计信息");

        match services.launch_event.get_launch_event_stats().await {
            Ok(stats) => {
                info!(
                    "成功获取Launch事件统计信息: total={}, success_rate={:.2}%",
                    stats.total_launches, stats.migration_success_rate
                );
                Ok(Json(stats))
            }
            Err(e) => {
                error!("获取Launch事件统计信息失败: error={}", e);
                Err(e)
            }
        }
    }

/// 获取待迁移的Launch事件列表
///
/// 获取所有状态为待迁移的Launch事件列表，用于监控和管理迁移流程。
#[utoipa::path(
    get,
    path = "/api/v1/solana/events/launch/pending",
    responses(
        (status = 200, description = "成功获取待迁移Launch事件列表", body = Vec<LaunchEventResponse>),
        (status = 500, description = "服务器内部错误")
    ),
    tag = "LaunchEvent"
)]
pub async fn get_pending_migrations(
        Extension(services): Extension<Services>,
    ) -> Result<Json<Vec<LaunchEventResponse>>, AppError> {
        debug!("API请求: 获取待迁移Launch事件列表");

        match services.launch_event.get_pending_migrations().await {
            Ok(events) => {
                info!(
                    "成功获取待迁移Launch事件列表: count={}",
                    events.len()
                );
                Ok(Json(events))
            }
            Err(e) => {
                error!("获取待迁移Launch事件列表失败: error={}", e);
                Err(e)
            }
        }
    }

/// 获取需要重试的失败Launch事件列表
///
/// 获取所有迁移失败但还可以重试的Launch事件列表。
#[utoipa::path(
    get,
    path = "/api/v1/solana/events/launch/failed-retry",
    responses(
        (status = 200, description = "成功获取需要重试的失败Launch事件列表", body = Vec<LaunchEventResponse>),
        (status = 500, description = "服务器内部错误")
    ),
    tag = "LaunchEvent"
)]
pub async fn get_failed_migrations_for_retry(
        Extension(services): Extension<Services>,
    ) -> Result<Json<Vec<LaunchEventResponse>>, AppError> {
        debug!("API请求: 获取需要重试的失败Launch事件列表");

        match services
            .launch_event
            .get_failed_migrations_for_retry()
            .await
        {
            Ok(events) => {
                info!(
                    "成功获取需要重试的失败Launch事件列表: count={}",
                    events.len()
                );
                Ok(Json(events))
            }
            Err(e) => {
                error!("获取需要重试的失败Launch事件列表失败: error={}", e);
                Err(e)
            }
        }
    }

