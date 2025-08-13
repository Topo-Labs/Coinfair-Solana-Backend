use anchor_lang::Discriminator;
use anyhow::Result;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use raydium_amm_v3::instruction;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

use super::{constants, MathUtils};

/// 交易构建器 - 统一管理交易构建逻辑
pub struct TransactionBuilder;

impl TransactionBuilder {
    /// 创建基础交易消息
    pub fn create_base_transaction_message(instructions: &[solana_sdk::instruction::Instruction], payer: &Pubkey) -> solana_sdk::message::Message {
        solana_sdk::message::Message::new(instructions, Some(payer))
    }

    /// 添加计算预算指令
    pub fn create_compute_budget_instruction(compute_units: u32) -> solana_sdk::instruction::Instruction {
        solana_sdk::compute_budget::ComputeBudgetInstruction::set_compute_unit_limit(compute_units)
    }

    /// 构建完整的交易
    pub fn build_transaction(
        instructions: Vec<solana_sdk::instruction::Instruction>,
        payer: &Pubkey,
        recent_blockhash: solana_sdk::hash::Hash,
    ) -> Result<solana_sdk::transaction::Transaction> {
        let mut full_instructions = vec![Self::create_compute_budget_instruction(1_400_000)];
        full_instructions.extend(instructions);

        let message = Self::create_base_transaction_message(&full_instructions, payer);
        let mut transaction = solana_sdk::transaction::Transaction::new_unsigned(message);
        transaction.message.recent_blockhash = recent_blockhash;

        Ok(transaction)
    }

    /// 序列化交易为Base64
    pub fn serialize_transaction_to_base64(transaction: &solana_sdk::transaction::Transaction) -> Result<String> {
        let serialized = bincode::serialize(transaction)?;
        Ok(STANDARD.encode(&serialized))
    }
}

/// AMM配置指令构建器 - 构建AMM配置相关指令
pub struct AmmConfigInstructionBuilder;

impl AmmConfigInstructionBuilder {
    /// 构建创建AMM配置指令
    pub fn build_create_amm_config_instruction(
        program_id: &Pubkey,
        owner: &Pubkey,
        config_index: u16,
        tick_spacing: u16,
        trade_fee_rate: u32,
        protocol_fee_rate: u32,
        fund_fee_rate: u32,
    ) -> Result<solana_sdk::instruction::Instruction> {
        use super::{LogUtils, PDACalculator};
        use borsh::BorshSerialize;

        LogUtils::log_operation_start("创建AMM配置指令构建", &format!("配置索引: {}", config_index));

        // 计算AMM配置PDA
        let (amm_config_key, _bump) = PDACalculator::calculate_amm_config_pda(program_id, config_index);

        // 使用预定义的discriminator常量 - CreateAmmConfig指令
        let discriminator = instruction::CreateAmmConfig::DISCRIMINATOR;

        #[derive(BorshSerialize)]
        struct CreateAmmConfigArgs {
            index: u16,
            tick_spacing: u16,
            trade_fee_rate: u32,
            protocol_fee_rate: u32,
            fund_fee_rate: u32,
        }

        let args = CreateAmmConfigArgs {
            index: config_index,
            tick_spacing,
            trade_fee_rate,
            protocol_fee_rate,
            fund_fee_rate,
        };

        let mut data = discriminator.to_vec();
        args.serialize(&mut data)?;

        // 构建账户列表
        let accounts = vec![
            AccountMetaBuilder::signer(*owner),                                    // owner (signer)
            AccountMetaBuilder::writable(amm_config_key, false),                   // amm_config (writable, PDA)
            AccountMetaBuilder::readonly(solana_sdk::system_program::id(), false), // system_program
        ];

        let instruction = solana_sdk::instruction::Instruction {
            program_id: *program_id,
            accounts,
            data,
        };

        LogUtils::log_operation_success(
            "创建AMM配置指令构建",
            &format!(
                "配置地址: {}, 参数: tick_spacing={}, trade_fee_rate={}, protocol_fee_rate={}, fund_fee_rate={}",
                amm_config_key, tick_spacing, trade_fee_rate, protocol_fee_rate, fund_fee_rate
            ),
        );

        Ok(instruction)
    }
}

/// 账户元数据构建器 - 统一管理账户元数据创建
pub struct AccountMetaBuilder;

impl AccountMetaBuilder {
    /// 创建只读账户元数据
    pub fn readonly(pubkey: Pubkey, is_signer: bool) -> solana_sdk::instruction::AccountMeta {
        solana_sdk::instruction::AccountMeta {
            pubkey,
            is_signer,
            is_writable: false,
        }
    }

    /// 创建可写账户元数据
    pub fn writable(pubkey: Pubkey, is_signer: bool) -> solana_sdk::instruction::AccountMeta {
        solana_sdk::instruction::AccountMeta {
            pubkey,
            is_signer,
            is_writable: true,
        }
    }

    /// 创建签名者账户元数据
    pub fn signer(pubkey: Pubkey) -> solana_sdk::instruction::AccountMeta {
        solana_sdk::instruction::AccountMeta {
            pubkey,
            is_signer: true,
            is_writable: false,
        }
    }

