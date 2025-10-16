/// NFT 领取统计相关的 DTO
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// NFT Mint 领取统计响应
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct NftMintClaimStatsResponse {
    /// NFT地址
    #[schema(example = "NFTaoszFxtEmGXvHcb8yfkGZxqLPAfwDqLN1mhrV2jM")]
    pub nft_mint: String,

    /// 领取次数
    #[schema(example = 150)]
    pub claim_count: u64,

    /// 总领取金额
    #[schema(example = 15000)]
    pub total_claim_amount: u64,

    /// 最新领取时间（Unix时间戳）
    #[schema(example = 1735203600)]
    pub latest_claim_time: Option<i64>,

    /// 最早领取时间（Unix时间戳）
    #[schema(example = 1704067200)]
    pub earliest_claim_time: Option<i64>,

    /// 独立领取者数量
    #[schema(example = 75)]
    pub unique_claimers_count: u64,
}

/// NFT Mint 领取统计列表响应
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct NftMintClaimStatsListResponse {
    /// 统计数据列表
    pub stats: Vec<NftMintClaimStatsResponse>,

    /// 总NFT数量
    #[schema(example = 10)]
    pub total_nfts: u64,
}

impl From<database::events::event_model::repository::NftMintClaimStats> for NftMintClaimStatsResponse {
    fn from(stats: database::events::event_model::repository::NftMintClaimStats) -> Self {
        Self {
            nft_mint: stats.nft_mint,
            claim_count: stats.claim_count,
            total_claim_amount: stats.total_claim_amount,
            latest_claim_time: stats.latest_claim_time,
            earliest_claim_time: stats.earliest_claim_time,
            unique_claimers_count: stats.unique_claimers_count,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nft_mint_claim_stats_response_serialization() {
        let response = NftMintClaimStatsResponse {
            nft_mint: "NFTaoszFxtEmGXvHcb8yfkGZxqLPAfwDqLN1mhrV2jM".to_string(),
            claim_count: 150,
            total_claim_amount: 15000,
            latest_claim_time: Some(1735203600),
            earliest_claim_time: Some(1704067200),
            unique_claimers_count: 75,
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("NFTaoszFxtEmGXvHcb8yfkGZxqLPAfwDqLN1mhrV2jM"));
        assert!(json.contains("150"));
    }

    #[test]
    fn test_nft_mint_claim_stats_list_response() {
        let stats = vec![
            NftMintClaimStatsResponse {
                nft_mint: "NFT1".to_string(),
                claim_count: 100,
                total_claim_amount: 10000,
                latest_claim_time: Some(1735203600),
                earliest_claim_time: Some(1704067200),
                unique_claimers_count: 50,
            },
            NftMintClaimStatsResponse {
                nft_mint: "NFT2".to_string(),
                claim_count: 50,
                total_claim_amount: 5000,
                latest_claim_time: Some(1735203600),
                earliest_claim_time: Some(1704067200),
                unique_claimers_count: 25,
            },
        ];

        let response = NftMintClaimStatsListResponse {
            stats: stats.clone(),
            total_nfts: 2,
        };

        assert_eq!(response.stats.len(), 2);
        assert_eq!(response.total_nfts, 2);
    }
}
