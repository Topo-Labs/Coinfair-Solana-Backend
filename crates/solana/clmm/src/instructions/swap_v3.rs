use std::collections::VecDeque;
use std::ops::Deref;

use crate::error::ErrorCode;
use crate::instructions::swap_internal_new;
use crate::libraries::tick_math;
use crate::util::*;
use crate::{states::*, util};
use anchor_lang::{prelude::*, solana_program};
use anchor_spl::memo::spl_memo;
use anchor_spl::token::Token;
use anchor_spl::token_interface::{Mint, Token2022, TokenAccount};
use coinfair_referral::{program::Referral, ReferralAccount};

/// Memo msg for swap
// pub const SWAP_MEMO_MSG: &'statics [u8] = b"coinfair_swap";
#[derive(Accounts)]
pub struct SwapSingleV3<'info> {
    /// The user performing the swap
    //#[account(mut, signer)]
    pub payer: Signer<'info>,

    // 指定收取手续费的代币Mint（upper和upper_upper对应分佣账户也对应该代币）
    pub input_mint: Box<InterfaceAccount<'info, Mint>>,

    /// The user PDA of referral_account（用于获取payer的upper)
    #[account(
        seeds = [b"referral", payer.key().as_ref()],
        bump,
        seeds::program = coinfair_referral::id()
    )]
    pub payer_referral: Option<Account<'info, ReferralAccount>>,

    /// CHECK: 仅用于与 payer_referral.upper 对比，不读取数据
    #[account(
        constraint = payer_referral.as_ref().unwrap().upper.is_none() || upper.key() == payer_referral.as_ref().unwrap().upper.unwrap()
        @ ErrorCode::UpperAccountMismatch
    )]
    pub upper: Option<UncheckedAccount<'info>>,

    /// upper接收分佣的 ATA（用于收手续费奖励）(该账户 owner 应为 `upper`，mint 应为 swap 所涉及的 token)
    #[account(
        mut,
        constraint = payer_referral.as_ref().unwrap().upper.is_none() || (
            upper_token_account.owner == upper.as_ref().unwrap().key() &&
            upper_token_account.mint == input_mint.key() //Token_Mint
        )
        @ ErrorCode::UpperTokenAccountMismatch
    )]
    pub upper_token_account: Option<InterfaceAccount<'info, TokenAccount>>,
    // #[account(
    //     init_if_needed,
    //     payer = payer,
    //     associated_token::mint = input_mint,
    //     associated_token::authority = upper,
    //     constraint = payer_referral.upper.is_none() || true // Allow initialization when upper exists
    // )]
    // pub upper_token_account: Option<InterfaceAccount<'info, TokenAccount>>,
    /// The user's upper PDA of referral_account(用于获取upper的upper)
    #[account(
        seeds = [b"referral", upper.as_ref().unwrap().key().as_ref()],
        bump,
        seeds::program = coinfair_referral::id(),
        constraint = payer_referral.as_ref().unwrap().upper.is_some()
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
            upper_upper_token_account.mint == input_mint.key()
        )
        @ ErrorCode::UpperUpperTokenAccountMismatch
    )]
    pub upper_upper_token_account: Option<InterfaceAccount<'info, TokenAccount>>,

    /// 项目方
    #[account(
        mut,
        constraint = project_token_account.owner == pool_state.load()?.owner @ ErrorCode::ProjectTokenAccountMismatch
    )]
    pub project_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    /// The factory state to read protocol fees
    #[account(address = pool_state.load()?.amm_config)]
    pub amm_config: Box<Account<'info, AmmConfig>>,

    /// The program account of the pool in which the swap will be performed
    #[account(mut)]
    pub pool_state: AccountLoader<'info, PoolState>,

    /// The user token account for input token
    #[account(mut)]
    pub input_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    /// The user token account for output token
    #[account(mut)]
    pub output_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    /// The vault token account for input token
    #[account(mut)]
    pub input_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    /// The vault token account for output token
    #[account(mut)]
    pub output_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    /// The program account for the most recent oracle observation
    #[account(mut, address = pool_state.load()?.observation_key)]
    pub observation_state: AccountLoader<'info, ObservationState>,

    /// SPL program for token transfers
    pub token_program: Program<'info, Token>,

    /// SPL program 2022 for token transfers
    pub token_program_2022: Program<'info, Token2022>,

    /// CHECK:
    #[account(
        address = spl_memo::id()
    )]
    pub memo_program: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,

    pub associated_token_program: Program<'info, anchor_spl::associated_token::AssociatedToken>,

    #[account(address = coinfair_referral::id())]
    pub referral: Program<'info, Referral>,

    /// The mint of token vault 0
    #[account(
        address = input_vault.mint
    )]
    pub input_vault_mint: Box<InterfaceAccount<'info, Mint>>,

    /// The mint of token vault 1
    #[account(
        address = output_vault.mint
    )]
    pub output_vault_mint: Box<InterfaceAccount<'info, Mint>>,
    // remaining accounts
    // tickarray_bitmap_extension: must add account if need regardless the sequence
    // tick_array_account_1
    // tick_array_account_2
    // tick_array_account_...
}

