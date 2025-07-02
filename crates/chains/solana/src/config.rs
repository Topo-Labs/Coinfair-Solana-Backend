use solana_sdk::{
    pubkey::Pubkey,
    signature::Keypair,
};
use std::str::FromStr;
use dotenv::dotenv;
use std::env;
use base64::Engine;

/// 配置结构体
#[derive(Debug, Clone)]
pub struct Config {
    /// Solana RPC URL
    pub rpc_url: String,
    
    /// 用户钱包地址
    pub user_wallet_address: String,
    
    /// 用户私钥
    pub user_private_key: String,
    
    /// 推荐程序ID
    pub referral_program_id: String,
    
    /// NFT Mint地址
    pub nft_mint_address: String,
}

impl Config {
    /// 从环境变量加载配置
    pub fn from_env() -> anyhow::Result<Self> {
        // 加载.env文件
        dotenv().ok();
        
        let config = Self {
            rpc_url: env::var("SOLANA_RPC_URL")
                .unwrap_or_else(|_| "https://api.devnet.solana.com".to_string()),
            
            user_wallet_address: env::var("USER_WALLET_ADDRESS")
                .unwrap_or_else(|_| "".to_string()),
            
            user_private_key: env::var("USER_PRIVATE_KEY")
                .unwrap_or_else(|_| "".to_string()),
            
            referral_program_id: env::var("REFERRAL_PROGRAM_ID")
                .unwrap_or_else(|_| "REFxcjx4pKym9j5Jzbo9wh92CtYTzHt9fqcjgvZGvUL".to_string()),
            
            nft_mint_address: env::var("NFT_MINT_ADDRESS")
                .unwrap_or_else(|_| "11111111111111111111111111111111".to_string()),
        };
        
        Ok(config)
    }
    
    /// 从私钥字符串创建Keypair
    pub fn get_user_keypair(&self) -> anyhow::Result<Keypair> {
        if self.user_private_key.is_empty() {
            return Err(anyhow::anyhow!("USER_PRIVATE_KEY not set in environment"));
        }
        
        // 支持base58和base64格式的私钥
        let keypair = if self.user_private_key.len() == 88 {
            // base58格式
            let bytes = bs58::decode(&self.user_private_key)
                .into_vec()
                .map_err(|e| anyhow::anyhow!("Invalid base58 private key: {}", e))?;
            Keypair::from_bytes(&bytes)
                .map_err(|e| anyhow::anyhow!("Invalid keypair bytes: {}", e))?
        } else {
            // base64格式
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(&self.user_private_key)
                .map_err(|e| anyhow::anyhow!("Invalid base64 private key: {}", e))?;
            Keypair::from_bytes(&bytes)
                .map_err(|e| anyhow::anyhow!("Invalid keypair bytes: {}", e))?
        };
        
        Ok(keypair)
    }
    
    /// 验证配置
    pub fn validate(&self) -> anyhow::Result<()> {
        if self.user_wallet_address.is_empty() {
            return Err(anyhow::anyhow!("USER_WALLET_ADDRESS not set"));
        }
        
        if self.user_private_key.is_empty() {
            return Err(anyhow::anyhow!("USER_PRIVATE_KEY not set"));
        }
        
        if self.referral_program_id.is_empty() {
            return Err(anyhow::anyhow!("REFERRAL_PROGRAM_ID not set"));
        }
        
        if self.nft_mint_address.is_empty() {
            return Err(anyhow::anyhow!("NFT_MINT_ADDRESS not set"));
        }
        
        // 验证钱包地址格式
        Pubkey::from_str(&self.user_wallet_address)
            .map_err(|e| anyhow::anyhow!("Invalid wallet address: {}", e))?;
        
        // 验证程序ID格式
        Pubkey::from_str(&self.referral_program_id)
            .map_err(|e| anyhow::anyhow!("Invalid referral program ID: {}", e))?;
        
        // 验证NFT mint地址格式
        Pubkey::from_str(&self.nft_mint_address)
            .map_err(|e| anyhow::anyhow!("Invalid NFT mint address: {}", e))?;
        
        Ok(())
    }
    
    /// 打印配置信息（隐藏私钥）
    pub fn print_info(&self) {
        println!("📋 配置信息:");
        println!("  RPC URL: {}", self.rpc_url);
        println!("  User Wallet: {}", self.user_wallet_address);
        println!("  Referral Program ID: {}", self.referral_program_id);
        println!("  NFT Mint Address: {}", self.nft_mint_address);
        println!("  Private Key: [隐藏]");
    }
} 