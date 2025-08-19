use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::Validate;

/// 仓位状态枚举
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub enum PositionStatus {
    /// 活跃状态
    Active,
    /// 已关闭
    Closed,
    /// 暂停状态
    Paused,
    /// 错误状态
    Error,
}

impl Default for PositionStatus {
    fn default() -> Self {
        PositionStatus::Active
    }
}

/// 仓位扩展元数据
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PositionMetadata {
    /// 初始开仓交易签名
    pub initial_transaction_signature: Option<String>,
    /// 滑点容忍度
    pub slippage_tolerance: Option<f64>,
    /// 价格范围利用率
    pub price_range_utilization: Option<f64>,
    /// 性能指标数据
    pub performance_metrics: Option<serde_json::Value>,
    /// 其他自定义数据
    pub custom_data: Option<serde_json::Value>,
}

/// 仓位数据模型
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct Position {
    /// MongoDB文档ID
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,

    // ============ 基本标识信息 ============
    /// 链上仓位键值（唯一标识）
    #[validate(length(min = 32, max = 44))]
    pub position_key: String,

    /// NFT Mint地址
    #[validate(length(min = 32, max = 44))]
    pub nft_mint: String,

    /// 用户钱包地址
    #[validate(length(min = 32, max = 44))]
    pub user_wallet: String,

    /// 池子地址
    #[validate(length(min = 32, max = 44))]
    pub pool_address: String,

    // ============ 价格范围信息 ============
    /// 下限tick索引
    pub tick_lower_index: i32,

    /// 上限tick索引
    pub tick_upper_index: i32,

    /// 下限价格
    #[validate(range(min = 0.0))]
    pub tick_lower_price: f64,

    /// 上限价格
    #[validate(range(min = 0.0))]
    pub tick_upper_price: f64,

    // ============ 流动性信息 ============
    /// 初始流动性数量（字符串避免精度丢失）
    pub initial_liquidity: String,

    /// 当前流动性数量
    pub current_liquidity: String,

    /// 累计增加的流动性
    #[serde(default = "String::new")]
    pub total_liquidity_added: String,

    /// 累计减少的流动性
    #[serde(default = "String::new")]
    pub total_liquidity_removed: String,

    // ============ 代币数量信息 ============
    /// 初始token0数量
    pub initial_amount_0: u64,

    /// 初始token1数量
    pub initial_amount_1: u64,

    /// 当前token0数量
    pub current_amount_0: u64,

    /// 当前token1数量
    pub current_amount_1: u64,

    // ============ 状态管理 ============
    /// 仓位状态
    #[serde(default)]
    pub status: PositionStatus,

    /// 是否活跃
    #[serde(default = "default_true")]
    pub is_active: bool,

    /// 是否在价格范围内
    #[serde(default = "default_true")]
    pub is_in_range: bool,

    // ============ 手续费和奖励 ============
    /// 累计赚取的token0手续费
    #[serde(default)]
    pub fees_earned_0: u64,

    /// 累计赚取的token1手续费
    #[serde(default)]
    pub fees_earned_1: u64,

    /// 未领取的token0手续费
    #[serde(default)]
    pub unclaimed_fees_0: u64,

    /// 未领取的token1手续费
    #[serde(default)]
    pub unclaimed_fees_1: u64,

    // ============ 操作历史 ============
    /// 总操作次数
    #[serde(default = "default_one")]
    pub total_operations: u32,

    /// 最后操作类型
    pub last_operation_type: Option<String>,

    // ============ 时间信息 ============
    /// 创建时间戳
    #[serde(with = "mongodb::bson::serde_helpers::u64_as_f64")]
    pub created_at: u64,

    /// 最后更新时间戳
    #[serde(with = "mongodb::bson::serde_helpers::u64_as_f64")]
    pub updated_at: u64,

    /// 最后同步链上状态时间戳
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_sync_at: Option<u64>,

    // ============ 扩展元数据 ============
    /// 扩展元数据
    pub metadata: Option<PositionMetadata>,
}

