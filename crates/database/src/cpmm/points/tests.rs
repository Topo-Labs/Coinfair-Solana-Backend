use super::model::UserPointsSummary;

#[cfg(test)]
mod points_tests {
    use super::*;

    #[test]
    fn test_new_from_first_swap() {
        let user = UserPointsSummary::new_from_first_swap("wallet_test_1".to_string());

        assert_eq!(user.user_wallet, "wallet_test_1");
        assert_eq!(user.points_from_transaction, 200);
        assert_eq!(user.points_from_nft_claimed, 0);
        assert_eq!(user.point_from_claim_nft, 0);
        assert_eq!(user.point_from_follow_x_account, 0);
        assert_eq!(user.point_from_join_telegram, 0);
        assert_eq!(user.record_init_from, "swap_event");
        assert_eq!(user.record_update_from, "swap_event");
        assert_eq!(user.total_points(), 200);

        println!("✅ 测试通过: new_from_first_swap");
    }

    #[test]
    fn test_new_from_claim_nft_upper() {
        let user = UserPointsSummary::new_from_claim_nft_upper("wallet_upper_1".to_string());

        assert_eq!(user.user_wallet, "wallet_upper_1");
        assert_eq!(user.points_from_transaction, 0);
        assert_eq!(user.points_from_nft_claimed, 300);
        assert_eq!(user.point_from_claim_nft, 0);
        assert_eq!(user.record_init_from, "claim_nft_event");
        assert_eq!(user.total_points(), 300);

        println!("✅ 测试通过: new_from_claim_nft_upper");
    }

    #[test]
    fn test_new_from_claim_nft_claimer() {
        let user = UserPointsSummary::new_from_claim_nft_claimer("wallet_claimer_1".to_string());

        assert_eq!(user.user_wallet, "wallet_claimer_1");
        assert_eq!(user.points_from_transaction, 0);
        assert_eq!(user.points_from_nft_claimed, 0);
        assert_eq!(user.point_from_claim_nft, 200);
        assert_eq!(user.record_init_from, "claim_nft_event");
        assert_eq!(user.total_points(), 200);

        println!("✅ 测试通过: new_from_claim_nft_claimer");
    }

    #[test]
    fn test_update_transaction_points() {
        let mut user = UserPointsSummary::new_from_first_swap("wallet_test_2".to_string());

        // 初始状态：首笔交易200积分
        assert_eq!(user.points_from_transaction, 200);

        // 第二笔交易
        user.update_transaction_points();
        assert_eq!(user.points_from_transaction, 210);
        assert_eq!(user.record_update_from, "swap_event");

        // 第三笔交易
        user.update_transaction_points();
        assert_eq!(user.points_from_transaction, 220);

        // 第四笔交易
        user.update_transaction_points();
        assert_eq!(user.points_from_transaction, 230);

        println!("✅ 测试通过: update_transaction_points");
    }

    #[test]
    fn test_update_nft_claimed_points() {
        let mut user = UserPointsSummary::new_from_claim_nft_upper("wallet_upper_2".to_string());

        // 初始状态：首次被领取300积分
        assert_eq!(user.points_from_nft_claimed, 300);

        // 第二次被领取
        user.update_nft_claimed_points();
        assert_eq!(user.points_from_nft_claimed, 600);
        assert_eq!(user.record_update_from, "claim_nft_event");

        // 第三次被领取
        user.update_nft_claimed_points();
        assert_eq!(user.points_from_nft_claimed, 900);

        println!("✅ 测试通过: update_nft_claimed_points");
    }

    #[test]
    fn test_update_claim_nft_points() {
        let mut user = UserPointsSummary::new_from_claim_nft_claimer("wallet_claimer_2".to_string());

        // 初始状态：领取NFT获得200积分
        assert_eq!(user.point_from_claim_nft, 200);

        // 再次调用应该保持200（一次性积分）
        user.update_claim_nft_points();
        assert_eq!(user.point_from_claim_nft, 200);
        assert_eq!(user.record_update_from, "claim_nft_event");

        println!("✅ 测试通过: update_claim_nft_points");
    }

