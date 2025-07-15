use anyhow::Result;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
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
    pub fn build_transaction(instructions: Vec<solana_sdk::instruction::Instruction>, payer: &Pubkey, recent_blockhash: solana_sdk::hash::Hash) -> Result<solana_sdk::transaction::Transaction> {
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

/// 账户元数据构建器 - 统一管理账户元数据创建
pub struct AccountMetaBuilder;

impl AccountMetaBuilder {
    /// 创建只读账户元数据
    pub fn readonly(pubkey: Pubkey, is_signer: bool) -> solana_sdk::instruction::AccountMeta {
        solana_sdk::instruction::AccountMeta { pubkey, is_signer, is_writable: false }
    }

    /// 创建可写账户元数据
    pub fn writable(pubkey: Pubkey, is_signer: bool) -> solana_sdk::instruction::AccountMeta {
        solana_sdk::instruction::AccountMeta { pubkey, is_signer, is_writable: true }
    }

    /// 创建签名者账户元数据
    pub fn signer(pubkey: Pubkey) -> solana_sdk::instruction::AccountMeta {
        solana_sdk::instruction::AccountMeta { pubkey, is_signer: true, is_writable: false }
    }

    /// 创建可写签名者账户元数据
    pub fn writable_signer(pubkey: Pubkey) -> solana_sdk::instruction::AccountMeta {
        solana_sdk::instruction::AccountMeta { pubkey, is_signer: true, is_writable: true }
    }

    /// 批量创建remaining accounts
    pub fn create_remaining_accounts(account_addresses: &[String], first_readonly: bool) -> Result<Vec<solana_sdk::instruction::AccountMeta>> {
        let mut accounts = Vec::new();
        for (index, account_str) in account_addresses.iter().enumerate() {
            let pubkey = Pubkey::from_str(account_str)?;
            let is_writable = if first_readonly { index > 0 } else { true };
            accounts.push(solana_sdk::instruction::AccountMeta { pubkey, is_signer: false, is_writable });
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
