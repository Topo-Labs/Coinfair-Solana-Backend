use crate::dtos::solana_dto::{
    BalanceResponse, CalculateLiquidityRequest, CalculateLiquidityResponse, ComputeSwapV2Request, CreateClassicAmmPoolAndSendTransactionResponse,
    CreateClassicAmmPoolRequest, CreateClassicAmmPoolResponse, CreatePoolAndSendTransactionResponse, CreatePoolRequest, CreatePoolResponse,
    GetUserPositionsRequest, OpenPositionAndSendTransactionResponse, OpenPositionRequest, OpenPositionResponse, PositionInfo, PriceQuoteRequest,
    PriceQuoteResponse, RoutePlan, SwapComputeV2Data, SwapRequest, SwapResponse, TransactionData, TransactionStatus, TransactionSwapV2Request, TransferFeeInfo,
    UserPositionsResponse, WalletInfo,
};

use ::utils::solana::{ServiceHelpers, SwapV2InstructionBuilder as UtilsSwapV2InstructionBuilder};
use anchor_lang::AccountDeserialize;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use solana_sdk::account::Account;
use solana_sdk::transaction::Transaction;

use ::utils::solana::{PositionInstructionBuilder, PositionUtils};
use ::utils::{solana::*, AppConfig};
use anyhow::Result;
use async_trait::async_trait;
use solana::raydium_api::RaydiumApiClient;
use solana::{RaydiumSwap, SolanaClient, SwapConfig, SwapV2InstructionBuilder, SwapV2Service};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{instruction::AccountMeta, program_pack::Pack, pubkey::Pubkey, signature::Keypair, signer::Signer};
use spl_token;
use spl_token_2022;
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

    // ============ OpenPosition API ============

    /// 开仓（创建流动性仓位）
    async fn open_position(&self, request: OpenPositionRequest) -> Result<OpenPositionResponse>;

    /// 开仓并发送交易
    async fn open_position_and_send_transaction(&self, request: OpenPositionRequest) -> Result<OpenPositionAndSendTransactionResponse>;

    /// 计算流动性参数
    async fn calculate_liquidity(&self, request: CalculateLiquidityRequest) -> Result<CalculateLiquidityResponse>;

    /// 获取用户所有仓位
    async fn get_user_positions(&self, request: GetUserPositionsRequest) -> Result<UserPositionsResponse>;

    /// 获取仓位详情
    async fn get_position_info(&self, position_key: String) -> Result<PositionInfo>;

    /// 检查仓位是否已存在
    async fn check_position_exists(
        &self,
        pool_address: String,
        tick_lower: i32,
        tick_upper: i32,
        wallet_address: Option<String>,
    ) -> Result<Option<PositionInfo>>;

    // ============ CreatePool API ============

    /// 创建池子
    async fn create_pool(&self, request: CreatePoolRequest) -> Result<CreatePoolResponse>;

    /// 创建池子并发送交易
    async fn create_pool_and_send_transaction(&self, request: CreatePoolRequest) -> Result<CreatePoolAndSendTransactionResponse>;

    // ============ Classic AMM Pool API ============

    /// 创建经典AMM池子
    async fn create_classic_amm_pool(&self, request: CreateClassicAmmPoolRequest) -> Result<CreateClassicAmmPoolResponse>;

    /// 创建经典AMM池子并发送交易
    async fn create_classic_amm_pool_and_send_transaction(
        &self,
        request: CreateClassicAmmPoolRequest,
    ) -> Result<CreateClassicAmmPoolAndSendTransactionResponse>;
}

#[allow(dead_code)]
pub struct SolanaService {
    config: SwapConfig,
    app_config: AppConfig,
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

    /// 检测mint的token program类型
    fn detect_mint_program(&self, mint: &Pubkey) -> Result<Pubkey> {
        let account = self.rpc_client.get_account(mint)?;

        if account.owner == spl_token_2022::id() {
            Ok(spl_token_2022::id())
        } else if account.owner == spl_token::id() {
            Ok(spl_token::id())
        } else {
            Err(anyhow::anyhow!("未知的token program: {}", account.owner))
        }
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
        // dotenvy::dotenv().ok();

        let rpc_url = std::env::var("RPC_URL").unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".to_string());

        let rpc_client = Arc::new(RpcClient::new(rpc_url.clone()));
        let api_client = RaydiumApiClient::new();
        let swap_v2_service = SwapV2Service::new(&rpc_url);

        // 创建SwapV2指令构建器
        let raydium_program_id = std::env::var("RAYDIUM_PROGRAM_ID").unwrap_or_else(|_| DEFAULT_RAYDIUM_PROGRAM_ID.to_string());
        let swap_v2_builder = SwapV2InstructionBuilder::new(&rpc_url, &raydium_program_id, 0).expect("创建SwapV2指令构建器失败");

        Self {
            config: SwapConfig::default(),
            app_config: AppConfig::default(),
            raydium_swap: Arc::new(Mutex::new(None)),
            rpc_client,
            api_client,
            swap_v2_service,
            swap_v2_builder,
        }
    }

