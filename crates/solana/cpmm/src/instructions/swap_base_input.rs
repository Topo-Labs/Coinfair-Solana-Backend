use crate::libraries::U512;
use crate::curve::calculator::CurveCalculator;
use crate::curve::TradeDirection;
use crate::curve::constant_product::pow_4th_normalized;
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

// pub fn swap_base_input(ctx: Context<Swap>, amount_in: u64, minimum_amount_out: u64) -> Result<()> {
//     msg!("=== Account Debug Start ===");
//     msg!("input_vault: {}", ctx.accounts.input_vault.key());
//     msg!("output_vault: {}", ctx.accounts.output_vault.key());
//     msg!("input_token_account: {}", ctx.accounts.input_token_account.key());
//     msg!("output_token_account: {}", ctx.accounts.output_token_account.key());
//     msg!("project_token_account: {}", ctx.accounts.project_token_account.key());

//     // 前置检查与验证（是否允许交换操作；是否已到开放时间）
//     let block_timestamp = solana_program::clock::Clock::get()?.unix_timestamp as u64;
//     let pool_id = ctx.accounts.pool_state.key();
//     let pool_state = &mut ctx.accounts.pool_state.load_mut()?;
//     if !pool_state.get_status_by_bit(PoolStatusBitIndex::Swap)
//         || block_timestamp < pool_state.open_time
//     {
//         return err!(ErrorCode::NotApproved);
//     }

//     // 2.转账费用处理（某些SPL代币有转账费用，需计算扣除之后的实际转入金额）
//     let transfer_fee =
//         get_transfer_fee(&ctx.accounts.input_token_mint.to_account_info(), amount_in)?;
//     let actual_amount_in = amount_in.saturating_sub(transfer_fee);
//     require_gt!(actual_amount_in, 0);

//     // 3.获取交换参数
//         // 确定交易方向（token0 → token1 或 token1 → token0）
//         // 获取当前池中两种代币的数量
//         // 获取当前价格信息
//         // 确定创建者费用的收取方式
//     let SwapParams {
//         trade_direction,
//         total_input_token_amount,
//         total_output_token_amount,
//         token_0_price_x64,
//         token_1_price_x64,
//         is_creator_fee_on_input,
//     } = pool_state.get_swap_params(
//         ctx.accounts.input_vault.key(),
//         ctx.accounts.output_vault.key(),
//         ctx.accounts.input_vault.amount,
//         ctx.accounts.output_vault.amount,
//     )?;

//     let x_vault_before = match trade_direction {
//         TradeDirection::ZeroForOne => total_input_token_amount,
//         TradeDirection::OneForZero => total_output_token_amount,
//     };
//     let y_vault_before = match trade_direction {
//         TradeDirection::ZeroForOne => total_output_token_amount,
//         TradeDirection::OneForZero => total_input_token_amount,
//     };

//     let x4_before = pow_4th_normalized(u128::from(x_vault_before));
//     let constant_before = x4_before.checked_mul(U512::from(y_vault_before)).unwrap();

//     // 4.恒定乘积验证准备（计算交换前的乘积，以后续验证）
//     // let constant_before = u128::from(total_input_token_amount)
//     //     .checked_mul(u128::from(total_output_token_amount))
//     //     .unwrap();

//     // 5.费用计算与交换计算
//         // 计算输出金额：amount_out = y - (x × y) / (x + amount_in)
//         // 扣除各种费用：
//            // 交易费用 (trade_fee)
//            // 创建者费用 (creator_fee)
//            // 协议费用 (protocol_fee)
//            // 基金费用 (fund_fee)
//     let creator_fee_rate =
//         pool_state.adjust_creator_fee_rate(ctx.accounts.amm_config.creator_fee_rate);

