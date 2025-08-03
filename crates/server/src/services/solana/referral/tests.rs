#[cfg(test)]
mod tests {
    use crate::dtos::solana_dto::{GetUpperRequest, GetMintCounterRequest};
    use crate::services::solana::referral::service::{ReferralAccount, MintCounter};
    use crate::services::solana::referral::ReferralService;
    use crate::services::solana::shared::SharedContext;
    use solana_sdk::pubkey::Pubkey;
    use std::str::FromStr;
    use std::sync::Arc;

    fn create_test_service() -> ReferralService {
        let shared_context = Arc::new(SharedContext::new().expect("Failed to create shared context"));
        ReferralService::new(shared_context)
    }

    #[test]
    fn test_get_referral_program_id() {
        let service = create_test_service();
        let program_id = service.get_referral_program_id();

        assert!(program_id.is_ok());
        let program_id = program_id.unwrap();

        // 验证程序ID格式正确
        assert_eq!(program_id.to_string().len(), 44); // Base58 公钥长度
        println!("Referral Program ID: {}", program_id);
    }

    #[test]
    fn test_calculate_referral_account_pda() {
        let service = create_test_service();

        // 使用测试用户钱包地址
        let user_wallet = Pubkey::from_str("8S2bcP66WehuF6cHryfZ7vfFpQWaUhYyAYSy5U3gX4Fy").expect("Invalid test wallet address");

        let result = service.calculate_referral_account_pda(&user_wallet);
        assert!(result.is_ok());

        let (pda, bump) = result.unwrap();

        // 验证PDA地址格式正确
        assert_eq!(pda.to_string().len(), 44);

        println!("User wallet: {}", user_wallet);
        println!("Referral PDA: {}", pda);
        println!("Bump: {}", bump);

        // 验证PDA计算的确定性（多次计算应该得到相同结果）
        let (pda2, bump2) = service.calculate_referral_account_pda(&user_wallet).unwrap();
        assert_eq!(pda, pda2);
        assert_eq!(bump, bump2);
    }

    #[test]
    fn test_calculate_mint_counter_pda() {
        let service = create_test_service();

        // 使用测试用户钱包地址
        let user_wallet = Pubkey::from_str("8S2bcP66WehuF6cHryfZ7vfFpQWaUhYyAYSy5U3gX4Fy").expect("Invalid test wallet address");

        let result = service.calculate_mint_counter_pda(&user_wallet);
        assert!(result.is_ok());

        let (pda, bump) = result.unwrap();

        // 验证PDA地址格式正确
        assert_eq!(pda.to_string().len(), 44);

        println!("User wallet: {}", user_wallet);
        println!("MintCounter PDA: {}", pda);
        println!("Bump: {}", bump);

        // 验证PDA计算的确定性（多次计算应该得到相同结果）
        let (pda2, bump2) = service.calculate_mint_counter_pda(&user_wallet).unwrap();
        assert_eq!(pda, pda2);
        assert_eq!(bump, bump2);
    }

    #[test]
    fn test_mint_counter_request_validation() {
        // 测试有效的请求
        let valid_request = GetMintCounterRequest {
            user_wallet: "8S2bcP66WehuF6cHryfZ7vfFpQWaUhYyAYSy5U3gX4Fy".to_string(),
        };

        // 验证有效地址能够解析
        let pubkey = Pubkey::from_str(&valid_request.user_wallet);
        assert!(pubkey.is_ok());

        println!("Valid MintCounter request: {:?}", valid_request);
    }

    #[test]
    fn test_different_pda_types() {
        let service = create_test_service();
        let user_wallet = Pubkey::from_str("8S2bcP66WehuF6cHryfZ7vfFpQWaUhYyAYSy5U3gX4Fy").unwrap();

        // 测试不同类型的PDA计算
        let (referral_pda, _) = service.calculate_referral_account_pda(&user_wallet).unwrap();
        let (mint_counter_pda, _) = service.calculate_mint_counter_pda(&user_wallet).unwrap();

        // 验证不同种子生成不同的PDA
        assert_ne!(referral_pda, mint_counter_pda);

        println!("Referral PDA: {}", referral_pda);
        println!("MintCounter PDA: {}", mint_counter_pda);
    }

