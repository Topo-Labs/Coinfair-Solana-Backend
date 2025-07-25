// AmmPoolService handles classic AMM pool creation operations

use crate::dtos::solana_dto::{CreateClassicAmmPoolAndSendTransactionResponse, CreateClassicAmmPoolRequest, CreateClassicAmmPoolResponse, TransactionStatus};

use super::super::shared::SharedContext;
use anchor_lang::Discriminator;
use anyhow::Result;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use raydium_cp_swap::instruction;
use solana_sdk::{instruction::Instruction, pubkey::Pubkey, signature::Keypair, transaction::Transaction};
use spl_associated_token_account;
use std::str::FromStr;
use std::sync::Arc;
use tracing::info;

/// AmmPoolService handles classic AMM pool creation operations
#[allow(dead_code)]
pub struct AmmPoolService {
    shared: Arc<SharedContext>,
}

impl AmmPoolService {
    /// Create a new AmmPoolService with shared context
    pub fn new(shared: Arc<SharedContext>) -> Self {
        Self { shared }
    }

    /// Create CPMM pool transaction (unsigned) - 100% faithful to CLI logic
    pub async fn create_classic_amm_pool(&self, request: CreateClassicAmmPoolRequest) -> Result<CreateClassicAmmPoolResponse> {
        info!("🏗️ 开始创建CPMM池子 (基于CLI逻辑)");
        info!("  Mint0: {}", request.mint0);
        info!("  Mint1: {}", request.mint1);
        info!("  初始数量0: {}", request.init_amount_0);
        info!("  初始数量1: {}", request.init_amount_1);
        info!("  开放时间: {}", request.open_time);

        // 解析mint地址
        let mut mint0 = Pubkey::from_str(&request.mint0)?;
        let mut mint1 = Pubkey::from_str(&request.mint1)?;
        let user_wallet = Pubkey::from_str(&request.user_wallet)?;

        // CLI逻辑第1步：排序代币 (确保mint0 < mint1)
        let (init_amount_0, init_amount_1) = if mint0 > mint1 {
            std::mem::swap(&mut mint0, &mut mint1);
            (request.init_amount_1, request.init_amount_0)
        } else {
            (request.init_amount_0, request.init_amount_1)
        };

        info!(
            "  排序后: mint0={}, mint1={}, amount0={}, amount1={}",
            mint0, mint1, init_amount_0, init_amount_1
        );

        // CLI逻辑第2步：获取代币程序信息
        let load_pubkeys = vec![mint0, mint1];
        let rsps = self.shared.rpc_client.get_multiple_accounts(&load_pubkeys)?;
        let token_0_program = rsps[0].as_ref().ok_or_else(|| anyhow::anyhow!("无法获取mint0账户信息"))?.owner;
        let token_1_program = rsps[1].as_ref().ok_or_else(|| anyhow::anyhow!("无法获取mint1账户信息"))?.owner;

        info!("  Token0程序: {}", token_0_program);
        info!("  Token1程序: {}", token_1_program);

        // CLI逻辑第3步：构建初始化指令（完全按照CLI的initialize_pool_instr逻辑）
        let instructions = self.build_initialize_pool_instructions(
            mint0,
            mint1,
            token_0_program,
            token_1_program,
            &user_wallet,
            init_amount_0,
            init_amount_1,
            request.open_time,
        )?;

        // 计算池子地址（用于响应）
        let pool_address = self.calculate_pool_address(mint0, mint1)?;

        // 创建交易
        let mut transaction = Transaction::new_with_payer(&instructions, Some(&user_wallet));

        // 获取最新的blockhash
        let recent_blockhash = self.shared.rpc_client.get_latest_blockhash()?;
        transaction.message.recent_blockhash = recent_blockhash;

        // 序列化交易为Base64
        let serialized_transaction = bincode::serialize(&transaction)?;
        let transaction_base64 = BASE64_STANDARD.encode(&serialized_transaction);

        let now = chrono::Utc::now().timestamp();

        info!("✅ CPMM池子交易构建成功");
        info!("  池子地址: {}", pool_address);

        Ok(CreateClassicAmmPoolResponse {
            transaction: transaction_base64,
            transaction_message: "创建CPMM池子交易".to_string(),
            pool_address: pool_address.to_string(),
            coin_mint: mint0.to_string(),
            pc_mint: mint1.to_string(),
            coin_vault: "待计算".to_string(), // CPMM中的vault地址需要从池子状态获取
            pc_vault: "待计算".to_string(),
            lp_mint: "待计算".to_string(),
            open_orders: "N/A".to_string(),    // CPMM没有open orders
            target_orders: "N/A".to_string(),  // CPMM没有target orders
            withdraw_queue: "N/A".to_string(), // CPMM没有withdraw queue
            init_coin_amount: init_amount_0,
            init_pc_amount: init_amount_1,
            open_time: request.open_time,
            timestamp: now,
        })
    }

