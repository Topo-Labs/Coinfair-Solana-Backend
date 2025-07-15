use crate::dtos::solana_dto::{
    BalanceResponse, ComputeSwapV2Request, PriceQuoteRequest, PriceQuoteResponse, RoutePlan, SwapComputeV2Data, SwapRequest, SwapResponse, TransactionData, TransactionStatus, TransactionSwapV2Request, TransferFeeInfo, WalletInfo,
};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use borsh::BorshSerialize;

/// 交换状态结构体（与CLI utils.rs中的SwapState完全一致）
#[derive(Debug)]
struct SwapState {
    /// 剩余需要交换的输入/输出资产数量
    amount_specified_remaining: u64,
    /// 已经交换出的输出/输入资产数量
    amount_calculated: u64,
    /// 当前价格的平方根
    sqrt_price_x64: u128,
    /// 与当前价格相关的tick
    tick: i32,
    /// 当前范围内的流动性
    liquidity: u128,
}

/// 步骤计算结构体（与CLI utils.rs中的StepComputations完全一致）
#[derive(Default)]
struct StepComputations {
    /// 步骤开始时的价格
    sqrt_price_start_x64: u128,
    /// 从当前tick开始，按交换方向的下一个要交换到的tick
    tick_next: i32,
    /// tick_next是否已初始化
    initialized: bool,
    /// 下一个tick的价格平方根
    sqrt_price_next_x64: u128,
    /// 在此步骤中被交换进来的数量
    amount_in: u64,
    /// 被交换出去的数量
    amount_out: u64,
    /// 支付的手续费数量
    fee_amount: u64,
}
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
        slippage_bps: u16,
        route_plan: Vec<RoutePlan>,
        transfer_fee_info: Option<TransferFeeInfo>,
        amount_specified: Option<u64>,
        epoch: Option<u64>,
    ) -> SwapComputeV2Data {
        let other_amount_threshold = MathUtils::calculate_minimum_amount_out(output_amount, slippage_bps);

        SwapComputeV2Data {
            swap_type,
            input_mint,
            input_amount,
            output_mint,
            output_amount: output_amount.to_string(),
            other_amount_threshold: other_amount_threshold.to_string(),
            slippage_bps,
            price_impact_pct: 0.1, // TODO: 实现精确的价格影响计算
            referrer_amount: "0".to_string(),
            route_plan,
            transfer_fee_info,
            amount_specified: amount_specified.map(|a| a.to_string()),
            epoch,
        }
    }
}

impl SolanaService {
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
            info!("🔧 正在初始化Raydium交换服务...");

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

    /// 将字符串转换为u64
    fn parse_amount(&self, amount_str: &str) -> Result<u64> {
        amount_str.parse::<u64>().map_err(|e| anyhow::anyhow!("金额格式错误: {}", e))
    }

    /// 计算池子地址（使用PDA）
    fn calculate_pool_address_pda(&self, input_mint: &str, output_mint: &str) -> Result<String> {
        LogUtils::log_operation_start("PDA池子地址计算", &format!("输入: {} -> 输出: {}", input_mint, output_mint));

        let result = PoolInfoManager::calculate_pool_address_pda(input_mint, output_mint)?;

        LogUtils::log_operation_success("PDA池子地址计算", &result);
        Ok(result)
    }

    /// 基于输入金额计算输出（base-in模式）- 使用与CLI完全相同的逻辑
    async fn calculate_output_for_input(&self, input_mint: &str, output_mint: &str, input_amount: u64) -> Result<(u64, String)> {
        // 使用PDA方法计算池子地址
        let pool_address = self.calculate_pool_address_pda(input_mint, output_mint)?;
        info!("🔧 使用与CLI完全相同的交换计算逻辑");
        info!("  池子地址: {}", pool_address);
        info!("  输入金额: {}", input_amount);

        // 【关键修复】使用与CLI完全相同的计算逻辑
        match self.calculate_output_using_cli_logic(input_mint, output_mint, input_amount, &pool_address, true).await {
            Ok(output_amount) => {
                info!("  ✅ CLI逻辑计算成功: {} -> {}", input_amount, output_amount);
                Ok((output_amount, pool_address))
            }
            Err(e) => {
                warn!("  ⚠️ CLI逻辑计算失败: {:?}，使用备用计算", e);
                // 如果计算失败，使用备用简化计算
                let output_amount = self.fallback_price_calculation(input_mint, output_mint, input_amount).await?;
                Ok((output_amount, pool_address))
            }
        }
    }

