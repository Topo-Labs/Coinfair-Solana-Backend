use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// 交易积分详情单条记录
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TransactionDetailItem {
    /// 交易签名（交易hash）
    pub signature: String,

    /// 是否是首笔交易
    #[serde(rename = "is_first_transaction")]
    pub is_first_transaction: bool,

    /// 积分获得数量
    #[serde(rename = "points_gained_amount")]
    pub points_gained_amount: u64,

    /// 积分获取时间
    #[serde(rename = "points_gained_time")]
    pub points_gained_time: DateTime<Utc>,
}

/// 用户交易积分详情列表响应数据
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TransactionDetailData {
    /// 用户钱包地址
    pub user_wallet: String,

    /// 积分列表
    pub point_list: Vec<TransactionDetailItem>,

    /// 总记录数
    pub total: u64,

    /// 当前页码
    pub page: u64,

    /// 每页数量
    pub page_size: u64,

    /// 总页数
    pub total_pages: u64,
}

/// 用户交易积分详情列表响应
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TransactionDetailResponse {
    /// 请求ID
    pub id: String,

    /// 是否成功
    pub success: bool,

    /// 错误信息
    pub error: Option<String>,

    /// 响应数据
    pub data: Option<TransactionDetailData>,
}

impl TransactionDetailResponse {
    /// 创建成功响应
    pub fn success(data: TransactionDetailData) -> Self {
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
