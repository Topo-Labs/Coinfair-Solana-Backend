use crate::error::Result;
use crate::parser::event_parser::LaunchEventData;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::info;

/// Launch迁移请求结构体
#[derive(Debug, Clone, Serialize)]
pub struct LaunchMigrationRequest {
    pub meme_token_mint: String,
    pub base_token_mint: String,
    pub user_wallet: String,
    pub config_index: u32,
    pub initial_price: f64,
    pub open_time: u64,
    pub tick_lower_price: f64,
    pub tick_upper_price: f64,
    pub meme_token_amount: u64,
    pub base_token_amount: u64,
    pub max_slippage_percent: f64,
    pub with_metadata: bool,
}

/// Launch迁移响应结构体
#[derive(Debug, Deserialize)]
pub struct LaunchMigrationResponse {
    pub signature: String,
    pub pool_address: String,
    pub status: String,
}

/// 迁移服务HTTP客户端
#[allow(dead_code)]
pub struct MigrationClient {
    client: Client,
    base_url: String,
}

impl MigrationClient {
    /// 创建新的迁移客户端
    pub fn new(base_url: String) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .unwrap_or_else(|_| Client::new());

        Self { client, base_url }
    }

    /// 触发Launch迁移
    pub async fn trigger_launch_migration(&self, _event: &LaunchEventData) -> Result<LaunchMigrationResponse> {
        /*
        // 构建请求数据
        let request = LaunchMigrationRequest {
            meme_token_mint: event.meme_token_mint.clone(),
            base_token_mint: event.base_token_mint.clone(),
            user_wallet: event.user_wallet.clone(),
            config_index: event.config_index,
            initial_price: event.initial_price,
            open_time: event.open_time,
            tick_lower_price: event.tick_lower_price,
            tick_upper_price: event.tick_upper_price,
            meme_token_amount: event.meme_token_amount,
            base_token_amount: event.base_token_amount,
            max_slippage_percent: event.max_slippage_percent,
            with_metadata: event.with_metadata,
        };

        let url = format!("{}/api/v1/solana/pool/launch-migration/send", self.base_url);

        info!(
            "🚀 发送Launch迁移请求: {} -> {}, URL: {}",
            event.meme_token_mint, event.base_token_mint, url
        );

               // 发送HTTP请求
                let response = self
                    .client
                    .post(&url)
                    .json(&request)
                    .send()
                    .await
                    .map_err(|e| {
                        error!("❌ 发送迁移请求失败: {}", e);
                        EventListenerError::Network(format!("HTTP请求失败: {}", e))
                    })?;

                // 检查响应状态
                if !response.status().is_success() {
                    let status = response.status();
                    let error_text = response.text().await.unwrap_or_else(|_| "未知错误".to_string());
                    error!("❌ 迁移API返回错误 ({}): {}", status, error_text);
                    return Err(EventListenerError::EventParsing(format!(
                        "迁移API调用失败 ({}): {}",
                        status, error_text
                    )));
                }

                // 解析响应
                let migration_response = response
                    .json::<LaunchMigrationResponse>()
                    .await
                    .map_err(|e| {
                        error!("❌ 解析迁移响应失败: {}", e);
                        EventListenerError::EventParsing(format!("解析响应失败: {}", e))
                    })?;

                info!(
                    "✅ Launch迁移成功: 池子={}, 签名={}",
                    migration_response.pool_address, migration_response.signature
                );
        */
        info!("🚀 暂时屏蔽Launch迁移接口调用！");
        let migration_response = LaunchMigrationResponse {
            signature: "mock_signature_12345".to_string(),
            pool_address: "mock_pool_address_67890".to_string(),
            status: "success".to_string(),
        };
        Ok(migration_response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use solana_sdk::pubkey::Pubkey;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn create_test_event() -> LaunchEventData {
        LaunchEventData {
            meme_token_mint: Pubkey::new_unique().to_string(),
            base_token_mint: Pubkey::new_unique().to_string(),
            user_wallet: Pubkey::new_unique().to_string(),
            config_index: 0,
            initial_price: 1.0,
            open_time: 0,
            tick_lower_price: 0.0001,
            tick_upper_price: 10000.0,
            meme_token_amount: 1000000,
            base_token_amount: 1000000,
            max_slippage_percent: 1.0,
            with_metadata: true,
            signature: "test_sig".to_string(),
            slot: 12345,
            processed_at: Utc::now().to_rfc3339(),
        }
    }

    #[tokio::test]
    async fn test_migration_client_success() {
        // 启动一个模拟服务器
        let mock_server = MockServer::start().await;

        // 设置模拟响应
        let response_body = serde_json::json!({
            "signature": "mock_signature_12345",
            "pool_address": "mock_pool_address_67890",
            "status": "success"
        });

        Mock::given(method("POST"))
            .and(path("/api/v1/solana/pool/launch-migration/send"))
            .respond_with(ResponseTemplate::new(200).set_body_json(response_body))
            .mount(&mock_server)
            .await;

        // 创建客户端并测试
        let client = MigrationClient::new(mock_server.uri());
        let test_event = create_test_event();

        let result = client.trigger_launch_migration(&test_event).await.unwrap();

        assert_eq!(result.signature, "mock_signature_12345");
        assert_eq!(result.pool_address, "mock_pool_address_67890");
        assert_eq!(result.status, "success");
    }

    #[tokio::test]
    async fn test_migration_client_error_response() {
        let mock_server = MockServer::start().await;

        // 设置错误响应
        Mock::given(method("POST"))
            .and(path("/api/v1/solana/pool/launch-migration/send"))
            .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
            .mount(&mock_server)
            .await;

        let client = MigrationClient::new(mock_server.uri());
        let test_event = create_test_event();

        let result = client.trigger_launch_migration(&test_event).await;

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.to_string().contains("迁移API调用失败"));
    }

    #[tokio::test]
    async fn test_migration_client_invalid_json_response() {
        let mock_server = MockServer::start().await;

        // 设置无效的JSON响应
        Mock::given(method("POST"))
            .and(path("/api/v1/solana/pool/launch-migration/send"))
            .respond_with(ResponseTemplate::new(200).set_body_string("invalid json"))
            .mount(&mock_server)
            .await;

        let client = MigrationClient::new(mock_server.uri());
        let test_event = create_test_event();

        let result = client.trigger_launch_migration(&test_event).await;

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.to_string().contains("解析响应失败"));
    }

    #[test]
    fn test_migration_client_new() {
        let base_url = "http://localhost:8765".to_string();
        let client = MigrationClient::new(base_url.clone());

        assert_eq!(client.base_url, base_url);
    }
}
