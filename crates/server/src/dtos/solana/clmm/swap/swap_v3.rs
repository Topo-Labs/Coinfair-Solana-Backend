use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use validator::Validate;

use crate::dtos::solana::common::{RoutePlan, TransactionStatus, TransferFeeInfo};
use crate::dtos::solana::clmm::swap::{
    raydium::RaydiumResponse,
    referral::{ReferralAccounts, ReferralInfo},
};

/// SwapV3计算交换请求参数（支持推荐系统）
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema, IntoParams)]
pub struct ComputeSwapV3Request {
    /// 输入代币的mint地址
    #[serde(rename = "inputMint")]
    pub input_mint: String,

    /// 输出代币的mint地址
    #[serde(rename = "outputMint")]
    pub output_mint: String,

    /// 输入或输出金额（以最小单位计算）
    #[validate(length(min = 1))]
    pub amount: String,

    /// 滑点容忍度（基点，如50表示0.5%）
    #[serde(rename = "slippageBps")]
    #[validate(range(min = 1, max = 10000))]
    pub slippage_bps: u16,

    /// 限价（可选）
    #[serde(rename = "limitPrice")]
    pub limit_price: Option<f64>,

    /// 是否启用转账费计算（默认为true）
    #[serde(rename = "enableTransferFee")]
    pub enable_transfer_fee: Option<bool>,

    /// 交易版本（V0或V1）
    #[serde(rename = "txVersion")]
    pub tx_version: String,
    // /// 推荐账户地址（可选）
    // #[serde(rename = "referralAccount")]
    // pub referral_account: Option<String>,

    // /// 上级地址（可选）
    // #[serde(rename = "upperAccount")]
    // pub upper_account: Option<String>,

    // /// 是否启用推荐奖励（默认为true）
    // #[serde(rename = "enableReferralRewards")]
    // pub enable_referral_rewards: Option<bool>,
}

/// SwapV3交换计算结果数据（支持推荐系统）
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SwapComputeV3Data {
    /// 交换类型（BaseInV3/BaseOutV3）
    #[serde(rename = "swapType")]
    pub swap_type: String,

    /// 输入代币mint地址
    #[serde(rename = "inputMint")]
    pub input_mint: String,

    /// 输入金额
    #[serde(rename = "inputAmount")]
    pub input_amount: String,

    /// 输出代币mint地址
    #[serde(rename = "outputMint")]
    pub output_mint: String,

    /// 输出金额
    #[serde(rename = "outputAmount")]
    pub output_amount: String,

    /// 最小输出阈值（考虑滑点）
    #[serde(rename = "otherAmountThreshold")]
    pub other_amount_threshold: String,

    /// 滑点设置（基点）
    #[serde(rename = "slippageBps")]
    pub slippage_bps: u16,

    /// 价格影响百分比
    #[serde(rename = "priceImpactPct")]
    pub price_impact_pct: f64,

    /// 推荐人费用
    #[serde(rename = "referrerAmount")]
    pub referrer_amount: String,

    /// 路由计划
    #[serde(rename = "routePlan")]
    pub route_plan: Vec<RoutePlan>,

    /// 转账费信息
    #[serde(rename = "transferFeeInfo")]
    pub transfer_fee_info: Option<TransferFeeInfo>,

    /// 扣除转账费后的实际金额
    #[serde(rename = "amountSpecified")]
    pub amount_specified: Option<String>,

    /// 当前epoch
    pub epoch: Option<u64>,
}

/// SwapV3交易构建请求
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct TransactionSwapV3Request {
    /// 用户钱包地址
    #[validate(length(min = 32, max = 64))]
    pub wallet: String,

    /// 计算单元价格（微lamports）
    #[serde(rename = "computeUnitPriceMicroLamports")]
    pub compute_unit_price_micro_lamports: String,

    /// SwapV3交换响应数据（来自compute-v3接口）
    #[serde(rename = "swapResponse")]
    pub swap_response: RaydiumResponse<SwapComputeV3Data>,

    /// 交易版本
    #[serde(rename = "txVersion")]
    pub tx_version: String,

    /// 是否包装SOL
    #[serde(rename = "wrapSol")]
    pub wrap_sol: bool,

    /// 是否解包装SOL
    #[serde(rename = "unwrapSol")]
    pub unwrap_sol: bool,

    /// 输入代币账户地址（可选）
    #[serde(rename = "inputAccount")]
    pub input_account: Option<String>,

    /// 输出代币账户地址（可选）
    #[serde(rename = "outputAccount")]
    pub output_account: Option<String>,

    /// 推荐系统相关账户
    #[serde(rename = "referralAccounts")]
    pub referral_accounts: Option<ReferralAccounts>,
}

// /// SwapV3交易构建响应（继承TransactionData但可能扩展）
// #[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
// pub struct SwapV3TransactionData {
//     /// 基础交易数据
//     #[serde(flatten)]
//     pub transaction_data: TransactionData,

//     /// 推荐系统相关信息
//     #[serde(rename = "referralInfo")]
//     pub referral_info: Option<ReferralTransactionInfo>,
// }

/// SwapV3并发送交易响应DTO（用于本地测试）
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SwapV3AndSendTransactionResponse {
    /// 交易签名
    pub signature: String,
    /// 用户钱包地址
    pub user_wallet: String,
    /// 输入代币mint地址
    pub input_mint: String,
    /// 输出代币mint地址
    pub output_mint: String,
    /// 输入金额
    pub input_amount: String,
    /// 输出金额（预期）
    pub output_amount: String,
    /// 最小输出阈值
    pub minimum_amount_out: String,
    /// 池子地址
    pub pool_address: String,
    /// 推荐系统信息
    pub referral_info: Option<ReferralInfo>,
    /// 交易状态
    pub status: TransactionStatus,
    /// Solana Explorer链接
    pub explorer_url: String,
    /// 交易时间戳
    pub timestamp: i64,
}
