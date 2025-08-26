use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use validator::Validate;

use crate::dtos::solana::position::open_position::{OpenPositionRequest, OpenPositionResponse, PositionInfo};

// ============ Position Storage API相关DTO ============

/// 仓位存储请求DTO
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct PositionStorageRequest {
    /// 开仓请求信息
    pub open_position_request: OpenPositionRequest,
    /// 开仓响应信息
    pub open_position_response: OpenPositionResponse,
    /// 交易签名（可选）
    pub transaction_signature: Option<String>,
}

/// 仓位更新请求DTO
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct PositionUpdateRequest {
    /// 仓位键值
    #[validate(length(min = 32, max = 44))]
    pub position_key: String,
    /// 操作类型 ("increase", "decrease", "close")
    #[validate(length(min = 1))]
    pub operation_type: String,
    /// 流动性变化数量
    pub liquidity_change: String,
    /// token0数量变化
    pub amount_0_change: u64,
    /// token1数量变化
    pub amount_1_change: u64,
    /// 交易签名（可选）
    pub transaction_signature: Option<String>,
}

/// 仓位查询请求DTO
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema, IntoParams)]
pub struct PositionQueryRequest {
    /// 用户钱包地址（可选）
    #[validate(length(min = 32, max = 44))]
    pub user_wallet: Option<String>,
    /// 池子地址（可选）
    #[validate(length(min = 32, max = 44))]
    pub pool_address: Option<String>,
    /// 仓位状态过滤 ("Active", "Closed", "Paused", "Error")
    pub status: Option<String>,
    /// 是否只显示活跃仓位
    pub active_only: Option<bool>,
    /// 页码（从1开始）
    #[validate(range(min = 1))]
    pub page: Option<u64>,
    /// 每页大小
    #[validate(range(min = 1, max = 100))]
    pub page_size: Option<u64>,
}

impl Default for PositionQueryRequest {
    fn default() -> Self {
        Self {
            user_wallet: None,
            pool_address: None,
            status: None,
            active_only: Some(true),
            page: Some(1),
            page_size: Some(20),
        }
    }
}

/// 链下仓位详细信息DTO
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct StoredPositionInfo {
    /// 基本仓位信息（复用现有的PositionInfo）
    #[serde(flatten)]
    pub position_info: PositionInfo,
    /// 链下存储的额外信息
    pub storage_info: PositionStorageInfo,
}

/// 仓位存储信息DTO
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PositionStorageInfo {
    /// 初始流动性
    pub initial_liquidity: String,
    /// 当前流动性
    pub current_liquidity: String,
    /// 累计增加的流动性
    pub total_liquidity_added: String,
    /// 累计减少的流动性
    pub total_liquidity_removed: String,
    /// 仓位状态
    pub status: String,
    /// 是否活跃
    pub is_active: bool,
    /// 是否在价格范围内
    pub is_in_range: bool,
    /// 累计赚取的手续费
    pub total_fees_earned_0: u64,
    pub total_fees_earned_1: u64,
    /// 未领取的手续费
    pub unclaimed_fees_0: u64,
    pub unclaimed_fees_1: u64,
    /// 总操作次数
    pub total_operations: u32,
    /// 最后操作类型
    pub last_operation_type: Option<String>,
    /// 创建时间
    pub created_at: i64,
    /// 最后更新时间
    pub updated_at: i64,
    /// 最后同步时间
    pub last_sync_at: Option<i64>,
    /// 扩展元数据
    pub metadata: Option<serde_json::Value>,
}

/// 仓位列表响应DTO
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PositionListResponse {
    /// 仓位列表
    pub positions: Vec<StoredPositionInfo>,
    /// 分页信息
    pub pagination: PaginationInfo,
    /// 统计信息
    pub statistics: PositionListStatistics,
    /// 查询时间戳
    pub timestamp: i64,
}

/// 分页信息DTO
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PaginationInfo {
    /// 当前页码
    pub current_page: u64,
    /// 每页大小
    pub page_size: u64,
    /// 总记录数
    pub total_count: u64,
    /// 总页数
    pub total_pages: u64,
    /// 是否有下一页
    pub has_next: bool,
    /// 是否有上一页
    pub has_previous: bool,
}

/// 仓位列表统计信息DTO
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PositionListStatistics {
    /// 总仓位数
    pub total_positions: u64,
    /// 活跃仓位数
    pub active_positions: u64,
    /// 已关闭仓位数
    pub closed_positions: u64,
    /// 总流动性
    pub total_liquidity: String,
    /// 总手续费收益
    pub total_fees_earned_0: u64,
    pub total_fees_earned_1: u64,
}

/// 用户仓位统计响应DTO
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UserPositionStatsResponse {
    /// 用户钱包地址
    pub user_wallet: String,
    /// 统计信息
    pub statistics: PositionListStatistics,
    /// 按池子分组的统计
    pub pool_breakdown: Vec<PoolPositionBreakdown>,
    /// 查询时间戳
    pub timestamp: i64,
}

/// 池子仓位分解统计DTO
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PoolPositionBreakdown {
    /// 池子地址
    pub pool_address: String,
    /// 该池子中的仓位数量
    pub position_count: u64,
    /// 该池子中的总流动性
    pub total_liquidity: String,
    /// 该池子中的手续费收益
    pub fees_earned_0: u64,
    pub fees_earned_1: u64,
}

/// 池子仓位统计响应DTO
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PoolPositionStatsResponse {
    /// 池子地址
    pub pool_address: String,
    /// 总仓位数
    pub total_positions: u64,
    /// 活跃仓位数
    pub active_positions: u64,
    /// 唯一用户数
    pub unique_users: u64,
    /// 总流动性
    pub total_liquidity: String,
    /// 平均仓位大小
    pub average_position_size: String,
    /// 按用户分组的前10名
    pub top_users: Vec<UserPositionSummary>,
    /// 查询时间戳
    pub timestamp: i64,
}

/// 用户仓位摘要DTO
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UserPositionSummary {
    /// 用户钱包地址
    pub user_wallet: String,
    /// 该用户的仓位数量
    pub position_count: u64,
    /// 该用户的总流动性
    pub total_liquidity: String,
    /// 该用户的手续费收益
    pub fees_earned_0: u64,
    pub fees_earned_1: u64,
}
