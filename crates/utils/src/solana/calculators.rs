use anyhow::{anyhow, Result};
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
            Ok(MAX_FEE_BASIS_POINTS as u64 * 2)
        }
    }
}
/// PDA计算器 - 统一管理PDA地址计算
pub struct PDACalculator;

impl PDACalculator {
    /// 计算AMM配置PDA
    pub fn calculate_amm_config_pda(raydium_program_id: &Pubkey, amm_config_index: u16) -> (Pubkey, u8) {
        info!("计算AMM配置PDA: raydium_program_id: {:?}, amm_config_index: {:?}", raydium_program_id, amm_config_index);
        Pubkey::find_program_address(&["amm_config".as_bytes(), &amm_config_index.to_be_bytes()], raydium_program_id)
    }

    /// 计算池子PDA
    pub fn calculate_pool_pda(raydium_program_id: &Pubkey, amm_config_key: &Pubkey, mint0: &Pubkey, mint1: &Pubkey) -> (Pubkey, u8) {
        info!(
            "计算池子PDA: raydium_program_id: {:?}, amm_config_key: {:?}, mint0: {:?}, mint1: {:?}",
            raydium_program_id, amm_config_key, mint0, mint1
        );
        Pubkey::find_program_address(
            &["pool".as_bytes(), amm_config_key.to_bytes().as_ref(), mint0.to_bytes().as_ref(), mint1.to_bytes().as_ref()],
            raydium_program_id,
        )
    }

    /// 计算tick array bitmap extension PDA
    pub fn calculate_tickarray_bitmap_extension_pda(raydium_program_id: &Pubkey, pool_pubkey: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(&["pool_tick_array_bitmap_extension".as_bytes(), pool_pubkey.as_ref()], raydium_program_id)
    }

    /// 计算tick array PDA
    pub fn calculate_tick_array_pda(raydium_program_id: &Pubkey, pool_pubkey: &Pubkey, tick_index: i32) -> (Pubkey, u8) {
        Pubkey::find_program_address(&["tick_array".as_bytes(), pool_pubkey.as_ref(), tick_index.to_be_bytes().as_ref()], raydium_program_id)
    }

    /// 计算observation PDA
    pub fn calculate_observation_pda(raydium_program_id: &Pubkey, pool_pubkey: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(&["observation".as_bytes(), pool_pubkey.as_ref()], raydium_program_id)
    }

    /// 计算池子vault PDA
    pub fn calculate_pool_vault_pda(raydium_program_id: &Pubkey, pool_pubkey: &Pubkey, mint: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(&["pool_vault".as_bytes(), pool_pubkey.as_ref(), mint.as_ref()], raydium_program_id)
    }

    // ============ V2 AMM (Classic AMM) PDA Calculations ============

    /// 计算V2 AMM池子PDA
    /// 基于Raydium V2 AMM程序的池子地址计算
    pub fn calculate_v2_amm_pool_pda(program_id: &Pubkey, mint0: &Pubkey, mint1: &Pubkey) -> (Pubkey, u8) {
        info!("计算V2 AMM池子PDA: program_id: {:?}, mint0: {:?}, mint1: {:?}", program_id, mint0, mint1);

        // Raydium V2 AMM uses "amm_associated_seed" as the seed for pool PDA
        // The order of mints matters - typically sorted by pubkey bytes
        let (mint_a, mint_b) = if mint0.to_bytes() < mint1.to_bytes() { (mint0, mint1) } else { (mint1, mint0) };

        Pubkey::find_program_address(&["amm_associated_seed".as_bytes(), mint_a.as_ref(), mint_b.as_ref()], program_id)
    }

    /// 计算V2 AMM池子coin token账户PDA
    /// coin token通常是第一个token mint
    pub fn calculate_v2_pool_coin_token_account(program_id: &Pubkey, pool_id: &Pubkey) -> (Pubkey, u8) {
        info!("计算V2 AMM coin token账户PDA: program_id: {:?}, pool_id: {:?}", program_id, pool_id);

        Pubkey::find_program_address(&["coin_vault_associated_seed".as_bytes(), pool_id.as_ref()], program_id)
    }

    /// 计算V2 AMM池子PC token账户PDA
    /// PC (Price Currency) token通常是第二个token mint (如USDC)
    pub fn calculate_v2_pool_pc_token_account(program_id: &Pubkey, pool_id: &Pubkey) -> (Pubkey, u8) {
        info!("计算V2 AMM PC token账户PDA: program_id: {:?}, pool_id: {:?}", program_id, pool_id);

        Pubkey::find_program_address(&["pc_vault_associated_seed".as_bytes(), pool_id.as_ref()], program_id)
    }

