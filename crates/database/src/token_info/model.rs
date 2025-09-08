use chrono::{DateTime, Utc};
use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use validator::Validate;

/// 静态DTO结构体，用于与现有API兼容
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaticTokenInfo {
    pub address: String,
    pub program_id: String,
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
    pub logo_uri: String,
    pub tags: Vec<String>,
    pub daily_volume: f64,
    pub created_at: DateTime<Utc>,
    pub freeze_authority: Option<String>,
    pub mint_authority: Option<String>,
    pub permanent_delegate: Option<String>,
    pub minted_at: Option<DateTime<Utc>>,
    pub extensions: serde_json::Value,
}

/// 代币信息数据库模型
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct TokenInfo {
    /// MongoDB文档ID
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,

    /// 代币地址 (主键，唯一索引)
    #[validate(length(min = 32, max = 44))]
    pub address: String,

    /// 程序ID
    #[validate(length(min = 32, max = 44))]
    pub program_id: String,

    /// 代币名称
    #[validate(length(min = 1, max = 100))]
    pub name: String,

    /// 代币符号
    #[validate(length(min = 1, max = 20))]
    pub symbol: String,

    /// 小数位数 (0-18)
    #[validate(range(min = 0, max = 18))]
    pub decimals: u8,

    /// Logo URI
    #[validate(url)]
    pub logo_uri: String,

    /// 标签列表
    pub tags: Vec<String>,

    /// 日交易量 (24小时)
    #[validate(range(min = 0.0))]
    pub daily_volume: f64,

    /// 代币创建时间
    pub created_at: DateTime<Utc>,

    /// 冻结权限地址 (可选)
    pub freeze_authority: Option<String>,

    /// 铸造权限地址 (可选)
    pub mint_authority: Option<String>,

    /// 永久委托地址 (可选)
    pub permanent_delegate: Option<String>,

    /// 铸造时间 (可选)
    pub minted_at: Option<DateTime<Utc>>,

    /// 扩展信息 (JSON格式)
    pub extensions: serde_json::Value,

    /// 数据推送时间
    pub push_time: DateTime<Utc>,

    /// 最后更新时间
    pub updated_at: DateTime<Utc>,

    /// 数据状态
    pub status: TokenStatus,

    /// 数据来源
    pub source: DataSource,

    /// 验证状态
    pub verification: VerificationStatus,
}

/// 代币状态枚举
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
pub enum TokenStatus {
    /// 活跃 - 正常使用
    #[serde(rename = "active")]
    Active,
    /// 已暂停 - 暂时不可用
    #[serde(rename = "paused")]
    Paused,
    /// 已弃用 - 不再使用
    #[serde(rename = "deprecated")]
    Deprecated,
    /// 黑名单 - 禁止使用
    #[serde(rename = "blacklisted")]
    Blacklisted,
}

impl Default for TokenStatus {
    fn default() -> Self {
        TokenStatus::Active
    }
}

/// 数据来源枚举
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
pub enum DataSource {
    /// 外部推送 - 来自meme币发射平台
    #[serde(rename = "external_push")]
    ExternalPush,
    /// 链上同步 - 从区块链同步
    #[serde(rename = "onchain_sync")]
    OnchainSync,
    /// 手动添加 - 管理员手动添加
    #[serde(rename = "manual")]
    Manual,
    /// 系统导入 - 批量导入
    #[serde(rename = "system_import")]
    SystemImport,
}

impl Default for DataSource {
    fn default() -> Self {
        DataSource::ExternalPush
    }
}

/// 验证状态枚举
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
pub enum VerificationStatus {
    /// 未验证
    #[serde(rename = "unverified")]
    Unverified,
    /// 已验证
    #[serde(rename = "verified")]
    Verified,
    /// 社区验证
    #[serde(rename = "community")]
    Community,
    /// 严格验证
    #[serde(rename = "strict")]
    Strict,
}

impl Default for VerificationStatus {
    fn default() -> Self {
        VerificationStatus::Unverified
    }
}

