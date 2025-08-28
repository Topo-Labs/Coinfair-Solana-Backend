use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::Validate;

/// 推荐系统信息
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ReferralInfo {
    /// 上级地址
    pub upper: Option<String>,

    /// 上上级地址
    #[serde(rename = "upperUpper")]
    pub upper_upper: Option<String>,

    /// 项目方账户地址
    #[serde(rename = "projectAccount")]
    pub project_account: String,

    /// 推荐程序ID
    #[serde(rename = "referralProgram")]
    pub referral_program: String,

    /// 推荐账户PDA地址
    #[serde(rename = "payerReferral")]
    pub payer_referral: String,

    /// 上级推荐账户PDA地址（可选）
    #[serde(rename = "upperReferral")]
    pub upper_referral: Option<String>,
}

/// 奖励分配信息
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RewardDistribution {
    /// 总奖励费用
    #[serde(rename = "totalRewardFee")]
    pub total_reward_fee: u64,

    /// 项目方奖励
    #[serde(rename = "projectReward")]
    pub project_reward: u64,

    /// 上级奖励
    #[serde(rename = "upperReward")]
    pub upper_reward: u64,

    /// 上上级奖励
    #[serde(rename = "upperUpperReward")]
    pub upper_upper_reward: u64,

    /// 奖励分配比例说明
    #[serde(rename = "distributionRatios")]
    pub distribution_ratios: RewardDistributionRatios,
}

/// 奖励分配比例
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RewardDistributionRatios {
    /// 项目方比例（百分比）
    #[serde(rename = "projectRatio")]
    pub project_ratio: f64,

    /// 上级比例（百分比）
    #[serde(rename = "upperRatio")]
    pub upper_ratio: f64,

    /// 上上级比例（百分比）
    #[serde(rename = "upperUpperRatio")]
    pub upper_upper_ratio: f64,
}

/// 推荐系统账户信息
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct ReferralAccounts {
    /// 推荐账户PDA地址
    #[serde(rename = "payerReferral")]
    pub payer_referral: String,

    /// 上级地址（可选）
    pub upper: Option<String>,

    /// 上级代币账户地址（可选）
    #[serde(rename = "upperTokenAccount")]
    pub upper_token_account: Option<String>,

    /// 上级推荐账户PDA地址（可选）
    #[serde(rename = "upperReferral")]
    pub upper_referral: Option<String>,

    /// 上上级地址（可选）
    #[serde(rename = "upperUpper")]
    pub upper_upper: Option<String>,

    /// 上上级代币账户地址（可选）
    #[serde(rename = "upperUpperTokenAccount")]
    pub upper_upper_token_account: Option<String>,

    /// 项目方代币账户地址
    #[serde(rename = "projectTokenAccount")]
    pub project_token_account: String,

    /// 推荐程序ID
    #[serde(rename = "referralProgram")]
    pub referral_program: String,
}

/// 推荐系统交易信息
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ReferralTransactionInfo {
    /// 是否启用推荐奖励
    #[serde(rename = "rewardsEnabled")]
    pub rewards_enabled: bool,

    /// 预期奖励分配
    #[serde(rename = "expectedRewards")]
    pub expected_rewards: Option<RewardDistribution>,

    /// 推荐系统账户验证状态
    #[serde(rename = "accountValidation")]
    pub account_validation: Vec<ReferralAccountValidation>,
}

/// 推荐账户验证状态
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ReferralAccountValidation {
    /// 账户类型
    #[serde(rename = "accountType")]
    pub account_type: String,

    /// 账户地址
    pub address: String,

    /// 验证状态
    pub valid: bool,

    /// 验证消息
    pub message: Option<String>,
}
