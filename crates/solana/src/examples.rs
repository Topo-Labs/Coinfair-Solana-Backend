use crate::{SolanaSwap, SwapConfig};
use anyhow::Result;
use tracing::info;

use crate::{PreciseSwapService, SolanaClient};

/// 基本的 SOL 到 USDC 交换示例（演示版本）
pub async fn example_swap_sol_to_usdc() -> Result<()> {
    info!("🚀 开始 SOL 到 USDC 交换演示");

    // 配置Solana交换参数
    let config = SwapConfig::default();

    // ⚠️ 重要：在实际使用中，你需要设置你的私钥
    // config.private_key = "你的Base58编码的私钥".to_string();

    // 如果使用测试网，可以更改RPC URL
    // config.rpc_url = "https://api.devnet.solana.com".to_string();

    info!("⚠️ 注意：这是演示模式，不会执行真实的代币交换");
    info!("要启用真实交换，请:");
    info!("1. 设置环境变量 SOLANA_PRIVATE_KEY");
    info!("2. 确保有足够的SOL余额");
    info!("3. 将代码中的demo指令替换为真实的Raydium AMM指令");

    // 如果没有私钥，跳过实际的区块链操作
    if config.private_key.is_empty() {
        info!("📝 私钥未设置，跳过实际交换演示");

        // 演示价格计算
        let mock_swap = SolanaSwap::new(config)?;
        let amount_in = 100_000_000; // 0.1 SOL
        let estimated_output = mock_swap.calculate_swap_output(amount_in, true)?;
        info!(
            "💰 模拟计算：{} lamports SOL -> {} micro-USDC",
            amount_in, estimated_output
        );

        return Ok(());
    }

    // 创建交换实例
    let swap = SolanaSwap::new(config)?;

    // 检查账户余额
    let (sol_balance, usdc_balance) = swap.get_account_balances().await?;
    info!(
        "当前 SOL 余额: {} lamports ({:.4} SOL)",
        sol_balance,
        sol_balance as f64 / 1_000_000_000.0
    );
    info!(
        "当前 USDC 余额: {} ({:.2} USDC)",
        usdc_balance,
        usdc_balance as f64 / 1_000_000.0
    );

    // 交换 0.01 SOL 到 USDC（较小金额用于演示）
    let amount_in = 10_000_000; // 0.01 SOL (以 lamports 为单位)
    let minimum_amount_out = 0; // 最小输出量

    // 计算预期输出
    let estimated_output = swap.calculate_swap_output(amount_in, true)?;
    info!(
        "📊 预期输出: {} micro-USDC ({:.6} USDC)",
        estimated_output,
        estimated_output as f64 / 1_000_000.0
    );

    match swap.swap_sol_to_usdc(amount_in, minimum_amount_out).await {
        Ok(signature) => {
            info!("✅ 演示交易成功!");
            info!("📋 交易签名: {}", signature);
            info!(
                "🔗 在 Solana Explorer 查看: https://explorer.solana.com/tx/{}",
                signature
            );
        }
        Err(e) => {
            info!("❌ 交换失败: {:?}", e);
        }
    }

    Ok(())
}

/// 基本的 USDC 到 SOL 交换示例
pub async fn example_swap_usdc_to_sol() -> Result<()> {
    // 配置Solana交换参数
    let config = SwapConfig::default();

    // ⚠️ 重要：在实际使用中，你需要设置你的私钥
    // config.private_key = "你的Base58编码的私钥".to_string();

    // 创建交换实例
    let swap = SolanaSwap::new(config)?;

    // 检查账户余额
    let (sol_balance, usdc_balance) = swap.get_account_balances().await?;
    info!("当前 SOL 余额: {} lamports", sol_balance);
    info!("当前 USDC 余额: {}", usdc_balance);

    // 交换 10 USDC 到 SOL
    let amount_in = 10_000_000; // 10 USDC (以微单位为单位，1 USDC = 1,000,000 microUSDC)
    let minimum_amount_out = 0; // 最小输出量

    match swap.swap_usdc_to_sol(amount_in, minimum_amount_out).await {
        Ok(signature) => {
            info!("✅ USDC 到 SOL 交换成功!");
            info!("交易签名: {}", signature);
        }
        Err(e) => {
            info!("❌ 交换失败: {:?}", e);
        }
    }

    Ok(())
}

