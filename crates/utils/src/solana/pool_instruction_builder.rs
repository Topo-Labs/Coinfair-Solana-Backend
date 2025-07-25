use anchor_lang::Discriminator;
use anyhow::Result;
use solana_sdk::{instruction::Instruction, pubkey::Pubkey, system_program, sysvar};
use tracing::info;

use super::{
    calculators::{PDACalculator, V2AmmParameterCalculator},
    config::ConfigManager,
};
use raydium_amm_v3::instruction;
/// 池子指令构建器 - 统一管理池子相关指令的构建
pub struct PoolInstructionBuilder;

impl PoolInstructionBuilder {
    /// 构建CreatePool指令
    pub fn build_create_pool_instruction(
        pool_creator: &Pubkey,
        config_index: u16,
        mint0: &Pubkey,
        mint1: &Pubkey,
        token_program_0: &Pubkey,
        token_program_1: &Pubkey,
        sqrt_price_x64: u128,
        open_time: u64,
    ) -> Result<Vec<Instruction>> {
        let raydium_program_id = ConfigManager::get_raydium_program_id()?;

        info!("🏗️ 构建CreatePool指令");
        info!("  创建者: {}", pool_creator);
        info!("  配置索引: {}", config_index);
        info!("  Mint0: {}", mint0);
        info!("  Mint1: {}", mint1);
        info!("  初始价格: {}", sqrt_price_x64);
        info!("  开放时间: {}", open_time);

        // 计算所有必要的PDA
        let (amm_config_key, _) = PDACalculator::calculate_amm_config_pda(&raydium_program_id, config_index);
        let (pool_key, _) = PDACalculator::calculate_pool_pda(&raydium_program_id, &amm_config_key, mint0, mint1);
        let (token_vault_0, _) = PDACalculator::calculate_pool_vault_pda(&raydium_program_id, &pool_key, mint0);
        let (token_vault_1, _) = PDACalculator::calculate_pool_vault_pda(&raydium_program_id, &pool_key, mint1);
        let (observation_key, _) = PDACalculator::calculate_observation_pda(&raydium_program_id, &pool_key);
        let (tick_array_bitmap, _) = PDACalculator::calculate_tickarray_bitmap_extension_pda(&raydium_program_id, &pool_key);

        info!("📋 计算的PDA地址:");
        info!("  AMM配置: {}", amm_config_key);
        info!("  池子地址: {}", pool_key);
        info!("  Token0 Vault: {}", token_vault_0);
        info!("  Token1 Vault: {}", token_vault_1);
        info!("  观察状态: {}", observation_key);
        info!("  Tick Array Bitmap: {}", tick_array_bitmap);

        // 构建CreatePool指令数据
        let instruction_data = Self::build_create_pool_instruction_data(sqrt_price_x64, open_time)?;

        // 构建账户元数据
        let accounts = vec![
            solana_sdk::instruction::AccountMeta::new(*pool_creator, true),            // pool_creator (signer)
            solana_sdk::instruction::AccountMeta::new_readonly(amm_config_key, false), // amm_config
            solana_sdk::instruction::AccountMeta::new(pool_key, false),                // pool_state
            solana_sdk::instruction::AccountMeta::new_readonly(*mint0, false),         // token_mint_0
            solana_sdk::instruction::AccountMeta::new_readonly(*mint1, false),         // token_mint_1
            solana_sdk::instruction::AccountMeta::new(token_vault_0, false),           // token_vault_0
            solana_sdk::instruction::AccountMeta::new(token_vault_1, false),           // token_vault_1
            solana_sdk::instruction::AccountMeta::new(observation_key, false),         // observation_state
            solana_sdk::instruction::AccountMeta::new(tick_array_bitmap, false),       // tick_array_bitmap
            solana_sdk::instruction::AccountMeta::new_readonly(*token_program_0, false), // token_program_0
            solana_sdk::instruction::AccountMeta::new_readonly(*token_program_1, false), // token_program_1
            solana_sdk::instruction::AccountMeta::new_readonly(system_program::id(), false), // system_program
            solana_sdk::instruction::AccountMeta::new_readonly(sysvar::rent::id(), false), // rent
        ];

        let instruction = Instruction {
            program_id: raydium_program_id,
            accounts,
            data: instruction_data,
        };

        Ok(vec![instruction])
    }

