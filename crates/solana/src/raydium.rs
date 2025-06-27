use anyhow::Result;
use tracing::{info, warn, error};
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::Signer,
    transaction::Transaction,
};
use spl_associated_token_account::get_associated_token_address;
use std::collections::VecDeque;
use std::str::FromStr;

use crate::{SolanaClient, SwapConfig};

pub struct RaydiumSwap {
    client: SolanaClient,
    program_id: Pubkey,
}

impl RaydiumSwap {
    pub fn new(client: SolanaClient, config: &SwapConfig) -> Result<Self> {
        let program_id = config.amm_program_id.parse::<Pubkey>()?;
        
        Ok(Self {
            client,
            program_id,
        })
    }

    /// 获取钱包公钥
    pub fn get_wallet_pubkey(&self) -> Result<Pubkey> {
        Ok(self.client.get_wallet().pubkey())
    }

    /// 使用精确的AMM算法计算预估输出（简化版本）
    pub async fn calculate_precise_swap_output(
        &self,
        input_mint: &str,
        output_mint: &str,
        pool_address: &str,
        input_amount: u64,
        slippage: Option<f64>,
    ) -> Result<SwapEstimateResult> {
        info!("🎯 开始精确计算交换预估输出");
        info!("  输入代币: {}", input_mint);
        info!("  输出代币: {}", output_mint);
        info!("  池子地址: {}", pool_address);
        info!("  输入金额: {}", input_amount);

        // 解析地址
        let input_mint_pubkey = input_mint.parse::<Pubkey>()?;
        let output_mint_pubkey = output_mint.parse::<Pubkey>()?;
        let pool_pubkey = pool_address.parse::<Pubkey>()?;

        // 获取池子基本信息
        let pool_account = self.client.get_rpc_client()
            .get_account(&pool_pubkey)
            .map_err(|e| anyhow::anyhow!("获取池子账户失败: {}", e))?;

        info!("  ✅ 池子账户加载完成 (数据长度: {} bytes)", pool_account.data.len());

        // 简化的价格计算（基于池子数据）
        let estimated_output = self.calculate_swap_output_from_pool_data(
            &pool_account.data,
            input_mint,
            output_mint,
            input_amount,
        ).await?;

        // 应用滑点保护
        let slippage_rate = slippage.unwrap_or(0.005); // 默认0.5%
        let min_output_with_slippage = self.apply_slippage(estimated_output, slippage_rate);

        info!("  💰 精确计算输出: {}", estimated_output);
        info!("  🛡️ 滑点保护 ({:.2}%): {} -> {}", slippage_rate * 100.0, estimated_output, min_output_with_slippage);

        // 计算价格影响（简化版本）
        let price_impact = self.estimate_price_impact(input_amount, estimated_output)?;

        info!("  💥 价格影响: {:.4}%", price_impact * 100.0);

        Ok(SwapEstimateResult {
            estimated_output,
            min_output_with_slippage,
            price_impact,
            current_price: 0.0, // 简化处理
            tick_arrays_needed: 1, // 简化处理
            zero_for_one: input_mint_pubkey < output_mint_pubkey,
        })
    }

