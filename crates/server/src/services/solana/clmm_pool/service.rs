// ClmmPoolService handles CLMM pool creation operations

use crate::dtos::solana_dto::{CreatePoolAndSendTransactionResponse, CreatePoolRequest, CreatePoolResponse, TransactionStatus};

use super::super::shared::SharedContext;
use anyhow::Result;
use solana_sdk::{program_pack::Pack, pubkey::Pubkey, signature::Keypair, transaction::Transaction};
use spl_token::state::Mint;
use std::str::FromStr;
use std::sync::Arc;
use tracing::info;

/// ClmmPoolService handles CLMM pool creation operations
pub struct ClmmPoolService {
    shared: Arc<SharedContext>,
}

impl ClmmPoolService {
    /// Create a new ClmmPoolService with shared context
    pub fn new(shared: Arc<SharedContext>) -> Self {
        Self { shared }
    }

    /// Create CLMM pool transaction (unsigned)
    pub async fn create_pool(&self, request: CreatePoolRequest) -> Result<CreatePoolResponse> {
        info!("🏗️ 开始构建创建池子交易");
        info!("  配置索引: {}", request.config_index);
        info!("  初始价格: {}", request.price);
        info!("  Mint0: {}", request.mint0);
        info!("  Mint1: {}", request.mint1);
        info!("  开放时间: {}", request.open_time);

        // 1. 解析和验证参数
        let mut mint0 = Pubkey::from_str(&request.mint0)?;
        let mut mint1 = Pubkey::from_str(&request.mint1)?;
        let mut price = request.price;
        let user_wallet = Pubkey::from_str(&request.user_wallet)?;

        // 2. 确保mint0 < mint1的顺序，如果不是则交换并调整价格
        if mint0 > mint1 {
            std::mem::swap(&mut mint0, &mut mint1);
            price = 1.0 / price;
            info!("  🔄 交换mint顺序，调整后价格: {}", price);
        }

        info!("  最终参数:");
        info!("    Mint0: {}", mint0);
        info!("    Mint1: {}", mint1);
        info!("    调整后价格: {}", price);

        // 3. 批量加载mint账户信息
        let load_pubkeys = vec![mint0, mint1];
        let rsps = self.shared.rpc_client.get_multiple_accounts(&load_pubkeys)?;

        let mint0_account = rsps[0].as_ref().ok_or_else(|| anyhow::anyhow!("Mint0账户不存在"))?;
        let mint1_account = rsps[1].as_ref().ok_or_else(|| anyhow::anyhow!("Mint1账户不存在"))?;

        let mint0_owner = mint0_account.owner;
        let mint1_owner = mint1_account.owner;

        // 4. 解析mint信息获取decimals
        let mint0_state = Mint::unpack(&mint0_account.data)?;
        let mint1_state = Mint::unpack(&mint1_account.data)?;

        info!("  Mint信息:");
        info!("    Mint0 decimals: {}, owner: {}", mint0_state.decimals, mint0_owner);
        info!("    Mint1 decimals: {}, owner: {}", mint1_state.decimals, mint1_owner);

        // 5. 计算sqrt_price_x64
        let sqrt_price_x64 = self.calculate_sqrt_price_x64(price, mint0_state.decimals, mint1_state.decimals);

        // 6. 计算对应的tick
        let tick = raydium_amm_v3::libraries::tick_math::get_tick_at_sqrt_price(sqrt_price_x64)?;

        info!("  价格计算结果:");
        info!("    sqrt_price_x64: {}", sqrt_price_x64);
        info!("    对应tick: {}", tick);

        // 7. 获取所有相关的PDA地址
        let pool_addresses = ::utils::solana::PoolInstructionBuilder::get_all_pool_addresses(request.config_index, &mint0, &mint1)?;

        info!("  计算的地址:");
        info!("    池子地址: {}", pool_addresses.pool);
        info!("    AMM配置: {}", pool_addresses.amm_config);
        info!("    Token0 Vault: {}", pool_addresses.token_vault_0);
        info!("    Token1 Vault: {}", pool_addresses.token_vault_1);

        // 8. 构建CreatePool指令
        let instructions = ::utils::solana::PoolInstructionBuilder::build_create_pool_instruction(
            &user_wallet,
            request.config_index,
            &mint0,
            &mint1,
            &mint0_owner,
            &mint1_owner,
            sqrt_price_x64,
            request.open_time,
        )?;

        // 9. 构建未签名交易
        let service_helpers = self.shared.create_service_helpers();
        let result_json = service_helpers.build_transaction_data(instructions, &user_wallet)?;
        let transaction_base64 = result_json["transaction"].as_str().unwrap_or_default().to_string();

        info!("✅ 创建池子交易构建成功");

        // 10. 构建交易消息摘要
        let transaction_message = format!(
            "创建池子 - 配置索引: {}, 价格: {:.6}, Mint0: {}..., Mint1: {}...",
            request.config_index,
            price,
            &request.mint0[..8],
            &request.mint1[..8]
        );

        let now = chrono::Utc::now().timestamp();

        Ok(CreatePoolResponse {
            transaction: transaction_base64,
            transaction_message,
            pool_address: pool_addresses.pool.to_string(),
            amm_config_address: pool_addresses.amm_config.to_string(),
            token_vault_0: pool_addresses.token_vault_0.to_string(),
            token_vault_1: pool_addresses.token_vault_1.to_string(),
            observation_address: pool_addresses.observation.to_string(),
            tickarray_bitmap_extension: pool_addresses.tick_array_bitmap.to_string(),
            initial_price: price,
            sqrt_price_x64: sqrt_price_x64.to_string(),
            initial_tick: tick,
            timestamp: now,
        })
    }

