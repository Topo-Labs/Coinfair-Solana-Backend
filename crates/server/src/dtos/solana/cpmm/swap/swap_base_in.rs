use crate::dtos::solana::common::{default_slippage_option, validate_pubkey, TransactionStatus};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use validator::Validate;

/// CPMM SwapBaseIn请求参数
///
/// 执行基于固定输入金额的代币交换
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema, IntoParams)]
pub struct CpmmSwapBaseInRequest {
    /// 池子地址
    #[validate(custom = "validate_pubkey")]
    pub pool_id: String,

    /// 用户输入代币账户地址
    #[validate(custom = "validate_pubkey")]
    pub user_input_token: String,

    /// 输入代币数量（以最小单位计算，如lamports）
    #[validate(range(min = 1, message = "输入金额必须大于0"))]
    pub user_input_amount: u64,

    /// 滑点容忍度（百分比，0.0-100.0，默认0.5%）
    #[validate(range(min = 0.0, max = 100.0, message = "滑点必须在0-100%之间"))]
    #[serde(default = "default_slippage_option")]
    pub slippage: Option<f64>,
}

/// CPMM SwapBaseIn响应结果
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CpmmSwapBaseInResponse {
    /// 交易签名
    pub signature: String,

    /// 池子地址
    pub pool_id: String,

    /// 输入代币Mint地址
    pub input_token_mint: String,

    /// 输出代币Mint地址
    pub output_token_mint: String,

    /// 实际输入金额（扣除转账费后）
    pub actual_amount_in: u64,

    /// 计算得出的输出金额（扣除转账费前）
    pub amount_out: u64,

    /// 用户实际收到的金额（扣除转账费后）
    pub amount_received: u64,

    /// 最小输出金额（考虑滑点）
    pub minimum_amount_out: u64,

    /// 输入代币转账费
    pub input_transfer_fee: u64,

    /// 输出代币转账费
    pub output_transfer_fee: u64,

    /// 交易状态
    pub status: TransactionStatus,

    /// 区块链浏览器链接
    pub explorer_url: String,

    /// 时间戳
    pub timestamp: i64,
}

/// CPMM SwapBaseIn计算结果（用于报价和预计算）
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CpmmSwapBaseInCompute {
    /// 池子地址
    pub pool_id: String,

    /// 输入代币Mint地址
    pub input_token_mint: String,

    /// 输出代币Mint地址
    pub output_token_mint: String,

    /// 输入金额
    pub user_input_amount: u64,

    /// 实际输入金额（扣除转账费后）
    pub actual_amount_in: u64,

    /// 计算得出的输出金额（扣除转账费前）
    pub amount_out: u64,

    /// 用户实际收到的金额（扣除转账费后）
    pub amount_received: u64,

    /// 最小输出金额（考虑滑点）
    pub minimum_amount_out: u64,

    /// 输入代币转账费
    pub input_transfer_fee: u64,

    /// 输出代币转账费
    pub output_transfer_fee: u64,

    /// 价格比率（output/input）
    pub price_ratio: f64,

    /// 价格影响（百分比）
    pub price_impact_percent: f64,

    /// 交换手续费
    pub trade_fee: u64,

    /// 滑点容忍度
    pub slippage: f64,

    /// 池子当前状态快照
    pub pool_info: PoolStateInfo,
}

/// 池子状态信息
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PoolStateInfo {
    /// Token 0总量
    pub total_token_0_amount: u64,

    /// Token 1总量
    pub total_token_1_amount: u64,

    /// Token 0 Mint地址
    pub token_0_mint: String,

    /// Token 1 Mint地址
    pub token_1_mint: String,

    /// 交易方向
    pub trade_direction: String, // "ZeroForOne" 或 "OneForZero"

    /// AMM配置状态
    pub amm_config: AmmConfigInfo,
}

/// AMM配置信息
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AmmConfigInfo {
    /// 交易手续费率 (basis points)
    pub trade_fee_rate: u64,

    /// 创建者手续费率 (basis points)
    pub creator_fee_rate: u64,

    /// 协议手续费率 (basis points)
    pub protocol_fee_rate: u64,

    /// 资金手续费率 (basis points)
    pub fund_fee_rate: u64,
}

/// CPMM SwapBaseIn交易构建请求
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct CpmmSwapBaseInTransactionRequest {
    /// 用户钱包地址
    #[validate(custom = "validate_pubkey")]
    pub wallet: String,

    /// 交易版本
    pub tx_version: String,

    /// 交换计算结果
    pub swap_compute: CpmmSwapBaseInCompute,
}

/// 交易构建数据
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CpmmTransactionData {
    /// 序列化的交易数据
    pub transaction: String,

    /// 交易大小（字节）
    pub transaction_size: usize,

    /// 交易描述
    pub description: String,
}
