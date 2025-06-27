use anyhow::Result;
use solana::{SolanaSwap, SwapConfig};
use std::sync::Arc;
use tracing::{info, error, warn};
use tokio::time::{sleep, Duration};

pub struct SolanaService {
    swap: Option<SolanaSwap>,
    config: SwapConfig,
}

impl SolanaService {
    pub fn new() -> Self {
        let config = SwapConfig::default();
        
        Self {
            swap: None,
            config,
        }
    }

    pub fn with_config(config: SwapConfig) -> Self {
        Self {
            swap: None,
            config,
        }
    }

    /// 初始化Solana交换服务
    pub async fn initialize(&mut self) -> Result<()> {
        if self.config.private_key.is_empty() {
            warn!("⚠️ Solana私钥未配置，跳过Solana服务初始化");
            return Ok(());
        }

        match SolanaSwap::new(self.config.clone()) {
            Ok(swap) => {
                info!("✅ Solana交换服务初始化成功");
                
                // 检查账户余额
                match swap.get_account_balances().await {
                    Ok((sol_balance, usdc_balance)) => {
                        info!("💰 当前账户余额:");
                        info!("   SOL: {} lamports ({:.4} SOL)", sol_balance, sol_balance as f64 / 1_000_000_000.0);
                        info!("   USDC: {} ({:.2} USDC)", usdc_balance, usdc_balance as f64 / 1_000_000.0);
                    }
                    Err(e) => {
                        warn!("⚠️ 获取账户余额失败: {:?}", e);
                    }
                }

                self.swap = Some(swap);
                Ok(())
            }
            Err(e) => {
                error!("❌ Solana交换服务初始化失败: {:?}", e);
                Err(e)
            }
        }
    }

    /// 执行SOL到USDC的交换
    pub async fn swap_sol_to_usdc(&self, amount_sol: f64, minimum_slippage: f64) -> Result<String> {
        let swap = self.swap.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Solana服务未初始化"))?;

        let amount_in_lamports = (amount_sol * 1_000_000_000.0) as u64;
        let minimum_amount_out = (amount_in_lamports as f64 * (1.0 - minimum_slippage)) as u64;

        info!("🔄 开始执行SOL到USDC交换");
        info!("   输入: {} SOL ({} lamports)", amount_sol, amount_in_lamports);
        info!("   最大滑点: {}%", minimum_slippage * 100.0);

        let signature = swap.swap_sol_to_usdc(amount_in_lamports, minimum_amount_out).await?;
        
        info!("✅ SOL到USDC交换完成，交易签名: {}", signature);
        Ok(signature.to_string())
    }

    /// 执行USDC到SOL的交换
    pub async fn swap_usdc_to_sol(&self, amount_usdc: f64, minimum_slippage: f64) -> Result<String> {
        let swap = self.swap.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Solana服务未初始化"))?;

        let amount_in_usdc = (amount_usdc * 1_000_000.0) as u64;
        let minimum_amount_out = (amount_in_usdc as f64 * (1.0 - minimum_slippage)) as u64;

        info!("🔄 开始执行USDC到SOL交换");
        info!("   输入: {} USDC ({} micro-USDC)", amount_usdc, amount_in_usdc);
        info!("   最大滑点: {}%", minimum_slippage * 100.0);

        let signature = swap.swap_usdc_to_sol(amount_in_usdc, minimum_amount_out).await?;
        
        info!("✅ USDC到SOL交换完成，交易签名: {}", signature);
        Ok(signature.to_string())
    }

    /// 获取账户余额
    pub async fn get_balances(&self) -> Result<(f64, f64)> {
        let swap = self.swap.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Solana服务未初始化"))?;

        let (sol_lamports, usdc_micro) = swap.get_account_balances().await?;
        
        let sol_balance = sol_lamports as f64 / 1_000_000_000.0;
        let usdc_balance = usdc_micro as f64 / 1_000_000.0;

        Ok((sol_balance, usdc_balance))
    }

    /// 检查服务是否已初始化
    pub fn is_initialized(&self) -> bool {
        self.swap.is_some()
    }

    /// 获取配置
    pub fn get_config(&self) -> &SwapConfig {
        &self.config
    }

    /// 更新配置
    pub fn update_config(&mut self, config: SwapConfig) {
        self.config = config;
        self.swap = None; // 重置swap实例，需要重新初始化
    }
}

impl Default for SolanaService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_solana_service_creation() {
        let service = SolanaService::new();
        assert!(!service.is_initialized());
    }

    #[tokio::test]
    async fn test_solana_service_with_config() {
        let config = SwapConfig::default();
        let service = SolanaService::with_config(config);
        assert!(!service.is_initialized());
    }
} 