// ClmmPoolService handles CLMM pool creation operations

use crate::dtos::solana_dto::{CreatePoolAndSendTransactionResponse, CreatePoolRequest, CreatePoolResponse, TransactionStatus};

use super::super::shared::SharedContext;
use super::storage::{ClmmPoolStorageBuilder, ClmmPoolStorageService};
use anyhow::Result;
use solana_sdk::{program_pack::Pack, pubkey::Pubkey, signature::Keypair, transaction::Transaction};
use spl_token::state::Mint;
use std::str::FromStr;
use std::sync::Arc;
use tracing::info;

/// ClmmPoolService handles CLMM pool creation operations
pub struct ClmmPoolService {
    shared: Arc<SharedContext>,
    storage: ClmmPoolStorageService,
}

impl ClmmPoolService {
    /// Create a new ClmmPoolService with shared context and database
    pub fn new(shared: Arc<SharedContext>, database: &database::Database) -> Self {
        let storage = ClmmPoolStorageBuilder::from_database(database);
        Self { shared, storage }
    }

    /// Create CLMM pool transaction (unsigned)
    pub async fn create_pool(&self, request: CreatePoolRequest) -> Result<CreatePoolResponse> {
        info!("🏗️ 开始构建创建池子交易");
        info!("  配置索引: {}", request.config_index);
        info!("  初始价格: {}", request.price);
        info!("  Mint0: {}", request.mint0);
        info!("  Mint1: {}", request.mint1);
        info!("  开放时间: {}", request.open_time);

        // 1. 输入参数验证
        self.validate_create_pool_request(&request)?;

        // 2. 解析和验证参数
        let mut price = request.price;
        let mut mint0 = Pubkey::from_str(&request.mint0).map_err(|_| anyhow::anyhow!("无效的mint0地址"))?;
        let mut mint1 = Pubkey::from_str(&request.mint1).map_err(|_| anyhow::anyhow!("无效的mint1地址"))?;
        let user_wallet = Pubkey::from_str(&request.user_wallet).map_err(|_| anyhow::anyhow!("无效的用户钱包地址"))?;

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

        let response = CreatePoolResponse {
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
        };

        // 11. 存储池子元数据到数据库
        match self.storage.store_pool_creation(&request, &response).await {
            Ok(pool_id) => {
                info!("💾 池子元数据存储成功，ID: {}", pool_id);
            }
            Err(e) => {
                // 存储失败不影响交易构建，只记录错误
                tracing::error!("❌ 池子元数据存储失败: {}", e);
            }
        }

        Ok(response)
    }

    /// Create CLMM pool and send transaction (signed just for local testing purposes, will not be used in production)
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

        let response = CreatePoolAndSendTransactionResponse {
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
            explorer_url: explorer_url.clone(),
            timestamp: now,
        };

        // 12. 存储池子元数据和交易信息到数据库
        match self.storage.store_pool_creation_with_transaction(&request, &response).await {
            Ok(pool_id) => {
                info!("💾 池子元数据和交易信息存储成功，ID: {}", pool_id);
            }
            Err(e) => {
                // 存储失败不影响交易执行，只记录错误
                tracing::error!("❌ 池子元数据存储失败: {}", e);
            }
        }

