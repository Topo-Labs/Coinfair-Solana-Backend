use anchor_lang::prelude::*;

#[account]
pub struct ReferralAccount {
    pub user: Pubkey,                // 本人
    pub upper: Option<Pubkey>,       // 上级
    pub upper_upper: Option<Pubkey>, // 上上级（废弃该字段）
    pub nft_mint: Pubkey,            // 绑定用的NFT
    pub bump: u8,                    // PDA bump
}
