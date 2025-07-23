use crate::dtos::solana_dto::{
    ComputeSwapV2Request, PriceQuoteRequest, PriceQuoteResponse, RoutePlan, SwapComputeV2Data, SwapRequest, SwapResponse, TransactionData, TransactionStatus,
    TransactionSwapV2Request, TransferFeeInfo,
};

use crate::services::solana::shared::SharedContext;

use ::utils::solana::{LogUtils, MathUtils, RoutePlanBuilder, ServiceHelpers, TokenType, TokenUtils};
use anyhow::Result;
use chrono;
use serde_json;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use std::sync::Arc;
use tracing::{info, warn};

/// SwapService handles all swap-related operations
pub struct SwapService {
    shared: Arc<SharedContext>,
}

impl SwapService {
    /// Create a new SwapService instance
    pub fn new(shared: Arc<SharedContext>) -> Self {
        Self { shared }
    }

    /// Execute token swap
    pub async fn swap_tokens(&self, request: SwapRequest) -> Result<SwapResponse> {
        self.execute_swap(request).await
    }

    /// Get price quote for a swap
    pub async fn get_price_quote(&self, request: PriceQuoteRequest) -> Result<PriceQuoteResponse> {
        info!("📊 获取价格报价");
        info!("  交换对: {} -> {}", request.from_token, request.to_token);
        info!("  池子地址: {}", request.pool_address);
        info!("  金额: {}", request.amount);

        let estimated_output = self
            .estimate_swap_output(&request.from_token, &request.to_token, &request.pool_address, request.amount)
            .await?;

        // 计算价格
        let price = if request.amount > 0 {
            estimated_output as f64 / request.amount as f64
        } else {
            0.0
        };

        // 简化的价格影响计算
        let price_impact_percent = 0.5; // 假设0.5%的价格影响

        // 建议最小输出金额（考虑5%滑点）
        let minimum_amount_out = (estimated_output as f64 * 0.95) as u64;

        let now = chrono::Utc::now().timestamp();

        Ok(PriceQuoteResponse {
            from_token: request.from_token,
            to_token: request.to_token,
            amount_in: request.amount,
            amount_out: estimated_output,
            price,
            price_impact_percent,
            minimum_amount_out,
            timestamp: now,
        })
    }

