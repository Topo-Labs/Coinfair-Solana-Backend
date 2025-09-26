use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use validator::Validate;

// ============ Referral API相关DTO ============

/// GetUpper请求DTO
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema, IntoParams)]
pub struct GetUpperRequest {
    /// 用户钱包地址
    #[validate(length(min = 32, max = 44))]
    pub user_wallet: String,
}

/// GetUpper响应DTO
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct GetUpperResponse {
    /// 用户钱包地址
    pub user_wallet: String,

    /// 上级钱包地址（如果存在）
    pub upper: Option<String>,

    /// 推荐账户PDA地址
    pub referral_account: String,

    /// 查询状态
    pub status: String,

    /// 时间戳
    pub timestamp: i64,
}

/// GetUpper验证响应DTO（用于本地测试）
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct GetUpperAndVerifyResponse {
    /// 基础响应数据
    #[serde(flatten)]
    pub base: GetUpperResponse,

    /// 链上账户是否存在
    pub account_exists: bool,

    /// 完整的ReferralAccount数据
    pub referral_account_data: Option<ReferralAccountData>,
}

/// ReferralAccount链上数据结构
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct ReferralAccountData {
    /// 用户地址
    pub user: String,

    /// 上级用户地址
    pub upper: Option<String>,

    /// 上上级用户地址
    pub upper_upper: Option<String>,

    /// 绑定的NFT mint地址
    pub nft_mint: String,

    /// PDA bump
    pub bump: u8,
}

/// GetMintCounter请求DTO
#[derive(Debug, Clone, Serialize, Deserialize, Validate, IntoParams, ToSchema)]
pub struct GetMintCounterRequest {
    /// 用户钱包地址
    #[validate(length(min = 32, max = 44))]
    pub user_wallet: String,
}

/// GetMintCounter响应DTO
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct GetMintCounterResponse {
    /// 用户钱包地址
    pub user_wallet: String,

    /// 总mint数量
    pub total_mint: u64,

    /// 剩余可claim数量
    pub remain_mint: u64,

    /// mint counter账户PDA地址
    pub mint_counter_account: String,

    /// 查询状态
    pub status: String,

    /// 时间戳
    pub timestamp: i64,
}

/// GetMintCounter验证响应DTO（用于本地测试）
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct GetMintCounterAndVerifyResponse {
    /// 基础响应数据
    #[serde(flatten)]
    pub base: GetMintCounterResponse,

    /// 链上账户是否存在
    pub account_exists: bool,

    /// 完整的MintCounter数据
    pub mint_counter_data: Option<MintCounterData>,
}

/// MintCounter链上数据结构
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct MintCounterData {
    /// 用户地址
    pub minter: String,

    /// 总mint数量
    pub total_mint: u64,

    /// 剩余可claim数量
    pub remain_mint: u64,

    /// PDA bump
    pub bump: u8,
}
