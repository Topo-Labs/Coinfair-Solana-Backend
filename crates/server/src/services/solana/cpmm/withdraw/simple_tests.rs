#[cfg(test)]
mod tests {
    use crate::dtos::solana::cpmm::withdraw::{CpmmWithdrawRequest, CpmmWithdrawAndSendRequest};
    use solana_sdk::pubkey::Pubkey;
    use std::str::FromStr;

    /// 测试CpmmWithdrawRequest结构体创建和字段访问
    #[test]
    fn test_cpmm_withdraw_request_creation() {
        let request = CpmmWithdrawRequest {
            pool_id: "11111111111111111111111111111112".to_string(),
            user_lp_token: "11111111111111111111111111111113".to_string(),
            lp_token_amount: 1000,
            slippage: Some(0.5),
            user_wallet: "11111111111111111111111111111114".to_string(),
        };

        assert_eq!(request.pool_id, "11111111111111111111111111111112");
        assert_eq!(request.user_lp_token, "11111111111111111111111111111113");
        assert_eq!(request.lp_token_amount, 1000);
        assert_eq!(request.slippage, Some(0.5));
        assert_eq!(request.user_wallet, "11111111111111111111111111111114");
    }

    /// 测试CpmmWithdrawAndSendRequest结构体
    #[test]
    fn test_cpmm_withdraw_and_send_request_creation() {
        let request = CpmmWithdrawAndSendRequest {
            pool_id: "11111111111111111111111111111112".to_string(),
            user_lp_token: "11111111111111111111111111111113".to_string(),
            lp_token_amount: 2000,
            slippage: Some(1.0),
            user_wallet: "11111111111111111111111111111114".to_string(),
        };

        assert_eq!(request.pool_id, "11111111111111111111111111111112");
        assert_eq!(request.user_lp_token, "11111111111111111111111111111113");
        assert_eq!(request.lp_token_amount, 2000);
        assert_eq!(request.slippage, Some(1.0));
        assert_eq!(request.user_wallet, "11111111111111111111111111111114");
    }

    /// 测试Pubkey解析验证
    #[test]
    fn test_pubkey_validation() {
        let valid_pubkey_str = "11111111111111111111111111111112";
        let result = Pubkey::from_str(valid_pubkey_str);
        assert!(result.is_ok());

        let invalid_pubkey_str = "invalid_pubkey";
        let result = Pubkey::from_str(invalid_pubkey_str);
        assert!(result.is_err());
    }

    /// 测试滑点值验证
    #[test]
    fn test_slippage_validation() {
        // 测试有效滑点值
        let valid_slippage_values = vec![0.0, 0.1, 0.5, 1.0, 5.0, 10.0];

        for slippage in valid_slippage_values {
            let request = CpmmWithdrawRequest {
                pool_id: "11111111111111111111111111111112".to_string(),
                user_lp_token: "11111111111111111111111111111113".to_string(),
                lp_token_amount: 1000,
                slippage: Some(slippage),
                user_wallet: "11111111111111111111111111111114".to_string(),
            };

            // 在实际业务中，应该验证滑点在合理范围内
            assert!(request.slippage.unwrap_or(0.0) >= 0.0);
            assert!(request.slippage.unwrap_or(0.0) <= 100.0);
        }
    }

    /// 测试LP代币数量验证
    #[test]
    fn test_lp_token_amount_validation() {
        // 测试不同的LP代币数量
        let test_amounts = vec![1, 100, 1000, 10000, u64::MAX];

        for amount in test_amounts {
            let request = CpmmWithdrawRequest {
                pool_id: "11111111111111111111111111111112".to_string(),
                user_lp_token: "11111111111111111111111111111113".to_string(),
                lp_token_amount: amount,
                slippage: Some(0.5),
                user_wallet: "11111111111111111111111111111114".to_string(),
            };

            assert_eq!(request.lp_token_amount, amount);
            // 在实际业务中，应该验证数量大于0
            // 这里我们只验证结构体能正确存储值
        }
    }

    /// 测试默认滑点处理
    #[test]
    fn test_default_slippage_handling() {
        let request_without_slippage = CpmmWithdrawRequest {
            pool_id: "11111111111111111111111111111112".to_string(),
            user_lp_token: "11111111111111111111111111111113".to_string(),
            lp_token_amount: 1000,
            slippage: None,
            user_wallet: "11111111111111111111111111111114".to_string(),
        };

        // 验证没有滑点时是None
        assert_eq!(request_without_slippage.slippage, None);

        // 业务逻辑中的默认值处理
        let default_slippage = request_without_slippage.slippage.unwrap_or(0.5);
        assert_eq!(default_slippage, 0.5);
    }

    /// 测试JSON序列化和反序列化
    #[test]
    fn test_json_serialization() {
        let request = CpmmWithdrawRequest {
            pool_id: "11111111111111111111111111111112".to_string(),
            user_lp_token: "11111111111111111111111111111113".to_string(),
            lp_token_amount: 1000,
            slippage: Some(0.5),
            user_wallet: "11111111111111111111111111111114".to_string(),
        };

        // 序列化
        let json_result = serde_json::to_string(&request);
        assert!(json_result.is_ok());

        // 反序列化
        let json_str = json_result.unwrap();
        let deserialized_result: Result<CpmmWithdrawRequest, _> = serde_json::from_str(&json_str);
        assert!(deserialized_result.is_ok());

        let deserialized = deserialized_result.unwrap();
        assert_eq!(deserialized.pool_id, request.pool_id);
        assert_eq!(deserialized.user_lp_token, request.user_lp_token);
        assert_eq!(deserialized.lp_token_amount, request.lp_token_amount);
        assert_eq!(deserialized.slippage, request.slippage);
        assert_eq!(deserialized.user_wallet, request.user_wallet);
    }
}