    /// 创建路由计划（支持正确的remainingAccounts和lastPoolPriceX64）
    async fn create_route_plan(&self, pool_id: String, input_mint: String, output_mint: String, fee_amount: u64, amount_specified: u64) -> Result<RoutePlan> {
        LogUtils::log_operation_start("路由计划创建", &format!("池子: {}", pool_id));

        // 获取正确的remaining accounts和pool price，使用扣除转账费后的金额
        let (remaining_accounts, last_pool_price_x64) = self.get_remaining_accounts_and_pool_price(&pool_id, &input_mint, &output_mint, amount_specified).await?;

        let route_plan = RoutePlan {
            pool_id,
            input_mint: input_mint.clone(),
            output_mint: output_mint.clone(),
            fee_mint: input_mint, // 通常手续费使用输入代币
            fee_rate: 25,         // 0.25% 手续费率（Raydium标准）
            fee_amount: fee_amount.to_string(),
            remaining_accounts,
            last_pool_price_x64,
        };

        LogUtils::log_operation_success("路由计划创建", "路由计划已生成");
        Ok(route_plan)
    }

    /// 获取remaining accounts和pool price（使用CLI完全相同的精确计算）
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
                self.get_remaining_accounts_from_official_api(pool_id, input_mint, output_mint, amount_specified).await
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
        // let zero_for_one = input_mint_pubkey == mint0;

        LogUtils::log_debug_info(
            "计算参数",
            &[("mint0", &mint0.to_string()), ("mint1", &mint1.to_string()), ("zero_for_one", &zero_for_one.to_string()), ("pool_pubkey", &pool_pubkey.to_string())],
        );

        // 批量加载账户
        let load_accounts = vec![input_mint_pubkey, output_mint_pubkey, amm_config_key, pool_pubkey, tickarray_bitmap_extension_pda, mint0, mint1];

        let accounts = self.rpc_client.get_multiple_accounts(&load_accounts)?;

        // 使用统一的错误处理
        let amm_config_account = accounts[2].as_ref().ok_or_else(|| ErrorHandler::handle_account_load_error("AMM配置"))?;
        let pool_account = accounts[3].as_ref().ok_or_else(|| ErrorHandler::handle_account_load_error("池子"))?;
        let tickarray_bitmap_extension_account = accounts[4].as_ref().ok_or_else(|| ErrorHandler::handle_account_load_error("bitmap扩展"))?;
        let _mint0_account = accounts[5].as_ref().ok_or_else(|| ErrorHandler::handle_account_load_error("mint0"))?;
        let _mint1_account = accounts[6].as_ref().ok_or_else(|| ErrorHandler::handle_account_load_error("mint1"))?;

        // 反序列化关键状态
        let amm_config_state: raydium_amm_v3::states::AmmConfig = self.deserialize_anchor_account(amm_config_account)?;
        let pool_state: raydium_amm_v3::states::PoolState = self.deserialize_anchor_account(pool_account)?;
        let tickarray_bitmap_extension: raydium_amm_v3::states::TickArrayBitmapExtension = self.deserialize_anchor_account(tickarray_bitmap_extension_account)?;

        let epoch = self.rpc_client.get_epoch_info()?.epoch;
        LogUtils::log_debug_info("计算状态", &[("epoch", &epoch.to_string()), ("amount_specified", &amount_specified.to_string())]);

        // 加载tick arrays
        let mut tick_arrays = self.load_cur_and_next_five_tick_array_like_cli(&pool_state, &tickarray_bitmap_extension, zero_for_one, &raydium_program_id, &pool_pubkey).await?;

        // 执行计算
        let (_other_amount_threshold, tick_array_indexs) = self.get_output_amount_and_remaining_accounts_cli_exact(amount_specified, None, zero_for_one, true, &amm_config_state, &pool_state, &tickarray_bitmap_extension, &mut tick_arrays)?;

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

    /// 从官方API获取remaining accounts（备用方案）
    async fn get_remaining_accounts_from_official_api(&self, pool_id: &str, input_mint: &str, output_mint: &str, amount_specified: u64) -> Result<(Vec<String>, String)> {
        warn!("🌐 使用官方API获取remaining accounts（备用方案）");

        // 调用Raydium官方API
        let url = format!(
            "https://transaction-v1.raydium.io/compute/swap-base-in?inputMint={}&outputMint={}&amount={}&slippageBps=50&txVersion=V0",
            input_mint, output_mint, amount_specified
        );

        let response = reqwest::get(&url).await?;
        if !response.status().is_success() {
            return Err(anyhow::anyhow!("官方API请求失败: {}", response.status()));
        }

        let data: serde_json::Value = response.json().await?;

        // 提取remaining accounts和lastPoolPriceX64
        if let Some(route_plan) = data.get("data").and_then(|d| d.get("routePlan")).and_then(|r| r.as_array()).and_then(|arr| arr.first()) {
            let remaining_accounts = route_plan
                .get("remainingAccounts")
                .and_then(|r| r.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect::<Vec<String>>())
                .unwrap_or_default();

            let last_pool_price_x64 = route_plan.get("lastPoolPriceX64").and_then(|p| p.as_str()).unwrap_or("0").to_string();

            info!("✅ 从官方API获取成功");
            info!("  Remaining accounts: {:?}", remaining_accounts);
            info!("  Pool price X64: {}", last_pool_price_x64);

            Ok((remaining_accounts, last_pool_price_x64))
        } else {
            Err(anyhow::anyhow!("无法从官方API响应中提取数据"))
        }
    }

