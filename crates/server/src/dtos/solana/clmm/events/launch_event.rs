use database::events::event_model::LaunchEvent;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// LaunchEvent响应DTO
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct LaunchEventResponse {
    /// 事件ID
    pub id: Option<String>,
    /// meme币合约地址
    pub meme_token_mint: String,
    /// 配对代币地址
    pub base_token_mint: String,
    /// 用户钱包地址
    pub user_wallet: String,
    /// CLMM配置索引
    pub config_index: u32,
    /// 初始价格
    pub initial_price: f64,
    /// 价格下限
    pub tick_lower_price: f64,
    /// 价格上限
    pub tick_upper_price: f64,
    /// meme币数量
    pub meme_token_amount: u64,
    /// 配对代币数量
    pub base_token_amount: u64,
    /// 最大滑点百分比
    pub max_slippage_percent: f64,
    /// 是否包含NFT元数据
    pub with_metadata: bool,
    /// 池子开放时间戳，0表示立即开放
    pub open_time: u64,
    /// 发射时间戳
    pub launched_at: i64,
    /// 迁移状态
    pub migration_status: String,
    /// 迁移后的池子地址
    pub migrated_pool_address: Option<String>,
    /// 迁移完成时间
    pub migration_completed_at: Option<i64>,
    /// 迁移错误信息
    pub migration_error: Option<String>,
    /// 迁移重试次数
    pub migration_retry_count: u32,
    /// 流动性总价值（USD）
    pub total_liquidity_usd: f64,
    /// 代币对类型
    pub pair_type: String,
    /// 价格区间宽度百分比
    pub price_range_width_percent: f64,
    /// 是否为高价值发射
    pub is_high_value_launch: bool,
    /// 交易签名
    pub signature: String,
    /// 区块高度
    pub slot: u64,
    /// 处理时间
    pub processed_at: i64,
    /// 最后更新时间
    pub updated_at: i64,
}

/// LaunchEvent统计响应DTO
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct LaunchEventStatsResponse {
    /// 总Launch事件数
    pub total_launches: u64,
    /// 迁移成功率
    pub migration_success_rate: f64,
    /// 待迁移数量
    pub pending_count: u64,
    /// 成功数量
    pub success_count: u64,
    /// 失败数量
    pub failed_count: u64,
    /// 重试中数量
    pub retrying_count: u64,
}

/// 数据转换：从LaunchEvent到LaunchEventResponse
impl From<LaunchEvent> for LaunchEventResponse {
    fn from(event: LaunchEvent) -> Self {
        Self {
            id: event.id.map(|id| id.to_hex()),
            meme_token_mint: event.meme_token_mint,
            base_token_mint: event.base_token_mint,
            user_wallet: event.user_wallet,
            config_index: event.config_index,
            initial_price: event.initial_price,
            tick_lower_price: event.tick_lower_price,
            tick_upper_price: event.tick_upper_price,
            meme_token_amount: event.meme_token_amount,
            base_token_amount: event.base_token_amount,
            max_slippage_percent: event.max_slippage_percent,
            with_metadata: event.with_metadata,
            open_time: event.open_time,
            launched_at: event.launched_at,
            migration_status: event.migration_status,
            migrated_pool_address: event.migrated_pool_address,
            migration_completed_at: event.migration_completed_at,
            migration_error: event.migration_error,
            migration_retry_count: event.migration_retry_count,
            total_liquidity_usd: event.total_liquidity_usd,
            pair_type: event.pair_type,
            price_range_width_percent: event.price_range_width_percent,
            is_high_value_launch: event.is_high_value_launch,
            signature: event.signature,
            slot: event.slot,
            processed_at: event.processed_at,
            updated_at: event.updated_at,
        }
    }
}