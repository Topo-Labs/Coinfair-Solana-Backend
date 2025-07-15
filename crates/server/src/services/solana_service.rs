use crate::dtos::solana_dto::{
    BalanceResponse, ComputeSwapV2Request, PriceQuoteRequest, PriceQuoteResponse, RoutePlan, SwapComputeV2Data, SwapRequest, SwapResponse, TransactionData, TransactionStatus, TransactionSwapV2Request, TransferFeeInfo,
    WalletInfo,
};

use ::utils::solana::{ServiceHelpers, SwapV2InstructionBuilder as UtilsSwapV2InstructionBuilder};

use ::utils::solana::*;
use anyhow::Result;
use async_trait::async_trait;
use solana::raydium_api::RaydiumApiClient;
use solana::{RaydiumSwap, SolanaClient, SwapConfig, SwapV2InstructionBuilder, SwapV2Service};
use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info, warn};

pub type DynSolanaService = Arc<dyn SolanaServiceTrait + Send + Sync>;

/// SwapV2账户信息辅助结构体
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct SwapV2AccountsInfo {
    epoch: u64,
    pool_address: String,
    input_mint_decimals: u8,
    output_mint_decimals: u8,
}

/// 临时池子配置结构体（简化版本）
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct TemporaryPoolConfig {
    pool_id_account: Option<Pubkey>,
    raydium_v3_program: Pubkey,
    #[allow(dead_code)]
    mint0: Option<Pubkey>,
    #[allow(dead_code)]
    mint1: Option<Pubkey>,
}

#[async_trait]
pub trait SolanaServiceTrait {
    /// 执行代币交换
    async fn swap_tokens(&self, request: SwapRequest) -> Result<SwapResponse>;

    /// 获取账户余额
    async fn get_balance(&self) -> Result<BalanceResponse>;

    /// 获取价格报价
    async fn get_price_quote(&self, request: PriceQuoteRequest) -> Result<PriceQuoteResponse>;

    /// 获取钱包信息
    async fn get_wallet_info(&self) -> Result<WalletInfo>;

    /// 检查服务状态
    async fn health_check(&self) -> Result<String>;

    // ============ SwapV2 API兼容接口 ============

    /// 计算swap-v2-base-in（固定输入金额，支持转账费）
    async fn compute_swap_v2_base_in(&self, params: ComputeSwapV2Request) -> Result<SwapComputeV2Data>;

    /// 计算swap-v2-base-out（固定输出金额，支持转账费）
    async fn compute_swap_v2_base_out(&self, params: ComputeSwapV2Request) -> Result<SwapComputeV2Data>;

    /// 构建swap-v2-base-in交易
    async fn build_swap_v2_transaction_base_in(&self, request: TransactionSwapV2Request) -> Result<TransactionData>;

    /// 构建swap-v2-base-out交易
    async fn build_swap_v2_transaction_base_out(&self, request: TransactionSwapV2Request) -> Result<TransactionData>;
}

pub struct SolanaService {
    config: SwapConfig,
    raydium_swap: Arc<Mutex<Option<RaydiumSwap>>>,
    rpc_client: Arc<RpcClient>,                // 只读RPC客户端
    api_client: RaydiumApiClient,              // 只读API客户端
    swap_v2_service: SwapV2Service,            // SwapV2专用服务
    swap_v2_builder: SwapV2InstructionBuilder, // SwapV2指令构建器
}

/// 响应数据构建器 - 统一管理响应数据创建
struct ResponseBuilder;

impl ResponseBuilder {
    /// 创建SwapComputeV2Data响应
    fn create_swap_compute_v2_data(
        swap_type: String,
        input_mint: String,
        input_amount: String,
        output_mint: String,
        output_amount: u64,
        other_amount_threshold: u64,
        slippage_bps: u16,
        route_plan: Vec<crate::dtos::solana_dto::RoutePlan>,
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
}

impl SolanaService {
    /// 创建服务助手
    fn create_service_helpers(&self) -> ServiceHelpers {
        ServiceHelpers::new(&self.rpc_client)
    }

