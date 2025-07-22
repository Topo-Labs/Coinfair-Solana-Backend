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
pub const DEFAULT_RAYDIUM_V2_AMM_PROGRAM_ID: &str = "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8";
pub const DEFAULT_V2_AMM_OPEN_TIME: u64 = 0;

pub const DEFAULT_SOL_PRICE_USDC: f64 = 100.0;

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
}
