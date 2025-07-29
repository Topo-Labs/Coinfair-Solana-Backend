// LiquidityService handles all liquidity management operations

use crate::dtos::solana_dto::{
    DecreaseLiquidityAndSendTransactionResponse, DecreaseLiquidityRequest, DecreaseLiquidityResponse, IncreaseLiquidityAndSendTransactionResponse, IncreaseLiquidityRequest,
    IncreaseLiquidityResponse, TransactionStatus,
};

use super::super::shared::{helpers::SolanaUtils, SharedContext};
use ::utils::solana::{ConfigManager, PositionInstructionBuilder, PositionUtils};

use anyhow::Result;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use solana_sdk::{instruction::AccountMeta, pubkey::Pubkey, signature::Keypair, transaction::Transaction};
use std::str::FromStr;
use std::sync::Arc;
use tracing::{info, warn};
use utils::TokenUtils;

/// LiquidityService handles all liquidity management operations
pub struct LiquidityService {
    shared: Arc<SharedContext>,
}

impl LiquidityService {
    /// Create a new LiquidityService with shared context
    pub fn new(shared: Arc<SharedContext>) -> Self {
        Self { shared }
    }

    /// 增加流动性（构建交易）
    pub async fn increase_liquidity(&self, request: IncreaseLiquidityRequest) -> Result<IncreaseLiquidityResponse> {
        info!("🔧 开始构建增加流动性交易");
        info!("  池子地址: {}", request.pool_address);
        info!("  用户钱包: {}", request.user_wallet);
        info!("  价格范围: {} - {}", request.tick_lower_price, request.tick_upper_price);
        info!("  输入金额: {}", request.input_amount);

        // 1. 验证请求参数
        self.validate_increase_liquidity_request(&request)?;

        // 2. 解析参数
        let pool_address = Pubkey::from_str(&request.pool_address)?;
        let user_wallet = Pubkey::from_str(&request.user_wallet)?;

        // 3. 加载池子状态
        let pool_account = self.shared.rpc_client.get_account(&pool_address)?;
        let pool_state: raydium_amm_v3::states::PoolState = SolanaUtils::deserialize_anchor_account(&pool_account)?;

        // 4. 使用Position工具进行计算
        let position_utils = PositionUtils::new(&self.shared.rpc_client);

        // 价格转换为tick（与CLI版本完全一致的流程）
        let sqrt_price_lower = position_utils.price_to_sqrt_price_x64(request.tick_lower_price, pool_state.mint_decimals_0, pool_state.mint_decimals_1);
        let sqrt_price_upper = position_utils.price_to_sqrt_price_x64(request.tick_upper_price, pool_state.mint_decimals_0, pool_state.mint_decimals_1);

        let tick_lower_raw = raydium_amm_v3::libraries::tick_math::get_tick_at_sqrt_price(sqrt_price_lower)?;
        let tick_upper_raw = raydium_amm_v3::libraries::tick_math::get_tick_at_sqrt_price(sqrt_price_upper)?;

        let tick_lower_adjusted = position_utils.tick_with_spacing(tick_lower_raw, pool_state.tick_spacing as i32);
        let tick_upper_adjusted = position_utils.tick_with_spacing(tick_upper_raw, pool_state.tick_spacing as i32);

        info!("  计算的tick范围: {} - {}", tick_lower_adjusted, tick_upper_adjusted);

        // 重新计算调整后的sqrt_price
        let sqrt_price_lower_adjusted = raydium_amm_v3::libraries::tick_math::get_sqrt_price_at_tick(tick_lower_adjusted)?;
        let sqrt_price_upper_adjusted = raydium_amm_v3::libraries::tick_math::get_sqrt_price_at_tick(tick_upper_adjusted)?;

        // 5. 查找现有的匹配仓位（必须）
        let existing_position = position_utils
            .find_existing_position(&user_wallet, &pool_address, tick_lower_adjusted, tick_upper_adjusted)
            .await?
            .ok_or_else(|| anyhow::anyhow!("未找到匹配的现有仓位。增加流动性需要先有相同价格范围的仓位。"))?;

        info!("  找到现有仓位: {}", existing_position.position_key);

        // 6. 计算新增流动性
        let liquidity = position_utils.calculate_liquidity_from_single_amount(
            pool_state.sqrt_price_x64,
            sqrt_price_lower_adjusted,
            sqrt_price_upper_adjusted,
            request.input_amount,
            request.is_base_0,
        )?;

        let (amount_0, amount_1) =
            position_utils.calculate_amounts_from_liquidity(pool_state.tick_current, pool_state.sqrt_price_x64, tick_lower_adjusted, tick_upper_adjusted, liquidity)?;

        // 7. 应用滑点保护
        let slippage = if request.max_slippage_percent == 0.5 {
            5.0 // 使用CLI版本的默认值
        } else {
            request.max_slippage_percent
        };
        let amount_0_with_slippage = position_utils.apply_slippage(amount_0, slippage, false);
        let amount_1_with_slippage = position_utils.apply_slippage(amount_1, slippage, false);

        // 8. 计算转账费用（支持Token-2022）
        let (transfer_fee_0, transfer_fee_1) =
            self.shared
                .swap_v2_service
                .get_pool_mints_inverse_fee(&pool_state.token_mint_0, &pool_state.token_mint_1, amount_0_with_slippage, amount_1_with_slippage)?;

        // 9. 计算包含转账费的最大金额
        let amount_0_max = amount_0_with_slippage.checked_add(transfer_fee_0.transfer_fee).ok_or_else(|| anyhow::anyhow!("金额溢出"))?;
        let amount_1_max = amount_1_with_slippage.checked_add(transfer_fee_1.transfer_fee).ok_or_else(|| anyhow::anyhow!("金额溢出"))?;

        info!("  新增流动性: {}", liquidity);
        info!("  Token0最大消耗: {}", amount_0_max);
        info!("  Token1最大消耗: {}", amount_1_max);

        // 10. 构建remaining accounts
        let mut remaining_accounts = Vec::new();
        let raydium_program_id = ConfigManager::get_raydium_program_id()?;
        let (tickarray_bitmap_extension, _) = Pubkey::find_program_address(&[b"pool_tick_array_bitmap_extension", pool_address.as_ref()], &raydium_program_id);
        remaining_accounts.push(AccountMeta::new(tickarray_bitmap_extension, false));

        // 11. 计算tick array索引
        let tick_array_lower_start_index = raydium_amm_v3::states::TickArrayState::get_array_start_index(tick_lower_adjusted, pool_state.tick_spacing);
        let tick_array_upper_start_index = raydium_amm_v3::states::TickArrayState::get_array_start_index(tick_upper_adjusted, pool_state.tick_spacing);

        // 12. 获取用户的代币账户（使用现有NFT的Token Program）
        let user_token_account_0 = spl_associated_token_account::get_associated_token_address_with_program_id(&user_wallet, &pool_state.token_mint_0, &transfer_fee_0.owner);
        let user_token_account_1 = spl_associated_token_account::get_associated_token_address_with_program_id(
            &user_wallet,
            &pool_state.token_mint_1,
            &transfer_fee_1.owner, // 修复CLI中的bug：应该使用transfer_fee_1.owner
        );

        // 13. 构建IncreaseLiquidity指令
        let instructions = PositionInstructionBuilder::build_increase_liquidity_instructions(
            &pool_address,
            &pool_state,
            &user_wallet,
            &existing_position.nft_mint,
            &existing_position.nft_token_account,
            &user_token_account_0,
            &user_token_account_1,
            tick_lower_adjusted,
            tick_upper_adjusted,
            tick_array_lower_start_index,
            tick_array_upper_start_index,
            liquidity,
            amount_0_max,
            amount_1_max,
            remaining_accounts,
        )?;

        // 14. 构建未签名交易
        let mut message = solana_sdk::message::Message::new(&instructions, Some(&user_wallet));
        message.recent_blockhash = self.shared.rpc_client.get_latest_blockhash()?;

        // 序列化交易消息为Base64
        let transaction_data = bincode::serialize(&message).map_err(|e| anyhow::anyhow!("序列化交易失败: {}", e))?;
        let transaction_base64 = BASE64_STANDARD.encode(&transaction_data);

        info!("✅ 增加流动性交易构建成功");

        // 构建交易消息摘要
        let transaction_message = format!(
            "增加流动性 - 池子: {}, 价格范围: {:.4}-{:.4}, 新增流动性: {}",
            &request.pool_address[..8],
            request.tick_lower_price,
            request.tick_upper_price,
            liquidity
        );

        let now = chrono::Utc::now().timestamp();

        Ok(IncreaseLiquidityResponse {
            transaction: transaction_base64,
            transaction_message,
            position_key: existing_position.position_key.to_string(),
            liquidity_added: liquidity.to_string(),
            amount_0: amount_0_max,
            amount_1: amount_1_max,
            tick_lower_index: tick_lower_adjusted,
            tick_upper_index: tick_upper_adjusted,
            pool_address: request.pool_address,
            timestamp: now,
        })
    }

