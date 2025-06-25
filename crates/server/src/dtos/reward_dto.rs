use database::reward::model::Reward;
use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Clone, Serialize, Deserialize, Debug, Validate, Default)]
pub struct SetRewardDto {
    pub address: String,
}

#[derive(Clone, Serialize, Deserialize, Debug, Validate, Default)]
pub struct SetRewardsDto {
    pub addresses: Vec<String>,
}

#[derive(Clone, Serialize, Deserialize, Debug, Validate, Default)]
pub struct MockRewardsDto {
    pub rewards: Vec<Reward>,
}
