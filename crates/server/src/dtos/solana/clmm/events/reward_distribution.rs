use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use validator::Validate;

use crate::dtos::solana::common::{default_page, default_page_size, validate_sort_order};

/// 奖励分发事件查询参数
#[derive(Debug, Clone, Serialize, Deserialize, Validate, IntoParams, ToSchema)]
pub struct RewardDistributionEventQuery {
    /// 页码（从1开始）
    #[validate(range(min = 1))]
    #[serde(default = "default_page")]
    pub page: u64,

    /// 每页条数（最大100）
    #[validate(range(min = 1, max = 100))]
    #[serde(default = "default_page_size")]
    pub page_size: u64,

    /// 排序字段
    pub sort_by: Option<String>,

    /// 排序方向（asc/desc）
    #[validate(custom = "validate_sort_order")]
    pub sort_order: Option<String>,

    /// 是否锁定
    pub is_locked: Option<bool>,

    /// 奖励类型
    pub reward_type: Option<u8>,

    /// 奖励来源
    pub reward_source: Option<u8>,

    /// 是否为推荐奖励
    pub is_referral_reward: Option<bool>,

    /// 开始日期时间戳
    pub start_date: Option<i64>,

    /// 结束日期时间戳
    pub end_date: Option<i64>,
}

/// 奖励分发事件高级查询参数
#[derive(Debug, Clone, Serialize, Deserialize, Validate, IntoParams, ToSchema)]
pub struct RewardDistributionAdvancedQuery {
    /// 页码（从1开始）
    #[validate(range(min = 1))]
    #[serde(default = "default_page")]
    pub page: u64,

    /// 每页条数（最大100）
    #[validate(range(min = 1, max = 100))]
    #[serde(default = "default_page_size")]
    pub page_size: u64,

    /// 排序字段
    pub sort_by: Option<String>,

    /// 排序方向（asc/desc）
    #[validate(custom = "validate_sort_order")]
    pub sort_order: Option<String>,

    /// 是否锁定
    pub is_locked: Option<bool>,

    /// 奖励类型
    pub reward_type: Option<u8>,

    /// 奖励来源
    pub reward_source: Option<u8>,

    /// 是否为推荐奖励
    pub is_referral_reward: Option<bool>,

    /// 开始日期时间戳
    pub start_date: Option<i64>,

    /// 结束日期时间戳
    pub end_date: Option<i64>,

    /// 推荐人地址过滤
    pub referrer: Option<String>,

    /// 接收者地址过滤
    pub recipient: Option<String>,

    /// 奖励代币mint地址过滤
    pub reward_token_mint: Option<String>,

    /// 最小奖励金额过滤
    #[validate(range(min = 0))]
    pub reward_amount_min: Option<u64>,

    /// 最大奖励金额过滤
    #[validate(range(min = 0))]
    pub reward_amount_max: Option<u64>,

    /// 最小分发ID过滤
    #[validate(range(min = 0))]
    pub distribution_id_min: Option<i64>,

    /// 最大分发ID过滤
    #[validate(range(min = 0))]
    pub distribution_id_max: Option<i64>,

    /// 奖励池地址过滤
    pub reward_pool: Option<String>,

    /// 是否有推荐人
    pub has_referrer: Option<bool>,

    /// 是否为高价值奖励
    pub is_high_value_reward: Option<bool>,

    /// 最小锁定天数
    #[validate(range(min = 0))]
    pub lock_days_min: Option<u64>,

    /// 最大锁定天数
    #[validate(range(min = 0))]
    pub lock_days_max: Option<u64>,

    /// 最小奖励倍率（基点）
    #[validate(range(min = 0))]
    pub multiplier_min: Option<u16>,

    /// 最大奖励倍率（基点）
    #[validate(range(min = 0))]
    pub multiplier_max: Option<u16>,

    /// 相关地址过滤
    pub related_address: Option<String>,

    /// 最小预估USD价值
    #[validate(range(min = 0.0))]
    pub estimated_usd_min: Option<f64>,

    /// 最大预估USD价值
    #[validate(range(min = 0.0))]
    pub estimated_usd_max: Option<f64>,
}

/// 通用分页响应
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct EventPaginatedResponse<T> {
    /// 数据项列表
    pub items: Vec<T>,

    /// 总记录数
    pub total: u64,

    /// 当前页码
    pub page: u64,

    /// 每页条数
    pub page_size: u64,

    /// 总页数
    pub total_pages: u64,
}

/// 奖励分发事件响应
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RewardDistributionEventResponse {
    /// 奖励分发ID
    pub distribution_id: i64,

    /// 接收者钱包地址
    pub recipient: String,

    /// 推荐人地址（可选）
    pub referrer: Option<String>,

    /// 奖励代币mint地址
    pub reward_token_mint: String,

    /// 奖励代币小数位数
    pub reward_token_decimals: Option<u8>,

    /// 奖励代币名称
    pub reward_token_name: Option<String>,

    /// 奖励代币符号
    pub reward_token_symbol: Option<String>,

    /// 奖励代币Logo URI
    pub reward_token_logo_uri: Option<String>,

    /// 奖励数量
    pub reward_amount: u64,

    /// 奖励类型名称
    pub reward_type_name: String,

    /// 是否已锁定
    pub is_locked: bool,

    /// 解锁时间戳
    pub unlock_timestamp: Option<String>,

    /// 是否为推荐奖励
    pub is_referral_reward: bool,

    /// 预估USD价值
    pub estimated_usd_value: f64,

    /// 发放时间戳
    pub distributed_at: String,

    /// 交易签名
    pub signature: String,
}

/// 奖励分发统计响应
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RewardStatsResponse {
    /// 总分发次数
    pub total_distributions: u64,

    /// 今日分发次数
    pub today_distributions: u64,

    /// 锁定中的奖励数量
    pub locked_rewards: u64,

    /// 奖励类型分布
    pub reward_type_distribution: Vec<RewardTypeDistribution>,
}

/// 奖励类型分布信息
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RewardTypeDistribution {
    /// 奖励类型
    pub reward_type: u8,

    /// 该类型的分发数量
    pub count: u64,

    /// 该类型的总金额
    pub total_amount: u64,
}

/// 用户奖励汇总响应
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UserRewardSummaryResponse {
    /// 接收者地址
    pub recipient: String,

    /// 总奖励次数
    pub total_rewards: u64,

    /// 总奖励金额
    pub total_amount: u64,

    /// 锁定金额
    pub locked_amount: u64,

    /// 未锁定金额
    pub unlocked_amount: u64,

    /// 推荐奖励次数
    pub referral_rewards: u64,

    /// 推荐奖励金额
    pub referral_amount: u64,
}
