use chrono::{DateTime, Local};
use alloy::primitives::{keccak256, FixedBytes, B256};
use std::time::SystemTime;

// 工具函数: 获取当前的日期和时间
pub fn current_date_and_time() -> String {
    let now_time = SystemTime::now();
    let now: DateTime<Local> = now_time.into();
    let formatted_time = now.format("%Y-%m-%d %H:%M:%S").to_string();
    formatted_time
}

// 工具函数：计算事件签名的 Keccak256 哈希
pub fn keccak256_hash(input: &str) -> FixedBytes<32> {
    keccak256(input.as_bytes())
}

// 工具函数：计算事件签名的 Keccak256 哈希
pub fn magic_number(event_signature: &str) -> B256 {
    B256::from(keccak256_hash(event_signature))
}
