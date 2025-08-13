use anyhow::Result;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

use super::{constants, ConfigManager, PDACalculator, TokenUtils};

/// 推荐系统管理器 - 统一管理推荐系统相关逻辑
pub struct ReferralManager;

impl ReferralManager {
    /// 计算推荐账户PDA地址
    pub fn calculate_referral_pda(
        referral_program_id: &Pubkey,
        user: &Pubkey,
    ) -> Result<(Pubkey, u8)> {
        let seeds = &[b"referral", user.as_ref()];
        let (pda, bump) = Pubkey::find_program_address(seeds, referral_program_id);
        Ok((pda, bump))
    }

    /// 查询用户的推荐关系信息
    pub async fn get_referral_info(
        user: &Pubkey,
        referral_program_id: &Pubkey,
    ) -> Result<Option<ReferralInfo>> {
        let (_referral_pda, _) = Self::calculate_referral_pda(referral_program_id, user)?;
        
        // 这里应该是从链上查询实际数据的逻辑
        // 为了示例，返回None表示没有推荐关系
        Ok(None)
    }

    /// 验证推荐系统账户的有效性
    pub fn validate_referral_accounts(
        payer: &Pubkey,
        payer_referral: &Pubkey,
        upper: Option<&Pubkey>,
        upper_referral: Option<&Pubkey>,
        _upper_upper: Option<&Pubkey>,
        referral_program_id: &Pubkey,
    ) -> Result<bool> {
        // 验证payer_referral PDA是否正确
        let (expected_payer_referral, _) = Self::calculate_referral_pda(referral_program_id, payer)?;
        if *payer_referral != expected_payer_referral {
            return Ok(false);
        }

        // 如果有upper，验证upper_referral PDA是否正确
        if let (Some(upper_pubkey), Some(upper_referral_pubkey)) = (upper, upper_referral) {
            let (expected_upper_referral, _) = Self::calculate_referral_pda(referral_program_id, upper_pubkey)?;
            if *upper_referral_pubkey != expected_upper_referral {
                return Ok(false);
            }
        }

        Ok(true)
    }

    /// 计算推荐奖励分配
    pub fn calculate_reward_distribution(total_fee: u64) -> RewardDistribution {
        let project_reward = total_fee / 2; // 50%给项目方
        let upper_total_reward = total_fee - project_reward; // 50%给推荐人系统

        RewardDistribution {
            total_reward_fee: total_fee,
            project_reward,
            upper_reward: upper_total_reward * 5 / 6, // 约41.67%总费用
            upper_upper_reward: upper_total_reward / 6, // 约8.33%总费用
            distribution_ratios: RewardDistributionRatios {
                project_ratio: 50.0,
                upper_ratio: 41.67,
                upper_upper_ratio: 8.33,
            },
        }
    }

    /// 获取项目方代币账户地址
    pub fn get_project_token_account(
        pool_owner: &Pubkey,
        token_mint: &Pubkey,
    ) -> Result<Pubkey> {
        // 使用关联代币账户
        let project_token_account = spl_associated_token_account::get_associated_token_address(
            pool_owner,
            token_mint,
        );
        Ok(project_token_account)
    }

    /// 获取上级用户的代币账户地址
    pub fn get_upper_token_account(
        upper: &Pubkey,
        token_mint: &Pubkey,
    ) -> Result<Pubkey> {
        let upper_token_account = spl_associated_token_account::get_associated_token_address(
            upper,
            token_mint,
        );
        Ok(upper_token_account)
    }
}

/// 推荐信息结构体
#[derive(Debug, Clone)]
pub struct ReferralInfo {
    pub user: Pubkey,
    pub upper: Option<Pubkey>,
    pub upper_upper: Option<Pubkey>,
    pub nft_mint: Pubkey,
}

/// 奖励分配信息
#[derive(Debug, Clone)]
pub struct RewardDistribution {
    pub total_reward_fee: u64,
    pub project_reward: u64,
    pub upper_reward: u64,
    pub upper_upper_reward: u64,
    pub distribution_ratios: RewardDistributionRatios,
}

/// 奖励分配比例
#[derive(Debug, Clone)]
pub struct RewardDistributionRatios {
    pub project_ratio: f64,
    pub upper_ratio: f64,
    pub upper_upper_ratio: f64,
}

/// 池子信息管理器 - 统一管理池子相关信息
pub struct PoolInfoManager;

impl PoolInfoManager {
    /// 获取已知池子映射
    pub fn get_known_pools() -> std::collections::HashMap<String, String> {
        let mut pools = std::collections::HashMap::new();

        // SOL相关主要池子
        let sol_mint = constants::SOL_MINT;
        let usdc_mint = constants::USDC_MINT_STANDARD;
        let usdt_mint = "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB";
        let coinfair_mint = "CF1Ms9vjvGEiSHqoj1jLadoLNXD9EqtnR6TZp1w8CeHz";

        // 添加主要交易对
        pools.insert(
            format!("{}_{}", sol_mint, usdc_mint),
            "58oQChx4yWmvKdwLLZzBi4ChoCc2fqCUWBkwMihLYQo2".to_string(),
        );
        pools.insert(
            format!("{}_{}", sol_mint, usdt_mint),
            "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM".to_string(),
        );
        pools.insert(
            format!("{}_{}", usdt_mint, coinfair_mint),
            "EWsjgXuVrcAESbAyBo6Q2JCuuAdotBhp8g7Qhvf8GNek".to_string(),
        );

        pools
    }

    /// 查找池子地址
    pub fn find_pool_address(input_mint: &str, output_mint: &str) -> Option<String> {
        let pool_map = Self::get_known_pools();
        let pair_key1 = format!("{}_{}", input_mint, output_mint);
        let pair_key2 = format!("{}_{}", output_mint, input_mint);

        pool_map.get(&pair_key1).or_else(|| pool_map.get(&pair_key2)).cloned()
    }

    /// 计算池子地址使用PDA
    pub fn calculate_pool_address_pda(input_mint: &str, output_mint: &str) -> Result<String> {
        let raydium_program_id = ConfigManager::get_raydium_program_id()?;
        let amm_config_index = ConfigManager::get_amm_config_index();

        let input_mint_pubkey = Pubkey::from_str(input_mint)?;
        let output_mint_pubkey = Pubkey::from_str(output_mint)?;

        let (mint0, mint1, _) = TokenUtils::normalize_mint_order(&input_mint_pubkey, &output_mint_pubkey);

        let (amm_config_key, _) = PDACalculator::calculate_amm_config_pda(&raydium_program_id, amm_config_index);
        let (pool_id_account, _) = PDACalculator::calculate_pool_pda(&raydium_program_id, &amm_config_key, &mint0, &mint1);

        Ok(pool_id_account.to_string())
    }
}

/// 错误处理工具 - 统一管理错误处理
pub struct ErrorHandler;

impl ErrorHandler {
    /// 创建标准错误
    pub fn create_error(message: &str) -> anyhow::Error {
        anyhow::anyhow!("{}", message)
    }

    /// 处理账户加载错误
    pub fn handle_account_load_error(account_name: &str) -> anyhow::Error {
        Self::create_error(&format!("无法加载{}账户", account_name))
    }

    /// 处理解析错误
    pub fn handle_parse_error(field_name: &str, error: impl std::fmt::Display) -> anyhow::Error {
        Self::create_error(&format!("解析{}失败: {}", field_name, error))
    }

    /// 处理计算错误
    pub fn handle_calculation_error(operation: &str, error: impl std::fmt::Display) -> anyhow::Error {
        Self::create_error(&format!("{}计算失败: {}", operation, error))
    }
}
