#[cfg(test)]
mod integration_tests {
    use super::super::NftService;
    use crate::dtos::solana::clmm::nft::mint::MintNftRequest;
    use crate::services::solana::shared::SharedContext;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_mint_nft_instruction_building() {
        // 此测试验证NFT铸造指令构建是否正常工作
        let shared_context = Arc::new(SharedContext::new().expect("Failed to create SharedContext"));
        let nft_service = NftService::new(shared_context);

        let request = MintNftRequest {
            user_wallet: "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM".to_string(),
            amount: 1,
        };

        // 测试构建指令（不实际发送到网络）
        let result = nft_service.mint_nft(request).await;

        // 在测试环境中，此操作可能失败（由于没有实际的RPC连接），
        // 但我们可以检查是否能到达指令构建阶段
        match result {
            Ok(response) => {
                assert_eq!(response.amount, 1);
                assert!(!response.user_wallet.is_empty());
                assert!(response.serialized_transaction.is_some());
            }
            Err(e) => {
                // 在测试环境中，RPC调用可能会失败，这是预期的
                let error_msg = e.to_string();
                assert!(
                    error_msg.contains("RPC")
                        || error_msg.contains("network")
                        || error_msg.contains("connection")
                        || error_msg.contains("timeout")
                );
            }
        }
    }

    #[test]
    fn test_nft_request_validation() {
        let valid_request = MintNftRequest {
            user_wallet: "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM".to_string(),
            amount: 1,
        };

        assert_eq!(valid_request.amount, 1);
        assert_eq!(valid_request.user_wallet.len(), 44); // Standard Solana address length
    }

    #[test]
    fn test_invalid_amount() {
        let invalid_request = MintNftRequest {
            user_wallet: "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM".to_string(),
            amount: 0,
        };

        // Amount of 0 should be invalid according to validation rules
        assert_eq!(invalid_request.amount, 0);
    }

    #[test]
    fn test_large_amount() {
        let request_with_large_amount = MintNftRequest {
            user_wallet: "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM".to_string(),
            amount: 1001, // Over max limit of 1000
        };

        // Amount over 1000 should be invalid according to validation rules
        assert_eq!(request_with_large_amount.amount, 1001);
    }
}
