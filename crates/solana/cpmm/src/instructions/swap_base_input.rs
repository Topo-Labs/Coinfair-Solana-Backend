use crate::curve::calculator::CurveCalculator;
use crate::error::ErrorCode;
use crate::states::*;
use crate::utils::token::*;
use anchor_lang::prelude::*;
use anchor_lang::solana_program;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};
use referral::{program::Referral, states::ReferralAccount};

#[derive(Accounts)]
pub struct Swap<'info> {
    /// 执行交换的用户
    pub payer: Signer<'info>,

    /// CHECK: 池子金库和LP铸币权限
    #[account(
        seeds = [
            crate::AUTH_SEED.as_bytes(),
        ],
        bump,
    )]
    pub authority: UncheckedAccount<'info>,

    /// 用于读取协议费用的工厂状态
    #[account(address = pool_state.load()?.amm_config)]
    pub amm_config: Box<Account<'info, AmmConfig>>,

    /// 将要执行交换的池子程序账户
    #[account(mut)]
    pub pool_state: AccountLoader<'info, PoolState>,

    /// 用户输入代币账户
    #[account(mut)]
    pub input_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    /// 用户输出代币账户
    #[account(mut)]
    pub output_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    /// 输入代币的金库账户
    #[account(
        mut,
        constraint = input_vault.key() == pool_state.load()?.token_0_vault || input_vault.key() == pool_state.load()?.token_1_vault
    )]
    pub input_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    /// 输出代币的金库账户
    #[account(
        mut,
        constraint = output_vault.key() == pool_state.load()?.token_0_vault || output_vault.key() == pool_state.load()?.token_1_vault
    )]
    pub output_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    /// 输入代币转账的SPL程序
    pub input_token_program: Interface<'info, TokenInterface>,

    /// 输出代币转账的SPL程序
    pub output_token_program: Interface<'info, TokenInterface>,

    /// 输入代币的铸币
    #[account(
        address = input_vault.mint
    )]
    pub input_token_mint: Box<InterfaceAccount<'info, Mint>>,

    /// 输出代币的铸币
    #[account(
        address = output_vault.mint
    )]
    pub output_token_mint: Box<InterfaceAccount<'info, Mint>>,

    /// 最近预言机观察的程序账户
    #[account(mut, address = pool_state.load()?.observation_key)]
    pub observation_state: AccountLoader<'info, ObservationState>,

    ////////////////// 新增 //////////////////

    /// 指定收取手续费的代币Mint（upper和upper_upper对应分佣账户也对应该代币）
    pub reward_mint: Box<InterfaceAccount<'info, Mint>>,

    /// The user PDA of referral_account（用于获取payer的upper)
    #[account(
        seeds = [b"referral", payer.key().as_ref()],
        bump,
        seeds::program = referral.key()
    )]
    pub payer_referral: Option<Account<'info, ReferralAccount>>,

    /// CHECK: 仅用于与 payer_referral.upper 对比，不读取数据
    #[account(
        constraint = 
        payer_referral.is_none() || payer_referral.as_ref().unwrap().upper.is_none() || upper.key() == payer_referral.as_ref().unwrap().upper.unwrap()
        @ ErrorCode::UpperAccountMismatch
    )]
    pub upper: Option<UncheckedAccount<'info>>,

    /// upper接收分佣的 ATA（用于收手续费奖励）(该账户 owner 应为 `upper`，mint 应为 swap 所涉及的 token)
    #[account(
        mut,
        constraint = payer_referral.is_none() || payer_referral.as_ref().unwrap().upper.is_none()|| (
            upper_token_account.owner == upper.as_ref().unwrap().key() &&
            upper_token_account.mint == reward_mint.key() //Token_Mint 
        )
        @ ErrorCode::UpperTokenAccountMismatch
    )]
    pub upper_token_account: Option<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        seeds = [b"referral", upper.as_ref().unwrap().key().as_ref()],
        bump,
        seeds::program = referral.key(),
        constraint = payer_referral.is_some() && payer_referral.as_ref().unwrap().upper.is_some()
    )]
    pub upper_referral: Option<Account<'info, ReferralAccount>>,


    /// CHECK: 仅用于与 payer_referral.upper_upper 对比，不读取数据
    #[account(
        constraint = upper_referral.is_none() || upper_referral.as_ref().unwrap().upper.is_none() || (
            upper_upper.key() == upper_referral.as_ref().unwrap().upper.unwrap()
        )
        @ ErrorCode::UpperUpperMismatch
        
    )]
    pub upper_upper: Option<UncheckedAccount<'info>>,

    /// 可选的上上级奖励账户
    #[account(
        mut,
        constraint = upper_referral.is_none() || upper_referral.as_ref().unwrap().upper.is_none() || (
            upper_upper_token_account.owner == upper_upper.as_ref().unwrap().key() &&
            upper_upper_token_account.mint == reward_mint.key()
        )
        @ ErrorCode::UpperUpperTokenAccountMismatch
    )]
    pub upper_upper_token_account: Option<InterfaceAccount<'info, TokenAccount>>,

    /// 项目方
    #[account(
        mut,
        constraint = project_token_account.owner == pool_state.load()?.pool_creator @ ErrorCode::ProjectTokenAccountMismatch
    )]
    pub project_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    pub system_program: Program<'info, System>,

    pub associated_token_program: Program<'info, anchor_spl::associated_token::AssociatedToken>,

    #[account(address = referral::id())]
    pub referral: Program<'info, Referral>,
}

