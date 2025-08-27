// Shared helper functions for Solana services
use super::SharedContext;
use crate::dtos::solana::common::{RoutePlan, TransferFeeInfo, WalletInfo};
use crate::dtos::solana::swap::basic::BalanceResponse;
use crate::dtos::solana::swap::raydium::SwapComputeV2Data;
use anchor_lang::AccountDeserialize;
use anyhow::Result;
use solana_sdk::account::Account;
use tracing::info;

/// 响应数据构建器 - 统一管理响应数据创建
pub struct ResponseBuilder;

impl ResponseBuilder {
    /// 创建SwapComputeV2Data响应
    pub fn create_swap_compute_v2_data(
        swap_type: String,
        input_mint: String,
        input_amount: String,
        output_mint: String,
        output_amount: u64,
        other_amount_threshold: u64,
        slippage_bps: u16,
        route_plan: Vec<RoutePlan>,
        transfer_fee_info: Option<TransferFeeInfo>,
        amount_specified: Option<u64>,
        epoch: Option<u64>,
        price_impact_pct: Option<f64>,
    ) -> SwapComputeV2Data {
        SwapComputeV2Data {
            swap_type,
            input_mint,
            input_amount,
            output_mint,
            output_amount: output_amount.to_string(),
            other_amount_threshold: other_amount_threshold.to_string(),
            slippage_bps,
            price_impact_pct: price_impact_pct.unwrap_or(0.1),
            referrer_amount: "0".to_string(),
            route_plan,
            transfer_fee_info,
            amount_specified: amount_specified.map(|a| a.to_string()),
            epoch,
        }
    }
}

/// Shared helper functions that can be used across different Solana services
pub struct SolanaHelpers;

impl SolanaHelpers {
    /// Get account balance - moved from original SolanaService
    pub async fn get_balance(shared: &SharedContext) -> Result<BalanceResponse> {
        info!("💰 获取钱包余额");

        shared.ensure_raydium_available().await?;

        let (sol_lamports, usdc_micro) = {
            let raydium_guard = shared.raydium_swap.lock().await;
            let raydium = raydium_guard.as_ref().unwrap();
            raydium.get_account_balances().await?
        };

        // 获取钱包地址
        let wallet_address = shared.get_wallet_address_from_private_key().await;

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

    /// Get wallet info - moved from original SolanaService
    pub async fn get_wallet_info(shared: &SharedContext) -> Result<WalletInfo> {
        let wallet_info = WalletInfo {
            address: shared.get_wallet_address_from_private_key().await,
            network: shared.swap_config.rpc_url.clone(),
            connected: shared.raydium_swap.lock().await.is_some(),
        };

        Ok(wallet_info)
    }

    /// Health check - moved from original SolanaService
    pub async fn health_check(shared: &SharedContext) -> Result<String> {
        if shared.raydium_swap.lock().await.is_some() {
            Ok("Solana服务运行正常".to_string())
        } else {
            Ok("Solana服务未初始化（私钥未配置）".to_string())
        }
    }
}

/// Shared utility functions for Solana services
pub struct SolanaUtils;

impl SolanaUtils {
    /// Generate explorer URL for a transaction
    pub fn get_explorer_url(signature: &str, rpc_url: &str) -> String {
        // Determine network based on RPC URL
        let network = if rpc_url.contains("devnet") {
            "devnet"
        } else if rpc_url.contains("testnet") {
            "testnet"
        } else {
            "mainnet-beta"
        };

        if network == "mainnet-beta" {
            format!("https://explorer.solana.com/tx/{}", signature)
        } else {
            format!("https://explorer.solana.com/tx/{}?cluster={}", signature, network)
        }
    }

    /// 反序列化anchor账户
    pub fn deserialize_anchor_account<T: AccountDeserialize>(account: &Account) -> Result<T> {
        let mut data: &[u8] = &account.data;
        T::try_deserialize(&mut data).map_err(Into::into)
    }
}