/// Performs a single exact input/output swap
/// if is_base_input = true, return vaule is the max_amount_out, otherwise is min_amount_in
pub fn exact_internal_v3<'c: 'info, 'info>(
    ctx: &mut SwapSingleV3<'info>,
    remaining_accounts: &'c [AccountInfo<'info>],
    amount_specified: u64,
    sqrt_price_limit_x64: u128,
    is_base_input: bool,
) -> Result<u64> {
    // invoke_memo_instruction(SWAP_MEMO_MSG, ctx.memo_program.to_account_info())?;

    // 获取当前区块时间戳(确保池子已经到开启时间点)
    let block_timestamp = solana_program::clock::Clock::get()?.unix_timestamp as u64;

    let amount_0;
    let amount_1;
    let zero_for_one;
    let swap_price_before;

    // 用户(交易前)的输入代币的余额
    let input_balance_before = ctx.input_token_account.amount;
    // 用户(交易前)的输出代币的余额
    let output_balance_before = ctx.output_token_account.amount;

    // 计算实际交换金额（考虑到转账手续费时-Token2022）
    let amount_calculate_specified = if is_base_input {
        // 当用户的输入货币是"基础货币"(即希望卖出时)，计算实际用于交换的（意味着卖不了那么多）
        let transfer_fee = util::get_transfer_fee(ctx.input_vault_mint.clone(), amount_specified).unwrap();
        amount_specified - transfer_fee
    } else {
        // 当用户的输入货币是"报价货币"(即希望买入时)，计算实际需要输入的金额（意味着需要支付更多）
        let transfer_fee = util::get_transfer_inverse_fee(ctx.output_vault_mint.clone(), amount_specified).unwrap();
        amount_specified + transfer_fee
    };
    #[allow(unused_assignments)]
    let mut total_reward_fee = 0;

    // 进入一个新的作用域(限制变量周期)
    {
        // 获取池子当前价格
        swap_price_before = ctx.pool_state.load()?.sqrt_price_x64;
        // 加载池子的(可变)状态
        let pool_state = &mut ctx.pool_state.load_mut()?;
        // 判断兑换方向
        zero_for_one = ctx.input_vault.mint == pool_state.token_mint_0;

        // 确保当前时间大于池的开放时间
        require_gt!(block_timestamp, pool_state.open_time);

        // 确保财库的输入&输出地址与池状态匹配
        // input_vault是由客户端的用户传入; token_vault是池初始化时配置
        require!(
            if zero_for_one {
                ctx.input_vault.key() == pool_state.token_vault_0 && ctx.output_vault.key() == pool_state.token_vault_1
            } else {
                ctx.input_vault.key() == pool_state.token_vault_1 && ctx.output_vault.key() == pool_state.token_vault_0
            },
            ErrorCode::InvalidInputPoolVault
        );

        // 初始化 tick array 扩展和状态队列
        let mut tickarray_bitmap_extension = None;
        let tick_array_states = &mut VecDeque::new();

        // 遍历剩余账户：
        // - 如果是 tickarray_bitmap_extension，加载扩展数据。
        // - 否则，将其作为 tick array 状态加入队列。
        let tick_array_bitmap_extension_key = TickArrayBitmapExtension::key(pool_state.key());
        for account_info in remaining_accounts.into_iter() {
            if account_info.key().eq(&tick_array_bitmap_extension_key) {
                tickarray_bitmap_extension = Some(
                    *(AccountLoader::<TickArrayBitmapExtension>::try_from(account_info)?
                        .load()?
                        .deref()),
                );
                continue;
            }
            tick_array_states.push_back(AccountLoad::load_data_mut(account_info)?);
        }

        // 调用内部交换函数 swap_internal，计算实际交换的 amount_0 和 amount_1
        // - 如果未指定价格限制，则使用默认最小/最大价格
        // - 更新池状态和预言机数据
        (amount_0, amount_1, total_reward_fee) = swap_internal_new(
            &ctx.amm_config,
            pool_state,
            tick_array_states,
            &mut ctx.observation_state.load_mut()?,
            &tickarray_bitmap_extension,
            amount_calculate_specified,
            if sqrt_price_limit_x64 == 0 {
                if zero_for_one {
                    tick_math::MIN_SQRT_PRICE_X64 + 1
                } else {
                    tick_math::MAX_SQRT_PRICE_X64 - 1
                }
            } else {
                sqrt_price_limit_x64
            },
            zero_for_one,
            is_base_input,
            oracle::block_timestamp(),
            // 新增分佣参数
            ctx.upper_token_account.as_ref().map(|a| a.as_ref()),
            // ctx.upper_upper_token_account.as_ref().map(|a| a.as_ref()),
            // ctx.upper_token_account.as_ref().map(|a| a.as_ref()),
            // ctx.upper_upper_token_account.as_ref().map(|a| a.as_ref()),
            // &ctx.token_program,
            // &ctx.token_program_2022,
        )?;

        #[cfg(feature = "enable-log")]
        msg!(
            "exact_swap_internal, is_base_input:{}, amount_0: {}, amount_1: {}",
            is_base_input,
            amount_0,
            amount_1
        );
        require!(amount_0 != 0 && amount_1 != 0, ErrorCode::TooSmallInputOrOutputAmount);
    }

    let (token_account_0, token_account_1, vault_0, vault_1, vault_0_mint, vault_1_mint) = if zero_for_one {
        (
            ctx.input_token_account.clone(),
            ctx.output_token_account.clone(),
            ctx.input_vault.clone(),
            ctx.output_vault.clone(),
            ctx.input_vault_mint.clone(),
            ctx.output_vault_mint.clone(),
        )
    } else {
        (
            ctx.output_token_account.clone(),
            ctx.input_token_account.clone(),
            ctx.output_vault.clone(),
            ctx.input_vault.clone(),
            ctx.output_vault_mint.clone(),
            ctx.input_vault_mint.clone(),
        )
    };

    // user or pool real amount delta without tranfer fee
    let amount_0_without_fee;
    let amount_1_without_fee;
    // the transfer fee amount charged by withheld_amount
    let transfer_fee_0;
    let transfer_fee_1;
    // transfer amount
    let transfer_amount_0;
    let transfer_amount_1;

    // 根据兑换方向，进行代币转移逻辑（池子和用户）
    if zero_for_one {
        // 从用户账户转入amount_0（净输入量）时额外付出的费用
        transfer_fee_0 = util::get_transfer_inverse_fee(vault_0_mint.clone(), amount_0).unwrap();
        // 从池子财库转出amount_1时扣除的费用
        transfer_fee_1 = util::get_transfer_fee(vault_1_mint.clone(), amount_1).unwrap();

        amount_0_without_fee = amount_0;
        amount_1_without_fee = amount_1.checked_sub(transfer_fee_1).unwrap();

        // 用户转给池子的总量，池子转出给用户的总量
        (transfer_amount_0, transfer_amount_1) = (amount_0 + transfer_fee_0, amount_1);

        #[cfg(feature = "enable-log")]
        msg!(
            "amount_0:{}, transfer_fee_0:{}, amount_1:{}, transfer_fee_1:{}",
            amount_0,
            transfer_fee_0,
            amount_1,
            transfer_fee_1
        );

        //  x -> y, deposit x token from user to pool vault.
        //  TODO: stevekeol
        transfer_from_user_to_pool_vault(
            &ctx.payer,
            &token_account_0.to_account_info(),
            &vault_0.to_account_info(),
            Some(vault_0_mint.clone()),
            &ctx.token_program,
            Some(ctx.token_program_2022.to_account_info()),
            transfer_amount_0,
        )?;

        // 如果池子的财库不够，则直接冻结池状态（并禁用所有指令）
        if vault_1.amount <= transfer_amount_1 {
            // freeze pool, disable all instructions
            ctx.pool_state.load_mut()?.set_status(255);
        }

        // x -> y，transfer y token from pool vault to user.
        // 即：Buy时，
        transfer_from_pool_vault_to_user(
            &ctx.pool_state,
            &vault_1.to_account_info(),
            &token_account_1.to_account_info(),
            Some(vault_1_mint.clone()),
            &ctx.token_program,
            Some(ctx.token_program_2022.to_account_info()),
            transfer_amount_1,
        )?;

        // 实时给上级&上上级，项目方分佣
        transfer_from_pool_vault_to_uppers_and_project(
            &ctx.pool_state,
            &vault_0.to_account_info(),
            &ctx.project_token_account.to_account_info(),
            ctx.upper_token_account.clone(),
            ctx.upper_upper_token_account.clone(),
            Some(vault_0_mint.clone()),
            &ctx.token_program,
            Some(ctx.token_program_2022.to_account_info()),
            total_reward_fee,
            //事件触发所需字段
            vault_0_mint.key(),
            ctx.payer.key(),                           // from: 交易发起者
            ctx.pool_state.load()?.owner,              // project: 项目方地址（从pool_state获取）
            ctx.upper.as_ref().map(|u| u.key()),       // upper: 上级地址（可选）
            ctx.upper_upper.as_ref().map(|u| u.key()), // upper_upper: 上上级地址（可选）
        )?;
    } else {
        transfer_fee_0 = util::get_transfer_fee(vault_0_mint.clone(), amount_0).unwrap();
        transfer_fee_1 = util::get_transfer_inverse_fee(vault_1_mint.clone(), amount_1).unwrap();

        amount_0_without_fee = amount_0.checked_sub(transfer_fee_0).unwrap();
        amount_1_without_fee = amount_1;
        (transfer_amount_0, transfer_amount_1) = (amount_0, amount_1 + transfer_fee_1);
        #[cfg(feature = "enable-log")]
        msg!(
            "amount_0:{}, transfer_fee_0:{}, amount_1:{}, transfer_fee_1:{}",
            amount_0,
            transfer_fee_0,
            amount_1,
            transfer_fee_1
        );
        transfer_from_user_to_pool_vault(
            &ctx.payer,
            &token_account_1.to_account_info(),
            &vault_1.to_account_info(),
            Some(vault_1_mint.clone()),
            &ctx.token_program,
            Some(ctx.token_program_2022.to_account_info()),
            transfer_amount_1,
        )?;
        if vault_0.amount <= transfer_amount_0 {
            // freeze pool, disable all instructions
            ctx.pool_state.load_mut()?.set_status(255);
        }
        transfer_from_pool_vault_to_user(
            &ctx.pool_state,
            &vault_0.to_account_info(),
            &token_account_0.to_account_info(),
            Some(vault_0_mint.clone()),
            &ctx.token_program,
            Some(ctx.token_program_2022.to_account_info()),
            transfer_amount_0,
        )?;

        // 实时给上级&上上级，项目方分佣
        transfer_from_pool_vault_to_uppers_and_project(
            &ctx.pool_state,
            &vault_1.to_account_info(),
            &ctx.project_token_account.to_account_info(),
            ctx.upper_token_account.clone(),
            ctx.upper_upper_token_account.clone(),
            Some(vault_1_mint.clone()),
            &ctx.token_program,
            Some(ctx.token_program_2022.to_account_info()),
            total_reward_fee,
            //事件触发所需字段
            vault_1_mint.key(),
            ctx.payer.key(),                           // from: 交易发起者
            ctx.pool_state.load()?.owner,              // project: 项目方地址（从pool_state获取）
            ctx.upper.as_ref().map(|u| u.key()),       // upper: 上级地址（可选）
            ctx.upper_upper.as_ref().map(|u| u.key()), // upper_upper: 上上级地址（可选）
        )?;
    }

    // 代币转移操作会修改链上的账户数据，但这些更改不会自动反应到其内存副本中
    ctx.output_token_account.reload()?; // swap兑换
    ctx.input_token_account.reload()?; // swap兑换
                                       // ctx.upper_token_account.reload()?; // 实时分佣
                                       // ctx.upper_upper_token_account.reload()?; // 实时分佣
    ctx.project_token_account.reload()?; // 实时分佣

    // 如果 upper_token_account 存在，则调用 reload()
    if let Some(upper_token_account) = ctx.upper_token_account.as_mut() {
        upper_token_account.reload()?;
    }

    // 如果 upper_upper_token_account 存在，则调用 reload()
    if let Some(upper_upper_token_account) = ctx.upper_upper_token_account.as_mut() {
        upper_upper_token_account.reload()?;
    }

    let pool_state = ctx.pool_state.load()?;

    emit!(SwapEvent {
        pool_state: pool_state.key(),
        sender: ctx.payer.key(),
        token_account_0: token_account_0.key(),
        token_account_1: token_account_1.key(),
        amount_0: amount_0_without_fee,
        transfer_fee_0,
        amount_1: amount_1_without_fee,
        transfer_fee_1,
        zero_for_one,
        sqrt_price_x64: pool_state.sqrt_price_x64,
        liquidity: pool_state.liquidity,
        tick: pool_state.tick_current
    });

    if zero_for_one {
        require_gt!(swap_price_before, pool_state.sqrt_price_x64);
    } else {
        require_gt!(pool_state.sqrt_price_x64, swap_price_before);
    }
    if sqrt_price_limit_x64 == 0 {
        // Does't allow partial filled without specified limit_price.
        if is_base_input {
            if zero_for_one {
                require_eq!(amount_specified, transfer_amount_0);
            } else {
                require_eq!(amount_specified, transfer_amount_1);
            }
        } else {
            if zero_for_one {
                require_eq!(amount_specified, transfer_amount_1);
            } else {
                require_eq!(amount_specified, transfer_amount_0);
            }
        }
    }

    // 返回用户在执行Swap操作之后，对某个Token账户的影响(ETH, USDT)
    // - true时: USDT增加(返回增加的量）
    // - false时: ETH减少(返回减少的量)
    if is_base_input {
        Ok(ctx
            .output_token_account
            .amount
            .checked_sub(output_balance_before)
            .unwrap())
    } else {
        Ok(input_balance_before
            .checked_sub(ctx.input_token_account.amount)
            .unwrap())
    }
}

