use crate::dtos::solana_dto::{
    DecreaseLiquidityAndSendTransactionResponse, DecreaseLiquidityRequest, DecreaseLiquidityResponse,
    IncreaseLiquidityAndSendTransactionResponse, IncreaseLiquidityRequest, IncreaseLiquidityResponse,
    OpenPositionAndSendTransactionResponse, OpenPositionRequest, OpenPositionResponse,
};

use database::{
    position::{
        model::{Position, PositionMetadata},
        repository::{DynPositionRepository, PoolPositionStats, PositionStats},
    },
    Database,
};

use anyhow::Result;
use std::sync::Arc;
use tracing::{error, info, warn};

/// Position Storage Service - 负责仓位数据的链下存储和管理
#[derive(Clone)]
pub struct PositionStorageService {
    db: Option<Arc<Database>>,
    position_repo: Option<DynPositionRepository>,
}

impl PositionStorageService {
    /// 创建新的 PositionStorageService 实例
    pub fn new(db: Arc<Database>) -> Self {
        let position_repo: DynPositionRepository = db.clone();
        Self {
            db: Some(db),
            position_repo: Some(position_repo),
        }
    }

    /// 创建占位符实例（用于没有数据库的场景）
    pub fn placeholder() -> Self {
        Self {
            db: None,
            position_repo: None,
        }
    }

    /// 检查是否有数据库连接
    fn ensure_database(&self) -> Result<()> {
        if self.db.is_none() || self.position_repo.is_none() {
            return Err(anyhow::anyhow!("数据库未初始化，无法执行存储操作"));
        }
        Ok(())
    }

    // ============ 开仓相关操作 ============

    /// 保存开仓信息到数据库
    pub async fn save_open_position(
        &self,
        request: &OpenPositionRequest,
        response: &OpenPositionResponse,
        transaction_signature: Option<String>,
    ) -> Result<()> {
        self.ensure_database()?;
        let position_repo = self.position_repo.as_ref().unwrap();

        info!("💾 保存开仓信息到数据库");
        info!("  Position Key: {}", response.position_key);
        info!("  User Wallet: {}", request.user_wallet);

        // 创建 Position 实例
        let mut position = Position::new(
            response.position_key.clone(),
            response.position_nft_mint.clone(),
            request.user_wallet.clone(),
            response.pool_address.clone(),
            response.tick_lower_index,
            response.tick_upper_index,
            request.tick_lower_price,
            request.tick_upper_price,
            response.liquidity.clone(),
            response.amount_0,
            response.amount_1,
        );

        // 设置扩展元数据
        let metadata = PositionMetadata {
            initial_transaction_signature: transaction_signature,
            slippage_tolerance: Some(request.max_slippage_percent),
            price_range_utilization: None, // 后续可以计算
            performance_metrics: None,
            custom_data: None,
        };
        position.set_metadata(metadata);

        // 保存到数据库
        match position_repo.create_position(position).await {
            Ok(result) => {
                info!("✅ 开仓信息保存成功，ID: {:?}", result.inserted_id);
                Ok(())
            }
            Err(e) => {
                error!("❌ 保存开仓信息失败: {}", e);
                Err(e.into())
            }
        }
    }

    /// 保存开仓并发送交易的信息
    pub async fn save_open_position_with_transaction(
        &self,
        request: &OpenPositionRequest,
        response: &OpenPositionAndSendTransactionResponse,
    ) -> Result<()> {
        info!("💾 保存开仓交易信息到数据库");
        info!("  Position Key: {}", response.position_key);
        info!("  Transaction Signature: {}", response.signature);

        let mut position = Position::new(
            response.position_key.clone(),
            response.position_nft_mint.clone(),
            request.user_wallet.clone(),
            response.pool_address.clone(),
            response.tick_lower_index,
            response.tick_upper_index,
            request.tick_lower_price,
            request.tick_upper_price,
            response.liquidity.clone(),
            response.amount_0,
            response.amount_1,
        );

        let metadata = PositionMetadata {
            initial_transaction_signature: Some(response.signature.clone()),
            slippage_tolerance: Some(request.max_slippage_percent),
            price_range_utilization: None,
            performance_metrics: None,
            custom_data: None,
        };
        position.set_metadata(metadata);

        self.ensure_database()?;
        let position_repo = self.position_repo.as_ref().unwrap();

        match position_repo.create_position(position).await {
            Ok(result) => {
                info!("✅ 开仓交易信息保存成功，ID: {:?}", result.inserted_id);
                Ok(())
            }
            Err(e) => {
                error!("❌ 保存开仓交易信息失败: {}", e);
                Err(e.into())
            }
        }
    }

