// PositionService handles all position management operations

use crate::dtos::solana::position::open_position::{
    CalculateLiquidityRequest, CalculateLiquidityResponse, GetUserPositionsRequest,
    OpenPositionAndSendTransactionResponse, OpenPositionRequest, OpenPositionResponse, PositionInfo,
    UserPositionsResponse,
};

use super::super::liquidity::LiquidityService;
use crate::services::position_storage::PositionStorageService;

use super::super::shared::{helpers::SolanaUtils, SharedContext};
use ::utils::solana::{ConfigManager, PositionInstructionBuilder, PositionUtilsOptimized};

use crate::dtos::solana::common::TransactionStatus;
use crate::dtos::solana::position::liquidity::{
    DecreaseLiquidityAndSendTransactionResponse, DecreaseLiquidityRequest, DecreaseLiquidityResponse,
    IncreaseLiquidityAndSendTransactionResponse, IncreaseLiquidityRequest, IncreaseLiquidityResponse,
};
use anyhow::Result;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use solana_sdk::{
    instruction::AccountMeta, pubkey::Pubkey, signature::Keypair, signer::Signer, transaction::Transaction,
};
use std::str::FromStr;
use std::sync::Arc;
use tracing::info;

/// PositionService handles all position management operations
pub struct PositionService {
    shared: Arc<SharedContext>,
    liquidity_service: LiquidityService,
    position_storage_service: PositionStorageService,
}

impl PositionService {
    /// Create a new PositionService with shared context
    pub fn new(shared: Arc<SharedContext>) -> Self {
        let liquidity_service = LiquidityService::new(shared.clone());
        // TODO: 这里需要传入数据库实例，暂时使用占位符
        // 实际使用时需要在创建 PositionService 时传入数据库
        let position_storage_service = PositionStorageService::placeholder();
        Self {
            shared,
            liquidity_service,
            position_storage_service,
        }
    }

    /// Create a new PositionService with database
    pub fn with_database(shared: Arc<SharedContext>, db: Arc<database::Database>) -> Self {
        let liquidity_service = LiquidityService::with_database(shared.clone(), db.clone());
        let position_storage_service = PositionStorageService::new(db);
        Self {
            shared,
            liquidity_service,
            position_storage_service,
        }
    }

