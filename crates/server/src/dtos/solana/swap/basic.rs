use crate::dtos::solana::common::validate_token_type;
use crate::dtos::solana::common::TransactionStatus;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::Validate;

/// 交换请求DTO
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct SwapRequest {
    /// 输入代币mint地址
    #[validate(custom = "validate_token_type")]
    pub from_token: String,

    /// 输出代币mint地址
    #[validate(custom = "validate_token_type")]
    pub to_token: String,

    /// 池子地址
    pub pool_address: String,

    /// 输入金额（以最小单位计算：SOL为lamports，USDC为micro-USDC）
    #[validate(range(min = 1000))] // 最小0.000001 SOL 或 0.001 USDC
    pub amount: u64,

    /// 最小输出金额（滑点保护）
    #[validate(range(min = 0))]
    pub minimum_amount_out: u64,

    /// 最大滑点百分比（0-100）
    #[validate(range(min = 0.0, max = 50.0))]
    pub max_slippage_percent: f64,
}

/// 交换响应DTO
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SwapResponse {
    /// 交易签名
    pub signature: String,

    /// 输入代币类型
    pub from_token: String,

    /// 输出代币类型
    pub to_token: String,

    /// 实际输入金额
    pub amount_in: u64,

    /// 预期输出金额
    pub amount_out_expected: u64,

    /// 实际输出金额（交易确认后更新）
    pub amount_out_actual: Option<u64>,

    /// 交易状态
    pub status: TransactionStatus,

    /// Solana Explorer链接
    pub explorer_url: String,

    /// 交易时间戳
    pub timestamp: i64,
}

/// 余额查询响应DTO
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct BalanceResponse {
    /// SOL余额（lamports）
    pub sol_balance_lamports: u64,

    /// SOL余额（SOL）
    pub sol_balance: f64,

    /// USDC余额（micro-USDC）
    pub usdc_balance_micro: u64,

    /// USDC余额（USDC）
    pub usdc_balance: f64,

    /// 钱包地址
    pub wallet_address: String,

    /// 查询时间戳
    pub timestamp: i64,
}

/// 价格查询请求DTO
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct PriceQuoteRequest {
    /// 输入代币mint地址
    #[validate(custom = "validate_token_type")]
    pub from_token: String,

    /// 输出代币mint地址
    #[validate(custom = "validate_token_type")]
    pub to_token: String,

    /// 池子地址
    pub pool_address: String,

    /// 输入金额
    #[validate(range(min = 1))]
    pub amount: u64,
}

/// 价格查询响应DTO
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PriceQuoteResponse {
    /// 输入代币类型
    pub from_token: String,

    /// 输出代币类型
    pub to_token: String,

    /// 输入金额
    pub amount_in: u64,

    /// 预期输出金额
    pub amount_out: u64,

    /// 价格（输出代币/输入代币）
    pub price: f64,

    /// 价格影响百分比
    pub price_impact_percent: f64,

    /// 建议最小输出金额（考虑5%滑点）
    pub minimum_amount_out: u64,

    /// 查询时间戳
    pub timestamp: i64,
}

// 交换历史记录DTO
// #[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
// pub struct SwapHistory {
//     /// 交易签名
//     pub signature: String,

//     /// 输入代币
//     pub from_token: String,

//     /// 输出代币
//     pub to_token: String,

//     /// 输入金额
//     pub amount_in: u64,

//     /// 输出金额
//     pub amount_out: u64,

//     /// 交易状态
//     pub status: TransactionStatus,

//     /// 交易时间
//     pub timestamp: i64,

//     /// Gas费用（lamports）
//     pub fee: u64,
// }