/// 代币信息推送请求
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Validate)]
pub struct TokenPushRequest {
    /// 代币地址
    #[validate(length(min = 32, max = 44))]
    pub address: String,

    /// 程序ID (可选，默认为Token Program)
    pub program_id: Option<String>,

    /// 代币名称
    #[validate(length(min = 1, max = 100))]
    pub name: String,

    /// 代币符号
    #[validate(length(min = 1, max = 20))]
    pub symbol: String,

    /// 小数位数
    #[validate(range(min = 0, max = 18))]
    pub decimals: u8,

    /// Logo URI
    #[validate(url)]
    pub logo_uri: String,

    /// 标签列表 (可选)
    pub tags: Option<Vec<String>>,

    /// 日交易量 (可选，默认为0)
    pub daily_volume: Option<f64>,

    /// 冻结权限地址 (可选)
    pub freeze_authority: Option<String>,

    /// 铸造权限地址 (可选)
    pub mint_authority: Option<String>,

    /// 永久委托地址 (可选)
    pub permanent_delegate: Option<String>,

    /// 铸造时间 (可选)
    pub minted_at: Option<DateTime<Utc>>,

    /// 扩展信息 (可选)
    pub extensions: Option<serde_json::Value>,

    /// 数据来源 (可选，默认为external_push)
    pub source: Option<DataSource>,
}

/// 代币信息推送响应
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Default)]
pub struct TokenPushResponse {
    /// 操作结果
    pub success: bool,

    /// 代币地址
    pub address: String,

    /// 操作类型 (created/updated)
    pub operation: String,

    /// 响应消息
    pub message: String,

    /// 推送时间戳
    pub timestamp: DateTime<Utc>,
}

/// 代币列表查询参数
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Validate, IntoParams)]
pub struct TokenListQuery {
    /// 页码 (1-based, 默认1)
    #[validate(range(min = 1))]
    pub page: Option<u64>,

    /// 每页数量 (1-1000, 默认100)
    #[serde(rename = "pageSize")]
    #[validate(range(min = 1, max = 1000))]
    pub page_size: Option<u64>,

    /// 按标签过滤 (可多选，用逗号分隔)
    pub tags: Option<String>,

    /// 按状态过滤
    pub status: Option<TokenStatus>,

    /// 按数据来源过滤
    pub source: Option<DataSource>,

    /// 按验证状态过滤
    pub verification: Option<VerificationStatus>,

    /// 最小日交易量过滤
    #[serde(rename = "minVolume")]
    pub min_volume: Option<f64>,

    /// 最大日交易量过滤
    #[serde(rename = "maxVolume")]
    pub max_volume: Option<f64>,

    /// 搜索关键词 (匹配名称、符号、地址)
    pub search: Option<String>,

    /// 按项目状态过滤 (从extensions.project_state字段过滤)
    #[serde(rename = "projectState")]
    pub project_state: Option<u8>,

    /// 按创建者过滤 (从extensions.creator字段过滤)
    pub creator: Option<String>,

    /// 按地址过滤 (可多选，用逗号分隔)
    pub addresses: Option<String>,

    /// 按参与者过滤 (钱包地址，查询该地址参与过的代币众筹活动)
    pub participate: Option<String>,

    /// 排序字段 (created_at, daily_volume, name, symbol, updated_at, push_time)
    /// 支持多字段排序，用逗号分隔，如: "daily_volume,created_at"
    #[serde(rename = "sortBy")]
    pub sort_by: Option<String>,

    /// 排序方向 (asc, desc)
    /// 支持多方向排序，用逗号分隔，如: "desc,asc"
    /// 如果字段数量多于方向数量，多余字段使用默认方向(desc)
    #[serde(rename = "sortOrder")]
    pub sort_order: Option<String>,
}

