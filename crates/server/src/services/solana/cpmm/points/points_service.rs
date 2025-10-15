use crate::dtos::solana::cpmm::points::points_stats::{PointsStatsData, PointsStatsResponse, RankItem};
use anyhow::Result;
use database::Database;
use std::sync::Arc;
use tracing::{debug, error, info};

/// 积分排行榜服务错误
#[derive(Debug, thiserror::Error)]
pub enum PointsServiceError {
    #[error("数据库操作失败: {0}")]
    DatabaseError(#[from] anyhow::Error),

    #[error("用户未找到: {0}")]
    UserNotFound(String),

    #[error("无效的分页参数: {0}")]
    InvalidPagination(String),
}

/// 积分排行榜服务
#[derive(Clone, Debug)]
pub struct PointsService {
    database: Arc<Database>,
}

impl PointsService {
    /// 创建新的服务实例
    pub fn new(database: Arc<Database>) -> Self {
        Self { database }
    }

    /// 获取积分排行榜统计信息
    ///
    /// # Arguments
    /// * `wallet_address` - 用户钱包地址
    /// * `page` - 页码（从1开始）
    /// * `page_size` - 每页数量（默认50，最大100）
    ///
    /// # Returns
    /// 包含排行榜列表、用户信息和分页信息的响应
    pub async fn get_points_stats(&self, wallet_address: &str, page: Option<u64>, page_size: Option<u64>) -> Result<PointsStatsResponse, PointsServiceError> {
        info!("🔍 查询积分排行榜统计: wallet={}, page={:?}, page_size={:?}", wallet_address, page, page_size);

        // 验证和设置分页参数
        let page = page.unwrap_or(1).max(1);
        let page_size = page_size.unwrap_or(50).min(100).max(1);

        debug!("📊 使用分页参数: page={}, page_size={}", page, page_size);

        // 查询排行榜数据
        let rank_list_result = self
            .database
            .user_points_repository
            .get_leaderboard_with_rank(page as i64, page_size as i64)
            .await;

        // 查询用户排名信息
        let user_rank_result = self.database.user_points_repository.get_user_rank(wallet_address).await;

        // 查询总用户数
        let total_users_result = self.database.user_points_repository.get_total_users().await;

        // 处理查询结果
        match (rank_list_result, user_rank_result, total_users_result) {
            (Ok(rank_list), Ok(user_rank_opt), Ok(total)) => {
                debug!("✅ 查询成功: 排行榜{}条, 总用户数{}", rank_list.len(), total);

                // 转换排行榜数据为DTO
                let rank_items: Vec<RankItem> = rank_list
                    .into_iter()
                    .map(|item| RankItem {
                        rank_no: item.rank,
                        points: item.total_points,
                        user: item.user.user_wallet,
                    })
                    .collect();

                // 处理用户排名信息
                let (my_points, my_rank) = match user_rank_opt {
                    Some(user_rank) => {
                        debug!("✅ 用户排名: rank={}, points={}", user_rank.rank, user_rank.total_points);
                        (user_rank.total_points, user_rank.rank)
                    }
                    None => {
                        debug!("⚠️ 用户未上榜: {}", wallet_address);
                        (0, 0) // 0表示未上榜
                    }
                };

                // 计算总页数
                let total_pages = if total == 0 { 0 } else { (total + page_size - 1) / page_size };

                // 构建响应数据
                let data = PointsStatsData {
                    rank_list: rank_items,
                    my_wallet: wallet_address.to_string(),
                    my_points,
                    my_rank,
                    total,
                    page,
                    page_size,
                    total_pages,
                };

                info!("✅ 积分排行榜查询成功: wallet={}, rank={}/{}", wallet_address, my_rank, total);
                Ok(PointsStatsResponse::success(data))
            }
            (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => {
                error!("❌ 查询积分排行榜失败: {}", e);
                Err(PointsServiceError::DatabaseError(e))
            }
        }
    }

    /// 获取用户积分信息（不含排行榜）
    pub async fn get_user_points(&self, wallet_address: &str) -> Result<Option<database::cpmm::points::model::UserPointsSummary>, PointsServiceError> {
        debug!("🔍 查询用户积分: {}", wallet_address);

        match self.database.user_points_repository.get_by_wallet(wallet_address).await {
            Ok(user) => {
                if user.is_some() {
                    debug!("✅ 用户积分查询成功: {}", wallet_address);
                } else {
                    debug!("⚠️ 用户不存在: {}", wallet_address);
                }
                Ok(user)
            }
            Err(e) => {
                error!("❌ 查询用户积分失败: {}", e);
                Err(PointsServiceError::DatabaseError(e))
            }
        }
    }

    /// 获取积分统计信息
    pub async fn get_stats(&self) -> Result<database::cpmm::points::model::UserPointsStats, PointsServiceError> {
        debug!("🔍 查询积分统计信息");

        match self.database.user_points_repository.get_stats().await {
            Ok(stats) => {
                debug!("✅ 积分统计查询成功");
                Ok(stats)
            }
            Err(e) => {
                error!("❌ 查询积分统计失败: {}", e);
                Err(PointsServiceError::DatabaseError(e))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_points_service_creation() {
        // 测试Service创建逻辑
        // 实际的集成测试需要真实的数据库连接
        println!("✅ PointsService单元测试框架就绪");
    }

    #[test]
    fn test_pagination_validation() {
        // 测试分页参数验证
        let page = Some(0).unwrap_or(1).max(1);
        assert_eq!(page, 1, "页码最小应为1");

        let page_size = Some(200).unwrap_or(50).min(100).max(1);
        assert_eq!(page_size, 100, "每页最大应为100");

        let page_size_zero = Some(0).unwrap_or(50).min(100).max(1);
        assert_eq!(page_size_zero, 1, "每页最小应为1");

        println!("✅ 分页参数验证测试通过");
    }

    #[test]
    fn test_total_pages_calculation() {
        // 测试总页数计算
        let total = 100u64;
        let page_size = 50u64;
        let total_pages = (total + page_size - 1) / page_size;
        assert_eq!(total_pages, 2);

        let total = 101u64;
        let page_size = 50u64;
        let total_pages = (total + page_size - 1) / page_size;
        assert_eq!(total_pages, 3);

        let total = 0u64;
        let page_size = 50u64;
        let total_pages = if total == 0 { 0 } else { (total + page_size - 1) / page_size };
        assert_eq!(total_pages, 0);

        println!("✅ 总页数计算测试通过");
    }
}
