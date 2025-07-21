use crate::{client::SolanaClient, config::SwapConfig};
use anyhow::Result;
use solana_sdk::{
    instruction::Instruction,
    pubkey::Pubkey,
    signature::{Signature, Signer},
    transaction::Transaction,
};
use spl_associated_token_account::get_associated_token_address;
use tracing::{error, info, warn};

pub struct SolanaSwap {
    client: SolanaClient,
    config: SwapConfig,
}

impl SolanaSwap {
    pub fn new(config: SwapConfig) -> Result<Self> {
        let client = SolanaClient::new(&config)?;
        Ok(Self { client, config })
    }

    /// 获取账户余额信息
    pub async fn get_account_balances(&self) -> Result<(u64, u64)> {
        let owner = self.client.get_wallet().pubkey();

        // 获取 SOL 余额
        let sol_balance = self
            .client
            .get_rpc_client()
            .get_balance(&owner)
            .map_err(|e| anyhow::anyhow!("获取 SOL 余额失败: {}", e))?;

        // 获取 USDC 余额
        let usdc_mint = self.config.get_usdc_mint()?;
        let usdc_token_account = get_associated_token_address(&owner, &usdc_mint);

        let usdc_balance = match self.client.get_rpc_client().get_token_account_balance(&usdc_token_account) {
            Ok(balance) => balance.amount.parse::<u64>().unwrap_or(0),
            Err(_) => {
                warn!("USDC 代币账户不存在或获取余额失败");
                0
            }
        };

        info!("SOL 余额: {} lamports ({} SOL)", sol_balance, sol_balance as f64 / 1_000_000_000.0);
        info!("USDC 余额: {} ({} USDC)", usdc_balance, usdc_balance as f64 / 1_000_000.0);

        Ok((sol_balance, usdc_balance))
    }

    /// 创建基本的swap指令 - 简化版本
    /// 注意：这是一个简化的实现，实际的Raydium AMM需要更复杂的逻辑
    pub async fn create_simple_swap_instruction(&self, input_mint: &Pubkey, output_mint: &Pubkey, amount: u64, minimum_amount_out: u64) -> Result<Instruction> {
        let owner = self.client.get_wallet().pubkey();

        // 获取用户的代币账户
        let input_token_account = get_associated_token_address(&owner, input_mint);
        let output_token_account = get_associated_token_address(&owner, output_mint);

        info!("输入代币账户: {}", input_token_account);
        info!("输出代币账户: {}", output_token_account);
        info!("交换金额: {}", amount);
        info!("最小输出: {}", minimum_amount_out);

        // 这里返回一个简单的memo指令作为示例
        // 在实际实现中，这里应该是Raydium的swap指令
        let memo_instruction = spl_memo::build_memo(format!("Swap {} {} to {}", amount, input_mint, output_mint).as_bytes(), &[&owner]);

        Ok(memo_instruction)
    }

    /// 执行 SOL 到 USDC 的交换（简化版本）
    pub async fn swap_sol_to_usdc(&self, amount_in_lamports: u64, minimum_amount_out: u64) -> Result<Signature> {
        info!("开始执行 SOL 到 USDC 的交换");
        info!(
            "输入金额: {} lamports ({} SOL)",
            amount_in_lamports,
            amount_in_lamports as f64 / 1_000_000_000.0
        );

        let sol_mint = spl_token::native_mint::id();
        let usdc_mint = self.config.get_usdc_mint()?;

        // 检查余额
        let (sol_balance, _) = self.get_account_balances().await?;
        if sol_balance < amount_in_lamports {
            return Err(anyhow::anyhow!("SOL余额不足"));
        }

        // 创建交换指令
        let swap_instruction = self
            .create_simple_swap_instruction(&sol_mint, &usdc_mint, amount_in_lamports, minimum_amount_out)
            .await?;

        // 构建交易
        let recent_blockhash = self.client.get_latest_blockhash()?;
        let owner = self.client.get_wallet().pubkey();

        let transaction = Transaction::new_signed_with_payer(&[swap_instruction], Some(&owner), &[self.client.get_wallet()], recent_blockhash);

        // 发送交易
        info!("📝 注意: 这是一个演示版本的交换，实际并不执行真实的代币交换");

        // 为了演示目的，我们只发送memo交易
        match self.client.send_transaction(&transaction).await {
            Ok(signature) => {
                info!("✅ 演示交易成功发送! 签名: {}", signature);
                Ok(signature)
            }
            Err(e) => {
                error!("❌ 交易失败: {:?}", e);
                Err(e)
            }
        }
    }