//     let result = CurveCalculator::swap_base_input(
//         trade_direction,
//         u128::from(actual_amount_in),
//         u128::from(total_input_token_amount),
//         u128::from(total_output_token_amount),
//         ctx.accounts.amm_config.trade_fee_rate,    // 交易费率
//         creator_fee_rate,                          // 创建者费用
//         ctx.accounts.amm_config.protocol_fee_rate, // 协议费率
//         ctx.accounts.amm_config.fund_fee_rate,     // 基金费率
//         is_creator_fee_on_input,                   // 创建者费用收取方式
//     )
//     .ok_or(ErrorCode::ZeroTradingTokens)?;

//     println!("result: {:?}", result);

//     // 6.验证交换之后的常量乘积
//     // let constant_after = u128::from(result.new_input_vault_amount)
//     //     .checked_mul(u128::from(result.new_output_vault_amount))
//     //     .unwrap();

//     let x_vault_after = match trade_direction {
//         TradeDirection::ZeroForOne => result.new_input_vault_amount,
//         TradeDirection::OneForZero => result.new_output_vault_amount,
//     };
//     let y_vault_after = match trade_direction {
//         TradeDirection::ZeroForOne => result.new_output_vault_amount,
//         TradeDirection::OneForZero => result.new_input_vault_amount,
//     };

//     let x4_after = pow_4th_normalized(x_vault_after);
//     let constant_after = x4_after.checked_mul(U512::from(y_vault_after)).unwrap();

//     #[cfg(feature = "enable-log")]
//     msg!(
//         "input_amount:{}, output_amount:{}, trade_fee:{}, input_transfer_fee:{}, constant_before:{},constant_after:{}, is_creator_fee_on_input:{}, creator_fee:{}",
//         result.input_amount,
//         result.output_amount,
//         result.trade_fee,
//         transfer_fee,
//         constant_before,
//         constant_after,
//         is_creator_fee_on_input,
//         result.creator_fee,
//     );
//     require_eq!(
//         u64::try_from(result.input_amount).unwrap(),
//         actual_amount_in
//     );

//     // 7.滑点保护（计算用户在扣除输出代币的转账费用之后，实际收到的代币数量，是否大于用户设置的最小输出金额）
//     let (input_transfer_amount, input_transfer_fee) = (amount_in, transfer_fee);
//     let (output_transfer_amount, output_transfer_fee) = {
//         let amount_out = u64::try_from(result.output_amount).unwrap();
//         let transfer_fee = get_transfer_fee(
//             &ctx.accounts.output_token_mint.to_account_info(),
//             amount_out,
//         )?;
//         let amount_received = amount_out.checked_sub(transfer_fee).unwrap();
//         require_gt!(amount_received, 0);
//         require_gte!(
//             amount_received,
//             minimum_amount_out,
//             ErrorCode::ExceededSlippage
//         );
//         (amount_out, transfer_fee)
//     };

//     // 8.池子根据交易方向来更新费用
//     pool_state.update_fees(
//         u64::try_from(result.protocol_fee).unwrap(),
//         u64::try_from(result.fund_fee).unwrap(),
//         u64::try_from(result.creator_fee).unwrap(),
//         trade_direction,
//     )?;

//     emit!(SwapEvent {
//         pool_id,
//         input_vault_before: total_input_token_amount,
//         output_vault_before: total_output_token_amount,
//         input_amount: u64::try_from(result.input_amount).unwrap(),
//         output_amount: u64::try_from(result.output_amount).unwrap(),
//         input_transfer_fee,
//         output_transfer_fee,
//         base_input: true,
//         input_mint: ctx.accounts.input_token_mint.key(),
//         output_mint: ctx.accounts.output_token_mint.key(),
//         trade_fee: u64::try_from(result.trade_fee).unwrap(),
//         creator_fee: u64::try_from(result.creator_fee).unwrap(),
//         creator_fee_on_input: is_creator_fee_on_input,
//     });
//     require_gte!(constant_after, constant_before);

//     let total_reward_fee = 0;

