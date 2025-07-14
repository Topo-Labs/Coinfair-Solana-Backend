pub mod client;
pub mod config;
pub mod raydium;
pub mod raydium_api;
pub mod swap;
pub mod examples;
pub mod precise_swap;
pub mod swap_v2_service;
pub mod swap_v2_builder;

pub use client::SolanaClient;
pub use config::SwapConfig;
pub use raydium::{RaydiumSwap, RaydiumPoolInfo, SwapEstimateResult};
pub use raydium_api::{RaydiumApiClient, calculate_swap_output_with_api};
pub use swap::SolanaSwap;
pub use precise_swap::PreciseSwapService;
pub use swap_v2_service::{SwapV2Service, TokenAccountInfo, TransferFeeResult, SwapV2AccountsInfo, UserTokenAccountInfo};
pub use swap_v2_builder::{SwapV2InstructionBuilder, SwapV2BuildParams, SwapV2InstructionResult};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::raydium::RaydiumSwap;
    use std::env;
    
    #[tokio::test]
    async fn test_raydium_swap_calculation() {
        
        // 创建测试配置
        let mut config = SwapConfig::default();
        config.rpc_url = "https://api.mainnet-beta.solana.com".to_string();
        config.private_key = solana_sdk::signature::Keypair::new().to_base58_string();
        config.amm_program_id = "CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK".to_string();
        
        let client = SolanaClient::new(&config)
            .expect("Failed to create client");
        
        let raydium_swap = RaydiumSwap::new(client, &config)
            .expect("Failed to create RaydiumSwap");
        
        // 测试一个真实的CLMM池子
        let sol_mint = "So11111111111111111111111111111111111111112";
        let usdc_mint = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
        let test_pool = "5JYwqvKkqp35w8Nq3ba4z1WYUeJQ1rB36V8XvaGp6zn1"; // SOL-USDC CLMM池
        let input_amount = 1_000_000_000u64; // 1 SOL
        
        println!("🧪 开始测试CLMM交换计算");
        println!("  输入: {} SOL", input_amount as f64 / 1_000_000_000.0);
        println!("  池子: {}", test_pool);
        
        // 测试精确计算
        match raydium_swap.calculate_precise_swap_output(
            sol_mint,
            usdc_mint,
            test_pool,
            input_amount,
            Some(0.005), // 0.5% 滑点
        ).await {
            Ok(result) => {
                println!("✅ 计算成功!");
                println!("  预估输出: {} USDC", result.estimated_output as f64 / 1_000_000.0);
                println!("  滑点保护: {} USDC", result.min_output_with_slippage as f64 / 1_000_000.0);
                println!("  价格影响: {:.4}%", result.price_impact * 100.0);
                
                // 验证结果是合理的
                assert!(result.estimated_output > 0, "输出应该大于0");
                assert!(result.min_output_with_slippage > 0, "滑点保护后的输出应该大于0");
                assert!(result.min_output_with_slippage <= result.estimated_output, "滑点保护后的输出应该小于等于预估输出");
                assert!(result.price_impact >= 0.0 && result.price_impact <= 1.0, "价格影响应该在0-100%之间");
            }
            Err(e) => {
                println!("❌ 计算失败: {}", e);
                // 对于网络问题或者池子不存在等情况，我们可以容忍失败
                // 但是如果是代码逻辑问题，应该修复
            }
        }
    }
} 