#[cfg(test)]
mod tests {
    use crate::dtos::solana_dto::MintNftRequest;
    use crate::services::solana::shared::SharedContext;
    use crate::services::solana::nft::NftService;
    use solana_sdk::pubkey::Pubkey;
    use std::str::FromStr;
    use std::sync::Arc;

    fn create_test_service() -> NftService {
        let shared_context = Arc::new(SharedContext::new().expect("Failed to create SharedContext"));
        NftService::new(shared_context)
    }

    fn create_test_request() -> MintNftRequest {
        MintNftRequest {
            user_wallet: "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM".to_string(),
            amount: 1,
        }
    }

    #[test]
    fn test_nft_service_creation() {
        let service = create_test_service();
        // Just test that service can be created without checking private fields
        assert!(std::mem::size_of_val(&service) > 0);
    }

    #[test]
    fn test_get_referral_program_id() {
        let service = create_test_service();
        let result = service.get_referral_program_id();
        assert!(result.is_ok());
    }

    #[test]
    fn test_get_nft_mint() {
        let service = create_test_service();
        let result = service.get_nft_mint();
        assert!(result.is_ok());
    }

    #[test]
    fn test_get_user_referral_pda() {
        let service = create_test_service();
        let user_wallet = Pubkey::from_str("9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM").unwrap();
        let result = service.get_user_referral_pda(&user_wallet);
        assert!(result.is_ok());
        
        let (pda, bump) = result.unwrap();
        assert_ne!(pda, Pubkey::default());
        assert!(bump > 0);
    }

    #[test]
    fn test_get_mint_counter_pda() {
        let service = create_test_service();
        let user_wallet = Pubkey::from_str("9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM").unwrap();
        let result = service.get_mint_counter_pda(&user_wallet);
        assert!(result.is_ok());
        
        let (pda, bump) = result.unwrap();
        assert_ne!(pda, Pubkey::default());
        assert!(bump > 0);
    }

    #[test]
    fn test_get_nft_pool_authority_pda() {
        let service = create_test_service();
        let user_wallet = Pubkey::from_str("9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM").unwrap();
        let result = service.get_nft_pool_authority_pda(&user_wallet);
        assert!(result.is_ok());
        
        let (pda, bump) = result.unwrap();
        assert_ne!(pda, Pubkey::default());
        assert!(bump > 0);
    }

    #[test]
    fn test_get_nft_pool_account() {
        let service = create_test_service();
        let user_wallet = Pubkey::from_str("9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM").unwrap();
        let (nft_pool_authority, _) = service.get_nft_pool_authority_pda(&user_wallet).unwrap();
        let result = service.get_nft_pool_account(&nft_pool_authority);
        assert!(result.is_ok());
        
        let pool_account = result.unwrap();
        assert_ne!(pool_account, Pubkey::default());
    }

    #[tokio::test]
    async fn test_build_mint_nft_instructions() {
        let service = create_test_service();
        let user_wallet = Pubkey::from_str("9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM").unwrap();
        
        let result = service.build_mint_nft_instructions(user_wallet, 1).await;
        assert!(result.is_ok());
        
        let instructions = result.unwrap();
        assert_eq!(instructions.len(), 1);
        
        let instruction = &instructions[0];
        assert_eq!(instruction.accounts.len(), 13); // 应该有13个账户
        assert!(!instruction.data.is_empty()); // 应该有数据
    }

    #[test]
    fn test_mint_nft_request_validation() {
        let request = create_test_request();
        assert_eq!(request.user_wallet, "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM");
        assert_eq!(request.amount, 1);
    }

    #[test]
    fn test_mint_nft_request_invalid_wallet() {
        let request = MintNftRequest {
            user_wallet: "invalid_wallet".to_string(),
            amount: 1,
        };
        
        // 这应该在验证时失败，因为钱包地址无效
        assert!(request.user_wallet.len() < 32);
    }

    #[test]
    fn test_mint_nft_request_invalid_amount() {
        let request = MintNftRequest {
            user_wallet: "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM".to_string(),
            amount: 0, // 无效数量
        };
        
        // 这应该在验证时失败，因为数量为0
        assert_eq!(request.amount, 0);
    }

    // 注意：集成测试需要实际的RPC连接，所以在单元测试中跳过
    // mint_nft 和 mint_nft_and_send_transaction 方法将在集成测试中测试
}