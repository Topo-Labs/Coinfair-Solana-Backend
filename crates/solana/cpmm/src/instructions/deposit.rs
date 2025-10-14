use crate::curve::CurveCalculator;
use crate::curve::RoundDirection;
use crate::error::ErrorCode;
use crate::states::*;
use crate::utils::token::*;
use anchor_lang::prelude::*;
use anchor_spl::token::Token;
use anchor_spl::token_interface::{Mint, Token2022, TokenAccount};

#[derive(Accounts)]
pub struct Deposit<'info> {
    /// 支付铸造仓位
    pub owner: Signer<'info>,

    /// CHECK: 池子金库和LP铸币权限
    #[account(
        seeds = [
            crate::AUTH_SEED.as_bytes(),
        ],
        bump,
    )]
    pub authority: UncheckedAccount<'info>,

    #[account(mut)]
    pub pool_state: AccountLoader<'info, PoolState>,

    /// 所有者LP代币账户
    #[account(mut,  token::authority = owner)]
    pub owner_lp_token: Box<InterfaceAccount<'info, TokenAccount>>,

    /// 付款人token_0代币账户
    #[account(
        mut,
        token::mint = token_0_vault.mint,
        token::authority = owner
    )]
    pub token_0_account: Box<InterfaceAccount<'info, TokenAccount>>,

    /// 付款人token_1代币账户
    #[account(
        mut,
        token::mint = token_1_vault.mint,
        token::authority = owner
    )]
    pub token_1_account: Box<InterfaceAccount<'info, TokenAccount>>,

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

    /// 代币程序
    pub token_program: Program<'info, Token>,

    /// 代币程序2022
    pub token_program_2022: Program<'info, Token2022>,

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

    /// LP代币铸币
    #[account(
        mut,
        address = pool_state.load()?.lp_mint @ ErrorCode::IncorrectLpMint)
    ]
    pub lp_mint: Box<InterfaceAccount<'info, Mint>>,

    /// TransferHook相关账户(可选)
    /// CHECK: 可选的 Transfer Hook 程序账户（可执行程序ID）
    pub transfer_hook_program: Option<UncheckedAccount<'info>>,
    /// CHECK: 可选的 ExtraAccountMetaList 账户（由发行方程序创建）
    pub extra_account_metas: Option<UncheckedAccount<'info>>,
    /// CHECK: 可选的发行方配置账户（按 EAML 解析需要）
    pub project_config: Option<UncheckedAccount<'info>>,
    /// CHECK: 发射平台Program
    pub fairlaunch_program: Option<UncheckedAccount<'info>>,
    /// 带TransferHook的Token_2022(Coinfair_FairGo)
    pub token_2022_hook_mint: Option<Box<InterfaceAccount<'info, Mint>>>,
    /// CHECK: 转账方用户存款账户
    pub source_user_deposit: Option<UncheckedAccount<'info>>,
    /// CHECK: 接收方用户存款账户
    pub destination_user_deposit: Option<UncheckedAccount<'info>>,
}