    /// Create CLMM pool and send transaction (signed)
    pub async fn create_pool_and_send_transaction(&self, request: CreatePoolRequest) -> Result<CreatePoolAndSendTransactionResponse> {
        info!("🏗️ 开始创建池子并发送交易");
        info!("  配置索引: {}", request.config_index);
        info!("  初始价格: {}", request.price);
        info!("  Mint0: {}", request.mint0);
        info!("  Mint1: {}", request.mint1);

        // 1. 解析和验证参数
        let mut mint0 = Pubkey::from_str(&request.mint0)?;
        let mut mint1 = Pubkey::from_str(&request.mint1)?;
        let mut price = request.price;
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

        // 2. 确保mint0 < mint1的顺序，如果不是则交换并调整价格
        if mint0 > mint1 {
            std::mem::swap(&mut mint0, &mut mint1);
            price = 1.0 / price;
            info!("  🔄 交换mint顺序，调整后价格: {}", price);
        }

        // 3. 批量加载mint账户信息
        let load_pubkeys = vec![mint0, mint1];
        let rsps = self.shared.rpc_client.get_multiple_accounts(&load_pubkeys)?;

        let mint0_account = rsps[0].as_ref().ok_or_else(|| anyhow::anyhow!("Mint0账户不存在"))?;
        let mint1_account = rsps[1].as_ref().ok_or_else(|| anyhow::anyhow!("Mint1账户不存在"))?;

        let mint0_owner = mint0_account.owner;
        let mint1_owner = mint1_account.owner;

        // 4. 解析mint信息获取decimals
        let mint0_state = Mint::unpack(&mint0_account.data)?;
        let mint1_state = Mint::unpack(&mint1_account.data)?;

        // 5. 计算sqrt_price_x64
        let sqrt_price_x64 = self.calculate_sqrt_price_x64(price, mint0_state.decimals, mint1_state.decimals);

        // 6. 计算对应的tick
        let tick = raydium_amm_v3::libraries::tick_math::get_tick_at_sqrt_price(sqrt_price_x64)?;

        // 7. 获取所有相关的PDA地址
        let pool_addresses = ::utils::solana::PoolInstructionBuilder::get_all_pool_addresses(request.config_index, &mint0, &mint1)?;

        // 8. 构建CreatePool指令
        let instructions = ::utils::solana::PoolInstructionBuilder::build_create_pool_instruction(
            &user_wallet,
            request.config_index,
            &mint0,
            &mint1,
            &mint0_owner,
            &mint1_owner,
            sqrt_price_x64,
            request.open_time,
        )?;

        // 9. 构建并发送交易
        let recent_blockhash = self.shared.rpc_client.get_latest_blockhash()?;
        let transaction = Transaction::new_signed_with_payer(&instructions, Some(&user_wallet), &[&user_keypair], recent_blockhash);

        // 10. 发送交易
        let signature = self.shared.rpc_client.send_and_confirm_transaction(&transaction)?;

        info!("✅ 创建池子成功，交易签名: {}", signature);

        // 11. 构建响应
        let explorer_url = format!("https://explorer.solana.com/tx/{}", signature);
        let now = chrono::Utc::now().timestamp();

        Ok(CreatePoolAndSendTransactionResponse {
            signature: signature.to_string(),
            pool_address: pool_addresses.pool.to_string(),
            amm_config_address: pool_addresses.amm_config.to_string(),
            token_vault_0: pool_addresses.token_vault_0.to_string(),
            token_vault_1: pool_addresses.token_vault_1.to_string(),
            observation_address: pool_addresses.observation.to_string(),
            tickarray_bitmap_extension: pool_addresses.tick_array_bitmap.to_string(),
            initial_price: price,
            sqrt_price_x64: sqrt_price_x64.to_string(),
            initial_tick: tick,
            status: TransactionStatus::Finalized,
            explorer_url,
            timestamp: now,
        })
    }

    /// Calculate sqrt_price_x64 (reusing CLI logic)
    fn calculate_sqrt_price_x64(&self, price: f64, decimals_0: u8, decimals_1: u8) -> u128 {
        // 使用与CLI完全相同的计算逻辑
        let multipler = |decimals: u8| -> f64 { (10_i32).checked_pow(decimals.try_into().unwrap()).unwrap() as f64 };

        let price_to_x64 = |price: f64| -> u128 { (price * raydium_amm_v3::libraries::fixed_point_64::Q64 as f64) as u128 };

        let price_with_decimals = price * multipler(decimals_1) / multipler(decimals_0);
        price_to_x64(price_with_decimals.sqrt())
    }
}