impl Default for TokenListQuery {
    fn default() -> Self {
        Self {
            page: Some(1),
            page_size: Some(100),
            tags: None,
            status: None,
            source: None,
            verification: None,
            min_volume: None,
            max_volume: None,
            search: None,
            project_state: None,
            creator: None,
            addresses: None,
            participate: None,
            sort_by: Some("created_at".to_string()),
            sort_order: Some("desc".to_string()),
        }
    }
}

impl TokenListQuery {
    /// 获取有效的排序字段列表
    pub const VALID_SORT_FIELDS: &'static [&'static str] = &[
        "created_at",
        "daily_volume",
        "name",
        "symbol",
        "updated_at",
        "push_time",
        "address",
        "decimals",
        "extensions.total_raised",
    ];

    /// 验证排序字段是否有效
    pub fn validate_sort_field(field: &str) -> bool {
        Self::VALID_SORT_FIELDS.contains(&field)
    }

    /// 解析排序参数为字段和方向的配对
    /// 返回 (字段名, 排序方向) 的向量，排序方向为 1(升序) 或 -1(降序)
    pub fn parse_sort_params(&self) -> Vec<(String, i32)> {
        let mut sort_fields = match &self.sort_by {
            Some(fields) => fields
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty() && Self::validate_sort_field(s))
                .collect::<Vec<_>>(),
            None => vec![],
        };

        // 如果没有有效的排序字段，使用默认排序
        if sort_fields.is_empty() {
            sort_fields.push("created_at".to_string());
        }

        let sort_orders = match &self.sort_order {
            Some(orders) => orders
                .split(',')
                .map(|s| s.trim())
                .map(|s| if s.eq_ignore_ascii_case("asc") { 1 } else { -1 })
                .collect::<Vec<_>>(),
            None => vec![-1], // 默认降序
        };

        // 将字段和方向配对，如果方向不够则使用默认值(-1降序)
        sort_fields
            .into_iter()
            .enumerate()
            .map(|(i, field)| {
                let order = sort_orders.get(i).copied().unwrap_or(-1);
                (field, order)
            })
            .collect()
    }
}

/// 代币列表响应
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TokenListResponse {
    /// 代币列表
    #[serde(rename = "mintList")]
    pub mint_list: Vec<StaticTokenInfo>,

    /// 黑名单 (被标记为blacklisted的代币地址)
    pub blacklist: Vec<String>,

    /// 白名单 (被标记为verified或更高级别的代币地址)
    #[serde(rename = "whiteList")]
    pub white_list: Vec<String>,

    /// 分页信息
    pub pagination: PaginationInfo,

    /// 过滤器统计信息
    pub stats: FilterStats,
}

/// 分页信息
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PaginationInfo {
    /// 当前页码
    #[serde(rename = "currentPage")]
    pub current_page: u64,

    /// 每页数量
    #[serde(rename = "pageSize")]
    pub page_size: u64,

    /// 总记录数
    #[serde(rename = "totalCount")]
    pub total_count: u64,

    /// 总页数
    #[serde(rename = "totalPages")]
    pub total_pages: u64,

    /// 是否有下一页
    #[serde(rename = "hasNext")]
    pub has_next: bool,

    /// 是否有上一页
    #[serde(rename = "hasPrev")]
    pub has_prev: bool,
}

/// 过滤器统计信息
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct FilterStats {
    /// 按状态统计
    #[serde(rename = "statusCounts")]
    pub status_counts: Vec<StatusCount>,

    /// 按数据来源统计
    #[serde(rename = "sourceCounts")]
    pub source_counts: Vec<SourceCount>,

    /// 按验证状态统计
    #[serde(rename = "verificationCounts")]
    pub verification_counts: Vec<VerificationCount>,

    /// 常见标签统计 (Top 10)
    #[serde(rename = "tagCounts")]
    pub tag_counts: Vec<TagCount>,
}

/// 状态统计
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct StatusCount {
    pub status: TokenStatus,
    pub count: u64,
}

/// 数据来源统计
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SourceCount {
    pub source: DataSource,
    pub count: u64,
}

/// 验证状态统计
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct VerificationCount {
    pub verification: VerificationStatus,
    pub count: u64,
}