/// 自定义配置示例
pub async fn example_custom_config() -> Result<()> {
    let config = SwapConfig {
        rpc_url: "https://api.mainnet-beta.solana.com".to_string(),
        private_key: "你的私钥".to_string(), // ⚠️ 请使用你的实际私钥
        amm_program_id: "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8".to_string(),
        openbook_program_id: "srmqPvymJeFKQ4zGQed1GFppgkRHL9kaELCbyksJtPX".to_string(),
        usdc_mint: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(),
        sol_usdc_pool_id: "58oQChx4yWmvKdwLLZzBi4ChoCc2fqCUWBkwMihLYQo2".to_string(),
    };

    let swap = SolanaSwap::new(config)?;

    // 获取余额信息
    let (sol_balance, usdc_balance) = swap.get_account_balances().await?;

    info!("配置完成，当前余额:");
    info!(
        "SOL: {} lamports ({} SOL)",
        sol_balance,
        sol_balance as f64 / 1_000_000_000.0
    );
    info!("USDC: {} ({} USDC)", usdc_balance, usdc_balance as f64 / 1_000_000.0);

    Ok(())
}

/// 演示如何使用 PreciseSwapService 计算精确的交换输出
pub async fn demonstrate_precise_swap_calculation() -> Result<()> {
    info!("🚀 演示精确交换计算服务");

    // 初始化配置
    let config = SwapConfig::default();
    let client = SolanaClient::new(&config)?;
    let precise_swap_service = PreciseSwapService::new(client, &config)?;

    // 示例1：计算1 SOL到USDC的预估输出
    info!("\n📊 示例1: 计算1 SOL -> USDC");
    let sol_mint = "So11111111111111111111111111111111111111112";
    let usdc_mint = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
    let pool_address = "58oQChx4yWmvKdwLLZzBi4ChoCc2fqCUWBkwMihLYQo2"; // 示例池地址
    let sol_amount = 1_000_000_000u64; // 1 SOL

    match precise_swap_service
        .calculate_exact_swap_output(
            sol_mint,
            usdc_mint,
            pool_address,
            sol_amount,
            Some(0.005), // 0.5% 滑点
        )
        .await
    {
        Ok(result) => {
            info!("✅ 计算成功!");
            info!("  预估输出: {} USDC (micro units)", result.estimated_output);
            info!("  预估输出: {:.6} USDC", result.estimated_output as f64 / 1_000_000.0);
            info!(
                "  最小输出(含滑点): {} USDC (micro units)",
                result.min_output_with_slippage
            );
            info!("  价格影响: {:.4}%", result.price_impact * 100.0);
            info!("  滑点率: {:.2}%", result.slippage_rate * 100.0);
            info!("  使用tick数组: {}", result.tick_arrays_used);
            info!(
                "  交换方向: {}",
                if result.zero_for_one {
                    "Token0 -> Token1"
                } else {
                    "Token1 -> Token0"
                }
            );
        }
        Err(e) => {
            info!("❌ 计算失败: {:?}", e);
        }
    }

    // 示例2：使用便捷方法计算1 SOL输出
    info!("\n📊 示例2: 使用便捷方法计算1 SOL输出");
    match precise_swap_service
        .estimate_1_sol_output(pool_address, usdc_mint)
        .await
    {
        Ok(output) => {
            info!("✅ 1 SOL 预估输出: {} USDC (micro units)", output);
            info!("✅ 1 SOL 预估输出: {:.6} USDC", output as f64 / 1_000_000.0);
        }
        Err(e) => {
            info!("❌ 计算失败: {:?}", e);
        }
    }

    // 示例3：不同金额的计算对比
    info!("\n📊 示例3: 不同金额的计算对比");
    let test_amounts = vec![
        500_000_000u64,    // 0.5 SOL
        1_000_000_000u64,  // 1 SOL
        5_000_000_000u64,  // 5 SOL
        10_000_000_000u64, // 10 SOL
    ];

    for amount in test_amounts {
        let sol_amount = amount as f64 / 1_000_000_000.0;
        info!("  计算 {:.1} SOL 的输出...", sol_amount);

        match precise_swap_service
            .calculate_exact_swap_output(sol_mint, usdc_mint, pool_address, amount, Some(0.005))
            .await
        {
            Ok(result) => {
                let usdc_output = result.estimated_output as f64 / 1_000_000.0;
                let price_per_sol = usdc_output / sol_amount;
                info!(
                    "    输出: {:.6} USDC (价格: {:.2} USDC/SOL, 影响: {:.4}%)",
                    usdc_output,
                    price_per_sol,
                    result.price_impact * 100.0
                );
            }
            Err(e) => {
                info!("    计算失败: {:?}", e);
            }
        }
    }

    info!("\n🎉 精确交换计算演示完成!");
    Ok(())
}