    /// Create CPMM pool and send transaction - 100% faithful to CLI logic
    pub async fn create_classic_amm_pool_and_send_transaction(
        &self,
        request: CreateClassicAmmPoolRequest,
    ) -> Result<CreateClassicAmmPoolAndSendTransactionResponse> {
        info!("🚀 开始创建CPMM池子并发送交易 (基于CLI逻辑)");
        info!("  Mint0: {}", request.mint0);
        info!("  Mint1: {}", request.mint1);
        info!("  初始数量0: {}", request.init_amount_0);
        info!("  初始数量1: {}", request.init_amount_1);
        info!("  开放时间: {}", request.open_time);

        // 解析mint地址
        let mut mint0 = Pubkey::from_str(&request.mint0)?;
        let mut mint1 = Pubkey::from_str(&request.mint1)?;
        let user_wallet = Pubkey::from_str(&request.user_wallet)?;

        // CLI逻辑第1步：排序代币 (确保mint0 < mint1)
        let (init_amount_0, init_amount_1) = if mint0 > mint1 {
            std::mem::swap(&mut mint0, &mut mint1);
            (request.init_amount_1, request.init_amount_0)
        } else {
            (request.init_amount_0, request.init_amount_1)
        };

        info!(
            "  排序后: mint0={}, mint1={}, amount0={}, amount1={}",
            mint0, mint1, init_amount_0, init_amount_1
        );

        // 从环境配置中获取私钥
        let private_key = self
            .shared
            .app_config
            .private_key
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("私钥未配置，请检查 .env.development 文件中的 PRIVATE_KEY"))?;

        // 使用正确的Base58解码方法
        let user_keypair = Keypair::from_base58_string(private_key);

        // CLI逻辑第2步：获取代币程序信息
        let load_pubkeys = vec![mint0, mint1];
        let rsps = self.shared.rpc_client.get_multiple_accounts(&load_pubkeys)?;
        let token_0_program = rsps[0].as_ref().ok_or_else(|| anyhow::anyhow!("无法获取mint0账户信息"))?.owner;
        let token_1_program = rsps[1].as_ref().ok_or_else(|| anyhow::anyhow!("无法获取mint1账户信息"))?.owner;

        info!("  Token0程序: {}", token_0_program);
        info!("  Token1程序: {}", token_1_program);

        // CLI逻辑第3步：构建初始化指令
        let instructions = self.build_initialize_pool_instructions(
            mint0,
            mint1,
            token_0_program,
            token_1_program,
            &user_wallet,
            init_amount_0,
            init_amount_1,
            request.open_time,
        )?;

        // 计算池子地址
        let pool_address = self.calculate_pool_address(mint0, mint1)?;

        // CLI逻辑第4步：构建并发送交易（完全按照CLI逻辑）
        let signers = vec![&user_keypair];
        let recent_hash = self.shared.rpc_client.get_latest_blockhash()?;
        let transaction = Transaction::new_signed_with_payer(&instructions, Some(&user_wallet), &signers, recent_hash);

        // 发送交易（使用CLI中的send_txn逻辑）
        let signature = self.shared.rpc_client.send_and_confirm_transaction(&transaction)?;

        info!("✅ 创建CPMM池子成功，交易签名: {}", signature);

        // 构建响应
        let explorer_url = format!("https://explorer.solana.com/tx/{}", signature);
        let now = chrono::Utc::now().timestamp();

