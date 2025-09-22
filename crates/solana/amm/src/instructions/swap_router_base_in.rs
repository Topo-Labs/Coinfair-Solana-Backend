use crate::error::ErrorCode;
use crate::states::*;
use crate::swap_v3::{exact_internal_v3, SwapSingleV3};
use anchor_lang::prelude::*;
use anchor_spl::{
    token::Token,
    token_interface::{Mint, Token2022, TokenAccount},
};

use referral::{program::Referral, states::ReferralAccount};

#[derive(Accounts)]
pub struct SwapRouterBaseIn<'info> {
    /// The user performing the swap
    pub payer: Signer<'info>,

    /// The token account that pays input tokens for the swap
    #[account(mut)]
    pub input_token_account: InterfaceAccount<'info, TokenAccount>,

    /// The mint of input token
    #[account(mut)]
    pub input_token_mint: InterfaceAccount<'info, Mint>,

    /// SPL program for token transfers
    pub token_program: Program<'info, Token>,
    /// SPL program 2022 for token transfers
    pub token_program_2022: Program<'info, Token2022>,

    /// CHECK:
    // #[account(
    //     address = spl_memo::id()
    // )]
    pub memo_program: UncheckedAccount<'info>,

    /// 项目方
    #[account(
        mut,
    )]
    pub project_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    // Referral
    pub input_mint: Box<InterfaceAccount<'info, Mint>>,

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
            upper_upper_token_account.mint == input_mint.key()
        )
        @ ErrorCode::UpperUpperTokenAccountMismatch
    )]
    pub upper_upper_token_account: Option<InterfaceAccount<'info, TokenAccount>>,

    #[account(address = referral::id())]
    pub referral: Program<'info, Referral>,

    pub system_program: Program<'info, System>,

    pub associated_token_program: Program<'info, anchor_spl::associated_token::AssociatedToken>,
}

pub fn swap_router_base_in<'a, 'b, 'c: 'info, 'info>(
    ctx: Context<'a, 'b, 'c, 'info, SwapRouterBaseIn<'info>>,
    amount_in: u64,
    amount_out_minimum: u64,
) -> Result<()> {
    let mut amount_in_internal = amount_in;
    let mut input_token_account = Box::new(ctx.accounts.input_token_account.clone());
    let mut input_token_mint = Box::new(ctx.accounts.input_token_mint.clone());
    let mut accounts: &[AccountInfo] = ctx.remaining_accounts;
    while !accounts.is_empty() {
        let mut remaining_accounts = accounts.iter();
        let account_info = remaining_accounts.next().unwrap();
        if accounts.len() != ctx.remaining_accounts.len()
            && account_info.data_len() != AmmConfig::LEN
        {
            accounts = remaining_accounts.as_slice();
            continue;
        }
        let amm_config = Box::new(Account::<AmmConfig>::try_from(account_info)?);
        let pool_state_loader =
            AccountLoader::<PoolState>::try_from(remaining_accounts.next().unwrap())?;
        let output_token_account = Box::new(InterfaceAccount::<TokenAccount>::try_from(
            &remaining_accounts.next().unwrap(),
        )?);
        let input_vault = Box::new(InterfaceAccount::<TokenAccount>::try_from(
            remaining_accounts.next().unwrap(),
        )?);
        let output_vault = Box::new(InterfaceAccount::<TokenAccount>::try_from(
            remaining_accounts.next().unwrap(),
        )?);
        let output_token_mint = Box::new(InterfaceAccount::<Mint>::try_from(
            remaining_accounts.next().unwrap(),
        )?);
        let observation_state =
            AccountLoader::<ObservationState>::try_from(remaining_accounts.next().unwrap())?;

        {
            let pool_state = pool_state_loader.load()?;
            // check observation account is owned by the pool
            require_keys_eq!(pool_state.observation_key, observation_state.key());
            // check ammConfig account is associate with the pool
            require_keys_eq!(pool_state.amm_config, amm_config.key());
        }

        // solana_program::log::sol_log_compute_units();
        accounts = remaining_accounts.as_slice();
        amount_in_internal = exact_internal_v3(
            &mut SwapSingleV3 {
                payer: ctx.accounts.payer.clone(),
                amm_config,
                input_token_account: input_token_account.clone(),
                pool_state: pool_state_loader,
                output_token_account: output_token_account.clone(),
                input_vault: input_vault.clone(),
                output_vault: output_vault.clone(),
                input_vault_mint: input_token_mint.clone(),
                output_vault_mint: output_token_mint.clone(),
                observation_state,
                project_token_account: ctx.accounts.project_token_account.clone(),
                token_program: ctx.accounts.token_program.clone(),
                token_program_2022: ctx.accounts.token_program_2022.clone(),
                memo_program: ctx.accounts.memo_program.clone(),
                // Add Referral
                input_mint: ctx.accounts.input_mint.clone(),
                payer_referral: ctx.accounts.payer_referral.clone(),
                upper: ctx.accounts.upper.clone(),
                upper_token_account: ctx.accounts.upper_token_account.clone(),
                upper_referral: ctx.accounts.upper_referral.clone(),
                upper_upper: ctx.accounts.upper_upper.clone(),
                upper_upper_token_account: ctx.accounts.upper_upper_token_account.clone(),
                referral: ctx.accounts.referral.clone(),
                system_program: ctx.accounts.system_program.clone(),
                associated_token_program: ctx.accounts.associated_token_program.clone(),

            },
            accounts,
            amount_in_internal,
            0,
            true,
        )?;
        // output token is the new swap input token
        input_token_account = output_token_account;
        input_token_mint = output_token_mint;
    }
    require_gte!(
        amount_in_internal,
        amount_out_minimum,
        ErrorCode::TooLittleOutputReceived
    );

    Ok(())
}
