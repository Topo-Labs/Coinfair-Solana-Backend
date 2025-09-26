
// ============ Classic AMM Pool API相关DTO ============

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::Validate;
use crate::dtos::solana::common::TransactionStatus;

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