    /// Position management operations
    pub async fn open_position(&self, request: OpenPositionRequest) -> Result<OpenPositionResponse> {
        info!("🎯 开始构建开仓交易");
        info!("  池子地址: {}", request.pool_address);
        info!("  用户钱包: {}", request.user_wallet);
        info!(
            "  价格范围: {} - {}",
            request.tick_lower_price, request.tick_upper_price
        );
        info!("  输入金额: {}", request.input_amount);

        // 1. 验证请求参数
        self.validate_position_request(&request)?;

        // 2. 解析参数
        let pool_address = Pubkey::from_str(&request.pool_address)?;
        let user_wallet = Pubkey::from_str(&request.user_wallet)?;

        // 2. 加载池子状态
        let pool_account = self.shared.rpc_client.get_account(&pool_address)?;
        let pool_state: raydium_amm_v3::states::PoolState = SolanaUtils::deserialize_anchor_account(&pool_account)?;

        // 3. 使用优化版本的Position工具进行计算（显著提升性能）
        let position_utils = PositionUtilsOptimized::new(&self.shared.rpc_client);

        // 价格转换为tick（与CLI版本完全一致的流程）
        // 步骤1: 价格转sqrt_price
        let sqrt_price_lower = position_utils.price_to_sqrt_price_x64(
            request.tick_lower_price,
            pool_state.mint_decimals_0,
            pool_state.mint_decimals_1,
        );
        let sqrt_price_upper = position_utils.price_to_sqrt_price_x64(
            request.tick_upper_price,
            pool_state.mint_decimals_0,
            pool_state.mint_decimals_1,
        );

        info!("  价格转换详情:");
        info!(
            "    下限价格: {} -> sqrt_price_x64: {}",
            request.tick_lower_price, sqrt_price_lower
        );
        info!(
            "    上限价格: {} -> sqrt_price_x64: {}",
            request.tick_upper_price, sqrt_price_upper
        );

        // 步骤2: sqrt_price转tick
        let tick_lower_raw = raydium_amm_v3::libraries::tick_math::get_tick_at_sqrt_price(sqrt_price_lower)?;
        let tick_upper_raw = raydium_amm_v3::libraries::tick_math::get_tick_at_sqrt_price(sqrt_price_upper)?;

        info!("  原始tick计算:");
        info!("    tick_lower_raw: {}", tick_lower_raw);
        info!("    tick_upper_raw: {}", tick_upper_raw);

        // 步骤3: 调整tick spacing
        let tick_lower_adjusted = position_utils.tick_with_spacing(tick_lower_raw, pool_state.tick_spacing as i32);
        let tick_upper_adjusted = position_utils.tick_with_spacing(tick_upper_raw, pool_state.tick_spacing as i32);
        let tick_spacing = pool_state.tick_spacing;
        info!("  Tick spacing调整 (spacing = {}):", tick_spacing);
        info!("    tick_lower: {} -> {}", tick_lower_raw, tick_lower_adjusted);
        info!("    tick_upper: {} -> {}", tick_upper_raw, tick_upper_adjusted);

        // 步骤4: 重新计算调整后的sqrt_price（关键步骤！）
        let sqrt_price_lower_adjusted =
            raydium_amm_v3::libraries::tick_math::get_sqrt_price_at_tick(tick_lower_adjusted)?;
        let sqrt_price_upper_adjusted =
            raydium_amm_v3::libraries::tick_math::get_sqrt_price_at_tick(tick_upper_adjusted)?;

        // 反向验证：从调整后的tick计算回实际价格
        // let actual_lower_price = position_utils.sqrt_price_x64_to_price(
        //     sqrt_price_lower_adjusted,
        //     pool_state.mint_decimals_0,
        //     pool_state.mint_decimals_1,
        // );

        let actual_lower_price = raydium_amm_v3_clent::sqrt_price_x64_to_price(
            sqrt_price_lower_adjusted,
            pool_state.mint_decimals_0,
            pool_state.mint_decimals_1,
        );
        // let actual_upper_price = position_utils.sqrt_price_x64_to_price(
        //     sqrt_price_upper_adjusted,
        //     pool_state.mint_decimals_0,
        //     pool_state.mint_decimals_1,
        // );
        let actual_upper_price = raydium_amm_v3_clent::sqrt_price_x64_to_price(
            sqrt_price_upper_adjusted,
            pool_state.mint_decimals_0,
            pool_state.mint_decimals_1,
        );

        info!("  最终价格验证:");
        info!(
            "    请求价格范围: {} - {}",
            request.tick_lower_price, request.tick_upper_price
        );
        info!("    实际价格范围: {} - {}", actual_lower_price, actual_upper_price);
        info!("    最终tick范围: {} - {}", tick_lower_adjusted, tick_upper_adjusted);

        // 4. 检查是否已存在相同仓位
        if let Some(_existing) = position_utils
            .find_existing_position(&user_wallet, &pool_address, tick_lower_adjusted, tick_upper_adjusted)
            .await?
        {
            return Err(anyhow::anyhow!("相同价格范围的仓位已存在"));
        }

        // 5. 使用重新计算的sqrt_price计算流动性（与CLI版本一致）
        let liquidity = position_utils.calculate_liquidity_from_single_amount(
            pool_state.sqrt_price_x64,
            sqrt_price_lower_adjusted, // 使用调整后的值
            sqrt_price_upper_adjusted, // 使用调整后的值
            request.input_amount,
            request.is_base_0,
        )?;

        let (amount_0, amount_1) = position_utils.calculate_amounts_from_liquidity(
            pool_state.tick_current,
            pool_state.sqrt_price_x64,
            tick_lower_adjusted,
            tick_upper_adjusted,
            liquidity,
        )?;

        // 6. 应用滑点保护（修正：使用false表示计算最大输入，与CLI的round_up=true一致）
        let slippage = if request.max_slippage_percent == 0.5 {
            5.0 // 使用CLI版本的默认值
        } else {
            request.max_slippage_percent
        };
        // 注意：is_min=false表示计算最大输入金额（增加金额）
        let amount_0_with_slippage = position_utils.apply_slippage(amount_0, slippage, false);
        let amount_1_with_slippage = position_utils.apply_slippage(amount_1, slippage, false);

        // 7. 计算转账费用（支持Token-2022）
        let (transfer_fee_0, transfer_fee_1) = self.shared.swap_v2_service.get_pool_mints_inverse_fee(
            &pool_state.token_mint_0,
            &pool_state.token_mint_1,
            amount_0_with_slippage,
            amount_1_with_slippage,
        )?;

        info!(
            "  转账费用 - Token0: {}, Token1: {}",
            transfer_fee_0.transfer_fee, transfer_fee_1.transfer_fee
        );
        info!(
            "  Token Program - Token0: {}, Token1: {}",
            transfer_fee_0.owner, transfer_fee_1.owner
        );

        // 8. 计算包含转账费的最大金额
        let amount_0_max = amount_0_with_slippage
            .checked_add(transfer_fee_0.transfer_fee)
            .ok_or_else(|| anyhow::anyhow!("金额溢出"))?;
        let amount_1_max = amount_1_with_slippage
            .checked_add(transfer_fee_1.transfer_fee)
            .ok_or_else(|| anyhow::anyhow!("金额溢出"))?;

        info!("  流动性: {}", liquidity);
        info!("  Token0最大消耗: {}", amount_0_max);
        info!("  Token1最大消耗: {}", amount_1_max);

        // 9. 生成NFT mint
        let nft_mint = Keypair::new();

        // 10. 构建remaining accounts - 只包含tickarray_bitmap_extension
        let mut remaining_accounts = Vec::new();
        let raydium_program_id = ConfigManager::get_raydium_program_id()?;
        let (tickarray_bitmap_extension, _) = Pubkey::find_program_address(
            &[b"pool_tick_array_bitmap_extension", pool_address.as_ref()],
            &raydium_program_id,
        );
        remaining_accounts.push(AccountMeta::new(tickarray_bitmap_extension, false));

        // 11. 计算tick array索引
        let tick_array_lower_start_index =
            raydium_amm_v3::states::TickArrayState::get_array_start_index(tick_lower_adjusted, pool_state.tick_spacing);
        let tick_array_upper_start_index =
            raydium_amm_v3::states::TickArrayState::get_array_start_index(tick_upper_adjusted, pool_state.tick_spacing);

        // 12. 获取用户的代币账户（使用transfer_fee的owner作为token program ID）
        let user_token_account_0 = spl_associated_token_account::get_associated_token_address_with_program_id(
            &user_wallet,
            &pool_state.token_mint_0,
            &transfer_fee_0.owner, // 这是mint账户的owner = token program ID
        );
        let user_token_account_1 = spl_associated_token_account::get_associated_token_address_with_program_id(
            &user_wallet,
            &pool_state.token_mint_1,
            &transfer_fee_1.owner, // 这是mint账户的owner = token program ID
        );
        let mut instructions = Vec::new();
        // 使用幂等方法创建用户的Token0账户（如果已存在则跳过）
        info!("  ➕ 确保用户Token0关联代币账户存在: {}", user_token_account_0);
        let create_ata_0_instruction =
            spl_associated_token_account::instruction::create_associated_token_account_idempotent(
                &user_wallet,             // payer
                &user_wallet,             // wallet
                &pool_state.token_mint_0, // token_mint
                &transfer_fee_0.owner,    // token_program
            );
        instructions.push(create_ata_0_instruction);

        // 使用幂等方法创建用户的Token1账户（如果已存在则跳过）
        info!("  ➕ 确保用户Token1关联代币账户存在: {}", user_token_account_1);
        let create_ata_1_instruction =
            spl_associated_token_account::instruction::create_associated_token_account_idempotent(
                &user_wallet,             // payer
                &user_wallet,             // wallet
                &pool_state.token_mint_1, // token_mint
                &transfer_fee_1.owner,    // token_program
            );
        instructions.push(create_ata_1_instruction);

        // 13. 构建OpenPosition指令
        let position_instructions = PositionInstructionBuilder::build_open_position_with_token22_nft_instructions(
            &pool_address,
            &pool_state.token_vault_0,
            &pool_state.token_vault_1,
            &pool_state.token_mint_0,
            &pool_state.token_mint_1,
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
            request.with_metadata,
            remaining_accounts,
        )?;
        instructions.extend(position_instructions);
        // 14. 构建未签名交易
        // 创建未签名的交易消息
        let mut message = solana_sdk::message::Message::new(&instructions, Some(&user_wallet));
        message.recent_blockhash = self.shared.rpc_client.get_latest_blockhash()?;

        // 序列化交易消息为Base64
        let transaction_data = bincode::serialize(&message).map_err(|e| anyhow::anyhow!("序列化交易失败: {}", e))?;
        let transaction_base64 = BASE64_STANDARD.encode(&transaction_data);

        info!("✅ 未签名交易构建成功");

        // 计算position key
        let position_key = self.calculate_position_key(&nft_mint.pubkey())?;

        // 构建交易消息摘要
        let transaction_message = format!(
            "开仓操作 - 池子: {}, 价格范围: {:.4}-{:.4}, 流动性: {}",
            &request.pool_address[..8],
            request.tick_lower_price,
            request.tick_upper_price,
            liquidity
        );

        let now = chrono::Utc::now().timestamp();

        let response = OpenPositionResponse {
            transaction: transaction_base64,
            transaction_message,
            position_nft_mint: nft_mint.pubkey().to_string(),
            position_key: position_key.to_string(),
            tick_lower_index: tick_lower_adjusted,
            tick_upper_index: tick_upper_adjusted,
            liquidity: liquidity.to_string(),
            amount_0: amount_0_max,
            amount_1: amount_1_max,
            pool_address: request.pool_address.clone(),
            timestamp: now,
        };

        // 异步保存开仓信息到数据库（不阻塞主流程）
        let storage_service = self.position_storage_service.clone();
        let request_clone = request.clone();
        let response_clone = response.clone();
        tokio::spawn(async move {
            if let Err(e) = storage_service
                .save_open_position(&request_clone, &response_clone, None)
                .await
            {
                tracing::warn!("保存开仓信息到数据库失败: {}", e);
            }
        });

        Ok(response)
    }