    /// 加载当前和接下来的5个tick arrays（临时禁用）
    #[allow(dead_code)]
    async fn load_cur_and_next_five_tick_array(&self, _pool_pubkey: Pubkey) -> Result<()> {
        // 临时禁用此方法，因为需要raydium_amm_v3依赖
        warn!("load_cur_and_next_five_tick_array 方法已临时禁用");
        Ok(())
    }

    /// 反序列化anchor账户（复制CLI逻辑）
    fn deserialize_anchor_account<T: anchor_lang::AccountDeserialize>(&self, account: &solana_sdk::account::Account) -> Result<T> {
        let mut data: &[u8] = &account.data;
        T::try_deserialize(&mut data).map_err(Into::into)
    }

    /// 加载当前和接下来的5个tick arrays（复制CLI逻辑）
    async fn load_cur_and_next_five_tick_array_like_cli(
        &self,
        pool_state: &raydium_amm_v3::states::PoolState,
        tickarray_bitmap_extension: &raydium_amm_v3::states::TickArrayBitmapExtension,
        zero_for_one: bool,
        raydium_program_id: &Pubkey,
        pool_pubkey: &Pubkey, // 新增池子地址参数
    ) -> Result<std::collections::VecDeque<raydium_amm_v3::states::TickArrayState>> {
        let (_, mut current_valid_tick_array_start_index) = pool_state
            .get_first_initialized_tick_array(&Some(*tickarray_bitmap_extension), zero_for_one)
            .map_err(|e| anyhow::anyhow!("获取第一个初始化的tick array失败: {:?}", e))?;

        let mut tick_array_keys = Vec::new();

        tick_array_keys.push(
            Pubkey::find_program_address(
                &[
                    "tick_array".as_bytes(),
                    pool_pubkey.as_ref(), // 使用传入的池子地址
                    current_valid_tick_array_start_index.to_be_bytes().as_ref(),
                ],
                raydium_program_id,
            )
            .0,
        );

        let mut max_array_size = 5;
        while max_array_size != 0 {
            let next_tick_array_index = pool_state
                .next_initialized_tick_array_start_index(&Some(*tickarray_bitmap_extension), current_valid_tick_array_start_index, zero_for_one)
                .map_err(|e| anyhow::anyhow!("获取下一个tick array索引失败: {:?}", e))?;

            if next_tick_array_index.is_none() {
                break;
            }
            current_valid_tick_array_start_index = next_tick_array_index.unwrap();
            tick_array_keys.push(
                Pubkey::find_program_address(
                    &[
                        "tick_array".as_bytes(),
                        pool_pubkey.as_ref(), // 使用传入的池子地址
                        current_valid_tick_array_start_index.to_be_bytes().as_ref(),
                    ],
                    raydium_program_id,
                )
                .0,
            );
            max_array_size -= 1;
        }

        let tick_array_rsps = self.rpc_client.get_multiple_accounts(&tick_array_keys)?;
        let mut tick_arrays = std::collections::VecDeque::new();

        for tick_array in tick_array_rsps {
            match tick_array {
                Some(account) => {
                    let tick_array_state: raydium_amm_v3::states::TickArrayState = self.deserialize_anchor_account(&account)?;
                    tick_arrays.push_back(tick_array_state);
                }
                None => {
                    warn!("某个tick array账户不存在，跳过");
                }
            }
        }

        Ok(tick_arrays)
    }

