use chrono::{DateTime, Utc};
use database::cpmm::lp_change_event::model::LpChangeEvent;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::Validate;
use crate::dtos::solana::common::validate_pubkey;

/// 创建LP变更事件请求DTO
#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct CreateLpChangeEventRequest {
    /// 用户钱包地址
    #[validate(custom = "validate_pubkey")]
    pub user_wallet: String,

    /// 池子地址
    #[validate(custom = "validate_pubkey")]
    pub pool_id: String,

    /// LP代币mint地址
    #[validate(custom = "validate_pubkey")]
    pub lp_mint: String,

    /// Token 0 mint地址
    #[validate(custom = "validate_pubkey")]
    pub token_0_mint: String,

    /// Token 1 mint地址
    #[validate(custom = "validate_pubkey")]
    pub token_1_mint: String,

    /// 变更类型：0=deposit, 1=withdraw, 2=initialize
    #[validate(range(max = 2, message = "change_type必须是0、1或2"))]
    pub change_type: u8,

    pub lp_amount_before: u64,
    pub lp_amount_after: u64,
    pub lp_amount_change: i64,
    pub token_0_amount: u64,
    pub token_1_amount: u64,
    pub token_0_transfer_fee: u64,
    pub token_1_transfer_fee: u64,
    pub token_0_vault_before: u64,
    pub token_1_vault_before: u64,
    pub token_0_vault_after: u64,
    pub token_1_vault_after: u64,

    /// LP mint程序ID
    #[validate(custom = "validate_pubkey")]
    pub lp_mint_program_id: String,

    /// Token 0程序ID
    #[validate(custom = "validate_pubkey")]
    pub token_0_program_id: String,

    /// Token 1程序ID
    #[validate(custom = "validate_pubkey")]
    pub token_1_program_id: String,

    pub lp_mint_decimals: u8,
    pub token_0_decimals: u8,
    pub token_1_decimals: u8,

    /// 交易签名
    #[validate(length(min = 64, max = 128, message = "signature长度无效"))]
    pub signature: String,

    /// 区块槽位
    #[validate(range(min = 1, message = "slot必须大于0"))]
    pub slot: u64,

    pub block_time: Option<i64>,
}

/// 查询LP变更事件请求DTO
#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct QueryLpChangeEventsRequest {
    /// 用户钱包地址（可选）
    #[validate(custom = "validate_pubkey")]
    pub user_wallet: Option<String>,

    /// 池子地址（可选）
    #[validate(custom = "validate_pubkey")]
    pub pool_id: Option<String>,

    /// 支持多个lp_mint，英文逗号分隔
    pub lp_mints: Option<String>,

    /// 变更类型（可选）：0=deposit, 1=withdraw, 2=initialize
    #[validate(range(max = 2, message = "change_type必须是0、1或2"))]
    pub change_type: Option<u8>,

    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,

    /// 页码（可选，默认1）
    #[validate(range(min = 1, message = "页码必须大于0"))]
    pub page: Option<u64>,

    /// 每页大小（可选，默认20，最大100）
    #[validate(range(min = 1, max = 100, message = "每页大小必须在1-100之间"))]
    pub page_size: Option<u64>,
}

/// LP变更事件响应DTO
#[derive(Debug, Serialize, ToSchema)]
pub struct LpChangeEventResponse {
    pub id: String,
    pub user_wallet: String,
    pub pool_id: String,
    pub lp_mint: String,
    pub token_0_mint: String,
    pub token_1_mint: String,
    pub change_type: u8,
    pub change_type_name: String, // deposit, withdraw, initialize
    pub lp_amount_before: String,
    pub lp_amount_after: String,
    pub lp_amount_change: String,
    pub token_0_amount: String,
    pub token_1_amount: String,
    pub token_0_transfer_fee: String,
    pub token_1_transfer_fee: String,
    pub token_0_vault_before: String,
    pub token_1_vault_before: String,
    pub token_0_vault_after: String,
    pub token_1_vault_after: String,
    pub lp_mint_program_id: String,
    pub token_0_program_id: String,
    pub token_1_program_id: String,
    pub lp_mint_decimals: u8,
    pub token_0_decimals: u8,
    pub token_1_decimals: u8,
    pub signature: String,
    pub slot: u64,
    pub block_time: Option<i64>,
    pub created_at: String,
}

/// LP变更事件分页响应DTO
#[derive(Debug, Serialize, ToSchema)]
pub struct LpChangeEventsPageResponse {
    pub data: Vec<LpChangeEventResponse>,
    pub total: u64,
    pub page: u64,
    pub page_size: u64,
    pub total_pages: u64,
}