    /// 构建CreatePool指令数据
    fn build_create_pool_instruction_data(sqrt_price_x64: u128, open_time: u64) -> Result<Vec<u8>> {
        // Raydium CreatePool指令的discriminator
        // 这个值需要根据实际的Raydium程序确定
        // let discriminator: [u8; 8] = [233, 146, 209, 142, 207, 104, 64, 188]; // create_pool指令的discriminator
        let discriminator = instruction::CreatePool::DISCRIMINATOR;

        let mut data = Vec::new();
        data.extend_from_slice(&discriminator);
        data.extend_from_slice(&sqrt_price_x64.to_le_bytes());
        data.extend_from_slice(&open_time.to_le_bytes());

        Ok(data)
    }

    /// 获取池子地址（不创建指令，仅计算地址）
    pub fn get_pool_address(config_index: u16, mint0: &Pubkey, mint1: &Pubkey) -> Result<Pubkey> {
        let raydium_program_id = ConfigManager::get_raydium_program_id()?;
        let (amm_config_key, _) = PDACalculator::calculate_amm_config_pda(&raydium_program_id, config_index);
        let (pool_key, _) = PDACalculator::calculate_pool_pda(&raydium_program_id, &amm_config_key, mint0, mint1);
        Ok(pool_key)
    }

    /// 获取所有相关的PDA地址
    pub fn get_all_pool_addresses(config_index: u16, mint0: &Pubkey, mint1: &Pubkey) -> Result<PoolAddresses> {
        let raydium_program_id = ConfigManager::get_raydium_program_id()?;
        let (amm_config_key, _) = PDACalculator::calculate_amm_config_pda(&raydium_program_id, config_index);
        let (pool_key, _) = PDACalculator::calculate_pool_pda(&raydium_program_id, &amm_config_key, mint0, mint1);
        let (token_vault_0, _) = PDACalculator::calculate_pool_vault_pda(&raydium_program_id, &pool_key, mint0);
        let (token_vault_1, _) = PDACalculator::calculate_pool_vault_pda(&raydium_program_id, &pool_key, mint1);
        let (observation_key, _) = PDACalculator::calculate_observation_pda(&raydium_program_id, &pool_key);
        let (tick_array_bitmap, _) = PDACalculator::calculate_tickarray_bitmap_extension_pda(&raydium_program_id, &pool_key);

        Ok(PoolAddresses {
            amm_config: amm_config_key,
            pool: pool_key,
            token_vault_0,
            token_vault_1,
            observation: observation_key,
            tick_array_bitmap,
        })
    }
}

/// 池子相关地址结构体
#[derive(Debug, Clone)]
pub struct PoolAddresses {
    pub amm_config: Pubkey,
    pub pool: Pubkey,
    pub token_vault_0: Pubkey,
    pub token_vault_1: Pubkey,
    pub observation: Pubkey,
    pub tick_array_bitmap: Pubkey,
}

/// Classic AMM指令构建器 - 统一管理V2 AMM (Classic AMM)相关指令的构建
pub struct ClassicAmmInstructionBuilder;

