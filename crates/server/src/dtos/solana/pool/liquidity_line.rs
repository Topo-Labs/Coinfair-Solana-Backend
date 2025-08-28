use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use validator::Validate;

// ========================= 流动性线图相关DTO =========================

/// 流动性线图查询请求参数
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, IntoParams, Validate)]
pub struct PoolLiquidityLineRequest {
    /// 池子地址
    #[validate(length(min = 32, max = 44))]
    pub id: String,

    /// 查询范围（可选，以当前价格为中心的tick范围）
    #[validate(range(min = 100, max = 10000))]
    pub range: Option<i32>,

    /// 最大返回点数（可选，默认100）
    #[validate(range(min = 10, max = 1000))]
    pub max_points: Option<u32>,
}

/// 流动性线图数据点
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct LiquidityLinePoint {
    /// 该tick对应的价格
    pub price: f64,

    /// 该tick的流动性数量（字符串避免精度丢失）
    pub liquidity: String,

    /// tick索引
    pub tick: i32,
}

/// 流动性线图数据
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PoolLiquidityLineData {
    /// 数据点数量
    pub count: u32,

    /// 流动性分布线图数据点列表
    pub line: Vec<LiquidityLinePoint>,
}

/// 流动性线图响应
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PoolLiquidityLineResponse {
    /// 请求ID
    pub id: String,

    /// 请求是否成功
    pub success: bool,

    /// 流动性线图数据
    pub data: PoolLiquidityLineData,
}

/// 流动性线图错误响应
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct LiquidityLineErrorResponse {
    /// 请求ID
    pub id: String,

    /// 请求失败
    pub success: bool,

    /// 错误信息
    pub error: String,

    /// 错误代码（可选）
    pub error_code: Option<String>,
}
