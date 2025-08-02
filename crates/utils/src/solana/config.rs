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

    /// 获取管理员密钥
    /// 从环境变量 ADMIN_PRIVATE_KEY 或 PRIVATE_KEY 读取私钥
    pub fn get_admin_keypair() -> Result<solana_sdk::signature::Keypair> {
        use solana_sdk::bs58;
        use solana_sdk::signature::Keypair;

        // 尝试从多个环境变量获取私钥
        let private_key_str = std::env::var("ADMIN_PRIVATE_KEY")
            .or_else(|_| std::env::var("PRIVATE_KEY"))
            .map_err(|_| anyhow::anyhow!("未找到管理员私钥，请设置 ADMIN_PRIVATE_KEY 或 PRIVATE_KEY 环境变量"))?;

        // 支持多种私钥格式
        let keypair = if private_key_str.starts_with('[') && private_key_str.ends_with(']') {
            // JSON 数组格式 [1,2,3,...]
            let bytes: Vec<u8> = serde_json::from_str(&private_key_str).map_err(|e| anyhow::anyhow!("解析私钥JSON格式失败: {}", e))?;
            if bytes.len() != 64 {
                return Err(anyhow::anyhow!("私钥长度必须是64字节"));
            }
            Keypair::from_bytes(&bytes)?
        } else {
            // Base58 格式
            let bytes = bs58::decode(&private_key_str).into_vec().map_err(|e| anyhow::anyhow!("解码Base58私钥失败: {}", e))?;
            if bytes.len() != 64 {
                return Err(anyhow::anyhow!("私钥长度必须是64字节"));
            }
            Keypair::from_bytes(&bytes)?
        };

        Ok(keypair)
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