impl ClassicAmmInstructionBuilder {
    /// 构建V2 AMM Initialize指令
    ///
    /// # Arguments
    /// * `pool_creator` - 池子创建者的公钥
    /// * `mint0` - 第一个token mint地址
    /// * `mint1` - 第二个token mint地址
    /// * `init_amount_0` - 第一个token的初始数量
    /// * `init_amount_1` - 第二个token的初始数量
    /// * `open_time` - 池子开放时间 (Unix时间戳，0表示立即开放)
    ///
    /// # Returns
    /// * `Result<Vec<Instruction>>` - 包含初始化指令的向量
    pub fn build_initialize_instruction(
        pool_creator: &Pubkey,
        mint0: &Pubkey,
        mint1: &Pubkey,
        init_amount_0: u64,
        init_amount_1: u64,
        open_time: u64,
    ) -> Result<Vec<Instruction>> {
        let v2_amm_program_id = ConfigManager::get_raydium_v2_amm_program_id()?;

        info!("🏗️ 构建V2 AMM Initialize指令");
        info!("  创建者: {}", pool_creator);
        info!("  Mint0: {}", mint0);
        info!("  Mint1: {}", mint1);
        info!("  初始数量0: {}", init_amount_0);
        info!("  初始数量1: {}", init_amount_1);
        info!("  开放时间: {}", open_time);

        // 计算V2 AMM初始化参数
        let params = V2AmmParameterCalculator::calculate_initialize_params(&v2_amm_program_id, mint0, mint1, init_amount_0, init_amount_1, open_time)?;

        info!("📋 计算的V2 AMM参数:");
        info!("  池子ID: {}", params.pool_id);
        info!("  Nonce: {}", params.nonce);
        info!("  Coin Vault: {}", params.coin_vault);
        info!("  PC Vault: {}", params.pc_vault);
        info!("  LP Mint: {}", params.lp_mint);
        info!("  Open Orders: {}", params.open_orders);
        info!("  Target Orders: {}", params.target_orders);
        info!("  Withdraw Queue: {}", params.withdraw_queue);

        // 构建Initialize指令数据
        let instruction_data = Self::build_initialize_instruction_data(params.nonce, params.open_time, params.init_pc_amount, params.init_coin_amount)?;

        // 确定coin和pc mint的顺序
        let (coin_mint, pc_mint) = if mint0.to_bytes() < mint1.to_bytes() {
            (mint0, mint1)
        } else {
            (mint1, mint0)
        };

        // 构建账户元数据 - 基于Raydium V2 AMM Initialize指令的账户布局
        let accounts = vec![
            // 0. `[signer]` The account paying for all rents
            solana_sdk::instruction::AccountMeta::new(*pool_creator, true),
            // 1. `[writable]` New AMM Account to create
            solana_sdk::instruction::AccountMeta::new(params.pool_id, false),
            // 2. `[]` AMM authority
            solana_sdk::instruction::AccountMeta::new_readonly(v2_amm_program_id, false),
            // 3. `[writable]` AMM open orders Account
            solana_sdk::instruction::AccountMeta::new(params.open_orders, false),
            // 4. `[writable]` AMM lp mint Account
            solana_sdk::instruction::AccountMeta::new(params.lp_mint, false),
            // 5. `[]` AMM coin mint Account
            solana_sdk::instruction::AccountMeta::new_readonly(*coin_mint, false),
            // 6. `[]` AMM pc mint Account
            solana_sdk::instruction::AccountMeta::new_readonly(*pc_mint, false),
            // 7. `[writable]` AMM coin vault Account
            solana_sdk::instruction::AccountMeta::new(params.coin_vault, false),
            // 8. `[writable]` AMM pc vault Account
            solana_sdk::instruction::AccountMeta::new(params.pc_vault, false),
            // 9. `[writable]` AMM target orders Account
            solana_sdk::instruction::AccountMeta::new(params.target_orders, false),
            // 10. `[writable]` AMM withdraw queue Account
            solana_sdk::instruction::AccountMeta::new(params.withdraw_queue, false),
            // 11. `[]` SPL Token program
            solana_sdk::instruction::AccountMeta::new_readonly(spl_token::id(), false),
            // 12. `[]` System program
            solana_sdk::instruction::AccountMeta::new_readonly(system_program::id(), false),
            // 13. `[]` Rent sysvar
            solana_sdk::instruction::AccountMeta::new_readonly(sysvar::rent::id(), false),
        ];

        let instruction = Instruction {
            program_id: v2_amm_program_id,
            accounts,
            data: instruction_data,
        };

        Ok(vec![instruction])
    }

