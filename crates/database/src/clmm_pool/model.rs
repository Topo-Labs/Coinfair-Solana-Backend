use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use utoipa::{IntoParams, ToSchema};
use validator::Validate;

/// Pool type enumeration
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
pub enum PoolType {
    /// Concentrated Liquidity Market Maker (CLMM)
    #[serde(rename = "concentrated")]
    Concentrated,
    /// Constant Product Market Maker (CPMM)  
    #[serde(rename = "standard")]
    Standard,
}

impl Default for PoolType {
    fn default() -> Self {
        PoolType::Concentrated
    }
}

impl fmt::Display for PoolType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PoolType::Concentrated => write!(f, "concentrated"),
            PoolType::Standard => write!(f, "standard"),
        }
    }
}

impl FromStr for PoolType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "concentrated" => Ok(PoolType::Concentrated),
            "standard" => Ok(PoolType::Standard),
            _ => Err(format!("Invalid pool type: {}", s)),
        }
    }
}

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

    /// 池子类型 (concentrated or standard)
    #[serde(default)]
    pub pool_type: PoolType,
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

impl TokenInfo {
    /// 判断代币信息是否为空/未补全，如果关键信息缺失则认为需要从链上获取
    pub fn is_empty(&self) -> bool {
        // 检查关键字段是否为空或者未填充
        let owner_empty = self.owner.is_empty();
        let decimals_empty = self.decimals == 0;
        let symbol_empty = self.symbol.is_none() || self.symbol.as_ref().map_or(true, |s| s.is_empty());
        let name_empty = self.name.is_none() || self.name.as_ref().map_or(true, |s| s.is_empty());
        
        // 只要关键信息（owner、decimals、symbol）有任何一个为空，就认为需要链上查询
        // mint_address 通常不应该为空，所以不检查它
        owner_empty || decimals_empty || symbol_empty || name_empty
    }
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
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
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

/// 池子列表查询请求参数
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Validate, IntoParams)]
pub struct PoolListRequest {
    /// 按池子类型过滤
    #[serde(rename = "poolType")]
    pub pool_type: Option<String>,

    /// 排序字段 (default, created_at, price, open_time)
    #[serde(rename = "poolSortField")]
    pub pool_sort_field: Option<String>,

    /// 排序方向 (asc, desc)
    #[serde(rename = "sortType")]
    pub sort_type: Option<String>,

    /// 页大小 (1-100, 默认20)
    #[serde(rename = "pageSize")]
    #[validate(range(min = 1, max = 100))]
    pub page_size: Option<u64>,

    /// 页码 (1-based, 默认1)
    #[validate(range(min = 1))]
    pub page: Option<u64>,

    /// 按创建者钱包地址过滤
    #[serde(rename = "creatorWallet")]
    pub creator_wallet: Option<String>,

    /// 按代币mint地址过滤 (兼容单一mint查询)
    #[serde(rename = "mintAddress")]
    pub mint_address: Option<String>,

    /// 按池子状态过滤
    pub status: Option<String>,

    /// 按第一个代币mint地址过滤 (用于双代币查询)
    pub mint1: Option<String>,

    /// 按第二个代币mint地址过滤 (用于双代币查询)
    pub mint2: Option<String>,
}

impl Default for PoolListRequest {
    fn default() -> Self {
        Self {
            pool_type: None,
            pool_sort_field: Some("default".to_string()),
            sort_type: Some("desc".to_string()),
            page_size: Some(20),
            page: Some(1),
            creator_wallet: None,
            mint_address: None,
            status: None,
            mint1: None,
            mint2: None,
        }
    }
}

/// 池子列表响应
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PoolListResponse {
    /// 池子列表
    pub pools: Vec<ClmmPool>,

    /// 分页元数据
    pub pagination: PaginationMeta,

    /// 过滤器摘要
    pub filters: FilterSummary,
}

/// 分页元数据
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PaginationMeta {
    /// 当前页码
    pub current_page: u64,

    /// 页大小
    pub page_size: u64,

    /// 符合条件的总记录数
    pub total_count: u64,

    /// 总页数
    pub total_pages: u64,

    /// 是否有下一页
    pub has_next: bool,

    /// 是否有上一页
    pub has_prev: bool,
}

/// 过滤器摘要
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct FilterSummary {
    /// 应用的池子类型过滤器
    pub pool_type: Option<String>,

    /// 应用的排序字段
    pub sort_field: String,

    /// 应用的排序方向
    pub sort_direction: String,

    /// 按池子类型统计数量
    pub type_counts: Vec<TypeCount>,
}

