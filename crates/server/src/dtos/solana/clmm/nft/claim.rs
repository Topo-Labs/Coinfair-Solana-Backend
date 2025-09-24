use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::Validate;

use crate::dtos::solana::common::TransactionStatus;

// ============ Claim NFT API相关DTO ============

/// 领取推荐NFT请求DTO
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct ClaimNftRequest {
    /// 下级用户钱包地址（发起领取的用户）
    #[validate(length(min = 32, max = 44))]
    pub user_wallet: String,

    /// 上级用户钱包地址（提供NFT的推荐人）
    #[validate(length(min = 32, max = 44))]
    pub upper: String,
}

/// 领取推荐NFT响应DTO
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ClaimNftResponse {
    /// 交易签名（未签名时为空）
    pub signature: Option<String>,

    /// 下级用户钱包地址
    pub user_wallet: String,

    /// 上级用户钱包地址
    pub upper: String,

    /// NFT mint地址
    pub nft_mint: String,

    /// 下级用户推荐账户地址
    pub user_referral: String,

    /// 上级用户推荐账户地址
    pub upper_referral: String,

    /// 上级用户mint计数器地址
    pub upper_mint_counter: String,

    /// NFT池子权限地址
    pub nft_pool_authority: String,

    /// NFT池子账户地址
    pub nft_pool_account: String,

    /// 下级用户ATA账户地址
    pub user_ata: String,

    /// 协议钱包地址
    pub protocol_wallet: String,

    /// 推荐配置账户地址
    pub referral_config: String,

    /// 交易状态
    pub status: TransactionStatus,

    /// 区块链浏览器链接
    pub explorer_url: Option<String>,

    /// 时间戳
    pub timestamp: i64,

    /// 序列化的交易（base64编码，用于前端签名）
    pub serialized_transaction: Option<String>,
}

/// 领取推荐NFT并发送交易响应DTO
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ClaimNftAndSendTransactionResponse {
    /// 交易签名
    pub signature: String,

    /// 下级用户钱包地址
    pub user_wallet: String,

    /// 上级用户钱包地址
    pub upper: String,

    /// NFT mint地址
    pub nft_mint: String,

    /// 下级用户推荐账户地址
    pub user_referral: String,

    /// 上级用户推荐账户地址
    pub upper_referral: String,

    /// 上级用户mint计数器地址
    pub upper_mint_counter: String,

    /// NFT池子权限地址
    pub nft_pool_authority: String,

    /// NFT池子账户地址
    pub nft_pool_account: String,

    /// 下级用户ATA账户地址
    pub user_ata: String,

    /// 协议钱包地址
    pub protocol_wallet: String,

    /// 推荐配置账户地址
    pub referral_config: String,

    /// 交易状态
    pub status: TransactionStatus,

    /// 区块链浏览器链接
    pub explorer_url: String,

    /// 时间戳
    pub timestamp: i64,
}
