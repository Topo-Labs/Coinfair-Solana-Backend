use crate::{
    dtos::reward_dto::{MockRewardsDto, SetRewardDto, SetRewardsDto},
    extractors::validation_extractor::ValidationExtractor,
    services::Services,
};
use axum::{
    extract::Path,
    routing::{get, post},
    Extension, Json, Router,
};
use chrono::Utc;
use database::reward::model::{Reward, RewardItem, RewardItemWithTime};
use mongodb::results::{InsertManyResult, UpdateResult};
use utils::{AppError, AppResult};

pub struct RewardController;
impl RewardController {
    pub fn app() -> Router {
        Router::new()
            .route("/reward", post(Self::set_reward))
            .route("/rewards", post(Self::set_rewards))
            .route("/reward/:address", get(Self::get_reward)) // api 查询某个buyer所触发的奖励
            .route("/rewards_by_day/:day", get(Self::get_rewards_by_day))
            .route("/all_rewards", get(Self::get_all_rewards)) // api 查询所有待发放的奖励
            .route("/set_all_rewards", get(Self::set_all_rewards)) //Test
            .route("/rank_rewards", get(Self::get_rank_rewards)) // api 查询奖励榜单
            .route(
                "/list_rewards_by_address/:address",
                get(Self::list_rewards_by_address),
            ) // api查询某个地址的奖励历史
            .route("/mock_rewards", post(Self::mock_rewards))
    }

    pub async fn set_reward(
        Extension(services): Extension<Services>,
        ValidationExtractor(req): ValidationExtractor<SetRewardDto>,
    ) -> AppResult<Json<UpdateResult>> {
        let reward = services.reward.set_reward(req.address).await?;

        Ok(Json(reward))
    }

    pub async fn set_rewards(
        Extension(services): Extension<Services>,
        ValidationExtractor(req): ValidationExtractor<SetRewardsDto>,
    ) -> AppResult<Json<UpdateResult>> {
        let rewards = services.reward.set_rewards(req.addresses).await?;

        Ok(Json(rewards))
    }

    pub async fn get_reward(
        Extension(services): Extension<Services>,
        Path(address): Path<String>,
    ) -> AppResult<Json<Reward>> {
        match services.reward.get_reward(address.to_string()).await? {
            Some(reward) => Ok(Json(reward)),
            None => Err(AppError::NotFound(format!(
                "Reward with address {} not found.",
                address
            ))),
        }
    }

    pub async fn get_rewards_by_day(
        Extension(services): Extension<Services>,
        Path(day): Path<String>,
    ) -> AppResult<Json<Vec<RewardItem>>> {
        let rewards = services.reward.get_rewards_by_day(day.to_string()).await?;

        Ok(Json(rewards))
    }

    pub async fn get_rewards_by_today(
        Extension(services): Extension<Services>,
    ) -> AppResult<Json<Vec<RewardItem>>> {
        let today = Utc::now().date_naive().to_string();

        let rewards = services.reward.get_rewards_by_day(today).await?;

        Ok(Json(rewards))
    }

    pub async fn get_all_rewards(
        Extension(services): Extension<Services>,
    ) -> AppResult<Json<Vec<RewardItem>>> {
        let rewards = services.reward.get_all_rewards().await?;

        Ok(Json(rewards))
    }

    pub async fn set_all_rewards(
        Extension(services): Extension<Services>,
    ) -> AppResult<Json<UpdateResult>> {
        let rewards = services.reward.set_all_rewards().await?;

        Ok(Json(rewards))
    }

    pub async fn get_rank_rewards(
        Extension(services): Extension<Services>,
    ) -> AppResult<Json<Vec<RewardItem>>> {
        let rewards = services.reward.get_rank_rewards().await?;

        Ok(Json(rewards))
    }

    pub async fn list_rewards_by_address(
        Extension(services): Extension<Services>,
        Path(address): Path<String>,
    ) -> AppResult<Json<Vec<RewardItemWithTime>>> {
        let rewards = services
            .reward
            .list_rewards_by_address(address.to_string().to_lowercase())
            .await?;

        Ok(Json(rewards))
    }

    pub async fn mock_rewards(
        Extension(services): Extension<Services>,
        ValidationExtractor(req): ValidationExtractor<MockRewardsDto>,
    ) -> AppResult<Json<InsertManyResult>> {
        let rewards = services.reward.mock_rewards(req.rewards).await?;

        Ok(Json(rewards))
    }
}
