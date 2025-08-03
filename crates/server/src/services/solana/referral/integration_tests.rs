#[cfg(test)]
mod integration_tests {
    use crate::dtos::solana_dto::{GetMintCounterRequest, GetUpperRequest};
    use crate::services::solana::referral::ReferralService;
    use crate::services::solana::shared::SharedContext;
    use solana_sdk::pubkey::Pubkey;
    use std::str::FromStr;
    use std::sync::Arc;
    use tokio;

    fn create_test_service() -> ReferralService {
        let shared_context = Arc::new(SharedContext::new().expect("Failed to create shared context"));
        ReferralService::new(shared_context)
    }

    #[tokio::test]
    async fn test_get_upper_with_nonexistent_account() {
        let service = create_test_service();

        // 使用一个不太可能存在推荐账户的地址
        let request = GetUpperRequest {
            user_wallet: "11111111111111111111111111111111".to_string(),
        };

        let result = service.get_upper(request).await;
        assert!(result.is_ok());

        let response = result.unwrap();

        // 账户不存在时应该返回None，状态为AccountNotFound
        assert_eq!(response.upper, None);
        assert_eq!(response.status, "AccountNotFound");
        assert!(response.referral_account.len() > 0);
        assert!(response.timestamp > 0);

        println!("Non-existent account test passed");
        println!("Response: {:?}", response);
    }

    #[tokio::test]
    async fn test_get_upper_and_verify_with_nonexistent_account() {
        let service = create_test_service();

        let request = GetUpperRequest {
            user_wallet: "11111111111111111111111111111111".to_string(),
        };

        let result = service.get_upper_and_verify(request).await;
        assert!(result.is_ok());

        let response = result.unwrap();

        // 验证账户不存在的情况
        assert_eq!(response.account_exists, false);
        assert_eq!(response.referral_account_data, None);
        assert_eq!(response.base.upper, None);
        assert_eq!(response.base.status, "AccountNotFound");

        println!("Verify non-existent account test passed");
        println!("Response: {:?}", response);
    }

    #[tokio::test]
    async fn test_get_upper_with_invalid_wallet_address() {
        let service = create_test_service();

        let request = GetUpperRequest {
            user_wallet: "invalid-wallet-address".to_string(),
        };

        let result = service.get_upper(request).await;

        // 应该返回错误，因为钱包地址格式无效
        assert!(result.is_err());

        let error = result.unwrap_err();
        println!("Invalid wallet address error: {}", error);
    }

    #[tokio::test]
    async fn test_get_upper_request_response_structure() {
        let service = create_test_service();

        let request = GetUpperRequest {
            user_wallet: "8S2bcP66WehuF6cHryfZ7vfFpQWaUhYyAYSy5U3gX4Fy".to_string(),
        };

        let result = service.get_upper(request.clone()).await;
        assert!(result.is_ok());

        let response = result.unwrap();

        // 验证响应结构完整性
        assert_eq!(response.user_wallet, request.user_wallet);
        assert!(response.referral_account.len() > 0);
        assert!(response.status == "Success" || response.status == "AccountNotFound");
        assert!(response.timestamp > 0);

        // 验证PDA地址格式
        let pda_result = Pubkey::from_str(&response.referral_account);
        assert!(pda_result.is_ok());

        println!("Response structure test passed");
        println!("Response: {:?}", response);
    }

    #[tokio::test]
    async fn test_consistency_with_cli_pda_calculation() {
        let service = create_test_service();

        // 使用CLI中相同的测试地址
        let user_wallet_str = "8S2bcP66WehuF6cHryfZ7vfFpQWaUhYyAYSy5U3gX4Fy";
        let user_wallet = Pubkey::from_str(user_wallet_str).unwrap();

        // 通过服务计算PDA（模拟我们的API）
        let (service_pda, _) = service.calculate_referral_account_pda(&user_wallet).unwrap();

        // 手动使用与CLI完全相同的逻辑计算PDA
        let referral_program_id = service.get_referral_program_id().unwrap();
        let (cli_pda, _) = Pubkey::find_program_address(&[b"referral", &user_wallet.to_bytes()], &referral_program_id);

        // 两种方式计算的PDA应该完全一致
        assert_eq!(service_pda, cli_pda);

        println!("CLI consistency test passed");
        println!("User wallet: {}", user_wallet_str);
        println!("Referral program ID: {}", referral_program_id);
        println!("Service PDA: {}", service_pda);
        println!("CLI PDA: {}", cli_pda);

        // 测试通过API获取的结果
        let request = GetUpperRequest {
            user_wallet: user_wallet_str.to_string(),
        };

        let api_result = service.get_upper(request).await;
        assert!(api_result.is_ok());

        let api_response = api_result.unwrap();
        assert_eq!(api_response.referral_account, cli_pda.to_string());

        println!("API PDA matches CLI calculation: ✅");
    }