impl CreateLpChangeEventRequest {
    /// 转换为LpChangeEvent模型
    pub fn to_model(self) -> LpChangeEvent {
        LpChangeEvent {
            id: None,
            user_wallet: self.user_wallet,
            pool_id: self.pool_id,
            lp_mint: self.lp_mint,
            token_0_mint: self.token_0_mint,
            token_1_mint: self.token_1_mint,
            change_type: self.change_type,
            lp_amount_before: self.lp_amount_before,
            lp_amount_after: self.lp_amount_after,
            lp_amount_change: self.lp_amount_change,
            token_0_amount: self.token_0_amount,
            token_1_amount: self.token_1_amount,
            token_0_transfer_fee: self.token_0_transfer_fee,
            token_1_transfer_fee: self.token_1_transfer_fee,
            token_0_vault_before: self.token_0_vault_before,
            token_1_vault_before: self.token_1_vault_before,
            token_0_vault_after: self.token_0_vault_after,
            token_1_vault_after: self.token_1_vault_after,
            lp_mint_program_id: self.lp_mint_program_id,
            token_0_program_id: self.token_0_program_id,
            token_1_program_id: self.token_1_program_id,
            lp_mint_decimals: self.lp_mint_decimals,
            token_0_decimals: self.token_0_decimals,
            token_1_decimals: self.token_1_decimals,
            signature: self.signature,
            slot: self.slot,
            block_time: self.block_time,
            created_at: Utc::now(), // 创建时间在Repository中设置
        }
    }
}

impl From<LpChangeEvent> for LpChangeEventResponse {
    fn from(event: LpChangeEvent) -> Self {
        let change_type_name = event.get_change_type_name().to_string();
        Self {
            id: event.id.map(|id| id.to_string()).unwrap_or_default(),
            user_wallet: event.user_wallet,
            pool_id: event.pool_id,
            lp_mint: event.lp_mint,
            token_0_mint: event.token_0_mint,
            token_1_mint: event.token_1_mint,
            change_type: event.change_type,
            change_type_name,
            lp_amount_before: event.lp_amount_before.to_string(),
            lp_amount_after: event.lp_amount_after.to_string(),
            lp_amount_change: event.lp_amount_change.to_string(),
            token_0_amount: event.token_0_amount.to_string(),
            token_1_amount: event.token_1_amount.to_string(),
            token_0_transfer_fee: event.token_0_transfer_fee.to_string(),
            token_1_transfer_fee: event.token_1_transfer_fee.to_string(),
            token_0_vault_before: event.token_0_vault_before.to_string(),
            token_1_vault_before: event.token_1_vault_before.to_string(),
            token_0_vault_after: event.token_0_vault_after.to_string(),
            token_1_vault_after: event.token_1_vault_after.to_string(),
            lp_mint_program_id: event.lp_mint_program_id,
            token_0_program_id: event.token_0_program_id,
            token_1_program_id: event.token_1_program_id,
            lp_mint_decimals: event.lp_mint_decimals,
            token_0_decimals: event.token_0_decimals,
            token_1_decimals: event.token_1_decimals,
            signature: event.signature,
            slot: event.slot,
            block_time: event.block_time,
            created_at: event.created_at.to_rfc3339(),
        }
    }
}

impl QueryLpChangeEventsRequest {
    /// 获取页码（默认1）
    pub fn get_page(&self) -> u64 {
        self.page.unwrap_or(1).max(1)
    }

    /// 获取每页大小（默认20，最大100）
    pub fn get_page_size(&self) -> u64 {
        self.page_size.unwrap_or(20).min(100).max(1)
    }

    /// 获取跳过的记录数
    pub fn get_skip(&self) -> u64 {
        (self.get_page() - 1) * self.get_page_size()
    }

