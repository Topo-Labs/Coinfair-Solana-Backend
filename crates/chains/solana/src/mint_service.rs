use solana_sdk::{
    commitment_config::CommitmentConfig,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    system_instruction,
    transaction::Transaction,
    program_pack::Pack,
};
use solana_client::rpc_client::RpcClient;
use std::str::FromStr;

use crate::config::Config;

/// Mint NFT服务
pub struct MintService {
    rpc_client: RpcClient,
    config: Config,
    referral_program_id: Pubkey,
}

impl MintService {
    /// 创建新的Mint服务（从环境变量加载配置）
    pub fn new() -> anyhow::Result<Self> {
        let config = Config::from_env()?;
        config.validate()?;
        
        let rpc_client = RpcClient::new_with_commitment(
            config.rpc_url.clone(),
            CommitmentConfig::confirmed(),
        );
        
        // 使用硬编码的程序ID
        let referral_program_id = Pubkey::from_str("REFxcjx4pKym9j5Jzbo9wh92CtYTzHt9fqcjgvZGvUL")?;
        
        Ok(Self {
            rpc_client,
            config,
            referral_program_id,
        })
    }
    
    /// 获取程序实例（简化版本，直接返回程序ID）
    pub fn program_id(&self) -> Pubkey {
        self.referral_program_id
    }
    
    /// 获取推荐配置账户地址
    pub fn get_config_address(&self) -> anyhow::Result<Pubkey> {
        let (config_address, _) = Pubkey::find_program_address(
            &[b"config"],
            &self.referral_program_id,
        );
        Ok(config_address)
    }
    
    /// 获取用户推荐账户地址
    pub fn get_referral_account_address(&self, user: &str) -> anyhow::Result<Pubkey> {
        let user_pubkey = Pubkey::from_str(user)?;
        let (referral_address, _) = Pubkey::find_program_address(
            &[b"referral", user_pubkey.as_ref()],
            &self.referral_program_id,
        );
        Ok(referral_address)
    }
    
    /// 获取mint计数器地址
    pub fn get_mint_counter_address(&self, user: &str) -> anyhow::Result<Pubkey> {
        let user_pubkey = Pubkey::from_str(user)?;
        let (counter_address, _) = Pubkey::find_program_address(
            &[b"mint_counter", user_pubkey.as_ref()],
            &self.referral_program_id,
        );
        Ok(counter_address)
    }
    
    /// 获取mint权限地址
    pub fn get_mint_authority_address(&self) -> anyhow::Result<Pubkey> {
        let (_mint_authority, _) = Pubkey::find_program_address(
            &[b"mint_authority"],
            &self.referral_program_id,
        );
        Ok(_mint_authority)
    }
    
    /// 获取NFT池权限地址
    pub fn get_nft_pool_authority_address(&self, user: &str) -> anyhow::Result<Pubkey> {
        let user_pubkey = Pubkey::from_str(user)?;
        let (pool_authority, _) = Pubkey::find_program_address(
            &[b"nft_pool", user_pubkey.as_ref()],
            &self.referral_program_id,
        );
        Ok(pool_authority)
    }
    
    /// 铸造NFT
    pub async fn mint_nft(&self, user_wallet: &str, amount: u64, keypair: &Keypair) -> anyhow::Result<String> {
        if amount == 0 {
            return Err(anyhow::anyhow!("Amount must be greater than 0"));
        }
        
        let user_pubkey = Pubkey::from_str(user_wallet)?;
        
        // 获取账户地址
        let (referral_address, _) = Pubkey::find_program_address(
            &[b"referral", user_pubkey.as_ref()],
            &self.referral_program_id,
        );
        
        let (mint_counter_address, _) = Pubkey::find_program_address(
            &[b"mint_counter", user_pubkey.as_ref()],
            &self.referral_program_id,
        );
        
        let (_mint_authority, _) = Pubkey::find_program_address(
            &[b"mint_authority"],
            &self.referral_program_id,
        );
        
        let (nft_pool_authority, _) = Pubkey::find_program_address(
            &[b"nft_pool", user_pubkey.as_ref()],
            &self.referral_program_id,
        );
        
        // 临时使用系统程序地址作为NFT mint（实际使用时需要从配置账户读取）
        let nft_mint = Pubkey::from_str("11111111111111111111111111111111")?;
        
        // 获取ATA地址
        let _user_ata = spl_associated_token_account::get_associated_token_address(&user_pubkey, &nft_mint);
        let _nft_pool_ata = spl_associated_token_account::get_associated_token_address(&nft_pool_authority, &nft_mint);
        
        let recent_blockhash = self.rpc_client.get_latest_blockhash()?;
        
        // 构建交易
        let instructions = vec![
            system_instruction::create_account(
                &keypair.pubkey(),
                &referral_address,
                self.rpc_client.get_minimum_balance_for_rent_exemption(8 + 32 * 4 + 1)?,
                8 + 32 * 4 + 1,
                &self.referral_program_id,
            ),
            system_instruction::create_account(
                &keypair.pubkey(),
                &mint_counter_address,
                self.rpc_client.get_minimum_balance_for_rent_exemption(8 + 32 + 8 + 8 + 1)?,
                8 + 32 + 8 + 8 + 1,
                &self.referral_program_id,
            ),
            spl_associated_token_account::instruction::create_associated_token_account(
                &keypair.pubkey(),
                &user_pubkey,
                &user_pubkey,
                &nft_mint,
            ),
            spl_associated_token_account::instruction::create_associated_token_account(
                &keypair.pubkey(),
                &nft_pool_authority,
                &nft_pool_authority,
                &nft_mint,
            ),
        ];
        
        let mut transaction = Transaction::new_with_payer(&instructions, Some(&keypair.pubkey()));
        transaction.sign(&[keypair], recent_blockhash);
        
        let signature = self.rpc_client.send_transaction(&transaction)?;
        self.rpc_client.confirm_transaction(&signature)?;
        
        Ok(signature.to_string())
    }
    
    /// 获取用户余额
    pub fn get_balance(&self, user: &str) -> anyhow::Result<u64> {
        let pubkey = Pubkey::from_str(user)?;
        Ok(self.rpc_client.get_balance(&pubkey)?)
    }
    
    /// 估算费用
    pub fn estimate_fee(&self) -> anyhow::Result<u64> {
        let rent1 = self.rpc_client.get_minimum_balance_for_rent_exemption(8 + 32 * 4 + 1)?;
        let rent2 = self.rpc_client.get_minimum_balance_for_rent_exemption(8 + 32 + 8 + 8 + 1)?;
        let rent3 = self.rpc_client.get_minimum_balance_for_rent_exemption(spl_token::state::Account::LEN)?;
        let rent4 = self.rpc_client.get_minimum_balance_for_rent_exemption(spl_token::state::Account::LEN)?;
        Ok(rent1 + rent2 + rent3 + rent4 + 5000)
    }
    
    /// 使用配置中的用户信息铸造NFT
    pub async fn mint_nft_with_config(&self, amount: u64) -> anyhow::Result<String> {
        let keypair = self.config.get_user_keypair()?;
        self.mint_nft(&self.config.user_wallet_address, amount, &keypair).await
    }
    
    /// 打印配置信息
    pub fn print_config(&self) {
        println!("=== Mint Service Configuration ===");
        println!("RPC URL: {}", self.config.rpc_url);
        println!("User Wallet: {}", self.config.user_wallet_address);
        println!("Program ID: {}", self.referral_program_id);
        println!("================================");
    }
} 