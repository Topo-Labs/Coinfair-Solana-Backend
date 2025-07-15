use solana_sdk::pubkey::Pubkey;
use tracing::{info, warn};

use super::constants;

/// 代币类型枚举
#[derive(Debug, Clone, PartialEq)]
pub enum TokenType {
    Sol,
    Usdc,
    Other,
}

/// 代币工具类 - 统一管理代币相关的工具方法
pub struct TokenUtils;

impl TokenUtils {
    /// 判断是否为SOL代币
    pub fn is_sol_mint(mint: &str) -> bool {
        mint == constants::SOL_MINT
    }

    /// 判断是否为USDC代币
    pub fn is_usdc_mint(mint: &str) -> bool {
        matches!(mint, constants::USDC_MINT_STANDARD | constants::USDC_MINT_CONFIG | constants::USDC_MINT_ALTERNATIVE)
    }

    /// 获取代币类型
    pub fn get_token_type(mint: &str) -> TokenType {
        if Self::is_sol_mint(mint) {
            TokenType::Sol
        } else if Self::is_usdc_mint(mint) {
            TokenType::Usdc
        } else {
            TokenType::Other
        }
    }

    /// 获取代币默认精度
    pub fn get_token_decimals(mint: &str) -> u8 {
        match Self::get_token_type(mint) {
            TokenType::Sol => 9,
            TokenType::Usdc => 6,
            TokenType::Other => 6, // 默认精度
        }
    }

    /// 标准化mint顺序（确保mint0 < mint1）
    /// 返回 (mint0, mint1, zero_for_one)
    pub fn normalize_mint_order(input_mint: &Pubkey, output_mint: &Pubkey) -> (Pubkey, Pubkey, bool) {
        if input_mint < output_mint {
            // input_mint 是 mint0，所以 zero_for_one = true
            (*input_mint, *output_mint, true)
        } else {
            // output_mint 是 mint0，所以 zero_for_one = false
            (*output_mint, *input_mint, false)
        }
    }
}

/// 日志工具 - 统一管理日志输出
pub struct LogUtils;

impl LogUtils {
    /// 记录操作开始
    pub fn log_operation_start(operation: &str, details: &str) {
        info!("开始{}: {}", operation, details);
    }

    /// 记录操作成功
    pub fn log_operation_success(operation: &str, result: &str) {
        info!("{}成功: {}", operation, result);
    }

    /// 记录操作失败
    pub fn log_operation_failure(operation: &str, error: &str) {
        warn!("{}失败: {}", operation, error);
    }

    /// 记录调试信息
    pub fn log_debug_info(title: &str, info: &[(&str, &str)]) {
        info!("📋 {}:", title);
        for (key, value) in info {
            info!("  {}: {}", key, value);
        }
    }

    /// 记录计算结果
    pub fn log_calculation_result(operation: &str, input: u64, output: u64, additional_info: &[(&str, &str)]) {
        info!("{}:", operation);
        info!("  输入金额: {}", input);
        info!("  输出金额: {}", output);
        for (key, value) in additional_info {
            info!("  {}: {}", key, value);
        }
    }
}