    pub async fn open_position_and_send_transaction(
        &self,
        request: OpenPositionRequest,
    ) -> Result<OpenPositionAndSendTransactionResponse> {
        info!("🎯 开始开仓操作");
        info!("  池子地址: {}", request.pool_address);
        info!(
            "  价格范围: {} - {}",
            request.tick_lower_price, request.tick_upper_price
        );
        info!("  输入金额: {}", request.input_amount);

        // 1. 验证请求参数
        self.validate_position_request(&request)?;

        // 2. 解析参数
        let pool_address = Pubkey::from_str(&request.pool_address)?;
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

        // 2. 加载池子状态
        let pool_account = self.shared.rpc_client.get_account(&pool_address)?;
        let pool_state: raydium_amm_v3::states::PoolState = SolanaUtils::deserialize_anchor_account(&pool_account)?;

        // 3. 使用优化版本的Position工具进行计算（显著提升性能）
        let position_utils = PositionUtilsOptimized::new(&self.shared.rpc_client);

        // 价格转换为tick（与CLI版本完全一致的流程）
        // 步骤1: 价格转sqrt_price
        let sqrt_price_lower = position_utils.price_to_sqrt_price_x64(
            request.tick_lower_price,
            pool_state.mint_decimals_0,
            pool_state.mint_decimals_1,
        );
        let sqrt_price_upper = position_utils.price_to_sqrt_price_x64(
            request.tick_upper_price,
            pool_state.mint_decimals_0,
            pool_state.mint_decimals_1,
        );

        // 步骤2: sqrt_price转tick
        let tick_lower_raw = raydium_amm_v3::libraries::tick_math::get_tick_at_sqrt_price(sqrt_price_lower)?;
        let tick_upper_raw = raydium_amm_v3::libraries::tick_math::get_tick_at_sqrt_price(sqrt_price_upper)?;

        // 步骤3: 调整tick spacing
        let tick_lower_adjusted = position_utils.tick_with_spacing(tick_lower_raw, pool_state.tick_spacing as i32);
        let tick_upper_adjusted = position_utils.tick_with_spacing(tick_upper_raw, pool_state.tick_spacing as i32);

        info!("  计算的tick范围: {} - {}", tick_lower_adjusted, tick_upper_adjusted);

        // 步骤4: 重新计算调整后的sqrt_price（关键步骤！）
        let sqrt_price_lower_adjusted =
            raydium_amm_v3::libraries::tick_math::get_sqrt_price_at_tick(tick_lower_adjusted)?;
        let sqrt_price_upper_adjusted =
            raydium_amm_v3::libraries::tick_math::get_sqrt_price_at_tick(tick_upper_adjusted)?;

        // 4. 检查是否已存在相同位置
        if let Some(_existing) = position_utils
            .find_existing_position(&user_wallet, &pool_address, tick_lower_adjusted, tick_upper_adjusted)
            .await?
        {
            return Err(anyhow::anyhow!("相同价格范围的位置已存在"));
        }

        // 5. 使用重新计算的sqrt_price计算流动性（与CLI版本一致）
        let liquidity = position_utils.calculate_liquidity_from_single_amount(
            pool_state.sqrt_price_x64,
            sqrt_price_lower_adjusted, // 使用调整后的值
            sqrt_price_upper_adjusted, // 使用调整后的值
            request.input_amount,
            request.is_base_0,
        )?;

        let (amount_0, amount_1) = position_utils.calculate_amounts_from_liquidity(
            pool_state.tick_current,
            pool_state.sqrt_price_x64,
            tick_lower_adjusted,
            tick_upper_adjusted,
            liquidity,
        )?;

        // 6. 应用滑点保护（修正：使用false表示计算最大输入，与CLI的round_up=true一致）
        let slippage = if request.max_slippage_percent == 0.5 {
            5.0 // 使用CLI版本的默认值
        } else {
            request.max_slippage_percent
        };
        // 注意：is_min=false表示计算最大输入金额（增加金额）
        let amount_0_with_slippage = position_utils.apply_slippage(amount_0, slippage, false);
        let amount_1_with_slippage = position_utils.apply_slippage(amount_1, slippage, false);

        // 7. 计算转账费用（支持Token-2022）
        let (transfer_fee_0, transfer_fee_1) = self.shared.swap_v2_service.get_pool_mints_inverse_fee(
            &pool_state.token_mint_0,
            &pool_state.token_mint_1,
            amount_0_with_slippage,
            amount_1_with_slippage,
        )?;

        info!(
            "  转账费用 - Token0: {}, Token1: {}",
            transfer_fee_0.transfer_fee, transfer_fee_1.transfer_fee
        );
        info!(
            "  Token Program - Token0: {}, Token1: {}",
            transfer_fee_0.owner, transfer_fee_1.owner
        );

        // 8. 计算包含转账费的最大金额
        let amount_0_max = amount_0_with_slippage
            .checked_add(transfer_fee_0.transfer_fee)
            .ok_or_else(|| anyhow::anyhow!("金额溢出"))?;
        let amount_1_max = amount_1_with_slippage
            .checked_add(transfer_fee_1.transfer_fee)
            .ok_or_else(|| anyhow::anyhow!("金额溢出"))?;

        info!("  流动性: {}", liquidity);
        info!("  Token0最大消耗: {}", amount_0_max);
        info!("  Token1最大消耗: {}", amount_1_max);

        // 9. 生成NFT mint
        let nft_mint = Keypair::new();

        // 10. 构建remaining accounts - 只包含tickarray_bitmap_extension
        let mut remaining_accounts = Vec::new();
        let raydium_program_id = ConfigManager::get_raydium_program_id()?;
        let (tickarray_bitmap_extension, _) = Pubkey::find_program_address(
            &[b"pool_tick_array_bitmap_extension", pool_address.as_ref()],
            &raydium_program_id,
        );
        remaining_accounts.push(AccountMeta::new(tickarray_bitmap_extension, false));

        // 11. 计算tick array索引
        let tick_array_lower_start_index =
            raydium_amm_v3::states::TickArrayState::get_array_start_index(tick_lower_adjusted, pool_state.tick_spacing);
        let tick_array_upper_start_index =
            raydium_amm_v3::states::TickArrayState::get_array_start_index(tick_upper_adjusted, pool_state.tick_spacing);

        // 12. 获取用户的代币账户（使用transfer_fee的owner作为token program ID）
        let user_token_account_0 = spl_associated_token_account::get_associated_token_address_with_program_id(
            &user_wallet,
            &pool_state.token_mint_0,
            &transfer_fee_0.owner, // 这是mint账户的owner = token program ID
        );
        let user_token_account_1 = spl_associated_token_account::get_associated_token_address_with_program_id(
            &user_wallet,
            &pool_state.token_mint_1,
            &transfer_fee_1.owner, // 这是mint账户的owner = token program ID
        );
        let mut instructions = Vec::new();
        // 使用幂等方法创建用户的Token0账户（如果已存在则跳过）
        info!("  ➕ 确保用户Token0关联代币账户存在: {}", user_token_account_0);
        let create_ata_0_instruction =
            spl_associated_token_account::instruction::create_associated_token_account_idempotent(
                &user_wallet,             // payer
                &user_wallet,             // wallet
                &pool_state.token_mint_0, // token_mint
                &transfer_fee_0.owner,    // token_program
            );
        instructions.push(create_ata_0_instruction);

        // 使用幂等方法创建用户的Token1账户（如果已存在则跳过）
        info!("  ➕ 确保用户Token1关联代币账户存在: {}", user_token_account_1);
        let create_ata_1_instruction =
            spl_associated_token_account::instruction::create_associated_token_account_idempotent(
                &user_wallet,             // payer
                &user_wallet,             // wallet
                &pool_state.token_mint_1, // token_mint
                &transfer_fee_1.owner,    // token_program
            );

        instructions.push(create_ata_1_instruction);
        // 13. 构建OpenPosition指令
        let position_instructions = PositionInstructionBuilder::build_open_position_with_token22_nft_instructions(
            &pool_address,
            &pool_state.token_vault_0,
            &pool_state.token_vault_1,
            &pool_state.token_mint_0,
            &pool_state.token_mint_1,
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
            request.with_metadata,
            remaining_accounts,
        )?;
        instructions.extend(position_instructions);
        // 14. 构建交易
        let recent_blockhash = self.shared.rpc_client.get_latest_blockhash()?;
        let transaction = Transaction::new_signed_with_payer(
            &instructions,
            Some(&user_wallet),
            &[&user_keypair, &nft_mint],
            recent_blockhash,
        );

        // 15. 发送交易
        let signature = self.shared.rpc_client.send_and_confirm_transaction(&transaction)?;

        info!("✅ 开仓成功，交易签名: {}", signature);

        // 计算position key
        let position_key = self.calculate_position_key(&nft_mint.pubkey())?;

        // 构建响应
        let explorer_url = format!("https://explorer.solana.com/tx/{}", signature);
        let now = chrono::Utc::now().timestamp();

        let response = OpenPositionAndSendTransactionResponse {
            signature: signature.to_string(),
            position_nft_mint: nft_mint.pubkey().to_string(),
            position_key: position_key.to_string(),
            tick_lower_index: tick_lower_adjusted,
            tick_upper_index: tick_upper_adjusted,
            liquidity: liquidity.to_string(),
            amount_0: amount_0_max,
            amount_1: amount_1_max,
            pool_address: request.pool_address.clone(),
            status: TransactionStatus::Finalized,
            explorer_url,
            timestamp: now,
        };

        // 异步保存开仓交易信息到数据库（不阻塞主流程）
        let storage_service = self.position_storage_service.clone();
        let request_clone = request.clone();
        let response_clone = response.clone();
        tokio::spawn(async move {
            if let Err(e) = storage_service
                .save_open_position_with_transaction(&request_clone, &response_clone)
                .await
            {
                tracing::warn!("保存开仓交易信息到数据库失败: {}", e);
            }
        });

        Ok(response)
    }

