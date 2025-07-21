use anyhow::Result;
use solana_sdk::pubkey::Pubkey;

use super::constants;
use spl_token_2022::extension::{transfer_fee::TransferFeeConfig, BaseStateWithExtensions, StateWithExtensions};
use tracing::info;

pub const MAX_FEE_BASIS_POINTS: u16 = 10_000;
/// Transfer Fee 计算器 - 统一管理转账费计算逻辑
pub struct TransferFeeCalculator;

impl TransferFeeCalculator {
    /// 从mint状态计算transfer fee
    pub fn get_transfer_fee_from_mint_state(mint_account_data: &[u8], epoch: u64, amount: u64) -> Result<u64> {
        let account_state = StateWithExtensions::<spl_token_2022::state::Mint>::unpack(mint_account_data);
        info!("💰 正算 Account state: {:?}", account_state);
        if let Ok(mint_state) = account_state {
            let transfer_fee_config = mint_state.get_extension::<TransferFeeConfig>();
            info!("💰 正算 Transfer fee config: {:?}", transfer_fee_config);
            let fee = if let Ok(transfer_fee_config) = transfer_fee_config {
                transfer_fee_config.calculate_epoch_fee(epoch, amount).unwrap_or(0)
            } else {
                0
            };
            Ok(fee)
        } else {
            Ok(0)
        }
    }

    pub fn get_transfer_fee_from_mint_state_inverse(mint_account_data: &[u8], epoch: u64, amount: u64) -> Result<u64> {
        let account_state = StateWithExtensions::<spl_token_2022::state::Mint>::unpack(mint_account_data);
        info!("💰 反算 Account state: {:?}", account_state);
        if let Ok(mint_state) = account_state {
            let transfer_fee_config = mint_state.get_extension::<TransferFeeConfig>();
            info!("💰 反算 Transfer fee config: {:?}", transfer_fee_config);
            let fee = if let Ok(transfer_fee_config) = transfer_fee_config {
                let transfer_fee = transfer_fee_config.get_epoch_fee(epoch);
                info!("💰 Transfer fee: {:?}", transfer_fee);
                if u16::from(transfer_fee.transfer_fee_basis_points) == MAX_FEE_BASIS_POINTS {
                    u64::from(transfer_fee.maximum_fee)
                } else {
                    transfer_fee_config.calculate_inverse_epoch_fee(epoch, amount).unwrap_or(0)
                }
            } else {
                MAX_FEE_BASIS_POINTS as u64 * 2
            };
            Ok(fee)
        } else {
            Ok(0)
        }
    }
}
/// PDA计算器 - 统一管理PDA地址计算
pub struct PDACalculator;

impl PDACalculator {
    /// 计算AMM配置PDA
    pub fn calculate_amm_config_pda(raydium_program_id: &Pubkey, amm_config_index: u16) -> (Pubkey, u8) {
        info!(
            "计算AMM配置PDA: raydium_program_id: {:?}, amm_config_index: {:?}",
            raydium_program_id, amm_config_index
        );
        Pubkey::find_program_address(&["amm_config".as_bytes(), &amm_config_index.to_be_bytes()], raydium_program_id)
    }

    /// 计算池子PDA
    pub fn calculate_pool_pda(raydium_program_id: &Pubkey, amm_config_key: &Pubkey, mint0: &Pubkey, mint1: &Pubkey) -> (Pubkey, u8) {
        info!(
            "计算池子PDA: raydium_program_id: {:?}, amm_config_key: {:?}, mint0: {:?}, mint1: {:?}",
            raydium_program_id, amm_config_key, mint0, mint1
        );
        Pubkey::find_program_address(
            &[
                "pool".as_bytes(),
                amm_config_key.to_bytes().as_ref(),
                mint0.to_bytes().as_ref(),
                mint1.to_bytes().as_ref(),
            ],
            raydium_program_id,
        )
    }

    /// 计算tick array bitmap extension PDA
    pub fn calculate_tickarray_bitmap_extension_pda(raydium_program_id: &Pubkey, pool_pubkey: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(&["pool_tick_array_bitmap_extension".as_bytes(), pool_pubkey.as_ref()], raydium_program_id)
    }

    /// 计算tick array PDA
    pub fn calculate_tick_array_pda(raydium_program_id: &Pubkey, pool_pubkey: &Pubkey, tick_index: i32) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &["tick_array".as_bytes(), pool_pubkey.as_ref(), tick_index.to_be_bytes().as_ref()],
            raydium_program_id,
        )
    }

    /// 计算observation PDA
    pub fn calculate_observation_pda(raydium_program_id: &Pubkey, pool_pubkey: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(&["observation".as_bytes(), pool_pubkey.as_ref()], raydium_program_id)
    }

    /// 计算池子vault PDA
    pub fn calculate_pool_vault_pda(raydium_program_id: &Pubkey, pool_pubkey: &Pubkey, mint: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(&["pool_vault".as_bytes(), pool_pubkey.as_ref(), mint.as_ref()], raydium_program_id)
    }
}

/// 数学工具类 - 统一管理数学计算
pub struct MathUtils;

impl MathUtils {
    /// 计算滑点保护的最小输出金额
    pub fn calculate_minimum_amount_out(amount_in: u64, slippage_bps: u16) -> u64 {
        let slippage_factor = 1.0 - (slippage_bps as f64 / 10000.0);
        (amount_in as f64 * slippage_factor) as u64
    }

    /// 计算滑点保护的最大输入金额
    pub fn calculate_maximum_amount_in(amount_out: u64, slippage_bps: u16) -> u64 {
        let slippage_factor = 1.0 + (slippage_bps as f64 / 10000.0);
        (amount_out as f64 * slippage_factor) as u64
    }

    /// 计算手续费
    pub fn calculate_fee(amount: u64, fee_rate: u64) -> u64 {
        amount / fee_rate
    }

    /// 简单的SOL/USDC价格转换
    pub fn convert_sol_to_usdc(sol_amount: u64) -> u64 {
        let sol_amount_f64 = sol_amount as f64 / 1_000_000_000.0; // lamports to SOL
        let usdc_amount = sol_amount_f64 * constants::DEFAULT_SOL_PRICE_USDC;
        (usdc_amount * 1_000_000.0) as u64 // USDC to micro-USDC
    }

    /// 简单的USDC/SOL价格转换
    pub fn convert_usdc_to_sol(usdc_amount: u64) -> u64 {
        let usdc_amount_f64 = usdc_amount as f64 / 1_000_000.0; // micro-USDC to USDC
        let sol_amount = usdc_amount_f64 / constants::DEFAULT_SOL_PRICE_USDC;
        (sol_amount * 1_000_000_000.0) as u64 // SOL to lamports
    }
}