    /// 【关键修复方法】精确移植CLI的get_out_put_amount_and_remaining_accounts函数逻辑
    /// 这是修复remainingAccounts和lastPoolPriceX64问题的核心方法
    fn get_output_amount_and_remaining_accounts_cli_exact(
        &self,
        input_amount: u64,
        sqrt_price_limit_x64: Option<u128>,
        zero_for_one: bool,
        is_base_input: bool,
        pool_config: &raydium_amm_v3::states::AmmConfig,
        pool_state: &raydium_amm_v3::states::PoolState,
        tickarray_bitmap_extension: &raydium_amm_v3::states::TickArrayBitmapExtension,
        tick_arrays: &mut std::collections::VecDeque<raydium_amm_v3::states::TickArrayState>,
    ) -> Result<(u64, std::collections::VecDeque<i32>)> {
        info!("🔧 执行CLI精确相同的get_out_put_amount_and_remaining_accounts逻辑");

        // 获取第一个初始化的tick array（与CLI第322-324行完全一致）
        let (is_pool_current_tick_array, current_vaild_tick_array_start_index) = pool_state
            .get_first_initialized_tick_array(&Some(*tickarray_bitmap_extension), zero_for_one)
            .map_err(|e| anyhow::anyhow!("获取第一个初始化tick array失败: {:?}", e))?;

        // 执行交换计算（与CLI第326-337行完全一致）
        let (amount_calculated, tick_array_start_index_vec) = self.swap_compute_cli_exact(
            zero_for_one,
            is_base_input,
            is_pool_current_tick_array,
            pool_config.trade_fee_rate,
            input_amount,
            current_vaild_tick_array_start_index,
            sqrt_price_limit_x64.unwrap_or(0),
            pool_state,
            tickarray_bitmap_extension,
            tick_arrays,
        )?;

        info!("  计算出的tick_array索引: {:?}", tick_array_start_index_vec);
        info!("  计算出的金额: {}", amount_calculated);

        Ok((amount_calculated, tick_array_start_index_vec))
    }

