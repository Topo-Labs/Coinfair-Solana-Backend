use crate::error::ErrorCode;
use crate::states::PoolState;
use anchor_lang::{prelude::*, system_program};
use anchor_spl::{
    token::{Token, TokenAccount},
    token_2022::{self},
    token_interface::{initialize_account3, InitializeAccount3, Mint},
};
use spl_token_2022::{
    self,
    extension::{
        transfer_fee::{TransferFeeConfig, MAX_FEE_BASIS_POINTS},
        BaseStateWithExtensions, ExtensionType, StateWithExtensions,
    },
};
use std::collections::HashSet;

const MINT_WHITELIST: [&'static str; 4] = [
    "HVbpJAQGNpkgBaYBZQBR1t7yFdvaYVp2vCQQfKKEN4tM", //USDP
    "Crn4x1Y2HUKko7ox2EZMT6N2t2ZyH7eKtwkBGVnhEq1g", //GYEN(?)
    "FrBfWJ4qE5sCzKm3k3JaAtqZcXUh4LvJygDeketsrsH4", //ZUSD(?)
    "2b1kV6DkPAnxd5ixfnxCpjxmKwqjjaYmCZfHsFu24GXo", //PYUSD
];

#[event]
pub struct ReferralRewardEvent {
    pub from: Pubkey,   // Payer
    pub to: Pubkey,     // Upper or Lower
    pub mint: Pubkey,   // 奖励的代币
    pub amount: u64,    // 奖励数量
    pub timestamp: i64, // 时间戳
}

// 实时分佣给swap payer的上级和上上级
#[allow(unused_variables)]
pub fn transfer_from_pool_vault_to_uppers_and_project<'info>(
    pool_state_loader: &AccountLoader<'info, PoolState>,
    authority: &AccountInfo<'info>,
    from_vault: &AccountInfo<'info>,
    project_token_account: &AccountInfo<'info>,
    upper_token_account: Option<AccountInfo<'info>>,
    upper_upper_token_account: Option<AccountInfo<'info>>,
    mint: AccountInfo<'info>,
    mint_decimals: u8,
    token_program: AccountInfo<'info>,
    // 这里的奖励总费用（已经根据是否存在上级，扣除了协议方部分）
    total_reward_fee: u64,
    signer_seeds: &[&[&[u8]]],
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

    let uppers_total_reward_fee;

    // 有上级存在：
    if let Some(upper_token_account) = upper_token_account.clone() {
        // 上级链和项目方，各占trade_fee的1/2
        let project_reward_fee = total_reward_fee / 2;
        transfer_from_pool_vault_to_user(
            authority.to_account_info(),
            from_vault.to_account_info(),
            project_token_account.to_account_info(),
            mint.clone(),
            token_program.clone(),
            project_reward_fee,
            mint_decimals,
            signer_seeds,
        )?;

        emit!(ReferralRewardEvent {
            from,
            to: project,
            mint: reward_mint,
            amount: project_reward_fee,
            timestamp: Clock::get()?.unix_timestamp,
        });

        // 上级链的总奖励
        uppers_total_reward_fee = total_reward_fee - project_reward_fee;

        // 若上上级存在，则上级和上上级各占trade_fee的25%，5%
        if let Some(upper_upper_token_account) = upper_upper_token_account {
            let upper_reward_fee = uppers_total_reward_fee * 5 / 6;
            let upper_upper_reward_fee = uppers_total_reward_fee - upper_reward_fee;

            // 给上级分佣（25%）
            transfer_from_pool_vault_to_user(
                authority.to_account_info(),
                from_vault.to_account_info(),
                upper_token_account.to_account_info(),
                mint.clone(),
                token_program.clone(),
                upper_reward_fee,
                mint_decimals,
                signer_seeds,
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
                authority.to_account_info(),
                from_vault.to_account_info(),
                upper_upper_token_account.to_account_info(),
                mint.clone(),
                token_program.clone(),
                upper_upper_reward_fee,
                mint_decimals,
                signer_seeds,
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

        // 若上上级不存在：上级独占trade_fee的30%
        } else {
            transfer_from_pool_vault_to_user(
                authority.to_account_info(),
                from_vault.to_account_info(),
                upper_token_account.to_account_info(),
                mint,
                token_program,
                uppers_total_reward_fee,
                mint_decimals,
                signer_seeds,
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

    // 如无上级存在：剩余奖励部分全为项目方所得
    } else {
        let project_reward_fee = total_reward_fee;
        transfer_from_pool_vault_to_user(
            authority.to_account_info(),
            from_vault.to_account_info(),
            project_token_account.to_account_info(),
            mint.clone(),
            token_program.clone(),
            project_reward_fee,
            mint_decimals,
            signer_seeds,
        )?;

        emit!(ReferralRewardEvent {
            from,
            to: project,
            mint: reward_mint,
            amount: project_reward_fee,
            timestamp: Clock::get()?.unix_timestamp,
        });
    }

    return Ok(());
}

pub fn transfer_from_user_to_pool_vault<'a>(
    authority: AccountInfo<'a>,
    from: AccountInfo<'a>,
    to_vault: AccountInfo<'a>,
    mint: AccountInfo<'a>,
    token_program: AccountInfo<'a>,
    amount: u64,
    mint_decimals: u8,
) -> Result<()> {
    if amount == 0 {
        return Ok(());
    }
    token_2022::transfer_checked(
        CpiContext::new(
            token_program.to_account_info(),
            token_2022::TransferChecked {
                from,
                to: to_vault,
                authority,
                mint,
            },
        ),
        amount,
        mint_decimals,
    )
}

pub fn transfer_from_pool_vault_to_user<'a>(
    authority: AccountInfo<'a>,
    from_vault: AccountInfo<'a>,
    to: AccountInfo<'a>,
    mint: AccountInfo<'a>,
    token_program: AccountInfo<'a>,
    amount: u64,
    mint_decimals: u8,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    if amount == 0 {
        return Ok(());
    }
    token_2022::transfer_checked(
        CpiContext::new_with_signer(
            token_program.to_account_info(),
            token_2022::TransferChecked {
                from: from_vault,
                to,
                authority,
                mint,
            },
            signer_seeds,
        ),
        amount,
        mint_decimals,
    )
}

