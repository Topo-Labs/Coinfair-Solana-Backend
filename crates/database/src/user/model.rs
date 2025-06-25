use chrono::prelude::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct User {
    pub address: String, // Address
    pub amount: String,  // 购买的Hope数量（取整后，存字符串）
    pub price: String,   // 购买时刻Hope对U的价格（保留20位小数，存字符串）
    pub timestamp: u64,  // 落库时刻：17112341234
}