    /// 创建可写签名者账户元数据
    pub fn writable_signer(pubkey: Pubkey) -> solana_sdk::instruction::AccountMeta {
        solana_sdk::instruction::AccountMeta {
            pubkey,
            is_signer: true,
            is_writable: true,
        }
    }

    /// 批量创建remaining accounts
    pub fn create_remaining_accounts(account_addresses: &[String], first_readonly: bool) -> Result<Vec<solana_sdk::instruction::AccountMeta>> {
        let mut accounts = Vec::new();
        for (index, account_str) in account_addresses.iter().enumerate() {
            let pubkey = Pubkey::from_str(account_str)?;
            let is_writable = if first_readonly { index > 0 } else { true };
            accounts.push(solana_sdk::instruction::AccountMeta {
                pubkey,
                is_signer: false,
                is_writable,
            });
        }
        Ok(accounts)
    }
}

/// 路由计划构建器 - 统一管理路由计划创建
pub struct RoutePlanBuilder;

impl RoutePlanBuilder {
    /// 计算标准手续费
    pub fn calculate_standard_fee(amount: u64) -> u64 {
        MathUtils::calculate_fee(amount, constants::DEFAULT_FEE_RATE)
    }
}

/// SwapV2指令构建器 - 统一管理SwapV2指令创建
pub struct SwapV2InstructionBuilder;

impl SwapV2InstructionBuilder {
    /// 构建SwapV2指令
    pub fn build_swap_v2_instruction(
        program_id: &Pubkey,
        amm_config: &Pubkey,
        pool_state: &Pubkey,
        payer: &Pubkey,
        input_token_account: &Pubkey,
        output_token_account: &Pubkey,
        input_vault: &Pubkey,
        output_vault: &Pubkey,
        input_vault_mint: &Pubkey,
        output_vault_mint: &Pubkey,
        observation_state: &Pubkey,
        remaining_accounts: Vec<solana_sdk::instruction::AccountMeta>,
        amount: u64,
        other_amount_threshold: u64,
        sqrt_price_limit_x64: Option<u128>,
        is_base_input: bool,
    ) -> Result<solana_sdk::instruction::Instruction> {
        use super::LogUtils;
        use borsh::BorshSerialize;

        LogUtils::log_operation_start("SwapV2指令构建", &format!("金额: {}", amount));

        // 使用预定义的discriminator常量 - SwapV2指令
        let discriminator = instruction::SwapV2::DISCRIMINATOR;

        #[derive(BorshSerialize)]
        struct SwapV2Args {
            amount: u64,
            other_amount_threshold: u64,
            sqrt_price_limit_x64: u128,
            is_base_input: bool,
        }

        let args = SwapV2Args {
            amount,
            other_amount_threshold,
            sqrt_price_limit_x64: sqrt_price_limit_x64.unwrap_or(0),
            is_base_input,
        };

        let mut data = discriminator.to_vec();
        args.serialize(&mut data)?;

        // 构建账户列表
        let mut accounts = vec![
            AccountMetaBuilder::signer(*payer),
            AccountMetaBuilder::readonly(*amm_config, false),
            AccountMetaBuilder::writable(*pool_state, false),
            AccountMetaBuilder::writable(*input_token_account, false),
            AccountMetaBuilder::writable(*output_token_account, false),
            AccountMetaBuilder::writable(*input_vault, false),
            AccountMetaBuilder::writable(*output_vault, false),
            AccountMetaBuilder::writable(*observation_state, false),
            AccountMetaBuilder::readonly(spl_token::id(), false),
            AccountMetaBuilder::readonly(spl_token_2022::id(), false),
            AccountMetaBuilder::readonly(spl_memo::id(), false),
            AccountMetaBuilder::readonly(*input_vault_mint, false),
            AccountMetaBuilder::readonly(*output_vault_mint, false),
        ];

        accounts.extend(remaining_accounts);

        LogUtils::log_operation_success("SwapV2指令构建", &format!("{}个账户", accounts.len()));
        Ok(solana_sdk::instruction::Instruction {
            program_id: *program_id,
            accounts,
            data,
        })
    }
}

/// SwapV3指令构建器 - 统一管理SwapV3指令创建，支持推荐系统
pub struct SwapV3InstructionBuilder;