//  // ✅ 修改点 1：提前提取数据，并立即在作用域内释放借用
//     let (pool_creator, auth_bump, reward_mint_key, payer_key, upper_key, upper_upper_key) = {
//         let ps = ctx.accounts.pool_state.load()?;
//         (
//             ps.pool_creator,
//             ps.auth_bump,
//             ctx.accounts.reward_mint.key(),
//             ctx.accounts.payer.key(),
//             ctx.accounts.upper.as_ref().map(|u| u.key()),
//             ctx.accounts.upper_upper.as_ref().map(|u| u.key()),
//         )
//     }; // ← ps 在这里自动释放

//     // ✅ 修改点 2：提前提取 decimals，避免在转账函数中访问
//     let input_decimals = ctx.accounts.input_token_mint.decimals;
//     let output_decimals = ctx.accounts.output_token_mint.decimals;

//     // ✅ 修改点 3：直接使用引用，不要 clone（除非绝对必要）
//     let input_account = &ctx.accounts.input_token_account;
//     let output_account = &ctx.accounts.output_token_account;
//     let input_vault = &ctx.accounts.input_vault;
//     let output_vault = &ctx.accounts.output_vault;
//     let input_mint = &ctx.accounts.input_token_mint;
//     let output_mint = &ctx.accounts.output_token_mint;
//     let input_program = &ctx.accounts.input_token_program;
//     let output_program = &ctx.accounts.output_token_program;

//     // 9.代币转账执行 - 第一步：从 vault 分佣给 project/uppers
//     transfer_from_pool_vault_to_uppers_and_project(
//         &ctx.accounts.pool_state,
//         &output_vault.to_account_info(),
//         &ctx.accounts.project_token_account.to_account_info(),
//         ctx.accounts.upper_token_account.as_ref().map(|acc| acc.to_account_info()),
//         ctx.accounts.upper_upper_token_account.as_ref().map(|acc| acc.to_account_info()),
//         ctx.accounts.reward_mint.to_account_info(),
//         output_decimals,  // ✅ 修改点 4：使用提前提取的值
//         output_program.to_account_info(),
//         total_reward_fee,
//         &[&[crate::AUTH_SEED.as_bytes(), &[auth_bump]]],
//         // 事件触发所需字段
//         reward_mint_key,
//         payer_key,
//         pool_creator,
//         upper_key,
//         upper_upper_key,
//     )?;

//     // 9.代币转账执行 - 第二步：用户转入到 vault
//     transfer_from_user_to_pool_vault(
//         ctx.accounts.payer.to_account_info(),
//         input_account.to_account_info(),
//         input_vault.to_account_info(),
//         input_mint.to_account_info(),
//         input_program.to_account_info(),
//         input_transfer_amount,
//         input_decimals,  // ✅ 修改点 5：使用提前提取的值
//     )?;

//     // 9.代币转账执行 - 第三步：vault 转出给用户
//     transfer_from_pool_vault_to_user(
//         ctx.accounts.authority.to_account_info(),
//         output_vault.to_account_info(),
//         output_account.to_account_info(),
//         output_mint.to_account_info(),
//         output_program.to_account_info(),
//         output_transfer_amount,
//         output_decimals,  // ✅ 修改点 6：使用提前提取的值
//         &[&[crate::AUTH_SEED.as_bytes(), &[auth_bump]]],
//     )?;

//     // 10.价格预言机更新
//     ctx.accounts.observation_state.load_mut()?.update(
//         oracle::block_timestamp(),
//         token_0_price_x64,
//         token_1_price_x64,
//     );

//     // // 🔧 提前提取所有需要的数据，避免在函数调用中重复借用
//     // let pool_creator = ctx.accounts.pool_state.load()?.pool_creator;
//     // let reward_mint_key = ctx.accounts.reward_mint.key();
//     // let payer_key = ctx.accounts.payer.key();
//     // let upper_key = ctx.accounts.upper.as_ref().map(|u| u.key());
//     // let upper_upper_key = ctx.accounts.upper_upper.as_ref().map(|u| u.key());
//     // let auth_bump = pool_state.auth_bump;

//     // // 创建临时变量引用，避免双重借用
//     // let (input_account, output_account) = (
//     //     ctx.accounts.input_token_account.clone(),
//     //     ctx.accounts.output_token_account.clone(),
//     // );

