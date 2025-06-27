use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwapConfig {
    pub rpc_url: String,
    pub private_key: String,
    pub amm_program_id: String,
    pub openbook_program_id: String,
    pub usdc_mint: String,
    pub sol_usdc_pool_id: String,
}

impl Default for SwapConfig {
    fn default() -> Self {
        Self {
            rpc_url: "https://api.mainnet-beta.solana.com".to_string(),
            private_key: "".to_string(),
            amm_program_id: "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8".to_string(),
            openbook_program_id: "srmqPvymJeFKQ4zGQed1GFppgkRHL9kaELCbyksJtPX".to_string(),
            usdc_mint: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(),
            sol_usdc_pool_id: "58oQChx4yWmvKdwLLZzBi4ChoCc2fqCUWBkwMihLYQo2".to_string(),
        }
    }
}

impl SwapConfig {
    pub fn get_amm_program_id(&self) -> anyhow::Result<Pubkey> {
        Pubkey::from_str(&self.amm_program_id)
            .map_err(|e| anyhow::anyhow!("Invalid AMM program ID: {}", e))
    }

    pub fn get_openbook_program_id(&self) -> anyhow::Result<Pubkey> {
        Pubkey::from_str(&self.openbook_program_id)
            .map_err(|e| anyhow::anyhow!("Invalid OpenBook program ID: {}", e))
    }

    pub fn get_usdc_mint(&self) -> anyhow::Result<Pubkey> {
        Pubkey::from_str(&self.usdc_mint)
            .map_err(|e| anyhow::anyhow!("Invalid USDC mint: {}", e))
    }

    pub fn get_pool_id(&self) -> anyhow::Result<Pubkey> {
        Pubkey::from_str(&self.sol_usdc_pool_id)
            .map_err(|e| anyhow::anyhow!("Invalid pool ID: {}", e))
    }
} 