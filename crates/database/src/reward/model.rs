use chrono::prelude::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::Validate;

/// 奖励记录模型
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct Reward {
    /// 是否已发放奖励
    pub is_rewarded: bool,        // Address
    /// 用户地址（可作为唯一标识）
    pub user_address: String,     // Address(可作为唯一标识)
    /// 奖励项目列表
    pub rewards: Vec<RewardItem>, // [{address1, 800}, {address2, 200}]
    /// 创建时间戳
    pub timestamp: u64,           // 2024-10-01T04:50:42.849324741Z
}

/// 奖励项目
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct RewardItem {
    /// 接收奖励的地址
    pub address: String, // Address
    /// 奖励金额
    pub amount: f64,     // 200
}

/// 带时间的奖励项目
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct RewardItemWithTime {
    /// 接收奖励的地址
    pub address: String, // Address
    /// 奖励金额
    pub amount: f64,     // 200
    /// 时间戳
    pub timestamp: u64,
    /// 触发奖励的用户地址
    pub user_address: String,
}