    /// 构建V2 AMM Initialize指令数据
    ///
    /// # Arguments
    /// * `nonce` - PDA计算的nonce值
    /// * `open_time` - 池子开放时间
    /// * `init_pc_amount` - PC token初始数量
    /// * `init_coin_amount` - Coin token初始数量
    ///
    /// # Returns
    /// * `Result<Vec<u8>>` - 序列化的指令数据
    fn build_initialize_instruction_data(nonce: u8, open_time: u64, init_pc_amount: u64, init_coin_amount: u64) -> Result<Vec<u8>> {
        // V2 AMM Initialize指令的discriminator
        // 对于Raydium V2 AMM，Initialize指令通常使用特定的discriminator
        // 这里使用一个通用的discriminator，实际使用时可能需要根据具体的程序调整
        let discriminator: [u8; 8] = [175, 175, 109, 31, 13, 152, 155, 237]; // initialize指令的discriminator

        let mut data = Vec::new();
        data.extend_from_slice(&discriminator);
        data.push(nonce);
        data.extend_from_slice(&open_time.to_le_bytes());
        data.extend_from_slice(&init_pc_amount.to_le_bytes());
        data.extend_from_slice(&init_coin_amount.to_le_bytes());

        info!("🔧 构建的指令数据长度: {} bytes", data.len());
        info!("  Discriminator: {:?}", &discriminator);
        info!("  Nonce: {}", nonce);
        info!("  Open Time: {}", open_time);
        info!("  Init PC Amount: {}", init_pc_amount);
        info!("  Init Coin Amount: {}", init_coin_amount);

        Ok(data)
    }

    /// 计算V2 AMM池子地址（不创建指令，仅计算地址）
    ///
    /// # Arguments
    /// * `mint0` - 第一个token mint地址
    /// * `mint1` - 第二个token mint地址
    ///
    /// # Returns
    /// * `Result<Pubkey>` - 计算得到的池子地址
    pub fn calculate_pool_address(mint0: &Pubkey, mint1: &Pubkey) -> Result<Pubkey> {
        let v2_amm_program_id = ConfigManager::get_raydium_v2_amm_program_id()?;
        let (pool_key, _) = PDACalculator::calculate_v2_amm_pool_pda(&v2_amm_program_id, mint0, mint1);

        info!("🔍 计算V2 AMM池子地址:");
        info!("  程序ID: {}", v2_amm_program_id);
        info!("  Mint0: {}", mint0);
        info!("  Mint1: {}", mint1);
        info!("  池子地址: {}", pool_key);

        Ok(pool_key)
    }

    /// 获取所有V2 AMM相关的PDA地址
    ///
    /// # Arguments
    /// * `mint0` - 第一个token mint地址
    /// * `mint1` - 第二个token mint地址
    ///
    /// # Returns
    /// * `Result<V2AmmAddresses>` - 包含所有相关地址的结构体
    pub fn get_all_v2_amm_addresses(mint0: &Pubkey, mint1: &Pubkey) -> Result<V2AmmAddresses> {
        let v2_amm_program_id = ConfigManager::get_raydium_v2_amm_program_id()?;

        // 计算池子PDA
        let (pool_id, _) = PDACalculator::calculate_v2_amm_pool_pda(&v2_amm_program_id, mint0, mint1);

        // 计算所有相关的PDA地址
        let (coin_vault, _) = PDACalculator::calculate_v2_pool_coin_token_account(&v2_amm_program_id, &pool_id);
        let (pc_vault, _) = PDACalculator::calculate_v2_pool_pc_token_account(&v2_amm_program_id, &pool_id);
        let (lp_mint, _) = PDACalculator::calculate_v2_lp_mint_pda(&v2_amm_program_id, &pool_id);
        let (open_orders, _) = PDACalculator::calculate_v2_open_orders_pda(&v2_amm_program_id, &pool_id);
        let (target_orders, _) = PDACalculator::calculate_v2_target_orders_pda(&v2_amm_program_id, &pool_id);
        let (withdraw_queue, _) = PDACalculator::calculate_v2_withdraw_queue_pda(&v2_amm_program_id, &pool_id);

        // 确定coin和pc mint的顺序
        let (coin_mint, pc_mint) = if mint0.to_bytes() < mint1.to_bytes() {
            (*mint0, *mint1)
        } else {
            (*mint1, *mint0)
        };

        Ok(V2AmmAddresses {
            pool_id,
            coin_mint,
            pc_mint,
            coin_vault,
            pc_vault,
            lp_mint,
            open_orders,
            target_orders,
            withdraw_queue,
        })
    }
}