    /// 从 serde_json::Value 创建 RoutePlan
    fn create_route_plan_from_json(&self, json_value: serde_json::Value) -> Result<RoutePlan> {
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

    /// 从 serde_json::Value 创建 TransactionData
    fn create_transaction_data_from_json(&self, json_value: serde_json::Value) -> Result<TransactionData> {
        Ok(TransactionData {
            transaction: json_value["transaction"].as_str().unwrap_or_default().to_string(),
        })
    }

    pub fn new() -> Self {
        // 确保加载环境变量
        dotenvy::dotenv().ok();

        let rpc_url = std::env::var("RPC_URL").unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".to_string());

        let rpc_client = Arc::new(RpcClient::new(rpc_url.clone()));
        let api_client = RaydiumApiClient::new();
        let swap_v2_service = SwapV2Service::new(&rpc_url);

        // 创建SwapV2指令构建器
        let raydium_program_id = std::env::var("RAYDIUM_PROGRAM_ID").unwrap_or_else(|_| DEFAULT_RAYDIUM_PROGRAM_ID.to_string());
        let swap_v2_builder = SwapV2InstructionBuilder::new(&rpc_url, &raydium_program_id, 0).expect("创建SwapV2指令构建器失败");

        Self {
            config: SwapConfig::default(),
            raydium_swap: Arc::new(Mutex::new(None)),
            rpc_client,
            api_client,
            swap_v2_service,
            swap_v2_builder,
        }
    }

    /// 使用统一的配置管理器获取配置
    fn get_config(&self) -> Result<SwapConfig> {
        info!("🔍 加载Solana配置...");
        dotenvy::dotenv().ok();

        let rpc_url = std::env::var("RPC_URL").unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".to_string());
        let amm_program_id = std::env::var("RAYDIUM_PROGRAM_ID").unwrap_or_else(|_| DEFAULT_RAYDIUM_PROGRAM_ID.to_string());

        let config = SwapConfig {
            rpc_url: rpc_url.clone(),
            private_key: "".to_string(),
            amm_program_id: amm_program_id.clone(),
            openbook_program_id: "".to_string(),
            usdc_mint: USDC_MINT_STANDARD.to_string(),
            sol_usdc_pool_id: "".to_string(),
        };

        info!("✅ Solana配置加载成功（只读模式）");
        info!("  RPC URL: {}", config.rpc_url);
        info!("  Raydium程序ID: {}", config.amm_program_id);
        Ok(config)
    }

    /// 使用统一的配置管理器获取完整配置
    fn _get_config_with_private_key(&self) -> Result<SwapConfig> {
        info!("🔍 加载完整Solana配置（包含私钥）...");
        dotenvy::dotenv().ok();

        let rpc_url = std::env::var("RPC_URL").unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".to_string());
        let amm_program_id = std::env::var("RAYDIUM_PROGRAM_ID").unwrap_or_else(|_| DEFAULT_RAYDIUM_PROGRAM_ID.to_string());
        let private_key = std::env::var("PRIVATE_KEY").map_err(|_| anyhow::anyhow!("环境变量PRIVATE_KEY未设置"))?;

        let config = SwapConfig {
            rpc_url: rpc_url.clone(),
            private_key,
            amm_program_id: amm_program_id.clone(),
            openbook_program_id: "".to_string(),
            usdc_mint: USDC_MINT_STANDARD.to_string(),
            sol_usdc_pool_id: "".to_string(),
        };

        info!("✅ 完整Solana配置加载成功");
        info!("  RPC URL: {}", config.rpc_url);
        info!("  Raydium程序ID: {}", config.amm_program_id);
        Ok(config)
    }

    async fn initialize_raydium(&self) -> Result<()> {
        let mut raydium_guard = self.raydium_swap.lock().await;
        if raydium_guard.is_none() {
            info!("正在初始化Raydium交换服务...");

            // 确保配置可用
            let config = self.get_config()?;

            // 创建SolanaClient
            let client = SolanaClient::new(&config)?;

            // 创建RaydiumSwap实例
            match RaydiumSwap::new(client, &config) {
                Ok(raydium_swap) => {
                    *raydium_guard = Some(raydium_swap);
                    info!("✅ Raydium交换服务初始化成功");
                }
                Err(e) => {
                    error!("❌ Raydium交换服务初始化失败: {:?}", e);
                    return Err(anyhow::anyhow!("Raydium交换服务初始化失败: {}", e));
                }
            }
        }
        Ok(())
    }

    async fn ensure_raydium_available(&self) -> Result<()> {
        self.initialize_raydium().await?;
        let raydium_guard = self.raydium_swap.lock().await;
        if raydium_guard.is_none() {
            Err(anyhow::anyhow!("Raydium交换服务未初始化"))
        } else {
            Ok(())
        }
    }

    async fn estimate_swap_output(&self, from_token: &str, to_token: &str, pool_address: &str, amount: u64) -> Result<u64> {
        info!("💱 估算交换输出 - 池子: {}", pool_address);
        info!("  输入: {} ({})", amount, from_token);
        info!("  输出代币: {}", to_token);

        self.ensure_raydium_available().await?;

        // 使用新的直接方法获取池子信息并计算输出
        let estimated_output = {
            let raydium_guard = self.raydium_swap.lock().await;
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

    /// 使用统一的备用价格计算方法
    async fn fallback_price_calculation(&self, from_token: &str, to_token: &str, amount: u64) -> Result<u64> {
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

    async fn get_wallet_address_from_private_key(&self) -> String {
        if let Some(raydium) = self.raydium_swap.lock().await.as_ref() {
            // 通过RaydiumSwap获取钱包地址
            match raydium.get_wallet_pubkey() {
                Ok(pubkey) => pubkey.to_string(),
                Err(_) => "无法获取钱包地址".to_string(),
            }
        } else if !self.config.private_key.is_empty() {
            // 如果私钥已配置但raydium未初始化，显示私钥的前8位作为标识
            format!("{}...(私钥已配置)", &self.config.private_key[..8])
        } else {
            "未配置私钥".to_string()
        }
    }

    /// 反序列化anchor账户（复制CLI逻辑）
    fn deserialize_anchor_account<T: anchor_lang::AccountDeserialize>(&self, account: &solana_sdk::account::Account) -> Result<T> {
        let mut data: &[u8] = &account.data;
        T::try_deserialize(&mut data).map_err(Into::into)
    }

    /// 执行交换
    pub async fn execute_swap(&self, request: SwapRequest) -> Result<SwapResponse> {
        info!("🔄 开始执行交换");
        info!("  交换对: {} -> {}", request.from_token, request.to_token);
        info!("  池子地址: {}", request.pool_address);
        info!("  输入金额: {}", request.amount);
        info!("  最小输出: {}", request.minimum_amount_out);
        info!("  最大滑点: {}%", request.max_slippage_percent);

        // 估算输出量
        let estimated_output = self.estimate_swap_output(&request.from_token, &request.to_token, &request.pool_address, request.amount).await?;

        // 执行交换
        let signature = {
            self.ensure_raydium_available().await?;
            let raydium_guard = self.raydium_swap.lock().await;
            let raydium = raydium_guard.as_ref().unwrap();

            raydium
                .swap_tokens(&request.from_token, &request.to_token, &request.pool_address, request.amount, request.minimum_amount_out)
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
            status: TransactionStatus::Pending,
            explorer_url,
            timestamp: now,
        })
    }
}

#[async_trait]
impl SolanaServiceTrait for SolanaService {
    async fn swap_tokens(&self, request: SwapRequest) -> Result<SwapResponse> {
        // 执行交换
        self.execute_swap(request).await
    }

    async fn get_balance(&self) -> Result<BalanceResponse> {
        info!("💰 获取钱包余额");

        self.ensure_raydium_available().await?;

        let (sol_lamports, usdc_micro) = {
            let raydium_guard = self.raydium_swap.lock().await;
            let raydium = raydium_guard.as_ref().unwrap();
            raydium.get_account_balances().await?
        };

        // 获取钱包地址
        let wallet_address = self.get_wallet_address_from_private_key().await;

        let now = chrono::Utc::now().timestamp();

        Ok(BalanceResponse {
            sol_balance_lamports: sol_lamports,
            sol_balance: sol_lamports as f64 / 1_000_000_000.0,
            usdc_balance_micro: usdc_micro,
            usdc_balance: usdc_micro as f64 / 1_000_000.0,
            wallet_address,
            timestamp: now,
        })
    }

    async fn get_price_quote(&self, request: PriceQuoteRequest) -> Result<PriceQuoteResponse> {
        info!("📊 获取价格报价");
        info!("  交换对: {} -> {}", request.from_token, request.to_token);
        info!("  池子地址: {}", request.pool_address);
        info!("  金额: {}", request.amount);

        let estimated_output = self.estimate_swap_output(&request.from_token, &request.to_token, &request.pool_address, request.amount).await?;

        // 计算价格
        let price = if request.amount > 0 { estimated_output as f64 / request.amount as f64 } else { 0.0 };

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

    async fn get_wallet_info(&self) -> Result<WalletInfo> {
        let wallet_info = WalletInfo {
            address: self.get_wallet_address_from_private_key().await,
            network: self.config.rpc_url.clone(),
            connected: self.raydium_swap.lock().await.is_some(),
        };

        Ok(wallet_info)
    }

    async fn health_check(&self) -> Result<String> {
        if self.raydium_swap.lock().await.is_some() {
            Ok("Solana服务运行正常".to_string())
        } else {
            Ok("Solana服务未初始化（私钥未配置）".to_string())
        }
    }
    // ============ SwapV2 API兼容接口实现 ============

    async fn compute_swap_v2_base_in(&self, params: ComputeSwapV2Request) -> Result<SwapComputeV2Data> {
        LogUtils::log_operation_start("swap-v2-base-in计算", &format!("{} -> {}", params.input_mint, params.output_mint));

        let service_helpers = self.create_service_helpers();
        let input_amount = service_helpers.parse_amount(&params.amount)?;
        let input_mint_pubkey = Pubkey::from_str(&params.input_mint)?;
        let output_mint_pubkey = Pubkey::from_str(&params.output_mint)?;

        // 计算转账费用
        let transfer_fee_info = if params.enable_transfer_fee.unwrap_or(false) {
            LogUtils::log_operation_start("transfer fee计算", "base-in模式");

            let input_transfer_fee = self.swap_v2_service.get_transfer_fee(&input_mint_pubkey, input_amount)?;
            let input_mint_info = self.swap_v2_service.load_mint_info(&input_mint_pubkey)?;
            let output_mint_info = self.swap_v2_service.load_mint_info(&output_mint_pubkey)?;

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
            .create_route_plan(pool_address_str.clone(), params.input_mint.clone(), params.output_mint.clone(), fee_amount, amount_specified)
            .await?;
        let route_plan = vec![self.create_route_plan_from_json(route_plan_json)?];

        let epoch = self.swap_v2_service.get_current_epoch()?;

        // 计算真实的价格影响
        let price_impact_pct = match service_helpers
            .calculate_price_impact(&params.input_mint, &params.output_mint, amount_specified, output_amount, &pool_address_str)
            .await
        {
            Ok(impact) => Some(impact),
            Err(e) => {
                warn!("价格影响计算失败: {:?}，使用默认值", e);
                Some(0.1)
            }
        };

        let result = ResponseBuilder::create_swap_compute_v2_data(
            "BaseInV2".to_string(),
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
                ("转账费", &transfer_fee_info.as_ref().map(|f| f.input_transfer_fee.to_string()).unwrap_or_else(|| "0".to_string())),
            ],
        );

        Ok(result)
    }

    async fn compute_swap_v2_base_out(&self, params: ComputeSwapV2Request) -> Result<SwapComputeV2Data> {
        LogUtils::log_operation_start("swap-v2-base-out计算", &format!("{} -> {}", params.input_mint, params.output_mint));

        let service_helpers = self.create_service_helpers();
        let output_amount = service_helpers.parse_amount(&params.amount)?;
        let input_mint_pubkey = Pubkey::from_str(&params.output_mint)?;
        let output_mint_pubkey = Pubkey::from_str(&params.input_mint)?;

        // 计算转账费用
        let transfer_fee_info = if params.enable_transfer_fee.unwrap_or(true) {
            LogUtils::log_operation_start("transfer fee计算", "base-out模式");

            let input_transfer_fee = self.swap_v2_service.get_transfer_inverse_fee(&input_mint_pubkey, output_amount)?;
            let output_transfer_fee = self.swap_v2_service.get_transfer_fee(&output_mint_pubkey, output_amount)?;

            let input_mint_info = self.swap_v2_service.load_mint_info(&input_mint_pubkey)?;
            let output_mint_info = self.swap_v2_service.load_mint_info(&output_mint_pubkey)?;

            Some(TransferFeeInfo {
                input_transfer_fee: input_transfer_fee.transfer_fee,
                output_transfer_fee: output_transfer_fee.transfer_fee,
                input_mint_decimals: input_mint_info.decimals,
                output_mint_decimals: output_mint_info.decimals,
            })
        } else {
            None
        };

        let amount_specified = if let Some(ref fee_info) = transfer_fee_info {
            output_amount.checked_sub(fee_info.input_transfer_fee).unwrap_or(output_amount)
        } else {
            output_amount
        };

        // 使用新的计算方法，包含滑点保护
        let (input_amount, other_amount_threshold, pool_address_str) = service_helpers
            .calculate_output_for_input_with_slippage(&params.input_mint, &params.output_mint, amount_specified, params.slippage_bps)
            .await?;

        let fee_amount = RoutePlanBuilder::calculate_standard_fee(output_amount);
        let route_plan_json = service_helpers
            .create_route_plan(pool_address_str.clone(), params.input_mint.clone(), params.output_mint.clone(), fee_amount, output_amount)
            .await?;
        let route_plan = vec![self.create_route_plan_from_json(route_plan_json)?];

        let epoch = self.swap_v2_service.get_current_epoch()?;

        // 计算真实的价格影响
        let price_impact_pct = match service_helpers
            .calculate_price_impact(&params.input_mint, &params.output_mint, input_amount, output_amount, &pool_address_str)
            .await
        {
            Ok(impact) => Some(impact),
            Err(e) => {
                warn!("价格影响计算失败: {:?}，使用默认值", e);
                Some(0.1)
            }
        };

        let result = ResponseBuilder::create_swap_compute_v2_data(
            "BaseOutV2".to_string(),
            params.input_mint,
            input_amount.to_string(),
            params.output_mint,
            output_amount,
            other_amount_threshold, // 使用正确计算的阈值
            params.slippage_bps,
            route_plan,
            transfer_fee_info,
            Some(input_amount),
            Some(epoch),
            price_impact_pct,
        );

        LogUtils::log_calculation_result(
            "swap-v2-base-out计算",
            input_amount,
            output_amount,
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

    async fn build_swap_v2_transaction_base_in(&self, request: TransactionSwapV2Request) -> Result<TransactionData> {
        LogUtils::log_operation_start("swap-v2-base-in交易构建", &format!("钱包: {}", request.wallet));

        let service_helpers = self.create_service_helpers();
        let swap_data = &request.swap_response.data;
        let input_amount = service_helpers.parse_amount(&swap_data.input_amount)?;
        let other_amount_threshold = service_helpers.parse_amount(&swap_data.other_amount_threshold)?;
        let user_wallet = Pubkey::from_str(&request.wallet)?;

        let actual_amount = if let Some(ref amount_specified) = swap_data.amount_specified {
            service_helpers.parse_amount(amount_specified)?
        } else {
            input_amount
        };

        let route_plan = swap_data.route_plan.first().ok_or_else(|| ErrorHandler::create_error("未找到路由计划"))?;

        let pool_id = Pubkey::from_str(&route_plan.pool_id)?;
        let input_mint = Pubkey::from_str(&swap_data.input_mint)?;
        let output_mint = Pubkey::from_str(&swap_data.output_mint)?;

        LogUtils::log_debug_info(
            "交易参数",
            &[("池子ID", &pool_id.to_string()), ("输入金额", &actual_amount.to_string()), ("最小输出", &other_amount_threshold.to_string())],
        );

        // 获取池子状态
        let pool_account = self.rpc_client.get_account(&pool_id)?;
        let pool_state: raydium_amm_v3::states::PoolState = self.deserialize_anchor_account(&pool_account)?;

        // 计算ATA账户
        let user_input_token_account = spl_associated_token_account::get_associated_token_address(&user_wallet, &input_mint);
        let user_output_token_account = spl_associated_token_account::get_associated_token_address(&user_wallet, &output_mint);

        // 确定vault账户
        let (input_vault, output_vault, input_vault_mint, output_vault_mint) = service_helpers.build_vault_info(&pool_state, &input_mint);

        // 构建remaining accounts
        let remaining_accounts = AccountMetaBuilder::create_remaining_accounts(&route_plan.remaining_accounts, true)?;

        let raydium_program_id = ConfigManager::get_raydium_program_id()?;

        // 构建SwapV2指令
        let ix = UtilsSwapV2InstructionBuilder::build_swap_v2_instruction(
            &raydium_program_id,
            &pool_state.amm_config,
            &pool_id,
            &user_wallet,
            &user_input_token_account,
            &user_output_token_account,
            &input_vault,
            &output_vault,
            &input_vault_mint,
            &output_vault_mint,
            &pool_state.observation_key,
            remaining_accounts,
            actual_amount,
            other_amount_threshold,
            None,
            true,
        )?;

        // 构建完整交易
        let result_json = service_helpers.build_transaction_data(vec![ix], &user_wallet)?;
        let result = self.create_transaction_data_from_json(result_json)?;

        LogUtils::log_operation_success("swap-v2-base-in交易构建", &format!("交易大小: {} bytes", result.transaction.len()));

        Ok(result)
    }

    async fn build_swap_v2_transaction_base_out(&self, request: TransactionSwapV2Request) -> Result<TransactionData> {
        info!("🔨 构建swap-v2-base-out交易");
        info!("  钱包地址: {}", request.wallet);
        info!("  交易版本: {}", request.tx_version);

        let service_helpers = self.create_service_helpers();
        // 从swap_response中提取交换数据
        let swap_data = &request.swap_response.data;
        let output_amount = service_helpers.parse_amount(&swap_data.output_amount)?;
        let other_amount_threshold = service_helpers.parse_amount(&swap_data.other_amount_threshold)?;
        let user_wallet = Pubkey::from_str(&request.wallet)?;

        // 对于base-out，amount_specified通常是期望的输出金额
        let actual_output_amount = if let Some(ref amount_specified) = swap_data.amount_specified {
            service_helpers.parse_amount(amount_specified)?
        } else {
            output_amount
        };

        // 从route_plan中获取池子信息和remaining accounts
        let route_plan = swap_data.route_plan.first().ok_or_else(|| anyhow::anyhow!("No route plan found"))?;

        let pool_id = Pubkey::from_str(&route_plan.pool_id)?;
        let input_mint = Pubkey::from_str(&swap_data.input_mint)?;
        let output_mint = Pubkey::from_str(&swap_data.output_mint)?;

        info!("📋 构建交易参数:");
        info!("  池子ID: {}", pool_id);
        info!("  期望输出金额: {}", actual_output_amount);
        info!("  最大输入: {}", other_amount_threshold);
        info!("  输入代币: {}", input_mint);
        info!("  输出代币: {}", output_mint);

        // 获取池子状态以获取必要的账户信息
        let pool_account = self.rpc_client.get_account(&pool_id)?;
        let pool_state: raydium_amm_v3::states::PoolState = self.deserialize_anchor_account(&pool_account)?;

        // 计算ATA账户
        let user_input_token_account = spl_associated_token_account::get_associated_token_address(&user_wallet, &input_mint);
        let user_output_token_account = spl_associated_token_account::get_associated_token_address(&user_wallet, &output_mint);

        // 确定vault账户（基于mint顺序）
        let (input_vault, output_vault, input_vault_mint, output_vault_mint) = service_helpers.build_vault_info(&pool_state, &input_mint);

        // 构建remaining accounts
        let mut remaining_accounts = Vec::new();
        for account_str in &route_plan.remaining_accounts {
            let pubkey = Pubkey::from_str(account_str)?;
            // 第一个是bitmap extension (只读)，其他是tick arrays (可写)
            let is_writable = remaining_accounts.len() > 0;
            remaining_accounts.push(solana_sdk::instruction::AccountMeta { pubkey, is_signer: false, is_writable });
        }

        info!("📝 构建SwapV2指令:");
        info!("  Remaining accounts数量: {}", remaining_accounts.len());

        // 获取Raydium程序ID
        let raydium_program_id = Pubkey::from_str(&std::env::var("RAYDIUM_PROGRAM_ID").unwrap_or_else(|_| "CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK".to_string()))?;

        // 构建SwapV2指令
        let ix = UtilsSwapV2InstructionBuilder::build_swap_v2_instruction(
            &raydium_program_id,
            &pool_state.amm_config,
            &pool_id,
            &user_wallet,
            &user_input_token_account,
            &user_output_token_account,
            &input_vault,
            &output_vault,
            &input_vault_mint,
            &output_vault_mint,
            &pool_state.observation_key,
            remaining_accounts,
            actual_output_amount,   // 对于base-out，这是期望的输出金额
            other_amount_threshold, // 这是最大允许的输入金额
            None,                   // sqrt_price_limit_x64
            false,                  // is_base_input = false for base-out
        )?;

        // 构建完整交易
        let result_json = service_helpers.build_transaction_data(vec![ix], &user_wallet)?;
        let result = self.create_transaction_data_from_json(result_json)?;

        info!("✅ 交易构建成功");
        info!("  交易大小: {} bytes", result.transaction.len());

        Ok(result)
    }
}

impl Default for SolanaService {
    fn default() -> Self {
        Self::new()
    }
}