/// 标签统计
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TagCount {
    pub tag: String,
    pub count: u64,
}

impl TokenInfo {
    /// 创建新的代币信息实例
    pub fn new(
        address: String,
        program_id: String,
        name: String,
        symbol: String,
        decimals: u8,
        logo_uri: String,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: None,
            address,
            program_id,
            name,
            symbol,
            decimals,
            logo_uri,
            tags: Vec::new(),
            daily_volume: 0.0,
            created_at: now,
            freeze_authority: None,
            mint_authority: None,
            permanent_delegate: None,
            minted_at: None,
            extensions: serde_json::Value::Null,
            push_time: now,
            updated_at: now,
            status: TokenStatus::default(),
            source: DataSource::default(),
            verification: VerificationStatus::default(),
        }
    }

    /// 从推送请求创建代币信息
    pub fn from_push_request(request: TokenPushRequest) -> Self {
        let now = Utc::now();
        Self {
            id: None,
            address: request.address,
            program_id: request
                .program_id
                .unwrap_or_else(|| "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA".to_string()),
            name: request.name,
            symbol: request.symbol,
            decimals: request.decimals,
            logo_uri: request.logo_uri,
            tags: request.tags.unwrap_or_default(),
            daily_volume: request.daily_volume.unwrap_or(0.0),
            created_at: now,
            freeze_authority: request.freeze_authority,
            mint_authority: request.mint_authority,
            permanent_delegate: request.permanent_delegate,
            minted_at: request.minted_at,
            extensions: request.extensions.unwrap_or_else(|| serde_json::json!({})),
            push_time: now,
            updated_at: now,
            status: TokenStatus::default(),
            source: request.source.unwrap_or_default(),
            verification: VerificationStatus::default(),
        }
    }

    /// 更新代币信息
    pub fn update_from_push_request(&mut self, request: TokenPushRequest) {
        let now = Utc::now();

        // 更新基本信息
        self.name = request.name;
        self.symbol = request.symbol;
        self.decimals = request.decimals;
        self.logo_uri = request.logo_uri;

        // 更新可选字段
        if let Some(tags) = request.tags {
            self.tags = tags;
        }
        if let Some(daily_volume) = request.daily_volume {
            self.daily_volume = daily_volume;
        }
        if let Some(freeze_authority) = request.freeze_authority {
            self.freeze_authority = Some(freeze_authority);
        }
        if let Some(mint_authority) = request.mint_authority {
            self.mint_authority = Some(mint_authority);
        }
        if let Some(permanent_delegate) = request.permanent_delegate {
            self.permanent_delegate = Some(permanent_delegate);
        }
        if let Some(minted_at) = request.minted_at {
            self.minted_at = Some(minted_at);
        }
        if let Some(extensions) = request.extensions {
            self.extensions = extensions;
        }
        if let Some(source) = request.source {
            self.source = source;
        }

        // 更新时间戳
        self.push_time = now;
        self.updated_at = now;
    }

    /// 转换为静态DTO格式 (与现有API兼容)
    pub fn to_static_dto(&self) -> StaticTokenInfo {
        StaticTokenInfo {
            address: self.address.clone(),
            program_id: self.program_id.clone(),
            name: self.name.clone(),
            symbol: self.symbol.clone(),
            decimals: self.decimals,
            logo_uri: self.logo_uri.clone(),
            tags: self.tags.clone(),
            daily_volume: self.daily_volume,
            created_at: self.created_at,
            freeze_authority: self.freeze_authority.clone(),
            mint_authority: self.mint_authority.clone(),
            permanent_delegate: self.permanent_delegate.clone(),
            minted_at: self.minted_at,
            extensions: self.extensions.clone(),
        }
    }

    /// 判断是否在黑名单中
    pub fn is_blacklisted(&self) -> bool {
        self.status == TokenStatus::Blacklisted
    }

    /// 判断是否在白名单中
    pub fn is_whitelisted(&self) -> bool {
        match self.verification {
            VerificationStatus::Verified | VerificationStatus::Community | VerificationStatus::Strict => true,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn test_token_status_serialization() {
        assert_eq!(serde_json::to_string(&TokenStatus::Active).unwrap(), "\"active\"");
        assert_eq!(serde_json::to_string(&TokenStatus::Paused).unwrap(), "\"paused\"");
        assert_eq!(
            serde_json::to_string(&TokenStatus::Deprecated).unwrap(),
            "\"deprecated\""
        );
        assert_eq!(
            serde_json::to_string(&TokenStatus::Blacklisted).unwrap(),
            "\"blacklisted\""
        );
    }

    #[test]
    fn test_data_source_serialization() {
        assert_eq!(
            serde_json::to_string(&DataSource::ExternalPush).unwrap(),
            "\"external_push\""
        );
        assert_eq!(
            serde_json::to_string(&DataSource::OnchainSync).unwrap(),
            "\"onchain_sync\""
        );
        assert_eq!(serde_json::to_string(&DataSource::Manual).unwrap(), "\"manual\"");
        assert_eq!(
            serde_json::to_string(&DataSource::SystemImport).unwrap(),
            "\"system_import\""
        );
    }

    #[test]
    fn test_verification_status_serialization() {
        assert_eq!(
            serde_json::to_string(&VerificationStatus::Unverified).unwrap(),
            "\"unverified\""
        );
        assert_eq!(
            serde_json::to_string(&VerificationStatus::Verified).unwrap(),
            "\"verified\""
        );
        assert_eq!(
            serde_json::to_string(&VerificationStatus::Community).unwrap(),
            "\"community\""
        );
        assert_eq!(
            serde_json::to_string(&VerificationStatus::Strict).unwrap(),
            "\"strict\""
        );
    }

    #[test]
    fn test_token_info_new() {
        let token = TokenInfo::new(
            "So11111111111111111111111111111111111111112".to_string(),
            "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA".to_string(),
            "Wrapped SOL".to_string(),
            "WSOL".to_string(),
            9,
            "https://example.com/wsol.png".to_string(),
        );

        assert_eq!(token.address, "So11111111111111111111111111111111111111112");
        assert_eq!(token.symbol, "WSOL");
        assert_eq!(token.decimals, 9);
        assert_eq!(token.status, TokenStatus::Active);
        assert_eq!(token.source, DataSource::ExternalPush);
        assert_eq!(token.verification, VerificationStatus::Unverified);
    }

    #[test]
    fn test_token_info_is_blacklisted() {
        let mut token = TokenInfo::new(
            "test_address".to_string(),
            "test_program".to_string(),
            "Test Token".to_string(),
            "TEST".to_string(),
            6,
            "https://example.com/test.png".to_string(),
        );

        assert!(!token.is_blacklisted());

        token.status = TokenStatus::Blacklisted;
        assert!(token.is_blacklisted());
    }

    #[test]
    fn test_token_info_is_whitelisted() {
        let mut token = TokenInfo::new(
            "test_address".to_string(),
            "test_program".to_string(),
            "Test Token".to_string(),
            "TEST".to_string(),
            6,
            "https://example.com/test.png".to_string(),
        );

        assert!(!token.is_whitelisted());

        token.verification = VerificationStatus::Verified;
        assert!(token.is_whitelisted());

        token.verification = VerificationStatus::Community;
        assert!(token.is_whitelisted());

        token.verification = VerificationStatus::Strict;
        assert!(token.is_whitelisted());
    }

    #[test]
    fn test_token_list_query_validate_sort_field() {
        // 测试有效字段
        assert!(TokenListQuery::validate_sort_field("created_at"));
        assert!(TokenListQuery::validate_sort_field("daily_volume"));
        assert!(TokenListQuery::validate_sort_field("name"));
        assert!(TokenListQuery::validate_sort_field("symbol"));
        assert!(TokenListQuery::validate_sort_field("updated_at"));
        assert!(TokenListQuery::validate_sort_field("push_time"));
        assert!(TokenListQuery::validate_sort_field("address"));
        assert!(TokenListQuery::validate_sort_field("decimals"));

        // 测试无效字段
        assert!(!TokenListQuery::validate_sort_field("invalid_field"));
        assert!(!TokenListQuery::validate_sort_field(""));
    }

    #[test]
    fn test_token_list_query_parse_sort_params_single_field() {
        // 单字段排序 - 默认参数
        let query = TokenListQuery::default();
        let params = query.parse_sort_params();
        assert_eq!(params, vec![("created_at".to_string(), -1)]);

        // 单字段排序 - 自定义字段和方向
        let query = TokenListQuery {
            sort_by: Some("daily_volume".to_string()),
            sort_order: Some("asc".to_string()),
            ..Default::default()
        };
        let params = query.parse_sort_params();
        assert_eq!(params, vec![("daily_volume".to_string(), 1)]);

        // 单字段排序 - 降序
        let query = TokenListQuery {
            sort_by: Some("name".to_string()),
            sort_order: Some("desc".to_string()),
            ..Default::default()
        };
        let params = query.parse_sort_params();
        assert_eq!(params, vec![("name".to_string(), -1)]);
    }

    #[test]
    fn test_token_list_query_parse_sort_params_multi_field() {
        // 多字段排序 - 字段和方向数量相等
        let query = TokenListQuery {
            sort_by: Some("daily_volume,created_at,name".to_string()),
            sort_order: Some("desc,asc,desc".to_string()),
            ..Default::default()
        };
        let params = query.parse_sort_params();
        assert_eq!(
            params,
            vec![
                ("daily_volume".to_string(), -1),
                ("created_at".to_string(), 1),
                ("name".to_string(), -1)
            ]
        );

        // 多字段排序 - 方向数量少于字段数量，使用默认方向
        let query = TokenListQuery {
            sort_by: Some("daily_volume,created_at,name".to_string()),
            sort_order: Some("desc,asc".to_string()),
            ..Default::default()
        };
        let params = query.parse_sort_params();
        assert_eq!(
            params,
            vec![
                ("daily_volume".to_string(), -1),
                ("created_at".to_string(), 1),
                ("name".to_string(), -1) // 使用默认方向
            ]
        );

        // 多字段排序 - 只有字段没有方向，全部使用默认方向
        let query = TokenListQuery {
            sort_by: Some("daily_volume,created_at".to_string()),
            sort_order: None,
            ..Default::default()
        };
        let params = query.parse_sort_params();
        assert_eq!(
            params,
            vec![("daily_volume".to_string(), -1), ("created_at".to_string(), -1)]
        );
    }

    #[test]
    fn test_token_list_query_parse_sort_params_invalid_fields() {
        // 包含无效字段，应该被过滤掉
        let query = TokenListQuery {
            sort_by: Some("daily_volume,invalid_field,created_at".to_string()),
            sort_order: Some("desc,asc,desc".to_string()),
            ..Default::default()
        };
        let params = query.parse_sort_params();
        assert_eq!(
            params,
            vec![("daily_volume".to_string(), -1), ("created_at".to_string(), 1)]
        );

        // 全部是无效字段，应该返回默认排序
        let query = TokenListQuery {
            sort_by: Some("invalid1,invalid2".to_string()),
            sort_order: Some("desc,asc".to_string()),
            ..Default::default()
        };
        let params = query.parse_sort_params();
        assert_eq!(params, vec![("created_at".to_string(), -1)]);
    }

    #[test]
    fn test_token_list_query_parse_sort_params_whitespace_handling() {
        // 测试空格处理
        let query = TokenListQuery {
            sort_by: Some(" daily_volume , created_at , name ".to_string()),
            sort_order: Some(" desc , asc , desc ".to_string()),
            ..Default::default()
        };
        let params = query.parse_sort_params();
        assert_eq!(
            params,
            vec![
                ("daily_volume".to_string(), -1),
                ("created_at".to_string(), 1),
                ("name".to_string(), -1)
            ]
        );

        // 测试大小写不敏感的方向处理
        let query = TokenListQuery {
            sort_by: Some("daily_volume,created_at".to_string()),
            sort_order: Some("DESC,ASC".to_string()),
            ..Default::default()
        };
        let params = query.parse_sort_params();
        assert_eq!(
            params,
            vec![("daily_volume".to_string(), -1), ("created_at".to_string(), 1)]
        );
    }

    #[test]
    fn test_token_list_query_project_state_default() {
        // 测试默认查询不包含project_state过滤
        let query = TokenListQuery::default();
        assert!(query.project_state.is_none());
    }

    #[test]
    fn test_token_list_query_project_state_filtering() {
        // 测试project_state过滤功能
        let query = TokenListQuery {
            project_state: Some(4),
            ..Default::default()
        };
        assert_eq!(query.project_state, Some(4));

        // 测试空格处理
        let query = TokenListQuery {
            project_state: Some(3),
            ..Default::default()
        };
        assert_eq!(query.project_state, Some(3));
    }

    #[test]
    fn test_token_info_extensions_structure() {
        // 测试扩展信息的结构
        let mut token = TokenInfo::new(
            "test_address".to_string(),
            "test_program".to_string(),
            "Test Token".to_string(),
            "TEST".to_string(),
            6,
            "https://example.com/test.png".to_string(),
        );

        // 测试设置extensions.project_state
        let extensions = serde_json::json!({
            "project_state": "launched",
            "total_raised": 1000000.0,
            "launch_date": "2024-01-01T00:00:00Z"
        });

        token.extensions = extensions;

        // 验证可以访问project_state
        if let serde_json::Value::Object(obj) = &token.extensions {
            assert_eq!(
                obj.get("project_state"),
                Some(&serde_json::Value::String("launched".to_string()))
            );
        } else {
            panic!("Extensions should be an object");
        }
    }

    #[test]
    fn test_token_list_query_creator_filtering() {
        // 测试creator过滤功能
        let query = TokenListQuery {
            creator: Some("8S2bcP66WehuF6cHryfZ7vfFpQWaUhYyAYSy5U3gX4Fy".to_string()),
            ..Default::default()
        };
        assert_eq!(
            query.creator,
            Some("8S2bcP66WehuF6cHryfZ7vfFpQWaUhYyAYSy5U3gX4Fy".to_string())
        );

        // 测试空creator过滤
        let query = TokenListQuery {
            creator: Some("".to_string()),
            ..Default::default()
        };
        assert_eq!(query.creator, Some("".to_string()));

        // 测试None creator过滤
        let query = TokenListQuery {
            creator: None,
            ..Default::default()
        };
        assert_eq!(query.creator, None);
    }

    #[test]
    fn test_token_list_query_creator_default() {
        // 测试默认查询不包含creator过滤
        let query = TokenListQuery::default();
        assert!(query.creator.is_none());
    }

    #[test]
    fn test_token_info_extensions_creator_structure() {
        // 测试扩展信息的creator结构
        let mut token = TokenInfo::new(
            "test_address".to_string(),
            "test_program".to_string(),
            "Test Token".to_string(),
            "TEST".to_string(),
            6,
            "https://example.com/test.png".to_string(),
        );

        // 测试设置extensions.creator
        let extensions = serde_json::json!({
            "creator": "8S2bcP66WehuF6cHryfZ7vfFpQWaUhYyAYSy5U3gX4Fy",
            "project_state": "launched",
            "total_raised": 1000000.0
        });

        token.extensions = extensions;

        // 验证可以访问creator
        if let serde_json::Value::Object(obj) = &token.extensions {
            assert_eq!(
                obj.get("creator"),
                Some(&serde_json::Value::String(
                    "8S2bcP66WehuF6cHryfZ7vfFpQWaUhYyAYSy5U3gX4Fy".to_string()
                ))
            );
            assert_eq!(
                obj.get("project_state"),
                Some(&serde_json::Value::String("launched".to_string()))
            );
        } else {
            panic!("Extensions should be an object");
        }
    }

    #[test]
    fn test_token_list_query_addresses_filtering() {
        // 测试单个地址过滤功能
        let query = TokenListQuery {
            addresses: Some("So11111111111111111111111111111111111111112".to_string()),
            ..Default::default()
        };
        assert_eq!(
            query.addresses,
            Some("So11111111111111111111111111111111111111112".to_string())
        );

        // 测试多个地址过滤功能
        let query = TokenListQuery {
            addresses: Some(
                "So11111111111111111111111111111111111111112,EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(),
            ),
            ..Default::default()
        };
        assert_eq!(
            query.addresses,
            Some(
                "So11111111111111111111111111111111111111112,EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string()
            )
        );

        // 测试空地址过滤
        let query = TokenListQuery {
            addresses: Some("".to_string()),
            ..Default::default()
        };
        assert_eq!(query.addresses, Some("".to_string()));

        // 测试None地址过滤
        let query = TokenListQuery {
            addresses: None,
            ..Default::default()
        };
        assert_eq!(query.addresses, None);
    }

    #[test]
    fn test_token_list_query_addresses_default() {
        // 测试默认查询不包含addresses过滤
        let query = TokenListQuery::default();
        assert!(query.addresses.is_none());
    }

    #[test]
    fn test_token_list_query_addresses_validation() {
        // 测试地址格式验证逻辑（这个测试主要验证我们的过滤参数设计）
        let valid_addresses = vec![
            "So11111111111111111111111111111111111111112",  // 44字符
            "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v", // 44字符
        ];

        let query = TokenListQuery {
            addresses: Some(valid_addresses.join(",")),
            ..Default::default()
        };

        // 验证地址字符串被正确存储
        let addresses_str = query.addresses.unwrap();
        let parsed_addresses: Vec<&str> = addresses_str.split(',').collect();
        assert_eq!(parsed_addresses.len(), 2);
        assert!(parsed_addresses.contains(&"So11111111111111111111111111111111111111112"));
        assert!(parsed_addresses.contains(&"EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"));
    }

    #[test]
    fn test_token_list_query_participate_filtering() {
        // 测试单个参与者过滤功能
        let query = TokenListQuery {
            participate: Some("8S2bcP66WehuF6cHryfZ7vfFpQWaUhYyAYSy5U3gX4Fy".to_string()),
            ..Default::default()
        };
        assert_eq!(
            query.participate,
            Some("8S2bcP66WehuF6cHryfZ7vfFpQWaUhYyAYSy5U3gX4Fy".to_string())
        );

        // 测试空参与者过滤
        let query = TokenListQuery {
            participate: Some("".to_string()),
            ..Default::default()
        };
        assert_eq!(query.participate, Some("".to_string()));

        // 测试None参与者过滤
        let query = TokenListQuery {
            participate: None,
            ..Default::default()
        };
        assert_eq!(query.participate, None);
    }

    #[test]
    fn test_token_list_query_participate_default() {
        // 测试默认查询不包含participate过滤
        let query = TokenListQuery::default();
        assert!(query.participate.is_none());
    }

    #[test]
    fn test_token_list_query_participate_with_other_filters() {
        // 测试participate与其他过滤条件结合使用
        let query = TokenListQuery {
            participate: Some("8S2bcP66WehuF6cHryfZ7vfFpQWaUhYyAYSy5U3gX4Fy".to_string()),
            status: Some(TokenStatus::Active),
            min_volume: Some(1000.0),
            verification: Some(VerificationStatus::Verified),
            ..Default::default()
        };

        assert_eq!(
            query.participate,
            Some("8S2bcP66WehuF6cHryfZ7vfFpQWaUhYyAYSy5U3gX4Fy".to_string())
        );
        assert_eq!(query.status, Some(TokenStatus::Active));
        assert_eq!(query.min_volume, Some(1000.0));
        assert_eq!(query.verification, Some(VerificationStatus::Verified));
    }
}