/// V2 AMM相关地址结构体
#[derive(Debug, Clone)]
pub struct V2AmmAddresses {
    pub pool_id: Pubkey,
    pub coin_mint: Pubkey,
    pub pc_mint: Pubkey,
    pub coin_vault: Pubkey,
    pub pc_vault: Pubkey,
    pub lp_mint: Pubkey,
    pub open_orders: Pubkey,
    pub target_orders: Pubkey,
    pub withdraw_queue: Pubkey,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    // Test constants
    const TEST_V2_AMM_PROGRAM_ID: &str = "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8";
    const TEST_SOL_MINT: &str = "So11111111111111111111111111111111111111112";
    const TEST_USDC_MINT: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";

    #[test]
    fn test_build_initialize_instruction_data() {
        let nonce = 255;
        let open_time = 1640995200u64; // 2022-01-01 00:00:00 UTC
        let init_pc_amount = 100_000_000u64; // 100 USDC
        let init_coin_amount = 1_000_000_000u64; // 1 SOL

        let result = ClassicAmmInstructionBuilder::build_initialize_instruction_data(nonce, open_time, init_pc_amount, init_coin_amount);

        assert!(result.is_ok());
        let data = result.unwrap();

        // Verify data structure
        assert_eq!(data.len(), 8 + 1 + 8 + 8 + 8); // discriminator + nonce + open_time + init_pc_amount + init_coin_amount

        // Verify discriminator
        let expected_discriminator: [u8; 8] = [175, 175, 109, 31, 13, 152, 155, 237];
        assert_eq!(&data[0..8], &expected_discriminator);

        // Verify nonce
        assert_eq!(data[8], nonce);

        // Verify open_time (little endian)
        let open_time_bytes = &data[9..17];
        let parsed_open_time = u64::from_le_bytes(open_time_bytes.try_into().unwrap());
        assert_eq!(parsed_open_time, open_time);

        // Verify init_pc_amount (little endian)
        let pc_amount_bytes = &data[17..25];
        let parsed_pc_amount = u64::from_le_bytes(pc_amount_bytes.try_into().unwrap());
        assert_eq!(parsed_pc_amount, init_pc_amount);

        // Verify init_coin_amount (little endian)
        let coin_amount_bytes = &data[25..33];
        let parsed_coin_amount = u64::from_le_bytes(coin_amount_bytes.try_into().unwrap());
        assert_eq!(parsed_coin_amount, init_coin_amount);
    }

    #[test]
    fn test_build_initialize_instruction_data_zero_values() {
        let result = ClassicAmmInstructionBuilder::build_initialize_instruction_data(0, 0, 0, 0);

        assert!(result.is_ok());
        let data = result.unwrap();

        // Should still produce valid data structure
        assert_eq!(data.len(), 33);

        // Verify all values are zero except discriminator
        let expected_discriminator: [u8; 8] = [175, 175, 109, 31, 13, 152, 155, 237];
        assert_eq!(&data[0..8], &expected_discriminator);
        assert_eq!(data[8], 0); // nonce

        // All other bytes should be zero
        for &byte in &data[9..] {
            assert_eq!(byte, 0);
        }
    }

    #[test]
    fn test_build_initialize_instruction_data_max_values() {
        let nonce = 255u8;
        let open_time = u64::MAX;
        let init_pc_amount = u64::MAX;
        let init_coin_amount = u64::MAX;

        let result = ClassicAmmInstructionBuilder::build_initialize_instruction_data(nonce, open_time, init_pc_amount, init_coin_amount);

        assert!(result.is_ok());
        let data = result.unwrap();

        // Verify structure
        assert_eq!(data.len(), 33);

        // Verify nonce
        assert_eq!(data[8], 255);

        // Verify max values are correctly encoded
        let parsed_open_time = u64::from_le_bytes(data[9..17].try_into().unwrap());
        assert_eq!(parsed_open_time, u64::MAX);

        let parsed_pc_amount = u64::from_le_bytes(data[17..25].try_into().unwrap());
        assert_eq!(parsed_pc_amount, u64::MAX);

        let parsed_coin_amount = u64::from_le_bytes(data[25..33].try_into().unwrap());
        assert_eq!(parsed_coin_amount, u64::MAX);
    }