    #[test]
    fn test_total_points_calculation() {
        let mut user = UserPointsSummary::new_from_first_swap("wallet_test_3".to_string());

        // 初始状态：首笔交易200积分
        assert_eq!(user.total_points(), 200);

        // 添加后续交易积分
        user.update_transaction_points(); // +10
        assert_eq!(user.total_points(), 210);

        // 添加NFT被领取积分
        user.update_nft_claimed_points(); // +300
        assert_eq!(user.total_points(), 510);

        // 添加领取NFT积分
        user.update_claim_nft_points(); // +200
        assert_eq!(user.total_points(), 710);

        // 添加关注X账户积分
        user.point_from_follow_x_account = 200;
        assert_eq!(user.total_points(), 910);

        // 添加加入Telegram积分
        user.point_from_join_telegram = 200;
        assert_eq!(user.total_points(), 1110);

        println!("✅ 测试通过: total_points_calculation");
    }

    #[test]
    fn test_complex_scenario() {
        // 模拟复杂场景：用户多次交易、多次NFT被领取
        let mut user = UserPointsSummary::new_from_first_swap("wallet_complex".to_string());

        // 首笔交易
        assert_eq!(user.points_from_transaction, 200);

        // 连续5笔交易
        for _ in 0..5 {
            user.update_transaction_points();
        }
        assert_eq!(user.points_from_transaction, 250); // 200 + 5*10

        // NFT被领取3次
        for _ in 0..3 {
            user.update_nft_claimed_points();
        }
        assert_eq!(user.points_from_nft_claimed, 900); // 3*300

        // 领取NFT（一次性）
        user.update_claim_nft_points();
        assert_eq!(user.point_from_claim_nft, 200);

        // 验证总积分
        assert_eq!(user.total_points(), 250 + 900 + 200); // 1350

        // 验证记录来源被正确更新
        assert_eq!(user.record_init_from, "swap_event");
        assert_eq!(user.record_update_from, "claim_nft_event");

        println!("✅ 测试通过: complex_scenario");
    }

    #[test]
    fn test_multiple_users_scenario() {
        // 场景1: 用户A - 纯交易用户
        let mut user_a = UserPointsSummary::new_from_first_swap("wallet_a".to_string());
        for _ in 0..10 {
            user_a.update_transaction_points();
        }
        assert_eq!(user_a.points_from_transaction, 300); // 200 + 10*10
        assert_eq!(user_a.total_points(), 300);

        // 场景2: 用户B - NFT铸造者
        let mut user_b = UserPointsSummary::new_from_claim_nft_upper("wallet_b".to_string());
        for _ in 0..5 {
            user_b.update_nft_claimed_points();
        }
        assert_eq!(user_b.points_from_nft_claimed, 1800); // 300 + 5*300
        assert_eq!(user_b.total_points(), 1800);

        // 场景3: 用户C - NFT领取者
        let user_c = UserPointsSummary::new_from_claim_nft_claimer("wallet_c".to_string());
        assert_eq!(user_c.point_from_claim_nft, 200);
        assert_eq!(user_c.total_points(), 200);

        // 场景4: 用户D - 全能用户
        let mut user_d = UserPointsSummary::new_from_first_swap("wallet_d".to_string());
        for _ in 0..3 {
            user_d.update_transaction_points();
        }
        for _ in 0..2 {
            user_d.update_nft_claimed_points();
        }
        user_d.update_claim_nft_points();
        user_d.point_from_follow_x_account = 200;
        user_d.point_from_join_telegram = 200;

        assert_eq!(user_d.points_from_transaction, 230); // 200 + 3*10
        assert_eq!(user_d.points_from_nft_claimed, 600); // 2*300
        assert_eq!(user_d.point_from_claim_nft, 200);
        assert_eq!(user_d.point_from_follow_x_account, 200);
        assert_eq!(user_d.point_from_join_telegram, 200);
        assert_eq!(user_d.total_points(), 1430);

        println!("✅ 测试通过: multiple_users_scenario");
    }

    #[test]
    fn test_edge_cases() {
        // 边界情况1: 用户只有社交积分
        let mut user1 = UserPointsSummary::new_from_first_swap("wallet_edge_1".to_string());
        user1.points_from_transaction = 0; // 重置交易积分
        user1.point_from_follow_x_account = 200;
        user1.point_from_join_telegram = 200;
        assert_eq!(user1.total_points(), 400);

        // 边界情况2: 用户0积分
        let mut user2 = UserPointsSummary::new_from_first_swap("wallet_edge_2".to_string());
        user2.points_from_transaction = 0;
        assert_eq!(user2.total_points(), 0);

        // 边界情况3: 用户单项最大积分
        let mut user3 = UserPointsSummary::new_from_first_swap("wallet_edge_3".to_string());
        // 模拟1000笔交易
        user3.points_from_transaction = 200 + 999 * 10; // 10190
        assert_eq!(user3.points_from_transaction, 10190);

        println!("✅ 测试通过: edge_cases");
    }
}