    #[test]
    fn test_calculate_referral_account_pda_different_users() {
        let service = create_test_service();

        // 测试不同用户应该有不同的PDA
        let user1 = Pubkey::from_str("8S2bcP66WehuF6cHryfZ7vfFpQWaUhYyAYSy5U3gX4Fy").expect("Invalid test wallet 1");
        let user2 = Pubkey::from_str("So11111111111111111111111111111111111111112").expect("Invalid test wallet 2");

        let (pda1, _) = service.calculate_referral_account_pda(&user1).unwrap();
        let (pda2, _) = service.calculate_referral_account_pda(&user2).unwrap();

        // 不同用户应该有不同的PDA
        assert_ne!(pda1, pda2);

        println!("User 1 PDA: {}", pda1);
        println!("User 2 PDA: {}", pda2);
    }

    #[test]
    fn test_referral_account_structure() {
        // 测试ReferralAccount结构的完整性
        let user = Pubkey::from_str("8S2bcP66WehuF6cHryfZ7vfFpQWaUhYyAYSy5U3gX4Fy").unwrap();
        let upper = Some(Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap());
        let upper_upper = Some(Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap());
        let nft_mint = Pubkey::from_str("NFTaoszFxtEmGXvHcb8yfkGZxqLPAfwDqLN1mhrV2jM").unwrap();
        let bump = 254u8;

        let referral_account = ReferralAccount {
            user,
            upper,
            upper_upper,
            nft_mint,
            bump,
        };

        // 验证结构体字段
        assert_eq!(referral_account.user, user);
        assert_eq!(referral_account.upper, upper);
        assert_eq!(referral_account.upper_upper, upper_upper);
        assert_eq!(referral_account.nft_mint, nft_mint);
        assert_eq!(referral_account.bump, bump);

        println!("ReferralAccount structure test passed");
    }

    #[test]
    fn test_mint_counter_structure() {
        // 测试MintCounter结构的完整性
        let minter = Pubkey::from_str("8S2bcP66WehuF6cHryfZ7vfFpQWaUhYyAYSy5U3gX4Fy").unwrap();
        let total_mint = 100u64;
        let remain_mint = 50u64;
        let bump = 253u8;

        let mint_counter = MintCounter {
            minter,
            total_mint,
            remain_mint,
            bump,
        };

        // 验证结构体字段
        assert_eq!(mint_counter.minter, minter);
        assert_eq!(mint_counter.total_mint, total_mint);
        assert_eq!(mint_counter.remain_mint, remain_mint);
        assert_eq!(mint_counter.bump, bump);

        println!("MintCounter structure test passed");
        println!("  Minter: {}", mint_counter.minter);
        println!("  Total mint: {}", mint_counter.total_mint);
        println!("  Remain mint: {}", mint_counter.remain_mint);
        println!("  Bump: {}", mint_counter.bump);
    }

    #[tokio::test]
    async fn test_get_upper_request_validation() {
        // 测试有效请求
        let valid_request = GetUpperRequest {
            user_wallet: "8S2bcP66WehuF6cHryfZ7vfFpQWaUhYyAYSy5U3gX4Fy".to_string(),
        };
        assert!(validator::Validate::validate(&valid_request).is_ok());

        // 测试无效请求（地址太短）
        let invalid_request = GetUpperRequest {
            user_wallet: "invalid".to_string(),
        };
        assert!(validator::Validate::validate(&invalid_request).is_err());

        // 测试空地址
        let empty_request = GetUpperRequest { user_wallet: "".to_string() };
        assert!(validator::Validate::validate(&empty_request).is_err());

        println!("Request validation tests passed");
    }

    #[test]
    fn test_pda_seeds_consistency() {
        // 验证PDA seeds与CLI中完全一致
        let service = create_test_service();
        let user_wallet = Pubkey::from_str("8S2bcP66WehuF6cHryfZ7vfFpQWaUhYyAYSy5U3gX4Fy").unwrap();
        let referral_program_id = service.get_referral_program_id().unwrap();

        // 手动计算PDA使用完全相同的seeds
        let (manual_pda, manual_bump) = Pubkey::find_program_address(&[b"referral", user_wallet.as_ref()], &referral_program_id);

        // 通过服务计算PDA
        let (service_pda, service_bump) = service.calculate_referral_account_pda(&user_wallet).unwrap();

        // 应该完全一致
        assert_eq!(manual_pda, service_pda);
        assert_eq!(manual_bump, service_bump);

        println!("PDA seeds consistency test passed");
        println!("Manual PDA: {}", manual_pda);
        println!("Service PDA: {}", service_pda);
        println!("Bump: {}", manual_bump);
    }
}
