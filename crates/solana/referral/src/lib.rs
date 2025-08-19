pub mod constants;
pub mod error;
pub mod instructions;
pub mod states;
pub mod utils;

use anchor_lang::prelude::*;
use instructions::*;

// #[cfg(feature = "localnet")]
// declare_id!("NFTqT8aVmB4WjzFkyKX9h5sEhhbTowNpuwbKNi2CZXs");

// #[cfg(not(feature = "localnet"))]
declare_id!("REFxcjx4pKym9j5Jzbo9wh92CtYTzHt9fqcjgvZGvUL");

pub mod admin {
    use anchor_lang::prelude::declare_id;
    declare_id!("adMAyHzh8f3budnNayPU4Nb8faYZDZdGU1GXMB1Jt4Z");
}

#[program]
pub mod referral {
    use super::*;

    pub fn init_config(
        ctx: Context<InitReferralConfig>,
        admin: Pubkey,
        nft_mint: Pubkey,
        protocol_wallet: Pubkey,
        claim_fee: u64,
    ) -> Result<()> {
        init_config::init_config(ctx, admin, nft_mint, protocol_wallet, claim_fee)
    }

    pub fn mint_nft(ctx: Context<MintReferralNFT>, amount: u64) -> Result<()> {
        mint_nft::mint_nft(ctx, amount)
    }

    pub fn claim_nft(ctx: Context<ClaimReferralNFT>) -> Result<()> {
        claim_nft::claim_nft(ctx)
    }

    pub fn get_upper(ctx: Context<GetUpper>) -> Result<Option<Pubkey>> {
        get_upper::get_upper(ctx)
    }

    pub fn get_upper_for_idl(ctx: Context<GetUpper>) -> Result<()> {
        get_upper::get_upper_for_cpi(ctx)
    }

    pub fn update_nft_mint(ctx: Context<UpdateNftMint>, new_nft_mint: Pubkey) -> Result<()> {
        init_config::update_nft_mint(ctx, new_nft_mint)
    }

    pub fn get_mint_counter(ctx: Context<GetMintCounter>) -> Result<(u64, u64)> {
        get_mint_counter::get_mint_counter(ctx)
    }
}