    /// Compute swap-v2-base-in (fixed input amount, supports transfer fee)
    pub async fn compute_swap_v2_base_in(&self, params: ComputeSwapV2Request) -> Result<SwapComputeV2Data> {
        LogUtils::log_operation_start("swap-v2-base-in计算", &format!("{} -> {}", params.input_mint, params.output_mint));

        let service_helpers = ServiceHelpers::new(&self.shared.rpc_client);
        let input_amount = service_helpers.parse_amount(&params.amount)?;
        let input_mint_pubkey = Pubkey::from_str(&params.input_mint)?;
        let output_mint_pubkey = Pubkey::from_str(&params.output_mint)?;

        // 计算转账费用
        let transfer_fee_info = if params.enable_transfer_fee.unwrap_or(false) {
            LogUtils::log_operation_start("transfer fee计算", "base-in模式");

            let input_transfer_fee = self.shared.swap_v2_service.get_transfer_fee(&input_mint_pubkey, input_amount)?;
            let input_mint_info = self.shared.swap_v2_service.load_mint_info(&input_mint_pubkey)?;
            let output_mint_info = self.shared.swap_v2_service.load_mint_info(&output_mint_pubkey)?;

            Some(TransferFeeInfo {
                input_transfer_fee: input_transfer_fee.transfer_fee,
                output_transfer_fee: 0,
                input_mint_decimals: input_mint_info.decimals,
                output_mint_decimals: output_mint_info.decimals,
            })
        } else {
            None
        };

        let amount_specified = if let Some(ref fee_info) = transfer_fee_info {
            input_amount.checked_sub(fee_info.input_transfer_fee).unwrap_or(input_amount)
        } else {
            input_amount
        };

        // 使用新的计算方法，包含滑点保护
        let (output_amount, other_amount_threshold, pool_address_str) = service_helpers
            .calculate_output_for_input_with_slippage(&params.input_mint, &params.output_mint, amount_specified, params.slippage_bps)
            .await?;

        let fee_amount = RoutePlanBuilder::calculate_standard_fee(amount_specified);
        let route_plan_json = service_helpers
            .create_route_plan(
                pool_address_str.clone(),
                params.input_mint.clone(),
                params.output_mint.clone(),
                fee_amount,
                amount_specified,
            )
            .await?;
        let route_plan = vec![self.create_route_plan_from_json(route_plan_json)?];

        let epoch = self.shared.swap_v2_service.get_current_epoch()?;

        // 计算真实的价格影响
        let price_impact_pct = match service_helpers
            .calculate_price_impact_simple(&params.input_mint, &params.output_mint, amount_specified, &pool_address_str)
            .await
        {
            Ok(impact) => Some(impact),
            Err(e) => {
                warn!("价格影响计算失败: {:?}，使用默认值", e);
                Some(0.1)
            }
        };

        let result = self.create_swap_compute_v2_data(
            "BaseIn".to_string(),
            params.input_mint,
            params.amount,
            params.output_mint,
            output_amount,
            other_amount_threshold, // 使用正确计算的阈值
            params.slippage_bps,
            route_plan,
            transfer_fee_info,
            Some(amount_specified),
            Some(epoch),
            price_impact_pct,
        );

        LogUtils::log_calculation_result(
            "swap-v2-base-in计算",
            amount_specified,
            output_amount,
            &[
                ("原始金额", &input_amount.to_string()),
                (
                    "转账费",
                    &transfer_fee_info
                        .as_ref()
                        .map(|f| f.input_transfer_fee.to_string())
                        .unwrap_or_else(|| "0".to_string()),
                ),
            ],
        );

        Ok(result)
    }

    /// Compute swap-v2-base-out (fixed output amount, supports transfer fee)
    pub async fn compute_swap_v2_base_out(&self, params: ComputeSwapV2Request) -> Result<SwapComputeV2Data> {
        LogUtils::log_operation_start("swap-v2-base-out计算", &format!("{} -> {}", params.input_mint, params.output_mint));

        let service_helpers = ServiceHelpers::new(&self.shared.rpc_client);
        let desired_output_amount = service_helpers.parse_amount(&params.amount)?;
        let input_mint_pubkey = Pubkey::from_str(&params.input_mint)?;
        let output_mint_pubkey = Pubkey::from_str(&params.output_mint)?;

        // 计算转账费用
        let transfer_fee_info = if params.enable_transfer_fee.unwrap_or(true) {
            LogUtils::log_operation_start("transfer fee计算", "base-out模式");

            let output_transfer_fee = self.shared.swap_v2_service.get_transfer_fee(&output_mint_pubkey, desired_output_amount)?;
            let input_mint_info = self.shared.swap_v2_service.load_mint_info(&input_mint_pubkey)?;
            let output_mint_info = self.shared.swap_v2_service.load_mint_info(&output_mint_pubkey)?;

            Some(TransferFeeInfo {
                input_transfer_fee: 0, // 输入转账费稍后计算
                output_transfer_fee: output_transfer_fee.transfer_fee,
                input_mint_decimals: input_mint_info.decimals,
                output_mint_decimals: output_mint_info.decimals,
            })
        } else {
            None
        };

        let amount_specified = desired_output_amount;

        // BaseOut计算方法
        let (required_input_amount, other_amount_threshold, pool_address_str) = service_helpers
            .calculate_input_for_output_with_slippage(&params.input_mint, &params.output_mint, amount_specified, params.slippage_bps)
            .await?;

        // 计算输入转账费（在获得所需输入金额后）
        let transfer_fee_info = if let Some(mut fee_info) = transfer_fee_info {
            let input_transfer_fee = self.shared.swap_v2_service.get_transfer_fee(&input_mint_pubkey, required_input_amount)?;
            fee_info.input_transfer_fee = input_transfer_fee.transfer_fee;
            Some(fee_info)
        } else {
            None
        };

        let fee_amount = RoutePlanBuilder::calculate_standard_fee(required_input_amount);
        let route_plan_json = service_helpers
            .create_route_plan(
                pool_address_str.clone(),
                params.input_mint.clone(),
                params.output_mint.clone(),
                fee_amount,
                required_input_amount,
            )
            .await?;
        let route_plan = vec![self.create_route_plan_from_json(route_plan_json)?];

        let epoch = self.shared.swap_v2_service.get_current_epoch()?;

        // 计算真实的价格影响（使用简化方法）
        let price_impact_pct = match service_helpers
            .calculate_price_impact_simple(&params.input_mint, &params.output_mint, required_input_amount, &pool_address_str)
            .await
        {
            Ok(impact) => Some(impact),
            Err(e) => {
                warn!("价格影响计算失败: {:?}，使用默认值", e);
                Some(0.1)
            }
        };

        let result = self.create_swap_compute_v2_data(
            "BaseOut".to_string(),
            params.input_mint,
            required_input_amount.to_string(),
            params.output_mint,
            desired_output_amount,
            other_amount_threshold, // 使用正确计算的阈值
            params.slippage_bps,
            route_plan,
            transfer_fee_info,
            Some(required_input_amount),
            Some(epoch),
            price_impact_pct,
        );

        LogUtils::log_calculation_result(
            "swap-v2-base-out计算",
            required_input_amount,
            desired_output_amount,
            &[(
                "转账费",
                &transfer_fee_info
                    .as_ref()
                    .map(|f| (f.input_transfer_fee, f.output_transfer_fee))
                    .map(|(i, o)| format!("{}, {}", i, o))
                    .unwrap_or_else(|| "0, 0".to_string()),
            )],
        );

        Ok(result)
    }