    /// 执行 USDC 到 SOL 的交换（简化版本）
    pub async fn swap_usdc_to_sol(&self, amount_in_usdc: u64, minimum_amount_out: u64) -> Result<Signature> {
        info!("开始执行 USDC 到 SOL 的交换");
        info!("输入金额: {} USDC", amount_in_usdc as f64 / 1_000_000.0);

        let usdc_mint = self.config.get_usdc_mint()?;
        let sol_mint = spl_token::native_mint::id();

        // 检查余额
        let (_, usdc_balance) = self.get_account_balances().await?;
        if usdc_balance < amount_in_usdc {
            return Err(anyhow::anyhow!("USDC余额不足"));
        }

        // 创建交换指令
        let swap_instruction = self
            .create_simple_swap_instruction(&usdc_mint, &sol_mint, amount_in_usdc, minimum_amount_out)
            .await?;

        // 构建交易
        let recent_blockhash = self.client.get_latest_blockhash()?;
        let owner = self.client.get_wallet().pubkey();

        let transaction = Transaction::new_signed_with_payer(&[swap_instruction], Some(&owner), &[self.client.get_wallet()], recent_blockhash);

        // 发送交易
        info!("📝 注意: 这是一个演示版本的交换，实际并不执行真实的代币交换");

        // 为了演示目的，我们只发送memo交易
        match self.client.send_transaction(&transaction).await {
            Ok(signature) => {
                info!("✅ 演示交易成功发送! 签名: {}", signature);
                Ok(signature)
            }
            Err(e) => {
                error!("❌ 交易失败: {:?}", e);
                Err(e)
            }
        }
    }

    /// 模拟价格计算（简化版本）
    pub fn calculate_swap_output(&self, input_amount: u64, is_sol_to_usdc: bool) -> Result<u64> {
        // 这里使用一个固定的模拟汇率进行计算
        // 在实际实现中，这应该从AMM池中获取真实价格

        let mock_sol_price = 100.0; // 假设1 SOL = 100 USDC

        let output_amount = if is_sol_to_usdc {
            // SOL to USDC
            let sol_amount = input_amount as f64 / 1_000_000_000.0; // 转换为SOL
            let usdc_amount = sol_amount * mock_sol_price;
            (usdc_amount * 1_000_000.0) as u64 // 转换为USDC micro units
        } else {
            // USDC to SOL
            let usdc_amount = input_amount as f64 / 1_000_000.0; // 转换为USDC
            let sol_amount = usdc_amount / mock_sol_price;
            (sol_amount * 1_000_000_000.0) as u64 // 转换为lamports
        };

        info!("💰 模拟价格计算:");
        info!("   输入: {}", input_amount);
        info!("   输出: {}", output_amount);
        info!("   方向: {}", if is_sol_to_usdc { "SOL -> USDC" } else { "USDC -> SOL" });

        Ok(output_amount)
    }

    /// 获取当前配置
    pub fn get_config(&self) -> &SwapConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_price_calculation() {
        let config = SwapConfig::default();
        let swap = SolanaSwap::new(config).unwrap();

        // 测试价格计算
        let sol_amount = 100_000_000; // 0.1 SOL
        let usdc_output = swap.calculate_swap_output(sol_amount, true).unwrap();
        assert!(usdc_output > 0);

        let usdc_amount = 10_000_000; // 10 USDC
        let sol_output = swap.calculate_swap_output(usdc_amount, false).unwrap();
        assert!(sol_output > 0);
    }
}
