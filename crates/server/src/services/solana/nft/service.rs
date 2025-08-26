use crate::dtos::solana::nft::mint::{
     MintNftAndSendTransactionResponse,
    MintNftRequest, MintNftResponse,
};

use super::super::shared::{helpers::SolanaUtils, SharedContext};
use ::utils::solana::ConfigManager;

use anyhow::Result;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use chrono::Utc;
use sha2::{Digest, Sha256};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    instruction::Instruction, pubkey::Pubkey, signer::Signer, system_program, sysvar::rent, transaction::Transaction,
};
use std::str::FromStr;
use std::sync::Arc;
use tracing::info;

use anchor_lang::prelude::AccountMeta;
use spl_associated_token_account::get_associated_token_address;
use crate::dtos::solana::common::TransactionStatus;
use crate::dtos::solana::nft::claim::{ClaimNftAndSendTransactionResponse, ClaimNftRequest, ClaimNftResponse};

/// NFT服务 - 处理推荐NFT的铸造
pub struct NftService {
    shared: Arc<SharedContext>,
}

impl NftService {
    /// 创建新的NFT服务实例
    pub fn new(shared: Arc<SharedContext>) -> Self {
        Self { shared }
    }

    /// 铸造推荐NFT（不签名，返回交易给前端签名）
    pub async fn mint_nft(&self, request: MintNftRequest) -> Result<MintNftResponse> {
        info!("🎯 开始构建铸造推荐NFT交易");
        info!("  用户钱包: {}", request.user_wallet);
        info!("  铸造数量: {}", request.amount);

        let user_wallet = Pubkey::from_str(&request.user_wallet)?;

        // 构建指令
        let instructions = self
            .build_mint_nft_instructions_internal(user_wallet, request.amount)
            .await?;

        // 创建交易
        let rpc_client = RpcClient::new(&self.shared.swap_config.rpc_url);
        let recent_blockhash = rpc_client.get_latest_blockhash()?;
        let mut transaction = Transaction::new_with_payer(&instructions, Some(&user_wallet));
        transaction.message.recent_blockhash = recent_blockhash;

        // 序列化交易供前端签名
        let serialized_transaction = bincode::serialize(&transaction)?;
        let serialized_transaction_base64 = BASE64_STANDARD.encode(&serialized_transaction);

        // 生成PDA地址信息
        let (user_referral, _) = self.get_user_referral_pda_internal(&user_wallet)?;
        let (mint_counter, _) = self.get_mint_counter_pda_internal(&user_wallet)?;
        let (nft_pool_authority, _) = self.get_nft_pool_authority_pda_internal(&user_wallet)?;
        let nft_pool_account = self.get_nft_pool_account_internal(&nft_pool_authority)?;
        let nft_mint = self.get_nft_mint_internal()?;

        let response = MintNftResponse {
            signature: None,
            user_wallet: request.user_wallet,
            amount: request.amount,
            nft_mint: nft_mint.to_string(),
            user_referral: user_referral.to_string(),
            mint_counter: mint_counter.to_string(),
            nft_pool_authority: nft_pool_authority.to_string(),
            nft_pool_account: nft_pool_account.to_string(),
            status: TransactionStatus::Pending,
            explorer_url: None,
            timestamp: Utc::now().timestamp(),
            serialized_transaction: Some(serialized_transaction_base64),
        };

        info!("✅ NFT铸造交易构建完成");
        Ok(response)
    }

