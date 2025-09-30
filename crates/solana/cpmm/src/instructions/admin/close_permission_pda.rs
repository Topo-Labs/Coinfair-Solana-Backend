use crate::error::ErrorCode;
use crate::states::*;
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct ClosePermissionPda<'info> {
    #[account(
        mut,
        address = crate::admin::ID @ ErrorCode::InvalidOwner
    )]
    pub owner: Signer<'info>,

    /// CHECK: 权限账户权限
    pub permission_authority: UncheckedAccount<'info>,

    /// 初始化配置状态账户来存储协议所有者地址和费率。
    #[account(
        mut,
        seeds = [
            PERMISSION_SEED.as_bytes(),
            permission_authority.key().as_ref()
        ],
        bump,
        close = owner
    )]
    pub permission: Account<'info, Permission>,

    pub system_program: Program<'info, System>,
}

pub fn close_permission_pda(_ctx: Context<ClosePermissionPda>) -> Result<()> {
    Ok(())
}
