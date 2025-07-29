use anchor_lang::Discriminator;
use anyhow::Result;
use raydium_amm_v3::instruction;
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
            &[
                POSITION_SEED.as_bytes(),
                pool_address.as_ref(),
                &tick_lower_index.to_be_bytes(),
                &tick_upper_index.to_be_bytes(),
            ],
            &raydium_program_id,
        );

        let (personal_position, _) = Pubkey::find_program_address(&[POSITION_SEED.as_bytes(), nft_mint.as_ref()], &raydium_program_id);

        let (tick_array_lower, _) = Pubkey::find_program_address(
            &[TICK_ARRAY_SEED.as_bytes(), pool_address.as_ref(), &tick_array_lower_start_index.to_be_bytes()],
            &raydium_program_id,
        );

        let (tick_array_upper, _) = Pubkey::find_program_address(
            &[TICK_ARRAY_SEED.as_bytes(), pool_address.as_ref(), &tick_array_upper_start_index.to_be_bytes()],
            &raydium_program_id,
        );

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

        // 使用预定义的discriminator常量
        let discriminator = instruction::OpenPositionV2::DISCRIMINATOR;
        data.extend_from_slice(&discriminator);

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

    /// 构建IncreaseLiquidityV2指令（支持Token-2022 NFT）
    pub fn build_increase_liquidity_instructions(
        pool_address: &Pubkey,
        pool_state: &raydium_amm_v3::states::PoolState,
        user_wallet: &Pubkey,
        nft_mint: &Pubkey,
        nft_token_account: &Pubkey,
        user_token_account_0: &Pubkey,
        user_token_account_1: &Pubkey,
        tick_lower_index: i32,
        tick_upper_index: i32,
        tick_array_lower_start_index: i32,
        tick_array_upper_start_index: i32,
        liquidity: u128,
        amount_0_max: u64,
        amount_1_max: u64,
        remaining_accounts: Vec<AccountMeta>,
    ) -> Result<Vec<Instruction>> {
        info!("🔨 构建IncreaseLiquidityV2指令（支持Token-2022）");

        let raydium_program_id = ConfigManager::get_raydium_program_id()?;
        let mut instructions = Vec::new();

        // 1. 计算所有需要的PDA地址
        let (protocol_position, _) = Pubkey::find_program_address(
            &[
                POSITION_SEED.as_bytes(),
                pool_address.as_ref(),
                &tick_lower_index.to_be_bytes(),
                &tick_upper_index.to_be_bytes(),
            ],
            &raydium_program_id,
        );

        let (personal_position, _) = Pubkey::find_program_address(&[POSITION_SEED.as_bytes(), nft_mint.as_ref()], &raydium_program_id);

        let (tick_array_lower, _) = Pubkey::find_program_address(
            &[TICK_ARRAY_SEED.as_bytes(), pool_address.as_ref(), &tick_array_lower_start_index.to_be_bytes()],
            &raydium_program_id,
        );

        let (tick_array_upper, _) = Pubkey::find_program_address(
            &[TICK_ARRAY_SEED.as_bytes(), pool_address.as_ref(), &tick_array_upper_start_index.to_be_bytes()],
            &raydium_program_id,
        );

        // 2. 构建账户列表（严格按照IncreaseLiquidityV2结构的顺序）
        let mut accounts = vec![
            AccountMeta::new(*user_wallet, true),                                 // 1. nft_owner (signer)
            AccountMeta::new_readonly(*nft_token_account, false),                 // 2. nft_account
            AccountMeta::new(*pool_address, false),                               // 3. pool_state
            AccountMeta::new(protocol_position, false),                           // 4. protocol_position
            AccountMeta::new(personal_position, false),                           // 5. personal_position
            AccountMeta::new(tick_array_lower, false),                            // 6. tick_array_lower
            AccountMeta::new(tick_array_upper, false),                            // 7. tick_array_upper
            AccountMeta::new(*user_token_account_0, false),                       // 8. token_account_0
            AccountMeta::new(*user_token_account_1, false),                       // 9. token_account_1
            AccountMeta::new(pool_state.token_vault_0, false),                    // 10. token_vault_0
            AccountMeta::new(pool_state.token_vault_1, false),                    // 11. token_vault_1
            AccountMeta::new_readonly(spl_token::id(), false),                    // 12. token_program
            AccountMeta::new_readonly(spl_token_2022::id(), false),               // 13. token_program_2022
            AccountMeta::new_readonly(pool_state.token_mint_0, false),            // 14. vault_0_mint
            AccountMeta::new_readonly(pool_state.token_mint_1, false),            // 15. vault_1_mint
        ];

        // 添加remaining accounts
        accounts.extend(remaining_accounts);

        // 3. 构建指令数据
        let instruction_data = Self::build_increase_liquidity_instruction_data(
            liquidity,
            amount_0_max,
            amount_1_max,
        )?;

        // 4. 创建IncreaseLiquidityV2指令（支持Token-2022）
        let increase_liquidity_instruction = Instruction {
            program_id: raydium_program_id,
            accounts,
            data: instruction_data,
        };

        instructions.push(increase_liquidity_instruction);

        info!("✅ IncreaseLiquidityV2指令构建完成（支持Token-2022）");
        Ok(instructions)
    }

    /// 构建IncreaseLiquidityV2指令数据（支持Token-2022）
    fn build_increase_liquidity_instruction_data(
        liquidity: u128,
        amount_0_max: u64,
        amount_1_max: u64,
    ) -> Result<Vec<u8>> {
        let mut data = Vec::new();

        // 使用预定义的discriminator常量 - IncreaseLiquidityV2指令
        let discriminator = instruction::IncreaseLiquidityV2::DISCRIMINATOR;
        data.extend_from_slice(&discriminator);

        // 参数序列化（按照Anchor的格式）
        data.extend_from_slice(&liquidity.to_le_bytes());
        data.extend_from_slice(&amount_0_max.to_le_bytes());
        data.extend_from_slice(&amount_1_max.to_le_bytes());
        // base_flag: Option<bool> = None，使用0表示None
        data.push(0);

        Ok(data)
    }

    /// 构建DecreaseLiquidityV2指令
    pub fn build_decrease_liquidity_instructions(
        pool_address: &Pubkey,
        pool_state: &raydium_amm_v3::states::PoolState,
        user_wallet: &Pubkey,
        nft_mint: &Pubkey,
        nft_token_account: &Pubkey,
        user_token_account_0: &Pubkey,
        user_token_account_1: &Pubkey,
        tick_lower_index: i32,
        tick_upper_index: i32,
        tick_array_lower_start_index: i32,
        tick_array_upper_start_index: i32,
        liquidity: u128,
        amount_0_min: u64,
        amount_1_min: u64,
        remaining_accounts: Vec<AccountMeta>,
    ) -> Result<Vec<Instruction>> {
        info!("🔨 构建DecreaseLiquidityV2指令");

        let raydium_program_id = ConfigManager::get_raydium_program_id()?;
        let mut instructions = Vec::new();

        // 1. 计算所有需要的PDA地址
        let (personal_position, _) = Pubkey::find_program_address(
            &[POSITION_SEED.as_bytes(), nft_mint.as_ref()],
            &raydium_program_id,
        );

        let (protocol_position, _) = Pubkey::find_program_address(
            &[
                POSITION_SEED.as_bytes(),
                pool_address.as_ref(),
                &tick_lower_index.to_be_bytes(),
                &tick_upper_index.to_be_bytes(),
            ],
            &raydium_program_id,
        );

        let (tick_array_lower, _) = Pubkey::find_program_address(
            &[
                TICK_ARRAY_SEED.as_bytes(),
                pool_address.as_ref(),
                &tick_array_lower_start_index.to_be_bytes(),
            ],
            &raydium_program_id,
        );

        let (tick_array_upper, _) = Pubkey::find_program_address(
            &[
                TICK_ARRAY_SEED.as_bytes(),
                pool_address.as_ref(),
                &tick_array_upper_start_index.to_be_bytes(),
            ],
            &raydium_program_id,
        );

        // 打印指令构建需要的账户
        info!("user_wallet: {:?}", user_wallet);
        info!("nft_token_account: {:?}", nft_token_account);
        info!("personal_position: {:?}", personal_position);
        info!("pool_address: {:?}", pool_address);
        info!("protocol_position: {:?}", protocol_position);
        info!("pool_state.token_vault_0: {:?}", pool_state.token_vault_0);
        info!("pool_state.token_vault_1: {:?}", pool_state.token_vault_1);
        info!("tick_array_lower: {:?}", tick_array_lower);
        info!("tick_array_upper: {:?}", tick_array_upper);
        info!("user_token_account_0: {:?}", user_token_account_0);
        info!("user_token_account_1: {:?}", user_token_account_1);
        info!("token_program: {:?}", spl_token::id());
        info!("token_program_2022: {:?}", spl_token_2022::id());
        info!("memo_program: {:?}", spl_memo::id());
        info!("pool_state.token_mint_0: {:?}", pool_state.token_mint_0);
        info!("pool_state.token_mint_1: {:?}", pool_state.token_mint_1);

        // 2. 构建账户列表
        let mut accounts = vec![
            AccountMeta::new(*user_wallet, true), // nft_owner
            AccountMeta::new(*nft_token_account, false), // nft_account
            AccountMeta::new(personal_position, false), // personal_position
            AccountMeta::new(*pool_address, false), // pool_state
            AccountMeta::new(protocol_position, false), // protocol_position
            AccountMeta::new(pool_state.token_vault_0, false), // token_vault_0
            AccountMeta::new(pool_state.token_vault_1, false), // token_vault_1
            AccountMeta::new(tick_array_lower, false), // tick_array_lower
            AccountMeta::new(tick_array_upper, false), // tick_array_upper
            AccountMeta::new(*user_token_account_0, false), // recipient_token_account_0
            AccountMeta::new(*user_token_account_1, false), // recipient_token_account_1
            AccountMeta::new_readonly(spl_token::id(), false), // token_program
            AccountMeta::new_readonly(spl_token_2022::id(), false), // token_program_2022
            AccountMeta::new_readonly(spl_memo::id(), false), // memo_program
            AccountMeta::new_readonly(pool_state.token_mint_0, false), // vault_0_mint
            AccountMeta::new_readonly(pool_state.token_mint_1, false), // vault_1_mint
        ];

        // 添加remaining accounts
        accounts.extend(remaining_accounts);

        // 3. 构建指令数据
        let instruction_data = Self::build_decrease_liquidity_instruction_data(
            liquidity,
            amount_0_min,
            amount_1_min,
        )?;

        // 4. 创建DecreaseLiquidityV2指令
        let decrease_liquidity_instruction = Instruction {
            program_id: raydium_program_id,
            accounts,
            data: instruction_data,
        };

        instructions.push(decrease_liquidity_instruction);

        info!("✅ DecreaseLiquidityV2指令构建完成");
        Ok(instructions)
    }

    /// 构建DecreaseLiquidityV2指令数据
    fn build_decrease_liquidity_instruction_data(
        liquidity: u128,
        amount_0_min: u64,
        amount_1_min: u64,
    ) -> Result<Vec<u8>> {
        let mut data = Vec::new();

        // 使用预定义的discriminator常量 - DecreaseLiquidityV2指令
        let discriminator = instruction::DecreaseLiquidityV2::DISCRIMINATOR;
        data.extend_from_slice(&discriminator);

        // 参数序列化（按照Anchor的格式）
        data.extend_from_slice(&liquidity.to_le_bytes());
        data.extend_from_slice(&amount_0_min.to_le_bytes());
        data.extend_from_slice(&amount_1_min.to_le_bytes());

        Ok(data)
    }

    /// 构建ClosePosition指令
    pub fn build_close_position_instructions(
        nft_mint: &Pubkey,
        nft_token_account: &Pubkey,
        nft_token_program: &Pubkey,
        user_wallet: &Pubkey,
    ) -> Result<Vec<Instruction>> {
        info!("🔨 构建ClosePosition指令");

        let raydium_program_id = ConfigManager::get_raydium_program_id()?;
        let mut instructions = Vec::new();

        // 1. 计算personal position PDA
        let (personal_position, _) = Pubkey::find_program_address(
            &[POSITION_SEED.as_bytes(), nft_mint.as_ref()],
            &raydium_program_id,
        );

        // 2. 构建账户列表
        let accounts = vec![
            AccountMeta::new(*user_wallet, true), // nft_owner
            AccountMeta::new(*nft_mint, false), // position_nft_mint
            AccountMeta::new(*nft_token_account, false), // position_nft_account
            AccountMeta::new(personal_position, false), // personal_position
            AccountMeta::new_readonly(system_program::id(), false), // system_program
            AccountMeta::new_readonly(*nft_token_program, false), // token_program
        ];

        // 3. 构建指令数据
        let instruction_data = Self::build_close_position_instruction_data()?;

        // 4. 创建ClosePosition指令
        let close_position_instruction = Instruction {
            program_id: raydium_program_id,
            accounts,
            data: instruction_data,
        };

        instructions.push(close_position_instruction);

        info!("✅ ClosePosition指令构建完成");
        Ok(instructions)
    }

    /// 构建ClosePosition指令数据
    fn build_close_position_instruction_data() -> Result<Vec<u8>> {
        let mut data = Vec::new();

        // 使用预定义的discriminator常量 - ClosePosition指令
        let discriminator = instruction::ClosePosition::DISCRIMINATOR;
        data.extend_from_slice(&discriminator);

        // ClosePosition指令没有额外参数

        Ok(data)
    }
}