    /// 【关键修复方法】精确移植CLI的swap_compute函数逻辑
    /// 完全按照CLI utils.rs中的swap_compute函数实现
    fn swap_compute_cli_exact(
        &self,
        zero_for_one: bool,
        is_base_input: bool,
        is_pool_current_tick_array: bool,
        fee: u32,
        amount_specified: u64,
        current_vaild_tick_array_start_index: i32,
        sqrt_price_limit_x64: u128,
        pool_state: &raydium_amm_v3::states::PoolState,
        tickarray_bitmap_extension: &raydium_amm_v3::states::TickArrayBitmapExtension,
        tick_arrays: &mut std::collections::VecDeque<raydium_amm_v3::states::TickArrayState>,
    ) -> Result<(u64, std::collections::VecDeque<i32>)> {
        use raydium_amm_v3::libraries::{liquidity_math, swap_math, tick_math};
        use std::ops::Neg;

        if amount_specified == 0 {
            return Err(anyhow::anyhow!("amountSpecified must not be 0"));
        }

        // 价格限制处理（与CLI第358-366行完全一致）
        let sqrt_price_limit_x64 = if sqrt_price_limit_x64 == 0 {
            if zero_for_one {
                tick_math::MIN_SQRT_PRICE_X64 + 1
            } else {
                tick_math::MAX_SQRT_PRICE_X64 - 1
            }
        } else {
            sqrt_price_limit_x64
        };

        // 价格限制验证（与CLI第367-381行完全一致）
        if zero_for_one {
            if sqrt_price_limit_x64 < tick_math::MIN_SQRT_PRICE_X64 {
                return Err(anyhow::anyhow!("sqrt_price_limit_x64 must greater than MIN_SQRT_PRICE_X64"));
            }
            if sqrt_price_limit_x64 >= pool_state.sqrt_price_x64 {
                return Err(anyhow::anyhow!("sqrt_price_limit_x64 must smaller than current"));
            }
        } else {
            if sqrt_price_limit_x64 > tick_math::MAX_SQRT_PRICE_X64 {
                return Err(anyhow::anyhow!("sqrt_price_limit_x64 must smaller than MAX_SQRT_PRICE_X64"));
            }
            if sqrt_price_limit_x64 <= pool_state.sqrt_price_x64 {
                return Err(anyhow::anyhow!("sqrt_price_limit_x64 must greater than current"));
            }
        }

        // 初始化交换状态（与CLI第384-390行完全一致）
        let mut tick_match_current_tick_array = is_pool_current_tick_array;
        let mut state = SwapState {
            amount_specified_remaining: amount_specified,
            amount_calculated: 0,
            sqrt_price_x64: pool_state.sqrt_price_x64,
            tick: pool_state.tick_current,
            liquidity: pool_state.liquidity,
        };

        // 获取当前tick array（与CLI第392-398行完全一致）
        let mut tick_array_current = tick_arrays.pop_front().ok_or_else(|| anyhow::anyhow!("没有可用的tick array"))?;
        if tick_array_current.start_tick_index != current_vaild_tick_array_start_index {
            return Err(anyhow::anyhow!("tick array start tick index does not match"));
        }
        let mut tick_array_start_index_vec = std::collections::VecDeque::new();
        tick_array_start_index_vec.push_back(tick_array_current.start_tick_index);

        let mut loop_count = 0;

        // 主交换循环（与CLI第400-525行完全一致）
        while state.amount_specified_remaining != 0 && state.sqrt_price_x64 != sqrt_price_limit_x64 && state.tick < tick_math::MAX_TICK && state.tick > tick_math::MIN_TICK {
            if loop_count > 10 {
                return Err(anyhow::anyhow!("loop_count limit"));
            }

            let mut step = StepComputations::default();
            step.sqrt_price_start_x64 = state.sqrt_price_x64;

            // 查找下一个初始化tick（与CLI第411-427行完全一致）
            let mut next_initialized_tick = if let Some(tick_state) = tick_array_current
                .next_initialized_tick(state.tick, pool_state.tick_spacing, zero_for_one)
                .map_err(|e| anyhow::anyhow!("next_initialized_tick failed: {:?}", e))?
            {
                Box::new(*tick_state)
            } else {
                if !tick_match_current_tick_array {
                    tick_match_current_tick_array = true;
                    Box::new(*tick_array_current.first_initialized_tick(zero_for_one).map_err(|e| anyhow::anyhow!("first_initialized_tick failed: {:?}", e))?)
                } else {
                    Box::new(raydium_amm_v3::states::TickState::default())
                }
            };

            // 如果当前tick array没有更多初始化tick，切换到下一个（与CLI第428-450行完全一致）
            if !next_initialized_tick.is_initialized() {
                let current_vaild_tick_array_start_index = pool_state
                    .next_initialized_tick_array_start_index(&Some(*tickarray_bitmap_extension), current_vaild_tick_array_start_index, zero_for_one)
                    .map_err(|e| anyhow::anyhow!("next_initialized_tick_array_start_index failed: {:?}", e))?;

                if current_vaild_tick_array_start_index.is_none() {
                    return Err(anyhow::anyhow!("tick array start tick index out of range limit"));
                }

                tick_array_current = tick_arrays.pop_front().ok_or_else(|| anyhow::anyhow!("没有更多tick arrays"))?;
                let expected_index = current_vaild_tick_array_start_index.unwrap();
                if tick_array_current.start_tick_index != expected_index {
                    return Err(anyhow::anyhow!("tick array start tick index does not match"));
                }
                tick_array_start_index_vec.push_back(tick_array_current.start_tick_index);

                let first_initialized_tick = tick_array_current.first_initialized_tick(zero_for_one).map_err(|e| anyhow::anyhow!("first_initialized_tick failed: {:?}", e))?;

                next_initialized_tick = Box::new(*first_initialized_tick);
            }

            // 设置下一个tick和价格（与CLI第451-467行完全一致）
            step.tick_next = next_initialized_tick.tick;
            step.initialized = next_initialized_tick.is_initialized();
            if step.tick_next < tick_math::MIN_TICK {
                step.tick_next = tick_math::MIN_TICK;
            } else if step.tick_next > tick_math::MAX_TICK {
                step.tick_next = tick_math::MAX_TICK;
            }

            step.sqrt_price_next_x64 = tick_math::get_sqrt_price_at_tick(step.tick_next).map_err(|e| anyhow::anyhow!("get_sqrt_price_at_tick failed: {:?}", e))?;

            let target_price = if (zero_for_one && step.sqrt_price_next_x64 < sqrt_price_limit_x64) || (!zero_for_one && step.sqrt_price_next_x64 > sqrt_price_limit_x64) {
                sqrt_price_limit_x64
            } else {
                step.sqrt_price_next_x64
            };

            // 计算交换步骤（与CLI第468-482行完全一致）
            let swap_step = swap_math::compute_swap_step(state.sqrt_price_x64, target_price, state.liquidity, state.amount_specified_remaining, fee, is_base_input, zero_for_one, 1).map_err(|e| anyhow::anyhow!("compute_swap_step failed: {:?}", e))?;

            state.sqrt_price_x64 = swap_step.sqrt_price_next_x64;
            step.amount_in = swap_step.amount_in;
            step.amount_out = swap_step.amount_out;
            step.fee_amount = swap_step.fee_amount;

            // 更新状态（与CLI第484-502行完全一致）
            if is_base_input {
                state.amount_specified_remaining = state.amount_specified_remaining.checked_sub(step.amount_in + step.fee_amount).unwrap();
                state.amount_calculated = state.amount_calculated.checked_add(step.amount_out).unwrap();
            } else {
                state.amount_specified_remaining = state.amount_specified_remaining.checked_sub(step.amount_out).unwrap();
                state.amount_calculated = state.amount_calculated.checked_add(step.amount_in + step.fee_amount).unwrap();
            }

            // 处理tick转换（与CLI第504-523行完全一致）
            if state.sqrt_price_x64 == step.sqrt_price_next_x64 {
                if step.initialized {
                    let mut liquidity_net = next_initialized_tick.liquidity_net;
                    if zero_for_one {
                        liquidity_net = liquidity_net.neg();
                    }
                    state.liquidity = liquidity_math::add_delta(state.liquidity, liquidity_net).map_err(|e| anyhow::anyhow!("add_delta failed: {:?}", e))?;
                }

                state.tick = if zero_for_one { step.tick_next - 1 } else { step.tick_next };
            } else if state.sqrt_price_x64 != step.sqrt_price_start_x64 {
                state.tick = tick_math::get_tick_at_sqrt_price(state.sqrt_price_x64).map_err(|e| anyhow::anyhow!("get_tick_at_sqrt_price failed: {:?}", e))?;
            }

            loop_count += 1;
        }

        Ok((state.amount_calculated, tick_array_start_index_vec))
    }

