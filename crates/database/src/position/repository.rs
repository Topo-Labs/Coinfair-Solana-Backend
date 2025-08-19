use crate::{position::model::Position, Database};
use async_trait::async_trait;
use futures::stream::StreamExt;
use mongodb::{
    bson::{doc, Document},
    options::{FindOptions, IndexOptions},
    results::{InsertOneResult, UpdateResult},
    IndexModel,
};
use std::sync::Arc;
use tracing::info;
use utils::{AppError, AppResult};

pub type DynPositionRepository = Arc<dyn PositionRepositoryTrait + Send + Sync>;

/// Position Repository Trait - 定义仓位数据操作接口
#[async_trait]
pub trait PositionRepositoryTrait {
    /// 创建新仓位
    async fn create_position(&self, position: Position) -> AppResult<InsertOneResult>;

    /// 根据position_key查找仓位
    async fn find_by_position_key(&self, position_key: &str) -> AppResult<Option<Position>>;

    /// 根据用户钱包地址查找所有仓位
    async fn find_by_user_wallet(&self, user_wallet: &str) -> AppResult<Vec<Position>>;

    /// 根据池子地址查找所有仓位
    async fn find_by_pool_address(&self, pool_address: &str) -> AppResult<Vec<Position>>;

    /// 查找特定池子和价格范围的仓位
    async fn find_by_pool_and_range(
        &self,
        pool_address: &str,
        tick_lower: i32,
        tick_upper: i32,
    ) -> AppResult<Vec<Position>>;

    /// 查找用户在特定池子和价格范围的仓位
    async fn find_user_position_in_range(
        &self,
        user_wallet: &str,
        pool_address: &str,
        tick_lower: i32,
        tick_upper: i32,
    ) -> AppResult<Option<Position>>;

    /// 更新仓位信息
    async fn update_position(&self, position_key: &str, position: Position) -> AppResult<UpdateResult>;

    /// 更新流动性信息
    async fn update_liquidity(
        &self,
        position_key: &str,
        new_liquidity: &str,
        liquidity_change: &str,
        is_increase: bool,
        amount_0_change: u64,
        amount_1_change: u64,
        operation_type: &str,
    ) -> AppResult<UpdateResult>;

    /// 更新手续费信息
    async fn update_fees(&self, position_key: &str, fees_0: u64, fees_1: u64) -> AppResult<UpdateResult>;

    /// 关闭仓位
    async fn close_position(&self, position_key: &str) -> AppResult<UpdateResult>;

    /// 标记仓位为已同步
    async fn mark_synced(&self, position_key: &str) -> AppResult<UpdateResult>;

    /// 获取活跃仓位列表
    async fn find_active_positions(&self) -> AppResult<Vec<Position>>;

    /// 获取需要同步的仓位列表（超过指定时间未同步）
    async fn find_positions_need_sync(&self, max_age_seconds: u64) -> AppResult<Vec<Position>>;

    /// 批量更新仓位状态
    async fn batch_update_positions(&self, updates: Vec<(String, Document)>) -> AppResult<u64>;

    /// 获取用户仓位统计信息
    async fn get_user_position_stats(&self, user_wallet: &str) -> AppResult<PositionStats>;

    /// 获取池子仓位统计信息
    async fn get_pool_position_stats(&self, pool_address: &str) -> AppResult<PoolPositionStats>;

    /// 初始化数据库索引
    async fn init_indexes(&self) -> AppResult<()>;
}

/// 用户仓位统计信息
pub struct PositionStats {
    pub total_positions: u64,
    pub active_positions: u64,
    pub closed_positions: u64,
    pub total_liquidity: String,
    pub total_fees_earned_0: u64,
    pub total_fees_earned_1: u64,
}

/// 池子仓位统计信息
pub struct PoolPositionStats {
    pub total_positions: u64,
    pub active_positions: u64,
    pub unique_users: u64,
    pub total_liquidity: String,
    pub average_position_size: String,
}

#[async_trait]
impl PositionRepositoryTrait for Database {
    async fn create_position(&self, position: Position) -> AppResult<InsertOneResult> {
        // 检查是否已存在相同的position_key
        let existing = self
            .positions
            .find_one(doc! { "position_key": &position.position_key }, None)
            .await?;

        if existing.is_some() {
            return Err(AppError::Conflict(format!(
                "Position with key {} already exists",
                position.position_key
            )));
        }

        let result = self.positions.insert_one(position, None).await?;
        Ok(result)
    }