    /// 铸造推荐NFT并发送交易（本地签名）
    pub async fn mint_nft_and_send_transaction(
        &self,
        request: MintNftRequest,
    ) -> Result<MintNftAndSendTransactionResponse> {
        info!("🎯 开始铸造推荐NFT并发送交易");
        info!("  用户钱包: {}", request.user_wallet);
        info!("  铸造数量: {}", request.amount);

        let user_wallet = Pubkey::from_str(&request.user_wallet)?;

        // 获取管理员密钥用于签名
        let payer_keypair = ConfigManager::get_admin_keypair()?;

        // 构建指令
        let instructions = self
            .build_mint_nft_instructions_internal(user_wallet, request.amount)
            .await?;

        // 创建和发送交易
        let rpc_client = RpcClient::new(&self.shared.swap_config.rpc_url);
        let recent_blockhash = rpc_client.get_latest_blockhash()?;

        let transaction = Transaction::new_signed_with_payer(
            &instructions,
            Some(&payer_keypair.pubkey()),
            &[&payer_keypair],
            recent_blockhash,
        );

        let signature = rpc_client.send_and_confirm_transaction(&transaction)?;

        // 生成PDA地址信息
        let (user_referral, _) = self.get_user_referral_pda_internal(&user_wallet)?;
        let (mint_counter, _) = self.get_mint_counter_pda_internal(&user_wallet)?;
        let (nft_pool_authority, _) = self.get_nft_pool_authority_pda_internal(&user_wallet)?;
        let nft_pool_account = self.get_nft_pool_account_internal(&nft_pool_authority)?;
        let nft_mint = self.get_nft_mint_internal()?;

        let explorer_url = SolanaUtils::get_explorer_url(&signature.to_string(), &self.shared.swap_config.rpc_url);

        let response = MintNftAndSendTransactionResponse {
            signature: signature.to_string(),
            user_wallet: request.user_wallet,
            amount: request.amount,
            nft_mint: nft_mint.to_string(),
            user_referral: user_referral.to_string(),
            mint_counter: mint_counter.to_string(),
            nft_pool_authority: nft_pool_authority.to_string(),
            nft_pool_account: nft_pool_account.to_string(),
            status: TransactionStatus::Confirmed,
            explorer_url,
            timestamp: Utc::now().timestamp(),
        };

        info!("✅ NFT铸造交易已成功发送，签名: {}", signature);
        Ok(response)
    }

    /// 领取推荐NFT（不签名，返回交易给前端签名）
    pub async fn claim_nft(&self, request: ClaimNftRequest) -> Result<ClaimNftResponse> {
        info!("🎯 开始构建领取推荐NFT交易");
        info!("  下级用户钱包: {}", request.user_wallet);
        info!("  上级用户钱包: {}", request.upper);

        let user_wallet = Pubkey::from_str(&request.user_wallet)?;
        let upper_wallet = Pubkey::from_str(&request.upper)?;

        // 构建指令
        let instructions = self
            .build_claim_nft_instructions_internal(user_wallet, upper_wallet)
            .await?;

        // 创建交易
        let rpc_client = RpcClient::new(&self.shared.swap_config.rpc_url);
        let recent_blockhash = rpc_client.get_latest_blockhash()?;
        let mut transaction = Transaction::new_with_payer(&instructions, Some(&user_wallet));
        transaction.message.recent_blockhash = recent_blockhash;

        // 序列化交易供前端签名
        let serialized_transaction = bincode::serialize(&transaction)?;
        let serialized_transaction_base64 = BASE64_STANDARD.encode(&serialized_transaction);

        // 生成所有相关PDA地址
        let (user_referral, _) = self.get_user_referral_pda_internal(&user_wallet)?;
        let (upper_referral, _) = self.get_user_referral_pda_internal(&upper_wallet)?;
        let (upper_mint_counter, _) = self.get_mint_counter_pda_internal(&upper_wallet)?;
        let (nft_pool_authority, _) = self.get_nft_pool_authority_pda_internal(&upper_wallet)?;
        let nft_pool_account = self.get_nft_pool_account_internal(&nft_pool_authority)?;
        let nft_mint = self.get_nft_mint_internal()?;
        let user_ata = get_associated_token_address(&user_wallet, &nft_mint);
        let (referral_config, _) = self.get_referral_config_pda_internal()?;
        let protocol_wallet = self.get_protocol_wallet_internal()?;

        let response = ClaimNftResponse {
            signature: None,
            user_wallet: request.user_wallet,
            upper: request.upper,
            nft_mint: nft_mint.to_string(),
            user_referral: user_referral.to_string(),
            upper_referral: upper_referral.to_string(),
            upper_mint_counter: upper_mint_counter.to_string(),
            nft_pool_authority: nft_pool_authority.to_string(),
            nft_pool_account: nft_pool_account.to_string(),
            user_ata: user_ata.to_string(),
            protocol_wallet: protocol_wallet.to_string(),
            referral_config: referral_config.to_string(),
            status: TransactionStatus::Pending,
            explorer_url: None,
            timestamp: Utc::now().timestamp(),
            serialized_transaction: Some(serialized_transaction_base64),
        };

        info!("✅ NFT领取交易构建完成");
        Ok(response)
    }

