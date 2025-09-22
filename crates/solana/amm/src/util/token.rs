use super::get_recent_epoch;
use crate::error::ErrorCode;
use crate::states::*;
use anchor_lang::{
    prelude::*,
    solana_program,
    system_program::{create_account, CreateAccount},
};
use anchor_spl::token::{self, Token};
use anchor_spl::token_2022::{
    self,
    spl_token_2022::{
        self,
        extension::{
            metadata_pointer,
            transfer_fee::{TransferFeeConfig, MAX_FEE_BASIS_POINTS},
            BaseStateWithExtensions, ExtensionType, StateWithExtensions,
        },
    },
    Token2022,
};
use anchor_spl::token_interface::{initialize_mint2, InitializeMint2, Mint, TokenAccount};
use std::collections::HashSet;

//TODO: Remove
const MINT_WHITELIST: [&'static str; 5] = [
    "HVbpJAQGNpkgBaYBZQBR1t7yFdvaYVp2vCQQfKKEN4tM", //USDP
    "Crn4x1Y2HUKko7ox2EZMT6N2t2ZyH7eKtwkBGVnhEq1g", //GYEN(?)
    "FrBfWJ4qE5sCzKm3k3JaAtqZcXUh4LvJygDeketsrsH4", //ZUSD(?)
    "2b1kV6DkPAnxd5ixfnxCpjxmKwqjjaYmCZfHsFu24GXo", //PYUSD
    "DAUDev5e4Go36o7d9LEXSKD4YeQ3tHri9oPTXWxh5YqU", //DAU
];

pub fn invoke_memo_instruction<'info>(
    memo_msg: &[u8],
    memo_program: AccountInfo<'info>,
) -> solana_program::entrypoint::ProgramResult {
    let ix = spl_memo::build_memo(memo_msg, &Vec::new());
    let accounts = vec![memo_program];
    solana_program::program::invoke(&ix, &accounts[..])
}

#[event]
pub struct ReferralRewardEvent {
    pub from: Pubkey,   // Payer
    pub to: Pubkey,     // Upper or Lower
    pub mint: Pubkey,   // 奖励的代币
    pub amount: u64,    // 奖励数量
    pub timestamp: i64, // 时间戳
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

pub fn transfer_from_user_to_pool_vault<'info>(
    signer: &Signer<'info>,
    from: &AccountInfo<'info>,
    to_vault: &AccountInfo<'info>,
    mint: Option<Box<InterfaceAccount<'info, Mint>>>,
    token_program: &AccountInfo<'info>,
    token_program_2022: Option<AccountInfo<'info>>,
    amount: u64,
) -> Result<()> {
    if amount == 0 {
        return Ok(());
    }
    let mut token_program_info = token_program.to_account_info();
    let from_token_info = from.to_account_info();
    match (mint, token_program_2022) {
        (Some(mint), Some(token_program_2022)) => {
            if from_token_info.owner == token_program_2022.key {
                token_program_info = token_program_2022.to_account_info()
            }
            token_2022::transfer_checked(
                CpiContext::new(
                    token_program_info,
                    token_2022::TransferChecked {
                        from: from_token_info,
                        to: to_vault.to_account_info(),
                        authority: signer.to_account_info(),
                        mint: mint.to_account_info(),
                    },
                ),
                amount,
                mint.decimals,
            )
        }
        _ => token::transfer(
            CpiContext::new(
                token_program_info,
                token::Transfer {
                    from: from_token_info,
                    to: to_vault.to_account_info(),
                    authority: signer.to_account_info(),
                },
            ),
            amount,
        ),
    }
}

pub fn transfer_from_pool_vault_to_user<'info>(
    pool_state_loader: &AccountLoader<'info, PoolState>,
    from_vault: &AccountInfo<'info>,
    to: &AccountInfo<'info>,
    mint: Option<Box<InterfaceAccount<'info, Mint>>>,
    token_program: &AccountInfo<'info>,
    token_program_2022: Option<AccountInfo<'info>>,
    amount: u64,
) -> Result<()> {
    if amount == 0 {
        return Ok(());
    }
    let mut token_program_info = token_program.to_account_info();
    let from_vault_info = from_vault.to_account_info();
    match (mint, token_program_2022) {
        (Some(mint), Some(token_program_2022)) => {
            if from_vault_info.owner == token_program_2022.key {
                token_program_info = token_program_2022.to_account_info()
            }
            token_2022::transfer_checked(
                CpiContext::new_with_signer(
                    token_program_info,
                    token_2022::TransferChecked {
                        from: from_vault_info,
                        to: to.to_account_info(),
                        authority: pool_state_loader.to_account_info(),
                        mint: mint.to_account_info(),
                    },
                    &[&pool_state_loader.load()?.seeds()],
                ),
                amount,
                mint.decimals,
            )
        }
        _ => token::transfer(
            CpiContext::new_with_signer(
                token_program_info,
                token::Transfer {
                    from: from_vault_info,
                    to: to.to_account_info(),
                    authority: pool_state_loader.to_account_info(),
                },
                &[&pool_state_loader.load()?.seeds()],
            ),
            amount,
        ),
    }
}

pub fn close_spl_account<'a, 'b, 'c, 'info>(
    owner: &AccountInfo<'info>,
    destination: &AccountInfo<'info>,
    close_account: &AccountInfo<'info>,
    token_program: &AccountInfo<'info>,
    signers_seeds: &[&[&[u8]]],
) -> Result<()> {
    token_2022::close_account(CpiContext::new_with_signer(
        token_program.to_account_info(),
        token_2022::CloseAccount {
            account: close_account.to_account_info(),
            destination: destination.to_account_info(),
            authority: owner.to_account_info(),
        },
        signers_seeds,
    ))
}

