use database::clmm::reward::model::Reward;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::Validate;

/// 设置单个奖励的请求体
#[derive(Clone, Serialize, Deserialize, Debug, Validate, Default, ToSchema)]
pub struct SetRewardDto {
    /// 用户地址
    pub address: String,
}

/// 批量设置奖励的请求体
#[derive(Clone, Serialize, Deserialize, Debug, Validate, Default, ToSchema)]
pub struct SetRewardsDto {
    /// 用户地址列表
    pub addresses: Vec<String>,
}

/// 创建模拟奖励数据的请求体
#[derive(Clone, Serialize, Deserialize, Debug, Validate, Default, ToSchema)]
pub struct MockRewardsDto {
    /// 奖励记录列表
    pub rewards: Vec<Reward>,
}