    /// Build swap-v2-base-in transaction
    pub async fn build_swap_v2_transaction_base_in(&self, request: TransactionSwapV2Request) -> Result<TransactionData> {
        LogUtils::log_operation_start("swap-v2-base-in交易构建", &format!("钱包: {}", request.wallet));

        let service_helpers = ServiceHelpers::new(&self.shared.rpc_client);
        let swap_data = &request.swap_response.data;
        let input_amount = service_helpers.parse_amount(&swap_data.input_amount)?;
        let other_amount_threshold = service_helpers.parse_amount(&swap_data.other_amount_threshold)?;
        let user_wallet = Pubkey::from_str(&request.wallet)?;

        let actual_amount = if let Some(ref amount_specified) = swap_data.amount_specified {
            service_helpers.parse_amount(amount_specified)?
        } else {
            input_amount
        };

        LogUtils::log_debug_info(
            "交易参数",
            &[("输入金额", &actual_amount.to_string()), ("最小输出", &other_amount_threshold.to_string())],
        );

        // 构建SwapV2参数
        let build_params = solana::SwapV2BuildParams {
            input_mint: swap_data.input_mint.clone(),
            output_mint: swap_data.output_mint.clone(),
            user_wallet,
            user_input_token_account: None,  // Will be derived automatically
            user_output_token_account: None, // Will be derived automatically
            amount: actual_amount,
            other_amount_threshold,
            sqrt_price_limit_x64: None,
            is_base_input: true, // This is base-in mode
            slippage_bps: swap_data.slippage_bps,
            compute_unit_limit: None,
        };

        // 构建交易指令
        let instruction_result = self.shared.swap_v2_builder.build_swap_v2_instructions(build_params).await?;

        // 创建交易 - for now, we'll serialize the instructions directly
        // This is a simplified approach - in a real implementation, you'd want to create a proper transaction
        let transaction_data = serde_json::json!({
            "instructions": instruction_result.instructions.len(),
            "compute_units": instruction_result.compute_units_used,
            "expected_fee": instruction_result.expected_fee
        });

        LogUtils::log_operation_success("swap-v2-base-in交易构建", "交易已成功构建");

        Ok(TransactionData {
            transaction: transaction_data.to_string(),
        })
    }