//     // let input_vault = ctx.accounts.input_vault.clone();
//     // let output_vault = ctx.accounts.output_vault.clone();
//     // let input_mint = ctx.accounts.input_token_mint.clone();
//     // let output_mint = ctx.accounts.output_token_mint.clone();
//     // let input_program = ctx.accounts.input_token_program.clone();
//     // let output_program = ctx.accounts.output_token_program.clone();

//     // // 9.代币转账执行 - 第一步：从 vault 分佣给 project/uppers
//     // transfer_from_pool_vault_to_uppers_and_project(
//     //     &ctx.accounts.pool_state,
//     //     &output_vault.to_account_info(),
//     //     &ctx.accounts.project_token_account.to_account_info(),
//     //     ctx.accounts.upper_token_account.as_ref().map(|acc| acc.to_account_info()),
//     //     ctx.accounts.upper_upper_token_account.as_ref().map(|acc| acc.to_account_info()),
//     //     ctx.accounts.reward_mint.to_account_info(),
//     //     output_mint.decimals,
//     //     output_program.to_account_info(),
//     //     total_reward_fee,
//     //     &[&[crate::AUTH_SEED.as_bytes(), &[auth_bump]]],
//     //     // 事件触发所需字段
//     //     reward_mint_key,
//     //     payer_key,
//     //     pool_creator,
//     //     upper_key,
//     //     upper_upper_key,
//     // )?;

//     // // 9.代币转账执行 - 第二步：用户转入到 vault
//     // transfer_from_user_to_pool_vault(
//     //     ctx.accounts.payer.to_account_info(),
//     //     input_account.to_account_info(),
//     //     input_vault.to_account_info(),
//     //     input_mint.to_account_info(),
//     //     input_program.to_account_info(),
//     //     input_transfer_amount,
//     //     input_mint.decimals,
//     // )?;

//     // // 9.代币转账执行 - 第三步：vault 转出给用户
//     // transfer_from_pool_vault_to_user(
//     //     ctx.accounts.authority.to_account_info(),
//     //     output_vault.to_account_info(),
//     //     output_account.to_account_info(),
//     //     output_mint.to_account_info(),
//     //     output_program.to_account_info(),
//     //     output_transfer_amount,
//     //     output_mint.decimals,
//     //     &[&[crate::AUTH_SEED.as_bytes(), &[pool_state.auth_bump]]],
//     // )?;

//     // // 10.价格预言机更新（更新上一个价格到观察数据）
//     // ctx.accounts.observation_state.load_mut()?.update(
//     //     oracle::block_timestamp(),
//     //     token_0_price_x64,
//     //     token_1_price_x64,
//     // );
//     pool_state.recent_epoch = Clock::get()?.epoch;

//     Ok(())
// }

