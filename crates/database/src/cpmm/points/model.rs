use chrono::{DateTime, Utc};
use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// 自定义日期时间反序列化器，兼容字符串和MongoDB日期对象格式
mod flexible_datetime {
    use chrono::{DateTime, Utc};
    use mongodb::bson;
    use serde::{Deserialize, Deserializer};

    pub fn deserialize<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = bson::Bson::deserialize(deserializer)?;

        match value {
            // 处理字符串格式的日期时间
            bson::Bson::String(s) => s
                .parse::<DateTime<Utc>>()
                .map_err(|e| serde::de::Error::custom(format!("Failed to parse datetime string '{}': {}", s, e))),
            // 处理MongoDB BSON日期对象格式
            bson::Bson::DateTime(dt) => Ok(DateTime::<Utc>::from_timestamp_millis(dt.timestamp_millis())
                .ok_or_else(|| serde::de::Error::custom("Invalid timestamp"))?),
            // 处理其他可能的格式
            other => Err(serde::de::Error::custom(format!(
                "Expected datetime string or BSON DateTime, found: {:?}",
                other
            ))),
        }
    }
}

/// 用户积分汇总表模型
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UserPointsSummary {
    /// MongoDB对象ID
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,

    /// 用户钱包地址（唯一主键）
    #[serde(rename = "userWallet")]
    pub user_wallet: String,

    /// 交易获得的积分：首笔交易200积分，后续每笔10积分
    #[serde(rename = "pointsFromTransaction")]
    pub points_from_transaction: u64,

    /// 用户铸造的NFT被别人领取获得的积分：每个NFT被领取获得300积分（可累计）
    #[serde(rename = "pointsFromNftClaimed")]
    pub points_from_nft_claimed: u64,

    /// 用户领取别人的NFT获得的积分：每个人只能领取一个NFT，获得200积分（一次性）
    #[serde(rename = "pointFromClaimNft")]
    pub point_from_claim_nft: u64,

    /// 关注X account获得的积分：关注后获得200积分（一次性）
    #[serde(rename = "pointFromFollowXAccount")]
    pub point_from_follow_x_account: u64,

    /// 加入telegram获得的积分：加入后获得200积分（一次性）
    #[serde(rename = "pointFromJoinTelegram")]
    pub point_from_join_telegram: u64,

    /// 初始化来源：SwapEvent 或 ClaimNFTEvent
    #[serde(rename = "recordInitFrom")]
    pub record_init_from: String,

    /// 初始化时间
    #[serde(rename = "recordInitTime", deserialize_with = "flexible_datetime::deserialize")]
    pub record_init_time: DateTime<Utc>,

    /// 最后更新来源：SwapEvent 或 ClaimNFTEvent
    #[serde(rename = "recordUpdateFrom")]
    pub record_update_from: String,

    /// 最后更新时间
    #[serde(rename = "recordUpdateTime", deserialize_with = "flexible_datetime::deserialize")]
    pub record_update_time: DateTime<Utc>,
}

impl UserPointsSummary {
    /// 从SwapEvent创建新的用户积分记录（首笔交易）
    pub fn new_from_first_swap(user_wallet: String) -> Self {
        let now = Utc::now();
        Self {
            id: None,
            user_wallet,
            points_from_transaction: 200, // 首笔交易获得200积分
            points_from_nft_claimed: 0,
            point_from_claim_nft: 0,
            point_from_follow_x_account: 0,
            point_from_join_telegram: 0,
            record_init_from: "swap_event".to_string(),
            record_init_time: now,
            record_update_from: "swap_event".to_string(),
            record_update_time: now,
        }
    }

    /// 从ClaimNFTEvent创建新的upper用户积分记录
    pub fn new_from_claim_nft_upper(user_wallet: String) -> Self {
        let now = Utc::now();
        Self {
            id: None,
            user_wallet,
            points_from_transaction: 0,
            points_from_nft_claimed: 300, // NFT被领取获得300积分
            point_from_claim_nft: 0,
            point_from_follow_x_account: 0,
            point_from_join_telegram: 0,
            record_init_from: "claim_nft_event".to_string(),
            record_init_time: now,
            record_update_from: "claim_nft_event".to_string(),
            record_update_time: now,
        }
    }

    /// 从ClaimNFTEvent创建新的claimer用户积分记录
    pub fn new_from_claim_nft_claimer(user_wallet: String) -> Self {
        let now = Utc::now();
        Self {
            id: None,
            user_wallet,
            points_from_transaction: 0,
            points_from_nft_claimed: 0,
            point_from_claim_nft: 200, // 领取NFT获得200积分
            point_from_follow_x_account: 0,
            point_from_join_telegram: 0,
            record_init_from: "claim_nft_event".to_string(),
            record_init_time: now,
            record_update_from: "claim_nft_event".to_string(),
            record_update_time: now,
        }
    }

    /// 更新交易积分（后续交易累加10积分）
    pub fn update_transaction_points(&mut self) {
        self.points_from_transaction += 10;
        self.record_update_from = "swap_event".to_string();
        self.record_update_time = Utc::now();
    }

    /// 更新NFT被领取积分（累加300积分）
    pub fn update_nft_claimed_points(&mut self) {
        self.points_from_nft_claimed += 300;
        self.record_update_from = "claim_nft_event".to_string();
        self.record_update_time = Utc::now();
    }

    /// 更新领取NFT积分（一次性设置为200）
    pub fn update_claim_nft_points(&mut self) {
        self.point_from_claim_nft = 200;
        self.record_update_from = "claim_nft_event".to_string();
        self.record_update_time = Utc::now();
    }

    /// 计算用户总积分
    pub fn total_points(&self) -> u64 {
        self.points_from_transaction
            + self.points_from_nft_claimed
            + self.point_from_claim_nft
            + self.point_from_follow_x_account
            + self.point_from_join_telegram
    }
}

/// 用户积分查询参数
#[derive(Debug, Clone, Default)]
pub struct UserPointsQuery {
    /// 用户钱包地址过滤
    pub user_wallet: Option<String>,
    /// 最小总积分过滤
    pub min_total_points: Option<u64>,
    /// 排序字段（total_points, record_update_time等）
    pub sort_by: Option<String>,
    /// 排序方向（asc, desc）
    pub sort_order: Option<String>,
    /// 分页参数
    pub page: Option<i64>,
    pub limit: Option<i64>,
}

/// 积分统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPointsStats {
    /// 总用户数
    pub total_users: u64,
    /// 总积分发放量
    pub total_points_distributed: u64,
    /// 平均每用户积分
    pub average_points_per_user: f64,
    /// 最高积分
    pub max_points: u64,
    /// 最低积分
    pub min_points: u64,
}

/// 用户积分和排名信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPointsWithRank {
    /// 用户积分记录
    #[serde(flatten)]
    pub user: UserPointsSummary,
    /// 用户排名
    pub rank: u64,
    /// 总积分
    pub total_points: u64,
}

/// 用户排名信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserRankInfo {
    /// 用户钱包地址
    pub user_wallet: String,
    /// 用户排名
    pub rank: u64,
    /// 总积分
    pub total_points: u64,
}
