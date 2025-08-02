use crate::states::ReferralAccount;
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct GetUpper<'info> {
    /// CHECK: 用户地址，客户端传过来的
    pub user: UncheckedAccount<'info>,

    // 读取用户对应的 ReferralAccount
    #[account(
        seeds = [b"referral", user.key().as_ref()],
        bump,
    )]
    pub referral_account: Account<'info, ReferralAccount>,
}

/// Use for Client
pub fn get_upper(ctx: Context<GetUpper>) -> Result<Option<Pubkey>> {
    let referral = &ctx.accounts.referral_account;
    Ok(referral.upper)
}

/// Use for CPI
pub fn get_upper_for_cpi(ctx: Context<GetUpper>) -> Result<()> {
    let referral = &ctx.accounts.referral_account;
    msg!("upper: {:?}", referral.upper);
    Ok(())
}
