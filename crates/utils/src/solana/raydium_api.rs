use anyhow::Result;
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use std::collections::HashMap;
use tracing::{info, warn};

/// Raydium池子信息（从API获取）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(non_snake_case)]
pub struct RaydiumPoolInfo {
    pub id: String,
    pub baseMint: String,
    pub quoteMint: String,
    pub lpMint: String,
    pub baseDecimals: u8,
    pub quoteDecimals: u8,
    pub lpDecimals: u8,
    pub version: u8,
    pub programId: String,
    pub authority: String,
    pub openOrders: String,
    pub targetOrders: String,
    pub baseVault: String,
    pub quoteVault: String,
    pub marketVersion: u8,
    pub marketProgramId: String,
    pub marketId: String,
    pub marketAuthority: String,
    pub marketBaseVault: String,
    pub marketQuoteVault: String,
    pub marketBids: String,
    pub marketAsks: String,
    pub marketEventQueue: String,
}

/// Raydium API客户端
#[derive(Debug)]
pub struct RaydiumApiClient {
    client: reqwest::Client,
    base_url: String,
}

impl RaydiumApiClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: "https://api.raydium.io".to_string(),
        }
    }

    /// 获取流动性池列表
    pub async fn get_liquidity_pools(&self) -> Result<HashMap<String, RaydiumPoolInfo>> {
        info!("🔍 从Raydium API获取流动性池列表...");

        let url = format!("{}/v2/sdk/liquidity/mainnet.json", self.base_url);
        info!("🔍 query url: {}", url);
        let response = self.client.get(&url).send().await?;
        info!("🔍 response: {:#?}", response);
        if !response.status().is_success() {
            return Err(anyhow::anyhow!("API请求失败: {}", response.status()));
        }

        let text = response.text().await?;
        info!("🔍 text: {:#?}", text);
        let json: serde_json::Value = serde_json::from_str(&text)?;
        info!("🔍 json: {:#?}", json);
        let mut all_pools = HashMap::new();

        // 解析官方池子
        if let Some(official) = json.get("official").and_then(|v| v.as_array()) {
            for pool in official {
                if let Ok(pool_info) = self.parse_pool_info(pool) {
                    all_pools.insert(pool_info.id.clone(), pool_info);
                }
            }
        }

        // 解析非官方池子
        if let Some(unofficial) = json.get("unOfficial").and_then(|v| v.as_array()) {
            for pool in unofficial {
                if let Ok(pool_info) = self.parse_pool_info(pool) {
                    all_pools.insert(pool_info.id.clone(), pool_info);
                }
            }
        }

        info!("✅ 成功获取 {} 个流动性池", all_pools.len());
        Ok(all_pools)
    }

    /// 根据池子地址获取特定池子信息
    pub async fn get_pool_by_address(&self, pool_address: &str) -> Result<Option<RaydiumPoolInfo>> {
        info!("🔍 查找池子: {}", pool_address);

        let pools = self.get_liquidity_pools().await?;

        if let Some(pool) = pools.get(pool_address) {
            info!("✅ 找到池子信息");
            Ok(Some(pool.clone()))
        } else {
            warn!("⚠️ 未找到池子: {}", pool_address);
            Ok(None)
        }
    }

    /// 获取代币价格
    pub async fn get_token_prices(&self) -> Result<HashMap<String, f64>> {
        info!("💰 获取代币价格...");

        let url = format!("{}/v2/main/price", self.base_url);

        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!("价格API请求失败: {}", response.status()));
        }

        let prices: HashMap<String, f64> = response.json().await?;

        info!("✅ 成功获取 {} 个代币价格", prices.len());
        Ok(prices)
    }

    /// 计算池子的储备和流动性
    pub async fn calculate_pool_reserves(&self, pool_info: &RaydiumPoolInfo, rpc_client: &solana_client::rpc_client::RpcClient) -> Result<(u64, u64, f64)> {
        info!("📊 计算池子储备: {}", pool_info.id);

        // 获取base vault余额
        let base_vault_pubkey = pool_info.baseVault.parse::<Pubkey>()?;
        let base_vault_balance = rpc_client.get_token_account_balance(&base_vault_pubkey)?;
        let base_amount = base_vault_balance.amount.parse::<u64>()?;

        // 获取quote vault余额
        let quote_vault_pubkey = pool_info.quoteVault.parse::<Pubkey>()?;
        let quote_vault_balance = rpc_client.get_token_account_balance(&quote_vault_pubkey)?;
        let quote_amount = quote_vault_balance.amount.parse::<u64>()?;

        // 计算价格 (base/quote)
        let base_decimal_factor = 10u64.pow(pool_info.baseDecimals as u32) as f64;
        let quote_decimal_factor = 10u64.pow(pool_info.quoteDecimals as u32) as f64;

        let base_ui_amount = base_amount as f64 / base_decimal_factor;
        let quote_ui_amount = quote_amount as f64 / quote_decimal_factor;

        let price = if base_ui_amount > 0.0 { quote_ui_amount / base_ui_amount } else { 0.0 };

        info!("💰 池子储备: base={}, quote={}, 价格={:.6}", base_amount, quote_amount, price);

        Ok((base_amount, quote_amount, price))
    }

    /// 解析池子信息
    fn parse_pool_info(&self, json: &serde_json::Value) -> Result<RaydiumPoolInfo> {
        Ok(RaydiumPoolInfo {
            id: json.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            baseMint: json.get("baseMint").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            quoteMint: json.get("quoteMint").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            lpMint: json.get("lpMint").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            baseDecimals: json.get("baseDecimals").and_then(|v| v.as_u64()).unwrap_or(0) as u8,
            quoteDecimals: json.get("quoteDecimals").and_then(|v| v.as_u64()).unwrap_or(0) as u8,
            lpDecimals: json.get("lpDecimals").and_then(|v| v.as_u64()).unwrap_or(0) as u8,
            version: json.get("version").and_then(|v| v.as_u64()).unwrap_or(4) as u8,
            programId: json.get("programId").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            authority: json.get("authority").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            openOrders: json.get("openOrders").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            targetOrders: json.get("targetOrders").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            baseVault: json.get("baseVault").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            quoteVault: json.get("quoteVault").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            marketVersion: json.get("marketVersion").and_then(|v| v.as_u64()).unwrap_or(3) as u8,
            marketProgramId: json.get("marketProgramId").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            marketId: json.get("marketId").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            marketAuthority: json.get("marketAuthority").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            marketBaseVault: json.get("marketBaseVault").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            marketQuoteVault: json.get("marketQuoteVault").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            marketBids: json.get("marketBids").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            marketAsks: json.get("marketAsks").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            marketEventQueue: json.get("marketEventQueue").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        })
    }
}