impl Position {
    /// 创建新的仓位记录
    pub fn new(
        position_key: String,
        nft_mint: String,
        user_wallet: String,
        pool_address: String,
        tick_lower_index: i32,
        tick_upper_index: i32,
        tick_lower_price: f64,
        tick_upper_price: f64,
        initial_liquidity: String,
        initial_amount_0: u64,
        initial_amount_1: u64,
    ) -> Self {
        let now = chrono::Utc::now().timestamp() as u64;

        Self {
            id: None,
            position_key,
            nft_mint,
            user_wallet,
            pool_address,
            tick_lower_index,
            tick_upper_index,
            tick_lower_price,
            tick_upper_price,
            initial_liquidity: initial_liquidity.clone(),
            current_liquidity: initial_liquidity,
            total_liquidity_added: "0".to_string(),
            total_liquidity_removed: "0".to_string(),
            initial_amount_0,
            initial_amount_1,
            current_amount_0: initial_amount_0,
            current_amount_1: initial_amount_1,
            status: PositionStatus::Active,
            is_active: true,
            is_in_range: true,
            fees_earned_0: 0,
            fees_earned_1: 0,
            unclaimed_fees_0: 0,
            unclaimed_fees_1: 0,
            total_operations: 1,
            last_operation_type: Some("open".to_string()),
            created_at: now,
            updated_at: now,
            last_sync_at: None,
            metadata: None,
        }
    }

    /// 更新流动性信息
    pub fn update_liquidity(
        &mut self,
        new_liquidity: String,
        liquidity_change: String,
        is_increase: bool,
        amount_0_change: u64,
        amount_1_change: u64,
        operation_type: String,
    ) {
        self.current_liquidity = new_liquidity;

        if is_increase {
            // 增加流动性
            let current_added = self.total_liquidity_added.parse::<u128>().unwrap_or(0);
            let change_amount = liquidity_change.parse::<u128>().unwrap_or(0);
            self.total_liquidity_added = (current_added + change_amount).to_string();

            self.current_amount_0 = self.current_amount_0.saturating_add(amount_0_change);
            self.current_amount_1 = self.current_amount_1.saturating_add(amount_1_change);
        } else {
            // 减少流动性
            let current_removed = self.total_liquidity_removed.parse::<u128>().unwrap_or(0);
            let change_amount = liquidity_change.parse::<u128>().unwrap_or(0);
            self.total_liquidity_removed = (current_removed + change_amount).to_string();

            self.current_amount_0 = self.current_amount_0.saturating_sub(amount_0_change);
            self.current_amount_1 = self.current_amount_1.saturating_sub(amount_1_change);
        }

        self.total_operations += 1;
        self.last_operation_type = Some(operation_type);
        self.updated_at = chrono::Utc::now().timestamp() as u64;

        // 如果流动性归零，更新状态
        if self.current_liquidity == "0" {
            self.status = PositionStatus::Closed;
            self.is_active = false;
        }
    }

    /// 更新手续费信息
    pub fn update_fees(&mut self, fees_0: u64, fees_1: u64) {
        self.unclaimed_fees_0 += fees_0;
        self.unclaimed_fees_1 += fees_1;
        self.fees_earned_0 += fees_0;
        self.fees_earned_1 += fees_1;
        self.updated_at = chrono::Utc::now().timestamp() as u64;
    }

    /// 标记为已同步
    pub fn mark_synced(&mut self) {
        self.last_sync_at = Some(chrono::Utc::now().timestamp() as u64);
        self.updated_at = chrono::Utc::now().timestamp() as u64;
    }

    /// 关闭仓位
    pub fn close(&mut self) {
        self.status = PositionStatus::Closed;
        self.is_active = false;
        self.current_liquidity = "0".to_string();
        self.last_operation_type = Some("close".to_string());
        self.updated_at = chrono::Utc::now().timestamp() as u64;
    }

    /// 设置元数据
    pub fn set_metadata(&mut self, metadata: PositionMetadata) {
        self.metadata = Some(metadata);
        self.updated_at = chrono::Utc::now().timestamp() as u64;
    }
}

