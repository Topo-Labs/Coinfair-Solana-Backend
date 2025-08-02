use crate::error::ReferralError;
use crate::states::ReferralConfig;
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct UpdateReferralConfig<'info> {
    #[account(mut, has_one = admin)]
    pub config: Account<'info, ReferralConfig>,

    pub admin: Signer<'info>,
}

pub fn update_config(
    ctx: Context<UpdateReferralConfig>,
    new_protocol_wallet: Pubkey,
    new_nft_mint: Pubkey,
    new_claim_fee_lamports: u64,
) -> Result<()> {
    let referral_config = &mut ctx.accounts.referral_config;

    require!(new_claim_fee_lamports > 0, ReferralError::InvalidClaimFee);

    config.protocol_receive_wallet = new_protocol_wallet;
    config.official_nft_mint = new_nft_mint;
    config.claim_fee_lamports = new_claim_fee_lamports;

    Ok(())
}
