use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::Validate;

use crate::dtos::solana::common::TransactionStatus;

// ============ CreatePool API相关DTO ============

/// 创建池子请求DTO
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct CreatePoolRequest {
    /// AMM配置索引
    #[validate(range(min = 0, max = 255))]
    pub config_index: u16,

    /// 初始价格（token1/token0的比率）
    #[validate(range(min = 0.000001, max = 1000000.0))]
    pub price: f64,

    /// 第一个代币mint地址
    pub mint0: String,

    /// 第二个代币mint地址
    pub mint1: String,

    /// 池子开放时间（Unix时间戳，0表示立即开放）
    #[validate(range(min = 0))]
    pub open_time: u64,

    /// 用户钱包地址（用于签名交易）
    pub user_wallet: String,
}

/// 创建池子响应DTO
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreatePoolResponse {
    /// Base64编码的未签名交易数据
    pub transaction: String,

    /// 交易描述信息
    pub transaction_message: String,

    /// 池子地址
    pub pool_address: String,

    /// AMM配置地址
    pub amm_config_address: String,

    /// Token0 Vault地址
    pub token_vault_0: String,

    /// Token1 Vault地址
    pub token_vault_1: String,

    /// 观察状态地址
    pub observation_address: String,

    /// Tick Array Bitmap Extension地址
    pub tickarray_bitmap_extension: String,

    /// 初始价格
    pub initial_price: f64,

    /// 初始sqrt_price_x64
    pub sqrt_price_x64: String,

    /// 对应的tick
    pub initial_tick: i32,

    /// 时间戳
    pub timestamp: i64,
}

/// 创建池子并发送交易响应DTO
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreatePoolAndSendTransactionResponse {
    /// 交易签名
    pub signature: String,

    /// 池子地址
    pub pool_address: String,

    /// AMM配置地址
    pub amm_config_address: String,

    /// Token0 Vault地址
    pub token_vault_0: String,

    /// Token1 Vault地址
    pub token_vault_1: String,

    /// 观察状态地址
    pub observation_address: String,

    /// Tick Array Bitmap Extension地址
    pub tickarray_bitmap_extension: String,

    /// 初始价格
    pub initial_price: f64,

    /// 初始sqrt_price_x64
    pub sqrt_price_x64: String,

    /// 对应的tick
    pub initial_tick: i32,

    /// 交易状态
    pub status: TransactionStatus,

    /// 区块链浏览器链接
    pub explorer_url: String,

    /// 时间戳
    pub timestamp: i64,
}

// ============ Classic AMM Pool API相关DTO ============

/// 创建经典AMM池子请求DTO
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct CreateClassicAmmPoolRequest {
    /// 第一个代币mint地址
    #[validate(length(min = 32, max = 44))]
    pub mint0: String,

    /// 第二个代币mint地址
    #[validate(length(min = 32, max = 44))]
    pub mint1: String,

    /// 第一个代币的初始数量（最小单位）
    #[validate(range(min = 1))]
    pub init_amount_0: u64,

    /// 第二个代币的初始数量（最小单位）
    #[validate(range(min = 1))]
    pub init_amount_1: u64,

    /// 池子开放时间（Unix时间戳，0表示立即开放）
    #[validate(range(min = 0))]
    pub open_time: u64,

    /// 用户钱包地址（用于签名交易）
    #[validate(length(min = 32, max = 44))]
    pub user_wallet: String,
}

/// 创建经典AMM池子响应DTO
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateClassicAmmPoolResponse {
    /// Base64编码的未签名交易数据
    pub transaction: String,

    /// 交易描述信息
    pub transaction_message: String,

    /// 池子地址
    pub pool_address: String,

    /// Coin mint地址（按字节序排序后的第一个mint）
    pub coin_mint: String,

    /// PC mint地址（按字节序排序后的第二个mint）
    pub pc_mint: String,

    /// Coin token账户地址
    pub coin_vault: String,

    /// PC token账户地址
    pub pc_vault: String,

    /// LP mint地址
    pub lp_mint: String,

    /// Open orders地址
    pub open_orders: String,

    /// Target orders地址
    pub target_orders: String,

    /// Withdraw queue地址
    pub withdraw_queue: String,

    /// 初始Coin数量
    pub init_coin_amount: u64,

    /// 初始PC数量
    pub init_pc_amount: u64,

    /// 池子开放时间
    pub open_time: u64,

    /// 时间戳
    pub timestamp: i64,
}

/// 创建经典AMM池子并发送交易响应DTO
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateClassicAmmPoolAndSendTransactionResponse {
    /// 交易签名
    pub signature: String,

    /// 池子地址
    pub pool_address: String,

    /// Coin mint地址（按字节序排序后的第一个mint）
    pub coin_mint: String,

    /// PC mint地址（按字节序排序后的第二个mint）
    pub pc_mint: String,

    /// Coin token账户地址
    pub coin_vault: String,

    /// PC token账户地址
    pub pc_vault: String,

    /// LP mint地址
    pub lp_mint: String,

    /// Open orders地址
    pub open_orders: String,

    /// Target orders地址
    pub target_orders: String,

    /// Withdraw queue地址
    pub withdraw_queue: String,

    /// 实际使用的Coin数量
    pub actual_coin_amount: u64,

    /// 实际使用的PC数量
    pub actual_pc_amount: u64,

    /// 池子开放时间
    pub open_time: u64,

    /// 交易状态
    pub status: TransactionStatus,

    /// 区块链浏览器链接
    pub explorer_url: String,

    /// 时间戳
    pub timestamp: i64,
}
