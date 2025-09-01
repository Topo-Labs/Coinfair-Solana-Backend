use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

// ==================== 请求DTO ====================

/// 创建存款事件请求DTO
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateDepositEventRequest {
    // ====== 核心必填字段 ======
    /// 存款用户钱包地址
    pub user: String,

    /// 项目配置地址
    pub project_config: String,

    /// 项目代币mint的地址（用于区分是哪个项目）
    pub token_mint: String,

    /// 存款数量（原始数量，需要根据decimals换算）
    pub amount: u64,

    /// 累计筹资总额
    pub total_raised: u64,

    /// 交易签名（唯一标识）
    pub signature: String,

    /// 存款时间戳
    pub deposited_at: i64,

    /// 区块高度
    pub slot: u64,

    // ====== 代币元数据字段（可选） ======
    /// 代币小数位数
    pub token_decimals: Option<u8>,

    /// 代币名称
    pub token_name: Option<String>,

    /// 代币符号
    pub token_symbol: Option<String>,

    /// 代币Logo URI
    pub token_logo_uri: Option<String>,

    // ====== 业务扩展字段（可选） ======
    /// 存款类型 (0: 初始存款, 1: 追加存款, 2: 应急存款)
    pub deposit_type: Option<u8>,

    /// 关联的流动性池地址（可选）
    pub related_pool: Option<String>,

    /// 预估USD价值
    pub estimated_usd_value: Option<f64>,
}

/// 创建存款事件响应DTO
#[derive(Debug, Serialize, ToSchema)]
pub struct CreateDepositEventResponse {
    /// 创建的存款事件ID
    pub id: String,

    /// 存款用户钱包地址
    pub user: String,

    /// 交易签名
    pub signature: String,

    /// 存款时间戳
    pub deposited_at: i64,

    /// 实际存款金额（考虑decimals后的可读数量）
    pub actual_amount: f64,

    /// 实际累计筹资额（考虑decimals后的可读数量）
    pub actual_total_raised: f64,

    /// 存款类型名称
    pub deposit_type_name: String,

    /// 预估USD价值
    pub estimated_usd_value: f64,

    /// 创建时间（ISO 8601格式）
    pub created_at: String,
}

/// 基础查询参数
#[derive(Debug, Deserialize, IntoParams)]
pub struct DepositEventQuery {
    /// 页码
    #[serde(default = "default_page")]
    pub page: u32,

    /// 每页条数
    #[serde(default = "default_page_size")]
    pub page_size: u32,

    /// 用户地址过滤
    pub user: Option<String>,

    /// 代币mint地址过滤
    pub token_mint: Option<String>,

    /// 项目配置地址过滤
    pub project_config: Option<String>,

    /// 存款类型过滤
    pub deposit_type: Option<u8>,

    /// 开始时间戳
    pub start_date: Option<i64>,

    /// 结束时间戳
    pub end_date: Option<i64>,

    /// 排序字段（deposited_at, amount, total_raised等）
    pub sort_by: Option<String>,

    /// 排序方向（asc, desc）
    pub sort_order: Option<String>,
}

/// 高级查询参数
#[derive(Debug, Deserialize, IntoParams)]
pub struct DepositAdvancedQuery {
    // 基础分页
    /// 页码
    #[serde(default = "default_page")]
    pub page: u32,

    /// 每页条数
    #[serde(default = "default_page_size")]
    pub page_size: u32,

    // 基础过滤
    /// 用户地址过滤
    pub user: Option<String>,

    /// 代币mint地址过滤
    pub token_mint: Option<String>,

    /// 项目配置地址过滤
    pub project_config: Option<String>,

    /// 存款类型过滤
    pub deposit_type: Option<u8>,

    /// 开始时间戳
    pub start_date: Option<i64>,

    /// 结束时间戳
    pub end_date: Option<i64>,

    // 高级过滤
    /// 最小存款金额
    pub amount_min: Option<u64>,

    /// 最大存款金额
    pub amount_max: Option<u64>,

    /// 最小累计筹资额
    pub total_raised_min: Option<u64>,

    /// 最大累计筹资额
    pub total_raised_max: Option<u64>,

    /// 是否为高价值存款
    pub is_high_value_deposit: Option<bool>,

    /// 关联流动性池地址
    pub related_pool: Option<String>,

    /// 最小预估USD价值
    pub estimated_usd_min: Option<f64>,

    /// 最大预估USD价值
    pub estimated_usd_max: Option<f64>,

    /// 代币符号过滤
    pub token_symbol: Option<String>,

    /// 代币名称过滤（模糊匹配）
    pub token_name: Option<String>,

    /// 排序字段
    pub sort_by: Option<String>,

    /// 排序方向
    pub sort_order: Option<String>,
}

