use chrono::{DateTime, Local};
use ethers::prelude::*;
use std::time::SystemTime;

// 工具函数: 获取当前的日期和时间
pub fn current_date_and_time() -> String {
    let now_time = SystemTime::now();
    let now: DateTime<Local> = now_time.into();
    let formatted_time = now.format("%Y-%m-%d %H:%M:%S").to_string();
    formatted_time
}

// 工具函数：计算事件签名的 Keccak256 哈希
pub fn keccak256(input: &str) -> [u8; 32] {
    use ethers::utils::keccak256;
    keccak256(input)
}

// 工具函数：计算事件签名的 Keccak256 哈希
pub fn magic_number(event_signature: &str) -> TxHash {
    H256::from_slice(&keccak256(event_signature))
}