use crate::dtos::solana::common::{TransactionStatus, validate_pubkey, default_slippage_option};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::Validate;

/// CPMM提取流动性请求 - 100%忠实CLI参数
#[derive(Debug, Serialize, Deserialize, Clone, Validate, ToSchema)]
pub struct CpmmWithdrawRequest {
    /// 池子ID
    #[validate(custom = "validate_pubkey")]
    pub pool_id: String,
    /// 用户LP代币账户地址
    #[validate(custom = "validate_pubkey")]
    pub user_lp_token: String,
    /// 要提取的LP代币数量
    #[validate(range(min = 1, message = "LP代币数量必须大于0"))]
    pub lp_token_amount: u64,
    /// 滑点容忍度(百分比，如0.5表示0.5%)
    #[validate(range(min = 0.0, max = 100.0, message = "滑点必须在0-100%之间"))]
    #[serde(default = "default_slippage_option")]
    pub slippage: Option<f64>,
    /// 用户钱包地址（用于构建ATA地址）
    #[validate(custom = "validate_pubkey")]
    pub user_wallet: String,
}

/// CPMM提取流动性响应 - 构建交易但不发送
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CpmmWithdrawResponse {
    /// 序列化的交易数据(Base64)
    pub transaction: String,
    /// 交易描述
    pub transaction_message: String,
    /// 池子地址
    pub pool_id: String,
    /// Token0 mint地址
    pub token_0_mint: String,
    /// Token1 mint地址
    pub token_1_mint: String,
    /// LP代币mint地址
    pub lp_mint: String,
    /// 要提取的LP代币数量
    pub lp_token_amount: u64,
    /// 计算得出的Token0最小输出量(含滑点，扣除转账费)
    pub amount_0_min: u64,
    /// 计算得出的Token1最小输出量(含滑点，扣除转账费)
    pub amount_1_min: u64,
    /// 基础Token0输出量(不含滑点)
    pub token_0_amount: u64,
    /// 基础Token1输出量(不含滑点)
    pub token_1_amount: u64,
    /// Token0转账费
    pub transfer_fee_0: u64,
    /// Token1转账费
    pub transfer_fee_1: u64,
    /// 用户Token0 ATA地址
    pub user_token_0_ata: String,
    /// 用户Token1 ATA地址
    pub user_token_1_ata: String,
    /// 时间戳
    pub timestamp: i64,
}

/// CPMM提取流动性并发送交易请求 - 使用本地私钥签名发送
#[derive(Debug, Serialize, Deserialize, Clone, Validate, ToSchema)]
pub struct CpmmWithdrawAndSendRequest {
    /// 池子ID
    #[validate(custom = "validate_pubkey")]
    pub pool_id: String,
    /// 用户LP代币账户地址
    #[validate(custom = "validate_pubkey")]
    pub user_lp_token: String,
    /// 要提取的LP代币数量
    #[validate(range(min = 1, message = "LP代币数量必须大于0"))]
    pub lp_token_amount: u64,
    /// 滑点容忍度(百分比，如0.5表示0.5%)
    #[validate(range(min = 0.0, max = 100.0, message = "滑点必须在0-100%之间"))]
    #[serde(default = "default_slippage_option")]
    pub slippage: Option<f64>,
    /// 用户钱包地址（用于构建ATA地址）
    #[validate(custom = "validate_pubkey")]
    pub user_wallet: String,
}

/// CPMM提取流动性并发送交易响应 - 交易已发送到链上
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CpmmWithdrawAndSendResponse {
    /// 交易签名
    pub signature: String,
    /// 池子地址
    pub pool_id: String,
    /// Token0 mint地址
    pub token_0_mint: String,
    /// Token1 mint地址
    pub token_1_mint: String,
    /// LP代币mint地址
    pub lp_mint: String,
    /// 提取的LP代币数量
    pub lp_token_amount: u64,
    /// 实际获得的Token0数量
    pub actual_amount_0: u64,
    /// 实际获得的Token1数量
    pub actual_amount_1: u64,
    /// Token0最小输出量(含滑点，扣除转账费)
    pub amount_0_min: u64,
    /// Token1最小输出量(含滑点，扣除转账费)
    pub amount_1_min: u64,
    /// Token0转账费
    pub transfer_fee_0: u64,
    /// Token1转账费
    pub transfer_fee_1: u64,
    /// 用户Token0 ATA地址
    pub user_token_0_ata: String,
    /// 用户Token1 ATA地址
    pub user_token_1_ata: String,
    /// 交易状态
    pub status: TransactionStatus,
    /// 区块浏览器链接
    pub explorer_url: String,
    /// 时间戳
    pub timestamp: i64,
}

/// CPMM提取流动性计算结果 - 预计算提取所得金额
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CpmmWithdrawCompute {
    /// 池子ID
    pub pool_id: String,
    /// Token0 mint地址
    pub token_0_mint: String,
    /// Token1 mint地址
    pub token_1_mint: String,
    /// LP代币mint地址
    pub lp_mint: String,
    /// 要提取的LP代币数量
    pub lp_token_amount: u64,
    /// 可获得的Token0基础数量
    pub token_0_amount: u64,
    /// 可获得的Token1基础数量
    pub token_1_amount: u64,
    /// Token0含滑点数量
    pub amount_0_with_slippage: u64,
    /// Token1含滑点数量
    pub amount_1_with_slippage: u64,
    /// Token0最小输出量(含滑点，扣除转账费)
    pub amount_0_min: u64,
    /// Token1最小输出量(含滑点，扣除转账费)
    pub amount_1_min: u64,
    /// Token0转账费
    pub transfer_fee_0: u64,
    /// Token1转账费
    pub transfer_fee_1: u64,
    /// 滑点百分比
    pub slippage: f64,
    /// 池子详细信息
    pub pool_info: WithdrawPoolInfo,
}

/// 提取流动性池子信息
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct WithdrawPoolInfo {
    /// Token0金库总量(扣除费用后)
    pub total_token_0_amount: u64,
    /// Token1金库总量(扣除费用后)
    pub total_token_1_amount: u64,
    /// 当前LP代币供应总量
    pub lp_supply: u64,
    /// Token0 mint地址
    pub token_0_mint: String,
    /// Token1 mint地址
    pub token_1_mint: String,
    /// LP代币mint地址
    pub lp_mint: String,
    /// Token0金库地址
    pub token_0_vault: String,
    /// Token1金库地址
    pub token_1_vault: String,
}

/// CPMM流动性提取交易数据 - 通用交易结构
#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct CpmmWithdrawTransactionData {
    /// 序列化的交易数据(Base64)
    pub transaction: String,
    /// 交易大小(字节)
    pub transaction_size: usize,
    /// 交易描述
    pub description: String,
}