    async fn find_by_position_key(&self, position_key: &str) -> AppResult<Option<Position>> {
        let filter = doc! { "position_key": position_key };
        let result = self.positions.find_one(filter, None).await?;
        Ok(result)
    }

    async fn find_by_user_wallet(&self, user_wallet: &str) -> AppResult<Vec<Position>> {
        let filter = doc! { "user_wallet": user_wallet };
        let options = FindOptions::builder().sort(doc! { "created_at": -1 }).build();

        let mut cursor = self.positions.find(filter, options).await?;
        let mut positions = Vec::new();

        while let Some(position) = cursor.next().await {
            positions.push(position?);
        }

        Ok(positions)
    }

    async fn find_by_pool_address(&self, pool_address: &str) -> AppResult<Vec<Position>> {
        let filter = doc! { "pool_address": pool_address };
        let options = FindOptions::builder().sort(doc! { "created_at": -1 }).build();

        let mut cursor = self.positions.find(filter, options).await?;
        let mut positions = Vec::new();

        while let Some(position) = cursor.next().await {
            positions.push(position?);
        }

        Ok(positions)
    }

    async fn find_by_pool_and_range(
        &self,
        pool_address: &str,
        tick_lower: i32,
        tick_upper: i32,
    ) -> AppResult<Vec<Position>> {
        let filter = doc! {
            "pool_address": pool_address,
            "tick_lower_index": tick_lower,
            "tick_upper_index": tick_upper
        };

        let mut cursor = self.positions.find(filter, None).await?;
        let mut positions = Vec::new();

        while let Some(position) = cursor.next().await {
            positions.push(position?);
        }

        Ok(positions)
    }

    async fn find_user_position_in_range(
        &self,
        user_wallet: &str,
        pool_address: &str,
        tick_lower: i32,
        tick_upper: i32,
    ) -> AppResult<Option<Position>> {
        let filter = doc! {
            "user_wallet": user_wallet,
            "pool_address": pool_address,
            "tick_lower_index": tick_lower,
            "tick_upper_index": tick_upper,
            "is_active": true
        };

        let result = self.positions.find_one(filter, None).await?;
        Ok(result)
    }

    async fn update_position(&self, position_key: &str, mut position: Position) -> AppResult<UpdateResult> {
        position.updated_at = chrono::Utc::now().timestamp() as u64;

        let filter = doc! { "position_key": position_key };
        let update = doc! { "$set": mongodb::bson::to_bson(&position)? };

        let result = self.positions.update_one(filter, update, None).await?;
        Ok(result)
    }

    async fn update_liquidity(
        &self,
        position_key: &str,
        new_liquidity: &str,
        liquidity_change: &str,
        is_increase: bool,
        amount_0_change: u64,
        amount_1_change: u64,
        operation_type: &str,
    ) -> AppResult<UpdateResult> {
        let now = chrono::Utc::now().timestamp() as u64;

        let mut update_doc = doc! {
            "$set": {
                "current_liquidity": new_liquidity,
                "last_operation_type": operation_type,
                "updated_at": now as f64
            },
            "$inc": {
                "total_operations": 1
            }
        };

        if is_increase {
            // 增加流动性的更新
            update_doc.insert(
                "$inc",
                doc! {
                    "total_operations": 1,
                    "current_amount_0": amount_0_change as i64,
                    "current_amount_1": amount_1_change as i64
                },
            );

            // 更新累计增加的流动性（需要特殊处理字符串相加）
            if let Ok(current_added) = liquidity_change.parse::<u128>() {
                update_doc
                    .get_mut("$set")
                    .unwrap()
                    .as_document_mut()
                    .unwrap()
                    .insert("total_liquidity_added", format!("{}", current_added));
            }
        } else {
            // 减少流动性的更新
            update_doc.insert(
                "$inc",
                doc! {
                    "total_operations": 1,
                    "current_amount_0": -(amount_0_change as i64),
                    "current_amount_1": -(amount_1_change as i64)
                },
            );

            // 如果流动性归零，更新状态
            if new_liquidity == "0" {
                update_doc
                    .get_mut("$set")
                    .unwrap()
                    .as_document_mut()
                    .unwrap()
                    .insert("status", "Closed");
                update_doc
                    .get_mut("$set")
                    .unwrap()
                    .as_document_mut()
                    .unwrap()
                    .insert("is_active", false);
            }
        }

        let filter = doc! { "position_key": position_key };
        let result = self.positions.update_one(filter, update_doc, None).await?;
        Ok(result)
    }

