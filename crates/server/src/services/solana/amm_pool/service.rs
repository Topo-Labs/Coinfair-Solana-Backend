// AmmPoolService handles classic AMM pool creation operations

use crate::dtos::solana_dto::{CreateClassicAmmPoolAndSendTransactionResponse, CreateClassicAmmPoolRequest, CreateClassicAmmPoolResponse, TransactionStatus};

use super::super::shared::SharedContext;
use ::utils::solana::ClassicAmmInstructionBuilder;
use anyhow::Result;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use solana_sdk::{pubkey::Pubkey, signature::Keypair, transaction::Transaction};
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

    /// Create classic AMM pool transaction (unsigned) - moved from original SolanaService
    pub async fn create_classic_amm_pool(&self, request: CreateClassicAmmPoolRequest) -> Result<CreateClassicAmmPoolResponse> {
        info!("🏗️ 开始创建经典AMM池子");
        info!("  Mint0: {}", request.mint0);
        info!("  Mint1: {}", request.mint1);
        info!("  初始数量0: {}", request.init_amount_0);
        info!("  初始数量1: {}", request.init_amount_1);
        info!("  开放时间: {}", request.open_time);

        // 解析mint地址
        let mint0 = Pubkey::from_str(&request.mint0)?;
        let mint1 = Pubkey::from_str(&request.mint1)?;
        let user_wallet = Pubkey::from_str(&request.user_wallet)?;

        // 使用ClassicAmmInstructionBuilder构建指令
        let instructions = ClassicAmmInstructionBuilder::build_initialize_instruction(
            &user_wallet,
            &mint0,
            &mint1,
            request.init_amount_0,
            request.init_amount_1,
            request.open_time,
        )?;

        // 获取所有相关地址
        let addresses = ClassicAmmInstructionBuilder::get_all_v2_amm_addresses(&mint0, &mint1)?;

        // 创建交易
        let mut transaction = Transaction::new_with_payer(&instructions, Some(&user_wallet));

        // 获取最新的blockhash
        let recent_blockhash = self.shared.rpc_client.get_latest_blockhash()?;
        transaction.message.recent_blockhash = recent_blockhash;

        // 序列化交易为Base64
        let serialized_transaction = bincode::serialize(&transaction)?;
        let transaction_base64 = BASE64_STANDARD.encode(&serialized_transaction);

        let now = chrono::Utc::now().timestamp();

        info!("✅ 经典AMM池子交易构建成功");
        info!("  池子地址: {}", addresses.pool_id);
        info!("  Coin Mint: {}", addresses.coin_mint);
        info!("  PC Mint: {}", addresses.pc_mint);

        Ok(CreateClassicAmmPoolResponse {
            transaction: transaction_base64,
            transaction_message: "创建经典AMM池子交易".to_string(),
            pool_address: addresses.pool_id.to_string(),
            coin_mint: addresses.coin_mint.to_string(),
            pc_mint: addresses.pc_mint.to_string(),
            coin_vault: addresses.coin_vault.to_string(),
            pc_vault: addresses.pc_vault.to_string(),
            lp_mint: addresses.lp_mint.to_string(),
            open_orders: addresses.open_orders.to_string(),
            target_orders: addresses.target_orders.to_string(),
            withdraw_queue: addresses.withdraw_queue.to_string(),
            init_coin_amount: if mint0.to_bytes() < mint1.to_bytes() {
                request.init_amount_0
            } else {
                request.init_amount_1
            },
            init_pc_amount: if mint0.to_bytes() < mint1.to_bytes() {
                request.init_amount_1
            } else {
                request.init_amount_0
            },
            open_time: request.open_time,
            timestamp: now,
        })
    }

    /// Create classic AMM pool and send transaction - moved from original SolanaService
    pub async fn create_classic_amm_pool_and_send_transaction(
        &self,
        request: CreateClassicAmmPoolRequest,
    ) -> Result<CreateClassicAmmPoolAndSendTransactionResponse> {
        info!("🚀 开始创建经典AMM池子并发送交易");
        info!("  Mint0: {}", request.mint0);
        info!("  Mint1: {}", request.mint1);
        info!("  初始数量0: {}", request.init_amount_0);
        info!("  初始数量1: {}", request.init_amount_1);
        info!("  开放时间: {}", request.open_time);

        // 解析mint地址
        let mint0 = Pubkey::from_str(&request.mint0)?;
        let mint1 = Pubkey::from_str(&request.mint1)?;
        let user_wallet = Pubkey::from_str(&request.user_wallet)?;

        // 从环境配置中获取私钥
        let private_key = self
            .shared
            .app_config
            .private_key
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("私钥未配置，请检查 .env.development 文件中的 PRIVATE_KEY"))?;

        // 使用正确的Base58解码方法
        let user_keypair = Keypair::from_base58_string(private_key);

        // 使用ClassicAmmInstructionBuilder构建指令
        let instructions = ClassicAmmInstructionBuilder::build_initialize_instruction(
            &user_wallet,
            &mint0,
            &mint1,
            request.init_amount_0,
            request.init_amount_1,
            request.open_time,
        )?;

        // 获取所有相关地址
        let addresses = ClassicAmmInstructionBuilder::get_all_v2_amm_addresses(&mint0, &mint1)?;

        // 构建并发送交易
        let recent_blockhash = self.shared.rpc_client.get_latest_blockhash()?;
        let transaction = Transaction::new_signed_with_payer(&instructions, Some(&user_wallet), &[&user_keypair], recent_blockhash);

        // 发送交易
        let signature = self.shared.rpc_client.send_and_confirm_transaction(&transaction)?;

        info!("✅ 创建经典AMM池子成功，交易签名: {}", signature);

        // 构建响应
        let explorer_url = format!("https://explorer.solana.com/tx/{}", signature);
        let now = chrono::Utc::now().timestamp();

        Ok(CreateClassicAmmPoolAndSendTransactionResponse {
            signature: signature.to_string(),
            pool_address: addresses.pool_id.to_string(),
            coin_mint: addresses.coin_mint.to_string(),
            pc_mint: addresses.pc_mint.to_string(),
            coin_vault: addresses.coin_vault.to_string(),
            pc_vault: addresses.pc_vault.to_string(),
            lp_mint: addresses.lp_mint.to_string(),
            open_orders: addresses.open_orders.to_string(),
            target_orders: addresses.target_orders.to_string(),
            withdraw_queue: addresses.withdraw_queue.to_string(),
            actual_coin_amount: if mint0.to_bytes() < mint1.to_bytes() {
                request.init_amount_0
            } else {
                request.init_amount_1
            },
            actual_pc_amount: if mint0.to_bytes() < mint1.to_bytes() {
                request.init_amount_1
            } else {
                request.init_amount_0
            },
            open_time: request.open_time,
            status: TransactionStatus::Finalized,
            explorer_url,
            timestamp: now,
        })
    }
}