    pub async fn calculate_liquidity(&self, request: CalculateLiquidityRequest) -> Result<CalculateLiquidityResponse> {
        info!("🧮 计算流动性参数");

        // 1. 解析参数
        let pool_address = Pubkey::from_str(&request.pool_address)?;

        // 2. 加载池子状态
        let pool_account = self.shared.rpc_client.get_account(&pool_address)?;
        let pool_state: raydium_amm_v3::states::PoolState = SolanaUtils::deserialize_anchor_account(&pool_account)?;

        // 3. 使用优化版本的Position工具进行计算
        let position_utils = PositionUtilsOptimized::new(&self.shared.rpc_client);

        // 价格转换为tick
        let tick_lower_index = position_utils.price_to_tick(
            request.tick_lower_price,
            pool_state.mint_decimals_0,
            pool_state.mint_decimals_1,
        )?;
        let tick_upper_index = position_utils.price_to_tick(
            request.tick_upper_price,
            pool_state.mint_decimals_0,
            pool_state.mint_decimals_1,
        )?;

        // 调整tick spacing
        let tick_lower_adjusted = position_utils.tick_with_spacing(tick_lower_index, pool_state.tick_spacing as i32);
        let tick_upper_adjusted = position_utils.tick_with_spacing(tick_upper_index, pool_state.tick_spacing as i32);

        // 计算流动性
        let sqrt_price_lower = raydium_amm_v3::libraries::tick_math::get_sqrt_price_at_tick(tick_lower_adjusted)?;
        let sqrt_price_upper = raydium_amm_v3::libraries::tick_math::get_sqrt_price_at_tick(tick_upper_adjusted)?;

        let liquidity = position_utils.calculate_liquidity_from_single_amount(
            pool_state.sqrt_price_x64,
            sqrt_price_lower,
            sqrt_price_upper,
            request.input_amount,
            request.is_base_0,
        )?;

        // 计算所需金额
        let (amount_0, amount_1) = position_utils.calculate_amounts_from_liquidity(
            pool_state.tick_current,
            pool_state.sqrt_price_x64,
            tick_lower_adjusted,
            tick_upper_adjusted,
            liquidity,
        )?;

        // 计算当前价格和利用率
        // let current_price = position_utils.sqrt_price_x64_to_price(
        //     pool_state.sqrt_price_x64,
        //     pool_state.mint_decimals_0,
        //     pool_state.mint_decimals_1,
        // );
        let current_price = raydium_amm_v3_clent::sqrt_price_x64_to_price(
            pool_state.sqrt_price_x64,
            pool_state.mint_decimals_0,
            pool_state.mint_decimals_1,
        );

        let price_range_utilization = position_utils.calculate_price_range_utilization(
            current_price,
            request.tick_lower_price,
            request.tick_upper_price,
        );

        Ok(CalculateLiquidityResponse {
            liquidity: liquidity.to_string(),
            amount_0,
            amount_1,
            tick_lower_index: tick_lower_adjusted,
            tick_upper_index: tick_upper_adjusted,
            current_price,
            price_range_utilization,
        })
    }