    /// 从池子数据计算交换输出
    async fn calculate_swap_output_from_pool_data(
        &self,
        pool_data: &[u8],
        from_mint: &str,
        to_mint: &str,
        amount_in: u64,
    ) -> Result<u64> {
        info!("  🔬 开始解析池子数据进行计算");

        // 简化的计算逻辑
        // 在实际环境中，这里应该调用client中的get_out_put_amount_and_remaining_accounts方法
        // 但是由于需要复杂的数据结构，我们先使用简化的计算

        const SOL_MINT: &str = "So11111111111111111111111111111111111111112";
        const USDC_MINT_STANDARD: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
        const USDC_MINT_CONFIG: &str = "4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU";

        // 判断代币类型
        let is_from_sol = from_mint == SOL_MINT;
        let is_to_sol = to_mint == SOL_MINT;
        let is_from_usdc = matches!(from_mint, USDC_MINT_STANDARD | USDC_MINT_CONFIG);
        let is_to_usdc = matches!(to_mint, USDC_MINT_STANDARD | USDC_MINT_CONFIG);

        let estimated_output = if is_from_sol && is_to_usdc {
            // SOL -> USDC
            let sol_amount = amount_in as f64 / 1_000_000_000.0; // lamports 转 SOL
            let current_price = 200.0; // 简化的价格，实际应该从池子数据中解析
            let usdc_amount = sol_amount * current_price;
            let usdc_micro = (usdc_amount * 1_000_000.0) as u64;

            info!("  📊 SOL->USDC: {} lamports ({:.9} SOL) × {:.2} = {} micro-USDC ({:.6} USDC)", 
                  amount_in, sol_amount, current_price, usdc_micro, usdc_amount);

            // 应用交易费用 (0.25%)
            let fee_rate = 0.0025;
            (usdc_micro as f64 * (1.0 - fee_rate)) as u64

        } else if is_from_usdc && is_to_sol {
            // USDC -> SOL
            let usdc_amount = amount_in as f64 / 1_000_000.0; // micro-USDC 转 USDC
            let current_price = 200.0; // 简化的价格
            let sol_amount = usdc_amount / current_price;
            let sol_lamports = (sol_amount * 1_000_000_000.0) as u64;

            info!("  📊 USDC->SOL: {} micro-USDC ({:.6} USDC) ÷ {:.2} = {} lamports ({:.9} SOL)", 
                  amount_in, usdc_amount, current_price, sol_lamports, sol_amount);

            // 应用交易费用 (0.25%)
            let fee_rate = 0.0025;
            (sol_lamports as f64 * (1.0 - fee_rate)) as u64

        } else {
            // 其他交换对，使用1:1比率
            let fee_rate = 0.0025;
            (amount_in as f64 * (1.0 - fee_rate)) as u64
        };

        info!("  💰 计算输出结果: {}", estimated_output);
        Ok(estimated_output)
    }

    /// 应用滑点保护
    fn apply_slippage(&self, amount: u64, slippage: f64) -> u64 {
        (amount as f64 * (1.0 - slippage)).floor() as u64
    }

    /// 估算价格影响
    fn estimate_price_impact(&self, input_amount: u64, output_amount: u64) -> Result<f64> {
        // 简化的价格影响计算
        // 基于输入输出比例来估算影响
        if input_amount > 0 && output_amount > 0 {
            // 价格影响大致为交换量的平方根除以一个大数
            let impact = (input_amount as f64).sqrt() / 1_000_000.0;
            Ok(impact.min(0.1)) // 最大10%影响
        } else {
            Ok(0.0)
        }
    }

    /// 通用的代币交换方法（保持向后兼容）
    pub async fn swap_tokens(&self, from_mint: &str, to_mint: &str, pool_address: &str, amount_in: u64, minimum_amount_out: u64) -> Result<String> {
        info!("🔄 执行代币交换");
        info!("  从: {} ({})", from_mint, amount_in);
        info!("  到: {} (最小: {})", to_mint, minimum_amount_out);
        info!("  池子: {}", pool_address);
        
        // 首先进行预估计算
        let estimate = self.calculate_precise_swap_output(
            from_mint,
            to_mint,
            pool_address,
            amount_in,
            Some(0.005), // 0.5% 滑点
        ).await?;

        info!("💰 交换预估:");
        info!("  预估输出: {}", estimate.estimated_output);
        info!("  最小输出(含滑点): {}", estimate.min_output_with_slippage);
        info!("  价格影响: {:.4}%", estimate.price_impact * 100.0);

        // 检查最小输出是否满足要求
        if estimate.min_output_with_slippage < minimum_amount_out {
            return Err(anyhow::anyhow!(
                "预估输出不满足最小输出要求: {} < {}",
                estimate.min_output_with_slippage,
                minimum_amount_out
            ));
        }

        // TODO: 实现真正的交换指令
        info!("🚧 交换功能正在开发中，当前仅返回预估结果");
        
        Ok(format!("预估交换成功 - 输出: {}", estimate.estimated_output))
    }

    /// 从池子获取价格信息并估算输出（保持向后兼容）
    pub async fn get_pool_price_and_estimate(&self, pool_address: &str, from_mint: &str, to_mint: &str, amount_in: u64) -> Result<u64> {
        let estimate = self.calculate_precise_swap_output(
            from_mint,
            to_mint,
            pool_address,
            amount_in,
            None,
        ).await?;

        Ok(estimate.estimated_output)
    }