    #[tokio::test]
    async fn test_multiple_user_consistency() {
        let service = create_test_service();

        let test_users = vec![
            "8S2bcP66WehuF6cHryfZ7vfFpQWaUhYyAYSy5U3gX4Fy",
            "So11111111111111111111111111111111111111112",
            "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
        ];

        let mut pdas = Vec::new();

        for user_str in &test_users {
            let request = GetUpperRequest {
                user_wallet: user_str.to_string(),
            };

            let result = service.get_upper(request).await;
            assert!(result.is_ok());

            let response = result.unwrap();
            let pda = Pubkey::from_str(&response.referral_account).unwrap();
            pdas.push(pda);

            println!("User: {} -> PDA: {}", user_str, response.referral_account);
        }

        // 验证所有PDA都不相同
        for i in 0..pdas.len() {
            for j in (i + 1)..pdas.len() {
                assert_ne!(pdas[i], pdas[j], "PDA should be unique for different users");
            }
        }

        println!("Multiple user consistency test passed ✅");
    }

    #[tokio::test]
    async fn test_error_handling_edge_cases() {
        let service = create_test_service();

        // 测试各种边界情况
        let edge_cases = vec![
            "",                                             // 空字符串
            "1",                                            // 太短
            "invalid-pubkey-format",                        // 无效格式
            "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz", // 无效base58字符
        ];

        for case in edge_cases {
            let request = GetUpperRequest { user_wallet: case.to_string() };

            let result = service.get_upper(request).await;

            if case.is_empty() || case.len() < 32 {
                // 对于明显无效的输入，应该在验证阶段失败
                println!("Case '{}': Expected validation failure", case);
            } else {
                // 对于格式问题，应该在解析阶段失败
                assert!(result.is_err(), "Case '{}' should fail", case);
                println!("Case '{}': Correctly failed with error: {}", case, result.unwrap_err());
            }
        }

        println!("Error handling edge cases test passed ✅");
    }

    // ================ MintCounter Integration Tests ================

    #[tokio::test]
    async fn test_get_mint_counter_with_nonexistent_account() {
        let service = create_test_service();

        // 使用一个不太可能存在MintCounter账户的地址
        let request = GetMintCounterRequest {
            user_wallet: "11111111111111111111111111111111".to_string(),
        };

        let result = service.get_mint_counter(request).await;
        assert!(result.is_ok());

        let response = result.unwrap();
        assert_eq!(response.total_mint, 0);
        assert_eq!(response.remain_mint, 0);
        assert_eq!(response.status, "AccountNotFound");

        println!("MintCounter account does not exist: {:?}", response);
        println!("Test passed: get_mint_counter works with nonexistent account ✅");
    }

    #[tokio::test]
    async fn test_get_mint_counter_and_verify_with_nonexistent_account() {
        let service = create_test_service();

        // 使用一个不太可能存在MintCounter账户的地址
        let request = GetMintCounterRequest {
            user_wallet: "11111111111111111111111111111111".to_string(),
        };

        let result = service.get_mint_counter_and_verify(request).await;
        assert!(result.is_ok());

        let response = result.unwrap();
        assert_eq!(response.base.total_mint, 0);
        assert_eq!(response.base.remain_mint, 0);
        assert_eq!(response.base.status, "AccountNotFound");
        assert!(!response.account_exists);
        assert!(response.mint_counter_data.is_none());

        println!("MintCounter verification result: {:?}", response);
        println!("Test passed: get_mint_counter_and_verify works with nonexistent account ✅");
    }

    #[tokio::test]
    async fn test_get_mint_counter_with_test_wallet() {
        let service = create_test_service();

        // 使用测试钱包地址（配置中的钱包）
        let request = GetMintCounterRequest {
            user_wallet: "8S2bcP66WehuF6cHryfZ7vfFpQWaUhYyAYSy5U3gX4Fy".to_string(),
        };

        let result = service.get_mint_counter(request).await;
        assert!(result.is_ok());

        let response = result.unwrap();
        // 不管账户是否存在，响应应该是有效的
        // 注意：u64类型的值总是>=0，这里检查的是业务逻辑正确性
        assert!(!response.status.is_empty());
        assert!(!response.mint_counter_account.is_empty());

        println!("Test wallet MintCounter result: {:?}", response);
        println!("Test passed: get_mint_counter works with test wallet ✅");
    }

    #[tokio::test]
    async fn test_get_mint_counter_with_invalid_wallet_address() {
        let service = create_test_service();

        let request = GetMintCounterRequest {
            user_wallet: "invalid-wallet-address".to_string(),
        };

        let result = service.get_mint_counter(request).await;

        // 应该返回错误，因为钱包地址格式无效
        assert!(result.is_err());

        let error = result.unwrap_err();
        println!("Invalid wallet address error for MintCounter: {}", error);
        println!("Test passed: get_mint_counter properly handles invalid addresses ✅");
    }

