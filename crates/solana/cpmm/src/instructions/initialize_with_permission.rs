use crate::curve::CurveCalculator;
use crate::error::ErrorCode;
use crate::initialize::create_pool;
use crate::states::*;
use crate::utils::*;
use anchor_lang::{
    accounts::interface_account::InterfaceAccount,
    prelude::*,
    solana_program::{clock, program::invoke, system_instruction},
};
use anchor_spl::{
    associated_token::AssociatedToken,
    token::spl_token,
    token::Token,
    token_interface::{Mint, TokenAccount, TokenInterface},
};
use spl_token_2022;
use std::ops::Deref;

/// 池子创建：权限模式
/// 作用：需要特殊权限才能创建的池子，支持代付模式（付费者和创建者可以是不同人），适用于特殊用途、需要管控的池子
#[derive(Accounts)]
pub struct InitializeWithPermission<'info> {
    /// 支付创建池子费用的地址。可以是任何人
    #[account(mut)]
    pub payer: Signer<'info>,

    /// CHECK: 池子创建者
    pub creator: UncheckedAccount<'info>,

    /// 池子所属的配置。
    pub amm_config: Box<Account<'info, AmmConfig>>,

    /// CHECK:
    /// 池子金库和LP铸币权限
    #[account(
        seeds = [
            crate::AUTH_SEED.as_bytes(),
        ],
        bump,
    )]
    pub authority: UncheckedAccount<'info>,

    /// CHECK: Initialize an account to store the pool state
    /// PDA account:
    /// seeds = [
    /// POOL_SEED.as_bytes(),
    /// amm_config.key().as_ref(),
    /// token_0_mint.key().as_ref(),
    /// token_1_mint.key().as_ref(),
    /// ],
    ///
    /// Or random account: must be signed by cli
    #[account(mut)]
    pub pool_state: UncheckedAccount<'info>,

    /// Token_0铸币，键值必须小于token_1铸币。
    #[account(
        constraint = token_0_mint.key() < token_1_mint.key(),
        mint::token_program = token_0_program,
    )]
    pub token_0_mint: Box<InterfaceAccount<'info, Mint>>,

    /// Token_1铸币，键值必须大于token_0铸币。
    #[account(
        mint::token_program = token_1_program,
    )]
    pub token_1_mint: Box<InterfaceAccount<'info, Mint>>,

    /// 池子LP铸币
    #[account(
        init,
        seeds = [
            POOL_LP_MINT_SEED.as_bytes(),
            pool_state.key().as_ref(),
        ],
        bump,
        mint::decimals = 9,
        mint::authority = authority,
        payer = payer,
        mint::token_program = token_program,
    )]
    pub lp_mint: Box<InterfaceAccount<'info, Mint>>,

    /// 付款人token0账户
    #[account(
        mut,
        token::mint = token_0_mint,
        token::authority = payer,
    )]
    pub payer_token_0: Box<InterfaceAccount<'info, TokenAccount>>,

    /// 付款人token1账户
    #[account(
        mut,
        token::mint = token_1_mint,
        token::authority = payer,
    )]
    pub payer_token_1: Box<InterfaceAccount<'info, TokenAccount>>,

    /// 付款人LP代币账户
    #[account(
        init,
        associated_token::mint = lp_mint,
        associated_token::authority = payer,
        payer = payer,
        token::token_program = token_program,
    )]
    pub payer_lp_token: Box<InterfaceAccount<'info, TokenAccount>>,

    /// CHECK: 池子的Token_0金库，由合约创建
    #[account(
        mut,
        seeds = [
            POOL_VAULT_SEED.as_bytes(),
            pool_state.key().as_ref(),
            token_0_mint.key().as_ref()
        ],
        bump,
    )]
    pub token_0_vault: UncheckedAccount<'info>,

    /// CHECK: 池子的Token_1金库，由合约创建
    #[account(
        mut,
        seeds = [
            POOL_VAULT_SEED.as_bytes(),
            pool_state.key().as_ref(),
            token_1_mint.key().as_ref()
        ],
        bump,
    )]
    pub token_1_vault: UncheckedAccount<'info>,

    /// 创建池子费用账户
    #[account(
        mut,
        address= crate::create_pool_fee_reveiver::ID,
    )]
    pub create_pool_fee: Box<InterfaceAccount<'info, TokenAccount>>,

    /// 存储预言机观察数据的账户
    #[account(
        init,
        seeds = [
            OBSERVATION_SEED.as_bytes(),
            pool_state.key().as_ref(),
        ],
        bump,
        payer = payer,
        space = ObservationState::LEN
    )]
    pub observation_state: AccountLoader<'info, ObservationState>,

    /// CHECK: 用于权限验证的PDA账户。
    #[account(
        seeds = [
            PERMISSION_SEED.as_bytes(),
            payer.key().as_ref(),
        ],
        bump,
    )]
    pub permission: Box<Account<'info, Permission>>,

    /// 创建铸币账户和铸造代币的程序
    pub token_program: Program<'info, Token>,
    /// Spl代币程序或代币程序2022
    pub token_0_program: Interface<'info, TokenInterface>,
    /// Spl代币程序或代币程序2022
    pub token_1_program: Interface<'info, TokenInterface>,
    /// 创建用于接收仓位NFT的ATA的程序
    pub associated_token_program: Program<'info, AssociatedToken>,
    /// 创建新程序账户
    pub system_program: Program<'info, System>,
}

