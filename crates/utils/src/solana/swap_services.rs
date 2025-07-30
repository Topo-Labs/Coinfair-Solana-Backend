// 重新导出现有的工具类
pub use crate::solana::raydium_api::*;
pub use crate::solana::solana_client::*;

use anyhow::Result;
use solana_sdk::account::Account;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Signer;
use spl_token_2022::extension::transfer_fee::TransferFeeConfig;
use spl_token_2022::extension::transfer_fee::MAX_FEE_BASIS_POINTS;
use spl_token_2022::extension::BaseStateWithExtensions;
use spl_token_2022::extension::StateWithExtensions;
use spl_token_2022::state::Mint;
use tracing::info;
// Raydium相关类型
#[derive(Debug, Clone)]
pub struct SwapEstimateResult {
    pub estimated_output: u64,
    pub min_output_with_slippage: u64,
    pub price_impact: f64,
    pub current_price: f64,
    pub tick_arrays_needed: usize,
}

// 简化的RaydiumSwap结构体，提供基本功能
#[allow(dead_code)]
pub struct RaydiumSwap {
    client: SolanaClient,
    program_id: solana_sdk::pubkey::Pubkey,
}

impl RaydiumSwap {
    pub fn new(client: SolanaClient, config: &SwapConfig) -> anyhow::Result<Self> {
        let program_id = config.amm_program_id.parse()?;
        Ok(Self { client, program_id })
    }

    pub fn get_wallet_pubkey(&self) -> anyhow::Result<solana_sdk::pubkey::Pubkey> {
        Ok(self.client.get_wallet().pubkey())
    }

    pub async fn get_account_balances(&self) -> anyhow::Result<(u64, u64)> {
        // 简化实现，返回固定值
        Ok((1000000000, 1000000)) // 1 SOL, 1 USDC
    }

    pub async fn swap_tokens(&self, _from_token: &str, _to_token: &str, _pool_address: &str, _amount: u64, _minimum_amount_out: u64) -> anyhow::Result<String> {
        // 简化实现，返回模拟签名
        Ok("simulation_signature".to_string())
    }

    pub async fn get_pool_price_and_estimate_direct(&self, pool_address: &str, from_token: &str, to_token: &str, amount: u64) -> anyhow::Result<u64> {
        // 使用简化计算
        calculate_swap_output_with_api(pool_address, amount, from_token, to_token, self.client.get_rpc_client()).await
    }

    pub async fn calculate_precise_swap_output(
        &self,
        input_mint: &str,
        output_mint: &str,
        pool_address: &str,
        input_amount: u64,
        slippage: Option<f64>,
    ) -> anyhow::Result<SwapEstimateResult> {
        let estimated_output = self.get_pool_price_and_estimate_direct(pool_address, input_mint, output_mint, input_amount).await?;
        let slippage_rate = slippage.unwrap_or(0.005);
        let min_output_with_slippage = (estimated_output as f64 * (1.0 - slippage_rate)) as u64;

        Ok(SwapEstimateResult {
            estimated_output,
            min_output_with_slippage,
            price_impact: 0.001,  // 简化处理
            current_price: 100.0, // 简化处理
            tick_arrays_needed: 1,
        })
    }
}

// SwapV2相关结构体
#[allow(dead_code)]
pub struct SwapV2Service {
    rpc_url: String,
}

/// SwapV2完整账户信息
#[derive(Debug, Clone)]
pub struct SwapV2AccountsInfo {
    pub input_token_account: Account,
    pub output_token_account: Account,
    pub amm_config_account: Account,
    pub pool_account: Account,
    pub tickarray_bitmap_extension_account: Account,
    pub mint0_account: Account,
    pub mint1_account: Account,
    pub epoch: u64,
    pub pool_address: Pubkey,
    pub input_mint_info: TokenAccountInfo,
    pub output_mint_info: TokenAccountInfo,
}

/// 代币账户信息
#[derive(Debug, Clone)]
pub struct TokenAccountInfo {
    pub mint: Pubkey,
    pub decimals: u8,
    pub owner: Pubkey,
    pub is_token_2022: bool,
}

impl SwapV2Service {
    pub fn new(rpc_url: &str) -> Self {
        Self { rpc_url: rpc_url.to_string() }
    }

    pub fn get_current_epoch(&self) -> anyhow::Result<u64> {
        Ok(500) // 简化实现
    }