/// 用户存款查询参数
#[derive(Debug, Deserialize, IntoParams)]
pub struct UserDepositQuery {
    /// 页码
    #[serde(default = "default_page")]
    pub page: u32,

    /// 每页条数
    #[serde(default = "default_page_size")]
    pub page_size: u32,
}

/// 代币存款查询参数
#[derive(Debug, Deserialize, IntoParams)]
pub struct TokenDepositQuery {
    /// 页码
    #[serde(default = "default_page")]
    pub page: u32,

    /// 每页条数
    #[serde(default = "default_page_size")]
    pub page_size: u32,
}

/// 存款趋势查询参数
#[derive(Debug, Deserialize, IntoParams)]
pub struct DepositTrendQuery {
    /// 趋势周期
    pub period: Option<TrendPeriod>,

    /// 开始时间戳
    pub start_date: Option<i64>,

    /// 结束时间戳
    pub end_date: Option<i64>,
}

// ==================== 响应DTO ====================

/// 存款事件响应
#[derive(Debug, Serialize, ToSchema)]
pub struct DepositEventResponse {
    /// 用户地址
    pub user: String,

    /// 代币mint地址
    pub token_mint: String,

    /// 项目配置地址
    pub project_config: String,

    /// 代币符号
    pub token_symbol: Option<String>,

    /// 代币名称
    pub token_name: Option<String>,

    /// 代币小数位数
    pub token_decimals: Option<u8>,

    /// 原始存款数量（字符串避免精度问题）
    pub amount: String,

    /// 实际可读存款数量
    pub actual_amount: f64,

    /// 原始累计筹资数量（字符串避免精度问题）
    pub total_raised: String,

    /// 实际可读累计筹资数量
    pub actual_total_raised: f64,

    /// 存款类型
    pub deposit_type: u8,

    /// 存款类型名称
    pub deposit_type_name: String,

    /// 是否为高价值存款
    pub is_high_value_deposit: bool,

    /// 预估USD价值
    pub estimated_usd_value: f64,

    /// 关联的流动性池地址
    pub related_pool: Option<String>,

    /// 存款时间（ISO 8601格式）
    pub deposited_at: String,

    /// 交易签名
    pub signature: String,
}

/// 分页响应
#[derive(Debug, Serialize, ToSchema)]
pub struct PaginatedDepositResponse {
    /// 存款事件列表
    pub items: Vec<DepositEventResponse>,

    /// 总记录数
    pub total: u64,

    /// 当前页码
    pub page: u64,

    /// 每页条数
    pub page_size: u64,

    /// 总页数
    pub total_pages: u64,

    /// 独特用户数
    pub unique_users: Option<u64>,
}

/// 存款统计响应
#[derive(Debug, Serialize, ToSchema)]
pub struct DepositStatsResponse {
    /// 总存款数
    pub total_deposits: u64,

    /// 今日存款数
    pub today_deposits: u64,

    /// 独特用户数
    pub unique_users: u64,

    /// 独特代币数
    pub unique_tokens: u64,

    /// 总美元交易量
    pub total_volume_usd: f64,

    /// 今日美元交易量
    pub today_volume_usd: f64,

    /// 存款类型分布
    pub deposit_type_distribution: Vec<DepositTypeDistributionResponse>,

    /// 代币分布（前10）
    pub token_distribution: Vec<TokenDistributionResponse>,
}

/// 存款类型分布响应
#[derive(Debug, Serialize, ToSchema)]
pub struct DepositTypeDistributionResponse {
    /// 存款类型
    pub deposit_type: u8,

    /// 类型名称
    pub name: String,

    /// 数量
    pub count: u64,
}

/// 代币分布响应
#[derive(Debug, Serialize, ToSchema)]
pub struct TokenDistributionResponse {
    /// 代币mint地址
    pub token_mint: String,

    /// 代币符号
    pub token_symbol: Option<String>,

    /// 代币名称
    pub token_name: Option<String>,

    /// 存款次数
    pub count: u64,

    /// 总美元交易量
    pub total_volume_usd: f64,
}

/// 用户存款汇总响应
#[derive(Debug, Serialize, ToSchema)]
pub struct UserDepositSummaryResponse {
    /// 用户地址
    pub user: String,

    /// 总存款数
    pub total_deposits: u64,

    /// 总美元交易量
    pub total_volume_usd: f64,

    /// 独特代币数
    pub unique_tokens: u32,

    /// 首次存款时间（ISO 8601格式）
    pub first_deposit_at: String,

    /// 最后存款时间（ISO 8601格式）
    pub last_deposit_at: String,

    /// 存款类型分布
    pub deposit_type_distribution: Vec<DepositTypeDistributionResponse>,

    /// 用户代币分布
    pub token_distribution: Vec<UserTokenDistributionResponse>,
}