        Ok(response)
    }

    /// 根据池子地址查询池子信息
    pub async fn get_pool_by_address(&self, pool_address: &str) -> Result<Option<database::clmm_pool::ClmmPool>> {
        info!("🔍 查询池子信息: {}", pool_address);

        match self.storage.get_pool_by_address(pool_address).await {
            Ok(pool) => {
                if pool.is_some() {
                    info!("✅ 找到池子信息: {}", pool_address);
                } else {
                    info!("⚠️ 未找到池子信息: {}", pool_address);
                }
                Ok(pool)
            }
            Err(e) => {
                tracing::error!("❌ 查询池子信息失败: {} - {}", pool_address, e);
                Err(e.into())
            }
        }
    }

    /// 根据代币mint地址查询相关池子列表
    pub async fn get_pools_by_mint(&self, mint_address: &str, limit: Option<i64>) -> Result<Vec<database::clmm_pool::ClmmPool>> {
        info!("🔍 查询代币相关池子: {} (限制: {:?})", mint_address, limit);

        match self.storage.get_pools_by_mint(mint_address, limit).await {
            Ok(pools) => {
                info!("✅ 找到 {} 个相关池子", pools.len());
                Ok(pools)
            }
            Err(e) => {
                tracing::error!("❌ 查询代币相关池子失败: {} - {}", mint_address, e);
                Err(e.into())
            }
        }
    }

    /// 根据创建者查询池子列表
    pub async fn get_pools_by_creator(&self, creator_wallet: &str, limit: Option<i64>) -> Result<Vec<database::clmm_pool::ClmmPool>> {
        info!("🔍 查询创建者池子: {} (限制: {:?})", creator_wallet, limit);

        match self.storage.get_pools_by_creator(creator_wallet, limit).await {
            Ok(pools) => {
                info!("✅ 找到 {} 个创建者池子", pools.len());
                Ok(pools)
            }
            Err(e) => {
                tracing::error!("❌ 查询创建者池子失败: {} - {}", creator_wallet, e);
                Err(e.into())
            }
        }
    }

    /// 复杂查询接口
    pub async fn query_pools(&self, params: &database::clmm_pool::PoolQueryParams) -> Result<Vec<database::clmm_pool::ClmmPool>> {
        info!("🔍 执行复杂池子查询");

        match self.storage.query_pools(params).await {
            Ok(pools) => {
                info!("✅ 查询完成，找到 {} 个池子", pools.len());
                Ok(pools)
            }
            Err(e) => {
                tracing::error!("❌ 复杂查询失败: {}", e);
                Err(e.into())
            }
        }
    }

    /// 获取池子统计信息
    pub async fn get_pool_statistics(&self) -> Result<database::clmm_pool::PoolStats> {
        info!("📊 获取池子统计信息");

        match self.storage.get_pool_statistics().await {
            Ok(stats) => {
                info!("✅ 统计信息获取成功 - 总池子: {}, 活跃池子: {}", stats.total_pools, stats.active_pools);
                Ok(stats)
            }
            Err(e) => {
                tracing::error!("❌ 获取统计信息失败: {}", e);
                Err(e.into())
            }
        }
    }

    /// 分页查询池子列表
    pub async fn query_pools_with_pagination(
        &self,
        params: &database::clmm_pool::model::PoolListRequest,
    ) -> Result<database::clmm_pool::model::PoolListResponse> {
        info!("📋 执行分页池子查询");
        info!("  池子类型: {:?}", params.pool_type);
        info!("  排序字段: {:?}", params.pool_sort_field);
        info!("  排序方向: {:?}", params.sort_type);
        info!("  页码: {}, 页大小: {}", params.page.unwrap_or(1), params.page_size.unwrap_or(20));

        match self.storage.query_pools_with_pagination(params).await {
            Ok(response) => {
                info!("✅ 分页查询完成，返回{}个池子", response.pools.len());
                Ok(response)
            }
            Err(e) => {
                tracing::error!("❌ 分页查询失败: {}", e);
                Err(e.into())
            }
        }
    }

    /// 初始化存储服务 (包括数据库索引)
    pub async fn init_storage(&self) -> Result<()> {
        info!("🔧 初始化CLMM池子存储服务...");

        match self.storage.init_indexes().await {
            Ok(_) => {
                info!("✅ 存储服务初始化完成");
                Ok(())
            }
            Err(e) => {
                tracing::error!("❌ 存储服务初始化失败: {}", e);
                Err(e.into())
            }
        }
    }

    /// 验证创建池子请求参数
    fn validate_create_pool_request(&self, request: &CreatePoolRequest) -> Result<()> {
        // 验证价格
        if request.price <= 0.0 {
            return Err(anyhow::anyhow!("价格必须大于0"));
        }
        if request.price.is_infinite() || request.price.is_nan() {
            return Err(anyhow::anyhow!("价格必须是有效的数值"));
        }
        if request.price > 1e18 {
            return Err(anyhow::anyhow!("价格过大，可能导致计算溢出"));
        }

        // 验证mint地址格式
        if request.mint0.len() < 32 || request.mint0.len() > 44 {
            return Err(anyhow::anyhow!("mint0地址格式不正确"));
        }
        if request.mint1.len() < 32 || request.mint1.len() > 44 {
            return Err(anyhow::anyhow!("mint1地址格式不正确"));
        }
        if request.mint0 == request.mint1 {
            return Err(anyhow::anyhow!("mint0和mint1不能相同"));
        }

        // 验证用户钱包地址格式
        if request.user_wallet.len() < 32 || request.user_wallet.len() > 44 {
            return Err(anyhow::anyhow!("用户钱包地址格式不正确"));
        }

        // 验证配置索引
        if request.config_index > 100 {
            return Err(anyhow::anyhow!("配置索引超出有效范围"));
        }

        // 验证开放时间
        let now = chrono::Utc::now().timestamp() as u64;
        if request.open_time > 0 && request.open_time < now && (now - request.open_time) > 86400 {
            return Err(anyhow::anyhow!("开放时间不能是过去超过24小时的时间"));
        }

        Ok(())
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