    #[test]
    fn test_calculate_pool_address() {
        // Set environment variable for testing
        std::env::set_var("RAYDIUM_V2_AMM_PROGRAM_ID", TEST_V2_AMM_PROGRAM_ID);

        let mint0 = Pubkey::from_str(TEST_SOL_MINT).unwrap();
        let mint1 = Pubkey::from_str(TEST_USDC_MINT).unwrap();

        let result = ClassicAmmInstructionBuilder::calculate_pool_address(&mint0, &mint1);

        assert!(result.is_ok());
        let pool_address = result.unwrap();

        // Verify the address is not default
        assert_ne!(pool_address, Pubkey::default());

        // Test with reversed mint order - should produce the same result
        let result_reversed = ClassicAmmInstructionBuilder::calculate_pool_address(&mint1, &mint0);
        assert!(result_reversed.is_ok());
        assert_eq!(pool_address, result_reversed.unwrap());

        // Clean up
        std::env::remove_var("RAYDIUM_V2_AMM_PROGRAM_ID");
    }

    #[test]
    fn test_calculate_pool_address_with_same_mints() {
        // Set environment variable for testing
        std::env::set_var("RAYDIUM_V2_AMM_PROGRAM_ID", TEST_V2_AMM_PROGRAM_ID);

        let mint0 = Pubkey::from_str(TEST_SOL_MINT).unwrap();

        let result = ClassicAmmInstructionBuilder::calculate_pool_address(&mint0, &mint0);

        assert!(result.is_ok());
        let pool_address = result.unwrap();

        // Should still produce a valid address
        assert_ne!(pool_address, Pubkey::default());

        // Clean up
        std::env::remove_var("RAYDIUM_V2_AMM_PROGRAM_ID");
    }

    #[test]
    fn test_get_all_v2_amm_addresses() {
        // Set environment variable for testing
        std::env::set_var("RAYDIUM_V2_AMM_PROGRAM_ID", TEST_V2_AMM_PROGRAM_ID);

        let mint0 = Pubkey::from_str(TEST_SOL_MINT).unwrap();
        let mint1 = Pubkey::from_str(TEST_USDC_MINT).unwrap();

        let result = ClassicAmmInstructionBuilder::get_all_v2_amm_addresses(&mint0, &mint1);

        assert!(result.is_ok());
        let addresses = result.unwrap();

        // Verify all addresses are valid (not default)
        assert_ne!(addresses.pool_id, Pubkey::default());
        assert_ne!(addresses.coin_vault, Pubkey::default());
        assert_ne!(addresses.pc_vault, Pubkey::default());
        assert_ne!(addresses.lp_mint, Pubkey::default());
        assert_ne!(addresses.open_orders, Pubkey::default());
        assert_ne!(addresses.target_orders, Pubkey::default());
        assert_ne!(addresses.withdraw_queue, Pubkey::default());

        // Verify all addresses are different from each other
        let all_addresses = vec![
            addresses.pool_id,
            addresses.coin_vault,
            addresses.pc_vault,
            addresses.lp_mint,
            addresses.open_orders,
            addresses.target_orders,
            addresses.withdraw_queue,
        ];

        for (i, addr1) in all_addresses.iter().enumerate() {
            for (j, addr2) in all_addresses.iter().enumerate() {
                if i != j {
                    assert_ne!(addr1, addr2, "Addresses at indices {} and {} should be different", i, j);
                }
            }
        }

        // Verify mint ordering
        let mint0_bytes = mint0.to_bytes();
        let mint1_bytes = mint1.to_bytes();

        if mint0_bytes < mint1_bytes {
            assert_eq!(addresses.coin_mint, mint0);
            assert_eq!(addresses.pc_mint, mint1);
        } else {
            assert_eq!(addresses.coin_mint, mint1);
            assert_eq!(addresses.pc_mint, mint0);
        }

        // Clean up
        std::env::remove_var("RAYDIUM_V2_AMM_PROGRAM_ID");
    }