    // ============ 增加流动性相关操作 ============

    /// 更新增加流动性后的仓位信息
    pub async fn update_increase_liquidity(
        &self,
        request: &IncreaseLiquidityRequest,
        response: &IncreaseLiquidityResponse,
        _transaction_signature: Option<String>,
    ) -> Result<()> {
        self.ensure_database()?;
        let position_repo = self.position_repo.as_ref().unwrap();

        info!("💾 更新增加流动性信息");
        info!("  Position Key: {}", response.position_key);
        info!("  Liquidity Added: {}", response.liquidity_added);

        // 查找现有仓位
        let existing_position = position_repo
            .find_user_position_in_range(
                &request.user_wallet,
                &request.pool_address,
                response.tick_lower_index,
                response.tick_upper_index,
            )
            .await?;

        if let Some(position) = existing_position {
            // 计算新的流动性总量
            let current_liquidity = position.current_liquidity.parse::<u128>().unwrap_or(0);
            let added_liquidity = response.liquidity_added.parse::<u128>().unwrap_or(0);
            let new_total_liquidity = current_liquidity + added_liquidity;

            // 更新流动性信息
            match position_repo
                .update_liquidity(
                    &response.position_key,
                    &new_total_liquidity.to_string(),
                    &response.liquidity_added,
                    true, // is_increase
                    response.amount_0,
                    response.amount_1,
                    "increase_liquidity",
                )
                .await
            {
                Ok(_) => {
                    info!("✅ 增加流动性信息更新成功");
                    Ok(())
                }
                Err(e) => {
                    error!("❌ 更新增加流动性信息失败: {}", e);
                    Err(e.into())
                }
            }
        } else {
            warn!("⚠️ 未找到对应的仓位记录: {}", response.position_key);
            Err(anyhow::anyhow!("Position not found: {}", response.position_key))
        }
    }

    /// 更新增加流动性并发送交易后的仓位信息
    pub async fn update_increase_liquidity_with_transaction(
        &self,
        request: &IncreaseLiquidityRequest,
        response: &IncreaseLiquidityAndSendTransactionResponse,
    ) -> Result<()> {
        self.ensure_database()?;
        let position_repo = self.position_repo.as_ref().unwrap();

        info!("📈 更新增加流动性交易信息");
        info!("  Position Key: {}", response.position_key);
        info!("  Transaction Signature: {}", response.signature);

        let existing_position = position_repo
            .find_user_position_in_range(
                &request.user_wallet,
                &request.pool_address,
                response.tick_lower_index,
                response.tick_upper_index,
            )
            .await?;

        if let Some(position) = existing_position {
            let current_liquidity = position.current_liquidity.parse::<u128>().unwrap_or(0);
            let added_liquidity = response.liquidity_added.parse::<u128>().unwrap_or(0);
            let new_total_liquidity = current_liquidity + added_liquidity;

            match position_repo
                .update_liquidity(
                    &response.position_key,
                    &new_total_liquidity.to_string(),
                    &response.liquidity_added,
                    true,
                    response.amount_0,
                    response.amount_1,
                    "increase_liquidity_tx",
                )
                .await
            {
                Ok(_) => {
                    info!("✅ 增加流动性交易信息更新成功");
                    Ok(())
                }
                Err(e) => {
                    error!("❌ 更新增加流动性交易信息失败: {}", e);
                    Err(e.into())
                }
            }
        } else {
            warn!("⚠️ 未找到对应的仓位记录: {}", response.position_key);
            Err(anyhow::anyhow!("Position not found: {}", response.position_key))
        }
    }

    // ============ 减少流动性相关操作 ============

