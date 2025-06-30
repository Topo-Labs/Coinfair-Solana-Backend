/*!
# Raydium CLMM 交换测试示例

这个文件展示了如何使用新的Raydium CLMM交换功能。

运行测试:
```bash
cargo run --bin test_pool
```
*/

use anyhow::Result;
use solana_sdk::signature::{Keypair, Signer};
use std::str::FromStr;

// 引入我们的模块
use crate::solana::raydium::{RaydiumSwap, SwapRequest};
use crate::solana::{SolanaClient, SwapConfig};

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志
    tracing_subscriber::init();
    
    println!("🚀 Raydium CLMM 交换测试开始");
    
    // 1. 创建钱包 (在实际使用中，请使用安全的密钥管理)
    let keypair = Keypair::new(); // 测试用的新钱包
    println!("💰 钱包地址: {}", keypair.pubkey());
    
    // 2. 配置RPC连接
    let rpc_url = "https://api.mainnet-beta.solana.com".to_string();
    
    // 3. 创建Solana客户端
    let client = SolanaClient::new(keypair, rpc_url)?;
    
    // 4. 配置Raydium交换
    let config = SwapConfig {
        amm_program_id: "CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK".to_string(), // Raydium CLMM Program ID
    };
    
    // 5. 创建RaydiumSwap实例
    let raydium_swap = RaydiumSwap::new(client, &config)?;
    
    // === 测试功能 ===
    
    // 测试1: 获取账户余额
    println!("\n📊 测试1: 获取账户余额");
    match raydium_swap.get_account_balances().await {
        Ok((sol_balance, usdc_balance)) => {
            println!("  SOL余额: {} lamports", sol_balance);
            println!("  USDC余额: {} micro USDC", usdc_balance);
        }
        Err(e) => println!("  ⚠️ 获取余额失败: {}", e),
    }
    
    // 测试2: 预估交换输出 (不执行实际交换)
    println!("\n🧮 测试2: 预估交换输出");
    let sol_mint = "So11111111111111111111111111111111111111112";
    let usdc_mint = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
    // 这是一个示例池子地址，在实际使用中需要替换为真实的池子地址
    let example_pool = "PoolAddressHere"; 
    
    let input_amount = 1_000_000_000; // 1 SOL
    
    match raydium_swap.calculate_precise_swap_output(
        sol_mint,
        usdc_mint,
        example_pool,
        input_amount,
        Some(0.005), // 0.5% 滑点
    ).await {
        Ok(estimate) => {
            println!("  ✅ 预估结果:");
            println!("    输入: {} SOL", input_amount as f64 / 1e9);
            println!("    预估输出: {} USDC", estimate.estimated_output as f64 / 1e6);
            println!("    最小输出(含滑点): {} USDC", estimate.min_output_with_slippage as f64 / 1e6);
            println!("    价格影响: {:.4}%", estimate.price_impact * 100.0);
            println!("    交换方向: {}", if estimate.zero_for_one { "SOL → USDC" } else { "USDC → SOL" });
        }
        Err(e) => println!("  ⚠️ 预估失败: {}", e),
    }
    
    // 测试3: 检查代币余额功能
    println!("\n💰 测试3: 检查指定代币余额");
    match raydium_swap.get_token_balance(usdc_mint).await {
        Ok(balance) => println!("  USDC余额: {} micro USDC", balance),
        Err(e) => println!("  ⚠️ 获取USDC余额失败: {}", e),
    }
    
    // 测试4: 获取池子信息 (如果有有效的池子地址)
    println!("\n🏊 测试4: 获取池子信息");
    println!("  注意: 需要有效的池子地址才能获取真实信息");
    
    // 测试5: 批量交换预估
    println!("\n🔄 测试5: 批量交换预估");
    let swap_requests = vec![
        SwapRequest {
            input_mint: sol_mint.to_string(),
            output_mint: usdc_mint.to_string(),
            pool_address: "pool1_address_here".to_string(),
            input_amount: 500_000_000, // 0.5 SOL
        },
        SwapRequest {
            input_mint: usdc_mint.to_string(),
            output_mint: sol_mint.to_string(),
            pool_address: "pool2_address_here".to_string(),
            input_amount: 100_000_000, // 100 USDC
        },
    ];
    
    println!("  准备了 {} 笔交换请求", swap_requests.len());
    for (i, req) in swap_requests.iter().enumerate() {
        println!("    交换 {}: {} -> {} ({})", 
                 i + 1, 
                 req.input_mint[..8].to_string() + "...",
                 req.output_mint[..8].to_string() + "...",
                 req.input_amount);
    }
    
    // 注意: 实际的交换执行需要:
    // 1. 有效的池子地址
    // 2. 钱包中有足够的代币余额
    // 3. 关联代币账户已创建或自动创建
    
    println!("\n✅ 测试完成！");
    println!("\n📝 下一步:");
    println!("1. 获取真实的Raydium CLMM池子地址");
    println!("2. 确保钱包中有足够的代币用于测试");
    println!("3. 在测试网上进行实际交换测试");
    println!("4. 验证交换结果和价格计算的准确性");
    
    Ok(())
}

// 辅助函数：格式化代币金额显示
fn format_token_amount(amount: u64, decimals: u8) -> String {
    let divisor = 10_u64.pow(decimals as u32);
    let whole = amount / divisor;
    let fractional = amount % divisor;
    format!("{}.{:0width$}", whole, fractional, width = decimals as usize)
}

// 测试用的常量
pub const SOL_MINT: &str = "So11111111111111111111111111111111111111112";
pub const USDC_MINT: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";

// 一些知名的代币地址供测试使用
pub const TOKEN_ADDRESSES: &[(&str, &str)] = &[
    ("SOL", "So11111111111111111111111111111111111111112"),
    ("USDC", "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"),
    ("USDT", "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB"),
    ("RAY", "4k3Dyjzvzp8eMZWUXbBCjEvwSkkk59S5iCNLY3QrkX6R"),
    ("mSOL", "mSoLzYCxHdYgdzU16g5QSh3i5K3z3KZK7ytfqcJm7So"),
];

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_format_token_amount() {
        assert_eq!(format_token_amount(1_000_000_000, 9), "1.000000000");
        assert_eq!(format_token_amount(1_500_000_000, 9), "1.500000000");
        assert_eq!(format_token_amount(1_000_000, 6), "1.000000");
    }
    
    #[test]
    fn test_token_addresses() {
        assert!(TOKEN_ADDRESSES.len() > 0);
        for (name, address) in TOKEN_ADDRESSES {
            assert!(!name.is_empty());
            assert!(address.len() > 40); // Solana地址长度检查
        }
    }
} 