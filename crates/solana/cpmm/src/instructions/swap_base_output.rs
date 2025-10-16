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

        // #[cfg(feature = "enable-log")]
        // msg!(
        //     "input_amount:{}, output_amount:{}, trade_fee:{}, output_transfer_fee:{}, constant_before:{}, constant_after:{}, is_creator_fee_on_input:{}, creator_fee:{}",
        //     result.input_amount,
        //     result.output_amount,
        //     result.trade_fee,
        //     out_transfer_fee,
        //     constant_before,
        //     constant_after,
        //     is_creator_fee_on_input,
        //     result.creator_fee,
        // );

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

    let pool_state = &mut ctx.accounts.pool_state.load_mut()?;

    match (
        &ctx.accounts.transfer_hook_program,
        &ctx.accounts.extra_account_metas,
        &ctx.accounts.fairlaunch_program,
        &ctx.accounts.project_config,
    ) {
        // 所有 Hook 相关账户都存在
        (Some(hook_program), Some(extra_metas), Some(fairlaunch), Some(config)) => {
            let auth_bump = pool_state.auth_bump;
            // let signer_seeds = &[&[crate::AUTH_SEED.as_bytes(), &[auth_bump]]];

            let is_token_0_hook = ctx
                .accounts
                .token_2022_hook_mint
                .as_ref()
                .map(|mint| mint.key() == ctx.accounts.input_vault.mint)
                .unwrap_or(false);

            let _is_token_1_hook = ctx
                .accounts
                .token_2022_hook_mint
                .as_ref()
                .map(|mint| mint.key() == ctx.accounts.output_vault.mint)
                .unwrap_or(false);

            let source_deposit = ctx
                .accounts
                .source_user_deposit
                .as_ref()
                .map(|acc| acc.to_account_info())
                .unwrap_or_else(|| fairlaunch.to_account_info());

            let destination_deposit = ctx
                .accounts
                .destination_user_deposit
                .as_ref()
                .map(|acc| acc.to_account_info())
                .unwrap_or_else(|| fairlaunch.to_account_info());

            // 1.当上述账户存在时，说明有其一为TranferHook Mint
            // 2.Mint_0和Mint_1只能有一个TransferHook Mint
            if is_token_0_hook {
                // 从 vault 分佣给 project/uppers
                transfer_from_pool_vault_to_uppers_and_project_with_hook(
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
                    extra_metas.to_account_info(),
                    fairlaunch.to_account_info(),
                    config.to_account_info(),
                    source_deposit.clone(),
                    destination_deposit.clone(),
                    hook_program.to_account_info(),
                )?;

                // 用户转入到 vault
                transfer_from_user_to_pool_vault_with_hook(
                    ctx.accounts.payer.to_account_info(),
                    input_account.to_account_info(),
                    input_vault.to_account_info(),
                    input_mint.to_account_info(),
                    input_program.to_account_info(),
                    input_transfer_amount,
                    input_decimals,
                    extra_metas.to_account_info(),
                    fairlaunch.to_account_info(),
                    config.to_account_info(),
                    source_deposit,
                    destination_deposit,
                    hook_program.to_account_info(),
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
            } else {
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
                transfer_from_pool_vault_to_user_with_hook(
                    ctx.accounts.authority.to_account_info(),
                    output_vault.to_account_info(),
                    output_account.to_account_info(),
                    output_mint.to_account_info(),
                    output_program.to_account_info(),
                    output_transfer_amount,
                    output_decimals,
                    &[&[crate::AUTH_SEED.as_bytes(), &[auth_bump]]],
                    extra_metas.to_account_info(),
                    fairlaunch.to_account_info(),
                    config.to_account_info(),
                    source_deposit,
                    destination_deposit,
                    hook_program.to_account_info(),
                )?;
            }
        }

        // 没有 Hook，使用标准转账
        (_, None, _, None) => {
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
        }

        // 账户不完整，返回错误
        _ => {
            return err!(ErrorCode::IncompleteTransferHookAccounts);
        }
    }

    // 更新观察状态
    ctx.accounts
        .observation_state
        .load_mut()?
        .update(oracle::block_timestamp(), token_0_price_x64, token_1_price_x64);

    Ok(())
}