        Ok(CreateClassicAmmPoolAndSendTransactionResponse {
            signature: signature.to_string(),
            pool_address: pool_address.to_string(),
            coin_mint: mint0.to_string(),
            pc_mint: mint1.to_string(),
            coin_vault: "待计算".to_string(), // CPMM中的vault需要从池子状态获取
            pc_vault: "待计算".to_string(),
            lp_mint: "待计算".to_string(),
            open_orders: "N/A".to_string(),    // CPMM没有open orders
            target_orders: "N/A".to_string(),  // CPMM没有target orders
            withdraw_queue: "N/A".to_string(), // CPMM没有withdraw queue
            actual_coin_amount: init_amount_0,
            actual_pc_amount: init_amount_1,
            open_time: request.open_time,
            status: TransactionStatus::Finalized,
            explorer_url,
            timestamp: now,
        })
    }

    /// Build initialize pool instructions - faithful to CLI initialize_pool_instr logic
    fn build_initialize_pool_instructions(
        &self,
        token_0_mint: Pubkey,
        token_1_mint: Pubkey,
        token_0_program: Pubkey,
        token_1_program: Pubkey,
        user_wallet: &Pubkey,
        init_amount_0: u64,
        init_amount_1: u64,
        open_time: u64,
    ) -> Result<Vec<Instruction>> {
        // 获取CPMM程序ID（从配置或常量）
        let raydium_cp_program = self.get_raydium_cp_program_id()?;

        // 计算所有必要的PDA (完全按照CLI逻辑)
        let amm_config_index: u16 = std::env::var("AMM_CONFIG_INDEX").unwrap_or("0".to_string()).parse()?;

        // AMM配置账户
        let (amm_config_key, _) = Pubkey::find_program_address(&["amm_config".as_bytes(), &amm_config_index.to_be_bytes()], &raydium_cp_program);

        // 池子状态账户
        let (pool_account_key, _) = Pubkey::find_program_address(
            &[
                "pool".as_bytes(),
                amm_config_key.to_bytes().as_ref(),
                token_0_mint.to_bytes().as_ref(),
                token_1_mint.to_bytes().as_ref(),
            ],
            &raydium_cp_program,
        );

        // 权限账户
        let (authority, _) = Pubkey::find_program_address(&["vault_and_lp_mint_auth_seed".as_bytes()], &raydium_cp_program);

        // 代币金库账户
        let (token_0_vault, _) = Pubkey::find_program_address(
            &["pool_vault".as_bytes(), pool_account_key.to_bytes().as_ref(), token_0_mint.to_bytes().as_ref()],
            &raydium_cp_program,
        );

        let (token_1_vault, _) = Pubkey::find_program_address(
            &["pool_vault".as_bytes(), pool_account_key.to_bytes().as_ref(), token_1_mint.to_bytes().as_ref()],
            &raydium_cp_program,
        );

        // LP代币铸造账户
        let (lp_mint_key, _) = Pubkey::find_program_address(&["pool_lp_mint".as_bytes(), pool_account_key.to_bytes().as_ref()], &raydium_cp_program);

        // 观察状态账户
        let (observation_key, _) = Pubkey::find_program_address(&["observation".as_bytes(), pool_account_key.to_bytes().as_ref()], &raydium_cp_program);

        // 用户关联代币账户
        let creator_token_0 = spl_associated_token_account::get_associated_token_address(user_wallet, &token_0_mint);
        let creator_token_1 = spl_associated_token_account::get_associated_token_address(user_wallet, &token_1_mint);
        let creator_lp_token = spl_associated_token_account::get_associated_token_address(user_wallet, &lp_mint_key);

        // 创建池子费用接收者（CLI中使用的常量）
        let create_pool_fee = self.get_create_pool_fee_receiver_id()?;

        info!("🔧 构建CPMM初始化指令:");
        info!("  AMM配置: {}", amm_config_key);
        info!("  池子地址: {}", pool_account_key);
        info!("  权限地址: {}", authority);
        info!("  Token0金库: {}", token_0_vault);
        info!("  Token1金库: {}", token_1_vault);
        info!("  LP代币: {}", lp_mint_key);
        info!("  观察状态: {}", observation_key);

        // 构建Initialize指令的账户（按照CLI中raydium_cp_accounts::Initialize的顺序）
        let accounts = vec![
            solana_sdk::instruction::AccountMeta::new(*user_wallet, true),              // creator (signer)
            solana_sdk::instruction::AccountMeta::new_readonly(amm_config_key, false),  // amm_config
            solana_sdk::instruction::AccountMeta::new_readonly(authority, false),       // authority
            solana_sdk::instruction::AccountMeta::new(pool_account_key, false),         // pool_state
            solana_sdk::instruction::AccountMeta::new_readonly(token_0_mint, false),    // token_0_mint
            solana_sdk::instruction::AccountMeta::new_readonly(token_1_mint, false),    // token_1_mint
            solana_sdk::instruction::AccountMeta::new(lp_mint_key, false),              // lp_mint
            solana_sdk::instruction::AccountMeta::new(creator_token_0, false),          // creator_token_0
            solana_sdk::instruction::AccountMeta::new(creator_token_1, false),          // creator_token_1
            solana_sdk::instruction::AccountMeta::new(creator_lp_token, false),         // creator_lp_token
            solana_sdk::instruction::AccountMeta::new(token_0_vault, false),            // token_0_vault
            solana_sdk::instruction::AccountMeta::new(token_1_vault, false),            // token_1_vault
            solana_sdk::instruction::AccountMeta::new(create_pool_fee, false),          // create_pool_fee
            solana_sdk::instruction::AccountMeta::new(observation_key, false),          // observation_state
            solana_sdk::instruction::AccountMeta::new_readonly(spl_token::id(), false), // token_program
            solana_sdk::instruction::AccountMeta::new_readonly(token_0_program, false), // token_0_program
            solana_sdk::instruction::AccountMeta::new_readonly(token_1_program, false), // token_1_program
            solana_sdk::instruction::AccountMeta::new_readonly(spl_associated_token_account::id(), false), // associated_token_program
            solana_sdk::instruction::AccountMeta::new_readonly(solana_sdk::system_program::id(), false), // system_program
            solana_sdk::instruction::AccountMeta::new_readonly(solana_sdk::sysvar::rent::id(), false), // rent
        ];

        // 构建指令数据（CLI中的raydium_cp_instructions::Initialize参数）
        let instruction_data = self.build_initialize_instruction_data(init_amount_0, init_amount_1, open_time)?;

        let instruction = Instruction {
            program_id: raydium_cp_program,
            accounts,
            data: instruction_data,
        };

        Ok(vec![instruction])
    }

    /// Build initialize instruction data - faithful to CLI logic
    fn build_initialize_instruction_data(&self, init_amount_0: u64, init_amount_1: u64, open_time: u64) -> Result<Vec<u8>> {
        // CPMM Initialize指令的discriminator (需要根据实际程序确定)
        // 这里使用一个通用的discriminator，实际使用时可能需要调整
        // let discriminator: [u8; 8] = [95, 180, 10, 172, 84, 174, 232, 40]; // initialize指令的discriminator
        let discriminator = instruction::Initialize::DISCRIMINATOR;

        let mut data = Vec::new();
        data.extend_from_slice(&discriminator);
        data.extend_from_slice(&init_amount_0.to_le_bytes());
        data.extend_from_slice(&init_amount_1.to_le_bytes());
        data.extend_from_slice(&open_time.to_le_bytes());

        info!("🔧 构建的CPMM指令数据长度: {} bytes", data.len());

        Ok(data)
    }

    /// Calculate pool address - faithful to CLI PDA calculation
    fn calculate_pool_address(&self, token_0_mint: Pubkey, token_1_mint: Pubkey) -> Result<Pubkey> {
        let raydium_cp_program = self.get_raydium_cp_program_id()?;
        let amm_config_index: u16 = 0;

        let (amm_config_key, _) = Pubkey::find_program_address(&["amm_config".as_bytes(), &amm_config_index.to_be_bytes()], &raydium_cp_program);

        let (pool_account_key, _) = Pubkey::find_program_address(
            &[
                "pool".as_bytes(),
                amm_config_key.to_bytes().as_ref(),
                token_0_mint.to_bytes().as_ref(),
                token_1_mint.to_bytes().as_ref(),
            ],
            &raydium_cp_program,
        );

        Ok(pool_account_key)
    }

    /// Get Raydium CP program ID from configuration
    fn get_raydium_cp_program_id(&self) -> Result<Pubkey> {
        // 从配置中获取，或使用默认值
        let program_id_str = std::env::var("RAYDIUM_CP_PROGRAM_ID").unwrap_or_else(|_| "CPMMoo8L3F4NbTegBCKVNunggL7H1ZpdTHKxQB5qKP1C".to_string());
        info!("🔍 获取CPMM程序ID: {}", program_id_str);
        Pubkey::from_str(&program_id_str).map_err(Into::into)
    }

    /// Get create pool fee receiver ID
    fn get_create_pool_fee_receiver_id(&self) -> Result<Pubkey> {
        // CLI中使用的费用接收者ID
        Pubkey::from_str("7YttLkHDoNj9wyDur5pM1ejNaAvT9X4eqaYcHQqtj2G5").map_err(Into::into)
    }
}
