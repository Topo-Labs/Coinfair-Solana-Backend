//! SwapV3推荐系统单元测试
//!
//! 测试ReferralManager的核心功能

use crate::solana::{config::ConfigManager, constants::DEFAULT_REFERRAL_PROGRAM_ID, managers::ReferralManager};
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试推荐账户PDA计算
    #[test]
    fn test_referral_pda_calculation() {
        let referral_program_id = Pubkey::from_str(DEFAULT_REFERRAL_PROGRAM_ID).unwrap();
        let user = Pubkey::new_unique();

        let result = ReferralManager::calculate_referral_pda(&referral_program_id, &user);
        assert!(result.is_ok());

        let (pda, bump) = result.unwrap();
        println!("✅ 推荐PDA计算成功: {}, bump: {}", pda, bump);

        // 验证PDA确实是用正确的种子生成的
        let (expected_pda, _) = Pubkey::find_program_address(&[b"referral", user.as_ref()], &referral_program_id);
        assert_eq!(pda, expected_pda);
    }

    /// 测试奖励分配计算
    #[test]
    fn test_reward_distribution_calculation() {
        let total_fee = 10000u64; // 10000 lamports

        let distribution = ReferralManager::calculate_reward_distribution(total_fee);

        println!("✅ 奖励分配计算:");
        println!("   总费用: {}", distribution.total_reward_fee);
        println!("   项目方奖励: {}", distribution.project_reward);
        println!("   上级奖励: {}", distribution.upper_reward);
        println!("   上上级奖励: {}", distribution.upper_upper_reward);

        // 验证分配比例
        assert_eq!(distribution.total_reward_fee, total_fee);

        // 验证分配加起来等于总费用（允许小的舍入误差）
        let total_distributed =
            distribution.project_reward + distribution.upper_reward + distribution.upper_upper_reward;

        // 由于整数除法可能有舍入，允许最多1的误差
        assert!((total_distributed as i64 - total_fee as i64).abs() <= 1);
    }

    /// 测试配置管理器的推荐系统配置
    #[test]
    fn test_config_manager_referral_settings() {
        // 测试推荐系统配置获取
        let referral_program_id = ConfigManager::get_referral_program_id();
        let project_wallet = ConfigManager::get_project_wallet();
        let swap_fee_rate = ConfigManager::get_swap_fee_rate_bps();
        let referral_fee_rate = ConfigManager::get_referral_fee_rate_bps();

        println!("✅ 配置管理器测试:");
        println!("   推荐程序ID: {:?}", referral_program_id);
        println!("   项目方钱包: {:?}", project_wallet);
        println!("   交换费率: {} bps", swap_fee_rate);
        println!("   推荐费率: {} bps", referral_fee_rate);

        // 验证配置值的合理性
        assert!(swap_fee_rate > 0 && swap_fee_rate <= 10000); // 0-100%
        assert!(referral_fee_rate > 0 && referral_fee_rate <= 1000); // 0-10%
        assert!(referral_fee_rate < swap_fee_rate); // 推荐费率应该小于总交换费率
    }

    /// 测试基本参数验证
    #[test]
    fn test_swap_v3_basic_validation() {
        let referral_program_id = Pubkey::from_str(DEFAULT_REFERRAL_PROGRAM_ID).unwrap();
        let payer = Pubkey::new_unique();
        let upper = Pubkey::new_unique();

        let (payer_referral, _) = ReferralManager::calculate_referral_pda(&referral_program_id, &payer).unwrap();
        let (upper_referral, _) = ReferralManager::calculate_referral_pda(&referral_program_id, &upper).unwrap();

        // 验证基本参数合理性
        assert_ne!(payer, upper);
        assert_ne!(payer_referral, upper_referral);

        println!("✅ SwapV3基本参数验证通过");
        println!("   支付者PDA: {}", payer_referral);
        println!("   上级PDA: {}", upper_referral);
    }

    /// 测试边界情况 - 零费用
    #[test]
    fn test_zero_fee_distribution() {
        let distribution = ReferralManager::calculate_reward_distribution(0);

        assert_eq!(distribution.total_reward_fee, 0);
        assert_eq!(distribution.project_reward, 0);
        assert_eq!(distribution.upper_reward, 0);
        assert_eq!(distribution.upper_upper_reward, 0);

        println!("✅ 零费用分配测试通过");
    }

    /// 测试PDA种子的唯一性
    #[test]
    fn test_pda_uniqueness() {
        let referral_program_id = Pubkey::from_str(DEFAULT_REFERRAL_PROGRAM_ID).unwrap();
        let user1 = Pubkey::new_unique();
        let user2 = Pubkey::new_unique();

        let (pda1, _) = ReferralManager::calculate_referral_pda(&referral_program_id, &user1).unwrap();
        let (pda2, _) = ReferralManager::calculate_referral_pda(&referral_program_id, &user2).unwrap();

        // 不同用户应该有不同的PDA
        assert_ne!(pda1, pda2);

        // 同一用户应该有相同的PDA
        let (pda1_again, _) = ReferralManager::calculate_referral_pda(&referral_program_id, &user1).unwrap();
        assert_eq!(pda1, pda1_again);

        println!("✅ PDA唯一性测试通过");
    }
}