    pub async fn get_user_positions(&self, request: GetUserPositionsRequest) -> Result<UserPositionsResponse> {
        info!("📋 获取用户仓位列表");

        // 1. 确定查询的钱包地址
        let wallet_address = if let Some(addr) = request.wallet_address {
            Pubkey::from_str(&addr)?
        } else {
            return Err(anyhow::anyhow!("缺少必需的钱包地址参数"));
        };

        // 2. 使用优化版本的Position工具获取NFT信息（显著提升性能）
        let position_utils_optimized = PositionUtilsOptimized::new(&self.shared.rpc_client);
        let position_nfts = position_utils_optimized.get_user_position_nfts(&wallet_address).await?;

        // 3. 批量加载position状态（优化性能）
        info!("🚀 开始批量获取 {} 个position账户", position_nfts.len());
        let position_addresses: Vec<Pubkey> = position_nfts.iter().map(|nft| nft.position_pda).collect();

        // 批量获取所有position账户
        let position_accounts = self.shared.rpc_client.get_multiple_accounts(&position_addresses)?;
        info!("✅ 批量获取position账户完成，收到 {} 个响应", position_accounts.len());

        // 解析position状态并收集需要的pool地址
        let mut position_states = Vec::new();
        let mut pool_addresses = std::collections::HashSet::new();

        for (i, account_option) in position_accounts.iter().enumerate() {
            if let Some(account) = account_option {
                if let Ok(position_state) = position_utils_optimized.deserialize_position_state(account) {
                    // 过滤池子（如果指定）
                    if let Some(ref pool_filter) = request.pool_address {
                        let pool_pubkey = Pubkey::from_str(pool_filter)?;
                        if position_state.pool_id != pool_pubkey {
                            continue;
                        }
                    }

                    pool_addresses.insert(position_state.pool_id);
                    position_states.push((i, position_state));
                }
            }
        }

        // 批量获取池子状态（去重）
        info!("🚀 开始批量获取 {} 个去重的pool账户", pool_addresses.len());
        let pool_addresses_vec: Vec<Pubkey> = pool_addresses.into_iter().collect();
        let pool_accounts = self.shared.rpc_client.get_multiple_accounts(&pool_addresses_vec)?;
        info!("✅ 批量获取pool账户完成，收到 {} 个响应", pool_accounts.len());

        // 构建pool状态缓存
        let mut pool_states_cache = std::collections::HashMap::new();
        for (i, account_option) in pool_accounts.iter().enumerate() {
            if let Some(account) = account_option {
                if let Ok(pool_state) =
                    SolanaUtils::deserialize_anchor_account::<raydium_amm_v3::states::PoolState>(account)
                {
                    pool_states_cache.insert(pool_addresses_vec[i], pool_state);
                }
            }
        }

        // 构建最终的position信息
        let mut positions = Vec::new();
        for (nft_index, position_state) in position_states {
            if let Some(pool_state) = pool_states_cache.get(&position_state.pool_id) {
                let tick_lower_price = position_utils_optimized.tick_to_price(
                    position_state.tick_lower_index,
                    pool_state.mint_decimals_0,
                    pool_state.mint_decimals_1,
                )?;
                let tick_upper_price = position_utils_optimized.tick_to_price(
                    position_state.tick_upper_index,
                    pool_state.mint_decimals_0,
                    pool_state.mint_decimals_1,
                )?;

                positions.push(PositionInfo {
                    position_key: position_nfts[nft_index].position_pda.to_string(),
                    nft_mint: position_state.nft_mint.to_string(),
                    pool_id: position_state.pool_id.to_string(),
                    tick_lower_index: position_state.tick_lower_index,
                    tick_upper_index: position_state.tick_upper_index,
                    liquidity: position_state.liquidity.to_string(),
                    tick_lower_price,
                    tick_upper_price,
                    token_fees_owed_0: position_state.token_fees_owed_0,
                    token_fees_owed_1: position_state.token_fees_owed_1,
                    reward_infos: vec![],                       // 简化处理
                    created_at: chrono::Utc::now().timestamp(), // 暂时使用当前时间
                });
            }
        }

        let total_count = positions.len();
        let now = chrono::Utc::now().timestamp();

        Ok(UserPositionsResponse {
            positions,
            total_count,
            wallet_address: wallet_address.to_string(),
            timestamp: now,
        })
    }

