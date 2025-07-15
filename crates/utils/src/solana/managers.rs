use anyhow::Result;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

use super::{constants, ConfigManager, PDACalculator, TokenUtils};

/// 池子信息管理器 - 统一管理池子相关信息
pub struct PoolInfoManager;

impl PoolInfoManager {
    /// 获取已知池子映射
    pub fn get_known_pools() -> std::collections::HashMap<String, String> {
        let mut pools = std::collections::HashMap::new();

        // SOL相关主要池子
        let sol_mint = constants::SOL_MINT;
        let usdc_mint = constants::USDC_MINT_STANDARD;
        let usdt_mint = "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB";
        let coinfair_mint = "CF1Ms9vjvGEiSHqoj1jLadoLNXD9EqtnR6TZp1w8CeHz";

        // 添加主要交易对
        pools.insert(format!("{}_{}", sol_mint, usdc_mint), "58oQChx4yWmvKdwLLZzBi4ChoCc2fqCUWBkwMihLYQo2".to_string());
        pools.insert(format!("{}_{}", sol_mint, usdt_mint), "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM".to_string());
        pools.insert(format!("{}_{}", usdt_mint, coinfair_mint), "EWsjgXuVrcAESbAyBo6Q2JCuuAdotBhp8g7Qhvf8GNek".to_string());

        pools
    }

    /// 查找池子地址
    pub fn find_pool_address(input_mint: &str, output_mint: &str) -> Option<String> {
        let pool_map = Self::get_known_pools();
        let pair_key1 = format!("{}_{}", input_mint, output_mint);
        let pair_key2 = format!("{}_{}", output_mint, input_mint);

        pool_map.get(&pair_key1).or_else(|| pool_map.get(&pair_key2)).cloned()
    }

    /// 计算池子地址使用PDA
    pub fn calculate_pool_address_pda(input_mint: &str, output_mint: &str) -> Result<String> {
        let raydium_program_id = ConfigManager::get_raydium_program_id()?;
        let amm_config_index = ConfigManager::get_amm_config_index();

        let input_mint_pubkey = Pubkey::from_str(input_mint)?;
        let output_mint_pubkey = Pubkey::from_str(output_mint)?;

        let (mint0, mint1, _) = TokenUtils::normalize_mint_order(&input_mint_pubkey, &output_mint_pubkey);

        let (amm_config_key, _) = PDACalculator::calculate_amm_config_pda(&raydium_program_id, amm_config_index);
        let (pool_id_account, _) = PDACalculator::calculate_pool_pda(&raydium_program_id, &amm_config_key, &mint0, &mint1);

        Ok(pool_id_account.to_string())
    }
}

/// 错误处理工具 - 统一管理错误处理
pub struct ErrorHandler;

impl ErrorHandler {
    /// 创建标准错误
    pub fn create_error(message: &str) -> anyhow::Error {
        anyhow::anyhow!("{}", message)
    }

    /// 处理账户加载错误
    pub fn handle_account_load_error(account_name: &str) -> anyhow::Error {
        Self::create_error(&format!("无法加载{}账户", account_name))
    }

    /// 处理解析错误
    pub fn handle_parse_error(field_name: &str, error: impl std::fmt::Display) -> anyhow::Error {
        Self::create_error(&format!("解析{}失败: {}", field_name, error))
    }

    /// 处理计算错误
    pub fn handle_calculation_error(operation: &str, error: impl std::fmt::Display) -> anyhow::Error {
        Self::create_error(&format!("{}计算失败: {}", operation, error))
    }
}