    /// 领取推荐NFT并发送交易（本地签名）
    pub async fn claim_nft_and_send_transaction(
        &self,
        request: ClaimNftRequest,
    ) -> Result<ClaimNftAndSendTransactionResponse> {
        info!("🎯 开始领取推荐NFT并发送交易");
        info!("  下级用户钱包: {}", request.user_wallet);
        info!("  上级用户钱包: {}", request.upper);

        let user_wallet = Pubkey::from_str(&request.user_wallet)?;
        let upper_wallet = Pubkey::from_str(&request.upper)?;

        // 获取签名密钥 - 注意：这里应该是下级用户的密钥
        let lower_keypair = ConfigManager::get_lower_keypair()?;
        info!("🔑 下级用户私钥: {:?}", lower_keypair.to_base58_string());

        // 构建指令
        let instructions = self
            .build_claim_nft_instructions_internal(user_wallet, upper_wallet)
            .await?;

        // 创建和发送交易
        let rpc_client = RpcClient::new(&self.shared.swap_config.rpc_url);
        let recent_blockhash = rpc_client.get_latest_blockhash()?;

        let transaction = Transaction::new_signed_with_payer(
            &instructions,
            Some(&lower_keypair.pubkey()),
            &[&lower_keypair],
            recent_blockhash,
        );

        let signature = rpc_client.send_and_confirm_transaction(&transaction)?;

        // 生成所有相关PDA地址
        let (user_referral, _) = self.get_user_referral_pda_internal(&user_wallet)?;
        let (upper_referral, _) = self.get_user_referral_pda_internal(&upper_wallet)?;
        let (upper_mint_counter, _) = self.get_mint_counter_pda_internal(&upper_wallet)?;
        let (nft_pool_authority, _) = self.get_nft_pool_authority_pda_internal(&upper_wallet)?;
        let nft_pool_account = self.get_nft_pool_account_internal(&nft_pool_authority)?;
        let nft_mint = self.get_nft_mint_internal()?;
        let user_ata = get_associated_token_address(&user_wallet, &nft_mint);
        let (referral_config, _) = self.get_referral_config_pda_internal()?;
        let protocol_wallet = self.get_protocol_wallet_internal()?;

        let explorer_url = SolanaUtils::get_explorer_url(&signature.to_string(), &self.shared.swap_config.rpc_url);

        let response = ClaimNftAndSendTransactionResponse {
            signature: signature.to_string(),
            user_wallet: request.user_wallet,
            upper: request.upper,
            nft_mint: nft_mint.to_string(),
            user_referral: user_referral.to_string(),
            upper_referral: upper_referral.to_string(),
            upper_mint_counter: upper_mint_counter.to_string(),
            nft_pool_authority: nft_pool_authority.to_string(),
            nft_pool_account: nft_pool_account.to_string(),
            user_ata: user_ata.to_string(),
            protocol_wallet: protocol_wallet.to_string(),
            referral_config: referral_config.to_string(),
            status: TransactionStatus::Confirmed,
            explorer_url,
            timestamp: Utc::now().timestamp(),
        };

        info!("✅ NFT领取交易已成功发送，签名: {}", signature);
        Ok(response)
    }

