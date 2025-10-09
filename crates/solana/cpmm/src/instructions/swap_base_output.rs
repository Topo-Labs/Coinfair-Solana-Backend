use super::swap_base_input::Swap;
use crate::curve::calculator::CurveCalculator;
use crate::curve::constant_product::pow_4th_normalized;
use crate::curve::TradeDirection;
use crate::error::ErrorCode;
use crate::libraries::U512;
use crate::states::*;
use crate::utils::token::*;
use anchor_lang::prelude::*;
use anchor_lang::solana_program;

// pub fn swap_base_output(
//     ctx: Context<Swap>,
//     max_amount_in: u64,
//     amount_out_received: u64,
// ) -> Result<()> {
//     require_gt!(amount_out_received, 0);
//     let block_timestamp = solana_program::clock::Clock::get()?.unix_timestamp as u64;
//     let pool_id = ctx.accounts.pool_state.key();
//     let pool_state = &mut ctx.accounts.pool_state.load_mut()?;
//     if !pool_state.get_status_by_bit(PoolStatusBitIndex::Swap)
//         || block_timestamp < pool_state.open_time
//     {
//         return err!(ErrorCode::NotApproved);
//     }
//     let out_transfer_fee = get_transfer_inverse_fee(
//         &ctx.accounts.output_token_mint.to_account_info(),
//         amount_out_received,
//     )?;
//     let amount_out_with_transfer_fee = amount_out_received.checked_add(out_transfer_fee).unwrap();

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
//     // let constant_before = u128::from(total_input_token_amount)
//     //     .checked_mul(u128::from(total_output_token_amount))
//     //     .unwrap();

//     // 🔧 修改1：使用4次方计算常量
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

//     let creator_fee_rate =
//         pool_state.adjust_creator_fee_rate(ctx.accounts.amm_config.creator_fee_rate);
//     let result = CurveCalculator::swap_base_output(
//         trade_direction,
//         u128::from(amount_out_with_transfer_fee),
//         u128::from(total_input_token_amount),
//         u128::from(total_output_token_amount),
//         ctx.accounts.amm_config.trade_fee_rate,
//         creator_fee_rate,
//         ctx.accounts.amm_config.protocol_fee_rate,
//         ctx.accounts.amm_config.fund_fee_rate,
//         is_creator_fee_on_input,
//     )
//     .ok_or(ErrorCode::ZeroTradingTokens)?;

//     // let constant_after = u128::from(result.new_input_vault_amount)
//     //     .checked_mul(u128::from(result.new_output_vault_amount))
//     //     .unwrap();

//     // 🔧 修改2：使用4次方计算交换后的常量
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
//         "input_amount:{}, output_amount:{}, trade_fee:{}, output_transfer_fee:{}, constant_before:{}, constant_after:{}, is_creator_fee_on_input:{}, creator_fee:{}",
//         result.input_amount,
//         result.output_amount,
//         result.trade_fee,
//         out_transfer_fee,
//         constant_before,
//         constant_after,
//         is_creator_fee_on_input,
//         result.creator_fee,
//     );

//     // 根据曲线结果重新计算源交换金额
//     let (input_transfer_amount, input_transfer_fee) = {
//         let input_amount = u64::try_from(result.input_amount).unwrap();
//         require_gt!(input_amount, 0);
//         let transfer_fee = get_transfer_inverse_fee(
//             &ctx.accounts.input_token_mint.to_account_info(),
//             input_amount,
//         )?;
//         let input_transfer_amount = input_amount.checked_add(transfer_fee).unwrap();
//         require_gte!(
//             max_amount_in,
//             input_transfer_amount,
//             ErrorCode::ExceededSlippage
//         );
//         (input_transfer_amount, transfer_fee)
//     };
//     require_eq!(
//         u64::try_from(result.output_amount).unwrap(),
//         amount_out_with_transfer_fee
//     );
//     let (output_transfer_amount, output_transfer_fee) =
//         (amount_out_with_transfer_fee, out_transfer_fee);

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
//         base_input: false,
//         input_mint: ctx.accounts.input_token_mint.key(),
//         output_mint: ctx.accounts.output_token_mint.key(),
//         trade_fee: u64::try_from(result.trade_fee).unwrap(),
//         creator_fee: u64::try_from(result.creator_fee).unwrap(),
//         creator_fee_on_input: is_creator_fee_on_input,
//     });
//     require_gte!(constant_after, constant_before);

//     let total_reward_fee = 0;

//     // 🔧 修改3：创建临时变量，避免重复可变引用
//     let input_account = ctx.accounts.input_token_account.clone();
//     let output_account = ctx.accounts.output_token_account.clone();
//     let input_vault = ctx.accounts.input_vault.clone();
//     let output_vault = ctx.accounts.output_vault.clone();
//     let input_mint = ctx.accounts.input_token_mint.clone();
//     let output_mint = ctx.accounts.output_token_mint.clone();
//     let input_program = ctx.accounts.input_token_program.clone();
//     let output_program = ctx.accounts.output_token_program.clone();