    pub async fn get_position_info(&self, position_key: String) -> Result<PositionInfo> {
        info!("🔍 获取仓位详情: {}", position_key);

        let position_pubkey = Pubkey::from_str(&position_key)?;
        let position_utils = PositionUtilsOptimized::new(&self.shared.rpc_client);

        // 加载position状态
        let position_account = self.shared.rpc_client.get_account(&position_pubkey)?;
        let position_state = position_utils.deserialize_position_state(&position_account)?;

        // 加载池子状态以计算价格
        let pool_account = self.shared.rpc_client.get_account(&position_state.pool_id)?;
        let pool_state: raydium_amm_v3::states::PoolState = SolanaUtils::deserialize_anchor_account(&pool_account)?;

        let tick_lower_price = position_utils.tick_to_price(
            position_state.tick_lower_index,
            pool_state.mint_decimals_0,
            pool_state.mint_decimals_1,
        )?;
        let tick_upper_price = position_utils.tick_to_price(
            position_state.tick_upper_index,
            pool_state.mint_decimals_0,
            pool_state.mint_decimals_1,
        )?;

        Ok(PositionInfo {
            position_key,
            nft_mint: position_state.nft_mint.to_string(),
            pool_id: position_state.pool_id.to_string(),
            tick_lower_index: position_state.tick_lower_index,
            tick_upper_index: position_state.tick_upper_index,
            liquidity: position_state.liquidity.to_string(),
            tick_lower_price,
            tick_upper_price,
            token_fees_owed_0: position_state.token_fees_owed_0,
            token_fees_owed_1: position_state.token_fees_owed_1,
            reward_infos: vec![], // 简化处理
            created_at: chrono::Utc::now().timestamp(),
        })
    }