/// 用户代币分布响应
#[derive(Debug, Serialize, ToSchema)]
pub struct UserTokenDistributionResponse {
    /// 代币mint地址
    pub token_mint: String,

    /// 代币符号
    pub token_symbol: Option<String>,

    /// 代币名称
    pub token_name: Option<String>,

    /// 存款次数
    pub count: u64,

    /// 总美元交易量
    pub total_volume_usd: f64,
}

/// 代币存款汇总响应
#[derive(Debug, Serialize, ToSchema)]
pub struct TokenDepositSummaryResponse {
    /// 代币mint地址
    pub token_mint: String,

    /// 代币符号
    pub token_symbol: Option<String>,

    /// 代币名称
    pub token_name: Option<String>,

    /// 代币小数位数
    pub token_decimals: Option<u8>,

    /// 总存款数
    pub total_deposits: u64,

    /// 总美元交易量
    pub total_volume_usd: f64,

    /// 独特用户数
    pub unique_users: u32,

    /// 首次存款时间（ISO 8601格式）
    pub first_deposit_at: String,

    /// 最后存款时间（ISO 8601格式）
    pub last_deposit_at: String,

    /// 存款类型分布
    pub deposit_type_distribution: Vec<DepositTypeDistributionResponse>,
}

/// 存款趋势响应
#[derive(Debug, Serialize, ToSchema)]
pub struct DepositTrendResponse {
    /// 趋势数据点列表
    pub trends: Vec<DepositTrendPoint>,
}

/// 存款趋势数据点
#[derive(Debug, Serialize, ToSchema)]
pub struct DepositTrendPoint {
    /// 时间周期
    pub period: String,

    /// 存款数量
    pub count: u64,

    /// 美元交易量
    pub volume_usd: f64,

    /// 独特用户数
    pub unique_users: u32,
}

/// 趋势周期枚举
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "PascalCase")]
pub enum TrendPeriod {
    /// 按小时
    Hour,
    /// 按天
    Day,
    /// 按周
    Week,
    /// 按月
    Month,
}

impl<'de> Deserialize<'de> for TrendPeriod {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.to_lowercase().as_str() {
            "hour" => Ok(TrendPeriod::Hour),
            "day" => Ok(TrendPeriod::Day),
            "week" => Ok(TrendPeriod::Week),
            "month" => Ok(TrendPeriod::Month),
            _ => Err(serde::de::Error::unknown_variant(
                &s,
                &["hour", "day", "week", "month", "Hour", "Day", "Week", "Month"],
            )),
        }
    }
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

impl From<database::event_model::DepositEvent> for DepositEventResponse {
    fn from(event: database::event_model::DepositEvent) -> Self {
        Self {
            user: event.user,
            token_mint: event.token_mint,
            project_config: event.project_config,
            token_symbol: event.token_symbol,
            token_name: event.token_name,
            token_decimals: event.token_decimals,
            amount: event.amount.to_string(),
            actual_amount: event.actual_amount,
            total_raised: event.total_raised.to_string(),
            actual_total_raised: event.actual_total_raised,
            deposit_type: event.deposit_type,
            deposit_type_name: event.deposit_type_name,
            is_high_value_deposit: event.is_high_value_deposit,
            estimated_usd_value: event.estimated_usd_value,
            related_pool: event.related_pool,
            deposited_at: chrono::DateTime::from_timestamp(event.deposited_at, 0)
                .unwrap_or_default()
                .to_rfc3339(),
            signature: event.signature,
        }
    }
}

