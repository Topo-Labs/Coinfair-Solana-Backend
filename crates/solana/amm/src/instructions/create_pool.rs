use crate::error::ErrorCode;
use crate::states::*;
use crate::{libraries::tick_math, util};
use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};
// use solana_program::{program::invoke_signed, system_instruction};
#[derive(Accounts)]
pub struct CreatePool<'info> {
    /// Address paying to create the pool. Can be anyone
    #[account(mut)]
    pub pool_creator: Signer<'info>,

    /// Which config the pool belongs to.
    pub amm_config: Box<Account<'info, AmmConfig>>,

    /// Initialize an account to store the pool state
    #[account(
        init,
        seeds = [
            POOL_SEED.as_bytes(),
            amm_config.key().as_ref(),
            token_mint_0.key().as_ref(),
            token_mint_1.key().as_ref(),
        ],
        bump,
        payer = pool_creator,
        space = PoolState::LEN
    )]
    pub pool_state: AccountLoader<'info, PoolState>,

    /// Base mint - must be either token_mint_0 or token_mint_1
    #[account(
        constraint = base_mint.key() == token_mint_0.key() || base_mint.key() == token_mint_1.key() @ ErrorCode::InvalidBaseMint,
    )]
    pub base_mint: Box<InterfaceAccount<'info, Mint>>,

    /// Token_0 mint, the key must be smaller then token_1 mint.
    #[account(
        constraint = token_mint_0.key() < token_mint_1.key(),
        mint::token_program = token_program_0
    )]
    pub token_mint_0: Box<InterfaceAccount<'info, Mint>>,

    /// Token_1 mint
    #[account(
        mint::token_program = token_program_1
    )]
    pub token_mint_1: Box<InterfaceAccount<'info, Mint>>,

    /// Token_0 vault for the pool
    #[account(
        init,
        seeds =[
            POOL_VAULT_SEED.as_bytes(),
            pool_state.key().as_ref(),
            token_mint_0.key().as_ref(),
        ],
        bump,
        payer = pool_creator,
        token::mint = token_mint_0,
        token::authority = pool_state,
        token::token_program = token_program_0,
    )]
    pub token_vault_0: Box<InterfaceAccount<'info, TokenAccount>>,

    /// Token_1 vault for the pool
    #[account(
        init,
        seeds =[
            POOL_VAULT_SEED.as_bytes(),
            pool_state.key().as_ref(),
            token_mint_1.key().as_ref(),
        ],
        bump,
        payer = pool_creator,
        token::mint = token_mint_1,
        token::authority = pool_state,
        token::token_program = token_program_1,
    )]
    pub token_vault_1: Box<InterfaceAccount<'info, TokenAccount>>,

    /// Initialize an account to store oracle observations
    #[account(
        init,
        seeds = [
            OBSERVATION_SEED.as_bytes(),
            pool_state.key().as_ref(),
        ],
        bump,
        payer = pool_creator,
        space = ObservationState::LEN
    )]
    pub observation_state: AccountLoader<'info, ObservationState>,

    /// Initialize an account to store if a tick array is initialized.
    #[account(
        init,
        seeds = [
            POOL_TICK_ARRAY_BITMAP_SEED.as_bytes(),
            pool_state.key().as_ref(),
        ],
        bump,
        payer = pool_creator,
        space = TickArrayBitmapExtension::LEN
    )]
    pub tick_array_bitmap: AccountLoader<'info, TickArrayBitmapExtension>,

    /// Spl token program or token program 2022
    pub token_program_0: Interface<'info, TokenInterface>,
    /// Spl token program or token program 2022
    pub token_program_1: Interface<'info, TokenInterface>,
    /// To create a new program account
    pub system_program: Program<'info, System>,
    /// Sysvar for program account
    pub rent: Sysvar<'info, Rent>,
}

