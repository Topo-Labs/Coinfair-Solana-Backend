use anyhow::Result;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    system_program, sysvar,
};
use tracing::info;

use super::ConfigManager;

/// Position相关的常量
pub const POSITION_SEED: &str = "position";
pub const TICK_ARRAY_SEED: &str = "tick_array";

/// OpenPosition指令构建器
pub struct PositionInstructionBuilder;

impl PositionInstructionBuilder {
    /// 构建OpenPositionWithToken22Nft指令
    pub fn build_open_position_with_token22_nft_instructions(
        pool_address: &Pubkey,
        pool_state: &raydium_amm_v3::states::PoolState,
        user_wallet: &Pubkey,
        nft_mint: &Pubkey,
        user_token_account_0: &Pubkey,
        user_token_account_1: &Pubkey,
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
        info!("🔨 构建OpenPositionWithToken22Nft指令");

        let raydium_program_id = ConfigManager::get_raydium_program_id()?;
        let mut instructions = Vec::new();

        // 1. 计算所有需要的PDA地址
        let (protocol_position, _) = Pubkey::find_program_address(
            &[POSITION_SEED.as_bytes(), pool_address.as_ref(), &tick_lower_index.to_be_bytes(), &tick_upper_index.to_be_bytes()],
            &raydium_program_id,
        );

        let (personal_position, _) = Pubkey::find_program_address(&[POSITION_SEED.as_bytes(), nft_mint.as_ref()], &raydium_program_id);

        let (tick_array_lower, _) = Pubkey::find_program_address(&[TICK_ARRAY_SEED.as_bytes(), pool_address.as_ref(), &tick_array_lower_start_index.to_be_bytes()], &raydium_program_id);

        let (tick_array_upper, _) = Pubkey::find_program_address(&[TICK_ARRAY_SEED.as_bytes(), pool_address.as_ref(), &tick_array_upper_start_index.to_be_bytes()], &raydium_program_id);

        // NFT ATA账户（始终使用Token-2022）
        let nft_ata_token_account = spl_associated_token_account::get_associated_token_address_with_program_id(
            user_wallet,
            nft_mint,
            &spl_token_2022::id(), // 始终使用Token-2022
        );

        // 2. 构建账户列表（严格按照CLI版本的顺序）
        let mut accounts = vec![
            AccountMeta::new(*user_wallet, true),                                 // 1. payer
            AccountMeta::new_readonly(*user_wallet, false),                       // 2. position_nft_owner
            AccountMeta::new(*nft_mint, true),                                    // 3. position_nft_mint
            AccountMeta::new(nft_ata_token_account, false),                       // 4. position_nft_account
            AccountMeta::new(*pool_address, false),                               // 5. pool_state
            AccountMeta::new(protocol_position, false),                           // 6. protocol_position
            AccountMeta::new(tick_array_lower, false),                            // 7. tick_array_lower
            AccountMeta::new(tick_array_upper, false),                            // 8. tick_array_upper
            AccountMeta::new(personal_position, false),                           // 9. personal_position
            AccountMeta::new(*user_token_account_0, false),                       // 10. token_account_0
            AccountMeta::new(*user_token_account_1, false),                       // 11. token_account_1
            AccountMeta::new(pool_state.token_vault_0, false),                    // 12. token_vault_0
            AccountMeta::new(pool_state.token_vault_1, false),                    // 13. token_vault_1
            AccountMeta::new_readonly(sysvar::rent::id(), false),                 // 14. rent
            AccountMeta::new_readonly(system_program::id(), false),               // 15. system_program
            AccountMeta::new_readonly(spl_token::id(), false),                    // 16. token_program
            AccountMeta::new_readonly(spl_associated_token_account::id(), false), // 17. associated_token_program
            AccountMeta::new_readonly(spl_token_2022::id(), false),               // 18. token_program_2022
            AccountMeta::new_readonly(pool_state.token_mint_0, false),            // 19. vault_0_mint
            AccountMeta::new_readonly(pool_state.token_mint_1, false),            // 20. vault_1_mint
        ];

        // 添加remaining accounts
        accounts.extend(remaining_accounts);

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

        info!("✅ OpenPositionWithToken22Nft指令构建完成");
        Ok(instructions)
    }

    /// 构建指令数据（使用正确的discriminator）
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
        let mut data = Vec::new();

        // 计算正确的discriminator
        // Anchor使用 sha256("global:open_position_with_token22_nft") 的前8字节
        use solana_sdk::hash::hash;
        let discriminator = hash(b"global:open_position_with_token22_nft").to_bytes();
        data.extend_from_slice(&discriminator[..8]);

        // 参数序列化（按照Anchor的格式）
        data.extend_from_slice(&tick_lower_index.to_le_bytes());
        data.extend_from_slice(&tick_upper_index.to_le_bytes());
        data.extend_from_slice(&tick_array_lower_start_index.to_le_bytes());
        data.extend_from_slice(&tick_array_upper_start_index.to_le_bytes());
        data.extend_from_slice(&liquidity.to_le_bytes());
        data.extend_from_slice(&amount_0_max.to_le_bytes());
        data.extend_from_slice(&amount_1_max.to_le_bytes());
        data.push(if with_metadata { 1 } else { 0 });
        // base_flag为None，使用0表示None
        data.push(0);

        Ok(data)
    }

    /// 构建计算预算指令
    pub fn build_compute_budget_instruction(compute_units: u32) -> Instruction {
        solana_sdk::compute_budget::ComputeBudgetInstruction::set_compute_unit_limit(compute_units)
    }

    /// 构建完整的交易指令序列（支持Token-2022）
    pub fn build_complete_open_position_transaction(
        _pool_address: &Pubkey,
        _user_wallet: &Pubkey,
        _nft_mint: &Pubkey,
        _token_mints: &[Pubkey], // [mint0, mint1]
        _tick_lower_index: i32,
        _tick_upper_index: i32,
        _tick_array_lower_start_index: i32,
        _tick_array_upper_start_index: i32,
        _liquidity: u128,
        _amount_0_max: u64,
        _amount_1_max: u64,
        _with_metadata: bool,
        _remaining_accounts: Vec<AccountMeta>,
        _compute_units: Option<u32>,
    ) -> Result<Vec<Instruction>> {
        // 这个方法需要pool_state参数，所以标记为过时
        // 请使用build_complete_open_position_transaction_v2
        Err(anyhow::anyhow!("请使用build_complete_open_position_transaction_v2方法"))
    }

    /// 构建完整的交易指令序列V2（支持Token-2022和transfer fee）
    pub fn build_complete_open_position_transaction_v2(
        pool_address: &Pubkey,
        pool_state: &raydium_amm_v3::states::PoolState,
        user_wallet: &Pubkey,
        nft_mint: &Pubkey,
        user_token_account_0: &Pubkey,
        user_token_account_1: &Pubkey,
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

        // 2. 添加OpenPositionWithToken22Nft核心指令
        let open_position_instructions = Self::build_open_position_with_token22_nft_instructions(
            pool_address,
            pool_state,
            user_wallet,
            nft_mint,
            user_token_account_0,
            user_token_account_1,
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