    /// 增加流动性并发送交易
    pub async fn increase_liquidity_and_send_transaction(&self, request: IncreaseLiquidityRequest) -> Result<IncreaseLiquidityAndSendTransactionResponse> {
        info!("🔧 开始增加流动性操作");
        info!("  池子地址: {}", request.pool_address);
        info!("  价格范围: {} - {}", request.tick_lower_price, request.tick_upper_price);
        info!("  输入金额: {}", request.input_amount);

        // 1. 验证请求参数
        self.validate_increase_liquidity_request(&request)?;

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

        let user_keypair = Keypair::from_base58_string(private_key);

        // 3-13. 执行与increase_liquidity相同的逻辑来构建指令
        let pool_account = self.shared.rpc_client.get_account(&pool_address)?;
        let pool_state: raydium_amm_v3::states::PoolState = SolanaUtils::deserialize_anchor_account(&pool_account)?;

        let position_utils = PositionUtils::new(&self.shared.rpc_client);

        let sqrt_price_lower = position_utils.price_to_sqrt_price_x64(request.tick_lower_price, pool_state.mint_decimals_0, pool_state.mint_decimals_1);
        let sqrt_price_upper = position_utils.price_to_sqrt_price_x64(request.tick_upper_price, pool_state.mint_decimals_0, pool_state.mint_decimals_1);

        let tick_lower_raw = raydium_amm_v3::libraries::tick_math::get_tick_at_sqrt_price(sqrt_price_lower)?;
        let tick_upper_raw = raydium_amm_v3::libraries::tick_math::get_tick_at_sqrt_price(sqrt_price_upper)?;

        let tick_lower_adjusted = position_utils.tick_with_spacing(tick_lower_raw, pool_state.tick_spacing as i32);
        let tick_upper_adjusted = position_utils.tick_with_spacing(tick_upper_raw, pool_state.tick_spacing as i32);

        let sqrt_price_lower_adjusted = raydium_amm_v3::libraries::tick_math::get_sqrt_price_at_tick(tick_lower_adjusted)?;
        let sqrt_price_upper_adjusted = raydium_amm_v3::libraries::tick_math::get_sqrt_price_at_tick(tick_upper_adjusted)?;

        // 查找现有的匹配仓位
        let existing_position = position_utils
            .find_existing_position(&user_wallet, &pool_address, tick_lower_adjusted, tick_upper_adjusted)
            .await?
            .ok_or_else(|| anyhow::anyhow!("未找到匹配的现有仓位。增加流动性需要先有相同价格范围的仓位。"))?;

        info!("🎯 找到匹配的现有仓位:");
        info!("  NFT Mint: {}", existing_position.nft_mint);
        info!("  NFT Token Account: {}", existing_position.nft_token_account);
        info!("  NFT Token Program: {}", existing_position.nft_token_program);

        // 验证NFT Token Program类型
        if existing_position.nft_token_program == spl_token_2022::id() {
            info!("✅ 检测到Token-2022 NFT，使用IncreaseLiquidityV2指令");
        } else if existing_position.nft_token_program == spl_token::id() {
            info!("✅ 检测到Legacy SPL Token NFT，使用IncreaseLiquidityV2指令（向后兼容）");
        } else {
            warn!("⚠️ 检测到未知的Token Program: {}", existing_position.nft_token_program);
        }

        let liquidity = position_utils.calculate_liquidity_from_single_amount(
            pool_state.sqrt_price_x64,
            sqrt_price_lower_adjusted,
            sqrt_price_upper_adjusted,
            request.input_amount,
            request.is_base_0,
        )?;

        let (amount_0, amount_1) =
            position_utils.calculate_amounts_from_liquidity(pool_state.tick_current, pool_state.sqrt_price_x64, tick_lower_adjusted, tick_upper_adjusted, liquidity)?;

        let slippage = if request.max_slippage_percent == 0.5 { 5.0 } else { request.max_slippage_percent };
        let amount_0_with_slippage = position_utils.apply_slippage(amount_0, slippage, false);
        let amount_1_with_slippage = position_utils.apply_slippage(amount_1, slippage, false);

        let (transfer_fee_0, transfer_fee_1) =
            self.shared
                .swap_v2_service
                .get_pool_mints_inverse_fee(&pool_state.token_mint_0, &pool_state.token_mint_1, amount_0_with_slippage, amount_1_with_slippage)?;

        let amount_0_max = amount_0_with_slippage.checked_add(transfer_fee_0.transfer_fee).ok_or_else(|| anyhow::anyhow!("金额溢出"))?;
        let amount_1_max = amount_1_with_slippage.checked_add(transfer_fee_1.transfer_fee).ok_or_else(|| anyhow::anyhow!("金额溢出"))?;

        let mut remaining_accounts = Vec::new();
        let raydium_program_id = ConfigManager::get_raydium_program_id()?;
        let (tickarray_bitmap_extension, _) = Pubkey::find_program_address(&[b"pool_tick_array_bitmap_extension", pool_address.as_ref()], &raydium_program_id);
        remaining_accounts.push(AccountMeta::new(tickarray_bitmap_extension, false));

        let tick_array_lower_start_index = raydium_amm_v3::states::TickArrayState::get_array_start_index(tick_lower_adjusted, pool_state.tick_spacing);
        let tick_array_upper_start_index = raydium_amm_v3::states::TickArrayState::get_array_start_index(tick_upper_adjusted, pool_state.tick_spacing);

        let user_token_account_0 = spl_associated_token_account::get_associated_token_address_with_program_id(&user_wallet, &pool_state.token_mint_0, &transfer_fee_0.owner);
        let user_token_account_1 = spl_associated_token_account::get_associated_token_address_with_program_id(
            &user_wallet,
            &pool_state.token_mint_1,
            &transfer_fee_1.owner, // 修复CLI中的bug：应该使用transfer_fee_1.owner
        );

        let instructions = PositionInstructionBuilder::build_increase_liquidity_instructions(
            &pool_address,
            &pool_state,
            &user_wallet,
            &existing_position.nft_mint,
            &existing_position.nft_token_account,
            &user_token_account_0,
            &user_token_account_1,
            tick_lower_adjusted,
            tick_upper_adjusted,
            tick_array_lower_start_index,
            tick_array_upper_start_index,
            liquidity,
            amount_0_max,
            amount_1_max,
            remaining_accounts,
        )?;

        // 14. 构建并发送交易
        let recent_blockhash = self.shared.rpc_client.get_latest_blockhash()?;
        let transaction = Transaction::new_signed_with_payer(&instructions, Some(&user_wallet), &[&user_keypair], recent_blockhash);

        // 15. 发送交易
        let signature = self.shared.rpc_client.send_and_confirm_transaction(&transaction)?;

        info!("✅ 增加流动性成功，交易签名: {}", signature);

        // 构建响应
        let explorer_url = format!("https://explorer.solana.com/tx/{}", signature);
        let now = chrono::Utc::now().timestamp();

        Ok(IncreaseLiquidityAndSendTransactionResponse {
            signature: signature.to_string(),
            position_key: existing_position.position_key.to_string(),
            liquidity_added: liquidity.to_string(),
            amount_0: amount_0_max,
            amount_1: amount_1_max,
            tick_lower_index: tick_lower_adjusted,
            tick_upper_index: tick_upper_adjusted,
            pool_address: request.pool_address,
            status: TransactionStatus::Finalized,
            explorer_url,
            timestamp: now,
        })
    }

