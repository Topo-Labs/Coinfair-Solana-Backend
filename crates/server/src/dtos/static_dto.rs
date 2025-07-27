use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// 静态 API 响应结构体
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ApiResponse<T> {
    /// 请求ID
    pub id: String,

    /// 是否成功
    pub success: bool,

    /// 响应数据
    pub data: T,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            success: true,
            data,
        }
    }
}

/// 版本配置
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct VersionConfig {
    /// 最新版本
    pub latest: String,

    /// 最低版本
    pub least: String,
}

/// 自动费用配置
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AutoFeeConfig {
    /// 默认费用配置
    pub default: DefaultFeeConfig,
}

/// 默认费用配置
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DefaultFeeConfig {
    /// 极高费用
    pub vh: u64,

    /// 高费用
    pub h: u64,

    /// 中等费用
    pub m: u64,
}

impl AutoFeeConfig {
    pub fn default_fees() -> DefaultFeeConfig {
        DefaultFeeConfig { vh: 25216, h: 18912, m: 10000 }
    }
}

/// RPC 节点配置
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RpcConfig {
    /// 策略
    pub strategy: String,

    /// RPC 节点列表
    pub rpcs: Vec<RpcNode>,
}

/// RPC 节点信息
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RpcNode {
    /// 节点URL
    pub url: String,

    /// 是否支持批量请求
    pub batch: bool,

    /// 节点名称
    pub name: String,

    /// 权重
    pub weight: u32,
}

impl Default for RpcConfig {
    fn default() -> Self {
        Self {
            strategy: "weight".to_string(),
            rpcs: vec![RpcNode {
                url: "https://api.devnet.solana.com".to_string(),
                batch: true,
                name: "Devnet".to_string(),
                weight: 100,
            }],
        }
    }
}

/// 链时间配置
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ChainTimeConfig {
    /// 时间值
    pub value: String,
}

/// 代币列表响应
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct MintListResponse {
    /// 黑名单
    pub blacklist: Vec<String>,

    /// 代币列表
    #[serde(rename = "mintList")]
    pub mint_list: Vec<TokenInfo>,

    /// 白名单
    #[serde(rename = "whiteList")]
    pub white_list: Vec<String>,
}

/// 代币信息
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TokenInfo {
    /// 代币地址
    pub address: String,

    /// 代币名称
    pub name: String,

    /// 代币符号
    pub symbol: String,

    /// 小数位数
    pub decimals: u8,

    /// 图标URI
    #[serde(rename = "logoURI")]
    pub logo_uri: String,

    /// 标签
    pub tags: Vec<String>,

    /// 日交易量
    pub daily_volume: f64,

    /// 创建时间
    pub created_at: DateTime<Utc>,

    /// 冻结权限
    pub freeze_authority: Option<String>,

    /// 铸造权限
    pub mint_authority: Option<String>,

    /// 永久委托
    pub permanent_delegate: Option<String>,

    /// 铸造时间
    pub minted_at: Option<DateTime<Utc>>,

    /// 扩展信息
    pub extensions: serde_json::Value,
}