pub fn swap_base_input(ctx: Context<Swap>, amount_in: u64, minimum_amount_out: u64) -> Result<()> {
    msg!("=== Step 1: Basic accounts ===");
    msg!("input_vault: {}", ctx.accounts.input_vault.key());
    msg!("output_vault: {}", ctx.accounts.output_vault.key());
    msg!("input_token_account: {}", ctx.accounts.input_token_account.key());
    msg!("output_token_account: {}", ctx.accounts.output_token_account.key());
    msg!("project_token_account: {}", ctx.accounts.project_token_account.key());
    
    // ✅ 提前提取需要在后面使用的数据
    let (pool_creator, auth_bump, token_0_price_x64, token_1_price_x64, input_transfer_amount, output_transfer_amount);

    let pool_owner_and_upper_fee ;
    
    // 将所有使用 pool_state 的代码放在一个作用域内
    {
        msg!("=== Step 2: Loading pool_state (load_mut) ===");
        let block_timestamp = solana_program::clock::Clock::get()?.unix_timestamp as u64;
        let pool_id = ctx.accounts.pool_state.key();
        let pool_state = &mut ctx.accounts.pool_state.load_mut()?;
        msg!("pool_state loaded successfully");
        
        if !pool_state.get_status_by_bit(PoolStatusBitIndex::Swap)
            || block_timestamp < pool_state.open_time
        {
            return err!(ErrorCode::NotApproved);
        }

        msg!("=== Step 3: Transfer fee calculation ===");
        let transfer_fee =
            get_transfer_fee(&ctx.accounts.input_token_mint.to_account_info(), amount_in)?;
        let actual_amount_in = amount_in.saturating_sub(transfer_fee);
        require_gt!(actual_amount_in, 0);
        msg!("transfer_fee: {}, actual_amount_in: {}", transfer_fee, actual_amount_in);

        msg!("=== Step 4: Get swap params ===");
        let SwapParams {
            trade_direction,
            total_input_token_amount,
            total_output_token_amount,
            token_0_price_x64: t0_price,
            token_1_price_x64: t1_price,
            is_creator_fee_on_input,
        } = pool_state.get_swap_params(
            ctx.accounts.input_vault.key(),
            ctx.accounts.output_vault.key(),
            ctx.accounts.input_vault.amount,
            ctx.accounts.output_vault.amount,
        )?;
        msg!("Swap params calculated");
        
        // 保存价格供后续使用
        token_0_price_x64 = t0_price;
        token_1_price_x64 = t1_price;

        let x_vault_before = match trade_direction {
            TradeDirection::ZeroForOne => total_input_token_amount,
            TradeDirection::OneForZero => total_output_token_amount,
        };
        let y_vault_before = match trade_direction {
            TradeDirection::ZeroForOne => total_output_token_amount,
            TradeDirection::OneForZero => total_input_token_amount,
        };
        msg!("x_vault_before: {}, y_vault_before: {}", x_vault_before, y_vault_before);

        let x4_before = pow_4th_normalized(u128::from(x_vault_before));
        let constant_before = x4_before.checked_mul(U512::from(y_vault_before)).unwrap();
        msg!("x4_before: {:?}, constant_before: {:?}", x4_before, constant_before);

        msg!("=== Step 5: Calculate swap result ===");
        let creator_fee_rate =
            pool_state.adjust_creator_fee_rate(ctx.accounts.amm_config.creator_fee_rate);

        let has_upper = ctx.accounts.upper.is_some();

        let result = CurveCalculator::swap_base_input(
            trade_direction,
            u128::from(actual_amount_in),
            u128::from(total_input_token_amount),
            u128::from(total_output_token_amount),
            ctx.accounts.amm_config.trade_fee_rate,
            creator_fee_rate,
            ctx.accounts.amm_config.protocol_fee_rate,
            ctx.accounts.amm_config.fund_fee_rate,
            is_creator_fee_on_input,
            has_upper,
        )
        .ok_or(ErrorCode::ZeroTradingTokens)?;

        pool_owner_and_upper_fee = result.pool_owner_and_upper_fee;
        msg!("Swap calculation complete");

        let x_vault_after = match trade_direction {
            TradeDirection::ZeroForOne => result.new_input_vault_amount,
            TradeDirection::OneForZero => result.new_output_vault_amount,
        };
        let y_vault_after = match trade_direction {
            TradeDirection::ZeroForOne => result.new_output_vault_amount,
            TradeDirection::OneForZero => result.new_input_vault_amount,
        };
        msg!("x_vault_after: {}, y_vault_after: {}", x_vault_after, y_vault_after);

        let x4_after = pow_4th_normalized(x_vault_after);
        let constant_after = x4_after.checked_mul(U512::from(y_vault_after)).unwrap();
        msg!("x4_after: {:?}, constant_after: {:?}", x4_after, constant_after);

        require_eq!(
            u64::try_from(result.input_amount).unwrap(),
            actual_amount_in
        );

        msg!("=== Step 6: Slippage protection ===");
        let (input_transfer_amount_local, input_transfer_fee) = (amount_in, transfer_fee);
        let (output_transfer_amount_local, output_transfer_fee) = {
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
 
        msg!("Slippage check passed");

        // ✅ 赋值给外部变量
        input_transfer_amount = input_transfer_amount_local;
        output_transfer_amount = output_transfer_amount_local;

        msg!("=== Step 7: Update fees ===");
        pool_state.update_fees(
            u64::try_from(result.protocol_fee).unwrap(),
            u64::try_from(result.fund_fee).unwrap(),
            u64::try_from(result.creator_fee).unwrap(),
            trade_direction,
        )?;
        msg!("Fees updated");

        msg!("=== Step 8: Emit event ===");
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
        msg!("Event emitted");
        
        // ✅ 提取后续需要的数据
        pool_creator = pool_state.pool_creator;
        auth_bump = pool_state.auth_bump;
        
        // 更新 recent_epoch
        pool_state.recent_epoch = Clock::get()?.epoch;
        
    } // ← pool_state 在这里被 drop，释放借用

    msg!("=== Step 9: Extract additional data ===");
    let reward_mint_key = ctx.accounts.reward_mint.key();
    let payer_key = ctx.accounts.payer.key();
    let upper_key = ctx.accounts.upper.as_ref().map(|u| u.key());
    let upper_upper_key = ctx.accounts.upper_upper.as_ref().map(|u| u.key());
    msg!("Data extraction complete");

    msg!("=== Step 10: Extract decimals ===");
    let input_decimals = ctx.accounts.input_token_mint.decimals;
    let output_decimals = ctx.accounts.output_token_mint.decimals;
    msg!("Decimals extracted");

    msg!("=== Step 11: Create references ===");
    let input_account = &ctx.accounts.input_token_account;
    let output_account = &ctx.accounts.output_token_account;
    let input_vault = &ctx.accounts.input_vault;
    let output_vault = &ctx.accounts.output_vault;
    let input_mint = &ctx.accounts.input_token_mint;
    let output_mint = &ctx.accounts.output_token_mint;
    let input_program = &ctx.accounts.input_token_program;
    let output_program = &ctx.accounts.output_token_program;
    msg!("References created");

    msg!("=== Step 12: Transfer to uppers and project ===");
    transfer_from_pool_vault_to_uppers_and_project(
        &ctx.accounts.pool_state,
        &ctx.accounts.authority.to_account_info(),
        &input_vault.to_account_info(),
        &ctx.accounts.project_token_account.to_account_info(),
        ctx.accounts.upper_token_account.as_ref().map(|acc| acc.to_account_info()),
        ctx.accounts.upper_upper_token_account.as_ref().map(|acc| acc.to_account_info()),
        ctx.accounts.reward_mint.to_account_info(),
        output_decimals,
        output_program.to_account_info(),
        pool_owner_and_upper_fee as u64,
        &[&[crate::AUTH_SEED.as_bytes(), &[auth_bump]]],
        reward_mint_key,
        payer_key,
        pool_creator,
        upper_key,
        upper_upper_key,
    )?;
    msg!("Transfer to uppers/project complete");

    msg!("=== Step 13: Transfer from user to vault ===");
    transfer_from_user_to_pool_vault(
        ctx.accounts.payer.to_account_info(),
        input_account.to_account_info(),
        input_vault.to_account_info(),
        input_mint.to_account_info(),
        input_program.to_account_info(),
        input_transfer_amount,
        input_decimals,
    )?;
    msg!("Transfer from user complete");

    msg!("=== Step 14: Transfer from vault to user ===");
    transfer_from_pool_vault_to_user(
        ctx.accounts.authority.to_account_info(),
        output_vault.to_account_info(),
        output_account.to_account_info(),
        output_mint.to_account_info(),
        output_program.to_account_info(),
        output_transfer_amount,
        output_decimals,
        &[&[crate::AUTH_SEED.as_bytes(), &[auth_bump]]],
    )?;
    msg!("Transfer to user complete");

    msg!("=== Step 15: Update observation ===");
    ctx.accounts.observation_state.load_mut()?.update(
        oracle::block_timestamp(),
        token_0_price_x64,
        token_1_price_x64,
    );
    msg!("Observation updated");

    msg!("=== Swap complete ===");
    Ok(())
}


