use anyhow::Result;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    system_program, sysvar,
};
use tracing::info;

use super::{ConfigManager, PDACalculator};

/// OpenPosition指令构建器
pub struct PositionInstructionBuilder;

impl PositionInstructionBuilder {
    /// 构建OpenPosition指令序列
    pub fn build_open_position_instructions(
        pool_address: &Pubkey,
        user_wallet: &Pubkey,
        nft_mint: &Pubkey,
        tick_lower_index: i32,
        tick_upper_index: i32,
        tick_array_lower_start_index: i32,
        tick_array_upper_start_index: i32,
        liquidity: u128,
        amount_0_max: u64,
        amount_1_max: u64,
        with_metadata: bool,
        remaining_accounts: Vec<AccountMeta>,
    ) -> Result<Vec<Instruction>> {
        info!("🔨 构建OpenPosition指令");

        let raydium_program_id = ConfigManager::get_raydium_program_id()?;
        let mut instructions = Vec::new();

        // 1. 计算所有需要的PDA地址
        let pdas = Self::calculate_position_pdas(
            &raydium_program_id,
            pool_address,
            nft_mint,
            tick_lower_index,
            tick_upper_index,
            tick_array_lower_start_index,
            tick_array_upper_start_index,
        )?;

        // 2. 构建账户列表
        let accounts = Self::build_open_position_accounts(user_wallet, nft_mint, &pdas, remaining_accounts)?;

        // 3. 构建指令数据
        let instruction_data = Self::build_open_position_instruction_data(
            tick_lower_index,
            tick_upper_index,
            tick_array_lower_start_index,
            tick_array_upper_start_index,
            liquidity,
            amount_0_max,
            amount_1_max,
            with_metadata,
        )?;

        // 4. 创建OpenPosition指令
        let open_position_instruction = Instruction {
            program_id: raydium_program_id,
            accounts,
            data: instruction_data,
        };

        instructions.push(open_position_instruction);

        info!("✅ OpenPosition指令构建完成，共{}个指令", instructions.len());
        Ok(instructions)
    }

    /// 计算Position相关的PDA地址
    fn calculate_position_pdas(
        raydium_program_id: &Pubkey,
        pool_address: &Pubkey,
        nft_mint: &Pubkey,
        tick_lower_index: i32,
        tick_upper_index: i32,
        tick_array_lower_start_index: i32,
        tick_array_upper_start_index: i32,
    ) -> Result<PositionPDAs> {
        // Protocol position PDA
        let (protocol_position, protocol_position_bump) = Pubkey::find_program_address(&[b"position", pool_address.as_ref(), &tick_lower_index.to_be_bytes(), &tick_upper_index.to_be_bytes()], raydium_program_id);

        // Personal position PDA
        let (personal_position, personal_position_bump) = Pubkey::find_program_address(&[b"position", nft_mint.as_ref()], raydium_program_id);

        // Tick arrays
        let (tick_array_lower, _) = PDACalculator::calculate_tick_array_pda(raydium_program_id, pool_address, tick_array_lower_start_index);

        let (tick_array_upper, _) = PDACalculator::calculate_tick_array_pda(raydium_program_id, pool_address, tick_array_upper_start_index);

        // NFT相关地址
        let nft_token_account = spl_associated_token_account::get_associated_token_address(
            &personal_position, // NFT所有者为personal position账户
            nft_mint,
        );

        // Metadata账户（如果需要）
        let metadata_account = Self::derive_metadata_pda(nft_mint)?;

        Ok(PositionPDAs {
            protocol_position,
            protocol_position_bump,
            personal_position,
            personal_position_bump,
            tick_array_lower,
            tick_array_upper,
            nft_token_account,
            metadata_account,
        })
    }

    /// 构建账户列表
    fn build_open_position_accounts(user_wallet: &Pubkey, nft_mint: &Pubkey, pdas: &PositionPDAs, remaining_accounts: Vec<AccountMeta>) -> Result<Vec<AccountMeta>> {
        let mut accounts = Vec::new();

        // 核心账户
        accounts.extend_from_slice(&[
            AccountMeta::new(*user_wallet, true),            // payer
            AccountMeta::new_readonly(*user_wallet, false),  // position_nft_owner
            AccountMeta::new(*nft_mint, true),               // position_nft_mint
            AccountMeta::new(pdas.nft_token_account, false), // position_nft_account
            AccountMeta::new(pdas.metadata_account, false),  // metadata_account
            AccountMeta::new(pdas.protocol_position, false), // protocol_position
            AccountMeta::new(pdas.tick_array_lower, false),  // tick_array_lower
            AccountMeta::new(pdas.tick_array_upper, false),  // tick_array_upper
            AccountMeta::new(pdas.personal_position, false), // personal_position
        ]);

        // TODO: 添加token账户（需要从pool状态中获取）
        // accounts.push(AccountMeta::new(user_token_account_0, false));
        // accounts.push(AccountMeta::new(user_token_account_1, false));
        // accounts.push(AccountMeta::new(token_vault_0, false));
        // accounts.push(AccountMeta::new(token_vault_1, false));

        // 系统账户
        accounts.extend_from_slice(&[
            AccountMeta::new_readonly(sysvar::rent::id(), false),
            AccountMeta::new_readonly(system_program::id(), false),
            AccountMeta::new_readonly(spl_token::id(), false),
            AccountMeta::new_readonly(spl_associated_token_account::id(), false),
            AccountMeta::new_readonly(mpl_token_metadata::ID, false),
        ]);

        // Remaining accounts
        accounts.extend(remaining_accounts);

        Ok(accounts)
    }

