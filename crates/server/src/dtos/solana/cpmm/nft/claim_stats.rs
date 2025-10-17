/// NFT 领取统计相关的 DTO
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

/// 推荐人统计响应
/// 统计每个推荐人的推荐效果：推荐人数、被推荐人列表等
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ReferrerStatsResponse {
    /// 推荐人地址
    #[schema(example = "9ZNTfG4NyQgxy2SWjSiQoUyBPEvXT2xo7fKc5hPYYJ7b")]
    pub referrer: String,

    /// 推荐人数（被推荐并领取的用户数）
    #[schema(example = 5)]
    pub referred_count: u64,

    /// 最新推荐领取时间（Unix时间戳）
    #[schema(example = 1735203600)]
    pub latest_claim_time: Option<i64>,

    /// 最早推荐领取时间（Unix时间戳）
    #[schema(example = 1704067200)]
    pub earliest_claim_time: Option<i64>,

    /// 被推荐人列表（去重的claimer地址列表）
    #[schema(example = json!(["8S2bcP66WehuF6cHryfZ7vfFpQWaUhYyAYSy5U3gX4Fy", "AnotherClaimer123"]))]
    pub claimers: Vec<String>,
}

/// 推荐人统计查询参数（分页）
#[derive(Debug, Deserialize, IntoParams)]
pub struct ReferrerStatsQuery {
    /// 页码
    #[serde(default = "default_page")]
    pub page: u32,

    /// 每页条数
    #[serde(default = "default_page_size")]
    pub page_size: u32,

    /// 排序字段（referred_count, latest_claim_time等）
    pub sort_by: Option<String>,

    /// 排序方向（asc, desc）
    pub sort_order: Option<String>,
}

/// 推荐人统计分页响应
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PaginatedReferrerStatsResponse {
    /// 统计数据列表
    pub items: Vec<ReferrerStatsResponse>,

    /// 总记录数
    pub total: u64,

    /// 当前页码
    pub page: u64,

    /// 每页条数
    pub page_size: u64,

    /// 总页数
    pub total_pages: u64,
}

// ==================== 辅助函数 ====================

/// 默认页码
fn default_page() -> u32 {
    1
}

/// 默认每页条数
fn default_page_size() -> u32 {
    20
}

// ==================== 转换函数 ====================

impl From<database::events::event_model::repository::ReferrerStats> for ReferrerStatsResponse {
    fn from(stats: database::events::event_model::repository::ReferrerStats) -> Self {
        Self {
            referrer: stats.referrer,
            referred_count: stats.referred_count,
            latest_claim_time: stats.latest_claim_time,
            earliest_claim_time: stats.earliest_claim_time,
            claimers: stats.claimers,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_referrer_stats_response_serialization() {
        let response = ReferrerStatsResponse {
            referrer: "9ZNTfG4NyQgxy2SWjSiQoUyBPEvXT2xo7fKc5hPYYJ7b".to_string(),
            referred_count: 5,
            latest_claim_time: Some(1735203600),
            earliest_claim_time: Some(1704067200),
            claimers: vec!["8S2bcP66WehuF6cHryfZ7vfFpQWaUhYyAYSy5U3gX4Fy".to_string()],
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("9ZNTfG4NyQgxy2SWjSiQoUyBPEvXT2xo7fKc5hPYYJ7b"));
        assert!(json.contains("\"referred_count\":5"));
        assert!(json.contains("8S2bcP66WehuF6cHryfZ7vfFpQWaUhYyAYSy5U3gX4Fy"));
    }

    #[test]
    fn test_referrer_stats_response_deserialization() {
        let json = r#"{
            "referrer": "9ZNTfG4NyQgxy2SWjSiQoUyBPEvXT2xo7fKc5hPYYJ7b",
            "referred_count": 5,
            "latest_claim_time": 1735203600,
            "earliest_claim_time": 1704067200,
            "claimers": ["8S2bcP66WehuF6cHryfZ7vfFpQWaUhYyAYSy5U3gX4Fy"]
        }"#;

        let response: ReferrerStatsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.referrer, "9ZNTfG4NyQgxy2SWjSiQoUyBPEvXT2xo7fKc5hPYYJ7b");
        assert_eq!(response.referred_count, 5);
        assert_eq!(response.latest_claim_time, Some(1735203600));
        assert_eq!(response.earliest_claim_time, Some(1704067200));
        assert_eq!(response.claimers.len(), 1);
    }

    #[test]
    fn test_from_database_referrer_stats() {
        let db_stats = database::events::event_model::repository::ReferrerStats {
            referrer: "9ZNTfG4NyQgxy2SWjSiQoUyBPEvXT2xo7fKc5hPYYJ7b".to_string(),
            referred_count: 10,
            latest_claim_time: Some(1735203600),
            earliest_claim_time: Some(1704067200),
            claimers: vec!["8S2bcP66WehuF6cHryfZ7vfFpQWaUhYyAYSy5U3gX4Fy".to_string()],
        };

        let response: ReferrerStatsResponse = db_stats.into();

        assert_eq!(response.referrer, "9ZNTfG4NyQgxy2SWjSiQoUyBPEvXT2xo7fKc5hPYYJ7b");
        assert_eq!(response.referred_count, 10);
        assert_eq!(response.latest_claim_time, Some(1735203600));
        assert_eq!(response.earliest_claim_time, Some(1704067200));
        assert_eq!(response.claimers.len(), 1);
    }

    #[test]
    fn test_from_database_referrer_stats_with_empty_claimers() {
        let db_stats = database::events::event_model::repository::ReferrerStats {
            referrer: "9ZNTfG4NyQgxy2SWjSiQoUyBPEvXT2xo7fKc5hPYYJ7b".to_string(),
            referred_count: 0,
            latest_claim_time: None,
            earliest_claim_time: None,
            claimers: vec![], // 没有被推荐人
        };

        let response: ReferrerStatsResponse = db_stats.into();

        assert_eq!(response.referrer, "9ZNTfG4NyQgxy2SWjSiQoUyBPEvXT2xo7fKc5hPYYJ7b");
        assert_eq!(response.referred_count, 0);
        assert_eq!(response.claimers.len(), 0);
        assert!(response.claimers.is_empty());
    }

    #[test]
    fn test_referrer_stats_response_with_multiple_claimers() {
        let response = ReferrerStatsResponse {
            referrer: "9ZNTfG4NyQgxy2SWjSiQoUyBPEvXT2xo7fKc5hPYYJ7b".to_string(),
            referred_count: 5,
            latest_claim_time: Some(1735203600),
            earliest_claim_time: Some(1704067200),
            claimers: vec![
                "Claimer1".to_string(),
                "Claimer2".to_string(),
            ],
        };

        assert_eq!(response.claimers.len(), 2);
        assert!(response.claimers.contains(&"Claimer1".to_string()));
        assert!(response.claimers.contains(&"Claimer2".to_string()));
    }
}