impl From<database::event_model::repository::DepositStats> for DepositStatsResponse {
    fn from(stats: database::event_model::repository::DepositStats) -> Self {
        Self {
            total_deposits: stats.total_deposits,
            today_deposits: stats.today_deposits,
            unique_users: stats.unique_users,
            unique_tokens: stats.unique_tokens,
            total_volume_usd: stats.total_volume_usd,
            today_volume_usd: stats.today_volume_usd,
            deposit_type_distribution: stats.deposit_type_distribution.into_iter().map(Into::into).collect(),
            token_distribution: stats.token_distribution.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<database::event_model::repository::DepositTypeDistribution> for DepositTypeDistributionResponse {
    fn from(dist: database::event_model::repository::DepositTypeDistribution) -> Self {
        Self {
            deposit_type: dist.deposit_type,
            name: dist.name,
            count: dist.count,
        }
    }
}

impl From<database::event_model::repository::TokenDistribution> for TokenDistributionResponse {
    fn from(dist: database::event_model::repository::TokenDistribution) -> Self {
        Self {
            token_mint: dist.token_mint,
            token_symbol: dist.token_symbol,
            token_name: dist.token_name,
            count: dist.count,
            total_volume_usd: dist.total_volume_usd,
        }
    }
}

impl From<crate::services::solana::event::deposit_service::UserDepositSummary> for UserDepositSummaryResponse {
    fn from(summary: crate::services::solana::event::deposit_service::UserDepositSummary) -> Self {
        Self {
            user: summary.user,
            total_deposits: summary.total_deposits,
            total_volume_usd: summary.total_volume_usd,
            unique_tokens: summary.unique_tokens,
            first_deposit_at: chrono::DateTime::from_timestamp(summary.first_deposit_at, 0)
                .unwrap_or_default()
                .to_rfc3339(),
            last_deposit_at: chrono::DateTime::from_timestamp(summary.last_deposit_at, 0)
                .unwrap_or_default()
                .to_rfc3339(),
            deposit_type_distribution: summary.deposit_type_distribution.into_iter().map(Into::into).collect(),
            token_distribution: summary.token_distribution.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<crate::services::solana::event::deposit_service::UserTokenDistribution> for UserTokenDistributionResponse {
    fn from(dist: crate::services::solana::event::deposit_service::UserTokenDistribution) -> Self {
        Self {
            token_mint: dist.token_mint,
            token_symbol: dist.token_symbol,
            token_name: dist.token_name,
            count: dist.count,
            total_volume_usd: dist.total_volume_usd,
        }
    }
}

impl From<crate::services::solana::event::deposit_service::TokenDepositSummary> for TokenDepositSummaryResponse {
    fn from(summary: crate::services::solana::event::deposit_service::TokenDepositSummary) -> Self {
        Self {
            token_mint: summary.token_mint,
            token_symbol: summary.token_symbol,
            token_name: summary.token_name,
            token_decimals: summary.token_decimals,
            total_deposits: summary.total_deposits,
            total_volume_usd: summary.total_volume_usd,
            unique_users: summary.unique_users,
            first_deposit_at: chrono::DateTime::from_timestamp(summary.first_deposit_at, 0)
                .unwrap_or_default()
                .to_rfc3339(),
            last_deposit_at: chrono::DateTime::from_timestamp(summary.last_deposit_at, 0)
                .unwrap_or_default()
                .to_rfc3339(),
            deposit_type_distribution: summary.deposit_type_distribution.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<crate::services::solana::event::deposit_service::DepositTrendPoint> for DepositTrendPoint {
    fn from(point: crate::services::solana::event::deposit_service::DepositTrendPoint) -> Self {
        Self {
            period: point.period,
            count: point.count,
            volume_usd: point.volume_usd,
            unique_users: point.unique_users,
        }
    }
}

impl From<crate::services::solana::event::deposit_service::TrendPeriod> for TrendPeriod {
    fn from(period: crate::services::solana::event::deposit_service::TrendPeriod) -> Self {
        match period {
            crate::services::solana::event::deposit_service::TrendPeriod::Hour => TrendPeriod::Hour,
            crate::services::solana::event::deposit_service::TrendPeriod::Day => TrendPeriod::Day,
            crate::services::solana::event::deposit_service::TrendPeriod::Week => TrendPeriod::Week,
            crate::services::solana::event::deposit_service::TrendPeriod::Month => TrendPeriod::Month,
        }
    }
}

impl From<CreateDepositEventRequest> for database::event_model::DepositEvent {
    fn from(request: CreateDepositEventRequest) -> Self {
        let now = chrono::Utc::now().timestamp();
        let deposit_type = request.deposit_type.unwrap_or(0);
        let deposit_type_name = match deposit_type {
            0 => "初始存款".to_string(),
            1 => "追加存款".to_string(),
            2 => "应急存款".to_string(),
            _ => format!("未知类型{}", deposit_type),
        };

        // 计算实际金额（考虑decimals）
        let decimals = request.token_decimals.unwrap_or(9); // 默认9位小数（SOL）
        let decimal_factor = 10_u64.pow(decimals as u32) as f64;
        let actual_amount = request.amount as f64 / decimal_factor;
        let actual_total_raised = request.total_raised as f64 / decimal_factor;

        // 计算USD价值
        let estimated_usd_value = request.estimated_usd_value.unwrap_or(0.0);
        let is_high_value_deposit = estimated_usd_value >= 1000.0; // 超过1000USD为高价值

        Self {
            id: None,
            user: request.user,
            project_config: request.project_config,
            token_mint: request.token_mint,
            amount: request.amount,
            total_raised: request.total_raised,
            token_decimals: request.token_decimals,
            token_name: request.token_name,
            token_symbol: request.token_symbol,
            token_logo_uri: request.token_logo_uri,
            deposit_type,
            deposit_type_name,
            related_pool: request.related_pool,
            is_high_value_deposit,
            estimated_usd_value,
            actual_amount,
            actual_total_raised,
            signature: request.signature,
            slot: request.slot,
            deposited_at: request.deposited_at,
            processed_at: now,
            updated_at: now,
        }
    }
}
