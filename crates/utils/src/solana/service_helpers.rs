use anyhow::Result;
use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use tracing::{info, warn};

use crate::ErrorHandler;

use super::{ConfigManager, LogUtils, MathUtils, PDACalculator, PoolInfoManager, SwapCalculator, TokenUtils};

/// 服务层辅助工具 - 抽取服务层的通用逻辑
pub struct ServiceHelpers<'a> {
    rpc_client: &'a RpcClient,
    swap_calculator: SwapCalculator<'a>,
}

impl<'a> ServiceHelpers<'a> {
    pub fn new(rpc_client: &'a RpcClient) -> Self {
        Self {
            rpc_client,
            swap_calculator: SwapCalculator::new(rpc_client),
        }
    }

    /// 使用PDA计算池子地址
    pub fn calculate_pool_address_pda(&self, input_mint: &str, output_mint: &str) -> Result<String> {
        LogUtils::log_operation_start("PDA池子地址计算", &format!("输入: {} -> 输出: {}", input_mint, output_mint));

        let result = PoolInfoManager::calculate_pool_address_pda(input_mint, output_mint)?;

        LogUtils::log_operation_success("PDA池子地址计算", &result);
        Ok(result)
    }

    /// 基于输入金额计算输出（base-in模式）
    pub async fn calculate_output_for_input_with_slippage(&self, input_mint: &str, output_mint: &str, input_amount: u64, slippage_bps: u16) -> Result<(u64, u64, String)> {
        // 使用PDA方法计算池子地址
        let pool_address = self.calculate_pool_address_pda(input_mint, output_mint)?;
        info!("使用与CLI完全相同的交换计算逻辑");
        info!("  池子地址: {}", pool_address);
        info!("  输入金额: {}", input_amount);

        // 使用与CLI完全相同的计算逻辑
        match self
            .swap_calculator
            .calculate_output_using_cli_logic(input_mint, output_mint, input_amount, &pool_address, true, slippage_bps)
            .await
        {
            Ok((output_amount, other_amount_threshold)) => {
                info!("  ✅ CLI逻辑计算成功: {} -> {} (阈值: {})", input_amount, output_amount, other_amount_threshold);
                Ok((output_amount, other_amount_threshold, pool_address))
            }
            Err(e) => {
                warn!("  ⚠️ CLI逻辑计算失败: {:?}，使用备用计算", e);
                // 如果计算失败，使用备用简化计算
                let output_amount = self.fallback_price_calculation(input_mint, output_mint, input_amount).await?;
                let other_amount_threshold = MathUtils::calculate_minimum_amount_out(output_amount, slippage_bps);
                Ok((output_amount, other_amount_threshold, pool_address))
            }
        }
    }

    /// 备用价格计算方法
    async fn fallback_price_calculation(&self, from_token: &str, to_token: &str, amount: u64) -> Result<u64> {
        info!("🔄 使用备用价格计算");

        let from_type = TokenUtils::get_token_type(from_token);
        let to_type = TokenUtils::get_token_type(to_token);

        let estimated_output = match (from_type, to_type) {
            (super::TokenType::Sol, super::TokenType::Usdc) => MathUtils::convert_sol_to_usdc(amount),
            (super::TokenType::Usdc, super::TokenType::Sol) => MathUtils::convert_usdc_to_sol(amount),
            _ => return Err(anyhow::anyhow!("不支持的交换对: {} -> {}", from_token, to_token)),
        };

        info!("  💰 备用计算结果: {}", estimated_output);
        Ok(estimated_output)
    }

    /// 基于输出金额计算输入（base-out模式）
    pub async fn calculate_input_for_output_with_slippage(&self, input_mint: &str, output_mint: &str, desired_output_amount: u64, slippage_bps: u16) -> Result<(u64, u64, String)> {
        // 使用PDA方法计算池子地址
        let pool_address = self.calculate_pool_address_pda(input_mint, output_mint)?;
        info!("使用与CLI完全相同的交换计算逻辑（BaseOut模式）");
        info!("  池子地址: {}", pool_address);
        info!("  期望输出金额: {}", desired_output_amount);

        // 使用与CLI完全相同的计算逻辑，但是是BaseOut模式
        match self
            .swap_calculator
            .calculate_output_using_cli_logic(
                input_mint,
                output_mint,
                desired_output_amount,
                &pool_address,
                false, // base_out = false
                slippage_bps,
            )
            .await
        {
            Ok((required_input_amount, other_amount_threshold)) => {
                info!(
                    "  ✅ CLI逻辑计算成功（BaseOut）: 需要输入 {} 来获得 {} 输出 (最大输入阈值: {})",
                    required_input_amount, desired_output_amount, other_amount_threshold
                );
                Ok((required_input_amount, other_amount_threshold, pool_address))
            }
            Err(e) => {
                warn!("  ⚠️ CLI逻辑计算失败: {:?}，使用备用计算", e);
                // 如果计算失败，使用备用简化计算
                let required_input_amount = self.fallback_input_calculation(input_mint, output_mint, desired_output_amount).await?;
                let other_amount_threshold = MathUtils::calculate_maximum_amount_in(required_input_amount, slippage_bps);
                Ok((required_input_amount, other_amount_threshold, pool_address))
            }
        }
    }