pub fn swap_base_input(ctx: Context<Swap>, amount_in: u64, minimum_amount_out: u64) -> Result<()> {
    // 前置检查与验证（是否允许交换操作；是否已到开放时间）
    let block_timestamp = solana_program::clock::Clock::get()?.unix_timestamp as u64;
    let pool_id = ctx.accounts.pool_state.key();
    let pool_state = &mut ctx.accounts.pool_state.load_mut()?;
    if !pool_state.get_status_by_bit(PoolStatusBitIndex::Swap)
        || block_timestamp < pool_state.open_time
    {
        return err!(ErrorCode::NotApproved);
    }

    // 2.转账费用处理（某些SPL代币有转账费用，需计算扣除之后的实际转入金额）
    let transfer_fee =
        get_transfer_fee(&ctx.accounts.input_token_mint.to_account_info(), amount_in)?;
    let actual_amount_in = amount_in.saturating_sub(transfer_fee);
    require_gt!(actual_amount_in, 0);

    // 3.获取交换参数
        // 确定交易方向（token0 → token1 或 token1 → token0）
        // 获取当前池中两种代币的数量
        // 获取当前价格信息
        // 确定创建者费用的收取方式
    let SwapParams {
        trade_direction,
        total_input_token_amount,
        total_output_token_amount,
        token_0_price_x64,
        token_1_price_x64,
        is_creator_fee_on_input,
    } = pool_state.get_swap_params(
        ctx.accounts.input_vault.key(),
        ctx.accounts.output_vault.key(),
        ctx.accounts.input_vault.amount,
        ctx.accounts.output_vault.amount,
    )?;

    // 4.恒定乘积验证准备（计算交换前的乘积，以后续验证）
    let constant_before = u128::from(total_input_token_amount)
        .checked_mul(u128::from(total_output_token_amount))
        .unwrap();

    // 5.费用计算与交换计算
        // 计算输出金额：amount_out = y - (x × y) / (x + amount_in)
        // 扣除各种费用：
           // 交易费用 (trade_fee)
           // 创建者费用 (creator_fee)
           // 协议费用 (protocol_fee)
           // 基金费用 (fund_fee)
    let creator_fee_rate =
        pool_state.adjust_creator_fee_rate(ctx.accounts.amm_config.creator_fee_rate);

    let result = CurveCalculator::swap_base_input(
        trade_direction,
        u128::from(actual_amount_in),
        u128::from(total_input_token_amount),
        u128::from(total_output_token_amount),
        ctx.accounts.amm_config.trade_fee_rate,    // 交易费率
        creator_fee_rate,                          // 创建者费用
        ctx.accounts.amm_config.protocol_fee_rate, // 协议费率
        ctx.accounts.amm_config.fund_fee_rate,     // 基金费率
        is_creator_fee_on_input,                   // 创建者费用收取方式
    )
    .ok_or(ErrorCode::ZeroTradingTokens)?;

    println!("result: {:?}", result);

    // 6.验证交换之后的常量乘积
    let constant_after = u128::from(result.new_input_vault_amount)
        .checked_mul(u128::from(result.new_output_vault_amount))
        .unwrap();

    #[cfg(feature = "enable-log")]
    msg!(
        "input_amount:{}, output_amount:{}, trade_fee:{}, input_transfer_fee:{}, constant_before:{},constant_after:{}, is_creator_fee_on_input:{}, creator_fee:{}",
        result.input_amount,
        result.output_amount,
        result.trade_fee,
        transfer_fee,
        constant_before,
        constant_after,
        is_creator_fee_on_input,
        result.creator_fee,
    );
    require_eq!(
        u64::try_from(result.input_amount).unwrap(),
        actual_amount_in
    );

    // 7.滑点保护（计算用户在扣除输出代币的转账费用之后，实际收到的代币数量，是否大于用户设置的最小输出金额）
    let (input_transfer_amount, input_transfer_fee) = (amount_in, transfer_fee);
    let (output_transfer_amount, output_transfer_fee) = {
        let amount_out = u64::try_from(result.output_amount).unwrap();
        let transfer_fee = get_transfer_fee(
            &ctx.accounts.output_token_mint.to_account_info(),
            amount_out,
        )?;
        let amount_received = amount_out.checked_sub(transfer_fee).unwrap();
        require_gt!(amount_received, 0);
        require_gte!(
            amount_received,
            minimum_amount_out,
            ErrorCode::ExceededSlippage
        );
        (amount_out, transfer_fee)
    };

    // 8.池子根据交易方向来更新费用
    pool_state.update_fees(
        u64::try_from(result.protocol_fee).unwrap(),
        u64::try_from(result.fund_fee).unwrap(),
        u64::try_from(result.creator_fee).unwrap(),
        trade_direction,
    )?;

    emit!(SwapEvent {
        pool_id,
        input_vault_before: total_input_token_amount,
        output_vault_before: total_output_token_amount,
        input_amount: u64::try_from(result.input_amount).unwrap(),
        output_amount: u64::try_from(result.output_amount).unwrap(),
        input_transfer_fee,
        output_transfer_fee,
        base_input: true,
        input_mint: ctx.accounts.input_token_mint.key(),
        output_mint: ctx.accounts.output_token_mint.key(),
        trade_fee: u64::try_from(result.trade_fee).unwrap(),
        creator_fee: u64::try_from(result.creator_fee).unwrap(),
        creator_fee_on_input: is_creator_fee_on_input,
    });
    require_gte!(constant_after, constant_before);

    let total_reward_fee = 0;

    // 9.代币转账执行
    transfer_from_pool_vault_to_uppers_and_project(
        &ctx.accounts.pool_state,
        &ctx.accounts.output_vault.to_account_info(),
        &ctx.accounts.project_token_account.to_account_info(),
        ctx.accounts.upper_token_account.as_ref().map(|acc| acc.to_account_info()),
        ctx.accounts.upper_upper_token_account.as_ref().map(|acc| acc.to_account_info()),
        ctx.accounts.reward_mint.to_account_info(),
        ctx.accounts.output_token_mint.decimals,
        ctx.accounts.output_token_program.to_account_info(),
        total_reward_fee,
        &[&[crate::AUTH_SEED.as_bytes(), &[pool_state.auth_bump]]],
        //事件触发所需字段
        ctx.accounts.reward_mint.key(),
        ctx.accounts.payer.key(),
        ctx.accounts.pool_state.load()?.pool_creator,
        ctx.accounts.upper.as_ref().map(|u| u.key()),
        ctx.accounts.upper_upper.as_ref().map(|u| u.key()),
    )?;


    // 9.代币转账执行
    transfer_from_user_to_pool_vault(
        ctx.accounts.payer.to_account_info(),
        ctx.accounts.input_token_account.to_account_info(),
        ctx.accounts.input_vault.to_account_info(),
        ctx.accounts.input_token_mint.to_account_info(),
        ctx.accounts.input_token_program.to_account_info(),
        input_transfer_amount,
        ctx.accounts.input_token_mint.decimals,
    )?;

    // 9.代币转账执行
    transfer_from_pool_vault_to_user(
        ctx.accounts.authority.to_account_info(),
        ctx.accounts.output_vault.to_account_info(),
        ctx.accounts.output_token_account.to_account_info(),
        ctx.accounts.output_token_mint.to_account_info(),
        ctx.accounts.output_token_program.to_account_info(),
        output_transfer_amount,
        ctx.accounts.output_token_mint.decimals,
        &[&[crate::AUTH_SEED.as_bytes(), &[pool_state.auth_bump]]],
    )?;

    // 10.价格预言机更新（更新上一个价格到观察数据）
    ctx.accounts.observation_state.load_mut()?.update(
        oracle::block_timestamp(),
        token_0_price_x64,
        token_1_price_x64,
    );
    pool_state.recent_epoch = Clock::get()?.epoch;

    Ok(())
}