    /// 构建铸造NFT的指令
    #[cfg(test)]
    pub async fn build_mint_nft_instructions(&self, user_wallet: Pubkey, amount: u64) -> Result<Vec<Instruction>> {
        self.build_mint_nft_instructions_internal(user_wallet, amount).await
    }

    async fn build_mint_nft_instructions_internal(&self, user_wallet: Pubkey, amount: u64) -> Result<Vec<Instruction>> {
        let referral_program_id = self.get_referral_program_id_internal()?;
        let nft_mint = self.get_nft_mint_internal()?;

        // 计算所有PDA地址
        let (referral_config, _) = Pubkey::find_program_address(&[b"config"], &referral_program_id);
        let (user_referral, _) = self.get_user_referral_pda_internal(&user_wallet)?;
        let (mint_counter, _) = self.get_mint_counter_pda_internal(&user_wallet)?;
        let (mint_authority, _) = Pubkey::find_program_address(&[b"mint_authority"], &referral_program_id);
        let (nft_pool_authority, _) = self.get_nft_pool_authority_pda_internal(&user_wallet)?;
        let nft_pool_account = self.get_nft_pool_account_internal(&nft_pool_authority)?;
        let user_ata = get_associated_token_address(&user_wallet, &nft_mint);

        // 构建账户元数据
        let accounts = vec![
            AccountMeta::new(user_wallet, true),                    // authority (signer)
            AccountMeta::new_readonly(referral_config, false),      // config
            AccountMeta::new(user_referral, false),                 // user_referral
            AccountMeta::new(nft_mint, false),                      // official_mint
            AccountMeta::new(user_ata, false),                      // user_ata
            AccountMeta::new(mint_counter, false),                  // mint_counter
            AccountMeta::new_readonly(mint_authority, false),       // mint_authority
            AccountMeta::new_readonly(nft_pool_authority, false),   // nft_pool_authority
            AccountMeta::new(nft_pool_account, false),              // nft_pool_account
            AccountMeta::new_readonly(spl_token::id(), false),      // token_program
            AccountMeta::new_readonly(system_program::id(), false), // system_program
            AccountMeta::new_readonly(spl_associated_token_account::id(), false), // associated_token_program
            AccountMeta::new_readonly(rent::id(), false),           // rent
        ];
        let mut instruction_data = vec![];
        // 构建指令数据 (discriminator) - 使用正确的Anchor哈希方法
        let discriminator = Self::calculate_instruction_discriminator("mint_nft");
        instruction_data.extend_from_slice(&discriminator);
        // mint_nft amount
        instruction_data.extend_from_slice(&amount.to_le_bytes());

        let instruction = Instruction {
            program_id: referral_program_id,
            accounts,
            data: instruction_data,
        };

        Ok(vec![instruction])
    }

    /// 构建领取NFT的指令
    #[cfg(test)]
    pub async fn build_claim_nft_instructions(
        &self,
        user_wallet: Pubkey,
        upper_wallet: Pubkey,
    ) -> Result<Vec<Instruction>> {
        self.build_claim_nft_instructions_internal(user_wallet, upper_wallet)
            .await
    }

