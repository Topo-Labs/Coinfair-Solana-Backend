use crate::error::ReferralError;
use crate::states::ReferralConfig;
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct InitReferralConfig<'info> {
    #[account(
        mut,
        address = crate::admin::id() @ ReferralError::NotApproved
    )]
    pub payer: Signer<'info>,

    #[account(
        init,
        payer = payer,
        seeds = [b"config"],
        bump,
        space = 8 + 32 * 3 + 8 + 1, // 8+32+32+32+8+1
    )]
    pub config: Account<'info, ReferralConfig>,

    pub system_program: Program<'info, System>,
}

pub fn init_config(
    ctx: Context<InitReferralConfig>,
    admin: Pubkey,
    nft_mint: Pubkey,
    protocol_wallet: Pubkey,
    claim_fee: u64,
) -> Result<()> {
    let config = &mut ctx.accounts.config;
    config.admin = admin;
    config.nft_mint = nft_mint;
    config.protocol_wallet = protocol_wallet;
    config.claim_fee = claim_fee;
    config.bump = ctx.bumps.config;
    Ok(())
}

#[derive(Accounts)]
pub struct UpdateAdmin<'info> {
    #[account(mut, has_one = admin @ ReferralError::NotApproved)]
    pub config: Account<'info, ReferralConfig>,
    pub admin: Signer<'info>,
}

pub fn update_admin(ctx: Context<UpdateAdmin>, new_admin: Pubkey) -> Result<()> {
    ctx.accounts.config.admin = new_admin;
    Ok(())
}

#[derive(Accounts)]
pub struct UpdateNftMint<'info> {
    #[account(mut, has_one = admin @ ReferralError::NotApproved)]
    pub config: Account<'info, ReferralConfig>,
    pub admin: Signer<'info>,
}

pub fn update_nft_mint(ctx: Context<UpdateNftMint>, new_nft_mint: Pubkey) -> Result<()> {
    ctx.accounts.config.nft_mint = new_nft_mint;
    Ok(())
}

#[derive(Accounts)]
pub struct UpdateProtocolWallet<'info> {
    #[account(mut, has_one = admin @ ReferralError::NotApproved)]
    pub config: Account<'info, ReferralConfig>,
    pub admin: Signer<'info>,
}

pub fn update_protocol_wallet(
    ctx: Context<UpdateProtocolWallet>,
    new_wallet: Pubkey,
) -> Result<()> {
    ctx.accounts.config.protocol_wallet = new_wallet;
    Ok(())
}

#[derive(Accounts)]
pub struct UpdateClaimFee<'info> {
    #[account(mut, has_one = admin @ ReferralError::NotApproved)]
    pub config: Account<'info, ReferralConfig>,
    pub admin: Signer<'info>,
}

pub fn update_claim_fee(ctx: Context<UpdateClaimFee>, new_fee: u64) -> Result<()> {
    ctx.accounts.config.claim_fee = new_fee;
    Ok(())
}
