#[cfg(test)]
mod tests {
    use crate::dtos::solana::common::TransactionStatus;
    use crate::dtos::solana::clmm::launch::*;
    use serde_json;
    use validator::Validate;

    #[test]
    fn test_launch_migration_request_serialization() {
        let request = LaunchMigrationRequest {
            meme_token_mint: "So11111111111111111111111111111111111111112".to_string(),
            base_token_mint: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(),
            user_wallet: "8S2bcP66WehuF6cHryfZ7vfFpQWaUhYyAYSy5U3gX4Fy".to_string(),
            config_index: 0,
            initial_price: 1.0,
            open_time: 0,
            tick_lower_price: 0.8,
            tick_upper_price: 1.2,
            meme_token_amount: 1000000000,
            base_token_amount: 1000000,
            max_slippage_percent: 5.0,
            with_metadata: Some(false),
        };

        // 测试序列化
        let json = serde_json::to_string(&request).unwrap();
        assert!(!json.is_empty());
        assert!(json.contains("So11111111111111111111111111111111111111112"));

        // 测试反序列化
        let deserialized: LaunchMigrationRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.meme_token_mint, request.meme_token_mint);
        assert_eq!(deserialized.base_token_mint, request.base_token_mint);
        assert_eq!(deserialized.user_wallet, request.user_wallet);
        assert_eq!(deserialized.initial_price, request.initial_price);
    }

    #[test]
    fn test_launch_migration_request_validation_success() {
        let request = LaunchMigrationRequest {
            meme_token_mint: "So11111111111111111111111111111111111111112".to_string(),
            base_token_mint: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(),
            user_wallet: "8S2bcP66WehuF6cHryfZ7vfFpQWaUhYyAYSy5U3gX4Fy".to_string(),
            config_index: 0,
            initial_price: 1.0,
            open_time: 0,
            tick_lower_price: 0.8,
            tick_upper_price: 1.2,
            meme_token_amount: 1000000000,
            base_token_amount: 1000000,
            max_slippage_percent: 5.0,
            with_metadata: Some(false),
        };

        let validation_result = request.validate();
        assert!(validation_result.is_ok(), "有效请求应该通过验证");
    }

    #[test]
    fn test_launch_migration_request_validation_invalid_token_mint() {
        let request = LaunchMigrationRequest {
            meme_token_mint: "short".to_string(), // 太短
            base_token_mint: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(),
            user_wallet: "8S2bcP66WehuF6cHryfZ7vfFpQWaUhYyAYSy5U3gX4Fy".to_string(),
            config_index: 0,
            initial_price: 1.0,
            open_time: 0,
            tick_lower_price: 0.8,
            tick_upper_price: 1.2,
            meme_token_amount: 1000000000,
            base_token_amount: 1000000,
            max_slippage_percent: 5.0,
            with_metadata: Some(false),
        };

        let validation_result = request.validate();
        assert!(validation_result.is_err(), "无效的token mint应该验证失败");
    }

    #[test]
    fn test_launch_migration_request_validation_invalid_config_index() {
        let request = LaunchMigrationRequest {
            meme_token_mint: "So11111111111111111111111111111111111111112".to_string(),
            base_token_mint: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(),
            user_wallet: "8S2bcP66WehuF6cHryfZ7vfFpQWaUhYyAYSy5U3gX4Fy".to_string(),
            config_index: 999, // 超出范围
            initial_price: 1.0,
            open_time: 0,
            tick_lower_price: 0.8,
            tick_upper_price: 1.2,
            meme_token_amount: 1000000000,
            base_token_amount: 1000000,
            max_slippage_percent: 5.0,
            with_metadata: Some(false),
        };

        let validation_result = request.validate();
        assert!(validation_result.is_err(), "无效的config_index应该验证失败");
    }

    #[test]
    fn test_launch_migration_request_validation_invalid_price() {
        let request = LaunchMigrationRequest {
            meme_token_mint: "So11111111111111111111111111111111111111112".to_string(),
            base_token_mint: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(),
            user_wallet: "8S2bcP66WehuF6cHryfZ7vfFpQWaUhYyAYSy5U3gX4Fy".to_string(),
            config_index: 0,
            initial_price: -1.0, // 负价格
            open_time: 0,
            tick_lower_price: 0.8,
            tick_upper_price: 1.2,
            meme_token_amount: 1000000000,
            base_token_amount: 1000000,
            max_slippage_percent: 5.0,
            with_metadata: Some(false),
        };

        let validation_result = request.validate();
        assert!(validation_result.is_err(), "负价格应该验证失败");
    }

    #[test]
    fn test_launch_migration_request_validation_zero_amount() {
        let request = LaunchMigrationRequest {
            meme_token_mint: "So11111111111111111111111111111111111111112".to_string(),
            base_token_mint: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(),
            user_wallet: "8S2bcP66WehuF6cHryfZ7vfFpQWaUhYyAYSy5U3gX4Fy".to_string(),
            config_index: 0,
            initial_price: 1.0,
            open_time: 0,
            tick_lower_price: 0.8,
            tick_upper_price: 1.2,
            meme_token_amount: 0, // 零金额
            base_token_amount: 1000000,
            max_slippage_percent: 5.0,
            with_metadata: Some(false),
        };

        let validation_result = request.validate();
        assert!(validation_result.is_err(), "零金额应该验证失败");
    }

    #[test]
    fn test_launch_migration_request_validation_invalid_slippage() {
        let request = LaunchMigrationRequest {
            meme_token_mint: "So11111111111111111111111111111111111111112".to_string(),
            base_token_mint: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(),
            user_wallet: "8S2bcP66WehuF6cHryfZ7vfFpQWaUhYyAYSy5U3gX4Fy".to_string(),
            config_index: 0,
            initial_price: 1.0,
            open_time: 0,
            tick_lower_price: 0.8,
            tick_upper_price: 1.2,
            meme_token_amount: 1000000000,
            base_token_amount: 1000000,
            max_slippage_percent: 150.0, // 超出范围
            with_metadata: Some(false),
        };

        let validation_result = request.validate();
        assert!(validation_result.is_err(), "超出范围的滑点应该验证失败");
    }

    #[test]
    fn test_launch_migration_response_serialization() {
        let response = LaunchMigrationResponse {
            transaction: "base64_transaction_data".to_string(),
            transaction_message: "Test transaction".to_string(),
            pool_address: "pool123".to_string(),
            amm_config_address: "config123".to_string(),
            token_vault_0: "vault0_123".to_string(),
            token_vault_1: "vault1_123".to_string(),
            observation_address: "obs123".to_string(),
            tickarray_bitmap_extension: "bitmap123".to_string(),
            position_nft_mint: "nft123".to_string(),
            position_key: "position123".to_string(),
            liquidity: "1000000".to_string(),
            initial_price: 1.0,
            sqrt_price_x64: "18446744073709551616".to_string(),
            initial_tick: 0,
            tick_lower_index: -1000,
            tick_upper_index: 1000,
            amount_0: 1000000000,
            amount_1: 1000000,
            timestamp: 1640995200,
        };

        // 测试序列化
        let json = serde_json::to_string(&response).unwrap();
        assert!(!json.is_empty());
        assert!(json.contains("base64_transaction_data"));
        assert!(json.contains("pool123"));

        // 测试反序列化
        let deserialized: LaunchMigrationResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.transaction, response.transaction);
        assert_eq!(deserialized.pool_address, response.pool_address);
        assert_eq!(deserialized.liquidity, response.liquidity);
    }

    #[test]
    fn test_launch_migration_and_send_transaction_response_serialization() {
        let response = LaunchMigrationAndSendTransactionResponse {
            signature: "signature123".to_string(),
            status: TransactionStatus::Finalized,
            explorer_url: "https://explorer.solana.com/tx/signature123".to_string(),
            pool_address: "pool123".to_string(),
            amm_config_address: "config123".to_string(),
            token_vault_0: "vault0_123".to_string(),
            token_vault_1: "vault1_123".to_string(),
            observation_address: "obs123".to_string(),
            tickarray_bitmap_extension: "bitmap123".to_string(),
            position_nft_mint: "nft123".to_string(),
            position_key: "position123".to_string(),
            liquidity: "1000000".to_string(),
            initial_price: 1.0,
            sqrt_price_x64: "18446744073709551616".to_string(),
            initial_tick: 0,
            tick_lower_index: -1000,
            tick_upper_index: 1000,
            amount_0: 1000000000,
            amount_1: 1000000,
            timestamp: 1640995200,
        };

        // 测试序列化
        let json = serde_json::to_string(&response).unwrap();
        assert!(!json.is_empty());
        assert!(json.contains("signature123"));
        assert!(json.contains("Finalized"));
        assert!(json.contains("explorer.solana.com"));

        // 测试反序列化
        let deserialized: LaunchMigrationAndSendTransactionResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.signature, response.signature);
        assert_eq!(deserialized.status, response.status);
        assert_eq!(deserialized.explorer_url, response.explorer_url);
    }

    #[test]
    fn test_migration_addresses_debug() {
        let addresses = MigrationAddresses {
            pool_address: "pool123".to_string(),
            amm_config_address: "config123".to_string(),
            token_vault_0: "vault0_123".to_string(),
            token_vault_1: "vault1_123".to_string(),
            observation_address: "obs123".to_string(),
            tickarray_bitmap_extension: "bitmap123".to_string(),
            position_nft_mint: "nft123".to_string(),
            position_key: "position123".to_string(),
            liquidity: 1000000,
            actual_initial_price: 1.0,
            sqrt_price_x64: 18446744073709551616u128,
            initial_tick: 0,
            tick_lower_index: -1000,
            tick_upper_index: 1000,
            amount_0: 1000000000,
            amount_1: 1000000,
        };

        // 测试Debug trait
        let debug_str = format!("{:?}", addresses);
        assert!(debug_str.contains("pool123"));
        assert!(debug_str.contains("1000000"));
        assert!(debug_str.contains("18446744073709551616"));
    }

    // 边界条件测试
    #[test]
    fn test_extreme_values() {
        let request = LaunchMigrationRequest {
            meme_token_mint: "So11111111111111111111111111111111111111112".to_string(),
            base_token_mint: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(),
            user_wallet: "8S2bcP66WehuF6cHryfZ7vfFpQWaUhYyAYSy5U3gX4Fy".to_string(),
            config_index: 100, // 最大允许值
            initial_price: f64::MAX,
            open_time: u64::MAX,
            tick_lower_price: f64::MIN_POSITIVE,
            tick_upper_price: f64::MAX,
            meme_token_amount: u64::MAX,
            base_token_amount: u64::MAX,
            max_slippage_percent: 100.0, // 最大允许值
            with_metadata: Some(true),
        };

        // 应该通过基本的结构验证
        let json = serde_json::to_string(&request).unwrap();
        let deserialized: LaunchMigrationRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.config_index, 100);
        assert_eq!(deserialized.meme_token_amount, u64::MAX);
    }

    // JSON格式兼容性测试
    #[test]
    fn test_json_format_compatibility() {
        let json_input = r#"{
            "meme_token_mint": "So11111111111111111111111111111111111111112",
            "base_token_mint": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
            "user_wallet": "8S2bcP66WehuF6cHryfZ7vfFpQWaUhYyAYSy5U3gX4Fy",
            "config_index": 0,
            "initial_price": 1.0,
            "open_time": 0,
            "tick_lower_price": 0.8,
            "tick_upper_price": 1.2,
            "meme_token_amount": 1000000000,
            "base_token_amount": 1000000,
            "max_slippage_percent": 5.0,
            "with_metadata": false
        }"#;

        let request: LaunchMigrationRequest = serde_json::from_str(json_input).unwrap();
        assert_eq!(request.meme_token_mint, "So11111111111111111111111111111111111111112");
        assert_eq!(request.initial_price, 1.0);
        assert_eq!(request.with_metadata, Some(false));
    }
}