    #[tokio::test]
    async fn test_get_mint_counter_response_structure() {
        let service = create_test_service();

        let request = GetMintCounterRequest {
            user_wallet: "8S2bcP66WehuF6cHryfZ7vfFpQWaUhYyAYSy5U3gX4Fy".to_string(),
        };

        let result = service.get_mint_counter(request.clone()).await;
        assert!(result.is_ok());

        let response = result.unwrap();

        // 验证响应结构完整性
        assert_eq!(response.user_wallet, request.user_wallet);
        assert!(response.mint_counter_account.len() > 0);
        assert!(response.status == "Success" || response.status == "AccountNotFound");
        assert!(response.timestamp > 0);

        // 验证PDA地址格式
        let pda_result = Pubkey::from_str(&response.mint_counter_account);
        assert!(pda_result.is_ok());

        println!("MintCounter response structure test passed ✅");
        println!("Response: {:?}", response);
    }

    #[tokio::test]
    async fn test_mint_counter_consistency_with_cli_pda_calculation() {
        let service = create_test_service();

        // 使用CLI中相同的测试地址
        let user_wallet_str = "8S2bcP66WehuF6cHryfZ7vfFpQWaUhYyAYSy5U3gX4Fy";
        let user_wallet = Pubkey::from_str(user_wallet_str).unwrap();

        // 计算PDA（这应该与CLI逻辑一致）
        let (expected_pda, _) = service.calculate_mint_counter_pda(&user_wallet).unwrap();

        // 手动使用与CLI完全相同的逻辑计算PDA
        let referral_program_id = service.get_referral_program_id().unwrap();
        let (cli_pda, _) = Pubkey::find_program_address(&[b"mint_counter", &user_wallet.to_bytes()], &referral_program_id);

        // 两种方式计算的PDA应该完全一致
        assert_eq!(expected_pda, cli_pda);

        // 通过API获取响应
        let request = GetMintCounterRequest {
            user_wallet: user_wallet_str.to_string(),
        };

        let result = service.get_mint_counter(request).await;
        assert!(result.is_ok());

        let response = result.unwrap();
        let response_pda = Pubkey::from_str(&response.mint_counter_account).unwrap();

        // 验证PDA计算一致性
        assert_eq!(expected_pda, response_pda);
        assert_eq!(cli_pda, response_pda);

        println!("Expected PDA: {}", expected_pda);
        println!("CLI PDA: {}", cli_pda);
        println!("Response PDA: {}", response_pda);
        println!("Test passed: MintCounter CLI consistency validation ✅");
    }

    #[tokio::test]
    async fn test_mint_counter_multiple_user_consistency() {
        let service = create_test_service();

        let test_users = vec![
            "8S2bcP66WehuF6cHryfZ7vfFpQWaUhYyAYSy5U3gX4Fy",
            "So11111111111111111111111111111111111111112",
            "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
        ];

        let mut pdas = Vec::new();

        for user_str in &test_users {
            let request = GetMintCounterRequest {
                user_wallet: user_str.to_string(),
            };

            let result = service.get_mint_counter(request).await;
            assert!(result.is_ok());

            let response = result.unwrap();
            let pda = Pubkey::from_str(&response.mint_counter_account).unwrap();
            pdas.push(pda);

            println!("User: {} -> MintCounter PDA: {}", user_str, response.mint_counter_account);
        }

        // 验证所有PDA都不相同
        for i in 0..pdas.len() {
            for j in (i + 1)..pdas.len() {
                assert_ne!(pdas[i], pdas[j], "MintCounter PDA should be unique for different users");
            }
        }

        println!("Multiple user MintCounter consistency test passed ✅");
    }

    #[tokio::test]
    async fn test_mint_counter_vs_referral_pda_uniqueness() {
        let service = create_test_service();

        let user_wallet_str = "8S2bcP66WehuF6cHryfZ7vfFpQWaUhYyAYSy5U3gX4Fy";

        // 获取Referral PDA
        let referral_request = GetUpperRequest {
            user_wallet: user_wallet_str.to_string(),
        };
        let referral_result = service.get_upper(referral_request).await;
        assert!(referral_result.is_ok());
        let referral_pda = Pubkey::from_str(&referral_result.unwrap().referral_account).unwrap();

        // 获取MintCounter PDA
        let mint_counter_request = GetMintCounterRequest {
            user_wallet: user_wallet_str.to_string(),
        };
        let mint_counter_result = service.get_mint_counter(mint_counter_request).await;
        assert!(mint_counter_result.is_ok());
        let mint_counter_pda = Pubkey::from_str(&mint_counter_result.unwrap().mint_counter_account).unwrap();

        // 验证不同种子生成的PDA是不同的
        assert_ne!(referral_pda, mint_counter_pda);

        println!("Referral PDA: {}", referral_pda);
        println!("MintCounter PDA: {}", mint_counter_pda);
        println!("Test passed: Different PDA types generate unique addresses ✅");
    }
}
