use anyhow::Result;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

use super::constants;

/// 配置管理器 - 统一管理配置加载逻辑
pub struct ConfigManager;

impl ConfigManager {
    /// 获取Raydium程序ID
    pub fn get_raydium_program_id() -> Result<Pubkey> {
        let program_id_str = std::env::var("RAYDIUM_PROGRAM_ID").unwrap_or_else(|_| constants::DEFAULT_RAYDIUM_PROGRAM_ID.to_string());
        Pubkey::from_str(&program_id_str).map_err(Into::into)
    }

    /// 获取AMM配置索引
    pub fn get_amm_config_index() -> u16 {
        std::env::var("AMM_CONFIG_INDEX")
            .unwrap_or_else(|_| constants::DEFAULT_AMM_CONFIG_INDEX.to_string())
            .parse()
            .unwrap_or(constants::DEFAULT_AMM_CONFIG_INDEX)
    }
}