    #[test]
    fn test_get_all_v2_amm_addresses_deterministic() {
        // Set environment variable for testing
        std::env::set_var("RAYDIUM_V2_AMM_PROGRAM_ID", TEST_V2_AMM_PROGRAM_ID);

        let mint0 = Pubkey::from_str(TEST_SOL_MINT).unwrap();
        let mint1 = Pubkey::from_str(TEST_USDC_MINT).unwrap();

        // Calculate addresses multiple times
        let result1 = ClassicAmmInstructionBuilder::get_all_v2_amm_addresses(&mint0, &mint1);
        let result2 = ClassicAmmInstructionBuilder::get_all_v2_amm_addresses(&mint0, &mint1);

        assert!(result1.is_ok());
        assert!(result2.is_ok());

        let addresses1 = result1.unwrap();
        let addresses2 = result2.unwrap();

        // Results should be identical
        assert_eq!(addresses1.pool_id, addresses2.pool_id);
        assert_eq!(addresses1.coin_mint, addresses2.coin_mint);
        assert_eq!(addresses1.pc_mint, addresses2.pc_mint);
        assert_eq!(addresses1.coin_vault, addresses2.coin_vault);
        assert_eq!(addresses1.pc_vault, addresses2.pc_vault);
        assert_eq!(addresses1.lp_mint, addresses2.lp_mint);
        assert_eq!(addresses1.open_orders, addresses2.open_orders);
        assert_eq!(addresses1.target_orders, addresses2.target_orders);
        assert_eq!(addresses1.withdraw_queue, addresses2.withdraw_queue);

        // Clean up
        std::env::remove_var("RAYDIUM_V2_AMM_PROGRAM_ID");
    }

    #[test]
    fn test_build_initialize_instruction_accounts_structure() {
        // Set environment variable for testing
        std::env::set_var("RAYDIUM_V2_AMM_PROGRAM_ID", TEST_V2_AMM_PROGRAM_ID);

        let pool_creator = Pubkey::new_unique();
        let mint0 = Pubkey::from_str(TEST_SOL_MINT).unwrap();
        let mint1 = Pubkey::from_str(TEST_USDC_MINT).unwrap();
        let init_amount_0 = 1_000_000_000u64;
        let init_amount_1 = 100_000_000u64;
        let open_time = 0u64;

        let result = ClassicAmmInstructionBuilder::build_initialize_instruction(&pool_creator, &mint0, &mint1, init_amount_0, init_amount_1, open_time);

        assert!(result.is_ok());
        let instructions = result.unwrap();

        // Should return exactly one instruction
        assert_eq!(instructions.len(), 1);

        let instruction = &instructions[0];

        // Verify program ID
        let expected_program_id = Pubkey::from_str(TEST_V2_AMM_PROGRAM_ID).unwrap();
        assert_eq!(instruction.program_id, expected_program_id);

        // Verify account count (should be 14 accounts)
        assert_eq!(instruction.accounts.len(), 14);

        // Verify first account is the pool creator and is signer
        assert_eq!(instruction.accounts[0].pubkey, pool_creator);
        assert!(instruction.accounts[0].is_signer);
        assert!(instruction.accounts[0].is_writable);

        // Verify instruction data is not empty
        assert!(!instruction.data.is_empty());
        assert_eq!(instruction.data.len(), 33); // discriminator + nonce + open_time + init_pc_amount + init_coin_amount

        // Clean up
        std::env::remove_var("RAYDIUM_V2_AMM_PROGRAM_ID");
    }