    /// 【关键修复方法】使用与CLI完全相同的计算逻辑
    /// 这个方法复制了CLI中 SwapV2 CommandsName::SwapV2 的完整计算逻辑
    async fn calculate_output_using_cli_logic(&self, input_mint: &str, output_mint: &str, amount: u64, pool_address: &str, base_in: bool) -> Result<u64> {
        info!("🔧 执行与CLI完全相同的交换计算逻辑");

        use std::str::FromStr;

        let pool_pubkey = Pubkey::from_str(pool_address)?;
        let input_mint_pubkey = Pubkey::from_str(input_mint)?;
        let output_mint_pubkey = Pubkey::from_str(output_mint)?;

        // 1. 使用ConfigManager获取配置
        let raydium_program_id = ConfigManager::get_raydium_program_id()?;
        // let amm_config_index = ConfigManager::get_amm_config_index();

        // 2. 使用PDACalculator计算PDA地址
        // let (_amm_config_key, _) = PDACalculator::calculate_amm_config_pda(&raydium_program_id, amm_config_index);
        // let (_tickarray_bitmap_extension_pda, _) = PDACalculator::calculate_tickarray_bitmap_extension_pda(&raydium_program_id, &pool_pubkey);

        // 3. 使用TokenUtils标准化mint顺序
        let (mint0, mint1, _zero_for_one) = TokenUtils::normalize_mint_order(&input_mint_pubkey, &output_mint_pubkey);

        // 4. 使用AccountLoader加载核心交换账户
        let account_loader = AccountLoader::new(&self.rpc_client);
        let swap_accounts = account_loader.load_swap_core_accounts(&pool_pubkey, &input_mint_pubkey, &output_mint_pubkey).await?;

        // 为了保持与CLI完全一致，我们仍需要获取原始mint账户数据用于transfer fee计算
        let load_accounts = vec![mint0, mint1];
        let mint_accounts = self.rpc_client.get_multiple_accounts(&load_accounts)?;
        let mint0_account = mint_accounts[0].as_ref().ok_or_else(|| anyhow::anyhow!("无法加载mint0账户"))?;
        let mint1_account = mint_accounts[1].as_ref().ok_or_else(|| anyhow::anyhow!("无法加载mint1账户"))?;

        // 5. 使用TransferFeeCalculator计算transfer fee
        let epoch = self.rpc_client.get_epoch_info()?.epoch;
        let transfer_fee = if base_in {
            if swap_accounts.zero_for_one {
                TransferFeeCalculator::get_transfer_fee_from_mint_state_simple(&mint0_account.data, epoch, amount)?
            } else {
                TransferFeeCalculator::get_transfer_fee_from_mint_state_simple(&mint1_account.data, epoch, amount)?
            }
        } else {
            0
        };
        let amount_specified = amount.checked_sub(transfer_fee).unwrap();

        info!("💰 Transfer fee计算:");
        info!("  原始金额: {}", amount);
        info!("  Transfer fee: {}", transfer_fee);
        info!("  扣除费用后金额: {}", amount_specified);

        // 6. 加载当前和接下来的5个tick arrays（与CLI第1824-1830行完全一致）
        let mut tick_arrays = self
            .load_cur_and_next_five_tick_array_like_cli(&swap_accounts.pool_state, &swap_accounts.tickarray_bitmap_extension, swap_accounts.zero_for_one, &raydium_program_id, &pool_pubkey)
            .await?;

        // 7. 使用CLI完全相同的get_out_put_amount_and_remaining_accounts逻辑
        let (other_amount_threshold, _tick_array_indexs) = self.get_output_amount_and_remaining_accounts_cli_exact(
            amount_specified,
            None, // sqrt_price_limit_x64
            swap_accounts.zero_for_one,
            base_in,
            &swap_accounts.amm_config_state,
            &swap_accounts.pool_state,
            &swap_accounts.tickarray_bitmap_extension,
            &mut tick_arrays,
        )?;

        info!("✅ CLI完全相同逻辑计算完成");
        info!("  输入金额: {} (原始: {})", amount_specified, amount);
        info!("  输出金额: {}", other_amount_threshold);
        info!("  Transfer fee: {}", transfer_fee);
        info!("  Zero for one: {}", swap_accounts.zero_for_one);

        Ok(other_amount_threshold)
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

            raydium.swap_tokens(&request.from_token, &request.to_token, &request.pool_address, request.amount, request.minimum_amount_out).await?
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

    /// 构建SwapV2指令
    fn build_swap_v2_instruction(
        &self,
        program_id: &Pubkey,
        amm_config: &Pubkey,
        pool_state: &Pubkey,
        payer: &Pubkey,
        input_token_account: &Pubkey,
        output_token_account: &Pubkey,
        input_vault: &Pubkey,
        output_vault: &Pubkey,
        input_vault_mint: &Pubkey,
        output_vault_mint: &Pubkey,
        observation_state: &Pubkey,
        remaining_accounts: Vec<solana_sdk::instruction::AccountMeta>,
        amount: u64,
        other_amount_threshold: u64,
        sqrt_price_limit_x64: Option<u128>,
        is_base_input: bool,
    ) -> Result<solana_sdk::instruction::Instruction> {
        LogUtils::log_operation_start("SwapV2指令构建", &format!("金额: {}", amount));

        use borsh::BorshSerialize;

        // SwapV2指令的discriminator
        let discriminator: [u8; 8] = [0x37, 0x32, 0xD4, 0xEC, 0xB6, 0x95, 0x4B, 0x5B];

        #[derive(BorshSerialize)]
        struct SwapV2Args {
            amount: u64,
            other_amount_threshold: u64,
            sqrt_price_limit_x64: u128,
            is_base_input: bool,
        }

        let args = SwapV2Args {
            amount,
            other_amount_threshold,
            sqrt_price_limit_x64: sqrt_price_limit_x64.unwrap_or(0),
            is_base_input,
        };

        let mut data = discriminator.to_vec();
        args.serialize(&mut data)?;

        // 使用工具类构建账户列表
        let mut accounts = vec![
            AccountMetaBuilder::signer(*payer),
            AccountMetaBuilder::readonly(*amm_config, false),
            AccountMetaBuilder::writable(*pool_state, false),
            AccountMetaBuilder::writable(*input_token_account, false),
            AccountMetaBuilder::writable(*output_token_account, false),
            AccountMetaBuilder::writable(*input_vault, false),
            AccountMetaBuilder::writable(*output_vault, false),
            AccountMetaBuilder::writable(*observation_state, false),
            AccountMetaBuilder::readonly(spl_token::id(), false),
            AccountMetaBuilder::readonly(spl_token_2022::id(), false),
            AccountMetaBuilder::readonly(spl_memo::id(), false),
            AccountMetaBuilder::readonly(*input_vault_mint, false),
            AccountMetaBuilder::readonly(*output_vault_mint, false),
        ];

        accounts.extend(remaining_accounts);

        LogUtils::log_operation_success("SwapV2指令构建", &format!("{}个账户", accounts.len()));
        Ok(solana_sdk::instruction::Instruction { program_id: *program_id, accounts, data })
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

        let input_amount = self.parse_amount(&params.amount)?;
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

        let (output_amount, pool_address_str) = self.calculate_output_for_input(&params.input_mint, &params.output_mint, amount_specified).await?;

        let fee_amount = RoutePlanBuilder::calculate_standard_fee(amount_specified);
        let route_plan = vec![self.create_route_plan(pool_address_str, params.input_mint.clone(), params.output_mint.clone(), fee_amount, amount_specified).await?];

        let epoch = self.swap_v2_service.get_current_epoch()?;

        let result = ResponseBuilder::create_swap_compute_v2_data(
            "BaseInV2".to_string(),
            params.input_mint,
            params.amount,
            params.output_mint,
            output_amount,
            params.slippage_bps,
            route_plan,
            transfer_fee_info,
            Some(amount_specified),
            Some(epoch),
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

        let output_amount = self.parse_amount(&params.amount)?;
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

        let (input_amount, pool_address_str) = self.calculate_output_for_input(&params.input_mint, &params.output_mint, amount_specified).await?;

        let fee_amount = RoutePlanBuilder::calculate_standard_fee(output_amount);
        let route_plan = vec![self.create_route_plan(pool_address_str, params.input_mint.clone(), params.output_mint.clone(), fee_amount, output_amount).await?];

        let epoch = self.swap_v2_service.get_current_epoch()?;

        let result = ResponseBuilder::create_swap_compute_v2_data(
            "BaseOutV2".to_string(),
            params.input_mint,
            input_amount.to_string(),
            params.output_mint,
            output_amount,
            params.slippage_bps,
            route_plan,
            transfer_fee_info,
            Some(input_amount),
            Some(epoch),
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

        let swap_data = &request.swap_response.data;
        let input_amount = self.parse_amount(&swap_data.input_amount)?;
        let other_amount_threshold = self.parse_amount(&swap_data.other_amount_threshold)?;
        let user_wallet = Pubkey::from_str(&request.wallet)?;

        let actual_amount = if let Some(ref amount_specified) = swap_data.amount_specified { self.parse_amount(amount_specified)? } else { input_amount };

        let route_plan = swap_data.route_plan.first().ok_or_else(|| ErrorHandler::create_error("未找到路由计划"))?;

        let pool_id = Pubkey::from_str(&route_plan.pool_id)?;
        let input_mint = Pubkey::from_str(&swap_data.input_mint)?;
        let output_mint = Pubkey::from_str(&swap_data.output_mint)?;

        LogUtils::log_debug_info("交易参数", &[("池子ID", &pool_id.to_string()), ("输入金额", &actual_amount.to_string()), ("最小输出", &other_amount_threshold.to_string())]);

        // 获取池子状态
        let pool_account = self.rpc_client.get_account(&pool_id)?;
        let pool_state: raydium_amm_v3::states::PoolState = self.deserialize_anchor_account(&pool_account)?;

        // 计算ATA账户
        let user_input_token_account = spl_associated_token_account::get_associated_token_address(&user_wallet, &input_mint);
        let user_output_token_account = spl_associated_token_account::get_associated_token_address(&user_wallet, &output_mint);

        // 确定vault账户
        let (input_vault, output_vault, input_vault_mint, output_vault_mint) = if input_mint == pool_state.token_mint_0 {
            (pool_state.token_vault_0, pool_state.token_vault_1, pool_state.token_mint_0, pool_state.token_mint_1)
        } else {
            (pool_state.token_vault_1, pool_state.token_vault_0, pool_state.token_mint_1, pool_state.token_mint_0)
        };

        // 构建remaining accounts
        let remaining_accounts = AccountMetaBuilder::create_remaining_accounts(&route_plan.remaining_accounts, true)?;

        let raydium_program_id = ConfigManager::get_raydium_program_id()?;

        // 构建SwapV2指令
        let ix = self.build_swap_v2_instruction(
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
        let recent_blockhash = self.rpc_client.get_latest_blockhash()?;
        let transaction = TransactionBuilder::build_transaction(vec![ix], &user_wallet, recent_blockhash)?;
        let transaction_base64 = TransactionBuilder::serialize_transaction_to_base64(&transaction)?;

        LogUtils::log_operation_success("swap-v2-base-in交易构建", &format!("交易大小: {} bytes", transaction_base64.len()));

        Ok(TransactionData { transaction: transaction_base64 })
    }

    async fn build_swap_v2_transaction_base_out(&self, request: TransactionSwapV2Request) -> Result<TransactionData> {
        info!("🔨 构建swap-v2-base-out交易");
        info!("  钱包地址: {}", request.wallet);
        info!("  交易版本: {}", request.tx_version);

        // 从swap_response中提取交换数据
        let swap_data = &request.swap_response.data;
        let output_amount = self.parse_amount(&swap_data.output_amount)?;
        let other_amount_threshold = self.parse_amount(&swap_data.other_amount_threshold)?;
        let user_wallet = Pubkey::from_str(&request.wallet)?;

        // 对于base-out，amount_specified通常是期望的输出金额
        let actual_output_amount = if let Some(ref amount_specified) = swap_data.amount_specified {
            self.parse_amount(amount_specified)?
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
        let (input_vault, output_vault, input_vault_mint, output_vault_mint) = if input_mint == pool_state.token_mint_0 {
            (pool_state.token_vault_0, pool_state.token_vault_1, pool_state.token_mint_0, pool_state.token_mint_1)
        } else {
            (pool_state.token_vault_1, pool_state.token_vault_0, pool_state.token_mint_1, pool_state.token_mint_0)
        };

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
        let ix = self.build_swap_v2_instruction(
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

        // 添加compute budget指令
        let compute_budget_ix = solana_sdk::compute_budget::ComputeBudgetInstruction::set_compute_unit_limit(1_400_000);

        // 创建交易
        let recent_blockhash = self.rpc_client.get_latest_blockhash()?;
        let mut transaction = solana_sdk::transaction::Transaction::new_unsigned(solana_sdk::message::Message::new(&[compute_budget_ix, ix], Some(&user_wallet)));
        transaction.message.recent_blockhash = recent_blockhash;

        // 序列化交易（不包含签名）
        let serialized = bincode::serialize(&transaction)?;
        let transaction_base64 = STANDARD.encode(&serialized);

        info!("✅ 交易构建成功");
        info!("  交易大小: {} bytes", serialized.len());
        info!("  Base64长度: {}", transaction_base64.len());

        Ok(TransactionData { transaction: transaction_base64 })
    }
}

impl Default for SolanaService {
    fn default() -> Self {
        Self::new()
    }
}