//     // 🔧 修改4：先从 vault 分佣给 project/uppers
//     transfer_from_pool_vault_to_uppers_and_project(
//         &ctx.accounts.pool_state,
//         &output_vault.to_account_info(),
//         &ctx.accounts.project_token_account.to_account_info(),
//         ctx.accounts
//             .upper_token_account
//             .as_ref()
//             .map(|acc| acc.to_account_info()),
//         ctx.accounts
//             .upper_upper_token_account
//             .as_ref()
//             .map(|acc| acc.to_account_info()),
//         ctx.accounts.reward_mint.to_account_info(),
//         output_mint.decimals,
//         output_program.to_account_info(),
//         total_reward_fee,
//         &[&[crate::AUTH_SEED.as_bytes(), &[pool_state.auth_bump]]],
//         //事件触发所需字段
//         ctx.accounts.reward_mint.key(),
//         ctx.accounts.payer.key(),
//         ctx.accounts.pool_state.load()?.pool_creator,
//         ctx.accounts.upper.as_ref().map(|u| u.key()),
//         ctx.accounts.upper_upper.as_ref().map(|u| u.key()),
//     )?;

//     // 用户转入到 vault
//     transfer_from_user_to_pool_vault(
//         ctx.accounts.payer.to_account_info(),
//         input_account.to_account_info(),
//         input_vault.to_account_info(),
//         input_mint.to_account_info(),
//         input_program.to_account_info(),
//         input_transfer_amount,
//         input_mint.decimals,
//     )?;

//     // vault 转出给用户
//     transfer_from_pool_vault_to_user(
//         ctx.accounts.authority.to_account_info(),
//         output_vault.to_account_info(),
//         output_account.to_account_info(),
//         output_mint.to_account_info(),
//         output_program.to_account_info(),
//         output_transfer_amount,
//         output_mint.decimals,
//         &[&[crate::AUTH_SEED.as_bytes(), &[pool_state.auth_bump]]],
//     )?;

//     // transfer_from_user_to_pool_vault(
//     //     ctx.accounts.payer.to_account_info(),
//     //     ctx.accounts.input_token_account.to_account_info(),
//     //     ctx.accounts.input_vault.to_account_info(),
//     //     ctx.accounts.input_token_mint.to_account_info(),
//     //     ctx.accounts.input_token_program.to_account_info(),
//     //     input_transfer_amount,
//     //     ctx.accounts.input_token_mint.decimals,
//     // )?;

//     // transfer_from_pool_vault_to_user(
//     //     ctx.accounts.authority.to_account_info(),
//     //     ctx.accounts.output_vault.to_account_info(),
//     //     ctx.accounts.output_token_account.to_account_info(),
//     //     ctx.accounts.output_token_mint.to_account_info(),
//     //     ctx.accounts.output_token_program.to_account_info(),
//     //     output_transfer_amount,
//     //     ctx.accounts.output_token_mint.decimals,
//     //     &[&[crate::AUTH_SEED.as_bytes(), &[pool_state.auth_bump]]],
//     // )?;

//     // 更新上一个价格到观察数据
//     ctx.accounts.observation_state.load_mut()?.update(
//         oracle::block_timestamp(),
//         token_0_price_x64,
//         token_1_price_x64,
//     );
//     pool_state.recent_epoch = Clock::get()?.epoch;

//     Ok(())
// }