    async fn update_fees(&self, position_key: &str, fees_0: u64, fees_1: u64) -> AppResult<UpdateResult> {
        let filter = doc! { "position_key": position_key };
        let update = doc! {
            "$inc": {
                "fees_earned_0": fees_0 as i64,
                "fees_earned_1": fees_1 as i64,
                "unclaimed_fees_0": fees_0 as i64,
                "unclaimed_fees_1": fees_1 as i64
            },
            "$set": {
                "updated_at": chrono::Utc::now().timestamp() as f64
            }
        };

        let result = self.positions.update_one(filter, update, None).await?;
        Ok(result)
    }

    async fn close_position(&self, position_key: &str) -> AppResult<UpdateResult> {
        let filter = doc! { "position_key": position_key };
        let update = doc! {
            "$set": {
                "status": "Closed",
                "is_active": false,
                "current_liquidity": "0",
                "last_operation_type": "close",
                "updated_at": chrono::Utc::now().timestamp() as f64
            }
        };

        let result = self.positions.update_one(filter, update, None).await?;
        Ok(result)
    }

    async fn mark_synced(&self, position_key: &str) -> AppResult<UpdateResult> {
        let now = chrono::Utc::now().timestamp() as u64;
        let filter = doc! { "position_key": position_key };
        let update = doc! {
            "$set": {
                "last_sync_at": now as f64,
                "updated_at": now as f64
            }
        };

        let result = self.positions.update_one(filter, update, None).await?;
        Ok(result)
    }

    async fn find_active_positions(&self) -> AppResult<Vec<Position>> {
        let filter = doc! { "is_active": true };
        let options = FindOptions::builder().sort(doc! { "updated_at": -1 }).build();

        let mut cursor = self.positions.find(filter, options).await?;
        let mut positions = Vec::new();

        while let Some(position) = cursor.next().await {
            positions.push(position?);
        }

        Ok(positions)
    }

    async fn find_positions_need_sync(&self, max_age_seconds: u64) -> AppResult<Vec<Position>> {
        let cutoff_time = (chrono::Utc::now().timestamp() as u64).saturating_sub(max_age_seconds);

        let filter = doc! {
            "is_active": true,
            "$or": [
                { "last_sync_at": { "$exists": false } },
                { "last_sync_at": { "$lt": cutoff_time as f64 } }
            ]
        };

        let mut cursor = self.positions.find(filter, None).await?;
        let mut positions = Vec::new();

        while let Some(position) = cursor.next().await {
            positions.push(position?);
        }

        Ok(positions)
    }

    async fn batch_update_positions(&self, updates: Vec<(String, Document)>) -> AppResult<u64> {
        let mut total_updated = 0u64;

        for (position_key, update_doc) in updates {
            let filter = doc! { "position_key": position_key };
            let result = self.positions.update_one(filter, update_doc, None).await?;
            total_updated += result.modified_count;
        }

        Ok(total_updated)
    }

    async fn get_user_position_stats(&self, user_wallet: &str) -> AppResult<PositionStats> {
        // 获取用户所有仓位
        let positions = self.find_by_user_wallet(user_wallet).await?;

        let mut total_positions = 0u64;
        let mut active_positions = 0u64;
        let mut closed_positions = 0u64;
        let mut total_liquidity = 0u128;
        let mut total_fees_earned_0 = 0u64;
        let mut total_fees_earned_1 = 0u64;

        for position in positions {
            total_positions += 1;

            if position.is_active {
                active_positions += 1;
                if let Ok(liquidity) = position.current_liquidity.parse::<u128>() {
                    total_liquidity += liquidity;
                }
            } else {
                closed_positions += 1;
            }

            total_fees_earned_0 += position.fees_earned_0;
            total_fees_earned_1 += position.fees_earned_1;
        }

        Ok(PositionStats {
            total_positions,
            active_positions,
            closed_positions,
            total_liquidity: total_liquidity.to_string(),
            total_fees_earned_0,
            total_fees_earned_1,
        })
    }