    /// 备用输入计算方法（BaseOut模式）
    async fn fallback_input_calculation(&self, input_mint: &str, output_mint: &str, desired_output_amount: u64) -> Result<u64> {
        info!("🔄 使用备用输入计算（BaseOut模式）");

        let input_type = TokenUtils::get_token_type(input_mint);
        let output_type = TokenUtils::get_token_type(output_mint);

        let required_input = match (input_type, output_type) {
            (super::TokenType::Sol, super::TokenType::Usdc) => MathUtils::convert_usdc_to_sol(desired_output_amount),
            (super::TokenType::Usdc, super::TokenType::Sol) => MathUtils::convert_sol_to_usdc(desired_output_amount),
            _ => return Err(anyhow::anyhow!("不支持的交换对: {} -> {}", input_mint, output_mint)),
        };

        info!("  💰 备用计算结果: 需要输入 {} 来获得 {} 输出", required_input, desired_output_amount);
        Ok(required_input)
    }

    /// 创建路由计划
    pub async fn create_route_plan(&self, pool_id: String, input_mint: String, output_mint: String, fee_amount: u64, amount_specified: u64) -> Result<serde_json::Value> {
        LogUtils::log_operation_start("路由计划创建", &format!("池子: {}", pool_id));

        // 获取正确的remaining accounts和pool price
        let (remaining_accounts, last_pool_price_x64) = self.get_remaining_accounts_and_pool_price(&pool_id, &input_mint, &output_mint, amount_specified).await?;

        let route_plan = serde_json::json!({
            "pool_id": pool_id,
            "input_mint": input_mint.clone(),
            "output_mint": output_mint.clone(),
            "fee_mint": input_mint, // 通常手续费使用输入代币
            "fee_rate": 25,         // 0.25% 手续费率（Raydium标准）
            "fee_amount": fee_amount.to_string(),
            "remaining_accounts": remaining_accounts,
            "last_pool_price_x64": last_pool_price_x64,
        });

        LogUtils::log_operation_success("路由计划创建", "路由计划已生成");
        Ok(route_plan)
    }

    /// 获取remaining accounts和pool price
    async fn get_remaining_accounts_and_pool_price(&self, pool_id: &str, input_mint: &str, output_mint: &str, amount_specified: u64) -> Result<(Vec<String>, String)> {
        info!("🔍 使用CLI完全相同逻辑获取remainingAccounts和lastPoolPriceX64");
        info!("  池子ID: {}", pool_id);
        info!("  输入代币: {}", input_mint);
        info!("  输出代币: {}", output_mint);
        info!("  扣除转账费后的金额: {}", amount_specified);

        // 尝试使用本地计算
        match self.get_remaining_accounts_and_pool_price_local(pool_id, input_mint, output_mint, amount_specified).await {
            Ok(result) => Ok(result),
            Err(e) => {
                warn!("⚠️ 本地计算失败: {:?}，尝试使用官方API", e);
                // 备用方案：调用官方API获取正确的值
                self.swap_calculator.get_remaining_accounts_from_official_api(pool_id, input_mint, output_mint, amount_specified).await
            }
        }
    }

