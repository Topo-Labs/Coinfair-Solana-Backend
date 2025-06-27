use crate::dtos::solana_dto::{
    SwapRequest, SwapResponse, BalanceResponse, PriceQuoteRequest, PriceQuoteResponse,
    WalletInfo, TransactionStatus, ErrorResponse, ApiResponse
};
use anyhow::Result;
use async_trait::async_trait;
use solana::{SwapConfig, RaydiumSwap, SolanaClient};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, error, warn};

pub type DynSolanaService = Arc<dyn SolanaServiceTrait + Send + Sync>;

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
}

pub struct SolanaService {
    config: SwapConfig,
    raydium_swap: Arc<Mutex<Option<RaydiumSwap>>>,
}

impl SolanaService {
    pub fn new() -> Self {
        Self {
            config: SwapConfig::default(),
            raydium_swap: Arc::new(Mutex::new(None)),
        }
    }

    fn get_config(&self) -> Result<SwapConfig> {
        // 尝试从环境变量加载配置
        info!("🔍 加载Solana配置...");
        
        let rpc_url = std::env::var("RPC_URL")
            .unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".to_string());
        let private_key = std::env::var("PRIVATE_KEY")
            .map_err(|_| anyhow::anyhow!("环境变量PRIVATE_KEY未设置"))?;
        let amm_program_id = std::env::var("RAYDIUM_PROGRAM_ID")
            .unwrap_or_else(|_| "CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK".to_string());
        
        let config = SwapConfig {
            rpc_url,
            private_key,
            amm_program_id,
            openbook_program_id: self.config.openbook_program_id.clone(),
            usdc_mint: self.config.usdc_mint.clone(),
            sol_usdc_pool_id: self.config.sol_usdc_pool_id.clone(),
        };
        
        info!("✅ Solana配置加载成功");
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

    fn calculate_minimum_amount_out(&self, amount_in: u64, slippage_percent: f64) -> u64 {
        let slippage_factor = 1.0 - (slippage_percent / 100.0);
        (amount_in as f64 * slippage_factor) as u64
    }

    async fn estimate_swap_output(&self, from_token: &str, to_token: &str, pool_address: &str, amount: u64) -> Result<u64> {
        info!("💱 估算交换输出 - 池子: {}", pool_address);
        info!("  输入: {} ({})", amount, from_token);
        info!("  输出代币: {}", to_token);
        
        self.ensure_raydium_available().await?;
        
        // 定义mint地址常量
        const SOL_MINT: &str = "So11111111111111111111111111111111111111112";
        const USDC_MINT_STANDARD: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
        const USDC_MINT_CONFIG: &str = "4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU";
        
        // 判断代币类型
        let is_from_sol = from_token == SOL_MINT;
        let is_to_sol = to_token == SOL_MINT;
        let is_from_usdc = matches!(from_token, USDC_MINT_STANDARD | USDC_MINT_CONFIG | "A9mUU4qviSctJVPJdBJWkb28deg915LYJKrzQ19ji3FM");
        let is_to_usdc = matches!(to_token, USDC_MINT_STANDARD | USDC_MINT_CONFIG | "A9mUU4qviSctJVPJdBJWkb28deg915LYJKrzQ19ji3FM");
        
        // 从实际池子获取价格信息
        let estimated_output = {
            let raydium_guard = self.raydium_swap.lock().await;
            let raydium = raydium_guard.as_ref().unwrap();
            
            match raydium.get_pool_price_and_estimate(pool_address, from_token, to_token, amount).await {
                Ok(output) => {
                    info!("  ✅ 从池子获取价格成功，估算输出: {}", output);
                    output
                }
                Err(e) => {
                    warn!("  ⚠️ 从池子获取价格失败: {:?}，使用备用计算", e);
                    
                    // 备用价格计算（简化版本）
                    let sol_price_usdc = 100.0; // 假设1 SOL = 100 USDC
                    
                    match (is_from_sol, is_from_usdc, is_to_sol, is_to_usdc) {
                        (true, false, false, true) => {
                            // SOL -> USDC
                            let sol_amount = amount as f64 / 1_000_000_000.0; // lamports to SOL
                            let usdc_amount = sol_amount * sol_price_usdc;
                            (usdc_amount * 1_000_000.0) as u64 // USDC to micro-USDC
                        }
                        (false, true, true, false) => {
                            // USDC -> SOL
                            let usdc_amount = amount as f64 / 1_000_000.0; // micro-USDC to USDC
                            let sol_amount = usdc_amount / sol_price_usdc;
                            (sol_amount * 1_000_000_000.0) as u64 // SOL to lamports
                        }
                        _ => return Err(anyhow::anyhow!("不支持的交换对: {} -> {}", from_token, to_token)),
                    }
                }
            }
        };

        info!("  📊 最终估算输出: {}", estimated_output);
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
}

impl Default for SolanaService {
    fn default() -> Self {
        Self::new()
    }
} 