    /// 解析lp_mints字符串为Vector
    pub fn parse_lp_mints(&self) -> Option<Vec<String>> {
        self.lp_mints.as_ref().map(|mints| {
            mints
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
    }
}

impl LpChangeEventsPageResponse {
    /// 创建分页响应
    pub fn new(data: Vec<LpChangeEvent>, total: u64, page: u64, page_size: u64) -> Self {
        let total_pages = if total == 0 { 1 } else { (total + page_size - 1) / page_size };

        Self {
            data: data.into_iter().map(LpChangeEventResponse::from).collect(),
            total,
            page,
            page_size,
            total_pages,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_query_request_defaults() {
        let request = QueryLpChangeEventsRequest {
            user_wallet: None,
            pool_id: None,
            lp_mints: None,
            change_type: None,
            start_time: None,
            end_time: None,
            page: None,
            page_size: None,
        };

        assert_eq!(request.get_page(), 1);
        assert_eq!(request.get_page_size(), 20);
        assert_eq!(request.get_skip(), 0);
    }

    #[test]
    fn test_query_request_page_size_limits() {
        let request = QueryLpChangeEventsRequest {
            user_wallet: None,
            pool_id: None,
            lp_mints: None,
            change_type: None,
            start_time: None,
            end_time: None,
            page: Some(2),
            page_size: Some(200), // 超过最大值
        };

        assert_eq!(request.get_page(), 2);
        assert_eq!(request.get_page_size(), 100); // 被限制为最大值
        assert_eq!(request.get_skip(), 100);
    }

    #[test]
    fn test_parse_lp_mints() {
        let request = QueryLpChangeEventsRequest {
            user_wallet: None,
            pool_id: None,
            lp_mints: Some("mint1,mint2, mint3 ,".to_string()),
            change_type: None,
            start_time: None,
            end_time: None,
            page: None,
            page_size: None,
        };

        let mints = request.parse_lp_mints().unwrap();
        assert_eq!(mints, vec!["mint1", "mint2", "mint3"]);
    }

    #[test]
    fn test_create_request_to_model() {
        let request = CreateLpChangeEventRequest {
            user_wallet: "test_wallet".to_string(),
            pool_id: "test_pool".to_string(),
            lp_mint: "test_lp_mint".to_string(),
            token_0_mint: "test_token_0".to_string(),
            token_1_mint: "test_token_1".to_string(),
            change_type: 0,
            lp_amount_before: 1000,
            lp_amount_after: 2000,
            lp_amount_change: 1000,
            token_0_amount: 500,
            token_1_amount: 500,
            token_0_transfer_fee: 10,
            token_1_transfer_fee: 10,
            token_0_vault_before: 10000,
            token_1_vault_before: 10000,
            token_0_vault_after: 10500,
            token_1_vault_after: 10500,
            lp_mint_program_id: "test_program".to_string(),
            token_0_program_id: "test_program".to_string(),
            token_1_program_id: "test_program".to_string(),
            lp_mint_decimals: 9,
            token_0_decimals: 9,
            token_1_decimals: 9,
            signature: "test_signature".to_string(),
            slot: 12345,
            block_time: Some(1234567890),
        };

        let model = request.to_model();
        assert_eq!(model.user_wallet, "test_wallet");
        assert_eq!(model.change_type, 0);
        assert_eq!(model.signature, "test_signature");
    }

    fn create_test_event() -> LpChangeEvent {
        LpChangeEvent {
            id: None,
            user_wallet: "test_wallet".to_string(),
            pool_id: "test_pool".to_string(),
            lp_mint: "test_lp_mint".to_string(),
            token_0_mint: "test_token_0".to_string(),
            token_1_mint: "test_token_1".to_string(),
            change_type: 0,
            lp_amount_before: 1000,
            lp_amount_after: 2000,
            lp_amount_change: 1000,
            token_0_amount: 500,
            token_1_amount: 500,
            token_0_transfer_fee: 10,
            token_1_transfer_fee: 10,
            token_0_vault_before: 10000,
            token_1_vault_before: 10000,
            token_0_vault_after: 10500,
            token_1_vault_after: 10500,
            lp_mint_program_id: "test_program".to_string(),
            token_0_program_id: "test_program".to_string(),
            token_1_program_id: "test_program".to_string(),
            lp_mint_decimals: 9,
            token_0_decimals: 9,
            token_1_decimals: 9,
            signature: "test_signature".to_string(),
            slot: 12345,
            block_time: Some(1234567890),
            created_at: Utc::now(),
        }
    }

    #[test]
    fn test_event_to_response() {
        let event = create_test_event();
        let response = LpChangeEventResponse::from(event);

        assert_eq!(response.user_wallet, "test_wallet");
        assert_eq!(response.change_type, 0);
        assert_eq!(response.change_type_name, "deposit");
        assert_eq!(response.lp_amount_before, "1000");
        assert_eq!(response.signature, "test_signature");
    }

    #[test]
    fn test_page_response_creation() {
        let events = vec![create_test_event()];
        let response = LpChangeEventsPageResponse::new(events, 1, 1, 20);

        assert_eq!(response.data.len(), 1);
        assert_eq!(response.total, 1);
        assert_eq!(response.page, 1);
        assert_eq!(response.page_size, 20);
        assert_eq!(response.total_pages, 1);
    }
}