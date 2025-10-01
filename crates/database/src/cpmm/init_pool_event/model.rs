use chrono::{DateTime, Utc};
use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitPoolEvent {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,

    // 池子信息
    pub pool_id: String,
    pub pool_creator: String,
    pub token_0_mint: String,
    pub token_1_mint: String,
    pub token_0_vault: String,
    pub token_1_vault: String,
    pub lp_mint: String,

    // AMM配置ID（从链上PoolState获取）
    // 旧数据可能没有此字段，所以设为 Option
    #[serde(default)]
    pub amm_config: Option<String>,

    // 程序ID
    pub lp_program_id: String,
    pub token_0_program_id: String,
    pub token_1_program_id: String,
    pub lp_mint_decimals: u8,
    pub token_0_decimals: u8,
    pub token_1_decimals: u8,

    // 交易信息
    pub signature: String,
    pub slot: u64,
    pub block_time: Option<i64>,

    // 时间戳
    pub created_at: DateTime<Utc>,
}

/// 用户池子创建统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPoolStats {
    pub total_pools_created: u64,
    pub first_pool_created_at: Option<String>,
    pub latest_pool_created_at: Option<String>,
}