    /// 验证增加流动性请求参数
    fn validate_increase_liquidity_request(&self, request: &IncreaseLiquidityRequest) -> Result<()> {
        // 验证价格范围
        if request.tick_lower_price >= request.tick_upper_price {
            return Err(anyhow::anyhow!("下限价格必须小于上限价格"));
        }

        // 验证输入金额
        if request.input_amount == 0 {
            return Err(anyhow::anyhow!("输入金额必须大于0"));
        }

        // 验证滑点
        if request.max_slippage_percent < 0.0 || request.max_slippage_percent > 100.0 {
            return Err(anyhow::anyhow!("滑点百分比必须在0-100之间"));
        }

        Ok(())
    }

    /// 减少流动性（构建交易）
    pub async fn decrease_liquidity(&self, request: DecreaseLiquidityRequest) -> Result<DecreaseLiquidityResponse> {
        info!("🔧 开始构建减少流动性交易");
        info!("  池子地址: {}", request.pool_address);
        info!("  用户钱包: {}", request.user_wallet);
        info!("  Tick范围: {} - {}", request.tick_lower_index, request.tick_upper_index);
        info!("  减少流动性: {:?}", request.liquidity);

        // 1. 验证请求参数
        self.validate_decrease_liquidity_request(&request)?;

        // 2. 解析参数
        let pool_address = Pubkey::from_str(&request.pool_address)?;
        let user_wallet = Pubkey::from_str(&request.user_wallet)?;

        // 3. 加载池子状态
        let pool_account = self.shared.rpc_client.get_account(&pool_address)?;
        let pool_state: raydium_amm_v3::states::PoolState = SolanaUtils::deserialize_anchor_account(&pool_account)?;

        // 4. 查找现有的匹配仓位
        let position_utils = PositionUtils::new(&self.shared.rpc_client);
        let existing_position = position_utils
            .find_existing_position(&user_wallet, &pool_address, request.tick_lower_index, request.tick_upper_index)
            .await?
            .ok_or_else(|| anyhow::anyhow!("未找到匹配的仓位。请检查tick索引范围和池子地址。"))?;

        info!("🎯 找到匹配的现有仓位:");
        info!("  NFT Mint: {}", existing_position.nft_mint);
        info!("  Position Key: {}", existing_position.position_key);
        info!("  Current Liquidity: {}", existing_position.liquidity);

        // 5. 确定要减少的流动性数量
        let liquidity_to_remove = if let Some(liquidity_str) = &request.liquidity {
            liquidity_str.parse::<u128>()?
        } else {
            existing_position.liquidity // 减少全部流动性
        };

        if liquidity_to_remove > existing_position.liquidity {
            return Err(anyhow::anyhow!("要减少的流动性数量不能超过现有仓位的流动性"));
        }

        // 6. 计算减少流动性后可获得的代币数量（使用负值流动性）
        let (amount_0_raw, amount_1_raw) = {
            // 对于减少流动性，我们需要使用负的流动性值
            let negative_liquidity = -(liquidity_to_remove as i128);
            raydium_amm_v3::libraries::liquidity_math::get_delta_amounts_signed(
                pool_state.tick_current,
                pool_state.sqrt_price_x64,
                request.tick_lower_index,
                request.tick_upper_index,
                negative_liquidity,
            )
            .map_err(|e| anyhow::anyhow!("计算减少流动性金额失败: {:?}", e))?
        };

        // 对于减少流动性，返回的是用户能获得的代币数量（已经是正数）
        let amount_0_expected = amount_0_raw;
        let amount_1_expected = amount_1_raw;

        // 7. 应用滑点保护
        let slippage = request.max_slippage_percent.unwrap_or(0.5) / 100.0; // 转换为小数
        let amount_0_with_slippage = position_utils.apply_slippage(amount_0_expected, slippage, false); // false表示减少
        let amount_1_with_slippage = position_utils.apply_slippage(amount_1_expected, slippage, false);

        // 8. 计算转账费
        let (transfer_fee_0, transfer_fee_1) =
            self.shared
                .swap_v2_service
                .get_pool_mints_transfer_fee(&pool_state.token_mint_0, &pool_state.token_mint_1, amount_0_with_slippage, amount_1_with_slippage)?;

        // 9. 计算最小输出金额（减去转账费）
        info!("transfer_fee_0: {:?}", transfer_fee_0);
        info!("transfer_fee_1: {:?}", transfer_fee_1);
        info!("amount_0_with_slippage: {:?}", amount_0_with_slippage);
        info!("amount_1_with_slippage: {:?}", amount_1_with_slippage);
        let amount_0_min = if amount_0_with_slippage > 0 {
            amount_0_with_slippage
                .checked_sub(transfer_fee_0.transfer_fee)
                .ok_or_else(|| anyhow::anyhow!("转账费超过预期获得金额"))?
        } else {
            amount_0_with_slippage
        };
        let amount_1_min = if amount_1_with_slippage > 0 {
            amount_1_with_slippage
                .checked_sub(transfer_fee_1.transfer_fee)
                .ok_or_else(|| anyhow::anyhow!("转账费超过预期获得金额"))?
        } else {
            amount_1_with_slippage
        };

        // 10. 构建remaining accounts（包含奖励账户）
        let mut remaining_accounts = Vec::new();
        let raydium_program_id = ConfigManager::get_raydium_program_id()?;
        let (tickarray_bitmap_extension, _) = Pubkey::find_program_address(&[b"pool_tick_array_bitmap_extension", pool_address.as_ref()], &raydium_program_id);
        remaining_accounts.push(AccountMeta::new(tickarray_bitmap_extension, false));

        // 添加奖励相关账户（与CLI版本保持100%一致）
        //
        // 重要说明：智能合约验证逻辑分析
        // 1. decrease_liquidity.rs:275-285 调用 collect_rewards 函数
        // 2. 在 DecreaseLiquidityV2 指令中，token_program_2022 总是传递 Some(...)
        // 3. 因此 need_reward_mint 在 V2 版本中始终为 true
        // 4. CLI版本总是添加3个账户：vault + user_account + mint
        // 5. 智能合约验证：remaining_accounts.len() == valid_reward_count * 3

        // CLI版本的奖励账户构建（main.rs:1147-1153）：
        // - reward_info.token_vault (第1个账户)
        // - get_associated_token_address(&user, &reward_mint) (第2个账户)  
        // - reward_info.token_mint (第3个账户，V2版本中总是添加)

        // 验证奖励账户数量逻辑
        let mut valid_reward_count = 0;
        let reward_group_account_num = 3; // V2版本中始终为3个账户

        for reward_info in &pool_state.reward_infos {
            if reward_info.token_mint != Pubkey::default() {
                valid_reward_count += 1;
                // 第1个账户：reward token vault
                remaining_accounts.push(AccountMeta::new(reward_info.token_vault, false));
                // 第2个账户：user reward token account
                let user_reward_token = spl_associated_token_account::get_associated_token_address_with_program_id(
                    &user_wallet,
                    &reward_info.token_mint,
                    &TokenUtils::detect_mint_program(&self.shared.rpc_client, &reward_info.token_mint)?,
                );
                remaining_accounts.push(AccountMeta::new(user_reward_token, false));
                // 第3个账户：reward mint（V2版本中总是添加）
                remaining_accounts.push(AccountMeta::new(reward_info.token_mint, false));
            }
        }
        
        let expected_remaining_accounts = valid_reward_count * reward_group_account_num + 1; // +1 for tickarray_bitmap_extension
        info!(
            "🔧 奖励账户验证 - valid_reward_count: {}, reward_group_account_num: {}, expected_total: {}, actual: {}",
            valid_reward_count, reward_group_account_num, expected_remaining_accounts, remaining_accounts.len()
        );
        info!("🔧 构建减少流动性剩余账户remaining_accounts: {:?}", remaining_accounts);

        // 11. 计算tick array索引
        let tick_array_lower_start_index = raydium_amm_v3::states::TickArrayState::get_array_start_index(request.tick_lower_index, pool_state.tick_spacing);
        let tick_array_upper_start_index = raydium_amm_v3::states::TickArrayState::get_array_start_index(request.tick_upper_index, pool_state.tick_spacing);

        // 12. 构建用户代币账户地址
        let user_token_account_0 = spl_associated_token_account::get_associated_token_address_with_program_id(&user_wallet, &pool_state.token_mint_0, &transfer_fee_0.owner);
        let user_token_account_1 = spl_associated_token_account::get_associated_token_address_with_program_id(&user_wallet, &pool_state.token_mint_1, &transfer_fee_1.owner);

        // 13. 构建指令
        let mut instructions = Vec::new();

        // 确保所有有效奖励代币的用户ATA账户存在（仅在构建交易时需要）
        for reward_info in &pool_state.reward_infos {
            if reward_info.token_mint != Pubkey::default() {
                let reward_token_program = TokenUtils::detect_mint_program(&self.shared.rpc_client, &reward_info.token_mint)?;
                let reward_token_account = spl_associated_token_account::get_associated_token_address_with_program_id(&user_wallet, &reward_info.token_mint, &reward_token_program);
                info!("📝 确保用户奖励代币领取ATA账户存在: {}", reward_token_account);
                let create_reward_ata_ix =
                    spl_associated_token_account::instruction::create_associated_token_account_idempotent(&user_wallet, &user_wallet, &reward_info.token_mint, &reward_token_program);
                instructions.push(create_reward_ata_ix);
            }
        }

        let decrease_liquidity_instructions = PositionInstructionBuilder::build_decrease_liquidity_instructions(
            &pool_address,
            &pool_state,
            &user_wallet,
            &existing_position.nft_mint,
            &existing_position.nft_token_account,
            &user_token_account_0,
            &user_token_account_1,
            request.tick_lower_index,
            request.tick_upper_index,
            tick_array_lower_start_index,
            tick_array_upper_start_index,
            liquidity_to_remove,
            amount_0_min,
            amount_1_min,
            remaining_accounts,
        )?;
        instructions.extend(decrease_liquidity_instructions);

        // 14. 如果减少全部流动性，还要关闭仓位
        let will_close_position = liquidity_to_remove == existing_position.liquidity;
        if will_close_position {
            let close_position_instructions = PositionInstructionBuilder::build_close_position_instructions(
                &existing_position.nft_mint,
                &existing_position.nft_token_account,
                &existing_position.nft_token_program,
                &user_wallet,
            )?;
            instructions.extend(close_position_instructions);
        }

        // 15. 序列化交易
        let transaction = Transaction::new_unsigned(solana_sdk::message::Message::new(&instructions, Some(&user_wallet)));
        let serialized_transaction = bincode::serialize(&transaction)?;
        let transaction_base64 = BASE64_STANDARD.encode(&serialized_transaction);

        let transaction_message = format!(
            "减少流动性 - 池子: {}, 仓位: {}, 减少流动性: {}{}",
            &request.pool_address[..8],
            &existing_position.position_key.to_string()[..8],
            liquidity_to_remove,
            if will_close_position { ", 并关闭仓位" } else { "" }
        );

        let now = chrono::Utc::now().timestamp();

        Ok(DecreaseLiquidityResponse {
            transaction: transaction_base64,
            transaction_message,
            position_key: existing_position.position_key.to_string(),
            liquidity_removed: liquidity_to_remove.to_string(),
            amount_0_min,
            amount_1_min,
            amount_0_expected,
            amount_1_expected,
            tick_lower_index: request.tick_lower_index,
            tick_upper_index: request.tick_upper_index,
            pool_address: request.pool_address,
            will_close_position,
            timestamp: now,
        })
    }

