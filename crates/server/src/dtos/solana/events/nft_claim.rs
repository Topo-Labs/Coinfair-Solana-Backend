use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use validator::Validate;

use crate::dtos::solana::common::{default_page, default_page_size, validate_sort_order};

/// NFT领取事件查询参数
#[derive(Debug, Clone, Serialize, Deserialize, Validate, IntoParams, ToSchema)]
pub struct NftClaimEventQuery {
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

    /// NFT等级过滤（1-5）
    #[validate(range(min = 1, max = 5))]
    pub tier: Option<u8>,

    /// 是否有推荐人
    pub has_referrer: Option<bool>,

    /// 开始日期时间戳
    pub start_date: Option<i64>,

    /// 结束日期时间戳
    pub end_date: Option<i64>,
}

/// NFT领取事件高级查询参数
#[derive(Debug, Clone, Serialize, Deserialize, Validate, IntoParams, ToSchema)]
pub struct NftClaimAdvancedQuery {
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

    /// NFT等级过滤（1-5）
    #[validate(range(min = 1, max = 5))]
    pub tier: Option<u8>,

    /// 是否有推荐人
    pub has_referrer: Option<bool>,

    /// 开始日期时间戳
    pub start_date: Option<i64>,

    /// 结束日期时间戳
    pub end_date: Option<i64>,

    /// 推荐人地址过滤
    pub referrer: Option<String>,

    /// 领取者地址过滤
    pub claimer: Option<String>,

    /// NFT mint地址过滤
    pub nft_mint: Option<String>,

    /// 最小奖励金额过滤
    #[validate(range(min = 0))]
    pub claim_amount_min: Option<u64>,

    /// 最大奖励金额过滤
    #[validate(range(min = 0))]
    pub claim_amount_max: Option<u64>,

    /// 领取类型过滤
    pub claim_type: Option<u8>,

    /// 是否为紧急领取
    pub is_emergency_claim: Option<bool>,

    /// 池子地址过滤
    pub pool_address: Option<String>,

    /// 代币mint地址过滤
    pub token_mint: Option<String>,

    /// 最小奖励倍率过滤
    #[validate(range(min = 0))]
    pub reward_multiplier_min: Option<u16>,

    /// 最大奖励倍率过滤
    #[validate(range(min = 0))]
    pub reward_multiplier_max: Option<u16>,
}

/// NFT领取事件响应
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct NftClaimEventResponse {
    /// NFT的mint地址
    pub nft_mint: String,

    /// 领取者钱包地址
    pub claimer: String,

    /// 推荐人地址（可选）
    pub referrer: Option<String>,

    /// NFT等级
    pub tier: u8,

    /// 等级名称
    pub tier_name: String,

    /// 领取的代币数量
    pub claim_amount: u64,

    /// 实际奖励金额
    pub bonus_amount: u64,

    /// 是否有推荐人
    pub has_referrer: bool,

    /// 预估USD价值
    pub estimated_usd_value: f64,

    /// 领取时间戳
    pub claimed_at: String,

    /// 交易签名
    pub signature: String,
}

/// NFT领取统计响应
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct NftClaimStatsResponse {
    /// 总领取次数
    pub total_claims: u64,

    /// 今日领取次数
    pub today_claims: u64,

    /// 等级分布 (等级, 数量, 总金额)
    pub tier_distribution: Vec<TierDistribution>,
}

/// 等级分布信息
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TierDistribution {
    /// 等级
    pub tier: u8,

    /// 该等级的领取数量
    pub count: u64,

    /// 该等级的总金额
    pub total_amount: u64,
}

/// 用户NFT领取汇总响应
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UserNftClaimSummaryResponse {
    /// 领取者地址
    pub claimer: String,

    /// 总领取次数
    pub total_claims: u64,

    /// 总领取金额
    pub total_claim_amount: u64,

    /// 总奖励金额
    pub total_bonus_amount: u64,

    /// 有推荐人的领取次数
    pub claims_with_referrer: u64,

    /// 等级分布
    pub tier_distribution: Vec<(u8, u32)>,
}
