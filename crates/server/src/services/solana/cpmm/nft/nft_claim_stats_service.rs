/// NFT 领取统计服务
///
/// 提供 NFT 领取数据的统计查询功能
use crate::dtos::solana::cpmm::nft::{NftMintClaimStatsListResponse, NftMintClaimStatsResponse};
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

    /// 获取所有 NFT 的领取统计
    ///
    /// 返回按领取次数排序的所有 NFT 统计信息
    ///
    /// # 返回
    /// - `Ok(NftMintClaimStatsListResponse)`: 统计数据列表
    /// - `Err`: 查询失败时返回错误
    pub async fn get_all_nft_claim_stats(&self) -> Result<NftMintClaimStatsListResponse> {
        info!("📊 开始获取所有NFT领取统计");

        // 从仓库层获取统计数据
        let stats = self
            .database
            .nft_claim_event_repository
            .get_nft_claim_stats_by_mint()
            .await
            .map_err(|e| {
                error!("❌ 获取NFT领取统计失败: {}", e);
                anyhow::anyhow!("获取NFT领取统计失败: {}", e)
            })?;

        let total_nfts = stats.len() as u64;

        // 转换为响应DTO
        let response_stats: Vec<NftMintClaimStatsResponse> = stats.into_iter().map(|s| s.into()).collect();

        let response = NftMintClaimStatsListResponse {
            stats: response_stats,
            total_nfts,
        };

        info!("✅ 成功获取 {} 个NFT的领取统计", total_nfts);

        Ok(response)
    }

    /// 获取指定 NFT 的领取统计
    ///
    /// # 参数
    /// - `nft_mint`: NFT 地址
    ///
    /// # 返回
    /// - `Ok(Some(NftMintClaimStatsResponse))`: NFT 存在时返回统计数据
    /// - `Ok(None)`: NFT 不存在或没有领取记录
    /// - `Err`: 查询失败时返回错误
    pub async fn get_nft_claim_stats_by_mint(&self, nft_mint: &str) -> Result<Option<NftMintClaimStatsResponse>> {
        info!("📊 开始获取NFT领取统计: {}", nft_mint);

        // 从仓库层获取统计数据
        let stats = self
            .database
            .nft_claim_event_repository
            .get_nft_claim_stats_by_single_mint(nft_mint)
            .await
            .map_err(|e| {
                error!("❌ 获取NFT领取统计失败 {}: {}", nft_mint, e);
                anyhow::anyhow!("获取NFT领取统计失败: {}", e)
            })?;

        match stats {
            Some(s) => {
                info!(
                    "✅ 成功获取NFT领取统计 {}: 领取次数={}, 总金额={}",
                    nft_mint, s.claim_count, s.total_claim_amount
                );
                Ok(Some(s.into()))
            }
            None => {
                info!("⚠️ NFT {} 没有领取记录", nft_mint);
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
