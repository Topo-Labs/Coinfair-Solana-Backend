use serde::{Deserialize, Serialize};
use std::fmt;

/// Solana相关常量定义
pub const SOL_MINT: &str = "So11111111111111111111111111111111111111112";
pub const USDC_MINT_STANDARD: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
pub const USDC_MINT_CONFIG: &str = "4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU";
pub const USDC_MINT_ALTERNATIVE: &str = "A9mUU4qviSctJVPJdBJWkb28deg915LYJKrzQ19ji3FM";

// Raydium V3 (CLMM) 常量
pub const DEFAULT_RAYDIUM_PROGRAM_ID: &str = "CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK";
pub const DEFAULT_AMM_CONFIG_INDEX: u16 = 1;
pub const DEFAULT_FEE_RATE: u64 = 400; // 0.25%

// Raydium V2 (Classic AMM) 常量
pub const DEFAULT_RAYDIUM_V2_AMM_PROGRAM_ID: &str = "CPMDWBwJDtYax9qW7AyRuVC19Cc4L4Vcy4n2BHAbHkCW";
pub const DEFAULT_V2_AMM_OPEN_TIME: u64 = 0;

pub const DEFAULT_SOL_PRICE_USDC: f64 = 100.0;

/// Solana 网络链 ID 枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SolanaChainId {
    /// Mainnet Beta 主网
    #[serde(rename = "mainnet")]
    Mainnet = 101,
    /// Testnet 测试网
    #[serde(rename = "testnet")]
    Testnet = 102,
    /// Devnet 开发网
    #[serde(rename = "devnet")]
    Devnet = 103,
}

impl SolanaChainId {
    /// 获取链 ID 数值
    pub fn chain_id(&self) -> u32 {
        *self as u32
    }

    /// 获取网络名称字符串
    pub fn network_name(&self) -> &'static str {
        match self {
            SolanaChainId::Mainnet => "mainnet",
            SolanaChainId::Testnet => "testnet",
            SolanaChainId::Devnet => "devnet",
        }
    }

    /// 从环境变量检测网络类型
    pub fn from_env() -> Self {
        // 优先检查 CARGO_ENV 环境变量
        if let Ok(cargo_env) = std::env::var("CARGO_ENV") {
            return match cargo_env.as_str() {
                "Development" | "development" | "dev" | "devnet" => SolanaChainId::Devnet,
                "Test" | "test" | "testnet" => SolanaChainId::Testnet,
                _ => SolanaChainId::Mainnet,
            };
        }

        // 检查 RPC_URL 环境变量
        if let Ok(rpc_url) = std::env::var("RPC_URL") {
            if rpc_url.contains("devnet") || rpc_url.contains("dev") {
                return SolanaChainId::Devnet;
            } else if rpc_url.contains("testnet") || rpc_url.contains("test") {
                return SolanaChainId::Testnet;
            } else {
                return SolanaChainId::Mainnet;
            }
        }

        // 默认返回主网
        SolanaChainId::Mainnet
    }

    /// 从链 ID 数值创建枚举
    pub fn from_chain_id(chain_id: u32) -> Option<Self> {
        match chain_id {
            101 => Some(SolanaChainId::Mainnet),
            102 => Some(SolanaChainId::Testnet),
            103 => Some(SolanaChainId::Devnet),
            _ => None,
        }
    }

    /// 从网络名称字符串创建枚举
    pub fn from_network_name(network: &str) -> Option<Self> {
        match network.to_lowercase().as_str() {
            "mainnet" | "mainnet-beta" => Some(SolanaChainId::Mainnet),
            "testnet" => Some(SolanaChainId::Testnet),
            "devnet" | "development" => Some(SolanaChainId::Devnet),
            _ => None,
        }
    }

    /// 获取默认的 RPC URL
    pub fn default_rpc_url(&self) -> &'static str {
        match self {
            SolanaChainId::Mainnet => "https://api.mainnet-beta.solana.com",
            SolanaChainId::Testnet => "https://api.testnet.solana.com",
            SolanaChainId::Devnet => "https://api.devnet.solana.com",
        }
    }

    /// 检查是否为生产环境（主网）
    pub fn is_production(&self) -> bool {
        matches!(self, SolanaChainId::Mainnet)
    }

    /// 检查是否为测试环境
    pub fn is_test_environment(&self) -> bool {
        matches!(self, SolanaChainId::Testnet | SolanaChainId::Devnet)
    }
}

impl Default for SolanaChainId {
    fn default() -> Self {
        SolanaChainId::from_env()
    }
}

impl fmt::Display for SolanaChainId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.network_name())
    }
}