pub fn swap_v3<'a, 'b, 'c: 'info, 'info>(
    ctx: Context<'a, 'b, 'c, 'info, SwapSingleV3<'info>>,
    amount: u64,
    other_amount_threshold: u64,
    sqrt_price_limit_x64: u128,
    is_base_input: bool,
) -> Result<()> {
    let amount_result = exact_internal_v3(
        ctx.accounts,
        ctx.remaining_accounts,
        amount,
        sqrt_price_limit_x64,
        is_base_input,
    )?;
    if is_base_input {
        require_gte!(
            amount_result,
            other_amount_threshold,
            ErrorCode::TooLittleOutputReceived
        );
    } else {
        require_gte!(other_amount_threshold, amount_result, ErrorCode::TooMuchInputPaid);
    }

    Ok(())
}

// 实时分佣给swap payer的上级和上上级
pub fn transfer_from_pool_vault_to_uppers_and_project<'info>(
    pool_state_loader: &AccountLoader<'info, PoolState>,
    from_vault: &AccountInfo<'info>,
    project_token_account: &AccountInfo<'info>,
    upper_token_account: Option<InterfaceAccount<'info, TokenAccount>>,
    upper_upper_token_account: Option<InterfaceAccount<'info, TokenAccount>>,
    mint: Option<Box<InterfaceAccount<'info, Mint>>>,
    token_program: &AccountInfo<'info>,
    token_program_2022: Option<AccountInfo<'info>>,
    total_reward_fee: u64,
    // 事件触发所需字段
    reward_mint: Pubkey,
    from: Pubkey,
    project: Pubkey,
    upper: Option<Pubkey>,
    upper_upper: Option<Pubkey>,
) -> Result<()> {
    if total_reward_fee == 0 {
        return Ok(());
    }

    let project_reward_fee = total_reward_fee / 2;
    let uppers_total_reward_fee = total_reward_fee - project_reward_fee;

    // 给项目方分佣（30%）
    transfer_from_pool_vault_to_user(
        pool_state_loader,
        &from_vault.to_account_info(),
        &project_token_account.to_account_info(),
        mint.clone(),
        token_program,
        token_program_2022.clone(),
        project_reward_fee,
    )?;

    emit!(ReferralRewardEvent {
        from,
        to: project,
        mint: reward_mint,
        amount: project_reward_fee,
        timestamp: Clock::get()?.unix_timestamp,
    });

    if let (Some(upper_token_account), Some(upper_upper_token_account)) =
        (upper_token_account.clone(), upper_upper_token_account)
    {
        let upper_reward_fee = uppers_total_reward_fee * 5 / 6;
        let upper_upper_reward_fee = uppers_total_reward_fee - upper_reward_fee;

        // 给上级分佣（25%）
        transfer_from_pool_vault_to_user(
            pool_state_loader,
            &from_vault.to_account_info(),
            &upper_token_account.to_account_info(),
            mint.clone(),
            token_program,
            token_program_2022.clone(),
            upper_reward_fee,
        )?;
        if let Some(upper_pubkey) = upper {
            emit!(ReferralRewardEvent {
                from,
                to: upper_pubkey,
                mint: reward_mint,
                amount: upper_reward_fee,
                timestamp: Clock::get()?.unix_timestamp,
            });
        }

        // 给上上级分佣（5%）
        transfer_from_pool_vault_to_user(
            pool_state_loader,
            &from_vault.to_account_info(),
            &upper_upper_token_account.to_account_info(),
            mint.clone(),
            token_program,
            token_program_2022.clone(),
            upper_upper_reward_fee,
        )?;
        if let Some(upper_upper_pubkey) = upper_upper {
            emit!(ReferralRewardEvent {
                from,
                to: upper_upper_pubkey,
                mint: reward_mint,
                amount: upper_upper_reward_fee,
                timestamp: Clock::get()?.unix_timestamp,
            });
        }
    } else if let Some(upper_token_account) = upper_token_account {
        // 全给上级分佣（30%）
        transfer_from_pool_vault_to_user(
            pool_state_loader,
            &from_vault.to_account_info(),
            &upper_token_account.to_account_info(),
            mint,
            token_program,
            token_program_2022,
            uppers_total_reward_fee,
        )?;
        if let Some(upper_pubkey) = upper {
            emit!(ReferralRewardEvent {
                from,
                to: upper_pubkey,
                mint: reward_mint,
                amount: uppers_total_reward_fee,
                timestamp: Clock::get()?.unix_timestamp,
            });
        }
    }

    return Ok(());
}

#[event]
pub struct ReferralRewardEvent {
    pub from: Pubkey,   // Payer
    pub to: Pubkey,     // Upper or Lower
    pub mint: Pubkey,   // 奖励的代币
    pub amount: u64,    // 奖励数量
    pub timestamp: i64, // 时间戳
}
