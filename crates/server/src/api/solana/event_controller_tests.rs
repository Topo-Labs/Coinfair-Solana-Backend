#[cfg(test)]
mod integration_tests {
    use std::sync::Arc;

    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use serde_json::Value;
    use tower::ServiceExt;

    /// 创建测试应用
    async fn create_test_app() -> axum::Router {
        // 使用测试配置，包含所有必需的字段
        let config = Arc::new(utils::AppConfig {
            cargo_env: utils::CargoEnv::Development,
            app_host: "0.0.0.0".to_string(),
            app_port: 8765,
            mongo_uri: std::env::var("MONGO_URI").unwrap_or_else(|_| "mongodb://localhost:27017".to_string()),
            mongo_db: std::env::var("MONGO_DB").unwrap_or_else(|_| "test_db".to_string()),
            rpc_url: "https://api.devnet.solana.com".to_string(),
            private_key: None,
            raydium_program_id: "FA1RJDDXysgwg5Gm3fJXWxt26JQzPkAzhTA114miqNUX".to_string(),
            amm_config_index: 0,
            rust_log: "info".to_string(),
            enable_pool_event_insert: false,
            event_listener_db_mode: "update_only".to_string(),
        });

        let db = database::Database::new(config.clone()).await.unwrap();
        let services = crate::services::Services::new(db);
        crate::router::AppRouter::new(services)
    }

    #[tokio::test]
    async fn test_get_nft_claim_events_endpoint() {
        let app = create_test_app().await;
        utils::AppConfig::new_for_test();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/solana/events/nft-claims?page=1&page_size=10")
                    .method("GET")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();

        assert!(json["success"].as_bool().unwrap_or(false));
        assert!(json["data"]["items"].is_array());
        assert!(json["data"]["total"].is_number());
        assert!(json["data"]["page"].as_u64().unwrap() == 1);
        assert!(json["data"]["page_size"].as_u64().unwrap() == 10);
    }

    #[tokio::test]
    async fn test_get_nft_claim_stats_endpoint() {
        let app = create_test_app().await;

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/solana/events/nft-claims/stats")
                    .method("GET")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();

        assert!(json["success"].as_bool().unwrap_or(false));
        assert!(json["data"]["total_claims"].is_number());
        assert!(json["data"]["today_claims"].is_number());
        assert!(json["data"]["tier_distribution"].is_array());
    }

    #[tokio::test]
    async fn test_get_reward_events_endpoint() {
        let app = create_test_app().await;

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/solana/events/rewards?page=1&page_size=10")
                    .method("GET")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();

        assert!(json["success"].as_bool().unwrap_or(false));
        assert!(json["data"]["items"].is_array());
        assert!(json["data"]["total"].is_number());
    }

    #[tokio::test]
    async fn test_get_reward_stats_endpoint() {
        let app = create_test_app().await;

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/solana/events/rewards/stats")
                    .method("GET")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();

        assert!(json["success"].as_bool().unwrap_or(false));
        assert!(json["data"]["total_distributions"].is_number());
        assert!(json["data"]["today_distributions"].is_number());
        assert!(json["data"]["locked_rewards"].is_number());
    }

    #[tokio::test]
    async fn test_get_user_nft_claim_summary_endpoint() {
        let app = create_test_app().await;
        let test_address = "11111111111111111111111111111111";

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/solana/events/nft-claims/summary/{}", test_address).as_str())
                    .method("GET")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();

        assert!(json["success"].as_bool().unwrap_or(false));
        assert_eq!(json["data"]["claimer"].as_str().unwrap(), test_address);
        assert!(json["data"]["total_claims"].is_number());
    }

    #[tokio::test]
    async fn test_get_user_reward_summary_endpoint() {
        let app = create_test_app().await;
        let test_address = "11111111111111111111111111111111";

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/solana/events/rewards/summary/{}", test_address).as_str())
                    .method("GET")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();

        assert!(json["success"].as_bool().unwrap_or(false));
        assert_eq!(json["data"]["recipient"].as_str().unwrap(), test_address);
        assert!(json["data"]["total_rewards"].is_number());
    }

    #[tokio::test]
    async fn test_pagination_parameters() {
        let app = create_test_app().await;

        // 测试默认分页参数
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/solana/events/nft-claims")
                    .method("GET")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();

        // 检查默认值
        assert_eq!(json["data"]["page"].as_u64().unwrap(), 1);
        assert_eq!(json["data"]["page_size"].as_u64().unwrap(), 20);

        // 测试自定义分页参数
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/solana/events/nft-claims?page=2&page_size=50")
                    .method("GET")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(json["data"]["page"].as_u64().unwrap(), 2);
        assert_eq!(json["data"]["page_size"].as_u64().unwrap(), 50);
    }