    /// Build swap-v2-base-out transaction
    pub async fn build_swap_v2_transaction_base_out(&self, request: TransactionSwapV2Request) -> Result<TransactionData> {
        info!("🔨 构建swap-v2-base-out交易");
        info!("  钱包地址: {}", request.wallet);

        let service_helpers = ServiceHelpers::new(&self.shared.rpc_client);
        let swap_data = &request.swap_response.data;
        let required_input_amount = service_helpers.parse_amount(&swap_data.input_amount)?;
        let desired_output_amount = service_helpers.parse_amount(&swap_data.output_amount)?;
        let other_amount_threshold = service_helpers.parse_amount(&swap_data.other_amount_threshold)?;
        let user_wallet = Pubkey::from_str(&request.wallet)?;

        info!("  需要输入: {}", required_input_amount);
        info!("  期望输出: {}", desired_output_amount);
        info!("  最大输入阈值: {}", other_amount_threshold);

        // 构建SwapV2参数
        let build_params = solana::SwapV2BuildParams {
            input_mint: swap_data.input_mint.clone(),
            output_mint: swap_data.output_mint.clone(),
            user_wallet,
            user_input_token_account: None,  // Will be derived automatically
            user_output_token_account: None, // Will be derived automatically
            amount: desired_output_amount,
            other_amount_threshold,
            sqrt_price_limit_x64: None,
            is_base_input: false, // This is base-out mode
            slippage_bps: swap_data.slippage_bps,
            compute_unit_limit: None,
        };

        // 构建交易指令
        let instruction_result = self.shared.swap_v2_builder.build_swap_v2_instructions(build_params).await?;

        // 创建交易 - for now, we'll serialize the instructions directly
        // This is a simplified approach - in a real implementation, you'd want to create a proper transaction
        let transaction_data = serde_json::json!({
            "instructions": instruction_result.instructions.len(),
            "compute_units": instruction_result.compute_units_used,
            "expected_fee": instruction_result.expected_fee
        });

        info!("✅ swap-v2-base-out交易构建成功");

        Ok(TransactionData {
            transaction: transaction_data.to_string(),
        })
    }

    // ============ Private Helper Methods ============

    /// Estimate swap output amount
    async fn estimate_swap_output(&self, from_token: &str, to_token: &str, pool_address: &str, amount: u64) -> Result<u64> {
        info!("💱 估算交换输出 - 池子: {}", pool_address);
        info!("  输入: {} ({})", amount, from_token);
        info!("  输出代币: {}", to_token);

        self.shared.ensure_raydium_available().await?;

        // 使用新的直接方法获取池子信息并计算输出
        let estimated_output = {
            let raydium_guard = self.shared.raydium_swap.lock().await;
            let raydium = raydium_guard.as_ref().unwrap();

            match raydium.get_pool_price_and_estimate_direct(pool_address, from_token, to_token, amount).await {
                Ok(output) => {
                    info!("  ✅ 直接从池子状态计算成功，估算输出: {}", output);
                    output
                }
                Err(e) => {
                    warn!("  ⚠️ 直接计算失败: {:?}，使用备用计算", e);

                    // 备用价格计算（简化版本）
                    self.fallback_price_calculation(from_token, to_token, amount).await?
                }
            }
        };

        info!("  📊 最终估算输出: {}", estimated_output);
        Ok(estimated_output)
    }

    /// Fallback price calculation method
    pub async fn fallback_price_calculation(&self, from_token: &str, to_token: &str, amount: u64) -> Result<u64> {
        info!("🔄 使用备用价格计算");

        let from_type = TokenUtils::get_token_type(from_token);
        let to_type = TokenUtils::get_token_type(to_token);

        let estimated_output = match (from_type, to_type) {
            (TokenType::Sol, TokenType::Usdc) => MathUtils::convert_sol_to_usdc(amount),
            (TokenType::Usdc, TokenType::Sol) => MathUtils::convert_usdc_to_sol(amount),
            _ => return Err(anyhow::anyhow!("不支持的交换对: {} -> {}", from_token, to_token)),
        };

        info!("  💰 备用计算结果: {}", estimated_output);
        Ok(estimated_output)
    }

