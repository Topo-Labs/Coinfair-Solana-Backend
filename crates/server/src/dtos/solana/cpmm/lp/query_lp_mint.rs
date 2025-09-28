use crate::dtos::solana::common::TokenInfo;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::Validate;

/// 查询LP mint的请求DTO
#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct QueryLpMintRequest {
    /// 支持多个lp_mint，英文逗号分隔
    #[validate(length(min = 1, message = "lps参数不能为空"))]
    pub lps: String,

    /// 页码（可选，默认1）
    #[validate(range(min = 1, message = "页码必须大于0"))]
    pub page: Option<u64>,

    /// 每页大小（可选，默认20，最大100）
    #[validate(range(min = 1, max = 100, message = "每页大小必须在1-100之间"))]
    pub page_size: Option<u64>,
}

/// 池子统计数据（天、周、月）
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PoolPeriodStats {
    pub volume: f64,
    #[serde(rename = "volumeQuote")]
    pub volume_quote: f64,
    #[serde(rename = "volumeFee")]
    pub volume_fee: f64,
    pub apr: f64,
    #[serde(rename = "feeApr")]
    pub fee_apr: f64,
    #[serde(rename = "priceMin")]
    pub price_min: f64,
    #[serde(rename = "priceMax")]
    pub price_max: f64,
    #[serde(rename = "rewardApr")]
    pub reward_apr: Vec<f64>,
}

/// 奖励代币信息
#[derive(Debug, Serialize, ToSchema)]
pub struct RewardInfo {
    pub mint: TokenInfo,
    #[serde(rename = "perSecond")]
    pub per_second: String,
    #[serde(rename = "startTime")]
    pub start_time: String,
    #[serde(rename = "endTime")]
    pub end_time: String,
}

/// LP代币查询返回的池子信息
#[derive(Debug, Serialize, ToSchema)]
pub struct LpMintPoolInfo {
    #[serde(rename = "type")]
    pub pool_type: String, // "Standard"
    #[serde(rename = "programId")]
    pub program_id: String,
    pub id: String, // 池子地址
    #[serde(rename = "mintA")]
    pub mint_a: TokenInfo,
    #[serde(rename = "mintB")]
    pub mint_b: TokenInfo,
    pub price: f64,
    #[serde(rename = "mintAmountA")]
    pub mint_amount_a: f64,
    #[serde(rename = "mintAmountB")]
    pub mint_amount_b: f64,
    #[serde(rename = "feeRate")]
    pub fee_rate: f64,
    #[serde(rename = "openTime")]
    pub open_time: String,
    pub tvl: f64,
    pub day: PoolPeriodStats,
    pub week: PoolPeriodStats,
    pub month: PoolPeriodStats,
    pub pooltype: Vec<String>, // ["Amm", "OpenBookMarket"]
    #[serde(rename = "rewardDefaultPoolInfos")]
    pub reward_default_pool_infos: String, // "Ecosystem"
    #[serde(rename = "rewardDefaultInfos")]
    pub reward_default_infos: Vec<RewardInfo>,
    #[serde(rename = "farmUpcomingCount")]
    pub farm_upcoming_count: u32,
    #[serde(rename = "farmOngoingCount")]
    pub farm_ongoing_count: u32,
    #[serde(rename = "farmFinishedCount")]
    pub farm_finished_count: u32,
    #[serde(rename = "marketId")]
    pub market_id: String,
    #[serde(rename = "lpMint")]
    pub lp_mint: TokenInfo,
    #[serde(rename = "lpPrice")]
    pub lp_price: f64,
    #[serde(rename = "lpAmount")]
    pub lp_amount: f64,
    #[serde(rename = "burnPercent")]
    pub burn_percent: f64,
    #[serde(rename = "launchMigratePool")]
    pub launch_migrate_pool: bool,
}

impl QueryLpMintRequest {
    /// 获取页码（默认1）
    pub fn get_page(&self) -> u64 {
        self.page.unwrap_or(1).max(1)
    }

