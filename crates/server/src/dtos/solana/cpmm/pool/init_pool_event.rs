use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::Validate;

use crate::dtos::solana::common::validate_pubkey;
use database::cpmm::init_pool_event::model::InitPoolEvent;

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct CreateInitPoolEventRequest {
    /// 池子地址
    #[validate(custom = "validate_pubkey")]
    pub pool_id: String,

    /// 池子创建者地址
    #[validate(custom = "validate_pubkey")]
    pub pool_creator: String,

    /// Token 0 mint地址
    #[validate(custom = "validate_pubkey")]
    pub token_0_mint: String,

    /// Token 1 mint地址
    #[validate(custom = "validate_pubkey")]
    pub token_1_mint: String,

    /// Token 0 金库地址
    #[validate(custom = "validate_pubkey")]
    pub token_0_vault: String,

    /// Token 1 金库地址
    #[validate(custom = "validate_pubkey")]
    pub token_1_vault: String,

    /// LP mint地址
    #[validate(custom = "validate_pubkey")]
    pub lp_mint: String,

    /// LP程序ID
    #[validate(custom = "validate_pubkey")]
    pub lp_program_id: String,

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

#[derive(Debug, Deserialize, ToSchema, Validate)]
pub struct QueryInitPoolEventsRequest {
    pub pool_ids: Option<String>, // 支持多个pool_id，英文逗号分隔
    pub pool_creator: Option<String>,
    pub lp_mint: Option<String>,
    pub token_0_mint: Option<String>,
    pub token_1_mint: Option<String>,
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub page: Option<u64>,
    pub page_size: Option<u64>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct InitPoolEventResponse {
    pub id: String,
    pub pool_id: String,
    pub pool_creator: String,
    pub token_0_mint: String,
    pub token_1_mint: String,
    pub token_0_vault: String,
    pub token_1_vault: String,
    pub lp_program_id: String,
    pub lp_mint: String,
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

#[derive(Debug, Serialize, ToSchema)]
pub struct InitPoolEventsPageResponse {
    pub data: Vec<InitPoolEventResponse>,
    pub total: u64,
    pub page: u64,
    pub page_size: u64,
    pub total_pages: u64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UserPoolStats {
    pub total_pools_created: u64,
    pub first_pool_created_at: Option<String>,
    pub latest_pool_created_at: Option<String>,
}

impl From<InitPoolEvent> for InitPoolEventResponse {
    fn from(event: InitPoolEvent) -> Self {
        Self {
            id: event.id.map(|id| id.to_hex()).unwrap_or_default(),
            pool_id: event.pool_id,
            pool_creator: event.pool_creator,
            token_0_mint: event.token_0_mint,
            token_1_mint: event.token_1_mint,
            token_0_vault: event.token_0_vault,
            token_1_vault: event.token_1_vault,
            lp_program_id: event.lp_program_id,
            lp_mint: event.lp_mint,
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

impl From<CreateInitPoolEventRequest> for InitPoolEvent {
    fn from(request: CreateInitPoolEventRequest) -> Self {
        Self {
            id: None,
            pool_id: request.pool_id,
            pool_creator: request.pool_creator,
            token_0_mint: request.token_0_mint,
            token_1_mint: request.token_1_mint,
            token_0_vault: request.token_0_vault,
            token_1_vault: request.token_1_vault,
            lp_mint: request.lp_mint,
            lp_program_id: request.lp_program_id,
            token_0_program_id: request.token_0_program_id,
            token_1_program_id: request.token_1_program_id,
            lp_mint_decimals: request.lp_mint_decimals,
            token_0_decimals: request.token_0_decimals,
            token_1_decimals: request.token_1_decimals,
            signature: request.signature,
            slot: request.slot,
            block_time: request.block_time,
            created_at: Utc::now(),
        }
    }
}
