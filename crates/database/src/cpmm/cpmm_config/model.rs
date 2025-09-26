use chrono::{DateTime, Utc};
use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// CPMM配置模型 - 数据库存储模型
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CpmmConfigModel {
    /// MongoDB对象ID
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,

    /// 配置ID (链上地址)
    #[serde(rename = "configId")]
    pub config_id: String,

    /// 配置索引
    pub index: u32,

    /// 协议费率
    #[serde(rename = "protocolFeeRate")]
    pub protocol_fee_rate: u64,

    /// 交易费率
    #[serde(rename = "tradeFeeRate")]
    pub trade_fee_rate: u64,

    /// 基金费率
    #[serde(rename = "fundFeeRate")]
    pub fund_fee_rate: u64,

    /// 创建池子费用
    #[serde(rename = "createPoolFee")]
    pub create_pool_fee: String,

    /// 创建者费率
    #[serde(rename = "creatorFeeRate")]
    pub creator_fee_rate: u64,

    /// 是否启用
    pub enabled: bool,

    /// 创建时间
    #[serde(rename = "createdAt")]
    pub created_at: DateTime<Utc>,

    /// 更新时间
    #[serde(rename = "updatedAt")]
    pub updated_at: DateTime<Utc>,

    /// 最后同步时间 (从链上同步的时间)
    #[serde(rename = "lastSyncAt", skip_serializing_if = "Option::is_none")]
    pub last_sync_at: Option<DateTime<Utc>>,
}

impl CpmmConfigModel {
    /// 创建新的CPMM配置模型
    pub fn new(
        config_id: String,
        index: u32,
        protocol_fee_rate: u64,
        trade_fee_rate: u64,
        fund_fee_rate: u64,
        create_pool_fee: String,
        creator_fee_rate: u64,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: None,
            config_id,
            index,
            protocol_fee_rate,
            trade_fee_rate,
            fund_fee_rate,
            create_pool_fee,
            creator_fee_rate,
            enabled: true,
            created_at: now,
            updated_at: now,
            last_sync_at: Some(now),
        }
    }

    /// 更新配置信息
    pub fn update_config(
        &mut self,
        protocol_fee_rate: u64,
        trade_fee_rate: u64,
        fund_fee_rate: u64,
        create_pool_fee: String,
        creator_fee_rate: u64,
    ) {
        self.protocol_fee_rate = protocol_fee_rate;
        self.trade_fee_rate = trade_fee_rate;
        self.fund_fee_rate = fund_fee_rate;
        self.create_pool_fee = create_pool_fee;
        self.creator_fee_rate = creator_fee_rate;
        self.updated_at = Utc::now();
        self.last_sync_at = Some(Utc::now());
    }
}

/// CPMM配置查询参数
#[derive(Debug, Clone, Default)]
pub struct CpmmConfigQuery {
    /// 配置ID过滤
    pub config_id: Option<String>,
    /// 索引过滤
    pub index: Option<u32>,
    /// 是否启用过滤
    pub enabled: Option<bool>,
    /// 分页参数
    pub page: Option<i64>,
    pub limit: Option<i64>,
}

/// CPMM配置统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpmmConfigStats {
    /// 总配置数量
    pub total_configs: u64,
    /// 启用的配置数量
    pub enabled_configs: u64,
    /// 禁用的配置数量
    pub disabled_configs: u64,
    /// 最后同步时间
    pub last_sync_time: Option<DateTime<Utc>>,
}