use crate::dtos::solana::common::{default_slippage, TransactionStatus};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::Validate;

// ============ OpenPosition API ============

/// 开仓请求DTO
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct OpenPositionRequest {
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

    /// 是否包含NFT元数据
    #[serde(default)]
    pub with_metadata: bool,

    /// 最大滑点百分比（0-100）
    #[validate(range(min = 0.0, max = 50.0))]
    #[serde(default = "default_slippage")]
    pub max_slippage_percent: f64,
}

/// 开仓响应DTO
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct OpenPositionResponse {
    /// Base64编码的未签名交易数据
    pub transaction: String,

    /// 交易消息摘要（用于前端显示）
    pub transaction_message: String,

    /// 预期的仓位NFT mint地址
    pub position_nft_mint: String,

    /// 预期的仓位键值
    pub position_key: String,

    /// 下限tick索引
    pub tick_lower_index: i32,

    /// 上限tick索引
    pub tick_upper_index: i32,

    /// 预期的流动性数量
    pub liquidity: String, // 使用字符串避免精度丢失

    /// 预期消耗的token0数量
    pub amount_0: u64,

    /// 预期消耗的token1数量
    pub amount_1: u64,

    /// 池子地址
    pub pool_address: String,

    /// 创建时间戳
    pub timestamp: i64,
}

/// 开仓响应DTO
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct OpenPositionAndSendTransactionResponse {
    /// 交易签名
    pub signature: String,

    /// 位置NFT mint地址
    pub position_nft_mint: String,

    /// 位置键值
    pub position_key: String,

    /// 下限tick索引
    pub tick_lower_index: i32,

    /// 上限tick索引
    pub tick_upper_index: i32,

    /// 流动性数量
    pub liquidity: String, // 使用字符串避免精度丢失

    /// 实际消耗的token0数量
    pub amount_0: u64,

    /// 实际消耗的token1数量
    pub amount_1: u64,

    /// 池子地址
    pub pool_address: String,

    /// 交易状态
    pub status: TransactionStatus,

    /// Solana Explorer链接
    pub explorer_url: String,

    /// 交易时间戳
    pub timestamp: i64,
}

/// 仓位信息DTO
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PositionInfo {
    /// 仓位键值
    pub position_key: String,

    /// 仓位NFT mint地址
    pub nft_mint: String,

    /// 池子地址
    pub pool_id: String,

    /// 下限tick索引
    pub tick_lower_index: i32,

    /// 上限tick索引
    pub tick_upper_index: i32,

    /// 流动性数量
    pub liquidity: String,

    /// 下限价格
    pub tick_lower_price: f64,

    /// 上限价格
    pub tick_upper_price: f64,

    /// 累计的token0手续费
    pub token_fees_owed_0: u64,

    /// 累计的token1手续费
    pub token_fees_owed_1: u64,

    /// 奖励信息
    pub reward_infos: Vec<PositionRewardInfo>,

    /// 创建时间戳
    pub created_at: i64,
}

/// 仓位奖励信息
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PositionRewardInfo {
    /// 奖励代币mint地址
    pub reward_mint: String,

    /// 累计奖励数量
    pub reward_amount_owed: u64,

    /// 奖励增长内部记录
    pub growth_inside_last_x64: String,
}

/// 获取用户仓位列表请求DTO
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct GetUserPositionsRequest {
    /// 用户钱包地址（可选，默认使用服务配置的钱包）
    #[validate(length(min = 32, max = 44))]
    pub wallet_address: Option<String>,

    /// 池子地址过滤（可选）
    #[validate(length(min = 32, max = 44))]
    pub pool_address: Option<String>,
}

/// 用户仓位列表响应DTO
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UserPositionsResponse {
    /// 仓位列表
    pub positions: Vec<PositionInfo>,

    /// 总仓位数量
    pub total_count: usize,

    /// 查询的钱包地址
    pub wallet_address: String,

    /// 查询时间戳
    pub timestamp: i64,
}

/// 流动性计算请求DTO
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct CalculateLiquidityRequest {
    /// 池子地址
    #[validate(length(min = 32, max = 44))]
    pub pool_address: String,

    /// 池子地址
    #[validate(length(min = 32, max = 44))]
    pub user_wallet: String,

    /// 下限价格
    #[validate(range(min = 0.000001, max = 1000000.0))]
    pub tick_lower_price: f64,

    /// 上限价格
    #[validate(range(min = 0.000001, max = 1000000.0))]
    pub tick_upper_price: f64,

    /// 是否基于token0计算
    pub is_base_0: bool,

    /// 输入金额
    #[validate(range(min = 1))]
    pub input_amount: u64,
}

/// 流动性计算响应DTO
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CalculateLiquidityResponse {
    /// 计算得到的流动性
    pub liquidity: String,

    /// 需要的token0数量
    pub amount_0: u64,

    /// 需要的token1数量
    pub amount_1: u64,

    /// 下限tick索引
    pub tick_lower_index: i32,

    /// 上限tick索引
    pub tick_upper_index: i32,

    /// 当前池子价格
    pub current_price: f64,

    /// 价格在范围内的比例
    pub price_range_utilization: f64,
}
