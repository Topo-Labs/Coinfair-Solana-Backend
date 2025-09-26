pub mod instructions;

use anyhow::{format_err, Result};
use solana_sdk::{pubkey::Pubkey, signature::Keypair};

// 重新导出常用的类型和函数
pub use instructions::{amm_instructions, utils};

/// 客户端配置结构体
#[derive(Debug, Clone)]
pub struct ClientConfig {
    pub http_url: String,
    pub ws_url: String,
    pub payer_path: String,
    pub admin_path: String,
    pub raydium_cp_program: Pubkey,
    pub slippage: f64,
}

/// 从文件读取密钥对
pub fn read_keypair_file(s: &str) -> Result<Keypair> {
    solana_sdk::signature::read_keypair_file(s)
        .map_err(|_| format_err!("failed to read keypair from {}", s))
}