// 默认值辅助函数
fn default_true() -> bool {
    true
}

fn default_one() -> u32 {
    1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_position_creation() {
        let position = Position::new(
            "test_position_key".to_string(),
            "test_nft_mint".to_string(),
            "test_user_wallet".to_string(),
            "test_pool_address".to_string(),
            -1000,
            1000,
            0.001,
            0.002,
            "1000000".to_string(),
            500000,
            500000,
        );

        assert_eq!(position.position_key, "test_position_key");
        assert_eq!(position.initial_liquidity, "1000000");
        assert_eq!(position.current_liquidity, "1000000");
        assert_eq!(position.status, PositionStatus::Active);
        assert!(position.is_active);
        assert_eq!(position.total_operations, 1);
    }

    #[test]
    fn test_position_increase_liquidity() {
        let mut position = Position::new(
            "test_key".to_string(),
            "test_nft".to_string(),
            "test_wallet".to_string(),
            "test_pool".to_string(),
            -1000,
            1000,
            0.001,
            0.002,
            "1000000".to_string(),
            500000,
            500000,
        );

        position.update_liquidity(
            "1500000".to_string(),
            "500000".to_string(),
            true,
            250000,
            250000,
            "increase".to_string(),
        );

        assert_eq!(position.current_liquidity, "1500000");
        assert_eq!(position.total_liquidity_added, "500000");
        assert_eq!(position.current_amount_0, 750000);
        assert_eq!(position.current_amount_1, 750000);
        assert_eq!(position.total_operations, 2);
        assert_eq!(position.last_operation_type, Some("increase".to_string()));
    }

    #[test]
    fn test_position_decrease_liquidity() {
        let mut position = Position::new(
            "test_key".to_string(),
            "test_nft".to_string(),
            "test_wallet".to_string(),
            "test_pool".to_string(),
            -1000,
            1000,
            0.001,
            0.002,
            "1000000".to_string(),
            500000,
            500000,
        );

        position.update_liquidity(
            "500000".to_string(),
            "500000".to_string(),
            false,
            250000,
            250000,
            "decrease".to_string(),
        );

        assert_eq!(position.current_liquidity, "500000");
        assert_eq!(position.total_liquidity_removed, "500000");
        assert_eq!(position.current_amount_0, 250000);
        assert_eq!(position.current_amount_1, 250000);
        assert_eq!(position.total_operations, 2);
    }

    #[test]
    fn test_position_close() {
        let mut position = Position::new(
            "test_key".to_string(),
            "test_nft".to_string(),
            "test_wallet".to_string(),
            "test_pool".to_string(),
            -1000,
            1000,
            0.001,
            0.002,
            "1000000".to_string(),
            500000,
            500000,
        );

        position.close();

        assert_eq!(position.status, PositionStatus::Closed);
        assert!(!position.is_active);
        assert_eq!(position.current_liquidity, "0");
        assert_eq!(position.last_operation_type, Some("close".to_string()));
    }

    #[test]
    fn test_position_update_fees() {
        let mut position = Position::new(
            "test_key".to_string(),
            "test_nft".to_string(),
            "test_wallet".to_string(),
            "test_pool".to_string(),
            -1000,
            1000,
            0.001,
            0.002,
            "1000000".to_string(),
            500000,
            500000,
        );

        position.update_fees(1000, 2000);

        assert_eq!(position.fees_earned_0, 1000);
        assert_eq!(position.fees_earned_1, 2000);
        assert_eq!(position.unclaimed_fees_0, 1000);
        assert_eq!(position.unclaimed_fees_1, 2000);
    }

    #[test]
    fn test_position_validation() {
        let position = Position::new(
            "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM".to_string(),
            "8WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM".to_string(),
            "7WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM".to_string(),
            "6WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM".to_string(),
            -1000,
            1000,
            0.001,
            0.002,
            "1000000".to_string(),
            500000,
            500000,
        );

        assert!(position.validate().is_ok());
    }
}
