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

/// 设置单个奖励
#[utoipa::path(
    post,
    path = "/api/v1/reward/reward",
    tag = "reward",
    request_body = SetRewardDto,
    responses(
        (status = 200, description = "成功设置奖励"),
        (status = 400, description = "请求参数错误")
    )
)]
pub async fn set_reward(
    Extension(services): Extension<Services>,
    ValidationExtractor(req): ValidationExtractor<SetRewardDto>,
) -> AppResult<Json<UpdateResult>> {
    let reward = services.reward.set_reward(req.address).await?;

    Ok(Json(reward))
}

/// 批量设置奖励
#[utoipa::path(
    post,
    path = "/api/v1/reward/rewards",
    tag = "reward",
    request_body = SetRewardsDto,
    responses(
        (status = 200, description = "成功批量设置奖励"),
        (status = 400, description = "请求参数错误")
    )
)]
pub async fn set_rewards(
    Extension(services): Extension<Services>,
    ValidationExtractor(req): ValidationExtractor<SetRewardsDto>,
) -> AppResult<Json<UpdateResult>> {
    let rewards = services.reward.set_rewards(req.addresses).await?;

    Ok(Json(rewards))
}

/// 获取单个用户的奖励信息
#[utoipa::path(
    get,
    path = "/api/v1/reward/{address}",
    tag = "reward",
    params(
        ("address" = String, Path, description = "用户钱包地址")
    ),
    responses(
        (status = 200, description = "成功返回奖励信息", body = Reward),
        (status = 404, description = "未找到奖励信息")
    )
)]
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

/// 获取指定日期的奖励列表
#[utoipa::path(
    get,
    path = "/api/v1/rewards_by_day/{day}",
    tag = "reward",
    params(
        ("day" = String, Path, description = "日期 (YYYY-MM-DD)")
    ),
    responses(
        (status = 200, description = "成功返回当日奖励列表", body = Vec<RewardItem>)
    )
)]
pub async fn get_rewards_by_day(
    Extension(services): Extension<Services>,
    Path(day): Path<String>,
) -> AppResult<Json<Vec<RewardItem>>> {
    let rewards = services.reward.get_rewards_by_day(day.to_string()).await?;

    Ok(Json(rewards))
}

/// 获取今日奖励列表
pub async fn get_rewards_by_today(
    Extension(services): Extension<Services>,
) -> AppResult<Json<Vec<RewardItem>>> {
    let today = Utc::now().date_naive().to_string();

    let rewards = services.reward.get_rewards_by_day(today).await?;

    Ok(Json(rewards))
}

/// 获取所有待发放的奖励
#[utoipa::path(
    get,
    path = "/api/v1/all_rewards",
    tag = "reward",
    responses(
        (status = 200, description = "成功返回所有待发放奖励", body = Vec<RewardItem>)
    )
)]
pub async fn get_all_rewards(
    Extension(services): Extension<Services>,
) -> AppResult<Json<Vec<RewardItem>>> {
    let rewards = services.reward.get_all_rewards().await?;

    Ok(Json(rewards))
}

/// 设置所有奖励（测试用）
#[utoipa::path(
    get,
    path = "/api/v1/set_all_rewards",
    tag = "reward",
    responses(
        (status = 200, description = "成功设置所有奖励")
    )
)]
pub async fn set_all_rewards(
    Extension(services): Extension<Services>,
) -> AppResult<Json<UpdateResult>> {
    let rewards = services.reward.set_all_rewards().await?;

    Ok(Json(rewards))
}

/// 获取奖励排行榜
#[utoipa::path(
    get,
    path = "/api/v1/rank_rewards",
    tag = "reward",
    responses(
        (status = 200, description = "成功返回奖励排行榜", body = Vec<RewardItem>)
    )
)]
pub async fn get_rank_rewards(
    Extension(services): Extension<Services>,
) -> AppResult<Json<Vec<RewardItem>>> {
    let rewards = services.reward.get_rank_rewards().await?;

    Ok(Json(rewards))
}

/// 获取指定地址的奖励历史
#[utoipa::path(
    get,
    path = "/api/v1/list_rewards_by_address/{address}",
    tag = "reward",
    params(
        ("address" = String, Path, description = "用户钱包地址")
    ),
    responses(
        (status = 200, description = "成功返回奖励历史", body = Vec<RewardItemWithTime>)
    )
)]
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

/// 模拟创建奖励数据
#[utoipa::path(
    post,
    path = "/api/v1/mock_rewards",
    tag = "reward",
    request_body = MockRewardsDto,
    responses(
        (status = 200, description = "成功创建模拟奖励数据"),
        (status = 400, description = "请求参数错误")
    )
)]
pub async fn mock_rewards(
    Extension(services): Extension<Services>,
    ValidationExtractor(req): ValidationExtractor<MockRewardsDto>,
) -> AppResult<Json<InsertManyResult>> {
    let rewards = services.reward.mock_rewards(req.rewards).await?;

    Ok(Json(rewards))
}

pub struct RewardController;
impl RewardController {
    pub fn app() -> Router {
        Router::new()
            .route("/reward", post(set_reward))
            .route("/rewards", post(set_rewards))
            .route("/reward/:address", get(get_reward)) // api 查询某个buyer所触发的奖励
            .route("/rewards_by_day/:day", get(get_rewards_by_day))
            .route("/all_rewards", get(get_all_rewards)) // api 查询所有待发放的奖励
            .route("/set_all_rewards", get(set_all_rewards)) //Test
            .route("/rank_rewards", get(get_rank_rewards)) // api 查询奖励榜单
            .route(
                "/list_rewards_by_address/:address",
                get(list_rewards_by_address),
            ) // api查询某个地址的奖励历史
            .route("/mock_rewards", post(mock_rewards))
    }
}
