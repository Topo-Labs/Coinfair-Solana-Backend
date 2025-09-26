use axum::{
    body::Body,
    http::{Request, StatusCode},
    Extension,
};
use chrono::Utc;
use serde_json::Value;
use std::sync::Arc;
use tower::ServiceExt;

use database::Database;
use database::events::event_model::DepositEvent;
use server::services::Services;
use server::api::solana::clmm::deposit_event_controller::DepositEventController;
use utils::AppConfig;

/// 集成测试 - 测试DepositEvent API的端到端功能
///
/// 这些测试验证API控制器、服务层和数据库层的集成是否正常工作

#[tokio::test(flavor = "multi_thread")]
async fn test_deposit_event_api_integration() {
    // 初始化测试环境
    let config = Arc::new(AppConfig::new_for_test());
    let database = Database::new(config).await.expect("Failed to connect to database");

    let services = Services::new(database);

    // 构建测试应用
    let app = DepositEventController::routes().layer(Extension(services));

    // 测试健康检查（如果有的话）
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/deposits")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // 基本的API可用性测试
    assert!(response.status().is_success() || response.status().is_client_error());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_deposit_stats_endpoint() {
    let config = Arc::new(AppConfig::new_for_test());
    let database = Database::new(config).await.expect("Failed to connect to database");
    let services = Services::new(database);

    let app = DepositEventController::routes().layer(Extension(services));

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/deposits/stats")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body_text = String::from_utf8(body_bytes.to_vec()).unwrap();

    // 验证响应是有效的JSON
    let json_response: Value = serde_json::from_str(&body_text).expect("Response should be valid JSON");

    // 验证API响应结构
    assert!(json_response.get("success").is_some());
    assert!(json_response.get("data").is_some());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_deposit_trends_endpoint() {
    let config = Arc::new(AppConfig::new_for_test());
    let database = Database::new(config).await.expect("Failed to connect to database");
    let services = Services::new(database);

    let app = DepositEventController::routes().layer(Extension(services));

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/deposits/trends?period=Day")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body_text = String::from_utf8(body_bytes.to_vec()).unwrap();

    let json_response: Value = serde_json::from_str(&body_text).expect("Response should be valid JSON");

    // 验证趋势数据响应结构
    assert!(json_response.get("success").is_some());
    if let Some(data) = json_response.get("data") {
        assert!(data.get("trends").is_some());
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_deposit_pagination_endpoint() {
    let config = Arc::new(AppConfig::new_for_test());
    let database = Database::new(config).await.expect("Failed to connect to database");
    let services = Services::new(database);

    let app = DepositEventController::routes().layer(Extension(services));

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/deposits?page=1&page_size=10")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body_text = String::from_utf8(body_bytes.to_vec()).unwrap();

    let json_response: Value = serde_json::from_str(&body_text).expect("Response should be valid JSON");

    // 验证分页响应结构
    assert!(json_response.get("success").is_some());
    if let Some(data) = json_response.get("data") {
        assert!(data.get("items").is_some());
        assert!(data.get("total").is_some());
        assert!(data.get("page").is_some());
        assert!(data.get("page_size").is_some());
        assert!(data.get("total_pages").is_some());
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_deposit_by_signature_endpoint() {
    let config = Arc::new(AppConfig::new_for_test());
    let database = Database::new(config).await.expect("Failed to connect to database");
    let services = Services::new(database);

    let app = DepositEventController::routes().layer(Extension(services));

    let test_signature = "test_signature_nonexistent_12345";

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(&format!("/deposits/by-signature/{}", test_signature))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body_text = String::from_utf8(body_bytes.to_vec()).unwrap();

    let json_response: Value = serde_json::from_str(&body_text).expect("Response should be valid JSON");

    // 验证单个事件查询响应结构
    assert!(json_response.get("success").is_some());
    assert!(json_response["success"].as_bool().unwrap());
    // 对于不存在的签名，data应该为null
    assert!(json_response.get("data").is_some());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_user_deposit_summary_endpoint() {
    let config = Arc::new(AppConfig::new_for_test());
    let database = Database::new(config).await.expect("Failed to connect to database");
    let services = Services::new(database);

    let app = DepositEventController::routes().layer(Extension(services));

    let test_user = "8S2bcP66WehuF6cHryfZ7vfFpQWaUhYyAYSy5U3gX4Fy";

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(&format!("/deposits/summary/{}", test_user))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body_text = String::from_utf8(body_bytes.to_vec()).unwrap();

    let json_response: Value = serde_json::from_str(&body_text).expect("Response should be valid JSON");

    // 验证用户汇总响应结构
    assert!(json_response.get("success").is_some());
    if let Some(data) = json_response.get("data") {
        assert!(data.get("user").is_some());
        assert!(data.get("total_deposits").is_some());
        assert!(data.get("total_volume_usd").is_some());
        assert!(data.get("unique_tokens").is_some());
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_advanced_query_endpoint() {
    let config = Arc::new(AppConfig::new_for_test());
    let database = Database::new(config).await.expect("Failed to connect to database");
    let services = Services::new(database);

    let app = DepositEventController::routes().layer(Extension(services));

    let query_params = "page=1&page_size=20&deposit_type=1&amount_min=1000000&amount_max=10000000";

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(&format!("/deposits/advanced?{}", query_params))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body_text = String::from_utf8(body_bytes.to_vec()).unwrap();

    let json_response: Value = serde_json::from_str(&body_text).expect("Response should be valid JSON");

    // 验证高级查询响应结构
    assert!(json_response.get("success").is_some());
    if let Some(data) = json_response.get("data") {
        assert!(data.get("items").is_some());
        assert!(data.get("total").is_some());
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_error_handling() {
    let config = Arc::new(AppConfig::new_for_test());
    let database = Database::new(config).await.expect("Failed to connect to database");
    let services = Services::new(database);

    let app = DepositEventController::routes().layer(Extension(services));

    // 测试无效的页面大小参数
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/deposits?page_size=999") // 超过最大限制
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // 应该返回成功，但页面大小被限制为最大值
    assert_eq!(response.status(), StatusCode::OK);
}

#[cfg(test)]
mod database_integration_tests {
    use super::*;

    /// 数据库集成测试 - 验证数据层集成
    #[tokio::test(flavor = "multi_thread")]
    async fn test_database_connection_and_indexes() {
        let config = Arc::new(AppConfig::new_for_test());
        let database = Database::new(config).await.expect("Failed to connect to database");

        // 测试连接是否正常
        assert_eq!(database.deposit_events.name(), "DepositEvent");

        // 测试索引初始化
        let result = database.deposit_event_repository.init_indexes().await;
        assert!(result.is_ok(), "索引初始化应该成功");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_deposit_event_crud_operations() {
        let config = Arc::new(AppConfig::new_for_test());
        let database = Database::new(config).await.expect("Failed to connect to database");

        // 创建测试数据
        let test_event = DepositEvent {
            id: None,
            user: "test_user_integration_123".to_string(),
            token_mint: "So11111111111111111111111111111111111111112".to_string(),
            amount: 1000000,
            project_config: "test_project_config".to_string(),
            total_raised: 5000000,
            deposited_at: Utc::now().timestamp(),
            token_decimals: Some(9),
            token_name: Some("Test Token".to_string()),
            token_symbol: Some("TEST".to_string()),
            token_logo_uri: Some("https://example.com/test.png".to_string()),
            deposit_type: 1,
            deposit_type_name: "集成测试存款".to_string(),
            is_high_value_deposit: false,
            related_pool: Some("test_pool".to_string()),
            estimated_usd_value: 100.0,
            actual_amount: 1.0,
            actual_total_raised: 5.0,
            signature: format!("integration_test_signature_{}", Utc::now().timestamp_millis()),
            slot: 12345,
            processed_at: Utc::now().timestamp(),
            updated_at: Utc::now().timestamp(),
        };

        // 测试插入
        let insert_result = database
            .deposit_event_repository
            .insert_deposit_event(test_event.clone())
            .await;
        assert!(insert_result.is_ok(), "插入存款事件应该成功");

        // 测试按签名查询
        let signature = &test_event.signature;
        let find_result = database.deposit_event_repository.find_by_signature(signature).await;
        assert!(find_result.is_ok(), "按签名查询应该成功");

        if let Ok(Some(found_event)) = find_result {
            assert_eq!(found_event.user, test_event.user);
            assert_eq!(found_event.token_mint, test_event.token_mint);
            assert_eq!(found_event.amount, test_event.amount);
        }

        // 测试统计查询
        let stats_result = database.deposit_event_repository.get_deposit_stats().await;
        assert!(stats_result.is_ok(), "获取统计信息应该成功");

        let stats = stats_result.unwrap();
        assert!(stats.total_deposits >= 1, "应该至少有1个存款记录");
        assert!(stats.unique_users >= 1, "应该至少有1个独特用户");
        assert!(stats.unique_tokens >= 1, "应该至少有1个独特代币");
        assert!(stats.total_volume_usd >= 0.0, "总交易量应该非负");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_deposit_event_pagination() {
        let config = Arc::new(AppConfig::new_for_test());
        let database = Database::new(config).await.expect("Failed to connect to database");

        // 测试分页查询
        use mongodb::bson::doc;
        use mongodb::options::FindOptions;

        let filter = doc! {};
        let options = FindOptions::builder().limit(5).skip(0).build();

        let paginated_result = database.deposit_event_repository.find_paginated(filter, options).await;

        assert!(paginated_result.is_ok(), "分页查询应该成功");

        let result = paginated_result.unwrap();
        assert!(result.items.len() <= 5); // 应该不超过限制
    }
}

#[cfg(test)]
mod service_integration_tests {
    use super::*;
    use server::services::solana::clmm::event::DepositEventService;

    /// 服务层集成测试 - 验证服务层与数据库的集成
    #[tokio::test(flavor = "multi_thread")]
    async fn test_deposit_service_with_real_database() {
        let config = Arc::new(AppConfig::new_for_test());
        let database = Database::new(config).await.expect("Failed to connect to database");

        let service = DepositEventService::new(Arc::new(database));

        // 测试获取统计信息
        let stats_result = service.get_deposit_stats().await;
        assert!(stats_result.is_ok(), "服务层获取统计信息应该成功");

        let stats = stats_result.unwrap();
        assert!(stats.total_volume_usd >= 0.0);
        assert!(stats.today_volume_usd >= 0.0);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_deposit_service_pagination() {
        let config = Arc::new(AppConfig::new_for_test());
        let database = Database::new(config).await.expect("Failed to connect to database");

        let service = DepositEventService::new(Arc::new(database));

        // 测试分页查询
        let paginated_result = service
            .get_deposit_events_paginated(
                Some(1),  // page
                Some(10), // page_size
                None,     // user
                None,     // token_mint
                None,     // project_config
                None,     // deposit_type
                None,     // start_date
                None,     // end_date
                None,     // sort_by
                None,     // sort_order
            )
            .await;

        assert!(paginated_result.is_ok(), "分页查询应该成功");

        let result = paginated_result.unwrap();
        assert_eq!(result.page, 1);
        assert_eq!(result.page_size, 10);
        assert!(result.items.len() <= 10);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_deposit_service_user_summary() {
        let config = Arc::new(AppConfig::new_for_test());
        let database = Database::new(config).await.expect("Failed to connect to database");

        let service = DepositEventService::new(Arc::new(database));

        let test_user = "test_user_for_summary";
        let summary_result = service.get_user_deposit_summary(test_user).await;

        assert!(summary_result.is_ok(), "用户汇总查询应该成功");

        let summary = summary_result.unwrap();
        assert_eq!(summary.user, test_user);
        assert!(summary.total_volume_usd >= 0.0);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_deposit_service_trends() {
        let config = Arc::new(AppConfig::new_for_test());
        let database = Database::new(config).await.expect("Failed to connect to database");

        let service = DepositEventService::new(Arc::new(database));

        use server::services::solana::clmm::event::deposit_service::TrendPeriod;

        let trends_result = service
            .get_deposit_trends(
                TrendPeriod::Day,
                None, // start_date
                None, // end_date
            )
            .await;

        assert!(trends_result.is_ok(), "趋势数据查询应该成功");

        let trends = trends_result.unwrap();
        // 趋势数据可能为空，这是正常的
        for trend in trends {
            assert!(trend.volume_usd >= 0.0);
            assert!(!trend.period.is_empty());
        }
    }
}
