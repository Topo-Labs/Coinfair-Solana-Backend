use chrono::prelude::{DateTime, Utc};
use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::Validate;

/// 用户模型
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct User {
    /// MongoDB文档ID
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    /// 用户钱包地址
    pub address: String, // Address
    /// 购买金额
    pub amount: String,  // Amount
    /// 购买时的代币价格
    pub price: String,   // Price
    /// 购买时间戳
    #[serde(with = "mongodb::bson::serde_helpers::u64_as_f64")]
    pub timestamp: u64,  // 1734187238
}
