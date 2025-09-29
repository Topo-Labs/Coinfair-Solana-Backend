use crate::error::ErrorCode;
use crate::states::*;
use crate::utils::*;
use anchor_lang::prelude::*;
use anchor_spl::token::Token;
use anchor_spl::token_interface::Mint;
use anchor_spl::token_interface::Token2022;
use anchor_spl::token_interface::TokenAccount;

#[derive(Accounts)]
pub struct CollectProtocolFee<'info> {
    /// 目前只有管理员或所有者可以收取费用
    #[account(constraint = (owner.key() == amm_config.protocol_owner || owner.key() == crate::admin::ID) @ ErrorCode::InvalidOwner)]
    pub owner: Signer<'info>,

    /// CHECK: 池子金库和LP铸币权限
    #[account(
        seeds = [
            crate::AUTH_SEED.as_bytes(),
        ],
        bump,
    )]
    pub authority: UncheckedAccount<'info>,

    /// 池子状态存储累积的协议费用金额
    #[account(mut)]
    pub pool_state: AccountLoader<'info, PoolState>,

    /// AMM配置账户存储所有者
    #[account(address = pool_state.load()?.amm_config)]
    pub amm_config: Account<'info, AmmConfig>,

    /// 持有token_0池子代币的地址
    #[account(
        mut,
        constraint = token_0_vault.key() == pool_state.load()?.token_0_vault
    )]
    pub token_0_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    /// 持有token_1池子代币的地址
    #[account(
        mut,
        constraint = token_1_vault.key() == pool_state.load()?.token_1_vault
    )]
    pub token_1_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    /// token_0金库的铸币
    #[account(
        address = token_0_vault.mint
    )]
    pub vault_0_mint: Box<InterfaceAccount<'info, Mint>>,

    /// token_1金库的铸币
    #[account(
        address = token_1_vault.mint
    )]
    pub vault_1_mint: Box<InterfaceAccount<'info, Mint>>,

    /// 接收收集的token_0协议费用的地址
    #[account(mut)]
    pub recipient_token_0_account: Box<InterfaceAccount<'info, TokenAccount>>,

    /// 接收收集的token_1协议费用的地址
    #[account(mut)]
    pub recipient_token_1_account: Box<InterfaceAccount<'info, TokenAccount>>,

    /// 执行代币转账的SPL程序
    pub token_program: Program<'info, Token>,

    /// 执行代币转账的SPL程序2022
    pub token_program_2022: Program<'info, Token2022>,
}

pub fn collect_protocol_fee(
    ctx: Context<CollectProtocolFee>,
    amount_0_requested: u64,
    amount_1_requested: u64,
) -> Result<()> {
    let amount_0: u64;
    let amount_1: u64;
    let auth_bump: u8;
    {
        let mut pool_state = ctx.accounts.pool_state.load_mut()?;

        amount_0 = amount_0_requested.min(pool_state.protocol_fees_token_0);
        amount_1 = amount_1_requested.min(pool_state.protocol_fees_token_1);

        pool_state.protocol_fees_token_0 = pool_state.protocol_fees_token_0.checked_sub(amount_0).unwrap();
        pool_state.protocol_fees_token_1 = pool_state.protocol_fees_token_1.checked_sub(amount_1).unwrap();

        auth_bump = pool_state.auth_bump;
        pool_state.recent_epoch = Clock::get()?.epoch;
    }
    transfer_from_pool_vault_to_user(
        ctx.accounts.authority.to_account_info(),
        ctx.accounts.token_0_vault.to_account_info(),
        ctx.accounts.recipient_token_0_account.to_account_info(),
        ctx.accounts.vault_0_mint.to_account_info(),
        if ctx.accounts.vault_0_mint.to_account_info().owner == ctx.accounts.token_program.key {
            ctx.accounts.token_program.to_account_info()
        } else {
            ctx.accounts.token_program_2022.to_account_info()
        },
        amount_0,
        ctx.accounts.vault_0_mint.decimals,
        &[&[crate::AUTH_SEED.as_bytes(), &[auth_bump]]],
    )?;

    transfer_from_pool_vault_to_user(
        ctx.accounts.authority.to_account_info(),
        ctx.accounts.token_1_vault.to_account_info(),
        ctx.accounts.recipient_token_1_account.to_account_info(),
        ctx.accounts.vault_1_mint.to_account_info(),
        if ctx.accounts.vault_1_mint.to_account_info().owner == ctx.accounts.token_program.key {
            ctx.accounts.token_program.to_account_info()
        } else {
            ctx.accounts.token_program_2022.to_account_info()
        },
        amount_1,
        ctx.accounts.vault_1_mint.decimals,
        &[&[crate::AUTH_SEED.as_bytes(), &[auth_bump]]],
    )?;

    Ok(())
}