/// 发出 spl_token `MintTo` 指令。
pub fn token_mint_to<'a>(
    authority: AccountInfo<'a>,
    token_program: AccountInfo<'a>,
    mint: AccountInfo<'a>,
    destination: AccountInfo<'a>,
    amount: u64,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    token_2022::mint_to(
        CpiContext::new_with_signer(
            token_program,
            token_2022::MintTo {
                to: destination,
                authority,
                mint,
            },
            signer_seeds,
        ),
        amount,
    )
}

pub fn token_burn<'a>(
    authority: AccountInfo<'a>,
    token_program: AccountInfo<'a>,
    mint: AccountInfo<'a>,
    from: AccountInfo<'a>,
    amount: u64,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    token_2022::burn(
        CpiContext::new_with_signer(
            token_program.to_account_info(),
            token_2022::Burn { from, authority, mint },
            signer_seeds,
        ),
        amount,
    )
}

/// 计算输出量的费用
pub fn get_transfer_inverse_fee(mint_info: &AccountInfo, post_fee_amount: u64) -> Result<u64> {
    if *mint_info.owner == Token::id() {
        return Ok(0);
    }
    if post_fee_amount == 0 {
        return err!(ErrorCode::InvalidInput);
    }
    let mint_data = mint_info.try_borrow_data()?;
    let mint = StateWithExtensions::<spl_token_2022::state::Mint>::unpack(&mint_data)?;

    let fee = if let Ok(transfer_fee_config) = mint.get_extension::<TransferFeeConfig>() {
        let epoch = Clock::get()?.epoch;

        let transfer_fee = transfer_fee_config.get_epoch_fee(epoch);
        if u16::from(transfer_fee.transfer_fee_basis_points) == MAX_FEE_BASIS_POINTS {
            u64::from(transfer_fee.maximum_fee)
        } else {
            let transfer_fee = transfer_fee_config
                .calculate_inverse_epoch_fee(epoch, post_fee_amount)
                .unwrap();
            let transfer_fee_for_check = transfer_fee_config
                .calculate_epoch_fee(epoch, post_fee_amount.checked_add(transfer_fee).unwrap())
                .unwrap();
            if transfer_fee != transfer_fee_for_check {
                return err!(ErrorCode::TransferFeeCalculateNotMatch);
            }
            transfer_fee
        }
    } else {
        0
    };
    Ok(fee)
}