    /// 计算V2 AMM LP mint PDA
    /// LP token mint用于表示流动性提供者的份额
    pub fn calculate_v2_lp_mint_pda(program_id: &Pubkey, pool_id: &Pubkey) -> (Pubkey, u8) {
        info!("计算V2 AMM LP mint PDA: program_id: {:?}, pool_id: {:?}", program_id, pool_id);

        Pubkey::find_program_address(&["lp_mint_associated_seed".as_bytes(), pool_id.as_ref()], program_id)
    }

    /// 计算V2 AMM open orders PDA
    /// 用于Serum市场集成的open orders账户
    pub fn calculate_v2_open_orders_pda(program_id: &Pubkey, pool_id: &Pubkey) -> (Pubkey, u8) {
        info!("计算V2 AMM open orders PDA: program_id: {:?}, pool_id: {:?}", program_id, pool_id);

        Pubkey::find_program_address(&["open_order_associated_seed".as_bytes(), pool_id.as_ref()], program_id)
    }

    /// 计算V2 AMM target orders PDA
    /// 用于目标订单管理
    pub fn calculate_v2_target_orders_pda(program_id: &Pubkey, pool_id: &Pubkey) -> (Pubkey, u8) {
        info!("计算V2 AMM target orders PDA: program_id: {:?}, pool_id: {:?}", program_id, pool_id);

        Pubkey::find_program_address(&["target_associated_seed".as_bytes(), pool_id.as_ref()], program_id)
    }

