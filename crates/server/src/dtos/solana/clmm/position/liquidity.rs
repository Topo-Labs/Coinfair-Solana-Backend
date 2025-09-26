use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::Validate;

use crate::dtos::solana::common::{default_slippage, TransactionStatus};

// ============ IncreaseLiquidity API相关DTO ============

/// 增加流动性请求DTO
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct IncreaseLiquidityRequest {
    /// 池子地址
    #[validate(length(min = 32, max = 44))]
    pub pool_address: String,

    /// 用户钱包地址
    #[validate(length(min = 32, max = 44))]
    pub user_wallet: String,

    /// 下限价格
    #[validate(range(min = 0.000001, max = 1000000.0))]
    pub tick_lower_price: f64,

    /// 上限价格
    #[validate(range(min = 0.000001, max = 1000000.0))]
    pub tick_upper_price: f64,

    /// 是否基于token0计算流动性
    pub is_base_0: bool,

    /// 输入金额（最小单位）
    #[validate(range(min = 1))]
    pub input_amount: u64,

    /// 最大滑点百分比（0-100）
    #[validate(range(min = 0.0, max = 50.0))]
    #[serde(default = "default_slippage")]
    pub max_slippage_percent: f64,
}

/// 增加流动性响应DTO
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct IncreaseLiquidityResponse {
    /// Base64编码的未签名交易数据
    pub transaction: String,

    /// 交易消息摘要（用于前端显示）
    pub transaction_message: String,

    /// 找到的现有仓位键值
    pub position_key: String,

    /// 增加的流动性数量
    pub liquidity_added: String, // 使用字符串避免精度丢失

    /// 需要消耗的token0数量
    pub amount_0: u64,

    /// 需要消耗的token1数量
    pub amount_1: u64,

    /// 下限tick索引
    pub tick_lower_index: i32,

    /// 上限tick索引
    pub tick_upper_index: i32,

    /// 池子地址
    pub pool_address: String,

    /// 创建时间戳
    pub timestamp: i64,
}

/// 增加流动性并发送交易响应DTO
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct IncreaseLiquidityAndSendTransactionResponse {
    /// 交易签名
    pub signature: String,

    /// 仓位键值
    pub position_key: String,

    /// 增加的流动性数量
    pub liquidity_added: String, // 使用字符串避免精度丢失

    /// 实际消耗的token0数量
    pub amount_0: u64,

    /// 实际消耗的token1数量
    pub amount_1: u64,

    /// 下限tick索引
    pub tick_lower_index: i32,

    /// 上限tick索引
    pub tick_upper_index: i32,

    /// 池子地址
    pub pool_address: String,

    /// 交易状态
    pub status: TransactionStatus,

    /// 区块链浏览器链接
    pub explorer_url: String,

    /// 时间戳
    pub timestamp: i64,
}

// ============ DecreaseLiquidity API相关DTO ============

/// 减少流动性请求DTO
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct DecreaseLiquidityRequest {
    /// 池子地址
    #[validate(length(min = 32, max = 44))]
    pub pool_address: String,

    /// 用户钱包地址
    #[validate(length(min = 32, max = 44))]
    pub user_wallet: String,

    /// 下限tick索引
    pub tick_lower_index: i32,

    /// 上限tick索引
    pub tick_upper_index: i32,

    /// 要减少的流动性数量（可选，如果为空则减少全部流动性）
    pub liquidity: Option<String>, // 使用字符串避免精度丢失

    /// 最大滑点百分比（0-100）
    #[validate(range(min = 0.0, max = 50.0))]
    pub max_slippage_percent: Option<f64>,

    /// 是否只模拟交易（不实际发送）
    #[serde(default)]
    pub simulate: bool,
}

/// 减少流动性响应DTO
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DecreaseLiquidityResponse {
    /// Base64编码的未签名交易数据
    pub transaction: String,

    /// 交易消息摘要（用于前端显示）
    pub transaction_message: String,

    /// 仓位键值
    pub position_key: String,

    /// 减少的流动性数量
    pub liquidity_removed: String, // 使用字符串避免精度丢失

    /// 预期获得的token0数量（减去滑点和转账费）
    pub amount_0_min: u64,

    /// 预期获得的token1数量（减去滑点和转账费）
    pub amount_1_min: u64,

    /// 预期实际获得的token0数量（未减去滑点和转账费）
    pub amount_0_expected: u64,

    /// 预期实际获得的token1数量（未减去滑点和转账费）
    pub amount_1_expected: u64,

    /// 下限tick索引
    pub tick_lower_index: i32,

    /// 上限tick索引
    pub tick_upper_index: i32,

    /// 池子地址
    pub pool_address: String,

    /// 是否会完全关闭仓位
    pub will_close_position: bool,

    /// 时间戳
    pub timestamp: i64,
}

/// 减少流动性并发送交易响应DTO
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DecreaseLiquidityAndSendTransactionResponse {
    /// 交易签名
    pub signature: String,

    /// 仓位键值
    pub position_key: String,

    /// 减少的流动性数量
    pub liquidity_removed: String, // 使用字符串避免精度丢失

    /// 实际获得的token0数量
    pub amount_0_actual: u64,

    /// 实际获得的token1数量
    pub amount_1_actual: u64,

    /// 下限tick索引
    pub tick_lower_index: i32,

    /// 上限tick索引
    pub tick_upper_index: i32,

    /// 池子地址
    pub pool_address: String,

    /// 是否已完全关闭仓位
    pub position_closed: bool,

    /// 交易状态
    pub status: TransactionStatus,

    /// Solana Explorer链接
    pub explorer_url: String,

    /// 时间戳
    pub timestamp: i64,
}
