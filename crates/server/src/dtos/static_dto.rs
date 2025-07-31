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

    pub program_id: String,

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
                    program_id: "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb".to_string(),
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
                    program_id: "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb".to_string(),
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
                    program_id: "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA".to_string(),
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
                    program_id: "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA".to_string(),
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
                    program_id: "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA".to_string(),
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
                    program_id: "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA".to_string(),
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

/// CLMM配置项
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ClmmConfig {
    /// 配置ID
    pub id: String,

    /// 索引
    pub index: u32,

    /// 协议费率
    #[serde(rename = "protocolFeeRate")]
    pub protocol_fee_rate: u64,

    /// 交易费率
    #[serde(rename = "tradeFeeRate")]
    pub trade_fee_rate: u64,

    /// tick间距
    #[serde(rename = "tickSpacing")]
    pub tick_spacing: u32,

    /// 基金费率
    #[serde(rename = "fundFeeRate")]
    pub fund_fee_rate: u64,

    /// 默认范围
    #[serde(rename = "defaultRange")]
    pub default_range: f64,

    /// 默认范围点
    #[serde(rename = "defaultRangePoint")]
    pub default_range_point: Vec<f64>,
}

/// CLMM配置响应类型
pub type ClmmConfigResponse = Vec<ClmmConfig>;

/// 创建AMM配置请求
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, validator::Validate)]
pub struct CreateAmmConfigRequest {
    /// 配置索引
    #[validate(range(min = 0, max = 65535))]
    pub config_index: u16,

    /// tick间距 - 决定价格点之间的间隔
    #[validate(range(min = 1, max = 1000))]
    pub tick_spacing: u16,

    /// 交易费率 - 以百万分之一为单位 (10^-6)
    #[validate(range(min = 0, max = 1000000))]
    pub trade_fee_rate: u32,

    /// 协议费率 - 以百万分之一为单位 (10^-6)
    #[validate(range(min = 0, max = 1000000))]
    pub protocol_fee_rate: u32,

    /// 基金费率 - 以百万分之一为单位 (10^-6)
    #[validate(range(min = 0, max = 1000000))]
    pub fund_fee_rate: u32,
}

/// 创建AMM配置响应（构建交易）
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateAmmConfigResponse {
    /// 序列化的交易数据（Base64）
    pub transaction: String,

    /// 交易消息描述
    #[serde(rename = "transactionMessage")]
    pub transaction_message: String,

    /// 创建的配置地址
    #[serde(rename = "configAddress")]
    pub config_address: String,

    /// 配置索引
    #[serde(rename = "configIndex")]
    pub config_index: u16,

    /// tick间距
    #[serde(rename = "tickSpacing")]
    pub tick_spacing: u16,

    /// 交易费率
    #[serde(rename = "tradeFeeRate")]
    pub trade_fee_rate: u32,

    /// 协议费率
    #[serde(rename = "protocolFeeRate")]
    pub protocol_fee_rate: u32,

    /// 基金费率
    #[serde(rename = "fundFeeRate")]
    pub fund_fee_rate: u32,

    /// 创建时间戳
    pub timestamp: i64,
}

/// 创建AMM配置并发送交易响应
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateAmmConfigAndSendTransactionResponse {
    /// 交易签名
    pub signature: String,

    /// 创建的配置地址
    #[serde(rename = "configAddress")]
    pub config_address: String,

    /// 配置索引
    #[serde(rename = "configIndex")]
    pub config_index: u16,

    /// tick间距
    #[serde(rename = "tickSpacing")]
    pub tick_spacing: u16,

    /// 交易费率
    #[serde(rename = "tradeFeeRate")]
    pub trade_fee_rate: u32,

    /// 协议费率
    #[serde(rename = "protocolFeeRate")]
    pub protocol_fee_rate: u32,

    /// 基金费率
    #[serde(rename = "fundFeeRate")]
    pub fund_fee_rate: u32,

    /// 浏览器链接
    #[serde(rename = "explorerUrl")]
    pub explorer_url: String,

    /// 配置保存到数据库的响应
    #[serde(rename = "dbSaveResponse")]
    pub db_save_response: SaveClmmConfigResponse,

    /// 创建时间戳
    pub timestamp: i64,
}

/// 保存CLMM配置请求
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, validator::Validate)]
pub struct SaveClmmConfigRequest {
    /// 索引
    #[validate(range(min = 0, max = 100000))]
    pub index: u32,

    /// 协议费率
    #[serde(rename = "protocolFeeRate")]
    #[validate(range(min = 0, max = 1000000))]
    pub protocol_fee_rate: u64,