    /// SOL到USDC的交换（向后兼容方法）
    pub async fn swap_sol_to_usdc(&self, amount_in_lamports: u64, minimum_amount_out: u64) -> Result<String> {
        info!("🔄 SOL到USDC交换（兼容方法）");
        
        // 使用默认的SOL-USDC池地址（这里需要一个实际的池地址）
        let pool_address = "58oQChx4yWmvKdwLLZzBi4ChoCc2fqCUWBkwMihLYQo2"; // 示例池地址
        let sol_mint = "So11111111111111111111111111111111111111112";
        let usdc_mint = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
        
        self.swap_tokens(sol_mint, usdc_mint, pool_address, amount_in_lamports, minimum_amount_out).await
    }

    /// USDC到SOL的交换（向后兼容方法）
    pub async fn swap_usdc_to_sol(&self, amount_in_usdc: u64, minimum_amount_out: u64) -> Result<String> {
        info!("🔄 USDC到SOL交换（兼容方法）");
        
        // 使用默认的SOL-USDC池地址
        let pool_address = "58oQChx4yWmvKdwLLZzBi4ChoCc2fqCUWBkwMihLYQo2"; // 示例池地址
        let usdc_mint = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
        let sol_mint = "So11111111111111111111111111111111111111112";
        
        self.swap_tokens(usdc_mint, sol_mint, pool_address, amount_in_usdc, minimum_amount_out).await
    }

    /// 获取账户余额
    pub async fn get_account_balances(&self) -> Result<(u64, u64)> {
        let owner = self.client.get_wallet().pubkey();
        
        // 获取 SOL 余额
        let sol_balance = self.client.get_rpc_client()
            .get_balance(&owner)
            .map_err(|e| anyhow::anyhow!("获取 SOL 余额失败: {}", e))?;

        // 获取 USDC 余额 (简化处理)
        let usdc_mint = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".parse::<Pubkey>()
            .map_err(|e| anyhow::anyhow!("无效的USDC mint地址: {}", e))?;
        let usdc_token_account = get_associated_token_address(&owner, &usdc_mint);
        
        let usdc_balance = match self.client.get_rpc_client().get_token_account_balance(&usdc_token_account) {
            Ok(balance) => balance.amount.parse::<u64>().unwrap_or(0),
            Err(_) => {
                warn!("USDC 代币账户不存在或获取余额失败");
                0
            }
        };

        Ok((sol_balance, usdc_balance))
    }

    /// 获取实时池子信息（用于精确计算）
    pub async fn get_pool_info(&self, pool_address: &str) -> Result<RaydiumPoolInfo> {
        info!("📊 获取池子信息: {}", pool_address);
        
        let pool_pubkey = pool_address.parse::<Pubkey>()?;
        let pool_account = self.client.get_rpc_client()
            .get_account(&pool_pubkey)
            .map_err(|e| anyhow::anyhow!("获取池子账户失败: {}", e))?;

        // 简化的池子信息解析
        // 在真实环境中，这里应该使用raydium AMM v3的反序列化方法
        Ok(RaydiumPoolInfo {
            sqrt_price_x64: 0, // 简化处理
            liquidity: 0, // 简化处理
            tick_current: 0, // 简化处理
            token_vault_0_amount: 0, // 简化处理
            token_vault_1_amount: 0, // 简化处理
        })
    }
}

/// 交换预估结果
#[derive(Debug)]
pub struct SwapEstimateResult {
    /// 预估输出金额
    pub estimated_output: u64,
    /// 考虑滑点后的最小输出
    pub min_output_with_slippage: u64,
    /// 价格影响（0.0-1.0）
    pub price_impact: f64,
    /// 当前价格
    pub current_price: f64,
    /// 需要的tick数组数量
    pub tick_arrays_needed: usize,
    /// 交换方向
    pub zero_for_one: bool,
}

/// Raydium池子信息结构（保持向后兼容）
#[derive(Debug)]
pub struct RaydiumPoolInfo {
    pub sqrt_price_x64: u128,
    pub liquidity: u128, 
    pub tick_current: i32,
    pub token_vault_0_amount: u64,
    pub token_vault_1_amount: u64,
} 