pub fn initialize_with_permission(
    ctx: Context<InitializeWithPermission>,
    init_amount_0: u64,
    init_amount_1: u64,
    open_time: u64,
    creator_fee_on: CreatorFeeOn,
) -> Result<()> {
    if !(is_supported_mint(&ctx.accounts.token_0_mint).unwrap()
        && is_supported_mint(&ctx.accounts.token_1_mint).unwrap())
    {
        return err!(ErrorCode::NotSupportMint);
    }

    if ctx.accounts.amm_config.disable_create_pool {
        return err!(ErrorCode::NotApproved);
    }
    let mut open_time = open_time;
    let block_timestamp = clock::Clock::get()?.unix_timestamp as u64;
    if open_time <= block_timestamp {
        open_time = block_timestamp + 1;
    }
    // 由于栈/堆限制，我们必须自己创建冗余的新账户。
    create_token_account(
        &ctx.accounts.authority.to_account_info(),
        &ctx.accounts.payer.to_account_info(),
        &ctx.accounts.token_0_vault.to_account_info(),
        &ctx.accounts.token_0_mint.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        &ctx.accounts.token_0_program.to_account_info(),
        &[
            POOL_VAULT_SEED.as_bytes(),
            ctx.accounts.pool_state.key().as_ref(),
            ctx.accounts.token_0_mint.key().as_ref(),
            &[ctx.bumps.token_0_vault][..],
        ],
    )?;

    create_token_account(
        &ctx.accounts.authority.to_account_info(),
        &ctx.accounts.payer.to_account_info(),
        &ctx.accounts.token_1_vault.to_account_info(),
        &ctx.accounts.token_1_mint.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        &ctx.accounts.token_1_program.to_account_info(),
        &[
            POOL_VAULT_SEED.as_bytes(),
            ctx.accounts.pool_state.key().as_ref(),
            ctx.accounts.token_1_mint.key().as_ref(),
            &[ctx.bumps.token_1_vault][..],
        ],
    )?;

    let pool_state_loader = create_pool(
        &ctx.accounts.payer.to_account_info(),
        &ctx.accounts.pool_state.to_account_info(),
        &ctx.accounts.amm_config.to_account_info(),
        &ctx.accounts.token_0_mint.to_account_info(),
        &ctx.accounts.token_1_mint.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
    )?;
    let pool_state = &mut pool_state_loader.load_init()?;

    let mut observation_state = ctx.accounts.observation_state.load_init()?;
    observation_state.pool_id = ctx.accounts.pool_state.key();

    transfer_from_user_to_pool_vault(
        ctx.accounts.payer.to_account_info(),
        ctx.accounts.payer_token_0.to_account_info(),
        ctx.accounts.token_0_vault.to_account_info(),
        ctx.accounts.token_0_mint.to_account_info(),
        ctx.accounts.token_0_program.to_account_info(),
        init_amount_0,
        ctx.accounts.token_0_mint.decimals,
    )?;

    transfer_from_user_to_pool_vault(
        ctx.accounts.payer.to_account_info(),
        ctx.accounts.payer_token_1.to_account_info(),
        ctx.accounts.token_1_vault.to_account_info(),
        ctx.accounts.token_1_mint.to_account_info(),
        ctx.accounts.token_1_program.to_account_info(),
        init_amount_1,
        ctx.accounts.token_1_mint.decimals,
    )?;

    let token_0_vault = spl_token_2022::extension::StateWithExtensions::<spl_token_2022::state::Account>::unpack(
        ctx.accounts.token_0_vault.to_account_info().try_borrow_data()?.deref(),
    )?
    .base;
    let token_1_vault = spl_token_2022::extension::StateWithExtensions::<spl_token_2022::state::Account>::unpack(
        ctx.accounts.token_1_vault.to_account_info().try_borrow_data()?.deref(),
    )?
    .base;

    CurveCalculator::validate_supply(token_0_vault.amount, token_1_vault.amount)?;

    let liquidity = U128::from(token_0_vault.amount)
        .checked_mul(token_1_vault.amount.into())
        .unwrap()
        .integer_sqrt()
        .as_u64();
    let lock_lp_amount = 100;
    msg!(
        "liquidity:{}, lock_lp_amount:{}, vault_0_amount:{},vault_1_amount:{}",
        liquidity,
        lock_lp_amount,
        token_0_vault.amount,
        token_1_vault.amount
    );
    token::token_mint_to(
        ctx.accounts.authority.to_account_info(),
        ctx.accounts.token_program.to_account_info(),
        ctx.accounts.lp_mint.to_account_info(),
        ctx.accounts.payer_lp_token.to_account_info(),
        liquidity
            .checked_sub(lock_lp_amount)
            .ok_or(ErrorCode::InitLpAmountTooLess)?,
        &[&[crate::AUTH_SEED.as_bytes(), &[ctx.bumps.authority]]],
    )?;

    // 收取创建池子的费用
    if ctx.accounts.amm_config.create_pool_fee != 0 {
        invoke(
            &system_instruction::transfer(
                ctx.accounts.payer.key,
                &ctx.accounts.create_pool_fee.key(),
                u64::from(ctx.accounts.amm_config.create_pool_fee),
            ),
            &[
                ctx.accounts.payer.to_account_info(),
                ctx.accounts.create_pool_fee.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            ],
        )?;
        invoke(
            &spl_token::instruction::sync_native(ctx.accounts.token_program.key, &ctx.accounts.create_pool_fee.key())?,
            &[
                ctx.accounts.token_program.to_account_info(),
                ctx.accounts.create_pool_fee.to_account_info(),
            ],
        )?;
    }

    pool_state.initialize(
        ctx.bumps.authority,
        liquidity,
        open_time,
        ctx.accounts.creator.key(),
        ctx.accounts.amm_config.key(),
        ctx.accounts.token_0_vault.key(),
        ctx.accounts.token_1_vault.key(),
        &ctx.accounts.token_0_mint,
        &ctx.accounts.token_1_mint,
        ctx.accounts.lp_mint.key(),
        ctx.accounts.lp_mint.decimals,
        ctx.accounts.observation_state.key(),
        creator_fee_on,
        true,
    );

    Ok(())
}