    async fn build_claim_nft_instructions_internal(
        &self,
        user_wallet: Pubkey,
        upper_wallet: Pubkey,
    ) -> Result<Vec<Instruction>> {
        let referral_program_id = self.get_referral_program_id_internal()?;
        let nft_mint = self.get_nft_mint_internal()?;
        let protocol_wallet = self.get_protocol_wallet_internal()?;

        // 计算所有PDA地址
        let (referral_config, _) = self.get_referral_config_pda_internal()?;
        let (user_referral, _) = self.get_user_referral_pda_internal(&user_wallet)?;
        let (upper_referral, _) = self.get_user_referral_pda_internal(&upper_wallet)?;
        let (upper_mint_counter, _) = self.get_mint_counter_pda_internal(&upper_wallet)?;
        let (nft_pool_authority, _) = self.get_nft_pool_authority_pda_internal(&upper_wallet)?;
        let nft_pool_account = self.get_nft_pool_account_internal(&nft_pool_authority)?;
        let user_ata = get_associated_token_address(&user_wallet, &nft_mint);

        // 检查upper_mint_counter是否存在
        let rpc_client = &self.shared.rpc_client;
        if let Err(_) = rpc_client.get_account(&upper_mint_counter) {
            return Err(anyhow::anyhow!(
                "上级用户({})的mint_counter账户不存在，上级用户需要先铸造NFT来初始化账户",
                upper_wallet
            ));
        }

        // 构建账户元数据 - 严格按照合约ClaimReferralNFT结构的字段顺序
        let mut accounts = vec![
            AccountMeta::new(user_wallet, true),                                  // user (signer)
            AccountMeta::new_readonly(upper_wallet, false),                       // upper
            AccountMeta::new(user_referral, false),                               // user_referral
            AccountMeta::new(upper_mint_counter, false),                          // upper_mint_counter
            AccountMeta::new_readonly(upper_referral, false),                     // upper_referral
            AccountMeta::new_readonly(referral_config, false),                    // config
            AccountMeta::new(nft_mint, false),                                    // official_mint
            AccountMeta::new(user_ata, false),                                    // user_ata
            AccountMeta::new(protocol_wallet, false),                             // protocol_wallet
            AccountMeta::new_readonly(nft_pool_authority, false),                 // nft_pool_authority
            AccountMeta::new(nft_pool_account, false),                            // nft_pool_account
            AccountMeta::new_readonly(spl_token::id(), false),                    // token_program
            AccountMeta::new_readonly(system_program::id(), false),               // system_program
            AccountMeta::new_readonly(spl_associated_token_account::id(), false), // associated_token_program
            AccountMeta::new_readonly(rent::id(), false),                         // rent
        ];

        // 重要：手动修复upper_mint_counter为可写状态（复现CLI逻辑）
        for account_meta in &mut accounts {
            if account_meta.pubkey == upper_mint_counter {
                account_meta.is_writable = true;
            }
        }

        // 构建指令数据 (discriminator) - 使用正确的Anchor哈希方法
        let discriminator = Self::calculate_instruction_discriminator("claim_nft");
        let instruction_data = discriminator.to_vec();

        let instruction = Instruction {
            program_id: referral_program_id,
            accounts,
            data: instruction_data,
        };

        Ok(vec![instruction])
    }

    /// 获取推荐配置PDA
    #[cfg(test)]
    pub fn get_referral_config_pda(&self) -> Result<(Pubkey, u8)> {
        self.get_referral_config_pda_internal()
    }

    fn get_referral_config_pda_internal(&self) -> Result<(Pubkey, u8)> {
        let referral_program_id = self.get_referral_program_id_internal()?;
        let (pda, bump) = Pubkey::find_program_address(&[b"config"], &referral_program_id);
        Ok((pda, bump))
    }

    /// 获取协议钱包地址
    #[cfg(test)]
    pub fn get_protocol_wallet(&self) -> Result<Pubkey> {
        self.get_protocol_wallet_internal()
    }

    fn get_protocol_wallet_internal(&self) -> Result<Pubkey> {
        let wallet_str = std::env::var("PROTOCOL_WALLET")
            .unwrap_or_else(|_| "8S2bcP66WehuF6cHryfZ7vfFpQWaUhYyAYSy5U3gX4Fy".to_string()); // 默认值，需要配置
        Pubkey::from_str(&wallet_str).map_err(Into::into)
    }

    /// 获取推荐程序ID
    #[cfg(test)]
    pub fn get_referral_program_id(&self) -> Result<Pubkey> {
        self.get_referral_program_id_internal()
    }