    /// Check if position exists
    pub async fn check_position_exists(
        &self,
        pool_address: String,
        tick_lower: i32,
        tick_upper: i32,
        wallet_address: Option<String>,
    ) -> Result<Option<PositionInfo>> {
        let pool_pubkey = Pubkey::from_str(&pool_address)?;
        let wallet_pubkey = if let Some(addr) = wallet_address {
            Pubkey::from_str(&addr)?
        } else {
            return Err(anyhow::anyhow!("缺少必需的钱包地址参数"));
        };

        let position_utils = PositionUtilsOptimized::new(&self.shared.rpc_client);

        if let Some(existing) = position_utils
            .find_existing_position(&wallet_pubkey, &pool_pubkey, tick_lower, tick_upper)
            .await?
        {
            // 转换为PositionInfo
            let position_info = self.get_position_info(existing.position_key.to_string()).await?;
            Ok(Some(position_info))
        } else {
            Ok(None)
        }
    }

    // ============ Private Helper Methods ============

    /// Validate position parameters before processing
    fn validate_position_request(&self, request: &OpenPositionRequest) -> Result<()> {
        // Validate price range
        if request.tick_lower_price >= request.tick_upper_price {
            return Err(anyhow::anyhow!("下限价格必须小于上限价格"));
        }

        // Validate input amount
        if request.input_amount == 0 {
            return Err(anyhow::anyhow!("输入金额必须大于0"));
        }

        // Validate slippage
        if request.max_slippage_percent < 0.0 || request.max_slippage_percent > 100.0 {
            return Err(anyhow::anyhow!("滑点百分比必须在0-100之间"));
        }

        Ok(())
    }