pub fn swap_base_output(ctx: Context<Swap>, max_amount_in: u64, amount_out_received: u64) -> Result<()> {
    require_gt!(amount_out_received, 0);

    let (pool_creator, auth_bump, token_0_price_x64, token_1_price_x64, input_transfer_amount, output_transfer_amount);

    let pool_owner_and_upper_fee;

    // 将所有使用 pool_state 的代码放在作用域内
    {
        let block_timestamp = solana_program::clock::Clock::get()?.unix_timestamp as u64;
        let pool_id = ctx.accounts.pool_state.key();
        let pool_state = &mut ctx.accounts.pool_state.load_mut()?;

        if !pool_state.get_status_by_bit(PoolStatusBitIndex::Swap) || block_timestamp < pool_state.open_time {
            return err!(ErrorCode::NotApproved);
        }

        let out_transfer_fee =
            get_transfer_inverse_fee(&ctx.accounts.output_token_mint.to_account_info(), amount_out_received)?;
        let amount_out_with_transfer_fee = amount_out_received.checked_add(out_transfer_fee).unwrap();

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

        let x4_before = pow_4th_normalized(u128::from(x_vault_before));
        let constant_before = x4_before.checked_mul(U512::from(y_vault_before)).unwrap();

        let creator_fee_rate = pool_state.adjust_creator_fee_rate(ctx.accounts.amm_config.creator_fee_rate);

        let has_upper = ctx.accounts.upper.is_some();
        let result = CurveCalculator::swap_base_output(
            trade_direction,
            u128::from(amount_out_with_transfer_fee),
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

        let x_vault_after = match trade_direction {
            TradeDirection::ZeroForOne => result.new_input_vault_amount,
            TradeDirection::OneForZero => result.new_output_vault_amount,
        };
        let y_vault_after = match trade_direction {
            TradeDirection::ZeroForOne => result.new_output_vault_amount,
            TradeDirection::OneForZero => result.new_input_vault_amount,
        };

        let x4_after = pow_4th_normalized(x_vault_after);
        let constant_after = x4_after.checked_mul(U512::from(y_vault_after)).unwrap();

        #[cfg(feature = "enable-log")]
        msg!(
            "input_amount:{}, output_amount:{}, trade_fee:{}, output_transfer_fee:{}, constant_before:{}, constant_after:{}, is_creator_fee_on_input:{}, creator_fee:{}",
            result.input_amount,
            result.output_amount,
            result.trade_fee,
            out_transfer_fee,
            constant_before,
            constant_after,
            is_creator_fee_on_input,
            result.creator_fee,
        );

        // 计算转账金额
        let (input_transfer_amount_local, input_transfer_fee) = {
            let input_amount = u64::try_from(result.input_amount).unwrap();
            require_gt!(input_amount, 0);
            let transfer_fee =
                get_transfer_inverse_fee(&ctx.accounts.input_token_mint.to_account_info(), input_amount)?;
            let input_transfer_amount = input_amount.checked_add(transfer_fee).unwrap();
            require_gte!(max_amount_in, input_transfer_amount, ErrorCode::ExceededSlippage);
            (input_transfer_amount, transfer_fee)
        };

        require_eq!(
            u64::try_from(result.output_amount).unwrap(),
            amount_out_with_transfer_fee
        );

        let (output_transfer_amount_local, output_transfer_fee) = (amount_out_with_transfer_fee, out_transfer_fee);

        // ✅ 赋值给外部变量
        input_transfer_amount = input_transfer_amount_local;
        output_transfer_amount = output_transfer_amount_local;

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
            base_input: false,
            input_mint: ctx.accounts.input_token_mint.key(),
            output_mint: ctx.accounts.output_token_mint.key(),
            trade_fee: u64::try_from(result.trade_fee).unwrap(),
            creator_fee: u64::try_from(result.creator_fee).unwrap(),
            creator_fee_on_input: is_creator_fee_on_input,
        });
        require_gte!(constant_after, constant_before);

        // ✅ 提取需要的数据
        pool_creator = pool_state.pool_creator;
        auth_bump = pool_state.auth_bump;
        pool_state.recent_epoch = Clock::get()?.epoch;
    } // pool_state 在这里释放

    // ✅ 提取其他数据
    let reward_mint_key = ctx.accounts.reward_mint.key();
    let payer_key = ctx.accounts.payer.key();
    let upper_key = ctx.accounts.upper.as_ref().map(|u| u.key());
    let upper_upper_key = ctx.accounts.upper_upper.as_ref().map(|u| u.key());

    // ✅ 提前提取 decimals
    let input_decimals = ctx.accounts.input_token_mint.decimals;
    let output_decimals = ctx.accounts.output_token_mint.decimals;

    // ✅ 使用引用而不是 clone
    let input_account = &ctx.accounts.input_token_account;
    let output_account = &ctx.accounts.output_token_account;
    let input_vault = &ctx.accounts.input_vault;
    let output_vault = &ctx.accounts.output_vault;
    let input_mint = &ctx.accounts.input_token_mint;
    let output_mint = &ctx.accounts.output_token_mint;
    let input_program = &ctx.accounts.input_token_program;
    let output_program = &ctx.accounts.output_token_program;

    // 从 vault 分佣给 project/uppers
    transfer_from_pool_vault_to_uppers_and_project(
        &ctx.accounts.pool_state,
        &ctx.accounts.authority.to_account_info(),
        &input_vault.to_account_info(),
        &ctx.accounts.project_token_account.to_account_info(),
        ctx.accounts
            .upper_token_account
            .as_ref()
            .map(|acc| acc.to_account_info()),
        ctx.accounts
            .upper_upper_token_account
            .as_ref()
            .map(|acc| acc.to_account_info()),
        ctx.accounts.reward_mint.to_account_info(),
        input_decimals,
        input_program.to_account_info(),
        pool_owner_and_upper_fee as u64,
        &[&[crate::AUTH_SEED.as_bytes(), &[auth_bump]]],
        reward_mint_key,
        payer_key,
        pool_creator,
        upper_key,
        upper_upper_key,
    )?;

    // 用户转入到 vault
    transfer_from_user_to_pool_vault(
        ctx.accounts.payer.to_account_info(),
        input_account.to_account_info(),
        input_vault.to_account_info(),
        input_mint.to_account_info(),
        input_program.to_account_info(),
        input_transfer_amount,
        input_decimals,
    )?;

    // vault 转出给用户
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

    // 更新观察状态
    ctx.accounts
        .observation_state
        .load_mut()?
        .update(oracle::block_timestamp(), token_0_price_x64, token_1_price_x64);

    Ok(())
}
