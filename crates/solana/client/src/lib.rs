pub mod instructions;

// 从main模块中引入ClientConfig和read_keypair_file
mod main_types {}

// 为了避免直接依赖main.rs，我们重新定义必要的类型
use anyhow::{format_err, Result};
use solana_sdk::{pubkey::Pubkey, signature::Keypair};

#[derive(Clone, Debug, PartialEq)]
pub struct ClientConfig {
    pub http_url: String,
    pub ws_url: String,
    pub payer_path: String,
    pub admin_path: String,
    pub raydium_v3_program: Pubkey,
    pub slippage: f64,
    pub amm_config_key: Pubkey,
    pub mint0: Option<Pubkey>,
    pub mint1: Option<Pubkey>,
    pub pool_id_account: Option<Pubkey>,
    pub tickarray_bitmap_extension: Option<Pubkey>,
    pub amm_config_index: u16,
}

pub fn read_keypair_file(s: &str) -> Result<Keypair> {
    solana_sdk::signature::read_keypair_file(s).map_err(|_| format_err!("failed to read keypair from {}", s))
}

// 重新导出常用的工具函数
pub use instructions::utils::{
    amount_with_slippage, deserialize_anchor_account, get_out_put_amount_and_remaining_accounts, multipler, price_to_sqrt_price_x64, sqrt_price_x64_to_price,
};

// 重新导出其他有用的模块
pub use instructions::amm_instructions;
pub use instructions::rpc;
pub use instructions::token_instructions;