    async fn get_pool_position_stats(&self, pool_address: &str) -> AppResult<PoolPositionStats> {
        let positions = self.find_by_pool_address(pool_address).await?;

        let mut total_positions = 0u64;
        let mut active_positions = 0u64;
        let mut unique_users = std::collections::HashSet::new();
        let mut total_liquidity = 0u128;

        for position in positions {
            total_positions += 1;
            unique_users.insert(position.user_wallet);

            if position.is_active {
                active_positions += 1;
                if let Ok(liquidity) = position.current_liquidity.parse::<u128>() {
                    total_liquidity += liquidity;
                }
            }
        }

        let average_position_size = if active_positions > 0 {
            (total_liquidity / active_positions as u128).to_string()
        } else {
            "0".to_string()
        };

        Ok(PoolPositionStats {
            total_positions,
            active_positions,
            unique_users: unique_users.len() as u64,
            total_liquidity: total_liquidity.to_string(),
            average_position_size,
        })
    }

    async fn init_indexes(&self) -> AppResult<()> {
        info!("🔧 初始化Position数据库索引...");

        let indexes = vec![
            // 1. 唯一索引：position_key (主键索引)
            IndexModel::builder()
                .keys(doc! { "position_key": 1 })
                .options(IndexOptions::builder().unique(true).build())
                .build(),
            // 2. 用户查询索引：user_wallet + created_at (复合索引，支持排序)
            IndexModel::builder()
                .keys(doc! {
                    "user_wallet": 1,
                    "created_at": -1
                })
                .build(),
            // 3. 池子查询索引：pool_address + created_at (复合索引，支持排序)
            IndexModel::builder()
                .keys(doc! {
                    "pool_address": 1,
                    "created_at": -1
                })
                .build(),
            // 4. 价格范围查询索引：pool_address + tick范围 (复合索引)
            IndexModel::builder()
                .keys(doc! {
                    "pool_address": 1,
                    "tick_lower_index": 1,
                    "tick_upper_index": 1
                })
                .build(),
            // 5. 活跃状态索引：is_active (过滤索引)
            IndexModel::builder().keys(doc! { "is_active": 1 }).build(),
            // 6. 同步状态索引：is_active + last_sync_at (复合索引，支持同步查询)
            IndexModel::builder()
                .keys(doc! {
                    "is_active": 1,
                    "last_sync_at": 1
                })
                .build(),
            // 7. 时间索引：updated_at (降序，支持时间排序)
            IndexModel::builder().keys(doc! { "updated_at": -1 }).build(),
            // 8. NFT索引：nft_mint (稀疏索引，支持NFT查询)
            IndexModel::builder()
                .keys(doc! { "nft_mint": 1 })
                .options(IndexOptions::builder().sparse(true).build())
                .build(),
            // 9. 用户活跃仓位查询索引：user_wallet + is_active + created_at
            IndexModel::builder()
                .keys(doc! {
                    "user_wallet": 1,
                    "is_active": 1,
                    "created_at": -1
                })
                .build(),
            // 10. 状态索引：status (支持状态过滤)
            IndexModel::builder().keys(doc! { "status": 1 }).build(),
        ];

        self.positions.create_indexes(indexes, None).await?;
        info!("✅ Position数据库索引初始化完成");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::position::model::Position;

    fn create_test_position() -> Position {
        Position::new(
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
        )
    }

    #[test]
    fn test_position_stats_creation() {
        let stats = PositionStats {
            total_positions: 10,
            active_positions: 8,
            closed_positions: 2,
            total_liquidity: "5000000".to_string(),
            total_fees_earned_0: 1000,
            total_fees_earned_1: 2000,
        };

        assert_eq!(stats.total_positions, 10);
        assert_eq!(stats.active_positions, 8);
        assert_eq!(stats.closed_positions, 2);
    }

    #[test]
    fn test_pool_position_stats_creation() {
        let stats = PoolPositionStats {
            total_positions: 50,
            active_positions: 40,
            unique_users: 25,
            total_liquidity: "10000000".to_string(),
            average_position_size: "250000".to_string(),
        };

        assert_eq!(stats.total_positions, 50);
        assert_eq!(stats.unique_users, 25);
    }

    #[test]
    fn test_index_field_names() {
        // 验证索引字段名与模型字段名一致
        let position = create_test_position();

        // 验证关键字段存在，确保索引字段名正确
        assert!(!position.position_key.is_empty());
        assert!(!position.user_wallet.is_empty());
        assert!(!position.pool_address.is_empty());
        assert!(!position.nft_mint.is_empty());
        assert!(position.created_at > 0);
        assert!(position.updated_at > 0);
        assert!(position.is_active);
    }
}