/// 演示client工具方法的正确使用流程
pub async fn demonstrate_client_utils_integration() -> Result<()> {
    info!("演示client工具方法集成");

    info!("使用client中的get_out_put_amount_and_remaining_accounts方法的步骤:");
    info!("  1. 加载池子账户数据");
    info!("  2. 反序列化为PoolState结构");
    info!("  3. 加载AMM配置账户");
    info!("  4. 反序列化为AmmConfig结构");
    info!("  5. 加载tick数组位图扩展");
    info!("  6. 反序列化为TickArrayBitmapExtension结构");
    info!("  7. 确定交换方向 (zero_for_one)");
    info!("  8. 加载所需的tick数组账户");
    info!("  9. 反序列化为TickArrayState结构");
    info!("  10. 调用get_out_put_amount_and_remaining_accounts方法");

    info!("\n💡 关键代码示例:");
    info!("```rust");
    info!("use client::instructions::utils::{{");
    info!("    get_out_put_amount_and_remaining_accounts,");
    info!("    deserialize_anchor_account,");
    info!("    amount_with_slippage,");
    info!("}};");
    info!("");
    info!("// 调用精确计算方法");
    info!("let (output_amount, tick_array_indexes) = get_out_put_amount_and_remaining_accounts(");
    info!("    input_amount,        // 输入金额");
    info!("    None,               // 价格限制 (可选)");
    info!("    zero_for_one,       // 交换方向");
    info!("    true,               // is_base_input");
    info!("    &amm_config,        // AMM配置");
    info!("    &pool_state,        // 池状态");
    info!("    &tick_bitmap,       // tick数组位图");
    info!("    &mut tick_arrays,   // tick数组队列");
    info!(")?;");
    info!("");
    info!("// 应用滑点保护");
    info!("let min_output = amount_with_slippage(output_amount, 0.005, false);");
    info!("```");

    info!("\n🚀 在完整实现中，PreciseSwapService会调用上述方法进行精确计算");

    Ok(())
}

/// 完整的使用示例
pub async fn example_calculate_1_sol_swap() -> Result<()> {
    info!("💰 完整示例：计算1 SOL在指定池子中的预估输出");

    // 步骤1：初始化服务
    let config = SwapConfig::default();
    let client = SolanaClient::new(&config)?;
    let precise_swap_service = PreciseSwapService::new(client, &config)?;

    // 步骤2：设置参数
    let sol_mint = "So11111111111111111111111111111111111111112";
    let usdc_mint = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
    let pool_address = "你的实际池子地址"; // 替换为真实池子地址
    let input_amount = 1_000_000_000u64; // 1 SOL
    let slippage = 0.005; // 0.5%

    info!("参数设置:");
    info!("  输入代币: SOL ({})", sol_mint);
    info!("  输出代币: USDC ({})", usdc_mint);
    info!("  池子地址: {}", pool_address);
    info!("  输入金额: {} lamports (1 SOL)", input_amount);
    info!("  滑点设置: {:.2}%", slippage * 100.0);

    // 步骤3：计算预估输出
    match precise_swap_service
        .calculate_exact_swap_output(sol_mint, usdc_mint, pool_address, input_amount, Some(slippage))
        .await
    {
        Ok(result) => {
            info!("\n✅ 计算完成!");
            info!("📊 结果详情:");
            info!("  预估输出: {} micro-USDC", result.estimated_output);
            info!("  预估输出: {:.6} USDC", result.estimated_output as f64 / 1_000_000.0);
            info!(
                "  最小输出: {} micro-USDC (含{:.2}%滑点)",
                result.min_output_with_slippage,
                result.slippage_rate * 100.0
            );
            info!(
                "  最小输出: {:.6} USDC",
                result.min_output_with_slippage as f64 / 1_000_000.0
            );
            info!("  价格影响: {:.4}%", result.price_impact * 100.0);
            info!(
                "  隐含价格: {:.2} USDC/SOL",
                result.estimated_output as f64 / 1_000_000.0
            );
            info!("  tick数组使用: {}", result.tick_arrays_used);

            // 步骤4：风险评估
            if result.price_impact > 0.01 {
                info!(
                    "⚠️ 警告：价格影响较大 (>{:.2}%), 请谨慎交易",
                    result.price_impact * 100.0
                );
            } else {
                info!("✅ 价格影响在合理范围内");
            }

            Ok(())
        }
        Err(e) => {
            info!("❌ 计算失败: {:?}", e);
            info!("💡 可能的原因:");
            info!("  - 池子地址不正确");
            info!("  - 网络连接问题");
            info!("  - RPC节点限流");
            info!("  - 池子数据格式不匹配");

            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_config_creation() {
        let config = SwapConfig::default();
        assert!(!config.rpc_url.is_empty());
        assert!(!config.amm_program_id.is_empty());
    }
}
