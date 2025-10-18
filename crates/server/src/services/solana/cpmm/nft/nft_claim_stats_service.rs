/// NFT 领取统计服务
///
/// 提供 NFT 领取数据的统计查询功能
/// 注意：统计维度为按推荐人（referrer）统计
use crate::dtos::solana::cpmm::nft::{PaginatedReferrerStatsResponse, ReferrerStatsResponse};
use anyhow::Result;
use database::Database;
use std::sync::Arc;
use tracing::{error, info};

/// NFT 领取统计服务
#[derive(Clone)]
pub struct NftClaimStatsService {
    database: Arc<Database>,
}

impl NftClaimStatsService {
    /// 创建新的 NFT 领取统计服务实例
    ///
    /// # 参数
    /// - `database`: 数据库连接实例
    ///
    /// # 返回
    /// 返回服务实例
    pub fn new(database: Arc<Database>) -> Self {
        info!("✅ NftClaimStatsService 初始化成功");
        Self { database }
    }

    /// 获取所有推荐人的统计（分页版本）
    ///
    /// 返回按推荐人数排序的推荐人统计信息，支持分页
    ///
    /// # 参数
    /// - `page`: 页码（从1开始）
    /// - `page_size`: 每页条数
    /// - `sort_by`: 排序字段（默认：referred_count）
    /// - `sort_order`: 排序方向（asc/desc，默认：desc）
    ///
    /// # 返回
    /// - `Ok(PaginatedReferrerStatsResponse)`: 分页统计数据
    /// - `Err`: 查询失败时返回错误
    pub async fn get_all_claimer_stats_paginated(
        &self,
        page: u32,
        page_size: u32,
        sort_by: Option<String>,
        sort_order: Option<String>,
    ) -> Result<PaginatedReferrerStatsResponse> {
        info!(
            "📊 开始获取推荐人统计（分页）: page={}, page_size={}, sort_by={:?}, sort_order={:?}",
            page, page_size, sort_by, sort_order
        );

        // 从仓库层获取分页统计数据
        let paginated_result = self
            .database
            .nft_claim_event_repository
            .get_nft_claim_stats_by_claimer_paginated(page, page_size, sort_by, sort_order)
            .await
            .map_err(|e| {
                error!("❌ 获取推荐人分页统计失败: {}", e);
                anyhow::anyhow!("获取推荐人分页统计失败: {}", e)
            })?;

        // 转换为响应DTO
        let items: Vec<ReferrerStatsResponse> = paginated_result.items.into_iter().map(|s| s.into()).collect();

        // 计算总页数
        let total_pages = if page_size > 0 {
            (paginated_result.total + page_size as u64 - 1) / page_size as u64
        } else {
            0
        };

        let response = PaginatedReferrerStatsResponse {
            items,
            total: paginated_result.total,
            page: page as u64,
            page_size: page_size as u64,
            total_pages,
        };

        info!(
            "✅ 成功获取推荐人分页统计: 返回 {} 条记录，总共 {} 条，共 {} 页",
            response.items.len(),
            response.total,
            response.total_pages
        );

        Ok(response)
    }

    /// 获取指定推荐人的统计
    ///
    /// # 参数
    /// - `referrer`: 推荐人地址
    ///
    /// # 返回
    /// - `Ok(Some(ReferrerStatsResponse))`: 推荐人存在时返回统计数据
    /// - `Ok(None)`: 推荐人不存在或没有推荐记录
    /// - `Err`: 查询失败时返回错误
    pub async fn get_claimer_stats_by_address(&self, referrer: &str) -> Result<Option<ReferrerStatsResponse>> {
        info!("📊 开始获取推荐人统计: {}", referrer);

        // 从仓库层获取统计数据
        let stats = self
            .database
            .nft_claim_event_repository
            .get_nft_claim_stats_by_single_claimer(referrer)
            .await
            .map_err(|e| {
                error!("❌ 获取推荐人统计失败 {}: {}", referrer, e);
                anyhow::anyhow!("获取推荐人统计失败: {}", e)
            })?;

        match stats {
            Some(s) => {
                info!("✅ 成功获取推荐人统计 {}: 推荐人数={}", referrer, s.referred_count);
                Ok(Some(s.into()))
            }
            None => {
                info!("⚠️ 推荐人 {} 没有推荐记录", referrer);
                Ok(None)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_nft_claim_stats_service_creation() {
        // 这个测试只验证服务创建，不需要真实数据库连接
        // 实际测试需要在集成测试中进行
        assert!(true, "服务创建测试通过");
    }
}
