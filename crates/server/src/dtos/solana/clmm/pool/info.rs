use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Raydium CLMM池子密钥信息响应格式
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PoolKeyResponse {
    /// 请求ID
    pub id: String,

    /// 请求是否成功
    pub success: bool,

    /// 池子密钥数据列表
    pub data: Vec<Option<PoolKeyInfo>>,
}

/// 池子密钥详细信息
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PoolKeyInfo {
    /// 程序ID
    #[serde(rename = "programId")]
    pub program_id: String,

    /// 池子ID（地址）
    pub id: String,

    /// 代币A信息
    #[serde(rename = "mintA")]
    pub mint_a: RaydiumMintInfo,

    /// 代币B信息
    #[serde(rename = "mintB")]
    pub mint_b: RaydiumMintInfo,

    /// 查找表账户
    #[serde(rename = "lookupTableAccount")]
    pub lookup_table_account: String,

    /// 开放时间
    #[serde(rename = "openTime")]
    pub open_time: String,

    /// 金库信息
    pub vault: VaultAddresses,

    /// 配置信息
    pub config: PoolConfig,

    /// 奖励信息列表
    #[serde(rename = "rewardInfos")]
    pub reward_infos: Vec<PoolRewardInfo>,

    /// 观察账户ID
    #[serde(rename = "observationId")]
    pub observation_id: String,

    /// 扩展位图账户
    #[serde(rename = "exBitmapAccount")]
    pub ex_bitmap_account: String,
}

/// Raydium代币信息
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RaydiumMintInfo {
    /// 链ID
    #[serde(rename = "chainId")]
    pub chain_id: u32,

    /// 代币地址
    pub address: String,

    /// 程序ID
    #[serde(rename = "programId")]
    pub program_id: String,

    /// 图标URI
    #[serde(rename = "logoURI")]
    pub logo_uri: String,

    /// 代币符号
    pub symbol: String,

    /// 代币名称
    pub name: String,

    /// 精度
    pub decimals: u8,

    /// 标签列表
    pub tags: Vec<String>,

    /// 扩展信息
    pub extensions: serde_json::Value,
}

/// 金库地址信息
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct VaultAddresses {
    /// 代币A金库
    #[serde(rename = "A")]
    pub vault_a: String,

    /// 代币B金库
    #[serde(rename = "B")]
    pub vault_b: String,
}

/// 池子配置信息
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PoolConfig {
    /// 配置ID
    pub id: String,

    /// 配置索引
    pub index: u32,

    /// 协议费率
    #[serde(rename = "protocolFeeRate")]
    pub protocol_fee_rate: u64,

    /// 交易费率
    #[serde(rename = "tradeFeeRate")]
    pub trade_fee_rate: u64,

    /// Tick间距
    #[serde(rename = "tickSpacing")]
    pub tick_spacing: u32,

    /// 基金费率
    #[serde(rename = "fundFeeRate")]
    pub fund_fee_rate: u64,

    /// 默认价格范围
    #[serde(rename = "defaultRange")]
    pub default_range: f64,

    /// 默认价格范围点位
    #[serde(rename = "defaultRangePoint")]
    pub default_range_point: Vec<f64>,
}

/// 池子奖励信息
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PoolRewardInfo {
    /// 奖励代币mint地址
    pub mint: String,

    /// 奖励金库地址
    pub vault: String,

    /// 每秒发放量
    pub emissions_per_second: u64,

    /// 权限地址
    pub authority: String,

    /// 最后更新时间
    pub last_update_time: u64,
}