    fn get_referral_program_id_internal(&self) -> Result<Pubkey> {
        let program_id_str = std::env::var("REFERRAL_PROGRAM_ID")
            .unwrap_or_else(|_| "REFxcjx4pKym9j5Jzbo9wh92CtYTzHt9fqcjgvZGvUL".to_string()); // 默认值，需要配置
        Pubkey::from_str(&program_id_str).map_err(Into::into)
    }

    /// 获取NFT mint地址
    #[cfg(test)]
    pub fn get_nft_mint(&self) -> Result<Pubkey> {
        self.get_nft_mint_internal()
    }

    fn get_nft_mint_internal(&self) -> Result<Pubkey> {
        let mint_str = std::env::var("COINFAIR_NFT_MINT")
            .unwrap_or_else(|_| "NFTaoszFxtEmGXvHcb8yfkGZxqLPAfwDqLN1mhrV2jM".to_string()); // 默认值，需要配置
        Pubkey::from_str(&mint_str).map_err(Into::into)
    }

    /// 获取用户推荐账户PDA
    #[cfg(test)]
    pub fn get_user_referral_pda(&self, user_wallet: &Pubkey) -> Result<(Pubkey, u8)> {
        self.get_user_referral_pda_internal(user_wallet)
    }

    fn get_user_referral_pda_internal(&self, user_wallet: &Pubkey) -> Result<(Pubkey, u8)> {
        let referral_program_id = self.get_referral_program_id_internal()?;
        let (pda, bump) = Pubkey::find_program_address(&[b"referral", user_wallet.as_ref()], &referral_program_id);
        Ok((pda, bump))
    }

    /// 获取mint计数器PDA
    #[cfg(test)]
    pub fn get_mint_counter_pda(&self, user_wallet: &Pubkey) -> Result<(Pubkey, u8)> {
        self.get_mint_counter_pda_internal(user_wallet)
    }

    fn get_mint_counter_pda_internal(&self, user_wallet: &Pubkey) -> Result<(Pubkey, u8)> {
        let referral_program_id = self.get_referral_program_id_internal()?;
        let (pda, bump) = Pubkey::find_program_address(&[b"mint_counter", user_wallet.as_ref()], &referral_program_id);
        Ok((pda, bump))
    }

    /// 获取NFT池子权限PDA
    #[cfg(test)]
    pub fn get_nft_pool_authority_pda(&self, user_wallet: &Pubkey) -> Result<(Pubkey, u8)> {
        self.get_nft_pool_authority_pda_internal(user_wallet)
    }

    fn get_nft_pool_authority_pda_internal(&self, user_wallet: &Pubkey) -> Result<(Pubkey, u8)> {
        let referral_program_id = self.get_referral_program_id_internal()?;
        let (pda, bump) = Pubkey::find_program_address(&[b"nft_pool", user_wallet.as_ref()], &referral_program_id);
        Ok((pda, bump))
    }

    /// 获取NFT池子账户地址
    #[cfg(test)]
    pub fn get_nft_pool_account(&self, nft_pool_authority: &Pubkey) -> Result<Pubkey> {
        self.get_nft_pool_account_internal(nft_pool_authority)
    }

    fn get_nft_pool_account_internal(&self, nft_pool_authority: &Pubkey) -> Result<Pubkey> {
        let nft_mint = self.get_nft_mint_internal()?;
        Ok(get_associated_token_address(nft_pool_authority, &nft_mint))
    }

    /// 计算Anchor指令discriminator
    /// 根据Anchor框架规范：SHA256("global:指令名称")的前8字节
    fn calculate_instruction_discriminator(instruction_name: &str) -> [u8; 8] {
        let preimage = format!("global:{}", instruction_name);
        let mut hasher = Sha256::new();
        hasher.update(preimage.as_bytes());
        let hash = hasher.finalize();

        let mut discriminator = [0u8; 8];
        discriminator.copy_from_slice(&hash[..8]);
        discriminator
    }
}