pub fn create_pool(
    ctx: Context<CreatePool>,
    sqrt_price_x64: u128, // 现在实际是 price^(1/5) in Q64.64 format
    open_time: u64,
) -> Result<()> {
    println!("prorgam/create_pool, sqrt_price_x64: {}", sqrt_price_x64);
    if !(util::is_supported_mint(&ctx.accounts.token_mint_0).unwrap()
        && util::is_supported_mint(&ctx.accounts.token_mint_1).unwrap())
    {
        return err!(ErrorCode::NotSupportMint);
    }
    let pool_id = ctx.accounts.pool_state.key();
    let mut pool_state = ctx.accounts.pool_state.load_init()?;

    // 确定是否需要交换 token 顺序
    let is_base_token_0 = ctx.accounts.base_mint.key() == ctx.accounts.token_mint_0.key();

    // 根据 base_mint 决定最终的 token 顺序和价格
    let (
        final_token_mint_0,
        final_token_mint_1,
        final_token_vault_0,
        final_token_vault_1,
        final_sqrt_price,
    ) = if is_base_token_0 {
        // base_mint 是 token_mint_0，保持原顺序
        (
            ctx.accounts.token_mint_0.as_ref(),
            ctx.accounts.token_mint_1.as_ref(),
            ctx.accounts.token_vault_0.key(),
            ctx.accounts.token_vault_1.key(),
            sqrt_price_x64,
        )
    } else {
        // base_mint 是 token_mint_1，需要交换顺序
        // 交换时，价格需要取倒数
        let inverted_price = calculate_inverted_price(sqrt_price_x64)?;
        (
            ctx.accounts.token_mint_1.as_ref(),
            ctx.accounts.token_mint_0.as_ref(),
            ctx.accounts.token_vault_1.key(),
            ctx.accounts.token_vault_0.key(),
            inverted_price,
        )
    };

    // 使用调整后的价格计算 tick
    let tick = tick_math::get_tick_at_sqrt_price(final_sqrt_price)?;

    #[cfg(feature = "enable-log")]
    msg!(
        "create pool, base_mint: {}, is_base_token_0: {}, init_price: {}, init_tick: {}",
        ctx.accounts.base_mint.key(),
        is_base_token_0,
        final_sqrt_price,
        tick
    );
    // init observation
    ctx.accounts
        .observation_state
        .load_init()?
        .initialize(pool_id)?;

    let bump = ctx.bumps.pool_state;

    // 使用调整后的参数初始化 pool_state
    pool_state.initialize(
        bump,
        final_sqrt_price,
        open_time,
        tick,
        ctx.accounts.pool_creator.key(),
        final_token_vault_0,
        final_token_vault_1,
        ctx.accounts.amm_config.as_ref(),
        final_token_mint_0,
        final_token_mint_1,
        ctx.accounts.observation_state.key(),
    )?;

    ctx.accounts
        .tick_array_bitmap
        .load_init()?
        .initialize(pool_id);

    // 事件中使用调整后的顺序
    emit!(PoolCreatedEvent {
        token_mint_0: final_token_mint_0.key(),
        token_mint_1: final_token_mint_1.key(),
        tick_spacing: ctx.accounts.amm_config.tick_spacing,
        pool_state: ctx.accounts.pool_state.key(),
        sqrt_price_x64: final_sqrt_price,
        tick,
        token_vault_0: final_token_vault_0,
        token_vault_1: final_token_vault_1,
    });
    Ok(())
}

// fn calculate_inverted_price(sqrt_price_x64: u128) -> Result<u128> {
//     let one_q64 = 1u128 << 64;
//     let inverted = one_q64
//         .checked_mul(one_q64)
//         .ok_or(ErrorCode::MathOverflow)?
//         .checked_div(sqrt_price_x64)
//         .ok_or(ErrorCode::DivisionByZero)?;
//     Ok(inverted)
// }

fn calculate_inverted_price(sqrt_price_x64: u128) -> Result<u128> {
    require!(sqrt_price_x64 > 0, ErrorCode::DivisionByZero);

    let one_q64 = 1u128 << 64;

    // 对于 >= 1.0 的价格，重排计算顺序
    if sqrt_price_x64 >= one_q64 {
        let intermediate = one_q64 / sqrt_price_x64;
        if let Some(result) = intermediate.checked_mul(one_q64) {
            return Ok(result);
        }
    }

    // 回退到浮点运算（精度略低但安全）
    let sqrt_price_f64 = sqrt_price_x64 as f64;
    let one_q64_f64 = one_q64 as f64;
    let result_f64 = (one_q64_f64 * one_q64_f64) / sqrt_price_f64;

    require!(result_f64 < (u128::MAX as f64), ErrorCode::MathOverflow);
    Ok(result_f64 as u128)
}
