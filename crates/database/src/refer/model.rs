use chrono::prelude::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct Refer {
    pub lower: String,  // Address
    pub upper: String,  // Address
    pub timestamp: u64, // 1734187238
}