    /// Calculate position key from NFT mint
    fn calculate_position_key(&self, nft_mint: &Pubkey) -> Result<Pubkey> {
        let raydium_program_id = ConfigManager::get_raydium_program_id()?;
        let (position_key, _) = Pubkey::find_program_address(&[b"position", nft_mint.as_ref()], &raydium_program_id);
        Ok(position_key)
    }

    /// Build remaining accounts for position operations
    fn _build_remaining_accounts(&self, pool_address: &Pubkey) -> Result<Vec<AccountMeta>> {
        let mut remaining_accounts = Vec::new();
        let raydium_program_id = ConfigManager::get_raydium_program_id()?;
        let (tickarray_bitmap_extension, _) = Pubkey::find_program_address(
            &[b"pool_tick_array_bitmap_extension", pool_address.as_ref()],
            &raydium_program_id,
        );
        remaining_accounts.push(AccountMeta::new(tickarray_bitmap_extension, false));
        Ok(remaining_accounts)
    }

    /// Apply slippage protection with proper validation
    fn _apply_slippage_protection(&self, amount: u64, slippage_percent: f64, is_minimum: bool) -> Result<u64> {
        if slippage_percent < 0.0 || slippage_percent > 100.0 {
            return Err(anyhow::anyhow!("无效的滑点百分比: {}", slippage_percent));
        }

        let position_utils = PositionUtilsOptimized::new(&self.shared.rpc_client);
        Ok(position_utils.apply_slippage(amount, slippage_percent, is_minimum))
    }

    /// Calculate tick array indices for position
    fn _calculate_tick_array_indices(&self, tick_lower: i32, tick_upper: i32, tick_spacing: u16) -> (i32, i32) {
        let tick_array_lower_start_index =
            raydium_amm_v3::states::TickArrayState::get_array_start_index(tick_lower, tick_spacing);
        let tick_array_upper_start_index =
            raydium_amm_v3::states::TickArrayState::get_array_start_index(tick_upper, tick_spacing);
        (tick_array_lower_start_index, tick_array_upper_start_index)
    }

    // ============ IncreaseLiquidity Methods (Delegated to LiquidityService) ============

    /// 增加流动性（构建交易）- 委托给LiquidityService
    pub async fn increase_liquidity(&self, request: IncreaseLiquidityRequest) -> Result<IncreaseLiquidityResponse> {
        self.liquidity_service.increase_liquidity(request).await
    }

    /// 增加流动性并发送交易 - 委托给LiquidityService
    pub async fn increase_liquidity_and_send_transaction(
        &self,
        request: IncreaseLiquidityRequest,
    ) -> Result<IncreaseLiquidityAndSendTransactionResponse> {
        self.liquidity_service
            .increase_liquidity_and_send_transaction(request)
            .await
    }

    // ============ DecreaseLiquidity Methods (Delegated to LiquidityService) ============

    /// 减少流动性（构建交易）- 委托给LiquidityService
    pub async fn decrease_liquidity(&self, request: DecreaseLiquidityRequest) -> Result<DecreaseLiquidityResponse> {
        self.liquidity_service.decrease_liquidity(request).await
    }

    /// 减少流动性并发送交易 - 委托给LiquidityService
    pub async fn decrease_liquidity_and_send_transaction(
        &self,
        request: DecreaseLiquidityRequest,
    ) -> Result<DecreaseLiquidityAndSendTransactionResponse> {
        self.liquidity_service
            .decrease_liquidity_and_send_transaction(request)
            .await
    }
}
