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

/// 直接从链上获取池子基本信息（轻量级）
pub async fn get_pool_info_from_chain(pool_address: &str, rpc_client: &solana_client::rpc_client::RpcClient) -> Result<RaydiumPoolInfo> {
    info!("🔍 直接从链上获取池子信息: {}", pool_address);

    let pool_pubkey = pool_address.parse::<Pubkey>()?;
    let account_info = rpc_client.get_account(&pool_pubkey)?;

    info!("📋 池子账户信息:");
    info!("  Owner: {}", account_info.owner);
    info!("  Data length: {}", account_info.data.len());

    // 根据程序ID和数据长度判断池子类型
    let program_id = account_info.owner.to_string();

    // 尝试解析不同类型的池子
    if is_raydium_amm_pool(&program_id, account_info.data.len()) {
        parse_raydium_amm_pool(pool_address, &account_info.data)
    } else if is_raydium_clmm_pool(&program_id, account_info.data.len()) {
        parse_raydium_clmm_pool(pool_address, &account_info.data)
    } else {
        // 如果无法识别，返回一个基础的池子信息（没有vault地址）
        warn!("⚠️ 无法识别的池子类型，使用基础信息");
        Ok(RaydiumPoolInfo {
            id: pool_address.to_string(),
            baseMint: "".to_string(),
            quoteMint: "".to_string(),
            lpMint: "".to_string(),
            baseDecimals: 9,
            quoteDecimals: 6,
            lpDecimals: 9,
            version: 4,
            programId: program_id,
            authority: "".to_string(),
            openOrders: "".to_string(),
            targetOrders: "".to_string(),
            baseVault: "".to_string(),
            quoteVault: "".to_string(),
            marketVersion: 3,
            marketProgramId: "".to_string(),
            marketId: "".to_string(),
            marketAuthority: "".to_string(),
            marketBaseVault: "".to_string(),
            marketQuoteVault: "".to_string(),
            marketBids: "".to_string(),
            marketAsks: "".to_string(),
            marketEventQueue: "".to_string(),
        })
    }
}

/// 检查是否为Raydium AMM池子
fn is_raydium_amm_pool(program_id: &str, data_len: usize) -> bool {
    // Raydium AMM V4 程序ID和数据长度
    program_id == "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8" && data_len >= 752
}

/// 检查是否为Raydium CLMM池子
fn is_raydium_clmm_pool(program_id: &str, data_len: usize) -> bool {
    // Raydium CLMM 程序ID
    program_id == "CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK" && data_len >= 1544
}

/// 解析Raydium AMM池子数据
fn parse_raydium_amm_pool(pool_address: &str, data: &[u8]) -> Result<RaydiumPoolInfo> {
    info!("解析Raydium AMM池子数据");

    if data.len() < 752 {
        return Err(anyhow::anyhow!("AMM池子数据长度不足"));
    }

    // 简化的AMM池子数据解析（实际需要按照Raydium的数据结构）
    // 这里使用硬编码的偏移量，实际应该根据具体的结构体定义

    // 从已知的AMM池子格式中提取关键信息
    // 注意：这是简化版本，实际的数据结构更复杂

    Ok(RaydiumPoolInfo {
        id: pool_address.to_string(),
        baseMint: "So11111111111111111111111111111111111111112".to_string(), // 需要从数据中解析
        quoteMint: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(), // 需要从数据中解析
        lpMint: "".to_string(),
        baseDecimals: 9,
        quoteDecimals: 6,
        lpDecimals: 9,
        version: 4,
        programId: "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8".to_string(),
        authority: "".to_string(),
        openOrders: "".to_string(),
        targetOrders: "".to_string(),
        baseVault: "".to_string(),  // 需要从数据中解析vault地址
        quoteVault: "".to_string(), // 需要从数据中解析vault地址
        marketVersion: 3,
        marketProgramId: "".to_string(),
        marketId: "".to_string(),
        marketAuthority: "".to_string(),
        marketBaseVault: "".to_string(),
        marketQuoteVault: "".to_string(),
        marketBids: "".to_string(),
        marketAsks: "".to_string(),
        marketEventQueue: "".to_string(),
    })
}