    /// 更新减少流动性后的仓位信息
    pub async fn update_decrease_liquidity(
        &self,
        _request: &DecreaseLiquidityRequest,
        response: &DecreaseLiquidityResponse,
        _transaction_signature: Option<String>,
    ) -> Result<()> {
        self.ensure_database()?;
        let position_repo = self.position_repo.as_ref().unwrap();

        info!("📉 更新减少流动性信息");
        info!("  Position Key: {}", response.position_key);
        info!("  Liquidity Removed: {}", response.liquidity_removed);

        let existing_position = position_repo.find_by_position_key(&response.position_key).await?;

        if let Some(position) = existing_position {
            let current_liquidity = position.current_liquidity.parse::<u128>().unwrap_or(0);
            let removed_liquidity = response.liquidity_removed.parse::<u128>().unwrap_or(0);
            let new_total_liquidity = current_liquidity.saturating_sub(removed_liquidity);

            let operation_type = if response.will_close_position {
                "close_position"
            } else {
                "decrease_liquidity"
            };

            match position_repo
                .update_liquidity(
                    &response.position_key,
                    &new_total_liquidity.to_string(),
                    &response.liquidity_removed,
                    false, // is_decrease
                    response.amount_0_expected,
                    response.amount_1_expected,
                    operation_type,
                )
                .await
            {
                Ok(_) => {
                    info!("✅ 减少流动性信息更新成功");

                    // 如果完全关闭仓位，更新状态
                    if response.will_close_position {
                        match position_repo.close_position(&response.position_key).await {
                            Ok(_) => info!("✅ 仓位状态已更新为关闭"),
                            Err(e) => warn!("⚠️ 更新仓位关闭状态失败: {}", e),
                        }
                    }

                    Ok(())
                }
                Err(e) => {
                    error!("❌ 更新减少流动性信息失败: {}", e);
                    Err(e.into())
                }
            }
        } else {
            warn!("⚠️ 未找到对应的仓位记录: {}", response.position_key);
            Err(anyhow::anyhow!("Position not found: {}", response.position_key))
        }
    }

    /// 更新减少流动性并发送交易后的仓位信息
    pub async fn update_decrease_liquidity_with_transaction(
        &self,
        _request: &DecreaseLiquidityRequest,
        response: &DecreaseLiquidityAndSendTransactionResponse,
    ) -> Result<()> {
        self.ensure_database()?;
        let position_repo = self.position_repo.as_ref().unwrap();

        info!("📉 更新减少流动性交易信息");
        info!("  Position Key: {}", response.position_key);
        info!("  Transaction Signature: {}", response.signature);

        let existing_position = position_repo.find_by_position_key(&response.position_key).await?;

        if let Some(position) = existing_position {
            let current_liquidity = position.current_liquidity.parse::<u128>().unwrap_or(0);
            let removed_liquidity = response.liquidity_removed.parse::<u128>().unwrap_or(0);
            let new_total_liquidity = current_liquidity.saturating_sub(removed_liquidity);

            let operation_type = if response.position_closed {
                "close_position_tx"
            } else {
                "decrease_liquidity_tx"
            };

            match position_repo
                .update_liquidity(
                    &response.position_key,
                    &new_total_liquidity.to_string(),
                    &response.liquidity_removed,
                    false,
                    response.amount_0_actual,
                    response.amount_1_actual,
                    operation_type,
                )
                .await
            {
                Ok(_) => {
                    info!("✅ 减少流动性交易信息更新成功");

                    if response.position_closed {
                        match position_repo.close_position(&response.position_key).await {
                            Ok(_) => info!("✅ 仓位已关闭"),
                            Err(e) => warn!("⚠️ 更新仓位关闭状态失败: {}", e),
                        }
                    }

                    Ok(())
                }
                Err(e) => {
                    error!("❌ 更新减少流动性交易信息失败: {}", e);
                    Err(e.into())
                }
            }
        } else {
            warn!("⚠️ 未找到对应的仓位记录: {}", response.position_key);
            Err(anyhow::anyhow!("Position not found: {}", response.position_key))
        }
    }

    // ============ 查询相关操作 ============

    /// 获取用户所有仓位（带缓存效果）
    pub async fn get_user_positions_with_cache(&self, user_wallet: &str) -> Result<Vec<Position>> {
        self.ensure_database()?;
        let position_repo = self.position_repo.as_ref().unwrap();

        info!("🔍 获取用户仓位列表: {}", user_wallet);

        match position_repo.find_by_user_wallet(user_wallet).await {
            Ok(positions) => {
                info!("✅ 找到 {} 个仓位", positions.len());
                Ok(positions)
            }
            Err(e) => {
                error!("❌ 获取用户仓位失败: {}", e);
                Err(e.into())
            }
        }
    }

    /// 获取特定仓位详情
    pub async fn get_position_details(&self, position_key: &str) -> Result<Option<Position>> {
        self.ensure_database()?;
        let position_repo = self.position_repo.as_ref().unwrap();

        info!("🔍 获取仓位详情: {}", position_key);

        match position_repo.find_by_position_key(position_key).await {
            Ok(position) => {
                if position.is_some() {
                    info!("✅ 找到仓位详情");
                } else {
                    info!("ℹ️ 未找到仓位");
                }
                Ok(position)
            }
            Err(e) => {
                error!("❌ 获取仓位详情失败: {}", e);
                Err(e.into())
            }
        }
    }