    // pub fn get_transfer_fee(&self, _mint: &solana_sdk::pubkey::Pubkey, amount: u64) -> anyhow::Result<TransferFeeResult> {
    //     Ok(TransferFeeResult {
    //         transfer_fee: amount / 1000, // 0.1%
    //         owner: spl_token::id(),
    //     })
    // }

    /// 计算输入代币的transfer fee（完全基于CLI实现）
    pub fn get_transfer_fee(&self, mint: &Pubkey, amount: u64) -> Result<TransferFeeResult> {
        let rpc_client = solana_client::rpc_client::RpcClient::new(&self.rpc_url);
        let account = rpc_client.get_account(mint)?;

        // 如果是标准Token程序，没有transfer fee
        if account.owner == spl_token::id() {
            return Ok(TransferFeeResult {
                mint: *mint,
                owner: account.owner,
                transfer_fee: 0,
            });
        }

        // Token-2022程序处理
        if account.owner == spl_token_2022::id() {
            let mint_data = StateWithExtensions::<Mint>::unpack(&account.data)?;

            let transfer_fee = if let Ok(transfer_fee_config) = mint_data.get_extension::<TransferFeeConfig>() {
                let epoch = self.get_current_epoch()?;
                // 与CLI保持一致：使用unwrap而不是unwrap_or，确保错误能被正确传播
                transfer_fee_config
                    .calculate_epoch_fee(epoch, amount)
                    .ok_or_else(|| anyhow::anyhow!("Transfer fee calculation failed"))?
            } else {
                0
            };

            Ok(TransferFeeResult {
                mint: *mint,
                owner: account.owner,
                transfer_fee,
            })
        } else {
            // 未知token程序
            Ok(TransferFeeResult {
                mint: *mint,
                owner: account.owner,
                transfer_fee: 0,
            })
        }
    }

    pub fn load_mint_info(&self, _mint: &solana_sdk::pubkey::Pubkey) -> anyhow::Result<MintInfo> {
        Ok(MintInfo {
            decimals: 9, // 简化处理
        })
    }

    /// 验证SwapV2账户信息的完整性
    pub fn validate_swap_v2_accounts(&self, accounts: &SwapV2AccountsInfo) -> Result<()> {
        // 验证输入和输出代币账户的有效性
        if accounts.input_token_account.owner != spl_token::id() && accounts.input_token_account.owner != spl_token_2022::id() {
            return Err(anyhow::anyhow!("无效的输入代币账户"));
        }

        if accounts.output_token_account.owner != spl_token::id() && accounts.output_token_account.owner != spl_token_2022::id() {
            return Err(anyhow::anyhow!("无效的输出代币账户"));
        }

        // 验证mint账户的有效性
        if accounts.mint0_account.owner != spl_token::id() && accounts.mint0_account.owner != spl_token_2022::id() {
            return Err(anyhow::anyhow!("无效的mint0账户"));
        }

        if accounts.mint1_account.owner != spl_token::id() && accounts.mint1_account.owner != spl_token_2022::id() {
            return Err(anyhow::anyhow!("无效的mint1账户"));
        }

        // 验证epoch有效性
        if accounts.epoch == 0 {
            return Err(anyhow::anyhow!("无效的epoch信息"));
        }

        info!("✅ SwapV2账户信息验证通过");
        Ok(())
    }

    pub fn get_pool_mints_inverse_fee(
        &self,
        mint0: &solana_sdk::pubkey::Pubkey,
        mint1: &solana_sdk::pubkey::Pubkey,
        amount0: u64,
        amount1: u64,
    ) -> anyhow::Result<(TransferFeeResult, TransferFeeResult)> {
        // 创建RPC客户端来获取实际的mint账户信息
        let rpc_client = solana_client::rpc_client::RpcClient::new(&self.rpc_url);

        // 批量加载两个mint账户以获取实际的owner
        let load_accounts = vec![*mint0, *mint1];
        let accounts = rpc_client.get_multiple_accounts(&load_accounts)?;
        let epoch = self.get_current_epoch()?;

        let mint0_account = accounts[0].as_ref().ok_or_else(|| anyhow::anyhow!("Failed to load mint0 account"))?;
        let mint1_account = accounts[1].as_ref().ok_or_else(|| anyhow::anyhow!("Failed to load mint1 account"))?;

        // 计算mint0的inverse transfer fee
        let transfer_fee_0 = self.calculate_inverse_transfer_fee_from_account(mint0, mint0_account, epoch, amount0)?;

        // 计算mint1的inverse transfer fee
        let transfer_fee_1 = self.calculate_inverse_transfer_fee_from_account(mint1, mint1_account, epoch, amount1)?;

        Ok((
            TransferFeeResult {
                mint: *mint0,
                transfer_fee: transfer_fee_0,
                owner: mint0_account.owner, // 使用实际的mint账户owner
            },
            TransferFeeResult {
                mint: *mint1,
                transfer_fee: transfer_fee_1,
                owner: mint1_account.owner, // 使用实际的mint账户owner
            },
        ))
    }