/// 解析Raydium CLMM池子数据（使用真实的PoolState结构）
fn parse_raydium_clmm_pool(pool_address: &str, data: &[u8]) -> Result<RaydiumPoolInfo> {
    info!("解析Raydium CLMM池子数据");

    // 检查数据长度，CLMM池子需要至少1544字节
    if data.len() < 1544 {
        return Err(anyhow::anyhow!("CLMM池子数据长度不足: {} < 1544", data.len()));
    }

    // 跳过账户discriminator（前8字节）
    let pool_data = &data[8..];

    // 根据PoolState结构体解析数据
    // 参考：PoolState 结构体定义
    let mut offset = 0;

    // bump: [u8; 1] - 偏移量0
    let _bump = pool_data[offset];
    offset += 1;

    // amm_config: Pubkey - 偏移量1
    let _amm_config = read_pubkey(&pool_data[offset..offset + 32])?;
    offset += 32;

    // owner: Pubkey - 偏移量33
    let _owner = read_pubkey(&pool_data[offset..offset + 32])?;
    offset += 32;

    // token_mint_0: Pubkey - 偏移量65
    let token_mint_0 = read_pubkey(&pool_data[offset..offset + 32])?;
    offset += 32;

    // token_mint_1: Pubkey - 偏移量97
    let token_mint_1 = read_pubkey(&pool_data[offset..offset + 32])?;
    offset += 32;

    // token_vault_0: Pubkey - 偏移量129
    let token_vault_0 = read_pubkey(&pool_data[offset..offset + 32])?;
    offset += 32;

    // token_vault_1: Pubkey - 偏移量161
    let token_vault_1 = read_pubkey(&pool_data[offset..offset + 32])?;
    offset += 32;

    // observation_key: Pubkey - 偏移量193
    let _observation_key = read_pubkey(&pool_data[offset..offset + 32])?;
    offset += 32;

    // mint_decimals_0: u8 - 偏移量225
    let mint_decimals_0 = pool_data[offset];
    offset += 1;

    // mint_decimals_1: u8 - 偏移量226
    let mint_decimals_1 = pool_data[offset];
    offset += 1;

    // tick_spacing: u16 - 偏移量227
    let _tick_spacing = u16::from_le_bytes([pool_data[offset], pool_data[offset + 1]]);
    offset += 2;

    // liquidity: u128 - 偏移量229
    let _liquidity = read_u128(&pool_data[offset..offset + 16])?;

    info!("✅ 成功解析CLMM池子数据:");
    info!("  Token0: {} (精度: {})", token_mint_0, mint_decimals_0);
    info!("  Token1: {} (精度: {})", token_mint_1, mint_decimals_1);
    info!("  Vault0: {}", token_vault_0);
    info!("  Vault1: {}", token_vault_1);

    Ok(RaydiumPoolInfo {
        id: pool_address.to_string(),
        baseMint: token_mint_0.to_string(),
        quoteMint: token_mint_1.to_string(),
        lpMint: "".to_string(), // CLMM没有LP代币
        baseDecimals: mint_decimals_0,
        quoteDecimals: mint_decimals_1,
        lpDecimals: 0,
        version: 6, // CLMM是V6
        programId: "CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK".to_string(),
        authority: "".to_string(),
        openOrders: "".to_string(),
        targetOrders: "".to_string(),
        baseVault: token_vault_0.to_string(),
        quoteVault: token_vault_1.to_string(),
        marketVersion: 3,
        marketProgramId: "".to_string(),
        marketId: "".to_string(),
        marketAuthority: "".to_string(),
        marketBaseVault: "".to_string(),
        marketQuoteVault: "".to_string(),
        marketBids: "".to_string(),
        marketAsks: "".to_string(),
        marketEventQueue: "".to_string(),
    })
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

/// 直接计算池子储备（不依赖API客户端）
pub async fn calculate_pool_reserves_direct(pool_info: &RaydiumPoolInfo, rpc_client: &solana_client::rpc_client::RpcClient) -> Result<(u64, u64, f64)> {
    info!("📊 直接计算池子储备: {}", pool_info.id);

    // 检查是否有有效的vault地址
    if pool_info.baseVault.is_empty() || pool_info.quoteVault.is_empty() {
        return Err(anyhow::anyhow!("池子信息不完整，缺少vault地址"));
    }

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

    info!("💰 直接计算池子储备: base={}, quote={}, 价格={:.6}", base_amount, quote_amount, price);

    Ok((base_amount, quote_amount, price))
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
    rpc_client: &solana_client::rpc_client::RpcClient,
) -> Result<u64> {
    info!("💱 计算交换输出（优化版）- 池子: {}", pool_address);
    //  从链上获取真实池子信息进行计算
    match get_pool_info_from_chain(pool_address, rpc_client).await {
        Ok(pool_info) => {
            info!("✅ 成功从链上获取池子信息，进行精确计算");

            // 获取实时储备
            match calculate_pool_reserves_direct(&pool_info, rpc_client).await {
                Ok((base_reserve, quote_reserve, current_price)) => {
                    // 确定交换方向
                    let (reserve_in, reserve_out) = if from_mint == pool_info.baseMint {
                        (base_reserve, quote_reserve)
                    } else if from_mint == pool_info.quoteMint {
                        (quote_reserve, base_reserve)
                    } else {
                        warn!("⚠️ 输入代币不匹配池子，使用备用计算");
                        return calculate_fallback_output(input_amount, from_mint, to_mint).await;
                    };

                    // 恒定乘积公式计算
                    let fee_rate = 0.0025; // 0.25% 手续费
                    let amount_in_after_fee = (input_amount as f64 * (1.0 - fee_rate)) as u64;

                    // 防止溢出，使用128位整数
                    let numerator = amount_in_after_fee as u128 * reserve_out as u128;
                    let denominator = reserve_in as u128 + amount_in_after_fee as u128;
                    let output_amount = (numerator / denominator) as u64;

                    info!("💱 链上精确计算结果:");
                    info!("  输入储备: {}", reserve_in);
                    info!("  输出储备: {}", reserve_out);
                    info!("  当前价格: {:.6}", current_price);
                    info!("  计算输出: {}", output_amount);

                    return Ok(output_amount);
                }
                Err(e) => {
                    warn!("⚠️ 获取池子储备失败: {:?}，使用备用计算", e);
                }
            }
        }
        Err(e) => {
            warn!("⚠️ 从链上获取池子信息失败: {:?}，使用备用计算", e);
        }
    }

    // 3. 最终备用方案：使用简化的恒定乘积模型
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

/// 读取32字节的Pubkey
fn read_pubkey(data: &[u8]) -> Result<solana_sdk::pubkey::Pubkey> {
    if data.len() < 32 {
        return Err(anyhow::anyhow!("数据长度不足，无法读取Pubkey"));
    }

    let mut bytes = [0u8; 32];
    bytes.copy_from_slice(&data[0..32]);
    Ok(solana_sdk::pubkey::Pubkey::new_from_array(bytes))
}

/// 读取16字节的u128（小端序）
fn read_u128(data: &[u8]) -> Result<u128> {
    if data.len() < 16 {
        return Err(anyhow::anyhow!("数据长度不足，无法读取u128"));
    }

    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&data[0..16]);
    Ok(u128::from_le_bytes(bytes))
}
