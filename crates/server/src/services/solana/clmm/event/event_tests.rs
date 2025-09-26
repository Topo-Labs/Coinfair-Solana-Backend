#[cfg(test)]
mod tests {
    use super::super::event_service::*;
    use database::Database;
    use std::sync::Arc;

    async fn setup_test_service() -> EventService {
        // 使用测试数据库配置
        // 注意：在实际测试中，我们应该使用测试专用的数据库
        let mongo_uri = std::env::var("MONGO_URI").unwrap_or_else(|_| "mongodb://localhost:27017".to_string());
        let mongo_db = std::env::var("MONGO_DB").unwrap_or_else(|_| "test_db".to_string());

        // 创建一个临时的配置
        let config = Arc::new(utils::AppConfig {
            cargo_env: utils::CargoEnv::Development,
            app_host: "127.0.0.1".to_string(),
            app_port: 8000,
            mongo_uri,
            mongo_db,
            rpc_url: "https://api.devnet.solana.com".to_string(),
            private_key: None,
            raydium_program_id: std::env::var("RAYDIUM_PROGRAM_ID")
                .unwrap_or_else(|_| "FA1RJDDXysgwg5Gm3fJXWxt26JQzPkAzhTA114miqNUX".to_string()),
            raydium_cp_program_id: std::env::var("RAYDIUM_CP_PROGRAM_ID")
                .unwrap_or_else(|_| "FairxoKThzWcDy9avKPsADqzni18LrXxKAZEHdXVo5gi".to_string()),
            create_pool_fee_receiver: std::env::var("CREATE_POOL_FEE_RECEIVER")
                .unwrap_or_else(|_| "3gXnxLQj6Zs1WNNAdafAbGamfMyZwS62SSesEVF65rBj".to_string()),
            amm_config_index: 0,
            rust_log: "info".to_string(),
            enable_pool_event_insert: false,
            event_listener_db_mode: "update_only".to_string(),
        });

        let database = Arc::new(Database::new(config).await.unwrap());
        EventService::new(database)
    }

    #[tokio::test]
    async fn test_get_nft_claim_events_by_claimer() {
        let service = setup_test_service().await;

        // 使用一个测试地址
        let test_claimer = "11111111111111111111111111111111";

        let result = service
            .get_nft_claim_events_by_claimer(test_claimer, Some(1), Some(10), None, None)
            .await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(response.page == 1);
        assert!(response.page_size == 10);
    }

    #[tokio::test]
    async fn test_get_nft_claim_stats() {
        let service = setup_test_service().await;

        let result = service.get_nft_claim_stats().await;

        assert!(result.is_ok());
        let stats = result.unwrap();
        // u64类型总是 >= 0，所以检查字段存在即可
        let _ = stats.total_claims; // 确保字段存在
        let _ = stats.today_claims; // 确保字段存在
    }

    #[tokio::test]
    async fn test_get_reward_events_by_recipient() {
        let service = setup_test_service().await;

        let test_recipient = "11111111111111111111111111111111";

        let result = service
            .get_reward_events_by_recipient(test_recipient, Some(1), Some(10), None, None, None, None)
            .await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(response.page == 1);
        assert!(response.page_size == 10);
    }

    #[tokio::test]
    async fn test_get_reward_stats() {
        let service = setup_test_service().await;

        let result = service.get_reward_stats().await;

        assert!(result.is_ok());
        let stats = result.unwrap();
        // u64类型总是 >= 0，所以检查字段存在即可
        let _ = stats.total_distributions; // 确保字段存在
        let _ = stats.today_distributions; // 确保字段存在
    }

    #[tokio::test]
    async fn test_pagination_parameters() {
        let service = setup_test_service().await;

        // 测试分页参数边界
        let result = service
            .get_nft_claim_events_paginated(
                Some(1),
                Some(200), // 超过最大值，应该被限制为100
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(response.page_size <= 100); // 确保被限制在最大值
    }

    #[tokio::test]
    async fn test_date_range_filter() {
        let service = setup_test_service().await;

        let start_date = chrono::Utc::now()
            .checked_sub_signed(chrono::Duration::days(30))
            .unwrap()
            .timestamp();
        let end_date = chrono::Utc::now().timestamp();

        let result = service
            .get_reward_events_paginated(
                Some(1),
                Some(20),
                None,
                None,
                None,
                None,
                Some(start_date),
                Some(end_date),
                None,
                None,
            )
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_user_reward_summary() {
        let service = setup_test_service().await;

        let test_recipient = "11111111111111111111111111111111";

        let result = service.get_user_reward_summary(test_recipient).await;

        assert!(result.is_ok());
        let summary = result.unwrap();
        assert_eq!(summary.recipient, test_recipient);
        assert!(summary.total_amount >= summary.locked_amount + summary.unlocked_amount);
    }

    #[tokio::test]
    async fn test_user_nft_claim_summary() {
        let service = setup_test_service().await;

        let test_claimer = "11111111111111111111111111111111";

        let result = service.get_user_nft_claim_summary(test_claimer).await;

        assert!(result.is_ok());
        let summary = result.unwrap();
        assert_eq!(summary.claimer, test_claimer);
    }

    #[tokio::test]
    async fn test_sort_order() {
        let service = setup_test_service().await;

        // 测试升序排序
        let result_asc = service
            .get_nft_claim_events_paginated(
                Some(1),
                Some(10),
                None,
                None,
                None,
                None,
                Some("claimed_at".to_string()),
                Some("asc".to_string()),
            )
            .await;

        assert!(result_asc.is_ok());

        // 测试降序排序
        let result_desc = service
            .get_nft_claim_events_paginated(
                Some(1),
                Some(10),
                None,
                None,
                None,
                None,
                Some("claimed_at".to_string()),
                Some("desc".to_string()),
            )
            .await;

        assert!(result_desc.is_ok());
    }
}