/// 计算输入量的费用
pub fn get_transfer_fee(mint_info: &AccountInfo, pre_fee_amount: u64) -> Result<u64> {
    if *mint_info.owner == Token::id() {
        return Ok(0);
    }
    let mint_data = mint_info.try_borrow_data()?;
    let mint = StateWithExtensions::<spl_token_2022::state::Mint>::unpack(&mint_data)?;

    let fee = if let Ok(transfer_fee_config) = mint.get_extension::<TransferFeeConfig>() {
        transfer_fee_config
            .calculate_epoch_fee(Clock::get()?.epoch, pre_fee_amount)
            .unwrap()
    } else {
        0
    };
    Ok(fee)
}

pub fn is_supported_mint(mint_account: &InterfaceAccount<Mint>) -> Result<bool> {
    let mint_info = mint_account.to_account_info();
    if *mint_info.owner == Token::id() {
        return Ok(true);
    }
    let mint_whitelist: HashSet<&str> = MINT_WHITELIST.into_iter().collect();
    if mint_whitelist.contains(mint_account.key().to_string().as_str()) {
        return Ok(true);
    }
    let mint_data = mint_info.try_borrow_data()?;
    let mint = StateWithExtensions::<spl_token_2022::state::Mint>::unpack(&mint_data)?;
    let extensions = mint.get_extension_types()?;
    for e in extensions {
        if e != ExtensionType::TransferFeeConfig
            && e != ExtensionType::MetadataPointer
            && e != ExtensionType::TokenMetadata
            && e != ExtensionType::InterestBearingConfig
            && e != ExtensionType::ScaledUiAmount
        {
            return Ok(false);
        }
    }
    Ok(true)
}

pub fn create_token_account<'a>(
    authority: &AccountInfo<'a>,
    payer: &AccountInfo<'a>,
    token_account: &AccountInfo<'a>,
    mint_account: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    signer_seeds: &[&[u8]],
) -> Result<()> {
    let space = {
        let mint_info = mint_account.to_account_info();
        if *mint_info.owner == token_2022::Token2022::id() {
            let mint_data = mint_info.try_borrow_data()?;
            let mint_state = StateWithExtensions::<spl_token_2022::state::Mint>::unpack(&mint_data)?;
            let mint_extensions = mint_state.get_extension_types()?;
            let required_extensions = ExtensionType::get_required_init_account_extensions(&mint_extensions);
            ExtensionType::try_calculate_account_len::<spl_token_2022::state::Account>(&required_extensions)?
        } else {
            TokenAccount::LEN
        }
    };
    create_or_allocate_account(
        token_program.key,
        payer.to_account_info(),
        system_program.to_account_info(),
        token_account.to_account_info(),
        signer_seeds,
        space,
    )?;
    initialize_account3(CpiContext::new(
        token_program.to_account_info(),
        InitializeAccount3 {
            account: token_account.to_account_info(),
            mint: mint_account.to_account_info(),
            authority: authority.to_account_info(),
        },
    ))
}

pub fn create_or_allocate_account<'a>(
    program_id: &Pubkey,
    payer: AccountInfo<'a>,
    system_program: AccountInfo<'a>,
    target_account: AccountInfo<'a>,
    siger_seed: &[&[u8]],
    space: usize,
) -> Result<()> {
    let rent = Rent::get()?;
    let current_lamports = target_account.lamports();

    if current_lamports == 0 {
        let lamports = rent.minimum_balance(space);
        let cpi_accounts = system_program::CreateAccount {
            from: payer,
            to: target_account.clone(),
        };
        let cpi_context = CpiContext::new(system_program.clone(), cpi_accounts);
        system_program::create_account(
            cpi_context.with_signer(&[siger_seed]),
            lamports,
            u64::try_from(space).unwrap(),
            program_id,
        )?;
    } else {
        let required_lamports = rent.minimum_balance(space).max(1).saturating_sub(current_lamports);
        if required_lamports > 0 {
            let cpi_accounts = system_program::Transfer {
                from: payer.to_account_info(),
                to: target_account.clone(),
            };
            let cpi_context = CpiContext::new(system_program.clone(), cpi_accounts);
            system_program::transfer(cpi_context, required_lamports)?;
        }
        let cpi_accounts = system_program::Allocate {
            account_to_allocate: target_account.clone(),
        };
        let cpi_context = CpiContext::new(system_program.clone(), cpi_accounts);
        system_program::allocate(cpi_context.with_signer(&[siger_seed]), u64::try_from(space).unwrap())?;

        let cpi_accounts = system_program::Assign {
            account_to_assign: target_account.clone(),
        };
        let cpi_context = CpiContext::new(system_program.clone(), cpi_accounts);
        system_program::assign(cpi_context.with_signer(&[siger_seed]), program_id)?;
    }
    Ok(())
}