pub fn burn<'a, 'b, 'c, 'info>(
    owner: &Signer<'info>,
    mint: &AccountInfo<'info>,
    burn_account: &AccountInfo<'info>,
    token_program: &AccountInfo<'info>,
    signers_seeds: &[&[&[u8]]],
    amount: u64,
) -> Result<()> {
    let mint_info = mint.to_account_info();
    let token_program_info: AccountInfo<'_> = token_program.to_account_info();
    token_2022::burn(
        CpiContext::new_with_signer(
            token_program_info,
            token_2022::Burn {
                mint: mint_info,
                from: burn_account.to_account_info(),
                authority: owner.to_account_info(),
            },
            signers_seeds,
        ),
        amount,
    )
}

/// Calculate the fee for output amount
pub fn get_transfer_inverse_fee(mint_account: Box<InterfaceAccount<Mint>>, post_fee_amount: u64) -> Result<u64> {
    let mint_info = mint_account.to_account_info();
    if *mint_info.owner == Token::id() {
        return Ok(0);
    }
    let mint_data = mint_info.try_borrow_data()?;
    let mint = StateWithExtensions::<spl_token_2022::state::Mint>::unpack(&mint_data)?;

    let fee = if let Ok(transfer_fee_config) = mint.get_extension::<TransferFeeConfig>() {
        let epoch = get_recent_epoch()?;

        let transfer_fee = transfer_fee_config.get_epoch_fee(epoch);
        if u16::from(transfer_fee.transfer_fee_basis_points) == MAX_FEE_BASIS_POINTS {
            u64::from(transfer_fee.maximum_fee)
        } else {
            transfer_fee_config
                .calculate_inverse_epoch_fee(epoch, post_fee_amount)
                .unwrap()
        }
    } else {
        0
    };
    Ok(fee)
}

/// Calculate the fee for input amount
pub fn get_transfer_fee(
    // 代币的Mint账户
    mint_account: Box<InterfaceAccount<Mint>>,
    // 转账金额
    pre_fee_amount: u64,
) -> Result<u64> {
    let mint_info = mint_account.to_account_info();
    // 确保该代币的Mint账户的Owner是Token Program的id
    if *mint_info.owner == Token::id() {
        // 返回0，意味着Token Program不收取"转账手续费"
        return Ok(0);
    }
    // 下面的逻辑意味着是 Token2022 标准
    let mint_data = mint_info.try_borrow_data()?;
    let mint = StateWithExtensions::<spl_token_2022::state::Mint>::unpack(&mint_data)?;

    // 获取该 Token2022标准的代币的"TransferFeeConfig"的配置
    let fee = if let Ok(transfer_fee_config) = mint.get_extension::<TransferFeeConfig>() {
        transfer_fee_config
            .calculate_epoch_fee(get_recent_epoch()?, pre_fee_amount)
            .unwrap()
    } else {
        // 如果该代币没有配置"TransferFeeConfig"
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
            && e != ExtensionType::MintCloseAuthority
        {
            return Ok(false);
        }
    }
    Ok(true)
}

pub fn create_position_nft_mint_with_extensions<'info>(
    payer: &Signer<'info>,
    position_nft_mint: &AccountInfo<'info>,
    mint_authority: &AccountInfo<'info>,
    mint_close_authority: &AccountInfo<'info>,
    system_program: &Program<'info, System>,
    token_2022_program: &Program<'info, Token2022>,
    with_matedata: bool,
) -> Result<()> {
    let extensions = if with_matedata {
        [ExtensionType::MintCloseAuthority, ExtensionType::MetadataPointer].to_vec()
    } else {
        [ExtensionType::MintCloseAuthority].to_vec()
    };
    let space = ExtensionType::try_calculate_account_len::<spl_token_2022::state::Mint>(&extensions)?;

    let lamports = Rent::get()?.minimum_balance(space);

    // create mint account
    create_account(
        CpiContext::new(
            system_program.to_account_info(),
            CreateAccount {
                from: payer.to_account_info(),
                to: position_nft_mint.to_account_info(),
            },
        ),
        lamports,
        space as u64,
        token_2022_program.key,
    )?;

    // initialize token extensions
    for e in extensions {
        match e {
            ExtensionType::MetadataPointer => {
                let ix = metadata_pointer::instruction::initialize(
                    token_2022_program.key,
                    position_nft_mint.key,
                    None,
                    Some(position_nft_mint.key()),
                )?;
                solana_program::program::invoke(
                    &ix,
                    &[
                        token_2022_program.to_account_info(),
                        position_nft_mint.to_account_info(),
                    ],
                )?;
            }
            ExtensionType::MintCloseAuthority => {
                let ix = spl_token_2022::instruction::initialize_mint_close_authority(
                    token_2022_program.key,
                    position_nft_mint.key,
                    Some(mint_close_authority.key),
                )?;
                solana_program::program::invoke(
                    &ix,
                    &[
                        token_2022_program.to_account_info(),
                        position_nft_mint.to_account_info(),
                    ],
                )?;
            }
            _ => {
                return err!(ErrorCode::NotSupportMint);
            }
        }
    }

    // initialize mint account
    initialize_mint2(
        CpiContext::new(
            token_2022_program.to_account_info(),
            InitializeMint2 {
                mint: position_nft_mint.to_account_info(),
            },
        ),
        0,
        &mint_authority.key(),
        None,
    )
}
