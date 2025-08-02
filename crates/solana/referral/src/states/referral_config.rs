use anchor_lang::prelude::*;

pub const REFERRAL_CONFIG_SEED: &str = "config";

#[account]
pub struct ReferralConfig {
    pub admin: Pubkey,           // 管理员（Program 部署者）
    pub protocol_wallet: Pubkey, // 当前SOL收款地址
    pub nft_mint: Pubkey,        // 当前官方NFT mint
    pub claim_fee: u64,          // 领取需要支付多少 lamports
    pub bump: u8,                // bump
}