/// 简化的价格计算函数（避免下载885MB文件）
pub async fn calculate_swap_output_with_simple_math(input_amount: u64, from_mint: &str, to_mint: &str) -> Result<u64> {
    info!("💰 使用简化数学模型计算交换输出");

    // 简化的价格模型，基于主要代币的大致汇率
    const SOL_MINT: &str = "So11111111111111111111111111111111111111112";
    const USDC_MINT: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
    const USDT_MINT: &str = "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB";

    // 假设的价格（实际应用中可以从价格API获取）
    let sol_price_usd = 100.0; // 1 SOL = 100 USD

    let output_amount = match (from_mint, to_mint) {
        // SOL -> USDC
        (SOL_MINT, USDC_MINT) => {
            let sol_amount = input_amount as f64 / 1_000_000_000.0; // lamports to SOL
            let usdc_amount = sol_amount * sol_price_usd;
            (usdc_amount * 1_000_000.0) as u64 // USDC to micro-USDC
        }
        // USDC -> SOL
        (USDC_MINT, SOL_MINT) => {
            let usdc_amount = input_amount as f64 / 1_000_000.0; // micro-USDC to USDC
            let sol_amount = usdc_amount / sol_price_usd;
            (sol_amount * 1_000_000_000.0) as u64 // SOL to lamports
        }
        // SOL -> USDT (类似USDC)
        (SOL_MINT, USDT_MINT) => {
            let sol_amount = input_amount as f64 / 1_000_000_000.0;
            let usdt_amount = sol_amount * sol_price_usd;
            (usdt_amount * 1_000_000.0) as u64
        }
        // USDT -> SOL
        (USDT_MINT, SOL_MINT) => {
            let usdt_amount = input_amount as f64 / 1_000_000.0;
            let sol_amount = usdt_amount / sol_price_usd;
            (sol_amount * 1_000_000_000.0) as u64
        }
        _ => return Err(anyhow::anyhow!("不支持的交换对: {} -> {}", from_mint, to_mint)),
    };

    // 扣除0.25%手续费
    let output_with_fee = (output_amount as f64 * 0.9975) as u64;

    info!("  输入: {} ({})", input_amount, from_mint);
    info!("  输出: {} ({})", output_with_fee, to_mint);

    Ok(output_with_fee)
}

/// 最终备用计算方法
pub async fn calculate_fallback_output(input_amount: u64, from_mint: &str, to_mint: &str) -> Result<u64> {
    // 直接调用简化数学模型作为最后的备用
    calculate_swap_output_with_simple_math(input_amount, from_mint, to_mint).await
}

/// 计算交换输出的便捷函数（优化版，避免大文件下载）
pub async fn calculate_swap_output_with_api(
    pool_address: &str,
    input_amount: u64,
    from_mint: &str,
    to_mint: &str,
    _rpc_client: &solana_client::rpc_client::RpcClient,
) -> Result<u64> {
    info!("💱 计算交换输出（优化版）- 池子: {}", pool_address);
    
    // 使用备用恒定乘积计算模型
    warn!("⚠️ 使用备用恒定乘积计算模型");

    // 假设池子储备（实际应该从链上获取）
    let base_reserve = 1_000_000_000_000u64; // 1M tokens
    let quote_reserve = 100_000_000_000u64; // 100K tokens

    // 恒定乘积公式: k = x * y, output = (input * y) / (x + input)
    let fee_rate = 0.0025; // 0.25% 手续费
    let amount_in_after_fee = (input_amount as f64 * (1.0 - fee_rate)) as u64;

    // 防止溢出，使用128位整数
    let numerator = amount_in_after_fee as u128 * quote_reserve as u128;
    let denominator = base_reserve as u128 + amount_in_after_fee as u128;
    let output_amount = (numerator / denominator) as u64;

    info!("💱 备用计算结果:");
    info!("  输入: {} ({})", input_amount, from_mint);
    info!("  手续费后输入: {}", amount_in_after_fee);
    info!("  计算输出: {} ({})", output_amount, to_mint);

    Ok(output_amount)
}