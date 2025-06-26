use utoipa::OpenApi;

// 导入所有需要文档化的组件
use database::{refer::model::Refer, reward::model::*, user::model::User};

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Coinfair Solana Backend API",
        description = "基于 Rust 和 Axum 的区块链推荐奖励系统 API 文档",
        version = "1.0.0",
        contact(
            name = "API Support",
            email = "support@coinfair.xyz"
        )
    ),
    paths(
        // Refer endpoints
        crate::api::refer_controller::get_upper,
        crate::api::refer_controller::get_uppers,
        crate::api::refer_controller::create_refers,
        // Reward endpoints
        crate::api::reward_controller::set_reward,
        crate::api::reward_controller::set_rewards,
        crate::api::reward_controller::get_reward,
        crate::api::reward_controller::get_rewards_by_day,
        crate::api::reward_controller::get_all_rewards,
        crate::api::reward_controller::set_all_rewards,
        crate::api::reward_controller::get_rank_rewards,
        crate::api::reward_controller::list_rewards_by_address,
        crate::api::reward_controller::mock_rewards,
        // User endpoints
        crate::api::user_controller::user,
        crate::api::user_controller::mock_users,
    ),
    components(
        schemas(
            // Database models
            database::refer::model::Refer,
            database::reward::model::Reward,
            database::reward::model::RewardItem,
            database::reward::model::RewardItemWithTime,
            database::user::model::User,
            // DTOs
            crate::dtos::refer_dto::SetRefersDto,
            crate::dtos::reward_dto::SetRewardDto,
            crate::dtos::reward_dto::SetRewardsDto,
            crate::dtos::reward_dto::MockRewardsDto,
            crate::dtos::user_dto::SetUsersDto
        )
    ),
    tags(
        (name = "refer", description = "推荐关系管理"),
        (name = "reward", description = "奖励系统"),
        (name = "user", description = "用户管理")
    )
)]
pub struct ApiDoc;