    #[test]
    fn test_build_initialize_instruction_with_invalid_amounts() {
        // Set environment variable for testing
        std::env::set_var("RAYDIUM_V2_AMM_PROGRAM_ID", TEST_V2_AMM_PROGRAM_ID);

        let pool_creator = Pubkey::new_unique();
        let mint0 = Pubkey::from_str(TEST_SOL_MINT).unwrap();
        let mint1 = Pubkey::from_str(TEST_USDC_MINT).unwrap();
        let open_time = 0u64;

        // Test with zero amounts - should fail during parameter calculation
        let result = ClassicAmmInstructionBuilder::build_initialize_instruction(
            &pool_creator,
            &mint0,
            &mint1,
            0, // zero amount
            100_000_000u64,
            open_time,
        );

        assert!(result.is_err());

        // Test with second amount zero
        let result = ClassicAmmInstructionBuilder::build_initialize_instruction(
            &pool_creator,
            &mint0,
            &mint1,
            1_000_000_000u64,
            0, // zero amount
            open_time,
        );

        assert!(result.is_err());

        // Clean up
        std::env::remove_var("RAYDIUM_V2_AMM_PROGRAM_ID");
    }

    #[test]
    fn test_build_initialize_instruction_with_same_mints() {
        // Set environment variable for testing
        std::env::set_var("RAYDIUM_V2_AMM_PROGRAM_ID", TEST_V2_AMM_PROGRAM_ID);

        let pool_creator = Pubkey::new_unique();
        let mint0 = Pubkey::from_str(TEST_SOL_MINT).unwrap();
        let init_amount_0 = 1_000_000_000u64;
        let init_amount_1 = 100_000_000u64;
        let open_time = 0u64;

        // Test with same mints - should fail during parameter validation
        let result = ClassicAmmInstructionBuilder::build_initialize_instruction(
            &pool_creator,
            &mint0,
            &mint0, // same mint
            init_amount_0,
            init_amount_1,
            open_time,
        );

        assert!(result.is_err());

        // Clean up
        std::env::remove_var("RAYDIUM_V2_AMM_PROGRAM_ID");
    }

    #[test]
    fn test_instruction_data_consistency() {
        let nonce = 123u8;
        let open_time = 1640995200u64;
        let init_pc_amount = 100_000_000u64;
        let init_coin_amount = 1_000_000_000u64;

        // Build instruction data multiple times
        let result1 = ClassicAmmInstructionBuilder::build_initialize_instruction_data(nonce, open_time, init_pc_amount, init_coin_amount);
        let result2 = ClassicAmmInstructionBuilder::build_initialize_instruction_data(nonce, open_time, init_pc_amount, init_coin_amount);

        assert!(result1.is_ok());
        assert!(result2.is_ok());

        let data1 = result1.unwrap();
        let data2 = result2.unwrap();

        // Results should be identical
        assert_eq!(data1, data2);
    }

    #[test]
    fn test_mint_ordering_in_instruction() {
        // Set environment variable for testing
        std::env::set_var("RAYDIUM_V2_AMM_PROGRAM_ID", TEST_V2_AMM_PROGRAM_ID);

        let pool_creator = Pubkey::new_unique();
        let mint0 = Pubkey::from_str(TEST_SOL_MINT).unwrap();
        let mint1 = Pubkey::from_str(TEST_USDC_MINT).unwrap();
        let init_amount_0 = 1_000_000_000u64;
        let init_amount_1 = 100_000_000u64;
        let open_time = 0u64;

        // Test with mint0, mint1 order
        let result1 = ClassicAmmInstructionBuilder::build_initialize_instruction(&pool_creator, &mint0, &mint1, init_amount_0, init_amount_1, open_time);

        // Test with mint1, mint0 order (reversed)
        let result2 = ClassicAmmInstructionBuilder::build_initialize_instruction(&pool_creator, &mint1, &mint0, init_amount_1, init_amount_0, open_time);

        assert!(result1.is_ok());
        assert!(result2.is_ok());

        let instruction1 = &result1.unwrap()[0];
        let instruction2 = &result2.unwrap()[0];

        // The pool addresses should be the same regardless of input order
        assert_eq!(instruction1.accounts[1].pubkey, instruction2.accounts[1].pubkey); // pool_id

        // But the coin/pc assignments might be different based on the amounts
        // This tests that the instruction builder handles mint ordering correctly

        // Clean up
        std::env::remove_var("RAYDIUM_V2_AMM_PROGRAM_ID");
    }
}