    /// Execute the actual swap operation
    pub async fn execute_swap(&self, request: SwapRequest) -> Result<SwapResponse> {
        info!("🔄 开始执行交换");
        info!("  交换对: {} -> {}", request.from_token, request.to_token);
        info!("  池子地址: {}", request.pool_address);
        info!("  输入金额: {}", request.amount);
        info!("  最小输出: {}", request.minimum_amount_out);
        info!("  最大滑点: {}%", request.max_slippage_percent);

        // 估算输出量
        let estimated_output = self
            .estimate_swap_output(&request.from_token, &request.to_token, &request.pool_address, request.amount)
            .await?;

        // 执行交换
        let signature = {
            self.shared.ensure_raydium_available().await?;
            let raydium_guard = self.shared.raydium_swap.lock().await;
            let raydium = raydium_guard.as_ref().unwrap();

            raydium
                .swap_tokens(
                    &request.from_token,
                    &request.to_token,
                    &request.pool_address,
                    request.amount,
                    request.minimum_amount_out,
                )
                .await?
        };

        info!("✅ 交换成功！交易签名: {}", signature);

        let explorer_url = format!("https://solscan.io/tx/{}", signature);
        let now = chrono::Utc::now().timestamp();

        Ok(SwapResponse {
            signature: signature.clone(),
            from_token: request.from_token.clone(),
            to_token: request.to_token.clone(),
            amount_in: request.amount,
            amount_out_expected: estimated_output,
            amount_out_actual: None, // 需要从链上获取实际输出
            status: TransactionStatus::Finalized,
            explorer_url,
            timestamp: now,
        })
    }

    /// Create SwapComputeV2Data response
    pub fn create_swap_compute_v2_data(
        &self,
        swap_type: String,
        input_mint: String,
        input_amount: String,
        output_mint: String,
        output_amount: u64,
        other_amount_threshold: u64,
        slippage_bps: u16,
        route_plan: Vec<RoutePlan>,
        transfer_fee_info: Option<TransferFeeInfo>,
        amount_specified: Option<u64>,
        epoch: Option<u64>,
        price_impact_pct: Option<f64>,
    ) -> SwapComputeV2Data {
        SwapComputeV2Data {
            swap_type,
            input_mint,
            input_amount,
            output_mint,
            output_amount: output_amount.to_string(),
            other_amount_threshold: other_amount_threshold.to_string(),
            slippage_bps,
            price_impact_pct: price_impact_pct.unwrap_or(0.1),
            referrer_amount: "0".to_string(),
            route_plan,
            transfer_fee_info,
            amount_specified: amount_specified.map(|a| a.to_string()),
            epoch,
        }
    }

    /// Create RoutePlan from JSON value
    pub fn create_route_plan_from_json(&self, json_value: serde_json::Value) -> Result<RoutePlan> {
        Ok(RoutePlan {
            pool_id: json_value["pool_id"].as_str().unwrap_or_default().to_string(),
            input_mint: json_value["input_mint"].as_str().unwrap_or_default().to_string(),
            output_mint: json_value["output_mint"].as_str().unwrap_or_default().to_string(),
            fee_mint: json_value["fee_mint"].as_str().unwrap_or_default().to_string(),
            fee_rate: json_value["fee_rate"].as_u64().unwrap_or(25) as u32,
            fee_amount: json_value["fee_amount"].as_str().unwrap_or_default().to_string(),
            remaining_accounts: json_value["remaining_accounts"]
                .as_array()
                .unwrap_or(&Vec::new())
                .iter()
                .map(|v| v.as_str().unwrap_or_default().to_string())
                .collect(),
            last_pool_price_x64: json_value["last_pool_price_x64"].as_str().unwrap_or_default().to_string(),
        })
    }
}
