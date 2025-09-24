use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::Validate;

use crate::dtos::solana::common::TransactionStatus;

// ============ MintNft API相关DTO ============

/// Mint NFT请求DTO
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct MintNftRequest {
    /// 用户钱包地址
    #[validate(length(min = 32, max = 44))]
    pub user_wallet: String,

    /// NFT铸造数量
    #[validate(range(min = 1, max = 1000))]
    pub amount: u64,
}

/// Mint NFT响应DTO
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct MintNftResponse {
    /// 交易签名（未签名时为空）
    pub signature: Option<String>,

    /// 用户钱包地址
    pub user_wallet: String,

    /// 铸造的NFT数量
    pub amount: u64,

    /// NFT mint地址
    pub nft_mint: String,

    /// 用户推荐账户地址
    pub user_referral: String,

    /// 用户mint计数器地址
    pub mint_counter: String,

    /// NFT池子权限地址
    pub nft_pool_authority: String,

    /// NFT池子账户地址
    pub nft_pool_account: String,

    /// 交易状态
    pub status: TransactionStatus,

    /// 区块链浏览器链接
    pub explorer_url: Option<String>,

    /// 时间戳
    pub timestamp: i64,

    /// 序列化的交易（base64编码，用于前端签名）
    pub serialized_transaction: Option<String>,
}

/// Mint NFT并发送交易响应DTO
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct MintNftAndSendTransactionResponse {
    /// 交易签名
    pub signature: String,

    /// 用户钱包地址
    pub user_wallet: String,

    /// 铸造的NFT数量
    pub amount: u64,

    /// NFT mint地址
    pub nft_mint: String,

    /// 用户推荐账户地址
    pub user_referral: String,

    /// 用户mint计数器地址
    pub mint_counter: String,

    /// NFT池子权限地址
    pub nft_pool_authority: String,

    /// NFT池子账户地址
    pub nft_pool_account: String,

    /// 交易状态
    pub status: TransactionStatus,

    /// 区块链浏览器链接
    pub explorer_url: String,

    /// 时间戳
    pub timestamp: i64,
}