    /// 减少流动性并发送交易
    pub async fn decrease_liquidity_and_send_transaction(&self, request: DecreaseLiquidityRequest) -> Result<DecreaseLiquidityAndSendTransactionResponse> {
        info!("🚀 开始减少流动性并发送交易");
        info!("  池子地址: {}", request.pool_address);
        info!("  用户钱包: {}", request.user_wallet);
        info!("  Tick范围: {} - {}", request.tick_lower_index, request.tick_upper_index);
        info!("  减少流动性: {:?}", request.liquidity);

        // 1. 验证请求参数
        self.validate_decrease_liquidity_request(&request)?;

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

        let user_keypair = Keypair::from_base58_string(private_key);

        // 3-15. 执行与decrease_liquidity相同的逻辑来构建指令
        // 重复所有的构建逻辑（为了保持代码一致性）
        let pool_account = self.shared.rpc_client.get_account(&pool_address)?;
        let pool_state: raydium_amm_v3::states::PoolState = SolanaUtils::deserialize_anchor_account(&pool_account)?;

        let position_utils = PositionUtils::new(&self.shared.rpc_client);
        let existing_position = position_utils
            .find_existing_position(&user_wallet, &pool_address, request.tick_lower_index, request.tick_upper_index)
            .await?
            .ok_or_else(|| anyhow::anyhow!("未找到匹配的仓位。请检查tick索引范围和池子地址。"))?;

        let liquidity_to_remove = if let Some(liquidity_str) = &request.liquidity {
            liquidity_str.parse::<u128>()?
        } else {
            existing_position.liquidity
        };

        if liquidity_to_remove > existing_position.liquidity {
            return Err(anyhow::anyhow!("要减少的流动性数量不能超过现有仓位的流动性"));
        }

        let (amount_0_raw, amount_1_raw) = {
            // 对于减少流动性，我们需要使用负的流动性值
            let negative_liquidity = -(liquidity_to_remove as i128);
            raydium_amm_v3::libraries::liquidity_math::get_delta_amounts_signed(
                pool_state.tick_current,
                pool_state.sqrt_price_x64,
                request.tick_lower_index,
                request.tick_upper_index,
                negative_liquidity,
            )
            .map_err(|e| anyhow::anyhow!("计算减少流动性金额失败: {:?}", e))?
        };

        let amount_0_expected = amount_0_raw;
        let amount_1_expected = amount_1_raw;

        let slippage = request.max_slippage_percent.unwrap_or(0.5) / 100.0;
        let amount_0_with_slippage = position_utils.apply_slippage(amount_0_expected, slippage, false);
        let amount_1_with_slippage = position_utils.apply_slippage(amount_1_expected, slippage, false);

        let (transfer_fee_0, transfer_fee_1) =
            self.shared
                .swap_v2_service
                .get_pool_mints_transfer_fee(&pool_state.token_mint_0, &pool_state.token_mint_1, amount_0_with_slippage, amount_1_with_slippage)?;
        info!("transfer_fee_0: {:?}", transfer_fee_0);
        info!("transfer_fee_1: {:?}", transfer_fee_1);
        info!("amount_0_with_slippage: {:?}", amount_0_with_slippage);
        info!("amount_1_with_slippage: {:?}", amount_1_with_slippage);
        let amount_0_min = if amount_0_with_slippage > 0 {
            amount_0_with_slippage
                .checked_sub(transfer_fee_0.transfer_fee)
                .ok_or_else(|| anyhow::anyhow!("转账费超过预期获得金额"))?
        } else {
            amount_0_with_slippage
        };
        let amount_1_min = if amount_1_with_slippage > 0 {
            amount_1_with_slippage
                .checked_sub(transfer_fee_1.transfer_fee)
                .ok_or_else(|| anyhow::anyhow!("转账费超过预期获得金额"))?
        } else {
            amount_1_with_slippage
        };

        let mut remaining_accounts = Vec::new();
        let raydium_program_id = ConfigManager::get_raydium_program_id()?;
        let (tickarray_bitmap_extension, _) = Pubkey::find_program_address(&[b"pool_tick_array_bitmap_extension", pool_address.as_ref()], &raydium_program_id);
        remaining_accounts.push(AccountMeta::new(tickarray_bitmap_extension, false));

        // 添加奖励相关账户（与CLI版本保持100%一致）
        //
        // 重要说明：智能合约验证逻辑分析
        // 1. decrease_liquidity.rs:275-285 调用 collect_rewards 函数
        // 2. 在 DecreaseLiquidityV2 指令中，token_program_2022 总是传递 Some(...)
        // 3. 因此 need_reward_mint 在 V2 版本中始终为 true
        // 4. CLI版本总是添加3个账户：vault + user_account + mint
        // 5. 智能合约验证：remaining_accounts.len() == valid_reward_count * 3

        // CLI版本的奖励账户构建（main.rs:1147-1153）：
        // - reward_info.token_vault (第1个账户)
        // - get_associated_token_address(&user, &reward_mint) (第2个账户)
        // - reward_info.token_mint (第3个账户，V2版本中总是添加)

        // 验证奖励账户数量逻辑
        let mut valid_reward_count = 0;
        let reward_group_account_num = 3; // V2版本中始终为3个账户
        
        for reward_info in &pool_state.reward_infos {
            if reward_info.token_mint != Pubkey::default() {
                valid_reward_count += 1;
                // 第1个账户：reward token vault
                remaining_accounts.push(AccountMeta::new(reward_info.token_vault, false));
                // 第2个账户：user reward token account
                let user_reward_token = spl_associated_token_account::get_associated_token_address_with_program_id(
                    &user_wallet,
                    &reward_info.token_mint,
                    &TokenUtils::detect_mint_program(&self.shared.rpc_client, &reward_info.token_mint)?,
                );
                remaining_accounts.push(AccountMeta::new(user_reward_token, false));
                // 第3个账户：reward mint（V2版本中总是添加）
                remaining_accounts.push(AccountMeta::new(reward_info.token_mint, false));
            }
        }
        
        let expected_remaining_accounts = valid_reward_count * reward_group_account_num + 1; // +1 for tickarray_bitmap_extension
        info!(
            "🔧 奖励账户验证 - valid_reward_count: {}, reward_group_account_num: {}, expected_total: {}, actual: {}",
            valid_reward_count, reward_group_account_num, expected_remaining_accounts, remaining_accounts.len()
        );
        info!("🔧 构建减少流动性剩余账户remaining_accounts: {:?}", remaining_accounts);

        let tick_array_lower_start_index = raydium_amm_v3::states::TickArrayState::get_array_start_index(request.tick_lower_index, pool_state.tick_spacing);
        let tick_array_upper_start_index = raydium_amm_v3::states::TickArrayState::get_array_start_index(request.tick_upper_index, pool_state.tick_spacing);

        let user_token_account_0 = spl_associated_token_account::get_associated_token_address_with_program_id(&user_wallet, &pool_state.token_mint_0, &transfer_fee_0.owner);
        let user_token_account_1 = spl_associated_token_account::get_associated_token_address_with_program_id(&user_wallet, &pool_state.token_mint_1, &transfer_fee_1.owner);

        // 打印所有构建指令需要的账户，排查问题
        info!("pool_address: {:?}", pool_address);
        info!("pool_state: {:?}", pool_state);
        info!("user_wallet: {:?}", user_wallet);
        info!("existing_position.nft_mint: {:?}", existing_position.nft_mint);
        info!("existing_position.nft_token_account: {:?}", existing_position.nft_token_account);
        info!("user_token_account_0: {:?}", user_token_account_0);
        info!("user_token_account_1: {:?}", user_token_account_1);
        info!("request.tick_lower_index: {:?}", request.tick_lower_index);
        info!("request.tick_upper_index: {:?}", request.tick_upper_index);
        info!("tick_array_lower_start_index: {:?}", tick_array_lower_start_index);
        info!("tick_array_upper_start_index: {:?}", tick_array_upper_start_index);
        info!("liquidity_to_remove: {:?}", liquidity_to_remove);
        info!("amount_0_min: {:?}", amount_0_min);
        info!("amount_1_min: {:?}", amount_1_min);
        info!("remaining_accounts: {:?}", remaining_accounts);

        let mut instructions = Vec::new();

        // 确保所有有效奖励代币的用户ATA账户存在
        for reward_info in &pool_state.reward_infos {
            if reward_info.token_mint != Pubkey::default() {
                let reward_token_program = TokenUtils::detect_mint_program(&self.shared.rpc_client, &reward_info.token_mint)?;
                let reward_token_account = spl_associated_token_account::get_associated_token_address_with_program_id(&user_wallet, &reward_info.token_mint, &reward_token_program);
                info!("📝 确保用户奖励代币领取ATA账户存在: {}", reward_token_account);
                let create_reward_ata_ix =
                    spl_associated_token_account::instruction::create_associated_token_account_idempotent(&user_wallet, &user_wallet, &reward_info.token_mint, &reward_token_program);
                instructions.push(create_reward_ata_ix);
            }
        }

        let decrease_liquidity_instructions = PositionInstructionBuilder::build_decrease_liquidity_instructions(
            &pool_address,
            &pool_state,
            &user_wallet,
            &existing_position.nft_mint,
            &existing_position.nft_token_account,
            &user_token_account_0,
            &user_token_account_1,
            request.tick_lower_index,
            request.tick_upper_index,
            tick_array_lower_start_index,
            tick_array_upper_start_index,
            liquidity_to_remove,
            amount_0_min,
            amount_1_min,
            remaining_accounts,
        )?;
        instructions.extend(decrease_liquidity_instructions);

        let will_close_position = liquidity_to_remove == existing_position.liquidity;
        if will_close_position {
            let close_position_instructions = PositionInstructionBuilder::build_close_position_instructions(
                &existing_position.nft_mint,
                &existing_position.nft_token_account,
                &existing_position.nft_token_program,
                &user_wallet,
            )?;
            instructions.extend(close_position_instructions);
        }

        // 16. 构建并发送交易
        let recent_blockhash = self.shared.rpc_client.get_latest_blockhash()?;
        let transaction = Transaction::new_signed_with_payer(&instructions, Some(&user_wallet), &[&user_keypair], recent_blockhash);

        // 17. 发送交易
        let signature = if request.simulate {
            // 模拟交易
            let simulation_result = self.shared.rpc_client.simulate_transaction(&transaction)?;
            info!("📋 交易模拟结果: {:?}", simulation_result);
            "simulation".to_string()
        } else {
            // 发送实际交易
            let sig = self.shared.rpc_client.send_and_confirm_transaction(&transaction)?;
            info!("✅ 减少流动性成功，交易签名: {}", sig);
            sig.to_string()
        };

        // 构建响应
        let explorer_url = if request.simulate {
            "simulation".to_string()
        } else {
            format!("https://explorer.solana.com/tx/{}", signature)
        };
        let now = chrono::Utc::now().timestamp();

        Ok(DecreaseLiquidityAndSendTransactionResponse {
            signature,
            position_key: existing_position.position_key.to_string(),
            liquidity_removed: liquidity_to_remove.to_string(),
            amount_0_actual: amount_0_expected, // 在实际实现中，应该从交易日志中解析
            amount_1_actual: amount_1_expected,
            tick_lower_index: request.tick_lower_index,
            tick_upper_index: request.tick_upper_index,
            pool_address: request.pool_address,
            position_closed: will_close_position,
            status: if request.simulate { TransactionStatus::Simulated } else { TransactionStatus::Finalized },
            explorer_url,
            timestamp: now,
        })
    }

    /// 验证减少流动性请求参数
    fn validate_decrease_liquidity_request(&self, request: &DecreaseLiquidityRequest) -> Result<()> {
        // 验证tick范围
        if request.tick_lower_index >= request.tick_upper_index {
            return Err(anyhow::anyhow!("下限tick索引必须小于上限tick索引"));
        }

        // 验证流动性数量（如果提供）
        if let Some(liquidity_str) = &request.liquidity {
            let liquidity = liquidity_str.parse::<u128>().map_err(|_| anyhow::anyhow!("流动性数量格式错误"))?;
            if liquidity == 0 {
                return Err(anyhow::anyhow!("流动性数量必须大于0"));
            }
        }

        // 验证滑点
        if let Some(slippage) = request.max_slippage_percent {
            if slippage < 0.0 || slippage > 100.0 {
                return Err(anyhow::anyhow!("滑点百分比必须在0-100之间"));
            }
        }

        Ok(())
    }
}
