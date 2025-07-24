use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::Validate;

/// CLMM池子元数据模型
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct ClmmPool {
    /// MongoDB文档ID
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    
    /// 池子地址 (主键)
    #[validate(length(min = 32, max = 44))]
    pub pool_address: String,
    
    /// AMM配置地址
    #[validate(length(min = 32, max = 44))]
    pub amm_config_address: String,
    
    /// 配置索引
    pub config_index: u16,
    
    /// 代币0信息
    pub mint0: TokenInfo,
    
    /// 代币1信息
    pub mint1: TokenInfo,
    
    /// 价格信息
    pub price_info: PriceInfo,
    
    /// 金库地址信息
    pub vault_info: VaultInfo,
    
    /// 扩展地址信息
    pub extension_info: ExtensionInfo,
    
    /// 创建者钱包地址
    #[validate(length(min = 32, max = 44))]
    pub creator_wallet: String,
    
    /// 池子开放时间
    #[serde(with = "mongodb::bson::serde_helpers::u64_as_f64")]
    pub open_time: u64,
    
    /// 创建时间戳
    #[serde(with = "mongodb::bson::serde_helpers::u64_as_f64")]
    pub created_at: u64,
    
    /// 更新时间戳
    #[serde(with = "mongodb::bson::serde_helpers::u64_as_f64")]
    pub updated_at: u64,
    
    /// 交易信息 (可选，仅在实际发送交易时填充)
    pub transaction_info: Option<TransactionInfo>,
    
    /// 池子状态
    pub status: PoolStatus,
    
    /// 同步状态
    pub sync_status: SyncStatus,
}

/// 代币信息
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TokenInfo {
    /// 代币mint地址
    pub mint_address: String,
    /// 代币精度
    pub decimals: u8,
    /// 代币所有者
    pub owner: String,
    /// 代币符号 (可选)
    pub symbol: Option<String>,
    /// 代币名称 (可选)
    pub name: Option<String>,
}

/// 价格信息
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PriceInfo {
    /// 初始价格
    pub initial_price: f64,
    /// sqrt_price_x64
    pub sqrt_price_x64: String,
    /// 初始tick
    pub initial_tick: i32,
    /// 当前价格 (可选，用于实时更新)
    pub current_price: Option<f64>,
    /// 当前tick (可选，用于实时更新)
    pub current_tick: Option<i32>,
}

/// 金库地址信息
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct VaultInfo {
    /// 代币0金库地址
    pub token_vault_0: String,
    /// 代币1金库地址
    pub token_vault_1: String,
}

/// 扩展地址信息
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ExtensionInfo {
    /// 观察地址
    pub observation_address: String,
    /// Tick数组位图扩展地址
    pub tickarray_bitmap_extension: String,
}

/// 交易信息
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TransactionInfo {
    /// 交易签名
    pub signature: String,
    /// 交易状态
    pub status: TransactionStatus,
    /// 区块链浏览器链接
    pub explorer_url: String,
    /// 交易确认时间
    #[serde(with = "mongodb::bson::serde_helpers::u64_as_f64")]
    pub confirmed_at: u64,
}

/// 交易状态
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub enum TransactionStatus {
    /// 已提交
    Submitted,
    /// 已确认
    Confirmed,
    /// 已完成
    Finalized,
    /// 失败
    Failed,
}

/// 池子状态
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub enum PoolStatus {
    /// 已创建 (仅构建交易，未发送)
    Created,
    /// 待确认 (交易已发送，等待确认)
    Pending,
    /// 活跃 (交易已确认，池子正常运行)
    Active,
    /// 暂停
    Paused,
    /// 已关闭
    Closed,
}

/// 同步状态
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SyncStatus {
    /// 最后同步时间
    #[serde(with = "mongodb::bson::serde_helpers::u64_as_f64")]
    pub last_sync_at: u64,
    /// 同步版本号
    pub sync_version: u64,
    /// 是否需要同步
    pub needs_sync: bool,
    /// 同步错误信息 (可选)
    pub sync_error: Option<String>,
}

/// 池子查询参数
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Validate)]
pub struct PoolQueryParams {
    /// 按池子地址查询
    pub pool_address: Option<String>,
    /// 按代币mint地址查询
    pub mint_address: Option<String>,
    /// 按创建者查询
    pub creator_wallet: Option<String>,
    /// 按状态查询
    pub status: Option<PoolStatus>,
    /// 价格范围查询 - 最小值
    pub min_price: Option<f64>,
    /// 价格范围查询 - 最大值
    pub max_price: Option<f64>,
    /// 时间范围查询 - 开始时间
    pub start_time: Option<u64>,
    /// 时间范围查询 - 结束时间
    pub end_time: Option<u64>,
    /// 分页 - 页码
    pub page: Option<u64>,
    /// 分页 - 每页数量
    pub limit: Option<u64>,
    /// 排序字段
    pub sort_by: Option<String>,
    /// 排序方向 (asc/desc)
    pub sort_order: Option<String>,
}

/// 池子统计信息
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PoolStats {
    /// 总池子数量
    pub total_pools: u64,
    /// 活跃池子数量
    pub active_pools: u64,
    /// 今日新增池子数量
    pub today_new_pools: u64,
    /// 按状态分组统计
    pub status_stats: Vec<StatusStat>,
    /// 按代币分组统计 (Top 10)
    pub token_stats: Vec<TokenStat>,
}

/// 状态统计
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct StatusStat {
    pub status: PoolStatus,
    pub count: u64,
}

/// 代币统计
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TokenStat {
    pub mint_address: String,
    pub symbol: Option<String>,
    pub pool_count: u64,
}