    pub fn get_pool_mints_transfer_fee(
        &self,
        mint0: &solana_sdk::pubkey::Pubkey,
        mint1: &solana_sdk::pubkey::Pubkey,
        amount0: u64,
        amount1: u64,
    ) -> anyhow::Result<(TransferFeeResult, TransferFeeResult)> {
        self.get_pool_mints_inverse_fee(mint0, mint1, amount0, amount1)
    }

    /// 从已加载的账户计算inverse transfer fee（内部辅助方法）
    fn calculate_inverse_transfer_fee_from_account(&self, mint: &Pubkey, account: &Account, epoch: u64, post_fee_amount: u64) -> Result<u64> {
        // 如果是标准Token程序，没有transfer fee
        if account.owner == spl_token::id() {
            return Ok(0);
        }

        // Token-2022程序处理
        if account.owner == spl_token_2022::id() {
            let mint_data = StateWithExtensions::<Mint>::unpack(&account.data)?;

            if let Ok(transfer_fee_config) = mint_data.get_extension::<TransferFeeConfig>() {
                let epoch_fee = transfer_fee_config.get_epoch_fee(epoch);

                // 完全按照CLI逻辑处理边界情况
                let transfer_fee = if u16::from(epoch_fee.transfer_fee_basis_points) == MAX_FEE_BASIS_POINTS {
                    // 当费率达到最大值时，直接返回最大费用
                    u64::from(epoch_fee.maximum_fee)
                } else {
                    // 正常情况下进行反向计算
                    transfer_fee_config
                        .calculate_inverse_epoch_fee(epoch, post_fee_amount)
                        .ok_or_else(|| anyhow::anyhow!("Inverse transfer fee calculation failed for {}", mint))?
                };
                Ok(transfer_fee)
            } else {
                Ok(0)
            }
        } else {
            Ok(0)
        }
    }

    /// 从已加载的账户计算transfer fee（内部辅助方法）
    fn calculate_transfer_fee_from_account(&self, mint: &Pubkey, account: &Account, epoch: u64, amount: u64) -> Result<u64> {
        // 如果是标准Token程序，没有transfer fee
        if account.owner == spl_token::id() {
            return Ok(0);
        }

        // Token-2022程序处理
        if account.owner == spl_token_2022::id() {
            let mint_data = StateWithExtensions::<Mint>::unpack(&account.data)?;
            let transfer_fee_config = mint_data.get_extension::<TransferFeeConfig>();
            info!("transfer_fee_config: {:?}", transfer_fee_config);
            if let Ok(transfer_fee_config) = transfer_fee_config {
                let transfer_fee = transfer_fee_config
                    .calculate_epoch_fee(epoch, amount)
                    .ok_or_else(|| anyhow::anyhow!("Transfer fee calculation failed for {}", mint))?;
                Ok(transfer_fee)
            } else {
                Ok(1)
            }
        } else {
            Ok(0)
        }
    }
}

#[derive(Debug, Clone)]
pub struct TransferFeeResult {
    pub mint: Pubkey,
    pub owner: Pubkey,
    pub transfer_fee: u64,
}

#[derive(Debug, Clone)]
pub struct MintInfo {
    pub decimals: u8,
}

#[allow(dead_code)]
pub struct SwapV2InstructionBuilder {
    rpc_url: String,
    program_id: String,
    config_index: u16,
}

impl SwapV2InstructionBuilder {
    pub fn new(rpc_url: &str, program_id: &str, config_index: u16) -> anyhow::Result<Self> {
        Ok(Self {
            rpc_url: rpc_url.to_string(),
            program_id: program_id.to_string(),
            config_index,
        })
    }
}