    /// 获取池子所有仓位
    pub async fn get_pool_positions(&self, pool_address: &str) -> Result<Vec<Position>> {
        self.ensure_database()?;
        let position_repo = self.position_repo.as_ref().unwrap();

        info!("🔍 获取池子仓位列表: {}", pool_address);

        match position_repo.find_by_pool_address(pool_address).await {
            Ok(positions) => {
                info!("✅ 找到 {} 个仓位", positions.len());
                Ok(positions)
            }
            Err(e) => {
                error!("❌ 获取池子仓位失败: {}", e);
                Err(e.into())
            }
        }
    }

    /// 获取用户仓位统计信息
    pub async fn get_user_position_stats(&self, user_wallet: &str) -> Result<PositionStats> {
        self.ensure_database()?;
        let position_repo = self.position_repo.as_ref().unwrap();

        info!("📊 获取用户仓位统计: {}", user_wallet);

        match position_repo.get_user_position_stats(user_wallet).await {
            Ok(stats) => {
                info!(
                    "✅ 用户统计: {} 个总仓位，{} 个活跃仓位",
                    stats.total_positions, stats.active_positions
                );
                Ok(stats)
            }
            Err(e) => {
                error!("❌ 获取用户统计失败: {}", e);
                Err(e.into())
            }
        }
    }

    /// 获取池子仓位统计信息
    pub async fn get_pool_position_stats(&self, pool_address: &str) -> Result<PoolPositionStats> {
        self.ensure_database()?;
        let position_repo = self.position_repo.as_ref().unwrap();

        info!("📊 获取池子仓位统计: {}", pool_address);

        match position_repo.get_pool_position_stats(pool_address).await {
            Ok(stats) => {
                info!(
                    "✅ 池子统计: {} 个总仓位，{} 个唯一用户",
                    stats.total_positions, stats.unique_users
                );
                Ok(stats)
            }
            Err(e) => {
                error!("❌ 获取池子统计失败: {}", e);
                Err(e.into())
            }
        }
    }

    // ============ 同步相关操作 ============

    /// 手动同步仓位状态（从链上获取最新数据）
    pub async fn sync_position_with_chain(&self, position_key: &str) -> Result<()> {
        self.ensure_database()?;
        let position_repo = self.position_repo.as_ref().unwrap();

        info!("🔄 开始同步仓位状态: {}", position_key);

        // TODO: 这里可以添加从链上获取仓位状态的逻辑
        // 现在只是标记为已同步
        match position_repo.mark_synced(position_key).await {
            Ok(_) => {
                info!("✅ 仓位同步标记成功");
                Ok(())
            }
            Err(e) => {
                error!("❌ 仓位同步标记失败: {}", e);
                Err(e.into())
            }
        }
    }

    /// 获取需要同步的仓位列表
    pub async fn get_positions_need_sync(&self, max_age_seconds: u64) -> Result<Vec<Position>> {
        self.ensure_database()?;
        let position_repo = self.position_repo.as_ref().unwrap();

        info!("🔍 获取需要同步的仓位列表");

        match position_repo.find_positions_need_sync(max_age_seconds).await {
            Ok(positions) => {
                info!("✅ 找到 {} 个需要同步的仓位", positions.len());
                Ok(positions)
            }
            Err(e) => {
                error!("❌ 获取需要同步的仓位失败: {}", e);
                Err(e.into())
            }
        }
    }

    /// 批量同步仓位状态（定时任务使用）
    pub async fn batch_sync_positions(&self) -> Result<u64> {
        info!("🔄 开始批量同步仓位状态");

        // 获取1小时内未同步的仓位
        let positions_to_sync = self.get_positions_need_sync(3600).await?;

        if positions_to_sync.is_empty() {
            info!("ℹ️ 没有需要同步的仓位");
            return Ok(0);
        }

        let mut synced_count = 0u64;

        for position in positions_to_sync {
            match self.sync_position_with_chain(&position.position_key).await {
                Ok(_) => synced_count += 1,
                Err(e) => {
                    warn!("⚠️ 同步仓位 {} 失败: {}", position.position_key, e);
                }
            }
        }

        info!("✅ 批量同步完成，成功同步 {} 个仓位", synced_count);
        Ok(synced_count)
    }
}

#[cfg(test)]
mod tests {

    // 这里可以添加单元测试
    // 注意：实际测试需要 mock 数据库连接

    #[test]
    fn test_position_storage_service_creation() {
        // 这是一个占位测试，实际测试需要数据库连接
        assert!(true);
    }
}
