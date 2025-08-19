pub mod repository;

use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};

/// CLMM池子信息模型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClmmPoolEvent {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,

    /// 池子地址
    pub pool_address: String,

    /// 代币A的mint地址
    pub token_a_mint: String,

    /// 代币B的mint地址
    pub token_b_mint: String,

    /// 代币A的小数位数
    pub token_a_decimals: u8,

    /// 代币B的小数位数
    pub token_b_decimals: u8,

    /// 手续费率 (万分之一)
    pub fee_rate: u32,

    /// 手续费率百分比
    pub fee_rate_percentage: f64,

    /// 年化手续费率
    pub annual_fee_rate: f64,

    /// 池子类型
    pub pool_type: String,

    /// 初始sqrt价格
    pub sqrt_price_x64: String,

    /// 初始价格比率
    pub initial_price: f64,

    /// 初始tick
    pub initial_tick: i32,

    /// 池子创建者
    pub creator: String,

    /// CLMM配置地址
    pub clmm_config: String,

    /// 是否为稳定币对
    pub is_stable_pair: bool,

    /// 预估流动性价值(USD)
    pub estimated_liquidity_usd: f64,

    /// 创建时间戳
    pub created_at: i64,

    /// 交易签名
    pub signature: String,

    /// 区块高度
    pub slot: u64,

    /// 处理时间
    pub processed_at: i64,

    /// 最后更新时间
    pub updated_at: i64,
}

/// NFT领取事件模型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NftClaimEvent {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,

    /// NFT的mint地址
    pub nft_mint: String,

    /// 领取者钱包地址
    pub claimer: String,

    /// 推荐人地址（可选）
    pub referrer: Option<String>,

    /// NFT等级 (1-5级)
    pub tier: u8,

    /// 等级名称
    pub tier_name: String,

    /// 等级奖励倍率
    pub tier_bonus_rate: f64,

    /// 领取的代币数量
    pub claim_amount: u64,

    /// 代币mint地址
    pub token_mint: String,

    /// 奖励倍率 (基点)
    pub reward_multiplier: u16,

    /// 奖励倍率百分比
    pub reward_multiplier_percentage: f64,

    /// 实际奖励金额（包含倍率）
    pub bonus_amount: u64,

    /// 领取类型
    pub claim_type: u8,

    /// 领取类型名称
    pub claim_type_name: String,

    /// 累计领取量
    pub total_claimed: u64,

    /// 领取进度百分比
    pub claim_progress_percentage: f64,

    /// NFT所属的池子地址（可选）
    pub pool_address: Option<String>,

    /// 是否有推荐人
    pub has_referrer: bool,

    /// 是否为紧急领取
    pub is_emergency_claim: bool,

    /// 预估USD价值
    pub estimated_usd_value: f64,

    /// 领取时间戳
    pub claimed_at: i64,

    /// 交易签名
    pub signature: String,

    /// 区块高度
    pub slot: u64,

    /// 处理时间
    pub processed_at: i64,

    pub updated_at: i64,
}

/// 奖励分发事件模型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RewardDistributionEvent {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,

    /// 奖励分发ID
    pub distribution_id: i64,

    /// 奖励池地址
    pub reward_pool: String,

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

    /// 基础奖励金额
    pub base_reward_amount: u64,

    /// 额外奖励金额
    pub bonus_amount: u64,

    /// 奖励类型
    pub reward_type: u8,

    /// 奖励类型名称
    pub reward_type_name: String,

    /// 奖励来源
    pub reward_source: u8,

    /// 奖励来源名称
    pub reward_source_name: String,

    /// 相关地址
    pub related_address: Option<String>,

    /// 奖励倍率 (基点)
    pub multiplier: u16,

    /// 奖励倍率百分比
    pub multiplier_percentage: f64,

    /// 是否已锁定
    pub is_locked: bool,

    /// 锁定期结束时间戳
    pub unlock_timestamp: Option<i64>,

    /// 锁定天数
    pub lock_days: u64,

    /// 是否有推荐人
    pub has_referrer: bool,

    /// 是否为推荐奖励
    pub is_referral_reward: bool,

    /// 是否为高价值奖励
    pub is_high_value_reward: bool,

    /// 预估USD价值
    pub estimated_usd_value: f64,

    /// 发放时间戳
    pub distributed_at: i64,

    /// 交易签名
    pub signature: String,

    /// 区块高度
    pub slot: u64,

    /// 处理时间
    pub processed_at: i64,

    /// 最后更新时间
    pub updated_at: i64,
}
