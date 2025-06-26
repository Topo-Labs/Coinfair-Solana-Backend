use chrono::prelude::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::Validate;

/// 用户模型
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct User {
    /// 用户钱包地址
    pub address: String, // Address
    /// 购买金额
    pub amount: String,  // Amount
    /// 购买时的代币价格
    pub price: String,   // Price
    /// 购买时间戳
    pub timestamp: u64,  // 1734187238
}
