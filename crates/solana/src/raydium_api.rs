use anyhow::Result;
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use std::collections::HashMap;
use tracing::{info, warn, error};

/// Raydium池子信息（从API获取）
#[derive(Debug, Clone, Serialize, Deserialize)]
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
        
        let response = self.client
            .get(&url)
            .send()
            .await?;
            
        if !response.status().is_success() {
            return Err(anyhow::anyhow!("API请求失败: {}", response.status()));
        }
        
        let text = response.text().await?;
        let json: serde_json::Value = serde_json::from_str(&text)?;
        
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
        
        let response = self.client
            .get(&url)
            .send()
            .await?;
            
        if !response.status().is_success() {
            return Err(anyhow::anyhow!("价格API请求失败: {}", response.status()));
        }
        
        let prices: HashMap<String, f64> = response.json().await?;
        
        info!("✅ 成功获取 {} 个代币价格", prices.len());
        Ok(prices)
    }
    
    /// 计算池子的储备和流动性
    pub async fn calculate_pool_reserves(
        &self, 
        pool_info: &RaydiumPoolInfo,
        rpc_client: &solana_client::rpc_client::RpcClient
    ) -> Result<(u64, u64, f64)> {
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
        
        let price = if base_ui_amount > 0.0 {
            quote_ui_amount / base_ui_amount
        } else {
            0.0
        };
        
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

/// 计算交换输出的便捷函数
pub async fn calculate_swap_output_with_api(
    pool_address: &str,
    input_amount: u64,
    from_mint: &str,
    to_mint: &str,
    rpc_client: &solana_client::rpc_client::RpcClient,
) -> Result<u64> {
    let api_client = RaydiumApiClient::new();
    
    // 1. 获取池子信息
    let pool_info = match api_client.get_pool_by_address(pool_address).await? {
        Some(info) => info,
        None => return Err(anyhow::anyhow!("未找到池子: {}", pool_address)),
    };
    
    info!("📋 池子信息:");
    info!("  程序ID: {}", pool_info.programId);
    info!("  Base代币: {}", pool_info.baseMint);
    info!("  Quote代币: {}", pool_info.quoteMint);
    info!("  Base精度: {}", pool_info.baseDecimals);
    info!("  Quote精度: {}", pool_info.quoteDecimals);
    
    // 2. 获取实时储备
    let (base_reserve, quote_reserve, current_price) = api_client
        .calculate_pool_reserves(&pool_info, rpc_client)
        .await?;
    
    // 3. 确定交换方向
    let (reserve_in, reserve_out) = if from_mint == pool_info.baseMint {
        (base_reserve, quote_reserve)
    } else if from_mint == pool_info.quoteMint {
        (quote_reserve, base_reserve)
    } else {
        return Err(anyhow::anyhow!("输入代币不匹配池子: {} != {} 或 {}", 
                                   from_mint, pool_info.baseMint, pool_info.quoteMint));
    };
    
    // 4. 恒定乘积公式计算
    let fee_rate = 0.0025; // 0.25% 手续费
    let amount_in_after_fee = (input_amount as f64 * (1.0 - fee_rate)) as u64;
    
    // 防止溢出，使用128位整数
    let numerator = amount_in_after_fee as u128 * reserve_out as u128;
    let denominator = reserve_in as u128 + amount_in_after_fee as u128;
    let output_amount = (numerator / denominator) as u64;
    
    info!("💱 交换计算:");
    info!("  输入储备: {}", reserve_in);
    info!("  输出储备: {}", reserve_out);
    info!("  手续费率: {:.2}%", fee_rate * 100.0);
    info!("  手续费后输入: {}", amount_in_after_fee);
    info!("  计算输出: {}", output_amount);
    info!("  当前价格: {:.6}", current_price);
    
    Ok(output_amount)
} 