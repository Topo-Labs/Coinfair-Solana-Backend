use crate::dtos::solana::common::{TransactionStatus, validate_pubkey, default_slippage_option};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::Validate;

/// CPMM存款请求 - 100%忠实CLI参数
#[derive(Debug, Serialize, Deserialize, Clone, Validate, ToSchema)]
pub struct CpmmDepositRequest {
    /// 池子ID
    #[validate(custom = "validate_pubkey")]
    pub pool_id: String,
    /// 用户Token0账户地址
    #[validate(custom = "validate_pubkey")]
    pub user_token_0: String,
    /// 用户Token1账户地址
    #[validate(custom = "validate_pubkey")]
    pub user_token_1: String,
    /// 期望获得的LP代币数量
    #[validate(range(min = 1, message = "LP代币数量必须大于0"))]
    pub lp_token_amount: u64,
    /// 滑点容忍度(百分比，如0.5表示0.5%)
    #[validate(range(min = 0.0, max = 100.0, message = "滑点必须在0-100%之间"))]
    #[serde(default = "default_slippage_option")]
    pub slippage: Option<f64>,
}

/// CPMM存款响应 - 构建交易但不发送
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CpmmDepositResponse {
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
    /// 期望获得的LP代币数量
    pub lp_token_amount: u64,
    /// 计算得出的Token0最大输入量(含滑点和转账费)
    pub amount_0_max: u64,
    /// 计算得出的Token1最大输入量(含滑点和转账费)
    pub amount_1_max: u64,
    /// 基础Token0输入量(不含滑点)
    pub token_0_amount: u64,
    /// 基础Token1输入量(不含滑点)
    pub token_1_amount: u64,
    /// Token0转账费
    pub transfer_fee_0: u64,
    /// Token1转账费
    pub transfer_fee_1: u64,
    /// 时间戳
    pub timestamp: i64,
}

/// CPMM存款并发送交易请求 - 使用本地私钥签名发送
#[derive(Debug, Serialize, Deserialize, Clone, Validate, ToSchema)]
pub struct CpmmDepositAndSendRequest {
    /// 池子ID
    #[validate(custom = "validate_pubkey")]
    pub pool_id: String,
    /// 用户Token0账户地址（可以是mint地址或ATA地址）
    #[validate(custom = "validate_pubkey")]
    pub user_token_0: String,
    /// 用户Token1账户地址（可以是mint地址或ATA地址）
    #[validate(custom = "validate_pubkey")]
    pub user_token_1: String,
    /// 期望获得的LP代币数量
    #[validate(range(min = 1, message = "LP代币数量必须大于0"))]
    pub lp_token_amount: u64,
    /// 滑点容忍度(百分比，如0.5表示0.5%)
    #[validate(range(min = 0.0, max = 100.0, message = "滑点必须在0-100%之间"))]
    #[serde(default = "default_slippage_option")]
    pub slippage: Option<f64>,
}

/// CPMM存款并发送交易响应 - 交易已发送到链上
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CpmmDepositAndSendResponse {
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
    /// 期望获得的LP代币数量
    pub lp_token_amount: u64,
    /// 实际存入的Token0数量
    pub actual_amount_0: u64,
    /// 实际存入的Token1数量
    pub actual_amount_1: u64,
    /// Token0最大输入量(含滑点和转账费)
    pub amount_0_max: u64,
    /// Token1最大输入量(含滑点和转账费)
    pub amount_1_max: u64,
    /// Token0转账费
    pub transfer_fee_0: u64,
    /// Token1转账费
    pub transfer_fee_1: u64,
    /// 交易状态
    pub status: TransactionStatus,
    /// 区块浏览器链接
    pub explorer_url: String,
    /// 时间戳
    pub timestamp: i64,
}

/// CPMM存款计算结果 - 预计算存款所需金额
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CpmmDepositCompute {
    /// 池子ID
    pub pool_id: String,
    /// Token0 mint地址
    pub token_0_mint: String,
    /// Token1 mint地址
    pub token_1_mint: String,
    /// LP代币mint地址
    pub lp_mint: String,
    /// 期望获得的LP代币数量
    pub lp_token_amount: u64,
    /// 需要存入的Token0基础数量
    pub token_0_amount: u64,
    /// 需要存入的Token1基础数量
    pub token_1_amount: u64,
    /// Token0含滑点数量
    pub amount_0_with_slippage: u64,
    /// Token1含滑点数量
    pub amount_1_with_slippage: u64,
    /// Token0最大输入量(含滑点和转账费)
    pub amount_0_max: u64,
    /// Token1最大输入量(含滑点和转账费)
    pub amount_1_max: u64,
    /// Token0转账费
    pub transfer_fee_0: u64,
    /// Token1转账费
    pub transfer_fee_1: u64,
    /// 滑点百分比
    pub slippage: f64,
    /// 池子详细信息
    pub pool_info: DepositPoolInfo,
}

/// 存款池子信息
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct DepositPoolInfo {
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
}

/// CPMM交易数据 - 通用交易结构
#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CpmmDepositTransactionData {
    /// 序列化的交易数据(Base64)
    pub transaction: String,
    /// 交易大小(字节)
    pub transaction_size: usize,
    /// 交易描述
    pub description: String,
}