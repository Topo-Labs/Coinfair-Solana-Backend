use chrono::prelude::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct Reward {
    pub is_rewarded: bool,        // Address
    pub user_address: String,     // Address(可作为唯一标识)
    pub rewards: Vec<RewardItem>, // [{address1, 800}, {address2, 200}]
    pub timestamp: u64,           // 2024-10-01T04:50:42.849324741Z
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct RewardItem {
    pub address: String, // Address
    pub amount: f64,     // 200
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct RewardItemWithTime {
    pub address: String, // Address
    pub amount: f64,     // 200
    pub timestamp: u64,
    pub user_address: String,
}