impl From<SolanaChainId> for u32 {
    fn from(chain_id: SolanaChainId) -> Self {
        chain_id.chain_id()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_sdk::pubkey::Pubkey;
    use std::str::FromStr;

    #[test]
    fn test_v2_amm_constants_are_valid() {
        // Test that the V2 AMM program ID is a valid Pubkey
        let program_id = Pubkey::from_str(DEFAULT_RAYDIUM_V2_AMM_PROGRAM_ID);
        assert!(program_id.is_ok(), "V2 AMM program ID should be a valid Pubkey");

        // Test that the default open time is valid
        assert_eq!(DEFAULT_V2_AMM_OPEN_TIME, 0);
    }

    #[test]
    fn test_v2_amm_program_id_format() {
        // Verify the program ID matches the expected Raydium V2 AMM program ID
        assert_eq!(DEFAULT_RAYDIUM_V2_AMM_PROGRAM_ID, "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8");
    }

    #[test]
    fn test_solana_chain_id_values() {
        assert_eq!(SolanaChainId::Mainnet.chain_id(), 101);
        assert_eq!(SolanaChainId::Testnet.chain_id(), 102);
        assert_eq!(SolanaChainId::Devnet.chain_id(), 103);
    }

    #[test]
    fn test_solana_chain_id_network_names() {
        assert_eq!(SolanaChainId::Mainnet.network_name(), "mainnet");
        assert_eq!(SolanaChainId::Testnet.network_name(), "testnet");
        assert_eq!(SolanaChainId::Devnet.network_name(), "devnet");
    }

    #[test]
    fn test_solana_chain_id_from_chain_id() {
        assert_eq!(SolanaChainId::from_chain_id(101), Some(SolanaChainId::Mainnet));
        assert_eq!(SolanaChainId::from_chain_id(102), Some(SolanaChainId::Testnet));
        assert_eq!(SolanaChainId::from_chain_id(103), Some(SolanaChainId::Devnet));
        assert_eq!(SolanaChainId::from_chain_id(999), None);
    }

    #[test]
    fn test_solana_chain_id_from_network_name() {
        assert_eq!(SolanaChainId::from_network_name("mainnet"), Some(SolanaChainId::Mainnet));
        assert_eq!(SolanaChainId::from_network_name("MAINNET"), Some(SolanaChainId::Mainnet));
        assert_eq!(SolanaChainId::from_network_name("mainnet-beta"), Some(SolanaChainId::Mainnet));
        assert_eq!(SolanaChainId::from_network_name("testnet"), Some(SolanaChainId::Testnet));
        assert_eq!(SolanaChainId::from_network_name("devnet"), Some(SolanaChainId::Devnet));
        assert_eq!(SolanaChainId::from_network_name("development"), Some(SolanaChainId::Devnet));
        assert_eq!(SolanaChainId::from_network_name("invalid"), None);
    }

    #[test]
    fn test_solana_chain_id_display() {
        assert_eq!(SolanaChainId::Mainnet.to_string(), "mainnet");
        assert_eq!(SolanaChainId::Testnet.to_string(), "testnet");
        assert_eq!(SolanaChainId::Devnet.to_string(), "devnet");
    }

    #[test]
    fn test_solana_chain_id_conversion_to_u32() {
        let mainnet: u32 = SolanaChainId::Mainnet.into();
        let testnet: u32 = SolanaChainId::Testnet.into();
        let devnet: u32 = SolanaChainId::Devnet.into();

        assert_eq!(mainnet, 101);
        assert_eq!(testnet, 102);
        assert_eq!(devnet, 103);
    }

    #[test]
    fn test_solana_chain_id_environment_checks() {
        assert!(SolanaChainId::Mainnet.is_production());
        assert!(!SolanaChainId::Testnet.is_production());
        assert!(!SolanaChainId::Devnet.is_production());

        assert!(!SolanaChainId::Mainnet.is_test_environment());
        assert!(SolanaChainId::Testnet.is_test_environment());
        assert!(SolanaChainId::Devnet.is_test_environment());
    }

    #[test]
    fn test_solana_chain_id_default_rpc_urls() {
        assert_eq!(SolanaChainId::Mainnet.default_rpc_url(), "https://api.mainnet-beta.solana.com");
        assert_eq!(SolanaChainId::Testnet.default_rpc_url(), "https://api.testnet.solana.com");
        assert_eq!(SolanaChainId::Devnet.default_rpc_url(), "https://api.devnet.solana.com");
    }

    #[test]
    fn test_solana_chain_id_from_env() {
        // 测试环境变量检测（需要模拟环境变量）
        std::env::set_var("CARGO_ENV", "Development");
        assert_eq!(SolanaChainId::from_env(), SolanaChainId::Devnet);

        std::env::set_var("CARGO_ENV", "Test");
        assert_eq!(SolanaChainId::from_env(), SolanaChainId::Testnet);

        std::env::set_var("CARGO_ENV", "Production");
        assert_eq!(SolanaChainId::from_env(), SolanaChainId::Mainnet);

        // 清理环境变量
        std::env::remove_var("CARGO_ENV");

        // 测试 RPC_URL 检测
        std::env::set_var("RPC_URL", "https://api.devnet.solana.com");
        assert_eq!(SolanaChainId::from_env(), SolanaChainId::Devnet);

        std::env::set_var("RPC_URL", "https://api.testnet.solana.com");
        assert_eq!(SolanaChainId::from_env(), SolanaChainId::Testnet);

        std::env::set_var("RPC_URL", "https://api.mainnet-beta.solana.com");
        assert_eq!(SolanaChainId::from_env(), SolanaChainId::Mainnet);

        // 清理环境变量
        std::env::remove_var("RPC_URL");
    }
}