impl Default for MintListResponse {
    fn default() -> Self {
        Self {
            blacklist: vec![],
            white_list: vec![],
            mint_list: vec![
                TokenInfo {
                    address: "CKgtJw9y47qAgxRHBdgjABY7DP4u6bLHXM1G68anWwJm".to_string(),
                    name: "JM-M1".to_string(),
                    symbol: "JM-M1".to_string(),
                    decimals: 6,
                    logo_uri: "http://localhost:8000/static/coin.png".to_string(),
                    tags: vec![],
                    daily_volume: 0.0,
                    created_at: DateTime::parse_from_rfc3339("2025-04-15T10:56:58.893768Z").unwrap().with_timezone(&Utc),
                    freeze_authority: Some("H8oqsdn6ETdgow2m7dTKh3tG2J6ns43FjA4HWnteX6Sx".to_string()),
                    mint_authority: Some("H8oqsdn6ETdgow2m7dTKh3tG2J6ns43FjA4HWnteX6Sx".to_string()),
                    permanent_delegate: None,
                    minted_at: None,
                    extensions: serde_json::json!({}),
                },
                TokenInfo {
                    address: "5pbcULDGXotRZjJvmoiqj3qYaHJeDYAWpsaT58j6Ao56".to_string(),
                    name: "56-M0".to_string(),
                    symbol: "56-M0".to_string(),
                    decimals: 6,
                    logo_uri: "http://localhost:8000/static/coin.png".to_string(),
                    tags: vec![],
                    daily_volume: 0.0,
                    created_at: DateTime::parse_from_rfc3339("2025-04-15T10:56:58.893768Z").unwrap().with_timezone(&Utc),
                    freeze_authority: Some("H8oqsdn6ETdgow2m7dTKh3tG2J6ns43FjA4HWnteX6Sx".to_string()),
                    mint_authority: Some("H8oqsdn6ETdgow2m7dTKh3tG2J6ns43FjA4HWnteX6Sx".to_string()),
                    permanent_delegate: None,
                    minted_at: None,
                    extensions: serde_json::json!({}),
                },
                TokenInfo {
                    address: "9C57seuQ3B6yNTmxwU4TdxmCwHEQWq8SMQUn6MYKXxUU".to_string(),
                    name: "cftest1".to_string(),
                    symbol: "CFT1".to_string(),
                    decimals: 9,
                    logo_uri: "http://localhost:8000/static/coin.png".to_string(),
                    tags: vec!["community".to_string(), "strict".to_string(), "verified".to_string()],
                    daily_volume: 0.0,
                    created_at: DateTime::parse_from_rfc3339("2025-04-15T10:56:58.893768Z").unwrap().with_timezone(&Utc),
                    freeze_authority: Some("H8oqsdn6ETdgow2m7dTKh3tG2J6ns43FjA4HWnteX6Sx".to_string()),
                    mint_authority: Some("H8oqsdn6ETdgow2m7dTKh3tG2J6ns43FjA4HWnteX6Sx".to_string()),
                    permanent_delegate: None,
                    minted_at: None,
                    extensions: serde_json::json!({"coingeckoId": "cftest1"}),
                },
                TokenInfo {
                    address: "4W4WpXG85nsZEGBdFJsnAR1BgFhR688BgHUqmvwnjgNE".to_string(),
                    name: "cftest2".to_string(),
                    symbol: "CFT2".to_string(),
                    decimals: 9,
                    logo_uri: "http://localhost:8000/static/coin.png".to_string(),
                    tags: vec!["community".to_string(), "strict".to_string(), "verified".to_string()],
                    daily_volume: 0.0,
                    created_at: DateTime::parse_from_rfc3339("2025-04-15T10:56:58.893768Z").unwrap().with_timezone(&Utc),
                    freeze_authority: Some("H8oqsdn6ETdgow2m7dTKh3tG2J6ns43FjA4HWnteX6Sx".to_string()),
                    mint_authority: Some("H8oqsdn6ETdgow2m7dTKh3tG2J6ns43FjA4HWnteX6Sx".to_string()),
                    permanent_delegate: None,
                    minted_at: None,
                    extensions: serde_json::json!({"coingeckoId": "cftest1"}),
                },
                TokenInfo {
                    address: "4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU".to_string(),
                    name: "USD Coin".to_string(),
                    symbol: "USDC".to_string(),
                    decimals: 6,
                    logo_uri: "https://raw.githubusercontent.com/solana-labs/token-list/main/assets/mainnet/EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v/logo.png".to_string(),
                    tags: vec!["community".to_string(), "strict".to_string(), "verified".to_string()],
                    daily_volume: 1047104708.8575294,
                    created_at: DateTime::parse_from_rfc3339("2024-04-26T10:56:58.893768Z").unwrap().with_timezone(&Utc),
                    freeze_authority: Some("CJtyoKSLrktozQzjERTiK3btQtiTK3nN4QrqGHLidyCT".to_string()),
                    mint_authority: Some("GrNg1XM2ctzeE2mXxXCfhcTUbejM8Z4z4wNVTy2FjMEz".to_string()),
                    permanent_delegate: None,
                    minted_at: None,
                    extensions: serde_json::json!({"coingeckoId": "usd-coin"}),
                },
                TokenInfo {
                    address: "CF1Ms9vjvGEiSHqoj1jLadoLNXD9EqtnR6TZp1w8CeHz".to_string(),
                    name: "FAIR".to_string(),
                    symbol: "FAIR".to_string(),
                    decimals: 9,
                    logo_uri: "https://img-v1.raydium.io/icon/CF1Ms9vjvGEiSHqoj1jLadoLNXD9EqtnR6TZp1w8CeHz.png".to_string(),
                    tags: vec![],
                    daily_volume: 0.0,
                    created_at: DateTime::parse_from_rfc3339("2025-04-15T10:56:58.893768Z").unwrap().with_timezone(&Utc),
                    freeze_authority: Some("H8oqsdn6ETdgow2m7dTKh3tG2J6ns43FjA4HWnteX6Sx".to_string()),
                    mint_authority: Some("H8oqsdn6ETdgow2m7dTKh3tG2J6ns43FjA4HWnteX6Sx".to_string()),
                    permanent_delegate: None,
                    minted_at: None,
                    extensions: serde_json::json!({}),
                },
            ],
        }
    }
}

/// 价格数据项
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PriceData {
    /// 代币mint地址
    pub mint: String,

    /// 价格
    pub price: String,
}

/// 代币价格响应
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct MintPriceResponse {
    /// 价格数据
    pub data: Vec<PriceData>,
}

/// 系统信息响应
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct InfoResponse {
    /// 24小时交易量
    pub volume24: f64,

    /// 总锁定价值
    pub tvl: f64,
}