    /// 本地计算remaining accounts和pool price
    async fn get_remaining_accounts_and_pool_price_local(&self, pool_id: &str, input_mint: &str, output_mint: &str, amount_specified: u64) -> Result<(Vec<String>, String)> {
        LogUtils::log_operation_start("本地remaining accounts计算", pool_id);

        let pool_pubkey = Pubkey::from_str(pool_id)?;
        let input_mint_pubkey = Pubkey::from_str(input_mint)?;
        let output_mint_pubkey = Pubkey::from_str(output_mint)?;

        // 使用工具类进行配置和PDA计算
        let raydium_program_id = ConfigManager::get_raydium_program_id()?;
        let amm_config_index = ConfigManager::get_amm_config_index();
        let (amm_config_key, _) = PDACalculator::calculate_amm_config_pda(&raydium_program_id, amm_config_index);
        let (tickarray_bitmap_extension_pda, _) = PDACalculator::calculate_tickarray_bitmap_extension_pda(&raydium_program_id, &pool_pubkey);

        // 使用工具类标准化mint顺序
        let (mint0, mint1, zero_for_one) = TokenUtils::normalize_mint_order(&input_mint_pubkey, &output_mint_pubkey);

        LogUtils::log_debug_info(
            "计算参数",
            &[
                ("mint0", &mint0.to_string()),
                ("mint1", &mint1.to_string()),
                ("zero_for_one", &zero_for_one.to_string()),
                ("pool_pubkey", &pool_pubkey.to_string()),
            ],
        );

        // 批量加载账户
        let load_accounts = vec![input_mint_pubkey, output_mint_pubkey, amm_config_key, pool_pubkey, tickarray_bitmap_extension_pda, mint0, mint1];

        let accounts = self.rpc_client.get_multiple_accounts(&load_accounts)?;

        // 使用统一的错误处理
        let amm_config_account = accounts[2].as_ref().ok_or_else(|| ErrorHandler::handle_account_load_error("AMM配置"))?;
        let pool_account = accounts[3].as_ref().ok_or_else(|| ErrorHandler::handle_account_load_error("池子"))?;
        let tickarray_bitmap_extension_account = accounts[4].as_ref().ok_or_else(|| ErrorHandler::handle_account_load_error("bitmap扩展"))?;

        // 反序列化关键状态
        let amm_config_state: raydium_amm_v3::states::AmmConfig = self.deserialize_anchor_account(amm_config_account)?;
        let pool_state: raydium_amm_v3::states::PoolState = self.deserialize_anchor_account(pool_account)?;
        let tickarray_bitmap_extension: raydium_amm_v3::states::TickArrayBitmapExtension = self.deserialize_anchor_account(tickarray_bitmap_extension_account)?;

        let epoch = self.rpc_client.get_epoch_info()?.epoch;
        LogUtils::log_debug_info("计算状态", &[("epoch", &epoch.to_string()), ("amount_specified", &amount_specified.to_string())]);

        // 加载tick arrays
        let mut tick_arrays = self
            .swap_calculator
            .load_cur_and_next_five_tick_array_like_cli(&pool_state, &tickarray_bitmap_extension, zero_for_one, &raydium_program_id, &pool_pubkey)
            .await?;

        // 执行计算
        let (_other_amount_threshold, tick_array_indexs) =
            self.swap_calculator
                .get_output_amount_and_remaining_accounts_cli_exact(amount_specified, None, zero_for_one, true, &amm_config_state, &pool_state, &tickarray_bitmap_extension, &mut tick_arrays)?;

        // 构建remaining accounts
        let mut remaining_accounts = Vec::new();
        remaining_accounts.push(tickarray_bitmap_extension_pda.to_string());

        for tick_index in tick_array_indexs {
            let (tick_array_key, _) = PDACalculator::calculate_tick_array_pda(&raydium_program_id, &pool_pubkey, tick_index);
            remaining_accounts.push(tick_array_key.to_string());
        }

        let last_pool_price_x64 = pool_state.sqrt_price_x64;
        let last_pool_price_x64 = last_pool_price_x64.to_string();

        LogUtils::log_operation_success("本地remaining accounts计算", &format!("{}个账户", remaining_accounts.len()));
        Ok((remaining_accounts, last_pool_price_x64))
    }

    /// 计算价格影响（简化版本，与TypeScript一致）
    pub async fn calculate_price_impact_simple(&self, input_mint: &str, output_mint: &str, input_amount: u64, pool_address: &str) -> Result<f64> {
        self.swap_calculator.calculate_price_impact_simple(input_mint, output_mint, input_amount, pool_address).await
    }

    /// 计算价格影响
    pub async fn calculate_price_impact(&self, input_mint: &str, output_mint: &str, input_amount: u64, output_amount: u64, pool_address: &str) -> Result<f64> {
        self.swap_calculator.calculate_price_impact(input_mint, output_mint, input_amount, output_amount, pool_address).await
    }

    /// 解析金额字符串
    pub fn parse_amount(&self, amount_str: &str) -> Result<u64> {
        amount_str.parse::<u64>().map_err(|e| anyhow::anyhow!("金额格式错误: {}", e))
    }

    /// 反序列化anchor账户
    fn deserialize_anchor_account<T: anchor_lang::AccountDeserialize>(&self, account: &solana_sdk::account::Account) -> Result<T> {
        let mut data: &[u8] = &account.data;
        T::try_deserialize(&mut data).map_err(Into::into)
    }

    /// 构建交易数据
    pub fn build_transaction_data(&self, instructions: Vec<solana_sdk::instruction::Instruction>, user_wallet: &Pubkey) -> Result<serde_json::Value> {
        let recent_blockhash = self.rpc_client.get_latest_blockhash()?;
        let transaction = super::TransactionBuilder::build_transaction(instructions, user_wallet, recent_blockhash)?;
        let transaction_base64 = super::TransactionBuilder::serialize_transaction_to_base64(&transaction)?;

        Ok(serde_json::json!({
            "transaction": transaction_base64,
        }))
    }

    /// 构建池子相关的vault信息
    pub fn build_vault_info(&self, pool_state: &raydium_amm_v3::states::PoolState, input_mint: &Pubkey) -> (Pubkey, Pubkey, Pubkey, Pubkey) {
        if *input_mint == pool_state.token_mint_0 {
            (pool_state.token_vault_0, pool_state.token_vault_1, pool_state.token_mint_0, pool_state.token_mint_1)
        } else {
            (pool_state.token_vault_1, pool_state.token_vault_0, pool_state.token_mint_1, pool_state.token_mint_0)
        }
    }
}