    /// 构建指令数据
    fn build_open_position_instruction_data(
        tick_lower_index: i32,
        tick_upper_index: i32,
        tick_array_lower_start_index: i32,
        tick_array_upper_start_index: i32,
        liquidity: u128,
        amount_0_max: u64,
        amount_1_max: u64,
        with_metadata: bool,
    ) -> Result<Vec<u8>> {
        // 这里需要根据实际的Raydium指令格式来构建
        // 暂时使用一个简化的结构
        let mut data = Vec::new();

        // 指令标识符（8字节）
        data.extend_from_slice(&[0x47, 0x32, 0xD4, 0xEC, 0xB6, 0x95, 0x4B, 0x5B]); // 示例discriminator

        // 参数序列化
        data.extend_from_slice(&tick_lower_index.to_le_bytes());
        data.extend_from_slice(&tick_upper_index.to_le_bytes());
        data.extend_from_slice(&tick_array_lower_start_index.to_le_bytes());
        data.extend_from_slice(&tick_array_upper_start_index.to_le_bytes());
        data.extend_from_slice(&liquidity.to_le_bytes());
        data.extend_from_slice(&amount_0_max.to_le_bytes());
        data.extend_from_slice(&amount_1_max.to_le_bytes());
        data.push(if with_metadata { 1 } else { 0 });

        Ok(data)
    }

    /// 派生Metadata PDA
    fn derive_metadata_pda(mint: &Pubkey) -> Result<Pubkey> {
        let (metadata_pda, _) = Pubkey::find_program_address(&[b"metadata", mpl_token_metadata::ID.as_ref(), mint.as_ref()], &mpl_token_metadata::ID);
        Ok(metadata_pda)
    }

    /// 构建计算预算指令
    pub fn build_compute_budget_instruction(compute_units: u32) -> Instruction {
        solana_sdk::compute_budget::ComputeBudgetInstruction::set_compute_unit_limit(compute_units)
    }

    /// 检查并创建关联代币账户指令
    pub fn build_create_ata_instructions_if_needed(payer: &Pubkey, owner: &Pubkey, mints: &[Pubkey]) -> Vec<Instruction> {
        let mut instructions = Vec::new();

        for mint in mints {
            // 在实际使用中，需要先检查账户是否存在
            // 这里简化处理，假设需要创建
            let create_ata_ix = spl_associated_token_account::instruction::create_associated_token_account(payer, owner, mint, &spl_token::id());
            instructions.push(create_ata_ix);
        }

        instructions
    }

    /// 构建完整的交易指令序列（包括预备指令）
    pub fn build_complete_open_position_transaction(
        pool_address: &Pubkey,
        user_wallet: &Pubkey,
        nft_mint: &Pubkey,
        token_mints: &[Pubkey], // [mint0, mint1]
        tick_lower_index: i32,
        tick_upper_index: i32,
        tick_array_lower_start_index: i32,
        tick_array_upper_start_index: i32,
        liquidity: u128,
        amount_0_max: u64,
        amount_1_max: u64,
        with_metadata: bool,
        remaining_accounts: Vec<AccountMeta>,
        compute_units: Option<u32>,
    ) -> Result<Vec<Instruction>> {
        let mut instructions = Vec::new();

        // 1. 添加计算预算指令
        if let Some(units) = compute_units {
            instructions.push(Self::build_compute_budget_instruction(units));
        }

        // 2. 创建必要的关联代币账户
        let ata_instructions = Self::build_create_ata_instructions_if_needed(user_wallet, user_wallet, token_mints);
        instructions.extend(ata_instructions);

        // 3. 添加OpenPosition核心指令
        let open_position_instructions = Self::build_open_position_instructions(
            pool_address,
            user_wallet,
            nft_mint,
            tick_lower_index,
            tick_upper_index,
            tick_array_lower_start_index,
            tick_array_upper_start_index,
            liquidity,
            amount_0_max,
            amount_1_max,
            with_metadata,
            remaining_accounts,
        )?;
        instructions.extend(open_position_instructions);

        info!("🎯 完整交易构建完成，共{}个指令", instructions.len());
        Ok(instructions)
    }
}

/// Position相关的PDA地址集合
#[derive(Debug, Clone)]
struct PositionPDAs {
    protocol_position: Pubkey,
    protocol_position_bump: u8,
    personal_position: Pubkey,
    personal_position_bump: u8,
    tick_array_lower: Pubkey,
    tick_array_upper: Pubkey,
    nft_token_account: Pubkey,
    metadata_account: Pubkey,
}
