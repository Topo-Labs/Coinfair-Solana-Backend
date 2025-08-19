use crate::error::ReferralError;
use crate::states::ReferralAccount;
use anchor_lang::prelude::*;

// Usless

#[derive(Accounts)]
pub struct SetUpper<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        seeds = [b"referral", user.key().as_ref()],
        bump,
        constraint = referral_account.upper.is_none() @ ReferralError::AlreadyHasParent,
    )]
    pub referral_account: Account<'info, ReferralAccount>,

    /// 上级
    #[account(mut)]
    /// CHECK: This is a trusted account set by the program logic
    pub upper: AccountInfo<'info>,

    #[account(
        seeds = [b"referral", upper.key().as_ref()],
        bump,
    )]
    pub upper_referral_account: Option<Account<'info, ReferralAccount>>,
}

pub fn set_upper(ctx: Context<SetUpper>) -> Result<()> {
    let user = &ctx.accounts.user;
    let upper = &ctx.accounts.upper;
    let referral_account = &mut ctx.accounts.referral_account;

    // 防止自己绑定自己
    require_keys_neq!(user.key(), upper.key(), ReferralError::CannotReferSelf);

    // 设置 upper（上级）
    referral_account.upper = Some(upper.key());

    // 设置 upper_upper（上上级）
    if let Some(upper_referral) = ctx.accounts.upper_referral_account.as_ref() {
        referral_account.upper_upper = upper_referral.upper;
    } else {
        referral_account.upper_upper = None;
    }

    Ok(())
}