    #[tokio::test]
    async fn test_filter_parameters() {
        let app = create_test_app().await;

        // 测试NFT领取事件过滤
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/solana/events/nft-claims?tier=3&has_referrer=true")
                    .method("GET")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        // 测试奖励分发事件过滤
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/solana/events/rewards?is_locked=true&reward_type=1")
                    .method("GET")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_sort_parameters() {
        let app = create_test_app().await;

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/solana/events/nft-claims?sort_by=claimed_at&sort_order=asc")
                    .method("GET")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_date_range_filter() {
        let app = create_test_app().await;

        let start_date = chrono::Utc::now()
            .checked_sub_signed(chrono::Duration::days(30))
            .unwrap()
            .timestamp();
        let end_date = chrono::Utc::now().timestamp();

        let response = app
            .oneshot(
                Request::builder()
                    .uri(
                        format!(
                            "/api/v1/solana/events/nft-claims?start_date={}&end_date={}",
                            start_date, end_date
                        )
                        .as_str(),
                    )
                    .method("GET")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_get_nft_claims_by_claimer() {
        let app = create_test_app().await;
        let test_claimer = "8S2bcP66WehuF6cHryfZ7vfFpQWaUhYyAYSy5U3gX4Fy";

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/solana/events/nft-claims/by-claimer/{}", test_claimer).as_str())
                    .method("GET")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();

        assert!(json["success"].as_bool().unwrap_or(false));
        assert!(json["data"]["items"].is_array());
    }

    #[tokio::test]
    async fn test_get_nft_claims_by_nft() {
        let app = create_test_app().await;
        let test_nft_mint = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/solana/events/nft-claims/by-nft/{}", test_nft_mint).as_str())
                    .method("GET")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();

        assert!(json["success"].as_bool().unwrap_or(false));
        assert!(json["data"]["items"].is_array());
    }

    #[tokio::test]
    async fn test_get_rewards_by_recipient() {
        let app = create_test_app().await;
        let test_recipient = "8S2bcP66WehuF6cHryfZ7vfFpQWaUhYyAYSy5U3gX4Fy";

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/solana/events/rewards/by-recipient/{}", test_recipient).as_str())
                    .method("GET")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();

        assert!(json["success"].as_bool().unwrap_or(false));
        assert!(json["data"]["items"].is_array());
    }

    #[tokio::test]
    async fn test_get_reward_by_distribution_id() {
        let app = create_test_app().await;
        let test_id = 999999; // 不存在的ID，应该返回404

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/solana/events/rewards/by-id/{}", test_id).as_str())
                    .method("GET")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        // 应该返回404
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_invalid_pagination_parameters() {
        let app = create_test_app().await;

        // 测试无效页码（0）
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/solana/events/nft-claims?page=0")
                    .method("GET")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        // 测试超大页大小（超过100）
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/solana/events/nft-claims?page_size=500")
                    .method("GET")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();

        // 页大小应该被限制为100
        assert!(json["data"]["page_size"].as_u64().unwrap() <= 100);
    }

    #[tokio::test]
    async fn test_invalid_tier_parameter() {
        let app = create_test_app().await;

        // 测试无效的tier值（超过5）
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/solana/events/nft-claims?tier=10")
                    .method("GET")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_combined_filters() {
        let app = create_test_app().await;

        // 测试组合多个过滤条件
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/solana/events/nft-claims?tier=3&has_referrer=true&page=1&page_size=5&sort_by=claimed_at&sort_order=desc")
                    .method("GET")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();

        assert!(json["success"].as_bool().unwrap_or(false));
        assert_eq!(json["data"]["page"].as_u64().unwrap(), 1);
        assert_eq!(json["data"]["page_size"].as_u64().unwrap(), 5);
    }

    #[tokio::test]
    async fn test_empty_results() {
        let app = create_test_app().await;

        // 使用一个不太可能存在的地址
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/solana/events/nft-claims/by-claimer/NonExistentAddress123456789012345")
                    .method("GET")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();

        assert!(json["success"].as_bool().unwrap_or(false));
        assert!(json["data"]["items"].is_array());
        assert_eq!(json["data"]["total"].as_u64().unwrap(), 0);
    }
}