    /// 获取每页大小（默认20，最大100）
    pub fn get_page_size(&self) -> u64 {
        self.page_size.unwrap_or(20).min(100).max(1)
    }

    /// 获取跳过的记录数
    pub fn get_skip(&self) -> u64 {
        (self.get_page() - 1) * self.get_page_size()
    }

    /// 解析lps字符串为Vector
    pub fn parse_lp_mints(&self) -> Vec<String> {
        self.lps
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    }
}

impl LpMintPoolInfo {
    /// 创建一个默认的空池子信息（当查询不到数据时使用）
    pub fn default_empty(lp_mint_address: &str) -> Self {
        let default_token = TokenInfo {
            chain_id: 101,
            address: "11111111111111111111111111111111".to_string(),
            program_id: "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA".to_string(),
            logo_uri: "".to_string(),
            symbol: "".to_string(),
            name: "".to_string(),
            decimals: 0,
            tags: vec![],
            extensions: serde_json::Value::Object(serde_json::Map::new()),
        };

        let default_stats = PoolPeriodStats {
            volume: 0.0,
            volume_quote: 0.0,
            volume_fee: 0.0,
            apr: 0.0,
            fee_apr: 0.0,
            price_min: 0.0,
            price_max: 0.0,
            reward_apr: vec![0.0],
        };

        let lp_token = TokenInfo {
            chain_id: 101,
            address: lp_mint_address.to_string(),
            program_id: "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA".to_string(),
            logo_uri: "".to_string(),
            symbol: "".to_string(),
            name: "".to_string(),
            decimals: 0,
            tags: vec![],
            extensions: serde_json::Value::Object(serde_json::Map::new()),
        };

        Self {
            pool_type: "Standard".to_string(),
            program_id: "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8".to_string(),
            id: "11111111111111111111111111111111".to_string(),
            mint_a: default_token.clone(),
            mint_b: default_token,
            price: 0.0,
            mint_amount_a: 0.0,
            mint_amount_b: 0.0,
            fee_rate: 0.0025,
            open_time: "0".to_string(),
            tvl: 0.0,
            day: default_stats.clone(),
            week: default_stats.clone(),
            month: default_stats,
            pooltype: vec!["Amm".to_string()],
            reward_default_pool_infos: "Ecosystem".to_string(),
            reward_default_infos: vec![],
            farm_upcoming_count: 0,
            farm_ongoing_count: 0,
            farm_finished_count: 0,
            market_id: "11111111111111111111111111111111".to_string(),
            lp_mint: lp_token,
            lp_price: 0.0,
            lp_amount: 0.0,
            burn_percent: 0.0,
            launch_migrate_pool: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_request_defaults() {
        let request = QueryLpMintRequest {
            lps: "mint1,mint2".to_string(),
            page: None,
            page_size: None,
        };

        assert_eq!(request.get_page(), 1);
        assert_eq!(request.get_page_size(), 20);
        assert_eq!(request.get_skip(), 0);
    }

    #[test]
    fn test_query_request_page_size_limits() {
        let request = QueryLpMintRequest {
            lps: "mint1".to_string(),
            page: Some(2),
            page_size: Some(200), // 超过最大值
        };

        assert_eq!(request.get_page(), 2);
        assert_eq!(request.get_page_size(), 100); // 被限制为最大值
        assert_eq!(request.get_skip(), 100);
    }

    #[test]
    fn test_parse_lp_mints() {
        let request = QueryLpMintRequest {
            lps: "mint1,mint2, mint3 ,".to_string(),
            page: None,
            page_size: None,
        };

        let mints = request.parse_lp_mints();
        assert_eq!(mints, vec!["mint1", "mint2", "mint3"]);
    }

    #[test]
    fn test_default_empty_pool_info() {
        let pool_info = LpMintPoolInfo::default_empty("test_lp_mint");
        assert_eq!(pool_info.lp_mint.address, "test_lp_mint");
        assert_eq!(pool_info.pool_type, "Standard");
        assert_eq!(pool_info.tvl, 0.0);
    }
}