pub fn deposit(
    ctx: Context<Deposit>,
    lp_token_amount: u64,
    maximum_token_0_amount: u64,
    maximum_token_1_amount: u64,
) -> Result<()> {
    require_gt!(lp_token_amount, 0);
    let pool_id = ctx.accounts.pool_state.key();
    let pool_state = &mut ctx.accounts.pool_state.load_mut()?;
    if !pool_state.get_status_by_bit(PoolStatusBitIndex::Deposit) {
        return err!(ErrorCode::NotApproved);
    }
    let (total_token_0_amount, total_token_1_amount) =
        pool_state.vault_amount_without_fee(ctx.accounts.token_0_vault.amount, ctx.accounts.token_1_vault.amount)?;
    let results = CurveCalculator::lp_tokens_to_trading_tokens(
        u128::from(lp_token_amount),
        u128::from(pool_state.lp_supply),
        u128::from(total_token_0_amount),
        u128::from(total_token_1_amount),
        RoundDirection::Ceiling,
    )
    .ok_or(ErrorCode::ZeroTradingTokens)?;
    if results.token_0_amount == 0 || results.token_1_amount == 0 {
        return err!(ErrorCode::ZeroTradingTokens);
    }
    let token_0_amount = u64::try_from(results.token_0_amount).unwrap();
    let (transfer_token_0_amount, transfer_token_0_fee) = {
        let transfer_fee = get_transfer_inverse_fee(&ctx.accounts.vault_0_mint.to_account_info(), token_0_amount)?;
        (token_0_amount.checked_add(transfer_fee).unwrap(), transfer_fee)
    };

    let token_1_amount = u64::try_from(results.token_1_amount).unwrap();
    let (transfer_token_1_amount, transfer_token_1_fee) = {
        let transfer_fee = get_transfer_inverse_fee(&ctx.accounts.vault_1_mint.to_account_info(), token_1_amount)?;
        (token_1_amount.checked_add(transfer_fee).unwrap(), transfer_fee)
    };

    #[cfg(feature = "enable-log")]
    msg!(
        "results.token_0_amount;{}, results.token_1_amount:{},transfer_token_0_amount:{},transfer_token_0_fee:{},
            transfer_token_1_amount:{},transfer_token_1_fee:{}",
        results.token_0_amount,
        results.token_1_amount,
        transfer_token_0_amount,
        transfer_token_0_fee,
        transfer_token_1_amount,
        transfer_token_1_fee
    );

    emit!(LpChangeEvent {
        user_wallet: ctx.accounts.owner.key(),
        pool_id,
        lp_mint: ctx.accounts.lp_mint.key(),
        token_0_mint: ctx.accounts.vault_0_mint.key(),
        token_1_mint: ctx.accounts.vault_1_mint.key(),
        lp_amount_before: pool_state.lp_supply,
        token_0_vault_before: total_token_0_amount,
        token_1_vault_before: total_token_1_amount,
        token_0_amount,
        token_1_amount,
        token_0_transfer_fee: transfer_token_0_fee,
        token_1_transfer_fee: transfer_token_1_fee,
        change_type: 0,
        lp_mint_program_id: ctx.accounts.lp_mint.to_account_info().owner.key(),
        token_0_program_id: ctx.accounts.vault_0_mint.to_account_info().owner.key(),
        token_1_program_id: ctx.accounts.vault_1_mint.to_account_info().owner.key(),
        lp_mint_decimals: ctx.accounts.lp_mint.decimals,
        token_0_decimals: ctx.accounts.vault_0_mint.decimals,
        token_1_decimals: ctx.accounts.vault_1_mint.decimals,
    });

    if transfer_token_0_amount > maximum_token_0_amount || transfer_token_1_amount > maximum_token_1_amount {
        return Err(ErrorCode::ExceededSlippage.into());
    }

    match (
        &ctx.accounts.transfer_hook_program,
        &ctx.accounts.extra_account_metas,
        &ctx.accounts.fairlaunch_program,
        &ctx.accounts.project_config,
    ) {
        // 所有 Hook 相关账户都存在
        (Some(hook_program), Some(extra_metas), Some(fairlaunch), Some(config)) => {
            let _auth_bump = pool_state.auth_bump;
            // let signer_seeds = &[&[crate::AUTH_SEED.as_bytes(), &[auth_bump]]];

            let is_token_0_hook = ctx
                .accounts
                .token_2022_hook_mint
                .as_ref()
                .map(|mint| mint.key() == ctx.accounts.token_0_vault.mint)
                .unwrap_or(false);

            let _is_token_1_hook = ctx
                .accounts
                .token_2022_hook_mint
                .as_ref()
                .map(|mint| mint.key() == ctx.accounts.token_0_vault.mint)
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
                // Token 0 转账（带 Hook）
                transfer_from_user_to_pool_vault_with_hook(
                    ctx.accounts.owner.to_account_info(),
                    ctx.accounts.token_0_account.to_account_info(),
                    ctx.accounts.token_0_vault.to_account_info(),
                    ctx.accounts.vault_0_mint.to_account_info(),
                    if ctx.accounts.vault_0_mint.to_account_info().owner == ctx.accounts.token_program.key {
                        ctx.accounts.token_program.to_account_info()
                    } else {
                        ctx.accounts.token_program_2022.to_account_info()
                    },
                    transfer_token_0_amount,
                    ctx.accounts.vault_0_mint.decimals,
                    extra_metas.to_account_info(),
                    fairlaunch.to_account_info(),
                    config.to_account_info(),
                    source_deposit,
                    destination_deposit,
                    hook_program.to_account_info(),
                )?;

                // Token 1 转账（不带 Hook）
                transfer_from_user_to_pool_vault(
                    ctx.accounts.owner.to_account_info(),
                    ctx.accounts.token_1_account.to_account_info(),
                    ctx.accounts.token_1_vault.to_account_info(),
                    ctx.accounts.vault_1_mint.to_account_info(),
                    if ctx.accounts.vault_1_mint.to_account_info().owner == ctx.accounts.token_program.key {
                        ctx.accounts.token_program.to_account_info()
                    } else {
                        ctx.accounts.token_program_2022.to_account_info()
                    },
                    transfer_token_1_amount,
                    ctx.accounts.vault_1_mint.decimals,
                )?;
            } else {
                // Token 0 转账（不带 Hook）
                transfer_from_user_to_pool_vault(
                    ctx.accounts.owner.to_account_info(),
                    ctx.accounts.token_0_account.to_account_info(),
                    ctx.accounts.token_0_vault.to_account_info(),
                    ctx.accounts.vault_0_mint.to_account_info(),
                    if ctx.accounts.vault_0_mint.to_account_info().owner == ctx.accounts.token_program.key {
                        ctx.accounts.token_program.to_account_info()
                    } else {
                        ctx.accounts.token_program_2022.to_account_info()
                    },
                    transfer_token_0_amount,
                    ctx.accounts.vault_0_mint.decimals,
                )?;

                // Token 1 转账（带 Hook）
                transfer_from_user_to_pool_vault_with_hook(
                    ctx.accounts.owner.to_account_info(),
                    ctx.accounts.token_1_account.to_account_info(),
                    ctx.accounts.token_1_vault.to_account_info(),
                    ctx.accounts.vault_1_mint.to_account_info(),
                    if ctx.accounts.vault_1_mint.to_account_info().owner == ctx.accounts.token_program.key {
                        ctx.accounts.token_program.to_account_info()
                    } else {
                        ctx.accounts.token_program_2022.to_account_info()
                    },
                    transfer_token_1_amount,
                    ctx.accounts.vault_1_mint.decimals,
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
            transfer_from_user_to_pool_vault(
                ctx.accounts.owner.to_account_info(),
                ctx.accounts.token_0_account.to_account_info(),
                ctx.accounts.token_0_vault.to_account_info(),
                ctx.accounts.vault_0_mint.to_account_info(),
                if ctx.accounts.vault_0_mint.to_account_info().owner == ctx.accounts.token_program.key {
                    ctx.accounts.token_program.to_account_info()
                } else {
                    ctx.accounts.token_program_2022.to_account_info()
                },
                transfer_token_0_amount,
                ctx.accounts.vault_0_mint.decimals,
            )?;

            transfer_from_user_to_pool_vault(
                ctx.accounts.owner.to_account_info(),
                ctx.accounts.token_1_account.to_account_info(),
                ctx.accounts.token_1_vault.to_account_info(),
                ctx.accounts.vault_1_mint.to_account_info(),
                if ctx.accounts.vault_1_mint.to_account_info().owner == ctx.accounts.token_program.key {
                    ctx.accounts.token_program.to_account_info()
                } else {
                    ctx.accounts.token_program_2022.to_account_info()
                },
                transfer_token_1_amount,
                ctx.accounts.vault_1_mint.decimals,
            )?;
            // transfer_from_user_to_pool_vault(
            //     ctx.accounts.creator.to_account_info(),
            //     ctx.accounts.creator_token_0.to_account_info(),
            //     ctx.accounts.token_0_vault.to_account_info(),
            //     ctx.accounts.token_0_mint.to_account_info(),
            //     ctx.accounts.token_0_program.to_account_info(),
            //     init_amount_0,
            //     ctx.accounts.token_0_mint.decimals,
            // )?;

            // transfer_from_user_to_pool_vault(
            //     ctx.accounts.creator.to_account_info(),
            //     ctx.accounts.creator_token_1.to_account_info(),
            //     ctx.accounts.token_1_vault.to_account_info(),
            //     ctx.accounts.token_1_mint.to_account_info(),
            //     ctx.accounts.token_1_program.to_account_info(),
            //     init_amount_1,
            //     ctx.accounts.token_1_mint.decimals,
            // )?;
        }

        // 账户不完整，返回错误
        _ => {
            return err!(ErrorCode::IncompleteTransferHookAccounts);
        }
    }

    pool_state.lp_supply = pool_state.lp_supply.checked_add(lp_token_amount).unwrap();

    token_mint_to(
        ctx.accounts.authority.to_account_info(),
        ctx.accounts.token_program.to_account_info(),
        ctx.accounts.lp_mint.to_account_info(),
        ctx.accounts.owner_lp_token.to_account_info(),
        lp_token_amount,
        &[&[crate::AUTH_SEED.as_bytes(), &[pool_state.auth_bump]]],
    )?;
    pool_state.recent_epoch = Clock::get()?.epoch;

    Ok(())
}
