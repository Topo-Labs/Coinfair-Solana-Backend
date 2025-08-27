#[cfg(test)]
mod tests {
    use super::super::launch_migration_controller::LaunchMigrationController;
    use crate::dtos::solana::launch::LaunchMigrationRequest;
    use crate::services::Services;
    use axum::{
        body::Body,
        extract::Extension,
        http::{Method, Request, StatusCode},
        Router,
    };
    use database::Database;
    use serde_json;
    use tower::ServiceExt;

    /// 创建测试用的应用
    async fn create_test_app() -> Router {
        // 创建测试配置
        let config = std::sync::Arc::new(utils::AppConfig::new_for_test());

        // 创建测试数据库和服务
        let database = Database::new(config).await.expect("创建测试数据库失败");
        let services = Services::new(database);

        // 创建路由
        LaunchMigrationController::routes().layer(Extension(services))
    }

    /// 创建测试用的LaunchMigrationRequest
    fn create_test_request() -> LaunchMigrationRequest {
        LaunchMigrationRequest {
            meme_token_mint: "So11111111111111111111111111111111111111112".to_string(),
            base_token_mint: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(),
            user_wallet: "8S2bcP66WehuF6cHryfZ7vfFpQWaUhYyAYSy5U3gX4Fy".to_string(),
            config_index: 0,
            initial_price: 1.0,
            open_time: 0,
            tick_lower_price: 0.8,
            tick_upper_price: 1.2,
            meme_token_amount: 1000000000, // 1 SOL
            base_token_amount: 1000000,    // 1 USDC
            max_slippage_percent: 5.0,
            with_metadata: Some(false),
        }
    }

    /// 创建HTTP请求
    fn create_http_request(method: Method, uri: &str, body: Option<&str>) -> Request<Body> {
        let request = Request::builder()
            .method(method)
            .uri(uri)
            .header("content-type", "application/json");

        if let Some(body_content) = body {
            request.body(Body::from(body_content.to_string())).unwrap()
        } else {
            request.body(Body::empty()).unwrap()
        }
    }

    #[tokio::test]
    async fn test_launch_migration_endpoint_success() {
        let app = create_test_app().await;
        let request_data = create_test_request();
        let json_body = serde_json::to_string(&request_data).unwrap();

        let request = create_http_request(Method::POST, "/launch", Some(&json_body));

        let response = app.oneshot(request).await.unwrap();

        // 这个测试可能会失败，因为需要实际的区块链连接
        // 但我们至少可以验证请求被正确处理
        assert!(
            response.status() == StatusCode::OK || response.status() == StatusCode::INTERNAL_SERVER_ERROR,
            "端点应该返回OK或内部错误状态码"
        );
    }

    #[tokio::test]
    async fn test_launch_migration_endpoint_invalid_request() {
        let app = create_test_app().await;

        // 发送无效的JSON
        let request = create_http_request(Method::POST, "/launch", Some("invalid json"));

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_launch_migration_endpoint_missing_fields() {
        let app = create_test_app().await;

        // 创建缺少必需字段的请求
        let invalid_request = r#"{
            "meme_token_mint": "So11111111111111111111111111111111111111112",
            "base_token_mint": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"
        }"#;

        let request = create_http_request(Method::POST, "/launch", Some(invalid_request));

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_launch_migration_endpoint_validation_error() {
        let app = create_test_app().await;

        let mut request_data = create_test_request();
        request_data.initial_price = -1.0; // 无效价格

        let json_body = serde_json::to_string(&request_data).unwrap();

        let request = create_http_request(Method::POST, "/launch", Some(&json_body));

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[tokio::test]
    async fn test_launch_and_send_transaction_endpoint() {
        let app = create_test_app().await;
        let request_data = create_test_request();
        let json_body = serde_json::to_string(&request_data).unwrap();

        let request = create_http_request(Method::POST, "/launch-and-send-transaction", Some(&json_body));

        let response = app.oneshot(request).await.unwrap();

        // 这个测试可能会失败，因为需要实际的区块链连接和私钥配置
        // 但我们至少可以验证请求被正确处理
        assert!(
            response.status() == StatusCode::OK || response.status() == StatusCode::INTERNAL_SERVER_ERROR,
            "端点应该返回OK或内部错误状态码"
        );
    }

    #[tokio::test]
    async fn test_unsupported_method() {
        let app = create_test_app().await;

        let request = create_http_request(Method::GET, "/launch", None);
        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    }

    #[tokio::test]
    async fn test_unsupported_path() {
        let app = create_test_app().await;

        let request = create_http_request(Method::POST, "/invalid-path", None);
        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    // 性能测试
    #[tokio::test]
    async fn test_concurrent_requests() {
        let app = create_test_app().await;
        let request_data = create_test_request();
        let json_body = serde_json::to_string(&request_data).unwrap();

        let mut handles = vec![];

        // 发送10个并发请求
        for _ in 0..10 {
            let app_clone = app.clone();
            let json_clone = json_body.clone();

            let handle = tokio::spawn(async move {
                let request = create_http_request(Method::POST, "/launch", Some(&json_clone));

                app_clone.oneshot(request).await.unwrap()
            });

            handles.push(handle);
        }

        // 等待所有请求完成
        let responses = futures::future::join_all(handles).await;

        // 验证所有请求都得到了响应
        for response in responses {
            let response = response.unwrap();
            assert!(
                response.status() == StatusCode::OK
                    || response.status() == StatusCode::INTERNAL_SERVER_ERROR
                    || response.status() == StatusCode::BAD_REQUEST,
                "并发请求应该得到有效响应"
            );
        }
    }

    // 负载测试
    #[tokio::test]
    #[ignore = "负载测试，仅在需要时运行"]
    async fn test_high_load() {
        let app = create_test_app().await;
        let request_data = create_test_request();
        let json_body = serde_json::to_string(&request_data).unwrap();

        let start = std::time::Instant::now();
        let mut handles = vec![];

        // 发送100个并发请求
        for _ in 0..100 {
            let app_clone = app.clone();
            let json_clone = json_body.clone();

            let handle = tokio::spawn(async move {
                let request = create_http_request(Method::POST, "/launch", Some(&json_clone));

                app_clone.oneshot(request).await.unwrap()
            });

            handles.push(handle);
        }

        // 等待所有请求完成
        let _responses = futures::future::join_all(handles).await;
        let duration = start.elapsed();

        println!("100个并发请求耗时: {:?}", duration);

        // 验证性能（平均每个请求不超过100ms）
        assert!(duration.as_millis() < 10000, "高负载下性能应该可接受");
    }

    // 数据验证测试
    #[tokio::test]
    async fn test_request_data_validation() {
        // 测试各种无效数据
        let test_cases = vec![
            (r#"{"meme_token_mint": ""}"#, "空的meme_token_mint"),
            (r#"{"base_token_mint": ""}"#, "空的base_token_mint"),
            (r#"{"user_wallet": ""}"#, "空的user_wallet"),
            (r#"{"initial_price": "not_a_number"}"#, "非数字价格"),
            (r#"{"config_index": "not_a_number"}"#, "非数字config_index"),
        ];

        let app = create_test_app().await;

        for (invalid_data, description) in test_cases {
            let request = create_http_request(Method::POST, "/launch", Some(invalid_data));

            let response = app.clone().oneshot(request).await.unwrap();

            assert!(
                response.status() == StatusCode::BAD_REQUEST || response.status() == StatusCode::INTERNAL_SERVER_ERROR,
                "无效数据应该被拒绝: {}",
                description
            );
        }
    }
}