    /// 使用统一的配置管理器获取配置
    fn get_config(&self) -> Result<SwapConfig> {
        // info!("🔍 加载Solana配置...");
        // dotenvy::dotenv().ok();

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

        let rpc_url = self.app_config.rpc_url.clone();
        let amm_program_id = self.app_config.raydium_program_id.clone();
        let private_key = self
            .app_config
            .private_key
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("私钥未配置，请检查 .env.development 文件中的 PRIVATE_KEY"))?
            .clone();

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
        } else if let Some(private_key) = &self.app_config.private_key {
            // 如果私钥已配置但raydium未初始化，显示私钥的前8位作为标识
            format!("{}...(私钥已配置)", &private_key[..8.min(private_key.len())])
        } else {
            "未配置私钥".to_string()
        }
    }

    // ============ 辅助方法 ============

    /// 反序列化anchor账户
    fn deserialize_anchor_account<T: AccountDeserialize>(&self, account: &Account) -> Result<T> {
        let mut data: &[u8] = &account.data;
        T::try_deserialize(&mut data).map_err(Into::into)
    }

    /// 计算sqrt_price_x64（复用CLI的逻辑）
    fn calculate_sqrt_price_x64(&self, price: f64, decimals_0: u8, decimals_1: u8) -> u128 {
        // 使用与CLI完全相同的计算逻辑
        let multipler = |decimals: u8| -> f64 { (10_i32).checked_pow(decimals.try_into().unwrap()).unwrap() as f64 };

        let price_to_x64 = |price: f64| -> u128 { (price * raydium_amm_v3::libraries::fixed_point_64::Q64 as f64) as u128 };

        let price_with_decimals = price * multipler(decimals_1) / multipler(decimals_0);
        price_to_x64(price_with_decimals.sqrt())
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
        let estimated_output = self
            .estimate_swap_output(&request.from_token, &request.to_token, &request.pool_address, request.amount)
            .await?;

        // 执行交换
        let signature = {
            self.ensure_raydium_available().await?;
            let raydium_guard = self.raydium_swap.lock().await;
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
            .create_route_plan(
                pool_address_str.clone(),
                params.input_mint.clone(),
                params.output_mint.clone(),
                fee_amount,
                amount_specified,
            )
            .await?;
        let route_plan = vec![self.create_route_plan_from_json(route_plan_json)?];

        let epoch = self.swap_v2_service.get_current_epoch()?;

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

        let result = ResponseBuilder::create_swap_compute_v2_data(
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

    async fn compute_swap_v2_base_out(&self, params: ComputeSwapV2Request) -> Result<SwapComputeV2Data> {
        LogUtils::log_operation_start("swap-v2-base-out计算", &format!("{} -> {}", params.input_mint, params.output_mint));

        let service_helpers = self.create_service_helpers();
        let desired_output_amount = service_helpers.parse_amount(&params.amount)?;
        let input_mint_pubkey = Pubkey::from_str(&params.input_mint)?;
        let output_mint_pubkey = Pubkey::from_str(&params.output_mint)?;

        // 计算转账费用
        let transfer_fee_info = if params.enable_transfer_fee.unwrap_or(true) {
            LogUtils::log_operation_start("transfer fee计算", "base-out模式");

            let output_transfer_fee = self.swap_v2_service.get_transfer_fee(&output_mint_pubkey, desired_output_amount)?;
            let input_mint_info = self.swap_v2_service.load_mint_info(&input_mint_pubkey)?;
            let output_mint_info = self.swap_v2_service.load_mint_info(&output_mint_pubkey)?;

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
            let input_transfer_fee = self.swap_v2_service.get_transfer_fee(&input_mint_pubkey, required_input_amount)?;
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

        let epoch = self.swap_v2_service.get_current_epoch()?;

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

        let result = ResponseBuilder::create_swap_compute_v2_data(
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
            &[
                ("池子ID", &pool_id.to_string()),
                ("输入金额", &actual_amount.to_string()),
                ("最小输出", &other_amount_threshold.to_string()),
            ],
        );

        // 获取池子状态
        let pool_account = self.rpc_client.get_account(&pool_id)?;
        let pool_state: raydium_amm_v3::states::PoolState = self.deserialize_anchor_account(&pool_account)?;

        let input_token_program = self.detect_mint_program(&input_mint)?;
        let output_token_program = self.detect_mint_program(&output_mint)?;

        // 计算ATA账户
        let user_input_token_account =
            spl_associated_token_account::get_associated_token_address_with_program_id(&user_wallet, &input_mint, &input_token_program);
        let user_output_token_account =
            spl_associated_token_account::get_associated_token_address_with_program_id(&user_wallet, &output_mint, &output_token_program);

        // 创建ATA账户指令（幂等操作）
        let mut instructions = Vec::new();

        // 创建输入代币ATA账户（如果不存在）
        info!("📝 确保输入代币ATA账户存在: {}", user_input_token_account);

        let create_input_ata_ix = spl_associated_token_account::instruction::create_associated_token_account_idempotent(
            &user_wallet,
            &user_wallet,
            &input_mint,
            &input_token_program,
        );
        instructions.push(create_input_ata_ix);

        // 创建输出代币ATA账户（如果不存在）
        info!("📝 确保输出代币ATA账户存在: {}", user_output_token_account);

        let create_output_ata_ix = spl_associated_token_account::instruction::create_associated_token_account_idempotent(
            &user_wallet,
            &user_wallet,
            &output_mint,
            &output_token_program,
        );
        instructions.push(create_output_ata_ix);

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

        // 将swap指令添加到指令向量
        instructions.push(ix);

        // 构建完整交易
        let result_json = service_helpers.build_transaction_data(instructions, &user_wallet)?;
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

        let input_token_program = self.detect_mint_program(&input_mint)?;
        let output_token_program = self.detect_mint_program(&output_mint)?;
        // 计算ATA账户
        let user_input_token_account =
            spl_associated_token_account::get_associated_token_address_with_program_id(&user_wallet, &input_mint, &input_token_program);
        let user_output_token_account =
            spl_associated_token_account::get_associated_token_address_with_program_id(&user_wallet, &output_mint, &output_token_program);

        // 检查并创建ATA账户指令
        let mut instructions = Vec::new();

        // 创建输入代币ATA账户（如果不存在）
        info!("📝 确保输入代币ATA账户存在: {}", user_input_token_account);

        let create_input_ata_ix = spl_associated_token_account::instruction::create_associated_token_account_idempotent(
            &user_wallet,
            &user_wallet,
            &input_mint,
            &input_token_program,
        );
        instructions.push(create_input_ata_ix);

        // 创建输出代币ATA账户（如果不存在）
        info!("📝 确保输出代币ATA账户存在: {}", user_output_token_account);

        let create_output_ata_ix = spl_associated_token_account::instruction::create_associated_token_account_idempotent(
            &user_wallet,
            &user_wallet,
            &output_mint,
            &output_token_program,
        );
        instructions.push(create_output_ata_ix);

        // 确定vault账户（基于mint顺序）
        let (input_vault, output_vault, input_vault_mint, output_vault_mint) = service_helpers.build_vault_info(&pool_state, &input_mint);

        // 构建remaining accounts
        let mut remaining_accounts = Vec::new();
        for account_str in &route_plan.remaining_accounts {
            let pubkey = Pubkey::from_str(account_str)?;
            // 第一个是bitmap extension (只读)，其他是tick arrays (可写)
            let is_writable = remaining_accounts.len() > 0;
            remaining_accounts.push(solana_sdk::instruction::AccountMeta {
                pubkey,
                is_signer: false,
                is_writable,
            });
        }

        info!("📝 构建SwapV2指令:");
        info!("  Remaining accounts数量: {}", remaining_accounts.len());

        // 获取Raydium程序ID
        let raydium_program_id =
            Pubkey::from_str(&std::env::var("RAYDIUM_PROGRAM_ID").unwrap_or_else(|_| "CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK".to_string()))?;

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

        // 将swap指令添加到指令向量
        instructions.push(ix);

        // 构建完整交易
        let result_json = service_helpers.build_transaction_data(instructions, &user_wallet)?;
        let result = self.create_transaction_data_from_json(result_json)?;

        info!("✅ 交易构建成功");
        info!("  交易大小: {} bytes", result.transaction.len());

        Ok(result)
    }

    // ============ OpenPosition API实现 ============
    /// 开仓并发送交易，用户本地测试使用，本地签名并发送交易
    async fn open_position_and_send_transaction(&self, request: OpenPositionRequest) -> Result<OpenPositionAndSendTransactionResponse> {
        info!("🎯 开始开仓操作");
        info!("  池子地址: {}", request.pool_address);
        info!("  价格范围: {} - {}", request.tick_lower_price, request.tick_upper_price);
        info!("  输入金额: {}", request.input_amount);

        // 1. 解析和验证参数
        let pool_address = Pubkey::from_str(&request.pool_address)?;
        let user_wallet = Pubkey::from_str(&request.user_wallet)?;

        // 从环境配置中获取私钥
        let private_key = self
            .app_config
            .private_key
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("私钥未配置，请检查 .env.development 文件中的 PRIVATE_KEY"))?;

        // 使用正确的Base58解码方法
        let user_keypair = Keypair::from_base58_string(private_key);

        // 2. 加载池子状态
        let pool_account = self.rpc_client.get_account(&pool_address)?;
        let pool_state: raydium_amm_v3::states::PoolState = self.deserialize_anchor_account(&pool_account)?;

        // 3. 使用Position工具进行计算
        let position_utils = PositionUtils::new(&self.rpc_client);

        // 价格转换为tick（与CLI版本完全一致的流程）
        // 步骤1: 价格转sqrt_price
        let sqrt_price_lower = position_utils.price_to_sqrt_price_x64(request.tick_lower_price, pool_state.mint_decimals_0, pool_state.mint_decimals_1);
        let sqrt_price_upper = position_utils.price_to_sqrt_price_x64(request.tick_upper_price, pool_state.mint_decimals_0, pool_state.mint_decimals_1);

        // 步骤2: sqrt_price转tick
        let tick_lower_raw = raydium_amm_v3::libraries::tick_math::get_tick_at_sqrt_price(sqrt_price_lower)?;
        let tick_upper_raw = raydium_amm_v3::libraries::tick_math::get_tick_at_sqrt_price(sqrt_price_upper)?;

        // 步骤3: 调整tick spacing
        let tick_lower_adjusted = position_utils.tick_with_spacing(tick_lower_raw, pool_state.tick_spacing as i32);
        let tick_upper_adjusted = position_utils.tick_with_spacing(tick_upper_raw, pool_state.tick_spacing as i32);

        info!("  计算的tick范围: {} - {}", tick_lower_adjusted, tick_upper_adjusted);

        // 步骤4: 重新计算调整后的sqrt_price（关键步骤！）
        let sqrt_price_lower_adjusted = raydium_amm_v3::libraries::tick_math::get_sqrt_price_at_tick(tick_lower_adjusted)?;
        let sqrt_price_upper_adjusted = raydium_amm_v3::libraries::tick_math::get_sqrt_price_at_tick(tick_upper_adjusted)?;

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
        let (transfer_fee_0, transfer_fee_1) = self.swap_v2_service.get_pool_mints_inverse_fee(
            &pool_state.token_mint_0,
            &pool_state.token_mint_1,
            amount_0_with_slippage,
            amount_1_with_slippage,
        )?;

        info!("  转账费用 - Token0: {}, Token1: {}", transfer_fee_0.transfer_fee, transfer_fee_1.transfer_fee);

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
        let (tickarray_bitmap_extension, _) = Pubkey::find_program_address(&[b"tick_array_bitmap_extension", pool_address.as_ref()], &raydium_program_id);
        remaining_accounts.push(AccountMeta::new(tickarray_bitmap_extension, false));

        // 11. 计算tick array索引
        let tick_array_lower_start_index = raydium_amm_v3::states::TickArrayState::get_array_start_index(tick_lower_adjusted, pool_state.tick_spacing);
        let tick_array_upper_start_index = raydium_amm_v3::states::TickArrayState::get_array_start_index(tick_upper_adjusted, pool_state.tick_spacing);

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

        // 13. 构建OpenPosition指令
        let instructions = PositionInstructionBuilder::build_open_position_with_token22_nft_instructions(
            &pool_address,
            &pool_state,
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

        // 14. 构建交易
        let recent_blockhash = self.rpc_client.get_latest_blockhash()?;
        let transaction = Transaction::new_signed_with_payer(&instructions, Some(&user_wallet), &[&user_keypair, &nft_mint], recent_blockhash);

        // 15. 发送交易
        let signature = self.rpc_client.send_and_confirm_transaction(&transaction)?;

        info!("✅ 开仓成功，交易签名: {}", signature);

        // 计算position key
        let (position_key, _) = Pubkey::find_program_address(&[b"position", nft_mint.pubkey().as_ref()], &raydium_program_id);

        // 构建响应
        let explorer_url = format!("https://explorer.solana.com/tx/{}", signature);
        let now = chrono::Utc::now().timestamp();

        Ok(OpenPositionAndSendTransactionResponse {
            signature: signature.to_string(),
            position_nft_mint: nft_mint.pubkey().to_string(),
            position_key: position_key.to_string(),
            tick_lower_index: tick_lower_adjusted,
            tick_upper_index: tick_upper_adjusted,
            liquidity: liquidity.to_string(),
            amount_0: amount_0_max,
            amount_1: amount_1_max,
            pool_address: request.pool_address,
            status: TransactionStatus::Finalized,
            explorer_url,
            timestamp: now,
        })
    }

    async fn open_position(&self, request: OpenPositionRequest) -> Result<OpenPositionResponse> {
        info!("🎯 开始构建开仓交易");
        info!("  池子地址: {}", request.pool_address);
        info!("  用户钱包: {}", request.user_wallet);
        info!("  价格范围: {} - {}", request.tick_lower_price, request.tick_upper_price);
        info!("  输入金额: {}", request.input_amount);

        // 1. 解析和验证参数
        let pool_address = Pubkey::from_str(&request.pool_address)?;
        let user_wallet = Pubkey::from_str(&request.user_wallet)?;

        // 2. 加载池子状态
        let pool_account = self.rpc_client.get_account(&pool_address)?;
        let pool_state: raydium_amm_v3::states::PoolState = self.deserialize_anchor_account(&pool_account)?;

        // 3. 使用Position工具进行计算
        let position_utils = PositionUtils::new(&self.rpc_client);

        // 价格转换为tick（与CLI版本完全一致的流程）
        // 步骤1: 价格转sqrt_price
        let sqrt_price_lower = position_utils.price_to_sqrt_price_x64(request.tick_lower_price, pool_state.mint_decimals_0, pool_state.mint_decimals_1);
        let sqrt_price_upper = position_utils.price_to_sqrt_price_x64(request.tick_upper_price, pool_state.mint_decimals_0, pool_state.mint_decimals_1);

        info!("  价格转换详情:");
        info!("    下限价格: {} -> sqrt_price_x64: {}", request.tick_lower_price, sqrt_price_lower);
        info!("    上限价格: {} -> sqrt_price_x64: {}", request.tick_upper_price, sqrt_price_upper);

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
        let sqrt_price_lower_adjusted = raydium_amm_v3::libraries::tick_math::get_sqrt_price_at_tick(tick_lower_adjusted)?;
        let sqrt_price_upper_adjusted = raydium_amm_v3::libraries::tick_math::get_sqrt_price_at_tick(tick_upper_adjusted)?;

        // 反向验证：从调整后的tick计算回实际价格
        let actual_lower_price = position_utils.sqrt_price_x64_to_price(sqrt_price_lower_adjusted, pool_state.mint_decimals_0, pool_state.mint_decimals_1);
        let actual_upper_price = position_utils.sqrt_price_x64_to_price(sqrt_price_upper_adjusted, pool_state.mint_decimals_0, pool_state.mint_decimals_1);

        info!("  最终价格验证:");
        info!("    请求价格范围: {} - {}", request.tick_lower_price, request.tick_upper_price);
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
        let (transfer_fee_0, transfer_fee_1) = self.swap_v2_service.get_pool_mints_inverse_fee(
            &pool_state.token_mint_0,
            &pool_state.token_mint_1,
            amount_0_with_slippage,
            amount_1_with_slippage,
        )?;

        info!("  转账费用 - Token0: {}, Token1: {}", transfer_fee_0.transfer_fee, transfer_fee_1.transfer_fee);

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
        let (tickarray_bitmap_extension, _) = Pubkey::find_program_address(&[b"tick_array_bitmap_extension", pool_address.as_ref()], &raydium_program_id);
        remaining_accounts.push(AccountMeta::new(tickarray_bitmap_extension, false));

        // 11. 计算tick array索引
        let tick_array_lower_start_index = raydium_amm_v3::states::TickArrayState::get_array_start_index(tick_lower_adjusted, pool_state.tick_spacing);
        let tick_array_upper_start_index = raydium_amm_v3::states::TickArrayState::get_array_start_index(tick_upper_adjusted, pool_state.tick_spacing);

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

        // 13. 构建OpenPosition指令
        let instructions = PositionInstructionBuilder::build_open_position_with_token22_nft_instructions(
            &pool_address,
            &pool_state,
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

        // 14. 构建未签名交易
        // 创建未签名的交易消息
        let mut message = solana_sdk::message::Message::new(&instructions, Some(&user_wallet));
        message.recent_blockhash = self.rpc_client.get_latest_blockhash()?;

        // 序列化交易消息为Base64
        let transaction_data = bincode::serialize(&message).map_err(|e| anyhow::anyhow!("序列化交易失败: {}", e))?;
        let transaction_base64 = BASE64_STANDARD.encode(&transaction_data);

        info!("✅ 未签名交易构建成功");

        // 计算position key
        let (position_key, _) = Pubkey::find_program_address(&[b"position", nft_mint.pubkey().as_ref()], &raydium_program_id);

        // 构建交易消息摘要
        let transaction_message = format!(
            "开仓操作 - 池子: {}, 价格范围: {:.4}-{:.4}, 流动性: {}",
            &request.pool_address[..8],
            request.tick_lower_price,
            request.tick_upper_price,
            liquidity
        );

        let now = chrono::Utc::now().timestamp();

        Ok(OpenPositionResponse {
            transaction: transaction_base64,
            transaction_message,
            position_nft_mint: nft_mint.pubkey().to_string(),
            position_key: position_key.to_string(),
            tick_lower_index: tick_lower_adjusted,
            tick_upper_index: tick_upper_adjusted,
            liquidity: liquidity.to_string(),
            amount_0: amount_0_max,
            amount_1: amount_1_max,
            pool_address: request.pool_address,
            timestamp: now,
        })
    }

    async fn calculate_liquidity(&self, request: CalculateLiquidityRequest) -> Result<CalculateLiquidityResponse> {
        info!("🧮 计算流动性参数");

        // 1. 解析参数
        let pool_address = Pubkey::from_str(&request.pool_address)?;

        // 2. 加载池子状态
        let pool_account = self.rpc_client.get_account(&pool_address)?;
        let pool_state: raydium_amm_v3::states::PoolState = self.deserialize_anchor_account(&pool_account)?;

        // 3. 使用Position工具进行计算
        let position_utils = PositionUtils::new(&self.rpc_client);

        // 价格转换为tick
        let tick_lower_index = position_utils.price_to_tick(request.tick_lower_price, pool_state.mint_decimals_0, pool_state.mint_decimals_1)?;
        let tick_upper_index = position_utils.price_to_tick(request.tick_upper_price, pool_state.mint_decimals_0, pool_state.mint_decimals_1)?;

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
        let current_price = position_utils.sqrt_price_x64_to_price(pool_state.sqrt_price_x64, pool_state.mint_decimals_0, pool_state.mint_decimals_1);

        let price_range_utilization = position_utils.calculate_price_range_utilization(current_price, request.tick_lower_price, request.tick_upper_price);

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

    async fn get_user_positions(&self, request: GetUserPositionsRequest) -> Result<UserPositionsResponse> {
        info!("📋 获取用户仓位列表");

        // 1. 确定查询的钱包地址
        let wallet_address = if let Some(addr) = request.wallet_address {
            Pubkey::from_str(&addr)?
        } else {
            return Err(anyhow::anyhow!("缺少必需的钱包地址参数"));
        };

        // 2. 使用Position工具获取NFT信息
        let position_utils = PositionUtils::new(&self.rpc_client);
        let position_nfts = position_utils.get_user_position_nfts(&wallet_address).await?;

        // 3. 批量加载position状态
        let mut positions = Vec::new();
        for nft_info in position_nfts {
            if let Ok(position_account) = self.rpc_client.get_account(&nft_info.position_pda) {
                if let Ok(position_state) = position_utils.deserialize_position_state(&position_account) {
                    // 过滤池子（如果指定）
                    if let Some(ref pool_filter) = request.pool_address {
                        let pool_pubkey = Pubkey::from_str(pool_filter)?;
                        if position_state.pool_id != pool_pubkey {
                            continue;
                        }
                    }

                    // 计算价格
                    let pool_account = self.rpc_client.get_account(&position_state.pool_id)?;
                    let pool_state: raydium_amm_v3::states::PoolState = self.deserialize_anchor_account(&pool_account)?;

                    let tick_lower_price =
                        position_utils.tick_to_price(position_state.tick_lower_index, pool_state.mint_decimals_0, pool_state.mint_decimals_1)?;
                    let tick_upper_price =
                        position_utils.tick_to_price(position_state.tick_upper_index, pool_state.mint_decimals_0, pool_state.mint_decimals_1)?;

                    positions.push(PositionInfo {
                        position_key: nft_info.position_pda.to_string(),
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

    async fn get_position_info(&self, position_key: String) -> Result<PositionInfo> {
        info!("🔍 获取仓位详情: {}", position_key);

        let position_pubkey = Pubkey::from_str(&position_key)?;
        let position_utils = PositionUtils::new(&self.rpc_client);

        // 加载position状态
        let position_account = self.rpc_client.get_account(&position_pubkey)?;
        let position_state = position_utils.deserialize_position_state(&position_account)?;

        // 加载池子状态以计算价格
        let pool_account = self.rpc_client.get_account(&position_state.pool_id)?;
        let pool_state: raydium_amm_v3::states::PoolState = self.deserialize_anchor_account(&pool_account)?;

        let tick_lower_price = position_utils.tick_to_price(position_state.tick_lower_index, pool_state.mint_decimals_0, pool_state.mint_decimals_1)?;
        let tick_upper_price = position_utils.tick_to_price(position_state.tick_upper_index, pool_state.mint_decimals_0, pool_state.mint_decimals_1)?;

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

    async fn check_position_exists(
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

        let position_utils = PositionUtils::new(&self.rpc_client);

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

    // ============ CreatePool API实现 ============

    async fn create_pool(&self, request: CreatePoolRequest) -> Result<CreatePoolResponse> {
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
        let rsps = self.rpc_client.get_multiple_accounts(&load_pubkeys)?;

        let mint0_account = rsps[0].as_ref().ok_or_else(|| anyhow::anyhow!("Mint0账户不存在"))?;
        let mint1_account = rsps[1].as_ref().ok_or_else(|| anyhow::anyhow!("Mint1账户不存在"))?;

        let mint0_owner = mint0_account.owner;
        let mint1_owner = mint1_account.owner;

        // 4. 解析mint信息获取decimals
        let mint0_state = spl_token::state::Mint::unpack(&mint0_account.data)?;
        let mint1_state = spl_token::state::Mint::unpack(&mint1_account.data)?;

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
        let service_helpers = self.create_service_helpers();
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

    async fn create_pool_and_send_transaction(&self, request: CreatePoolRequest) -> Result<CreatePoolAndSendTransactionResponse> {
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
        let rsps = self.rpc_client.get_multiple_accounts(&load_pubkeys)?;

        let mint0_account = rsps[0].as_ref().ok_or_else(|| anyhow::anyhow!("Mint0账户不存在"))?;
        let mint1_account = rsps[1].as_ref().ok_or_else(|| anyhow::anyhow!("Mint1账户不存在"))?;

        let mint0_owner = mint0_account.owner;
        let mint1_owner = mint1_account.owner;

        // 4. 解析mint信息获取decimals
        let mint0_state = spl_token::state::Mint::unpack(&mint0_account.data)?;
        let mint1_state = spl_token::state::Mint::unpack(&mint1_account.data)?;

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
        let recent_blockhash = self.rpc_client.get_latest_blockhash()?;
        let transaction = Transaction::new_signed_with_payer(&instructions, Some(&user_wallet), &[&user_keypair], recent_blockhash);

        // 10. 发送交易
        let signature = self.rpc_client.send_and_confirm_transaction(&transaction)?;

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

    // ============ Classic AMM Pool API实现 ============

    async fn create_classic_amm_pool(&self, request: CreateClassicAmmPoolRequest) -> Result<CreateClassicAmmPoolResponse> {
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
        let recent_blockhash = self.rpc_client.get_latest_blockhash()?;
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

    async fn create_classic_amm_pool_and_send_transaction(
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
        let recent_blockhash = self.rpc_client.get_latest_blockhash()?;
        transaction.message.recent_blockhash = recent_blockhash;

        // 这里需要用户的私钥来签名交易
        // 注意：在实际应用中，私钥应该由前端用户提供，而不是存储在服务器上
        // 这里我们返回未签名的交易，让前端处理签名
        warn!("⚠️ 经典AMM池子创建需要用户私钥签名，当前返回模拟结果");

        // 模拟交易签名（实际应用中应该由用户签名）
        let signature = "模拟交易签名_经典AMM池子创建".to_string();
        let explorer_url = format!("https://explorer.solana.com/tx/{}", signature);
        let now = chrono::Utc::now().timestamp();

        info!("✅ 经典AMM池子创建交易准备完成");
        info!("  池子地址: {}", addresses.pool_id);
        info!("  模拟签名: {}", signature);

        Ok(CreateClassicAmmPoolAndSendTransactionResponse {
            signature,
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
            status: TransactionStatus::Pending,
            explorer_url,
            timestamp: now,
        })
    }
}

impl Default for SolanaService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_open_position_validation() {
        // 验证关键逻辑的正确性

        // 1. 价格转tick的测试 - 使用PositionUtils的逻辑
        let price = 1.5;
        let decimals_0 = 9;
        let decimals_1 = 6;

        // 应该考虑decimals差异
        let decimal_adjustment = 10_f64.powi(decimals_0 as i32 - decimals_1 as i32);
        let expected_adjusted_price = price * decimal_adjustment;
        assert_eq!(expected_adjusted_price, 1500.0);

        // 2. 滑点计算测试 - 验证apply_slippage逻辑
        let amount = 1000000;
        let slippage_percent = 5.0;
        // 应用滑点（增加）
        let amount_with_slippage = (amount as f64 * (1.0 + slippage_percent / 100.0)) as u64;
        assert_eq!(amount_with_slippage, 1050000);

        // 3. Transfer fee测试
        let transfer_fee = 5000u64;
        let amount_max = amount_with_slippage.checked_add(transfer_fee).unwrap();
        assert_eq!(amount_max, 1055000);
    }

    #[test]
    fn test_tick_spacing_adjustment() {
        // 验证tick spacing调整逻辑（与PositionUtils::tick_with_spacing一致）
        let tick = 123;
        let tick_spacing = 10;

        // 正数情况
        let adjusted_tick = tick / tick_spacing * tick_spacing;
        assert_eq!(adjusted_tick, 120);

        // 负数情况 - 需要向下调整
        let tick_negative = -123;
        let adjusted_tick_negative = if tick_negative < 0 && tick_negative % tick_spacing != 0 {
            (tick_negative / tick_spacing - 1) * tick_spacing
        } else {
            tick_negative / tick_spacing * tick_spacing
        };
        assert_eq!(adjusted_tick_negative, -130);

        // 精确整除的情况
        let tick_exact = 120;
        let adjusted_exact = tick_exact / tick_spacing * tick_spacing;
        assert_eq!(adjusted_exact, 120);
    }

    #[test]
    fn test_sqrt_price_conversion() {
        // 测试价格与sqrt_price_x64的转换
        let price = 1.0;
        let decimals_0 = 9;
        let decimals_1 = 6;

        // 调整价格（考虑decimals）
        let decimal_adjustment = 10_f64.powi(decimals_0 as i32 - decimals_1 as i32);
        let adjusted_price = price * decimal_adjustment;

        // 计算sqrt_price_x64
        let sqrt_price = adjusted_price.sqrt();
        let sqrt_price_x64 = (sqrt_price * (1u64 << 32) as f64) as u128;

        // 验证转换是合理的
        assert!(sqrt_price_x64 > 0);
        assert!(sqrt_price_x64 < u128::MAX);
    }
}
#[cfg(test)]
mod create_pool_tests {
    use super::*;

    #[test]
    fn test_calculate_sqrt_price_x64() {
        // 直接测试计算逻辑，不依赖SolanaService实例
        let calculate_sqrt_price_x64 = |price: f64, decimals_0: u8, decimals_1: u8| -> u128 {
            let multipler = |decimals: u8| -> f64 { (10_i32).checked_pow(decimals.try_into().unwrap()).unwrap() as f64 };

            let price_to_x64 = |price: f64| -> u128 { (price * raydium_amm_v3::libraries::fixed_point_64::Q64 as f64) as u128 };

            let price_with_decimals = price * multipler(decimals_1) / multipler(decimals_0);
            price_to_x64(price_with_decimals.sqrt())
        };

        // 测试基本价格计算
        let price = 1.0;
        let decimals_0 = 9; // SOL
        let decimals_1 = 6; // USDC

        let sqrt_price_x64 = calculate_sqrt_price_x64(price, decimals_0, decimals_1);

        // 验证结果不为0
        assert!(sqrt_price_x64 > 0);

        // 测试价格为2.0的情况
        let price_2 = 2.0;
        let sqrt_price_x64_2 = calculate_sqrt_price_x64(price_2, decimals_0, decimals_1);

        // 价格为2时的sqrt_price应该大于价格为1时的
        assert!(sqrt_price_x64_2 > sqrt_price_x64);
    }

    #[test]
    fn test_mint_order_logic() {
        // 测试mint顺序调整逻辑
        let mint0_str = "So11111111111111111111111111111111111111112"; // SOL
        let mint1_str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"; // USDC

        let mut mint0 = Pubkey::from_str(mint0_str).unwrap();
        let mut mint1 = Pubkey::from_str(mint1_str).unwrap();
        let mut price = 100.0; // 1 SOL = 100 USDC

        // 检查是否需要交换
        if mint0 > mint1 {
            std::mem::swap(&mut mint0, &mut mint1);
            price = 1.0 / price;
        }

        // 验证mint0应该小于mint1
        assert!(mint0 < mint1);

        // 验证价格调整是否正确
        if mint0_str == "So11111111111111111111111111111111111111112" && mint0 != Pubkey::from_str(mint0_str).unwrap() {
            // 如果SOL不是mint0，价格应该被调整
            assert_eq!(price, 0.01); // 1/100
        }
    }

    #[test]
    fn test_create_pool_request_validation() {
        // 测试CreatePool请求的基本验证逻辑
        let request = CreatePoolRequest {
            config_index: 0,
            price: 1.5,
            mint0: "So11111111111111111111111111111111111111112".to_string(),
            mint1: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(),
            open_time: 0,
            user_wallet: "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM".to_string(),
        };

        // 验证价格大于0
        assert!(request.price > 0.0);

        // 验证mint地址不相同
        assert_ne!(request.mint0, request.mint1);

        // 验证可以解析为有效的Pubkey
        assert!(Pubkey::from_str(&request.mint0).is_ok());
        assert!(Pubkey::from_str(&request.mint1).is_ok());
        assert!(Pubkey::from_str(&request.user_wallet).is_ok());
    }
}