/// 池子类型统计
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TypeCount {
    /// 池子类型
    pub pool_type: String,

    /// 数量
    pub count: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn test_pool_type_default() {
        let pool_type = PoolType::default();
        assert_eq!(pool_type, PoolType::Concentrated);
    }

    #[test]
    fn test_pool_type_display() {
        assert_eq!(PoolType::Concentrated.to_string(), "concentrated");
        assert_eq!(PoolType::Standard.to_string(), "standard");
    }

    #[test]
    fn test_pool_type_from_str() {
        assert_eq!("concentrated".parse::<PoolType>().unwrap(), PoolType::Concentrated);
        assert_eq!("standard".parse::<PoolType>().unwrap(), PoolType::Standard);
        assert_eq!("CONCENTRATED".parse::<PoolType>().unwrap(), PoolType::Concentrated);
        assert_eq!("Standard".parse::<PoolType>().unwrap(), PoolType::Standard);

        assert!("invalid".parse::<PoolType>().is_err());
    }

    #[test]
    fn test_pool_type_serialization() {
        let concentrated = PoolType::Concentrated;
        let standard = PoolType::Standard;

        let concentrated_json = serde_json::to_string(&concentrated).unwrap();
        let standard_json = serde_json::to_string(&standard).unwrap();

        assert_eq!(concentrated_json, "\"concentrated\"");
        assert_eq!(standard_json, "\"standard\"");
    }

    #[test]
    fn test_pool_type_deserialization() {
        let concentrated: PoolType = serde_json::from_str("\"concentrated\"").unwrap();
        let standard: PoolType = serde_json::from_str("\"standard\"").unwrap();

        assert_eq!(concentrated, PoolType::Concentrated);
        assert_eq!(standard, PoolType::Standard);
    }

    #[test]
    fn test_clmm_pool_with_pool_type() {
        let pool = ClmmPool {
            id: None,
            pool_address: "11111111111111111111111111111111".to_string(),
            amm_config_address: "22222222222222222222222222222222".to_string(),
            config_index: 0,
            mint0: TokenInfo {
                mint_address: "33333333333333333333333333333333".to_string(),
                decimals: 6,
                owner: "44444444444444444444444444444444".to_string(),
                symbol: Some("TOKEN0".to_string()),
                name: Some("Token 0".to_string()),
            },
            mint1: TokenInfo {
                mint_address: "55555555555555555555555555555555".to_string(),
                decimals: 9,
                owner: "66666666666666666666666666666666".to_string(),
                symbol: Some("TOKEN1".to_string()),
                name: Some("Token 1".to_string()),
            },
            price_info: PriceInfo {
                initial_price: 1.0,
                sqrt_price_x64: "18446744073709551616".to_string(),
                initial_tick: 0,
                current_price: None,
                current_tick: None,
            },
            vault_info: VaultInfo {
                token_vault_0: "77777777777777777777777777777777".to_string(),
                token_vault_1: "88888888888888888888888888888888".to_string(),
            },
            extension_info: ExtensionInfo {
                observation_address: "99999999999999999999999999999999".to_string(),
                tickarray_bitmap_extension: "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".to_string(),
            },
            creator_wallet: "BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB".to_string(),
            open_time: 1640995200,
            created_at: 1640995200,
            updated_at: 1640995200,
            transaction_info: None,
            status: PoolStatus::Created,
            sync_status: SyncStatus {
                last_sync_at: 1640995200,
                sync_version: 1,
                needs_sync: false,
                sync_error: None,
            },
            pool_type: PoolType::Concentrated,
        };

        assert_eq!(pool.pool_type, PoolType::Concentrated);
    }

    #[test]
    fn test_token_info_is_empty() {
        // 测试完全空的TokenInfo
        let empty_token = TokenInfo {
            mint_address: "So11111111111111111111111111111111111111112".to_string(),
            decimals: 0,
            owner: "".to_string(),
            symbol: None,
            name: None,
        };
        assert!(empty_token.is_empty());

        // 测试你描述的情况：空字符串
        let empty_string_token = TokenInfo {
            mint_address: "So11111111111111111111111111111111111111112".to_string(),
            decimals: 0,
            owner: "".to_string(),
            symbol: Some("".to_string()),
            name: Some("".to_string()),
        };
        assert!(empty_string_token.is_empty());

        // 测试部分填充的TokenInfo (应该被认为是不完整的)
        let partial_token = TokenInfo {
            mint_address: "So11111111111111111111111111111111111111112".to_string(),
            decimals: 9,
            owner: "".to_string(), // owner为空
            symbol: Some("WSOL".to_string()),
            name: Some("Wrapped SOL".to_string()),
        };
        assert!(partial_token.is_empty()); // owner为空，所以仍然是empty

        // 测试完整填充的TokenInfo
        let complete_token = TokenInfo {
            mint_address: "So11111111111111111111111111111111111111112".to_string(),
            decimals: 9,
            owner: "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA".to_string(),
            symbol: Some("WSOL".to_string()),
            name: Some("Wrapped SOL".to_string()),
        };
        assert!(!complete_token.is_empty());

        // 测试只有symbol为空的情况
        let no_symbol_token = TokenInfo {
            mint_address: "So11111111111111111111111111111111111111112".to_string(),
            decimals: 9,
            owner: "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA".to_string(),
            symbol: None,
            name: Some("Wrapped SOL".to_string()),
        };
        assert!(no_symbol_token.is_empty()); // symbol为空，所以是empty
    }
}
