use chrono::prelude::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::Validate;

/// 推荐关系模型
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct Refer {
    /// 被推荐人地址
    pub lower: String,  // Address
    /// 推荐人地址
    pub upper: String,  // Address
    /// 创建时间戳
    pub timestamp: u64, // 1734187238
}
