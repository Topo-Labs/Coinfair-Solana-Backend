// LaunchMigrationService handles meme token launch migration to DEX pools

use crate::dtos::solana::clmm::launch::{
    DailyLaunchCount, LaunchMigrationAndSendTransactionResponse, LaunchMigrationRequest, LaunchMigrationResponse,
    LaunchMigrationStats, MigrationAddresses,
};

use crate::dtos::solana::common::TransactionStatus;

use crate::services::solana::clmm::ClmmConfigService;
use crate::services::solana::clmm::ClmmPoolService;
use crate::services::solana::clmm::liquidity::LiquidityService;
use crate::services::solana::clmm::position::PositionService;
use crate::services::solana::shared::SharedContext;

use ::utils::solana::{ConfigManager, PoolInstructionBuilder, PositionInstructionBuilder, PositionUtilsOptimized};

use anyhow::Result;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use mongodb::bson::doc;
use solana_sdk::instruction::AccountMeta;
use solana_sdk::program_pack::Pack;
use solana_sdk::{
    instruction::Instruction,
    message::Message,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use spl_token::state::Mint;
use std::str::FromStr;
use std::sync::Arc;
use tracing::info;

/// 发射迁移服务，负责协调池子创建、开仓和流动性注入的原子操作
#[allow(dead_code)]
pub struct LaunchMigrationService {
    shared: Arc<SharedContext>,
    database: Arc<database::Database>,
    clmm_pool_service: ClmmPoolService,
    position_service: PositionService,
    liquidity_service: LiquidityService,
}

impl LaunchMigrationService {
    /// 创建新的发射迁移服务实例
    pub fn new(shared: Arc<SharedContext>, database: &database::Database) -> Self {
        let database = Arc::new(database.clone());
        let config_service = Arc::new(ClmmConfigService::new(database.clone(), shared.rpc_client.clone()));
        let clmm_pool_service = ClmmPoolService::new(shared.clone(), database.as_ref(), config_service);
        let position_service = PositionService::with_database(shared.clone(), database.clone());
        let liquidity_service = LiquidityService::with_database(shared.clone(), database.clone());

        Self {
            shared,
            database,
            clmm_pool_service,
            position_service,
            liquidity_service,
        }
    }

    /// 构建发射迁移交易（不签名不发送）
    // #[instrument(skip(self), fields(user_wallet = %request.user_wallet))]
    pub async fn launch(&self, request: LaunchMigrationRequest) -> Result<LaunchMigrationResponse> {
        info!("🚀 开始构建发射迁移交易");
        info!("  Meme币: {}", request.meme_token_mint);
        info!("  配对币: {}", request.base_token_mint);
        info!("  初始价格: {}", request.initial_price);

        // 1. 参数验证
        self.validate_migration_request(&request)?;

        // 2. 构建所有指令
        let instructions = self.build_migration_instructions(&request).await?;

        // 3. 组合成原子交易
        let user_wallet = Pubkey::from_str(&request.user_wallet)?;
        let transaction_data = self.build_atomic_transaction(instructions, &user_wallet)?;

        // 4. 计算相关地址信息
        let addresses = self.calculate_migration_addresses(&request).await?;

        // 5. 构建响应
        let transaction_message = format!(
            "Meme币迁移 - 池子: {}..., 价格: {}, 流动性: {}",
            &addresses.pool_address[..8],
            request.initial_price,
            addresses.liquidity
        );

        let now = chrono::Utc::now().timestamp();

        let response = LaunchMigrationResponse {
            transaction: transaction_data,
            transaction_message,
            pool_address: addresses.pool_address,
            amm_config_address: addresses.amm_config_address,
            token_vault_0: addresses.token_vault_0,
            token_vault_1: addresses.token_vault_1,
            observation_address: addresses.observation_address,
            tickarray_bitmap_extension: addresses.tickarray_bitmap_extension,
            position_nft_mint: addresses.position_nft_mint,
            position_key: addresses.position_key,
            liquidity: addresses.liquidity.to_string(),
            initial_price: addresses.actual_initial_price,
            sqrt_price_x64: addresses.sqrt_price_x64.to_string(),
            initial_tick: addresses.initial_tick,
            tick_lower_index: addresses.tick_lower_index,
            tick_upper_index: addresses.tick_upper_index,
            amount_0: addresses.amount_0,
            amount_1: addresses.amount_1,
            timestamp: now,
        };

        // 异步持久化Launch Migration记录
        self.persist_launch_migration(&request, &response).await;

        info!("✅ 发射迁移交易构建成功");
        Ok(response)
    }

    /// 构建并发送发射迁移交易
    // #[instrument(skip(self), fields(user_wallet = %request.user_wallet))]
    pub async fn launch_and_send_transaction(
        &self,
        request: LaunchMigrationRequest,
    ) -> Result<LaunchMigrationAndSendTransactionResponse> {
        info!("🚀 开始发射迁移并发送交易");

        // 1. 参数验证
        self.validate_migration_request(&request)?;

        // 2. 获取私钥
        let private_key = self
            .shared
            .app_config
            .private_key
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("私钥未配置，请检查环境配置文件"))?;

        let user_keypair = Keypair::from_base58_string(private_key);
        let user_wallet = Pubkey::from_str(&request.user_wallet)?;

        // 3. 构建指令（这次需要同时返回nft_mint_keypair）
        let (instructions, addresses, nft_mint_keypair) =
            self.build_migration_instructions_with_keypair(&request).await?;

        // 4. 构建并发送交易
        let recent_blockhash = self.shared.rpc_client.get_latest_blockhash()?;

        let transaction = Transaction::new_signed_with_payer(
            &instructions,
            Some(&user_wallet),
            &[&user_keypair, &nft_mint_keypair],
            recent_blockhash,
        );

        // 5. 发送交易
        let signature = self.shared.rpc_client.send_and_confirm_transaction(&transaction)?;
        info!("✅ 发射迁移交易发送成功，签名: {}", signature);

        // 6. 构建响应
        let explorer_url = format!("https://explorer.solana.com/tx/{}", signature);
        let now = chrono::Utc::now().timestamp();

        let response = LaunchMigrationAndSendTransactionResponse {
            signature: signature.to_string(),
            status: TransactionStatus::Finalized,
            explorer_url,
            pool_address: addresses.pool_address,
            amm_config_address: addresses.amm_config_address,
            token_vault_0: addresses.token_vault_0,
            token_vault_1: addresses.token_vault_1,
            observation_address: addresses.observation_address,
            tickarray_bitmap_extension: addresses.tickarray_bitmap_extension,
            position_nft_mint: addresses.position_nft_mint,
            position_key: addresses.position_key,
            liquidity: addresses.liquidity.to_string(),
            initial_price: addresses.actual_initial_price,
            sqrt_price_x64: addresses.sqrt_price_x64.to_string(),
            initial_tick: addresses.initial_tick,
            tick_lower_index: addresses.tick_lower_index,
            tick_upper_index: addresses.tick_upper_index,
            amount_0: addresses.amount_0,
            amount_1: addresses.amount_1,
            timestamp: now,
        };

        // 异步持久化Launch Migration记录（发送交易版本）
        self.persist_launch_migration_with_transaction(&request, &response)
            .await;

        Ok(response)
    }

    /// 异步持久化Launch Migration记录
    async fn persist_launch_migration(&self, request: &LaunchMigrationRequest, response: &LaunchMigrationResponse) {
        let database = self.database.clone();
        let request_clone = request.clone();
        let response_clone = response.clone();

        // 使用tokio::spawn异步执行，不阻塞主流程
        tokio::spawn(async move {
            let result = Self::do_persist_launch_migration(&database, &request_clone, &response_clone).await;

            match result {
                Ok(_) => {
                    info!(
                        "✅ Launch Migration持久化成功: pool_address={}",
                        response_clone.pool_address
                    );
                }
                Err(e) => {
                    tracing::error!("❌ Launch Migration持久化失败: {}", e);
                    // 可以考虑重试机制或报警
                }
            }
        });
    }

    /// 执行Launch Migration持久化操作
    async fn do_persist_launch_migration(
        database: &database::Database,
        request: &LaunchMigrationRequest,
        response: &LaunchMigrationResponse,
    ) -> Result<()> {
        use database::clmm_pool::model::*;

        // 解析代币地址，确保mint0 < mint1的顺序
        let mut mint0_str = request.meme_token_mint.clone();
        let mut mint1_str = request.base_token_mint.clone();
        let mut initial_price = request.initial_price;

        let mint0_pubkey = Pubkey::from_str(&mint0_str)?;
        let mint1_pubkey = Pubkey::from_str(&mint1_str)?;

        // 如果需要交换顺序
        if mint0_pubkey > mint1_pubkey {
            std::mem::swap(&mut mint0_str, &mut mint1_str);
            initial_price = 1.0 / initial_price;
        }

        let pool_model = ClmmPool {
            id: None,
            pool_address: response.pool_address.clone(),
            amm_config_address: response.amm_config_address.clone(),
            config_index: request.config_index as u16,

            // 代币信息映射
            mint0: TokenInfo {
                mint_address: mint0_str,
                decimals: 0, // 初始值，后续链上同步补全
                owner: String::new(),
                symbol: None,
                name: None,
                log_uri: None,
                description: None,
                external_url: None,
                tags: None,
                attributes: None,
            },
            mint1: TokenInfo {
                mint_address: mint1_str,
                decimals: 0, // 初始值，后续链上同步补全
                owner: String::new(),
                symbol: None,
                name: None,
                log_uri: None,
                description: None,
                external_url: None,
                tags: None,
                attributes: None,
            },

            // 价格信息
            price_info: PriceInfo {
                initial_price,
                sqrt_price_x64: response.sqrt_price_x64.clone(),
                initial_tick: response.initial_tick,
                current_price: None,
                current_tick: None,
            },

            // 地址信息
            vault_info: VaultInfo {
                token_vault_0: response.token_vault_0.clone(),
                token_vault_1: response.token_vault_1.clone(),
            },
            extension_info: ExtensionInfo {
                observation_address: response.observation_address.clone(),
                tickarray_bitmap_extension: response.tickarray_bitmap_extension.clone(),
            },

            // 创建者和时间
            creator_wallet: request.user_wallet.clone(),
            open_time: request.open_time,
            api_created_at: response.timestamp as u64,
            api_created_slot: None,
            updated_at: chrono::Utc::now().timestamp() as u64,

            // 链上事件字段（初始为空，等待事件监听器填充）
            event_signature: None,
            event_updated_slot: None,
            event_confirmed_at: None,
            event_updated_at: None,

            // 交易信息（初始为空，仅构建交易时）
            transaction_info: None,

            // 状态管理
            status: PoolStatus::Created, // 初始状态：已创建交易
            sync_status: SyncStatus {
                last_sync_at: chrono::Utc::now().timestamp() as u64,
                sync_version: 1,
                needs_sync: true, // 需要同步代币元数据
                sync_error: None,
            },

            // 类型标识 - 关键区分字段
            pool_type: PoolType::Concentrated,
            data_source: DataSource::ApiCreated, // 标识为API创建
            chain_confirmed: false,
        };

        // 插入数据库
        database.clmm_pool_repository.insert_pool(pool_model).await?;

        info!("📝 Launch Migration记录已保存到数据库: {}", response.pool_address);
        Ok(())
    }

    /// 异步持久化Launch Migration记录（发送交易版本）
    async fn persist_launch_migration_with_transaction(
        &self,
        request: &LaunchMigrationRequest,
        response: &LaunchMigrationAndSendTransactionResponse,
    ) {
        let database = self.database.clone();
        let request_clone = request.clone();
        let response_clone = response.clone();

        // 使用tokio::spawn异步执行，不阻塞主流程
        tokio::spawn(async move {
            let result =
                Self::do_persist_launch_migration_with_transaction(&database, &request_clone, &response_clone).await;

            match result {
                Ok(_) => {
                    info!(
                        "✅ Launch Migration持久化成功（含交易）: pool_address={}, signature={}",
                        response_clone.pool_address, response_clone.signature
                    );
                }
                Err(e) => {
                    tracing::error!("❌ Launch Migration持久化失败（含交易）: {}", e);
                    // 可以考虑重试机制或报警
                }
            }
        });
    }

    /// 执行Launch Migration持久化操作（发送交易版本）
    async fn do_persist_launch_migration_with_transaction(
        database: &database::Database,
        request: &LaunchMigrationRequest,
        response: &LaunchMigrationAndSendTransactionResponse,
    ) -> Result<()> {
        use database::clmm_pool::model::*;

        // 解析代币地址，确保mint0 < mint1的顺序
        let mut mint0_str = request.meme_token_mint.clone();
        let mut mint1_str = request.base_token_mint.clone();
        let mut initial_price = request.initial_price;

        let mint0_pubkey = Pubkey::from_str(&mint0_str)?;
        let mint1_pubkey = Pubkey::from_str(&mint1_str)?;

        // 如果需要交换顺序
        if mint0_pubkey > mint1_pubkey {
            std::mem::swap(&mut mint0_str, &mut mint1_str);
            initial_price = 1.0 / initial_price;
        }

        let pool_model = ClmmPool {
            id: None,
            pool_address: response.pool_address.clone(),
            amm_config_address: response.amm_config_address.clone(),
            config_index: request.config_index as u16,

            // 代币信息映射
            mint0: TokenInfo {
                mint_address: mint0_str,
                decimals: 0, // 初始值，后续链上同步补全
                owner: String::new(),
                symbol: None,
                name: None,
                log_uri: None,
                description: None,
                external_url: None,
                tags: None,
                attributes: None,
            },
            mint1: TokenInfo {
                mint_address: mint1_str,
                decimals: 0, // 初始值，后续链上同步补全
                owner: String::new(),
                symbol: None,
                name: None,
                log_uri: None,
                description: None,
                external_url: None,
                tags: None,
                attributes: None,
            },

            // 价格信息
            price_info: PriceInfo {
                initial_price,
                sqrt_price_x64: response.sqrt_price_x64.clone(),
                initial_tick: response.initial_tick,
                current_price: None,
                current_tick: None,
            },

            // 地址信息
            vault_info: VaultInfo {
                token_vault_0: response.token_vault_0.clone(),
                token_vault_1: response.token_vault_1.clone(),
            },
            extension_info: ExtensionInfo {
                observation_address: response.observation_address.clone(),
                tickarray_bitmap_extension: response.tickarray_bitmap_extension.clone(),
            },

            // 创建者和时间
            creator_wallet: request.user_wallet.clone(),
            open_time: request.open_time,
            api_created_at: response.timestamp as u64,
            api_created_slot: None,
            updated_at: chrono::Utc::now().timestamp() as u64,

            // 链上事件字段（初始为空，等待事件监听器填充）
            event_signature: None,
            event_updated_slot: None,
            event_confirmed_at: None,
            event_updated_at: None,

            // 交易信息（包含已发送的交易）
            transaction_info: Some(database::clmm_pool::model::TransactionInfo {
                signature: response.signature.clone(),
                status: database::clmm_pool::model::TransactionStatus::Finalized,
                explorer_url: response.explorer_url.clone(),
                confirmed_at: response.timestamp as u64,
            }),

            // 状态管理
            status: PoolStatus::Pending, // 交易已发送，等待确认
            sync_status: SyncStatus {
                last_sync_at: chrono::Utc::now().timestamp() as u64,
                sync_version: 1,
                needs_sync: true, // 需要同步代币元数据
                sync_error: None,
            },

            // 类型标识 - 关键区分字段
            pool_type: PoolType::Concentrated,
            data_source: DataSource::ApiCreated, // 标识为API创建
            chain_confirmed: false,
        };

        // 插入数据库
        database.clmm_pool_repository.insert_pool(pool_model).await?;

        info!(
            "📝 Launch Migration记录已保存到数据库（含交易）: {} ({})",
            response.pool_address, response.signature
        );
        Ok(())
    }

    // ========== 私有辅助方法 ==========

    /// 验证迁移请求参数
    fn validate_migration_request(&self, request: &LaunchMigrationRequest) -> Result<()> {
        // 价格验证
        if request.initial_price <= 0.0 {
            return Err(anyhow::anyhow!("初始价格必须大于0"));
        }
        if request.tick_lower_price >= request.tick_upper_price {
            return Err(anyhow::anyhow!("下限价格必须小于上限价格"));
        }

        // 金额验证
        if request.meme_token_amount == 0 || request.base_token_amount == 0 {
            return Err(anyhow::anyhow!("流动性金额必须大于0"));
        }

        // 地址验证
        Pubkey::from_str(&request.meme_token_mint).map_err(|_| anyhow::anyhow!("无效的meme币地址"))?;
        Pubkey::from_str(&request.base_token_mint).map_err(|_| anyhow::anyhow!("无效的配对币地址"))?;
        Pubkey::from_str(&request.user_wallet).map_err(|_| anyhow::anyhow!("无效的用户钱包地址"))?;

        // 代币地址不能相同
        if request.meme_token_mint == request.base_token_mint {
            return Err(anyhow::anyhow!("meme币和配对币不能相同"));
        }

        // 滑点验证
        if request.max_slippage_percent < 0.0 || request.max_slippage_percent > 100.0 {
            return Err(anyhow::anyhow!("滑点百分比必须在0-100之间"));
        }

        Ok(())
    }

    /// 构建迁移的所有指令
    async fn build_migration_instructions(&self, request: &LaunchMigrationRequest) -> Result<Vec<Instruction>> {
        let (instructions, _) = self.build_migration_instructions_with_addresses(request).await?;
        Ok(instructions)
    }

    /// 构建迁移的所有指令并返回地址信息
    async fn build_migration_instructions_with_addresses(
        &self,
        request: &LaunchMigrationRequest,
    ) -> Result<(Vec<Instruction>, MigrationAddresses)> {
        let (instructions, addresses, _) = self.build_migration_instructions_with_keypair(request).await?;
        Ok((instructions, addresses))
    }

    /// 构建迁移的所有指令并返回地址信息和NFT mint keypair
    async fn build_migration_instructions_with_keypair(
        &self,
        request: &LaunchMigrationRequest,
    ) -> Result<(Vec<Instruction>, MigrationAddresses, Keypair)> {
        let mut instructions = Vec::new();

        // 解析基础参数
        let mut mint0 = Pubkey::from_str(&request.meme_token_mint)?;
        let mut mint1 = Pubkey::from_str(&request.base_token_mint)?;
        let mut price = request.initial_price;
        let user_wallet = Pubkey::from_str(&request.user_wallet)?;

        // 确保mint0 < mint1的顺序，如果不是则交换并调整价格
        if mint0 > mint1 {
            std::mem::swap(&mut mint0, &mut mint1);
            price = 1.0 / price;
            info!("  🔄 交换mint顺序，调整后价格: {}", price);
        }

        // 批量加载mint账户信息
        let load_pubkeys = vec![mint0, mint1];
        let rsps = self.shared.rpc_client.get_multiple_accounts(&load_pubkeys)?;

        let mint0_account = rsps[0].as_ref().ok_or_else(|| anyhow::anyhow!("Mint0账户不存在"))?;
        let mint1_account = rsps[1].as_ref().ok_or_else(|| anyhow::anyhow!("Mint1账户不存在"))?;

        let mint0_owner = mint0_account.owner;
        let mint1_owner = mint1_account.owner;

        // 解析mint信息获取decimals
        let mint0_state = Mint::unpack(&mint0_account.data)?;
        let mint1_state = Mint::unpack(&mint1_account.data)?;

        info!("  Mint信息:");
        info!("    Mint0 decimals: {}, owner: {}", mint0_state.decimals, mint0_owner);
        info!("    Mint1 decimals: {}, owner: {}", mint1_state.decimals, mint1_owner);

        // 计算sqrt_price_x64
        let sqrt_price_x64 = self.calculate_sqrt_price_x64(price, mint0_state.decimals, mint1_state.decimals);

        // 计算对应的tick
        let tick = raydium_amm_v3::libraries::tick_math::get_tick_at_sqrt_price(sqrt_price_x64)?;

        info!("  价格计算结果:");
        info!("    sqrt_price_x64: {}", sqrt_price_x64);
        info!("    对应tick: {}", tick);

        // 获取所有相关的PDA地址
        let pool_addresses =
            PoolInstructionBuilder::get_all_pool_addresses(request.config_index.try_into()?, &mint0, &mint1)?;

        info!("  计算的地址:");
        info!("    池子地址: {}", pool_addresses.pool);
        info!("    AMM配置: {}", pool_addresses.amm_config);
        info!("    Token0 Vault: {}", pool_addresses.token_vault_0);
        info!("    Token1 Vault: {}", pool_addresses.token_vault_1);

        // 阶段1: 创建池子指令
        let pool_instructions = PoolInstructionBuilder::build_create_pool_instruction(
            &user_wallet,
            request.config_index.try_into()?,
            &mint0,
            &mint1,
            &mint0_owner,
            &mint1_owner,
            sqrt_price_x64,
            request.open_time,
        )?;
        instructions.extend(pool_instructions);

        // 生成NFT mint keypair
        let nft_mint = Keypair::new();

        // 使用PositionUtilsOptimized进行价格和流动性计算
        let position_utils = PositionUtilsOptimized::new(&self.shared.rpc_client);

        // 价格转换为tick（与现有服务保持一致）
        let sqrt_price_lower = position_utils.price_to_sqrt_price_x64(
            request.tick_lower_price,
            mint0_state.decimals,
            mint1_state.decimals,
        );
        let sqrt_price_upper = position_utils.price_to_sqrt_price_x64(
            request.tick_upper_price,
            mint0_state.decimals,
            mint1_state.decimals,
        );

        let tick_lower_raw = raydium_amm_v3::libraries::tick_math::get_tick_at_sqrt_price(sqrt_price_lower)?;
        let tick_upper_raw = raydium_amm_v3::libraries::tick_math::get_tick_at_sqrt_price(sqrt_price_upper)?;

        // 获取tick spacing (这里需要从config中获取，暂时使用默认值)
        let tick_spacing = 60; // 根据config_index获取实际的tick spacing

        let tick_lower_adjusted = position_utils.tick_with_spacing(tick_lower_raw, tick_spacing);
        let tick_upper_adjusted = position_utils.tick_with_spacing(tick_upper_raw, tick_spacing);

        info!("  Tick spacing调整 (spacing = {}):", tick_spacing);
        info!("    tick_lower: {} -> {}", tick_lower_raw, tick_lower_adjusted);
        info!("    tick_upper: {} -> {}", tick_upper_raw, tick_upper_adjusted);

        // 重新计算调整后的sqrt_price
        let sqrt_price_lower_adjusted =
            raydium_amm_v3::libraries::tick_math::get_sqrt_price_at_tick(tick_lower_adjusted)?;
        let sqrt_price_upper_adjusted =
            raydium_amm_v3::libraries::tick_math::get_sqrt_price_at_tick(tick_upper_adjusted)?;

        // 计算流动性
        let input_amount = std::cmp::max(request.meme_token_amount, request.base_token_amount);
        let is_base_0 = request.meme_token_amount >= request.base_token_amount;

        let liquidity = position_utils.calculate_liquidity_from_single_amount(
            sqrt_price_x64,
            sqrt_price_lower_adjusted,
            sqrt_price_upper_adjusted,
            input_amount,
            is_base_0,
        )?;

        let (amount_0, amount_1) = position_utils.calculate_amounts_from_liquidity(
            tick,
            sqrt_price_x64,
            tick_lower_adjusted,
            tick_upper_adjusted,
            liquidity,
        )?;

        // 应用滑点保护
        let slippage = if request.max_slippage_percent == 0.0 {
            5.0
        } else {
            request.max_slippage_percent
        };
        let amount_0_with_slippage = position_utils.apply_slippage(amount_0, slippage, false);
        let amount_1_with_slippage = position_utils.apply_slippage(amount_1, slippage, false);

        // 计算转账费用
        let (transfer_fee_0, transfer_fee_1) = self.shared.swap_v2_service.get_pool_mints_inverse_fee(
            &mint0,
            &mint1,
            amount_0_with_slippage,
            amount_1_with_slippage,
        )?;

        // 计算包含转账费的最大金额
        let amount_0_max = amount_0_with_slippage
            .checked_add(transfer_fee_0.transfer_fee)
            .ok_or_else(|| anyhow::anyhow!("金额溢出"))?;
        let amount_1_max = amount_1_with_slippage
            .checked_add(transfer_fee_1.transfer_fee)
            .ok_or_else(|| anyhow::anyhow!("金额溢出"))?;

        info!("  流动性: {}", liquidity);
        info!("  Token0最大消耗: {}", amount_0_max);
        info!("  Token1最大消耗: {}", amount_1_max);

        // 阶段1.5: 预创建用户代币账户（修复 token_account_1 not initialized 错误）
        // 获取用户的代币账户地址
        let user_token_account_0 = spl_associated_token_account::get_associated_token_address_with_program_id(
            &user_wallet,
            &mint0,
            &mint0_owner,
        );
        let user_token_account_1 = spl_associated_token_account::get_associated_token_address_with_program_id(
            &user_wallet,
            &mint1,
            &mint1_owner,
        );

        // 使用幂等方法创建用户的Token0账户（如果已存在则跳过）
        info!("  ➕ 确保用户Token0关联代币账户存在: {}", user_token_account_0);
        let create_ata_0_instruction =
            spl_associated_token_account::instruction::create_associated_token_account_idempotent(
                &user_wallet, // payer
                &user_wallet, // wallet
                &mint0,       // token_mint
                &mint0_owner, // token_program
            );
        instructions.push(create_ata_0_instruction);

        // 使用幂等方法创建用户的Token1账户（如果已存在则跳过）
        info!("  ➕ 确保用户Token1关联代币账户存在: {}", user_token_account_1);
        let create_ata_1_instruction =
            spl_associated_token_account::instruction::create_associated_token_account_idempotent(
                &user_wallet, // payer
                &user_wallet, // wallet
                &mint1,       // token_mint
                &mint1_owner, // token_program
            );
        instructions.push(create_ata_1_instruction);

        // 阶段2: 构建开仓指令
        let mut remaining_accounts = Vec::new();
        let raydium_program_id = ConfigManager::get_raydium_program_id()?;
        let (tickarray_bitmap_extension, _) = Pubkey::find_program_address(
            &[b"pool_tick_array_bitmap_extension", pool_addresses.pool.as_ref()],
            &raydium_program_id,
        );
        remaining_accounts.push(AccountMeta::new(tickarray_bitmap_extension, false));

        // 计算tick array索引
        let tick_array_lower_start_index =
            raydium_amm_v3::states::TickArrayState::get_array_start_index(tick_lower_adjusted, tick_spacing as u16);
        let tick_array_upper_start_index =
            raydium_amm_v3::states::TickArrayState::get_array_start_index(tick_upper_adjusted, tick_spacing as u16);

        let position_instructions = PositionInstructionBuilder::build_open_position_with_token22_nft_instructions(
            &pool_addresses.pool,
            // 这里需要构建一个简单的PoolState，或者传递必要的参数
            &pool_addresses.token_vault_0,
            &pool_addresses.token_vault_1,
            &mint0,
            &mint1,
            &user_wallet,
            &nft_mint.pubkey(),
            &user_token_account_0,
            &user_token_account_1,
            tick_lower_adjusted,
            tick_upper_adjusted,
            tick_array_lower_start_index,
            tick_array_upper_start_index,
            liquidity,
            amount_0_max,
            amount_1_max,
            request.with_metadata.unwrap_or(false),
            remaining_accounts,
        )?;
        instructions.extend(position_instructions);

        // 计算position key
        let position_key = self.calculate_position_key(&nft_mint.pubkey())?;

        // 构建地址信息
        let addresses = MigrationAddresses {
            pool_address: pool_addresses.pool.to_string(),
            amm_config_address: pool_addresses.amm_config.to_string(),
            token_vault_0: pool_addresses.token_vault_0.to_string(),
            token_vault_1: pool_addresses.token_vault_1.to_string(),
            observation_address: pool_addresses.observation.to_string(),
            tickarray_bitmap_extension: tickarray_bitmap_extension.to_string(),
            position_nft_mint: nft_mint.pubkey().to_string(),
            position_key: position_key.to_string(),
            liquidity,
            actual_initial_price: price,
            sqrt_price_x64,
            initial_tick: tick,
            tick_lower_index: tick_lower_adjusted,
            tick_upper_index: tick_upper_adjusted,
            amount_0: amount_0_max,
            amount_1: amount_1_max,
        };

        info!("📦 总共构建了 {} 个指令", instructions.len());
        Ok((instructions, addresses, nft_mint))
    }

    /// 构建原子交易
    fn build_atomic_transaction(&self, instructions: Vec<Instruction>, payer: &Pubkey) -> Result<String> {
        let mut message = Message::new(&instructions, Some(payer));
        message.recent_blockhash = self.shared.rpc_client.get_latest_blockhash()?;

        let transaction_data = bincode::serialize(&message).map_err(|e| anyhow::anyhow!("序列化交易失败: {}", e))?;
        let transaction_base64 = BASE64_STANDARD.encode(&transaction_data);

        Ok(transaction_base64)
    }

    /// 计算迁移相关的所有地址信息
    async fn calculate_migration_addresses(&self, request: &LaunchMigrationRequest) -> Result<MigrationAddresses> {
        // 这里重用build_migration_instructions_with_addresses的逻辑
        let (_, addresses) = self.build_migration_instructions_with_addresses(request).await?;
        Ok(addresses)
    }

    /// Calculate sqrt_price_x64 (复用现有逻辑)
    fn calculate_sqrt_price_x64(&self, price: f64, decimals_0: u8, decimals_1: u8) -> u128 {
        let multipler = |decimals: u8| -> f64 { (10_i32).checked_pow(decimals.try_into().unwrap()).unwrap() as f64 };

        let price_to_x64 =
            |price: f64| -> u128 { (price * raydium_amm_v3::libraries::fixed_point_64::Q64 as f64) as u128 };

        let price_with_decimals = price * multipler(decimals_1) / multipler(decimals_0);
        price_to_x64(price_with_decimals.sqrt())
    }

    /// 计算position key
    fn calculate_position_key(&self, nft_mint: &Pubkey) -> Result<Pubkey> {
        let raydium_program_id = ConfigManager::get_raydium_program_id()?;
        let (position_key, _) = Pubkey::find_program_address(&[b"position", nft_mint.as_ref()], &raydium_program_id);
        Ok(position_key)
    }

    /// 查询用户的Launch Migration历史
    pub async fn get_user_launch_history(
        &self,
        creator: &str,
        page: u64,
        limit: u64,
    ) -> Result<Vec<database::clmm_pool::model::ClmmPool>> {
        // 边界检查：确保page不为0
        let safe_page = std::cmp::max(page, 1);

        // 使用现有的Repository方法查询Launch Migration创建的池子
        let filter_doc = doc! {
            "creator_wallet": creator,
            "data_source": { "$in": ["api", "api_chain_confirmed"] },
            "pool_type": "concentrated"
        };

        // 通过Repository接口查询
        let pools = self
            .database
            .clmm_pool_repository
            .get_collection()
            .find(
                filter_doc,
                mongodb::options::FindOptions::builder()
                    .sort(doc! { "api_created_at": -1 })
                    .skip((safe_page - 1) * limit)
                    .limit(limit as i64)
                    .build(),
            )
            .await?;

        let mut results = Vec::new();
        let mut cursor = pools;
        while cursor.advance().await? {
            results.push(cursor.deserialize_current()?);
        }

        Ok(results)
    }

    /// 获取用户Launch Migration历史记录总数
    pub async fn get_user_launch_history_count(&self, creator: &str) -> Result<u64> {
        let filter_doc = doc! {
            "creator_wallet": creator,
            "data_source": { "$in": ["api", "api_chain_confirmed"] },
            "pool_type": "concentrated"
        };

        let count = self
            .database
            .clmm_pool_repository
            .get_collection()
            .count_documents(filter_doc, None)
            .await? as u64;

        Ok(count)
    }

    /// 获取Launch Migration统计信息
    pub async fn get_launch_stats(&self) -> Result<LaunchMigrationStats> {
        use mongodb::bson::doc;

        // 统计总Launch次数
        let total_launches = self
            .database
            .clmm_pool_repository
            .get_collection()
            .count_documents(
                doc! {
                    "data_source": { "$in": ["api", "api_chain_confirmed"] },
                    "pool_type": "concentrated"
                },
                None,
            )
            .await? as u64;

        // 统计成功的Launch次数
        let successful_launches = self
            .database
            .clmm_pool_repository
            .get_collection()
            .count_documents(
                doc! {
                    "chain_confirmed": true,
                    "data_source": { "$in": ["api", "api_chain_confirmed"] },
                    "pool_type": "concentrated"
                },
                None,
            )
            .await? as u64;

        // 统计待确认的Launch次数
        let pending_launches = self
            .database
            .clmm_pool_repository
            .get_collection()
            .count_documents(
                doc! {
                    "status": "Pending",
                    "data_source": { "$in": ["api", "api_chain_confirmed"] },
                    "pool_type": "concentrated"
                },
                None,
            )
            .await? as u64;

        // 统计今日Launch次数
        let today_start = chrono::Utc::now()
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp();

        let today_launches = self
            .database
            .clmm_pool_repository
            .get_collection()
            .count_documents(
                doc! {
                    "api_created_at": { "$gte": today_start as f64 },
                    "data_source": { "$in": ["api", "api_chain_confirmed"] },
                    "pool_type": "concentrated"
                },
                None,
            )
            .await? as u64;

        // 计算成功率
        let success_rate = if total_launches > 0 {
            (successful_launches as f64 / total_launches as f64) * 100.0
        } else {
            0.0
        };

        // 获取按天统计的Launch数量（最近7天）
        let daily_launch_counts = self.get_daily_launch_counts(7).await?;

        Ok(LaunchMigrationStats {
            total_launches,
            successful_launches,
            pending_launches,
            today_launches,
            success_rate,
            daily_launch_counts,
        })
    }

    /// 获取按天统计的Launch数量
    async fn get_daily_launch_counts(&self, days: i64) -> Result<Vec<DailyLaunchCount>> {
        use mongodb::bson::doc;
        let today = chrono::Utc::now().date_naive();
        let mut daily_counts = Vec::new();

        for i in 0..days {
            let date = today - chrono::Duration::days(i);
            let date_str = date.format("%Y-%m-%d").to_string();

            let day_start = date.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp();
            let day_end = date.and_hms_opt(23, 59, 59).unwrap().and_utc().timestamp();

            // 统计当天总Launch数
            let count = self
                .database
                .clmm_pool_repository
                .get_collection()
                .count_documents(
                    doc! {
                        "api_created_at": {
                            "$gte": day_start as f64,
                            "$lte": day_end as f64
                        },
                        "data_source": { "$in": ["api", "api_chain_confirmed"] },
                        "pool_type": "concentrated"
                    },
                    None,
                )
                .await? as u64;

            // 统计当天成功Launch数
            let success_count = self
                .database
                .clmm_pool_repository
                .get_collection()
                .count_documents(
                    doc! {
                        "api_created_at": {
                            "$gte": day_start as f64,
                            "$lte": day_end as f64
                        },
                        "chain_confirmed": true,
                        "data_source": { "$in": ["api", "api_chain_confirmed"] },
                        "pool_type": "concentrated"
                    },
                    None,
                )
                .await? as u64;

            daily_counts.push(DailyLaunchCount {
                date: date_str,
                count,
                success_count,
            });
        }

        // 按日期正序排列（最早的在前）
        daily_counts.reverse();
        Ok(daily_counts)
    }

    // /// 构建临时的PoolState供指令构建使用
    // fn build_temporary_pool_state(
    //     &self,
    //     mint0: &Pubkey,
    //     mint1: &Pubkey,
    //     sqrt_price_x64: u128,
    //     tick_current: i32,
    //     mint_decimals_0: u8,
    //     mint_decimals_1: u8,
    //     tick_spacing: u16,
    // ) -> raydium_amm_v3::states::PoolState {
    //     // 这里构建一个最小化的PoolState用于指令构建
    //     // 实际使用中可能需要更完整的字段
    //     raydium_amm_v3::states::PoolState {
    //         amm_config: Pubkey::default(), // 会在后续填充
    //         token_mint_0: *mint0,
    //         token_mint_1: *mint1,
    //         token_vault_0: Pubkey::default(),
    //         token_vault_1: Pubkey::default(),
    //         observation_key: Pubkey::default(),
    //         mint_decimals_0,
    //         mint_decimals_1,
    //         tick_spacing,
    //         liquidity: 0,
    //         sqrt_price_x64,
    //         tick_current,
    //         ..Default::default()
    //     }
    // }
}
