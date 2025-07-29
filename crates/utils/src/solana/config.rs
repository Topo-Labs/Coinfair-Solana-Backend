use anyhow::Result;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

use super::constants;

/// 配置管理器 - 统一管理配置加载逻辑
pub struct ConfigManager;

impl ConfigManager {
    /// 获取Raydium程序ID (V3 CLMM)
    pub fn get_raydium_program_id() -> Result<Pubkey> {
        let program_id_str = std::env::var("RAYDIUM_PROGRAM_ID").unwrap_or_else(|_| constants::DEFAULT_RAYDIUM_PROGRAM_ID.to_string());
        Pubkey::from_str(&program_id_str).map_err(Into::into)
    }

    /// 获取Raydium V2 AMM程序ID (Classic AMM)
    pub fn get_raydium_v2_amm_program_id() -> Result<Pubkey> {
        let program_id_str = std::env::var("RAYDIUM_V2_AMM_PROGRAM_ID").unwrap_or_else(|_| constants::DEFAULT_RAYDIUM_V2_AMM_PROGRAM_ID.to_string());
        Pubkey::from_str(&program_id_str).map_err(Into::into)
    }

    /// 获取Raydium V3 AMM程序ID (CLMM)
    pub fn get_raydium_v3_program_id() -> Result<Pubkey> {
        let program_id_str = std::env::var("RAYDIUM_V3_PROGRAM").unwrap_or_else(|_| constants::DEFAULT_RAYDIUM_PROGRAM_ID.to_string());
        Pubkey::from_str(&program_id_str).map_err(Into::into)
    }

    /// 获取AMM配置索引
    pub fn get_amm_config_index() -> u16 {
        std::env::var("AMM_CONFIG_INDEX")
            .unwrap_or_else(|_| constants::DEFAULT_AMM_CONFIG_INDEX.to_string())
            .parse()
            .unwrap_or(constants::DEFAULT_AMM_CONFIG_INDEX)
    }

    /// 获取V2 AMM默认开放时间
    pub fn get_v2_amm_open_time() -> u64 {
        std::env::var("V2_AMM_OPEN_TIME")
            .unwrap_or_else(|_| constants::DEFAULT_V2_AMM_OPEN_TIME.to_string())
            .parse()
            .unwrap_or(constants::DEFAULT_V2_AMM_OPEN_TIME)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_get_raydium_v2_amm_program_id_default() {
        // Clear environment variable to test default
        env::remove_var("RAYDIUM_V2_AMM_PROGRAM_ID");

        let program_id = ConfigManager::get_raydium_v2_amm_program_id().unwrap();
        let expected = Pubkey::from_str(constants::DEFAULT_RAYDIUM_V2_AMM_PROGRAM_ID).unwrap();

        assert_eq!(program_id, expected);
    }

    #[test]
    fn test_get_raydium_v2_amm_program_id_from_env() {
        let test_program_id = "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8";
        env::set_var("RAYDIUM_V2_AMM_PROGRAM_ID", test_program_id);

        let program_id = ConfigManager::get_raydium_v2_amm_program_id().unwrap();
        let expected = Pubkey::from_str(test_program_id).unwrap();

        assert_eq!(program_id, expected);

        // Clean up
        env::remove_var("RAYDIUM_V2_AMM_PROGRAM_ID");
    }

    #[test]
    fn test_get_v2_amm_open_time_default() {
        // Clear environment variable to test default
        env::remove_var("V2_AMM_OPEN_TIME");

        let open_time = ConfigManager::get_v2_amm_open_time();

        assert_eq!(open_time, constants::DEFAULT_V2_AMM_OPEN_TIME);
    }

    #[test]
    fn test_get_v2_amm_open_time_from_env() {
        let test_open_time = "1234567890";
        env::set_var("V2_AMM_OPEN_TIME", test_open_time);

        let open_time = ConfigManager::get_v2_amm_open_time();

        assert_eq!(open_time, 1234567890);

        // Clean up
        env::remove_var("V2_AMM_OPEN_TIME");
    }
}
