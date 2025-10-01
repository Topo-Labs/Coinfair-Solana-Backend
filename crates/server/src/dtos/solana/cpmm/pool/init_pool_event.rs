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

    /// AMM配置地址
    #[validate(custom = "validate_pubkey")]
    pub amm_config: String,

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
    pub amm_config: Option<String>,
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
            amm_config: event.amm_config,
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
            amm_config: Some(request.amm_config),
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

/// CPMM配置信息（用于前端展示）
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ConfigInfo {
    /// 配置ID
    pub id: String,
    /// 配置索引
    pub index: u32,
    /// 协议费率
    #[serde(rename = "protocolFeeRate")]
    pub protocol_fee_rate: u64,
    /// 交易费率
    #[serde(rename = "tradeFeeRate")]
    pub trade_fee_rate: u64,
    /// 基金费率
    #[serde(rename = "fundFeeRate")]
    pub fund_fee_rate: u64,
    /// 创建池子费用
    #[serde(rename = "createPoolFee")]
    pub create_pool_fee: String,
    /// 创建者费率
    #[serde(rename = "creatorFeeRate")]
    pub creator_fee_rate: u64,
}

/// 代币基础信息（用于前端展示）
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct MintInfo {
    /// Logo URI
    #[serde(rename = "logoURI")]
    pub logo_uri: String,
    /// 代币符号
    pub symbol: String,
    /// 代币名称
    pub name: String,
}

/// 带详细信息的池子初始化事件响应
#[derive(Debug, Serialize, ToSchema)]
pub struct InitPoolEventDetailedResponse {
    /// 基础事件信息
    #[serde(flatten)]
    pub event: InitPoolEventResponse,

    /// 配置信息
    pub config: Option<ConfigInfo>,

    /// Token A 信息
    #[serde(rename = "mintA")]
    pub mint_a: Option<MintInfo>,

    /// Token B 信息
    #[serde(rename = "mintB")]
    pub mint_b: Option<MintInfo>,

    /// Token A 数量（从池子 vault 实时查询，字符串格式避免科学计数法）
    #[serde(rename = "mintAmountA", skip_serializing_if = "Option::is_none")]
    pub mint_amount_a: Option<String>,

    /// Token B 数量（从池子 vault 实时查询，字符串格式避免科学计数法）
    #[serde(rename = "mintAmountB", skip_serializing_if = "Option::is_none")]
    pub mint_amount_b: Option<String>,

    /// 价格（wsol / token，基于 vault 余额计算，字符串格式保留8位小数）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price: Option<String>,

    /// 手续费率（从 config.protocolFeeRate / 10000 计算，保留4位小数）
    #[serde(rename = "feeRate", skip_serializing_if = "Option::is_none")]
    pub fee_rate: Option<String>,
}

/// 带详细信息的分页响应
#[derive(Debug, Serialize, ToSchema)]
pub struct InitPoolEventsDetailedPageResponse {
    pub data: Vec<InitPoolEventDetailedResponse>,
    pub total: u64,
    pub page: u64,
    pub page_size: u64,
    pub total_pages: u64,
}