    /// 计算V2 AMM withdraw queue PDA
    /// 用于管理提取队列
    pub fn calculate_v2_withdraw_queue_pda(program_id: &Pubkey, pool_id: &Pubkey) -> (Pubkey, u8) {
        info!("计算V2 AMM withdraw queue PDA: program_id: {:?}, pool_id: {:?}", program_id, pool_id);

        Pubkey::find_program_address(&["withdraw_associated_seed".as_bytes(), pool_id.as_ref()], program_id)
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

/// V2 AMM初始化参数结构体
#[derive(Debug, Clone, PartialEq)]
pub struct V2InitializeParams {
    /// PDA计算的nonce值
    pub nonce: u8,
    /// 池子开放时间 (Unix时间戳)
    pub open_time: u64,
    /// PC token (通常是USDC等稳定币) 的初始数量
    pub init_pc_amount: u64,
    /// Coin token (通常是其他代币) 的初始数量
    pub init_coin_amount: u64,
    /// 池子ID (PDA地址)
    pub pool_id: Pubkey,
    /// Coin token账户地址
    pub coin_vault: Pubkey,
    /// PC token账户地址
    pub pc_vault: Pubkey,
    /// LP mint地址
    pub lp_mint: Pubkey,
    /// Open orders地址
    pub open_orders: Pubkey,
    /// Target orders地址
    pub target_orders: Pubkey,
    /// Withdraw queue地址
    pub withdraw_queue: Pubkey,
}

/// V2 AMM参数计算器 - 统一管理V2 AMM池子创建参数计算
pub struct V2AmmParameterCalculator;

impl V2AmmParameterCalculator {
    /// 计算V2 AMM初始化所需的所有参数
    ///
    /// # Arguments
    /// * `program_id` - Raydium V2 AMM程序ID
    /// * `mint0` - 第一个token mint地址
    /// * `mint1` - 第二个token mint地址
    /// * `init_amount_0` - 第一个token的初始数量
    /// * `init_amount_1` - 第二个token的初始数量
    /// * `open_time` - 池子开放时间 (Unix时间戳，0表示立即开放)
    ///
    /// # Returns
    /// * `Result<V2InitializeParams>` - 包含所有初始化参数的结构体
    pub fn calculate_initialize_params(program_id: &Pubkey, mint0: &Pubkey, mint1: &Pubkey, init_amount_0: u64, init_amount_1: u64, open_time: u64) -> Result<V2InitializeParams> {
        info!(
            "计算V2 AMM初始化参数: program_id: {:?}, mint0: {:?}, mint1: {:?}, amounts: ({}, {}), open_time: {}",
            program_id, mint0, mint1, init_amount_0, init_amount_1, open_time
        );

        // 验证输入参数
        Self::validate_initialize_params(mint0, mint1, init_amount_0, init_amount_1)?;

        // 计算池子PDA和nonce
        let (pool_id, nonce) = PDACalculator::calculate_v2_amm_pool_pda(program_id, mint0, mint1);

        // 计算所有相关的PDA地址
        let (coin_vault, _) = PDACalculator::calculate_v2_pool_coin_token_account(program_id, &pool_id);
        let (pc_vault, _) = PDACalculator::calculate_v2_pool_pc_token_account(program_id, &pool_id);
        let (lp_mint, _) = PDACalculator::calculate_v2_lp_mint_pda(program_id, &pool_id);
        let (open_orders, _) = PDACalculator::calculate_v2_open_orders_pda(program_id, &pool_id);
        let (target_orders, _) = PDACalculator::calculate_v2_target_orders_pda(program_id, &pool_id);
        let (withdraw_queue, _) = PDACalculator::calculate_v2_withdraw_queue_pda(program_id, &pool_id);

        // 确定coin和pc的顺序 (按照mint地址字节序排序)
        let (coin_mint, pc_mint, init_coin_amount, init_pc_amount) = if mint0.to_bytes() < mint1.to_bytes() {
            (mint0, mint1, init_amount_0, init_amount_1)
        } else {
            (mint1, mint0, init_amount_1, init_amount_0)
        };

        info!(
            "V2 AMM参数计算完成: pool_id: {:?}, nonce: {}, coin_mint: {:?}, pc_mint: {:?}",
            pool_id, nonce, coin_mint, pc_mint
        );

        Ok(V2InitializeParams {
            nonce,
            open_time,
            init_pc_amount,
            init_coin_amount,
            pool_id,
            coin_vault,
            pc_vault,
            lp_mint,
            open_orders,
            target_orders,
            withdraw_queue,
        })
    }

    /// 验证初始化参数的有效性
    ///
    /// # Arguments
    /// * `mint0` - 第一个token mint地址
    /// * `mint1` - 第二个token mint地址
    /// * `init_amount_0` - 第一个token的初始数量
    /// * `init_amount_1` - 第二个token的初始数量
    ///
    /// # Returns
    /// * `Result<()>` - 验证成功返回Ok，失败返回错误
    pub fn validate_initialize_params(mint0: &Pubkey, mint1: &Pubkey, init_amount_0: u64, init_amount_1: u64) -> Result<()> {
        // 验证mint地址不能相同
        if mint0 == mint1 {
            return Err(anyhow!("Token mint addresses cannot be the same: {:?}", mint0));
        }

        // 验证mint地址不能是默认值
        if *mint0 == Pubkey::default() {
            return Err(anyhow!("Invalid mint0 address: cannot be default pubkey"));
        }

        if *mint1 == Pubkey::default() {
            return Err(anyhow!("Invalid mint1 address: cannot be default pubkey"));
        }

        // 验证初始数量必须大于0
        if init_amount_0 == 0 {
            return Err(anyhow!("Initial amount for mint0 must be greater than 0"));
        }

        if init_amount_1 == 0 {
            return Err(anyhow!("Initial amount for mint1 must be greater than 0"));
        }

        // 验证初始数量不能超过最大值 (防止溢出)
        const MAX_INIT_AMOUNT: u64 = u64::MAX / 2; // 保守的最大值
        if init_amount_0 > MAX_INIT_AMOUNT {
            return Err(anyhow!("Initial amount for mint0 is too large: {}", init_amount_0));
        }

        if init_amount_1 > MAX_INIT_AMOUNT {
            return Err(anyhow!("Initial amount for mint1 is too large: {}", init_amount_1));
        }

        Ok(())
    }

    /// 根据token的小数位数格式化金额
    ///
    /// # Arguments
    /// * `amount` - 原始金额
    /// * `decimals` - token的小数位数
    ///
    /// # Returns
    /// * `Result<u64>` - 格式化后的金额
    pub fn format_token_amount(amount: u64, decimals: u8) -> Result<u64> {
        if decimals > 18 {
            return Err(anyhow!("Token decimals cannot exceed 18, got: {}", decimals));
        }

        // 检查是否会溢出
        let multiplier = 10_u64.pow(decimals as u32);
        amount
            .checked_mul(multiplier)
            .ok_or_else(|| anyhow!("Token amount overflow when formatting: {} * 10^{}", amount, decimals))
    }

    /// 验证token mint地址的有效性
    ///
    /// # Arguments
    /// * `mint` - token mint地址
    ///
    /// # Returns
    /// * `Result<()>` - 验证成功返回Ok，失败返回错误
    pub fn validate_mint_address(mint: &Pubkey) -> Result<()> {
        // 检查是否是系统程序地址 (通常不应该用作mint)
        // 注意：系统程序ID实际上就是默认pubkey，所以这个检查要在默认pubkey检查之前
        if *mint == solana_sdk::system_program::id() {
            return Err(anyhow!("Invalid mint address: cannot be system program"));
        }

        // 检查是否是SPL Token程序地址
        if *mint == spl_token::id() {
            return Err(anyhow!("Invalid mint address: cannot be SPL Token program"));
        }

        // 检查是否是默认pubkey (这个检查应该在系统程序检查之后)
        if *mint == Pubkey::default() {
            return Err(anyhow!("Invalid mint address: cannot be default pubkey"));
        }

        Ok(())
    }

    /// 计算nonce值 (从PDA计算中获取)
    ///
    /// # Arguments
    /// * `program_id` - 程序ID
    /// * `mint0` - 第一个token mint
    /// * `mint1` - 第二个token mint
    ///
    /// # Returns
    /// * `u8` - nonce值
    pub fn calculate_nonce(program_id: &Pubkey, mint0: &Pubkey, mint1: &Pubkey) -> u8 {
        let (_, nonce) = PDACalculator::calculate_v2_amm_pool_pda(program_id, mint0, mint1);
        nonce
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    // Test constants
    const TEST_V2_AMM_PROGRAM_ID: &str = "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8";
    const TEST_SOL_MINT: &str = "So11111111111111111111111111111111111111112";
    const TEST_USDC_MINT: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";

    #[test]
    fn test_calculate_v2_amm_pool_pda() {
        let program_id = Pubkey::from_str(TEST_V2_AMM_PROGRAM_ID).unwrap();
        let mint0 = Pubkey::from_str(TEST_SOL_MINT).unwrap();
        let mint1 = Pubkey::from_str(TEST_USDC_MINT).unwrap();

        let (pool_pda, bump) = PDACalculator::calculate_v2_amm_pool_pda(&program_id, &mint0, &mint1);

        // Verify the PDA is valid
        assert_ne!(pool_pda, Pubkey::default());
        assert!(bump > 0); // bump should be a valid nonce

        // Test with reversed mint order - should produce the same result due to sorting
        let (pool_pda_reversed, bump_reversed) = PDACalculator::calculate_v2_amm_pool_pda(&program_id, &mint1, &mint0);
        assert_eq!(pool_pda, pool_pda_reversed);
        assert_eq!(bump, bump_reversed);

        // Test with same mints - should still work
        let (pool_pda_same, bump_same) = PDACalculator::calculate_v2_amm_pool_pda(&program_id, &mint0, &mint0);
        assert_ne!(pool_pda_same, Pubkey::default());
        assert!(bump_same > 0);
    }

    #[test]
    fn test_calculate_v2_pool_coin_token_account() {
        let program_id = Pubkey::from_str(TEST_V2_AMM_PROGRAM_ID).unwrap();
        let pool_id = Pubkey::new_unique();

        let (coin_token_account, _bump) = PDACalculator::calculate_v2_pool_coin_token_account(&program_id, &pool_id);

        // Verify the PDA is valid
        assert_ne!(coin_token_account, Pubkey::default());

        // Test with different pool_id should produce different result
        let different_pool_id = Pubkey::new_unique();
        let (different_coin_token_account, _) = PDACalculator::calculate_v2_pool_coin_token_account(&program_id, &different_pool_id);
        assert_ne!(coin_token_account, different_coin_token_account);
    }

    #[test]
    fn test_calculate_v2_pool_pc_token_account() {
        let program_id = Pubkey::from_str(TEST_V2_AMM_PROGRAM_ID).unwrap();
        let pool_id = Pubkey::new_unique();

        let (pc_token_account, _bump) = PDACalculator::calculate_v2_pool_pc_token_account(&program_id, &pool_id);

        // Verify the PDA is valid
        assert_ne!(pc_token_account, Pubkey::default());

        // Test with different pool_id should produce different result
        let different_pool_id = Pubkey::new_unique();
        let (different_pc_token_account, _) = PDACalculator::calculate_v2_pool_pc_token_account(&program_id, &different_pool_id);
        assert_ne!(pc_token_account, different_pc_token_account);
    }

    #[test]
    fn test_calculate_v2_lp_mint_pda() {
        let program_id = Pubkey::from_str(TEST_V2_AMM_PROGRAM_ID).unwrap();
        let pool_id = Pubkey::new_unique();

        let (lp_mint, _bump) = PDACalculator::calculate_v2_lp_mint_pda(&program_id, &pool_id);

        // Verify the PDA is valid
        assert_ne!(lp_mint, Pubkey::default());

        // Test with different pool_id should produce different result
        let different_pool_id = Pubkey::new_unique();
        let (different_lp_mint, _) = PDACalculator::calculate_v2_lp_mint_pda(&program_id, &different_pool_id);
        assert_ne!(lp_mint, different_lp_mint);
    }

    #[test]
    fn test_calculate_v2_open_orders_pda() {
        let program_id = Pubkey::from_str(TEST_V2_AMM_PROGRAM_ID).unwrap();
        let pool_id = Pubkey::new_unique();

        let (open_orders, _bump) = PDACalculator::calculate_v2_open_orders_pda(&program_id, &pool_id);

        // Verify the PDA is valid
        assert_ne!(open_orders, Pubkey::default());

        // Test with different pool_id should produce different result
        let different_pool_id = Pubkey::new_unique();
        let (different_open_orders, _) = PDACalculator::calculate_v2_open_orders_pda(&program_id, &different_pool_id);
        assert_ne!(open_orders, different_open_orders);
    }

    #[test]
    fn test_calculate_v2_target_orders_pda() {
        let program_id = Pubkey::from_str(TEST_V2_AMM_PROGRAM_ID).unwrap();
        let pool_id = Pubkey::new_unique();

        let (target_orders, _bump) = PDACalculator::calculate_v2_target_orders_pda(&program_id, &pool_id);

        // Verify the PDA is valid
        assert_ne!(target_orders, Pubkey::default());

        // Test with different pool_id should produce different result
        let different_pool_id = Pubkey::new_unique();
        let (different_target_orders, _) = PDACalculator::calculate_v2_target_orders_pda(&program_id, &different_pool_id);
        assert_ne!(target_orders, different_target_orders);
    }

    #[test]
    fn test_calculate_v2_withdraw_queue_pda() {
        let program_id = Pubkey::from_str(TEST_V2_AMM_PROGRAM_ID).unwrap();
        let pool_id = Pubkey::new_unique();

        let (withdraw_queue, _bump) = PDACalculator::calculate_v2_withdraw_queue_pda(&program_id, &pool_id);

        // Verify the PDA is valid
        assert_ne!(withdraw_queue, Pubkey::default());

        // Test with different pool_id should produce different result
        let different_pool_id = Pubkey::new_unique();
        let (different_withdraw_queue, _) = PDACalculator::calculate_v2_withdraw_queue_pda(&program_id, &different_pool_id);
        assert_ne!(withdraw_queue, different_withdraw_queue);
    }

    #[test]
    fn test_v2_amm_pda_consistency() {
        let program_id = Pubkey::from_str(TEST_V2_AMM_PROGRAM_ID).unwrap();
        let mint0 = Pubkey::from_str(TEST_SOL_MINT).unwrap();
        let mint1 = Pubkey::from_str(TEST_USDC_MINT).unwrap();

        // Calculate pool PDA
        let (pool_id, _) = PDACalculator::calculate_v2_amm_pool_pda(&program_id, &mint0, &mint1);

        // Calculate all related PDAs
        let (coin_token_account, _) = PDACalculator::calculate_v2_pool_coin_token_account(&program_id, &pool_id);
        let (pc_token_account, _) = PDACalculator::calculate_v2_pool_pc_token_account(&program_id, &pool_id);
        let (lp_mint, _) = PDACalculator::calculate_v2_lp_mint_pda(&program_id, &pool_id);
        let (open_orders, _) = PDACalculator::calculate_v2_open_orders_pda(&program_id, &pool_id);
        let (target_orders, _) = PDACalculator::calculate_v2_target_orders_pda(&program_id, &pool_id);
        let (withdraw_queue, _) = PDACalculator::calculate_v2_withdraw_queue_pda(&program_id, &pool_id);

        // Verify all PDAs are different from each other
        let pdas = vec![pool_id, coin_token_account, pc_token_account, lp_mint, open_orders, target_orders, withdraw_queue];

        for (i, pda1) in pdas.iter().enumerate() {
            for (j, pda2) in pdas.iter().enumerate() {
                if i != j {
                    assert_ne!(pda1, pda2, "PDAs at indices {} and {} should be different", i, j);
                }
            }
        }

        // Verify none of the PDAs are the default pubkey
        for pda in pdas {
            assert_ne!(pda, Pubkey::default());
        }
    }

    #[test]
    fn test_v2_amm_pda_deterministic() {
        let program_id = Pubkey::from_str(TEST_V2_AMM_PROGRAM_ID).unwrap();
        let mint0 = Pubkey::from_str(TEST_SOL_MINT).unwrap();
        let mint1 = Pubkey::from_str(TEST_USDC_MINT).unwrap();

        // Calculate the same PDA multiple times
        let (pool_id1, bump1) = PDACalculator::calculate_v2_amm_pool_pda(&program_id, &mint0, &mint1);
        let (pool_id2, bump2) = PDACalculator::calculate_v2_amm_pool_pda(&program_id, &mint0, &mint1);

        // Results should be identical
        assert_eq!(pool_id1, pool_id2);
        assert_eq!(bump1, bump2);

        // Test with other PDA calculations
        let (coin_account1, coin_bump1) = PDACalculator::calculate_v2_pool_coin_token_account(&program_id, &pool_id1);
        let (coin_account2, coin_bump2) = PDACalculator::calculate_v2_pool_coin_token_account(&program_id, &pool_id1);

        assert_eq!(coin_account1, coin_account2);
        assert_eq!(coin_bump1, coin_bump2);
    }

    #[test]
    fn test_v2_amm_pda_with_invalid_program_id() {
        let invalid_program_id = Pubkey::default();
        let mint0 = Pubkey::from_str(TEST_SOL_MINT).unwrap();
        let mint1 = Pubkey::from_str(TEST_USDC_MINT).unwrap();

        // Should still calculate a valid PDA, just with different program
        let (pool_pda, _bump) = PDACalculator::calculate_v2_amm_pool_pda(&invalid_program_id, &mint0, &mint1);

        assert_ne!(pool_pda, Pubkey::default());

        // Should be different from the valid program ID result
        let valid_program_id = Pubkey::from_str(TEST_V2_AMM_PROGRAM_ID).unwrap();
        let (valid_pool_pda, _) = PDACalculator::calculate_v2_amm_pool_pda(&valid_program_id, &mint0, &mint1);

        assert_ne!(pool_pda, valid_pool_pda);
    }

    #[test]
    fn test_mint_ordering_in_pool_pda() {
        let program_id = Pubkey::from_str(TEST_V2_AMM_PROGRAM_ID).unwrap();
        let mint0 = Pubkey::from_str(TEST_SOL_MINT).unwrap();
        let mint1 = Pubkey::from_str(TEST_USDC_MINT).unwrap();

        // Test that mint ordering is handled correctly
        let (pool_pda_order1, bump1) = PDACalculator::calculate_v2_amm_pool_pda(&program_id, &mint0, &mint1);
        let (pool_pda_order2, bump2) = PDACalculator::calculate_v2_amm_pool_pda(&program_id, &mint1, &mint0);

        // Should produce the same result regardless of input order
        assert_eq!(pool_pda_order1, pool_pda_order2);
        assert_eq!(bump1, bump2);

        // Verify the ordering logic by checking which mint comes first
        let mint0_bytes = mint0.to_bytes();
        let mint1_bytes = mint1.to_bytes();

        // The implementation should sort by bytes
        if mint0_bytes < mint1_bytes {
            // mint0 should be first in the seed
            let expected_pda = Pubkey::find_program_address(&["amm_associated_seed".as_bytes(), mint0.as_ref(), mint1.as_ref()], &program_id);
            assert_eq!(pool_pda_order1, expected_pda.0);
        } else {
            // mint1 should be first in the seed
            let expected_pda = Pubkey::find_program_address(&["amm_associated_seed".as_bytes(), mint1.as_ref(), mint0.as_ref()], &program_id);
            assert_eq!(pool_pda_order1, expected_pda.0);
        }
    }

    // ============ V2AmmParameterCalculator Tests ============

    #[test]
    fn test_calculate_initialize_params_success() {
        let program_id = Pubkey::from_str(TEST_V2_AMM_PROGRAM_ID).unwrap();
        let mint0 = Pubkey::from_str(TEST_SOL_MINT).unwrap();
        let mint1 = Pubkey::from_str(TEST_USDC_MINT).unwrap();
        let init_amount_0 = 1_000_000_000; // 1 SOL
        let init_amount_1 = 100_000_000; // 100 USDC
        let open_time = 1640995200; // 2022-01-01 00:00:00 UTC

        let result = V2AmmParameterCalculator::calculate_initialize_params(&program_id, &mint0, &mint1, init_amount_0, init_amount_1, open_time);

        assert!(result.is_ok());
        let params = result.unwrap();

        // Verify basic parameters
        assert_eq!(params.open_time, open_time);
        assert!(params.nonce > 0);

        // Verify all PDAs are valid (not default)
        assert_ne!(params.pool_id, Pubkey::default());
        assert_ne!(params.coin_vault, Pubkey::default());
        assert_ne!(params.pc_vault, Pubkey::default());
        assert_ne!(params.lp_mint, Pubkey::default());
        assert_ne!(params.open_orders, Pubkey::default());
        assert_ne!(params.target_orders, Pubkey::default());
        assert_ne!(params.withdraw_queue, Pubkey::default());

        // Verify amounts are assigned correctly based on mint ordering
        let mint0_bytes = mint0.to_bytes();
        let mint1_bytes = mint1.to_bytes();

        if mint0_bytes < mint1_bytes {
            // mint0 is coin, mint1 is pc
            assert_eq!(params.init_coin_amount, init_amount_0);
            assert_eq!(params.init_pc_amount, init_amount_1);
        } else {
            // mint1 is coin, mint0 is pc
            assert_eq!(params.init_coin_amount, init_amount_1);
            assert_eq!(params.init_pc_amount, init_amount_0);
        }
    }

    #[test]
    fn test_calculate_initialize_params_with_zero_open_time() {
        let program_id = Pubkey::from_str(TEST_V2_AMM_PROGRAM_ID).unwrap();
        let mint0 = Pubkey::from_str(TEST_SOL_MINT).unwrap();
        let mint1 = Pubkey::from_str(TEST_USDC_MINT).unwrap();
        let init_amount_0 = 1_000_000_000;
        let init_amount_1 = 100_000_000;
        let open_time = 0; // Immediate open

        let result = V2AmmParameterCalculator::calculate_initialize_params(&program_id, &mint0, &mint1, init_amount_0, init_amount_1, open_time);

        assert!(result.is_ok());
        let params = result.unwrap();
        assert_eq!(params.open_time, 0);
    }

    #[test]
    fn test_validate_initialize_params_success() {
        let mint0 = Pubkey::from_str(TEST_SOL_MINT).unwrap();
        let mint1 = Pubkey::from_str(TEST_USDC_MINT).unwrap();
        let init_amount_0 = 1_000_000_000;
        let init_amount_1 = 100_000_000;

        let result = V2AmmParameterCalculator::validate_initialize_params(&mint0, &mint1, init_amount_0, init_amount_1);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_initialize_params_same_mints() {
        let mint0 = Pubkey::from_str(TEST_SOL_MINT).unwrap();
        let mint1 = mint0; // Same mint
        let init_amount_0 = 1_000_000_000;
        let init_amount_1 = 100_000_000;

        let result = V2AmmParameterCalculator::validate_initialize_params(&mint0, &mint1, init_amount_0, init_amount_1);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cannot be the same"));
    }

    #[test]
    fn test_validate_initialize_params_default_mint() {
        let mint0 = Pubkey::default(); // Invalid default mint
        let mint1 = Pubkey::from_str(TEST_USDC_MINT).unwrap();
        let init_amount_0 = 1_000_000_000;
        let init_amount_1 = 100_000_000;

        let result = V2AmmParameterCalculator::validate_initialize_params(&mint0, &mint1, init_amount_0, init_amount_1);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cannot be default pubkey"));
    }

    #[test]
    fn test_validate_initialize_params_zero_amounts() {
        let mint0 = Pubkey::from_str(TEST_SOL_MINT).unwrap();
        let mint1 = Pubkey::from_str(TEST_USDC_MINT).unwrap();

        // Test zero amount for mint0
        let result = V2AmmParameterCalculator::validate_initialize_params(&mint0, &mint1, 0, 100_000_000);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("must be greater than 0"));

        // Test zero amount for mint1
        let result = V2AmmParameterCalculator::validate_initialize_params(&mint0, &mint1, 1_000_000_000, 0);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("must be greater than 0"));
    }

    #[test]
    fn test_validate_initialize_params_overflow_amounts() {
        let mint0 = Pubkey::from_str(TEST_SOL_MINT).unwrap();
        let mint1 = Pubkey::from_str(TEST_USDC_MINT).unwrap();
        let max_amount = u64::MAX; // This should trigger overflow protection

        let result = V2AmmParameterCalculator::validate_initialize_params(&mint0, &mint1, max_amount, 100_000_000);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("too large"));
    }

    #[test]
    fn test_format_token_amount_success() {
        // Test with 6 decimals (like USDC)
        let result = V2AmmParameterCalculator::format_token_amount(100, 6);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 100_000_000); // 100 * 10^6

        // Test with 9 decimals (like SOL)
        let result = V2AmmParameterCalculator::format_token_amount(1, 9);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1_000_000_000); // 1 * 10^9