impl SwapV3InstructionBuilder {
    /// 构建SwapV3指令（支持推荐系统）
    pub fn build_swap_v3_instruction(
        program_id: &Pubkey,
        raydium_program_id: &Pubkey,
        referral_program_id: &Pubkey,
        amm_config: &Pubkey,
        pool_state: &Pubkey,
        payer: &Pubkey,
        input_token_account: &Pubkey,
        output_token_account: &Pubkey,
        input_vault: &Pubkey,
        output_vault: &Pubkey,
        input_vault_mint: &Pubkey,
        output_vault_mint: &Pubkey,
        observation_state: &Pubkey,
        remaining_accounts: Vec<solana_sdk::instruction::AccountMeta>,
        amount: u64,
        other_amount_threshold: u64,
        sqrt_price_limit_x64: Option<u128>,
        is_base_input: bool,
        // 推荐系统相关参数
        input_mint: &Pubkey,
        payer_referral: &Pubkey,
        upper: Option<&Pubkey>,
        upper_token_account: Option<&Pubkey>,
        upper_referral: Option<&Pubkey>,
        upper_upper: Option<&Pubkey>,
        upper_upper_token_account: Option<&Pubkey>,
        project_token_account: &Pubkey,
    ) -> Result<solana_sdk::instruction::Instruction> {
        use super::LogUtils;
        use borsh::BorshSerialize;

        LogUtils::log_operation_start("SwapV3指令构建", &format!("金额: {}, 推荐人: {:?}", amount, upper));

        // 使用预定义的discriminator常量 - SwapV3指令
        let discriminator = instruction::SwapV3::DISCRIMINATOR;

        #[derive(BorshSerialize)]
        struct SwapV3Args {
            amount: u64,
            other_amount_threshold: u64,
            sqrt_price_limit_x64: u128,
            is_base_input: bool,
        }

        let args = SwapV3Args {
            amount,
            other_amount_threshold,
            sqrt_price_limit_x64: sqrt_price_limit_x64.unwrap_or(0),
            is_base_input,
        };

        let mut data = discriminator.to_vec();
        args.serialize(&mut data)?;

        // 构建账户列表 - 按照SwapV3合约要求的顺序
        let mut accounts = vec![
            AccountMetaBuilder::signer(*payer),                                              // payer
            AccountMetaBuilder::readonly(*input_mint, false),                               // input_mint
            AccountMetaBuilder::readonly(*payer_referral, false),                           // payer_referral
        ];

        // 添加可选的upper账户
        if let Some(upper_pubkey) = upper {
            accounts.push(AccountMetaBuilder::readonly(*upper_pubkey, false));              // upper
        } else {
            accounts.push(AccountMetaBuilder::readonly(*program_id, false));                // 占位符
        }

        // 添加可选的upper_token_account
        if let Some(upper_token_pubkey) = upper_token_account {
            accounts.push(AccountMetaBuilder::writable(*upper_token_pubkey, false));        // upper_token_account
        } else {
            accounts.push(AccountMetaBuilder::readonly(*program_id, false));                // 占位符
        }

        // 添加可选的upper_referral账户
        if let Some(upper_referral_pubkey) = upper_referral {
            accounts.push(AccountMetaBuilder::readonly(*upper_referral_pubkey, false));     // upper_referral
        } else {
            accounts.push(AccountMetaBuilder::readonly(*program_id, false));                // 占位符
        }

        // 添加可选的upper_upper账户
        if let Some(upper_upper_pubkey) = upper_upper {
            accounts.push(AccountMetaBuilder::readonly(*upper_upper_pubkey, false));        // upper_upper
        } else {
            accounts.push(AccountMetaBuilder::readonly(*program_id, false));                // 占位符
        }

        // 添加可选的upper_upper_token_account
        if let Some(upper_upper_token_pubkey) = upper_upper_token_account {
            accounts.push(AccountMetaBuilder::writable(*upper_upper_token_pubkey, false));  // upper_upper_token_account
        } else {
            accounts.push(AccountMetaBuilder::readonly(*program_id, false));                // 占位符
        }

        // 添加必需的项目方代币账户
        accounts.push(AccountMetaBuilder::writable(*project_token_account, false));        // project_token_account

        // 添加核心交换账户
        accounts.extend(vec![
            AccountMetaBuilder::readonly(*amm_config, false),                               // amm_config
            AccountMetaBuilder::writable(*pool_state, false),                               // pool_state
            AccountMetaBuilder::writable(*input_token_account, false),                      // input_token_account
            AccountMetaBuilder::writable(*output_token_account, false),                     // output_token_account
            AccountMetaBuilder::writable(*input_vault, false),                              // input_vault
            AccountMetaBuilder::writable(*output_vault, false),                             // output_vault
            AccountMetaBuilder::writable(*observation_state, false),                        // observation_state
            AccountMetaBuilder::readonly(spl_token::id(), false),                           // token_program
            AccountMetaBuilder::readonly(spl_token_2022::id(), false),                      // token_program_2022
            AccountMetaBuilder::readonly(spl_memo::id(), false),                            // memo_program
            AccountMetaBuilder::readonly(solana_sdk::system_program::id(), false),          // system_program
            AccountMetaBuilder::readonly(spl_associated_token_account::id(), false),        // associated_token_program
            AccountMetaBuilder::readonly(*referral_program_id, false),                      // referral
            AccountMetaBuilder::readonly(*input_vault_mint, false),                         // input_vault_mint
            AccountMetaBuilder::readonly(*output_vault_mint, false),                        // output_vault_mint
        ]);

        // 添加remaining accounts（tick arrays等）
        accounts.extend(remaining_accounts);

        LogUtils::log_operation_success(
            "SwapV3指令构建",
            &format!("{}个账户, 推荐系统启用: {}", accounts.len(), upper.is_some())
        );

        Ok(solana_sdk::instruction::Instruction {
            program_id: *raydium_program_id,
            accounts,
            data,
        })
    }
}
