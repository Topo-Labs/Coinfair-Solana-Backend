use anchor_lang::Discriminator;
use anyhow::Result;
use solana_sdk::{instruction::Instruction, pubkey::Pubkey, system_program, sysvar};
use tracing::info;

use super::{calculators::PDACalculator, config::ConfigManager};
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
