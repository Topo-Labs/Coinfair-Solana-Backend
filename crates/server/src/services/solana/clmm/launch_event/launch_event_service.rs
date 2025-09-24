use crate::dtos::solana::clmm::events::launch_event::{LaunchEventResponse, LaunchEventStatsResponse};
use database::{event_model::repository::LaunchEventRepository, Database};
use std::sync::Arc;
use tracing::{debug, info};
use utils::{AppResult, AppError};

/// LaunchEvent服务，提供Launch事件的业务逻辑
#[derive(Debug, Clone)]
pub struct LaunchEventService {
    /// Launch事件仓库
    launch_event_repository: Arc<LaunchEventRepository>,
}

impl LaunchEventService {
    /// 创建新的LaunchEvent服务
    pub fn new(database: Arc<Database>) -> Self {
        Self {
            launch_event_repository: Arc::new(database.launch_event_repository.clone()),
        }
    }

    /// 根据签名获取Launch事件详情
    pub async fn get_launch_event_by_signature(&self, signature: &str) -> AppResult<Option<LaunchEventResponse>> {
        debug!("查询Launch事件详情: signature={}", signature);

        let event = self
            .launch_event_repository
            .find_by_signature(signature)
            .await
            .map_err(|e| AppError::InternalServerErrorWithContext(format!("查询Launch事件失败: {}", e)))?;

        match event {
            Some(launch_event) => {
                debug!("找到Launch事件: signature={}, status={}", signature, launch_event.migration_status);
                Ok(Some(launch_event.into()))
            }
            None => {
                debug!("未找到Launch事件: signature={}", signature);
                Ok(None)
            }
        }
    }

    /// 获取Launch事件统计信息
    pub async fn get_launch_event_stats(&self) -> AppResult<LaunchEventStatsResponse> {
        debug!("获取Launch事件统计信息");

        // 获取总Launch数量
        let total_launches = self
            .launch_event_repository
            .count_total_launches()
            .await
            .map_err(|e| AppError::InternalServerErrorWithContext(format!("获取总Launch数量失败: {}", e)))?;

        // 获取迁移成功率
        let migration_success_rate = self
            .launch_event_repository
            .get_migration_success_rate()
            .await
            .map_err(|e| AppError::InternalServerErrorWithContext(format!("获取迁移成功率失败: {}", e)))?;

        // 获取各种状态的真实数量
        let pending_count = self
            .launch_event_repository
            .count_pending_migrations()
            .await
            .map_err(|e| AppError::InternalServerErrorWithContext(format!("获取待迁移数量失败: {}", e)))?;

        let success_count = self
            .launch_event_repository
            .count_success_migrations()
            .await
            .map_err(|e| AppError::InternalServerErrorWithContext(format!("获取成功迁移数量失败: {}", e)))?;

        let failed_count = self
            .launch_event_repository
            .count_failed_migrations()
            .await
            .map_err(|e| AppError::InternalServerErrorWithContext(format!("获取失败迁移数量失败: {}", e)))?;

        let retrying_count = self
            .launch_event_repository
            .count_retrying_migrations()
            .await
            .map_err(|e| AppError::InternalServerErrorWithContext(format!("获取重试迁移数量失败: {}", e)))?;

        let stats = LaunchEventStatsResponse {
            total_launches,
            migration_success_rate,
            pending_count,
            success_count,
            failed_count,
            retrying_count,
        };

        info!(
            "Launch事件统计信息: total={}, success_rate={:.2}%, pending={}, success={}, failed={}, retrying={}",
            stats.total_launches,
            stats.migration_success_rate,
            stats.pending_count,
            stats.success_count,
            stats.failed_count,
            stats.retrying_count
        );

        Ok(stats)
    }

    /// 获取待迁移的Launch事件列表
    pub async fn get_pending_migrations(&self) -> AppResult<Vec<LaunchEventResponse>> {
        debug!("获取待迁移的Launch事件列表");

        let pending_events = self
            .launch_event_repository
            .find_pending_migrations()
            .await
            .map_err(|e| AppError::InternalServerErrorWithContext(format!("查询待迁移事件失败: {}", e)))?;

        let response_events: Vec<LaunchEventResponse> = pending_events.into_iter().map(Into::into).collect();

        info!("待迁移Launch事件查询完成: count={}", response_events.len());

        Ok(response_events)
    }

    /// 获取需要重试的失败Launch事件列表
    pub async fn get_failed_migrations_for_retry(&self) -> AppResult<Vec<LaunchEventResponse>> {
        debug!("获取需要重试的失败Launch事件列表");

        let failed_events = self
            .launch_event_repository
            .find_failed_migrations_for_retry()
            .await
            .map_err(|e| AppError::InternalServerErrorWithContext(format!("查询需要重试的事件失败: {}", e)))?;

        let response_events: Vec<LaunchEventResponse> = failed_events.into_iter().map(Into::into).collect();

        info!("需要重试的失败Launch事件查询完成: count={}", response_events.len());

        Ok(response_events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use database::event_model::LaunchEvent;
    use chrono::Utc;

    fn create_mock_launch_event() -> LaunchEvent {
        LaunchEvent {
            id: None,
            meme_token_mint: "So11111111111111111111111111111111111111112".to_string(),
            base_token_mint: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(),
            user_wallet: "8S2bcP66WehuF6cHryfZ7vfFpQWaUhYyAYSy5U3gX4Fy".to_string(),
            config_index: 1,
            initial_price: 0.001,
            tick_lower_price: 0.0005,
            tick_upper_price: 0.002,
            meme_token_amount: 1000000,
            base_token_amount: 1000,
            max_slippage_percent: 1.0,
            with_metadata: true,
            open_time: 0,
            launched_at: Utc::now().timestamp(),
            migration_status: "pending".to_string(),
            migrated_pool_address: None,
            migration_completed_at: None,
            migration_error: None,
            migration_retry_count: 0,
            total_liquidity_usd: 1000.0,
            pair_type: "MemeToUsdc".to_string(),
            price_range_width_percent: 300.0,
            is_high_value_launch: true,
            signature: "test_signature_123".to_string(),
            slot: 12345,
            processed_at: Utc::now().timestamp(),
            updated_at: Utc::now().timestamp(),
        }
    }

    #[tokio::test]
    async fn test_launch_event_response_conversion() {
        let launch_event = create_mock_launch_event();
        let response: LaunchEventResponse = launch_event.into();

        assert_eq!(response.meme_token_mint, "So11111111111111111111111111111111111111112");
        assert_eq!(response.base_token_mint, "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
        assert_eq!(response.user_wallet, "8S2bcP66WehuF6cHryfZ7vfFpQWaUhYyAYSy5U3gX4Fy");
        assert_eq!(response.migration_status, "pending");
        assert_eq!(response.pair_type, "MemeToUsdc");
        assert_eq!(response.signature, "test_signature_123");
        assert!(response.is_high_value_launch);
    }

    #[test]
    fn test_launch_event_stats_response_creation() {
        let stats = LaunchEventStatsResponse {
            total_launches: 100,
            migration_success_rate: 85.5,
            pending_count: 10,
            success_count: 85,
            failed_count: 3,
            retrying_count: 2,
        };

        assert_eq!(stats.total_launches, 100);
        assert_eq!(stats.migration_success_rate, 85.5);
        assert_eq!(stats.success_count, 85);
        assert_eq!(stats.pending_count + stats.success_count + stats.failed_count + stats.retrying_count, 100);
    }
}