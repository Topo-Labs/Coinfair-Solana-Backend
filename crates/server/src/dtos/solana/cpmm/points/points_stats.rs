use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// 排行榜单个条目
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RankItem {
    /// 排名
    #[serde(rename = "rank_no")]
    pub rank_no: u64,

    /// 总积分
    pub points: u64,

    /// 用户钱包地址
    pub user: String,
}

/// 积分排行榜统计响应数据
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PointsStatsData {
    /// 排行榜列表
    pub rank_list: Vec<RankItem>,

    /// 我的钱包地址
    pub my_wallet: String,

    /// 我的总积分
    pub my_points: u64,

    /// 我的排名（0表示未上榜）
    pub my_rank: u64,

    /// 总记录数
    pub total: u64,

    /// 当前页码
    pub page: u64,

    /// 每页数量
    pub page_size: u64,

    /// 总页数
    pub total_pages: u64,
}

/// 积分排行榜统计响应
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PointsStatsResponse {
    /// 请求ID
    pub id: String,

    /// 是否成功
    pub success: bool,

    /// 错误信息
    pub error: Option<String>,

    /// 响应数据
    pub data: Option<PointsStatsData>,
}

impl PointsStatsResponse {
    /// 创建成功响应
    pub fn success(data: PointsStatsData) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            success: true,
            error: None,
            data: Some(data),
        }
    }

    /// 创建失败响应
    pub fn error(error: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            success: false,
            error: Some(error),
            data: None,
        }
    }
}