        // Test with 0 decimals
        let result = V2AmmParameterCalculator::format_token_amount(100, 0);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 100);
    }

    #[test]
    fn test_format_token_amount_invalid_decimals() {
        // Test with too many decimals
        let result = V2AmmParameterCalculator::format_token_amount(100, 19);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cannot exceed 18"));
    }

    #[test]
    fn test_format_token_amount_overflow() {
        // Test with amount that would cause overflow
        let large_amount = u64::MAX / 100; // This should cause overflow with high decimals
        let result = V2AmmParameterCalculator::format_token_amount(large_amount, 18);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("overflow"));
    }

    #[test]
    fn test_validate_mint_address_success() {
        let mint = Pubkey::from_str(TEST_SOL_MINT).unwrap();
        let result = V2AmmParameterCalculator::validate_mint_address(&mint);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_mint_address_default() {
        let mint = Pubkey::default();
        let result = V2AmmParameterCalculator::validate_mint_address(&mint);
        assert!(result.is_err());
        // Since system program ID is the same as default pubkey, it will be caught by system program check
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("cannot be system program") || error_msg.contains("cannot be default pubkey"));
    }

    #[test]
    fn test_validate_mint_address_system_program() {
        let mint = solana_sdk::system_program::id();
        let result = V2AmmParameterCalculator::validate_mint_address(&mint);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cannot be system program"));
    }

    #[test]
    fn test_validate_mint_address_spl_token_program() {
        let mint = spl_token::id();
        let result = V2AmmParameterCalculator::validate_mint_address(&mint);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cannot be SPL Token program"));
    }

    #[test]
    fn test_calculate_nonce() {
        let program_id = Pubkey::from_str(TEST_V2_AMM_PROGRAM_ID).unwrap();
        let mint0 = Pubkey::from_str(TEST_SOL_MINT).unwrap();
        let mint1 = Pubkey::from_str(TEST_USDC_MINT).unwrap();

        let nonce = V2AmmParameterCalculator::calculate_nonce(&program_id, &mint0, &mint1);
        assert!(nonce > 0);

        // Test that nonce is consistent
        let nonce2 = V2AmmParameterCalculator::calculate_nonce(&program_id, &mint0, &mint1);
        assert_eq!(nonce, nonce2);

        // Test with reversed mint order should give same nonce (due to sorting)
        let nonce3 = V2AmmParameterCalculator::calculate_nonce(&program_id, &mint1, &mint0);
        assert_eq!(nonce, nonce3);
    }

    #[test]
    fn test_v2_initialize_params_struct() {
        let program_id = Pubkey::from_str(TEST_V2_AMM_PROGRAM_ID).unwrap();
        let mint0 = Pubkey::from_str(TEST_SOL_MINT).unwrap();
        let mint1 = Pubkey::from_str(TEST_USDC_MINT).unwrap();

        let params1 = V2AmmParameterCalculator::calculate_initialize_params(&program_id, &mint0, &mint1, 1_000_000_000, 100_000_000, 0).unwrap();

        let params2 = V2AmmParameterCalculator::calculate_initialize_params(&program_id, &mint0, &mint1, 1_000_000_000, 100_000_000, 0).unwrap();

        // Test that the struct implements PartialEq correctly
        assert_eq!(params1, params2);

        // Test Clone
        let params3 = params1.clone();
        assert_eq!(params1, params3);

        // Test Debug (should not panic)
        let debug_str = format!("{:?}", params1);
        assert!(!debug_str.is_empty());
    }

    #[test]
    fn test_parameter_calculation_consistency() {
        let program_id = Pubkey::from_str(TEST_V2_AMM_PROGRAM_ID).unwrap();
        let mint0 = Pubkey::from_str(TEST_SOL_MINT).unwrap();
        let mint1 = Pubkey::from_str(TEST_USDC_MINT).unwrap();
        let init_amount_0 = 1_000_000_000;
        let init_amount_1 = 100_000_000;
        let open_time = 1640995200;

        // Calculate parameters multiple times
        let params1 = V2AmmParameterCalculator::calculate_initialize_params(&program_id, &mint0, &mint1, init_amount_0, init_amount_1, open_time).unwrap();

        let params2 = V2AmmParameterCalculator::calculate_initialize_params(&program_id, &mint0, &mint1, init_amount_0, init_amount_1, open_time).unwrap();

        // Results should be identical
        assert_eq!(params1, params2);

        // Test with reversed mint order
        let params3 = V2AmmParameterCalculator::calculate_initialize_params(&program_id, &mint1, &mint0, init_amount_1, init_amount_0, open_time).unwrap();

        // Pool ID and nonce should be the same (due to mint sorting)
        assert_eq!(params1.pool_id, params3.pool_id);
        assert_eq!(params1.nonce, params3.nonce);

        // But amounts should be swapped to maintain coin/pc ordering
        assert_eq!(params1.init_coin_amount, params3.init_coin_amount);
        assert_eq!(params1.init_pc_amount, params3.init_pc_amount);
    }

    #[test]
    fn test_edge_case_amounts() {
        let program_id = Pubkey::from_str(TEST_V2_AMM_PROGRAM_ID).unwrap();
        let mint0 = Pubkey::from_str(TEST_SOL_MINT).unwrap();
        let mint1 = Pubkey::from_str(TEST_USDC_MINT).unwrap();

        // Test with minimum valid amounts
        let result = V2AmmParameterCalculator::calculate_initialize_params(
            &program_id,
            &mint0,
            &mint1,
            1, // Minimum amount
            1, // Minimum amount
            0,
        );
        assert!(result.is_ok());

        // Test with large but valid amounts
        let large_amount = u64::MAX / 4; // Safe large amount
        let result = V2AmmParameterCalculator::calculate_initialize_params(&program_id, &mint0, &mint1, large_amount, large_amount, 0);
        assert!(result.is_ok());
    }
}