    /// 交易费率
    #[serde(rename = "tradeFeeRate")]
    #[validate(range(min = 0, max = 100000))]
    pub trade_fee_rate: u64,

    /// tick间距
    #[serde(rename = "tickSpacing")]
    #[validate(range(min = 1, max = 1000))]
    pub tick_spacing: u32,

    /// 基金费率
    #[serde(rename = "fundFeeRate")]
    #[validate(range(min = 0, max = 1000000))]
    pub fund_fee_rate: u64,

    /// 默认范围
    #[serde(rename = "defaultRange")]
    #[validate(range(min = 0.001, max = 1.0))]
    pub default_range: f64,

    /// 默认范围点
    #[serde(rename = "defaultRangePoint")]
    #[validate(length(min = 1, max = 10))]
    pub default_range_point: Vec<f64>,
}

/// 保存CLMM配置响应
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SaveClmmConfigResponse {
    /// 配置ID
    pub id: String,

    /// 是否为新创建的配置
    pub created: bool,

    /// 消息
    pub message: String,
}

impl ClmmConfig {
    /// 创建默认的CLMM配置数据
    pub fn default_configs() -> Vec<ClmmConfig> {
        vec![
            ClmmConfig {
                id: "9iFER3bpjf1PTTCQCfTRu17EJgvsxo9pVyA9QWwEuX4x".to_string(),
                index: 4,
                protocol_fee_rate: 120000,
                trade_fee_rate: 100,
                tick_spacing: 1,
                fund_fee_rate: 40000,
                default_range: 0.001,
                default_range_point: vec![0.001, 0.003, 0.005, 0.008, 0.01],
            },
            ClmmConfig {
                id: "EdPxg8QaeFSrTYqdWJn6Kezwy9McWncTYueD9eMGCuzR".to_string(),
                index: 6,
                protocol_fee_rate: 120000,
                trade_fee_rate: 200,
                tick_spacing: 1,
                fund_fee_rate: 40000,
                default_range: 0.1,
                default_range_point: vec![0.01, 0.05, 0.1, 0.2, 0.5],
            },
            ClmmConfig {
                id: "9EeWRCL8CJnikDFCDzG8rtmBs5KQR1jEYKCR5rRZ2NEi".to_string(),
                index: 7,
                protocol_fee_rate: 120000,
                trade_fee_rate: 300,
                tick_spacing: 1,
                fund_fee_rate: 40000,
                default_range: 0.1,
                default_range_point: vec![0.01, 0.05, 0.1, 0.2, 0.5],
            },
            ClmmConfig {
                id: "3h2e43PunVA5K34vwKCLHWhZF4aZpyaC9RmxvshGAQpL".to_string(),
                index: 8,
                protocol_fee_rate: 120000,
                trade_fee_rate: 400,
                tick_spacing: 1,
                fund_fee_rate: 40000,
                default_range: 0.1,
                default_range_point: vec![0.01, 0.05, 0.1, 0.2, 0.5],
            },
            ClmmConfig {
                id: "3XCQJQryqpDvvZBfGxR7CLAw5dpGJ9aa7kt1jRLdyxuZ".to_string(),
                index: 5,
                protocol_fee_rate: 120000,
                trade_fee_rate: 500,
                tick_spacing: 1,
                fund_fee_rate: 40000,
                default_range: 0.1,
                default_range_point: vec![0.01, 0.05, 0.1, 0.2, 0.5],
            },
            ClmmConfig {
                id: "DrdecJVzkaRsf1TQu1g7iFncaokikVTHqpzPjenjRySY".to_string(),
                index: 10,
                protocol_fee_rate: 120000,
                trade_fee_rate: 1000,
                tick_spacing: 10,
                fund_fee_rate: 40000,
                default_range: 0.1,
                default_range_point: vec![0.01, 0.05, 0.1, 0.2, 0.5],
            },
            ClmmConfig {
                id: "J8u7HvA1g1p2CdhBFdsnTxDzGkekRpdw4GrL9MKU2D3U".to_string(),
                index: 11,
                protocol_fee_rate: 120000,
                trade_fee_rate: 1500,
                tick_spacing: 10,
                fund_fee_rate: 40000,
                default_range: 0.1,
                default_range_point: vec![0.01, 0.05, 0.1, 0.2, 0.5],
            },
            ClmmConfig {
                id: "RPxHtdN5V7ajwkoG6NnwSBAeaX5k9giY37dpp98xTjD".to_string(),
                index: 12,
                protocol_fee_rate: 120000,
                trade_fee_rate: 1600,
                tick_spacing: 10,
                fund_fee_rate: 40000,
                default_range: 0.1,
                default_range_point: vec![0.01, 0.05, 0.1, 0.2, 0.5],
            },
            ClmmConfig {
                id: "9WjDVMHWCirG9jkchbetHTnSzdXbAPnD9bsoGRcz1xUw".to_string(),
                index: 13,
                protocol_fee_rate: 120000,
                trade_fee_rate: 1800,
                tick_spacing: 10,
                fund_fee_rate: 40000,
                default_range: 0.1,
                default_range_point: vec![0.01, 0.05, 0.1, 0.2, 0.5],
            },
            ClmmConfig {
                id: "FMrUDGjEe1izXPbn8SZPNjMfB5JvvhVq5ymmpZDebB5R".to_string(),
                index: 14,
                protocol_fee_rate: 120000,
                trade_fee_rate: 2000,
                tick_spacing: 10,
                fund_fee_rate: 40000,
                default_range: 0.1,
                default_range_point: vec![0.01, 0.05, 0.1, 0.2, 0.5],
            },
            ClmmConfig {
                id: "E64NGkDLLCdQ2yFNPcavaKptrEgmiQaNykUuLC1Qgwyp".to_string(),
                index: 1,
                protocol_fee_rate: 120000,
                trade_fee_rate: 2500,
                tick_spacing: 60,
                fund_fee_rate: 40000,
                default_range: 0.1,
                default_range_point: vec![0.01, 0.05, 0.1, 0.2, 0.5],
            },
            ClmmConfig {
                id: "Y6YhgJbt9FRk3JVjwdZtsioVCJwCKhy1hum8HMDYyB1".to_string(),
                index: 15,
                protocol_fee_rate: 120000,
                trade_fee_rate: 4000,
                tick_spacing: 60,
                fund_fee_rate: 40000,
                default_range: 0.1,
                default_range_point: vec![0.01, 0.05, 0.1, 0.2, 0.5],
            },
            ClmmConfig {
                id: "47Nq74YtwjVeTQF6KFKRKU4cY1Vd5AXBHpYRkubkDLZi".to_string(),
                index: 16,
                protocol_fee_rate: 120000,
                trade_fee_rate: 6000,
                tick_spacing: 60,
                fund_fee_rate: 40000,
                default_range: 0.1,
                default_range_point: vec![0.01, 0.05, 0.1, 0.2, 0.5],
            },
            ClmmConfig {
                id: "DQeN7dZyQvXKT7YwmgqyuC7AYFkwMoP7RwtucsDEdfYZ".to_string(),
                index: 17,
                protocol_fee_rate: 120000,
                trade_fee_rate: 8000,
                tick_spacing: 60,
                fund_fee_rate: 40000,
                default_range: 0.1,
                default_range_point: vec![0.01, 0.05, 0.1, 0.2, 0.5],
            },
            ClmmConfig {
                id: "A1BBtTYJd4i3xU8D6Tc2FzU6ZN4oXZWXKZnCxwbHXr8x".to_string(),
                index: 3,
                protocol_fee_rate: 120000,
                trade_fee_rate: 10000,
                tick_spacing: 120,
                fund_fee_rate: 40000,
                default_range: 0.1,
                default_range_point: vec![0.01, 0.05, 0.1, 0.2, 0.5],
            },
            ClmmConfig {
                id: "Gex2NJRS3jVLPfbzSFM5d5DRsNoL5ynnwT1TXoDEhanz".to_string(),
                index: 9,
                protocol_fee_rate: 120000,
                trade_fee_rate: 20000,
                tick_spacing: 120,
                fund_fee_rate: 40000,
                default_range: 0.1,
                default_range_point: vec![0.01, 0.05, 0.1, 0.2, 0.5],
            },
            ClmmConfig {
                id: "CDpiwv9eLsRvvuzZEJ8CBtK14wdvkSnkub4vmGtzzdK8".to_string(),
                index: 18,
                protocol_fee_rate: 120000,
                trade_fee_rate: 30000,
                tick_spacing: 120,
                fund_fee_rate: 40000,
                default_range: 0.1,
                default_range_point: vec![0.01, 0.05, 0.1, 0.2, 0.5],
            },
            ClmmConfig {
                id: "6tBc3ABLaYTTWu94DiRD5PWi92HML34UpAQ8pPTYgudw".to_string(),
                index: 19,
                protocol_fee_rate: 120000,
                trade_fee_rate: 40000,
                tick_spacing: 120,
                fund_fee_rate: 40000,
                default_range: 0.1,
                default_range_point: vec![0.01, 0.05, 0.1, 0.2, 0.5],
            },
        ]
    }
}
