use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
/// 池子列表查询请求参数
// #[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Validate, IntoParams)]
// pub struct PoolListRequest {
//     /// 按池子类型过滤
//     #[serde(rename = "poolType")]
//     pub pool_type: Option<String>,

//     /// 排序字段 (default, created_at, price, open_time)
//     #[serde(rename = "poolSortField")]
//     pub pool_sort_field: Option<String>,

//     /// 排序方向 (asc, desc)
//     #[serde(rename = "sortType")]
//     pub sort_type: Option<String>,

//     /// 页大小 (1-100, 默认20)
//     #[serde(rename = "pageSize")]
//     #[validate(range(min = 1, max = 100))]
//     pub page_size: Option<u64>,

//     /// 页码 (1-based, 默认1)
//     #[validate(range(min = 1))]
//     pub page: Option<u64>,

//     /// 按创建者钱包地址过滤
//     #[serde(rename = "creatorWallet")]
//     pub creator_wallet: Option<String>,

//     /// 按代币mint地址过滤
//     #[serde(rename = "mintAddress")]
//     pub mint_address: Option<String>,

//     /// 按池子状态过滤
//     pub status: Option<String>,
// }

// impl Default for PoolListRequest {
//     fn default() -> Self {
//         Self {
//             pool_type: None,
//             pool_sort_field: Some("default".to_string()),
//             sort_type: Some("desc".to_string()),
//             page_size: Some(20),
//             page: Some(1),
//             creator_wallet: None,
//             mint_address: None,
//             status: None,
//         }
//     }
// }

/// 新的池子列表响应格式（匹配期望格式）
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct NewPoolListResponse {
    /// 请求ID
    pub id: String,

    /// 请求是否成功
    pub success: bool,

    /// 响应数据
    pub data: PoolListData,
}

/// 新的池子列表响应格式（匹配期望格式）
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct NewPoolListResponse2 {
    /// 请求ID
    pub id: String,

    /// 请求是否成功
    pub success: bool,

    /// 响应数据
    // pub data: PoolListData2,
    pub data: Vec<PoolInfo>,
}

/// 池子列表数据
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PoolListData {
    /// 池子总数
    pub count: u64,

    /// 池子详细信息列表
    pub data: Vec<PoolInfo>,

    /// 是否有下一页
    #[serde(rename = "hasNextPage")]
    pub has_next_page: bool,
}

/// 池子列表数据
// #[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
// pub struct PoolListData2 {
//     /// 池子详细信息列表
//     pub data: Vec<PoolInfo>,
// }
/// 池子信息（新格式）
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PoolInfo {
    /// 池子类型
    #[serde(rename = "type")]
    pub pool_type: String,

    /// 程序ID
    #[serde(rename = "programId")]
    pub program_id: String,

    /// 池子ID（地址）
    pub id: String,

    /// 代币A信息
    #[serde(rename = "mintA")]
    pub mint_a: ExtendedMintInfo,

    /// 代币B信息
    #[serde(rename = "mintB")]
    pub mint_b: ExtendedMintInfo,

    /// 默认奖励池信息
    #[serde(rename = "rewardDefaultPoolInfos")]
    pub reward_default_pool_infos: String,

    /// 默认奖励信息
    #[serde(rename = "rewardDefaultInfos")]
    pub reward_default_infos: Vec<RewardInfo>,

    /// 当前价格
    pub price: f64,

    /// 代币A数量
    #[serde(rename = "mintAmountA")]
    pub mint_amount_a: f64,

    /// 代币B数量
    #[serde(rename = "mintAmountB")]
    pub mint_amount_b: f64,

    /// 手续费率
    #[serde(rename = "feeRate")]
    pub fee_rate: f64,

    /// 开放时间
    #[serde(rename = "openTime")]
    pub open_time: String,

    /// 总价值锁定
    pub tvl: f64,

    /// 日统计
    pub day: Option<PeriodStats>,

    /// 周统计
    pub week: Option<PeriodStats>,

    /// 月统计
    pub month: Option<PeriodStats>,

    /// 池子类型标签
    pub pooltype: Vec<String>,

    /// 即将开始的农场数量
    #[serde(rename = "farmUpcomingCount")]
    pub farm_upcoming_count: u32,

    /// 进行中的农场数量
    #[serde(rename = "farmOngoingCount")]
    pub farm_ongoing_count: u32,

    /// 已结束的农场数量
    #[serde(rename = "farmFinishedCount")]
    pub farm_finished_count: u32,

    /// 配置信息
    pub config: Option<PoolConfigInfo>,

    /// 燃烧百分比
    #[serde(rename = "burnPercent")]
    pub burn_percent: f64,

    /// 启动迁移池
    #[serde(rename = "launchMigratePool")]
    pub launch_migrate_pool: bool,
}

/// 扩展的mint信息（新格式）
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ExtendedMintInfo {
    /// 链ID
    #[serde(rename = "chainId")]
    pub chain_id: u32,

    /// 代币地址
    pub address: String,

    /// 程序ID
    #[serde(rename = "programId")]
    pub program_id: String,

    /// Logo URI
    #[serde(rename = "logoURI")]
    pub logo_uri: Option<String>,

    /// 代币符号
    pub symbol: Option<String>,

    /// 代币名称
    pub name: Option<String>,

    /// 精度
    pub decimals: u8,

    /// 标签
    pub tags: Vec<String>,

    /// 扩展信息
    pub extensions: serde_json::Value,
}

/// 奖励信息
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RewardInfo {
    /// 奖励代币信息
    pub mint: ExtendedMintInfo,

    /// 每秒奖励
    #[serde(rename = "perSecond")]
    pub per_second: String,

    /// 开始时间
    #[serde(rename = "startTime")]
    pub start_time: String,

    /// 结束时间
    #[serde(rename = "endTime")]
    pub end_time: String,
}

/// 周期统计信息
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Default)]
pub struct PeriodStats {
    /// 交易量
    pub volume: f64,

    /// 报价交易量
    #[serde(rename = "volumeQuote")]
    pub volume_quote: f64,

    /// 手续费交易量
    #[serde(rename = "volumeFee")]
    pub volume_fee: f64,

    /// 年化收益率
    pub apr: f64,

    /// 手续费年化收益率
    #[serde(rename = "feeApr")]
    pub fee_apr: f64,

    /// 最低价格
    #[serde(rename = "priceMin")]
    pub price_min: f64,

    /// 最高价格
    #[serde(rename = "priceMax")]
    pub price_max: f64,

    /// 奖励年化收益率
    #[serde(rename = "rewardApr")]
    pub reward_apr: Vec<f64>,
}

/// 池子配置信息
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PoolConfigInfo {
    /// 配置ID
    pub id: String,

    /// 配置索引
    pub index: u32,

    /// 协议费率
    #[serde(rename = "protocolFeeRate")]
    pub protocol_fee_rate: u32,

    /// 交易费率
    #[serde(rename = "tradeFeeRate")]
    pub trade_fee_rate: u32,

    /// Tick间距
    #[serde(rename = "tickSpacing")]
    pub tick_spacing: u32,

    /// 基金费率
    #[serde(rename = "fundFeeRate")]
    pub fund_fee_rate: u32,

    /// 默认范围
    #[serde(rename = "defaultRange")]
    pub default_range: f64,

    /// 默认范围点
    #[serde(rename = "defaultRangePoint")]
    pub default_range_point: Vec<f64>,
}

// 旧版池子列表响应（保持向后兼容）
// #[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
// pub struct PoolListResponse {
//     /// 池子列表
//     pub pools: Vec<ClmmPool>,

//     /// 分页元数据
//     pub pagination: PaginationMeta,

//     /// 过滤器摘要
//     pub filters: FilterSummary,
// }

// /// 分页元数据
// #[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
// pub struct PaginationMeta {
//     /// 当前页码
//     pub current_page: u64,

//     /// 页大小
//     pub page_size: u64,

//     /// 符合条件的总记录数
//     pub total_count: u64,

//     /// 总页数
//     pub total_pages: u64,

//     /// 是否有下一页
//     pub has_next: bool,

//     /// 是否有上一页
//     pub has_prev: bool,
// }

// /// 过滤器摘要
// #[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
// pub struct FilterSummary {
//     /// 应用的池子类型过滤器
//     pub pool_type: Option<String>,

//     /// 应用的排序字段
//     pub sort_field: String,

//     /// 应用的排序方向
//     pub sort_direction: String,

//     /// 按池子类型统计数量
//     pub type_counts: Vec<TypeCount>,
// }

// /// 池子类型统计
// #[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
// pub struct TypeCount {
//     /// 池子类型
//     pub pool_type: String,

//     /// 数量
//     pub count: u64,
// }
/*
#[cfg(test)]
mod tests {
    use super::*;

        #[test]
        fn test_pool_list_request_default() {
            let request = PoolListRequest::default();

            assert_eq!(request.pool_type, None);
            assert_eq!(request.pool_sort_field, Some("default".to_string()));
            assert_eq!(request.sort_type, Some("desc".to_string()));
            assert_eq!(request.page_size, Some(20));
            assert_eq!(request.page, Some(1));
            assert_eq!(request.creator_wallet, None);
            assert_eq!(request.mint_address, None);
            assert_eq!(request.status, None);
        }

        #[test]
        fn test_pool_list_request_validation_valid() {
            let request = PoolListRequest {
                pool_type: Some("concentrated".to_string()),
                pool_sort_field: Some("created_at".to_string()),
                sort_type: Some("asc".to_string()),
                page_size: Some(50),
                page: Some(2),
                creator_wallet: Some("9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM".to_string()),
                mint_address: Some("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string()),
                status: Some("Active".to_string()),
            };

            assert!(request.validate().is_ok());
        }

        #[test]
        fn test_pool_list_request_validation_invalid_page_size() {
            let request = PoolListRequest {
                page_size: Some(101), // 超过最大值100
                ..Default::default()
            };

            let validation_result = request.validate();
            assert!(validation_result.is_err());

            let errors = validation_result.unwrap_err();
            assert!(errors.field_errors().contains_key("pageSize"));
        }

        #[test]
        fn test_pool_list_request_validation_invalid_page_size_zero() {
            let request = PoolListRequest {
                page_size: Some(0), // 小于最小值1
                ..Default::default()
            };

            let validation_result = request.validate();
            assert!(validation_result.is_err());

            let errors = validation_result.unwrap_err();
            assert!(errors.field_errors().contains_key("pageSize"));
        }

        #[test]
        fn test_pool_list_request_validation_invalid_page_zero() {
            let request = PoolListRequest {
                page: Some(0), // 小于最小值1
                ..Default::default()
            };

            let validation_result = request.validate();
            assert!(validation_result.is_err());

            let errors = validation_result.unwrap_err();
            assert!(errors.field_errors().contains_key("page"));
        }

        #[test]
        fn test_pool_list_request_serialization() {
            let request = PoolListRequest {
                pool_type: Some("concentrated".to_string()),
                pool_sort_field: Some("created_at".to_string()),
                sort_type: Some("asc".to_string()),
                page_size: Some(50),
                page: Some(2),
                creator_wallet: Some("9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM".to_string()),
                mint_address: Some("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string()),
                status: Some("Active".to_string()),
            };

            let json = serde_json::to_string(&request).unwrap();
            let deserialized: PoolListRequest = serde_json::from_str(&json).unwrap();

            assert_eq!(request.pool_type, deserialized.pool_type);
            assert_eq!(request.pool_sort_field, deserialized.pool_sort_field);
            assert_eq!(request.sort_type, deserialized.sort_type);
            assert_eq!(request.page_size, deserialized.page_size);
            assert_eq!(request.page, deserialized.page);
            assert_eq!(request.creator_wallet, deserialized.creator_wallet);
            assert_eq!(request.mint_address, deserialized.mint_address);
            assert_eq!(request.status, deserialized.status);
        }

        #[test]
        fn test_pool_list_request_serde_rename() {
            let json = r#"{
                "poolType": "concentrated",
                "poolSortField": "created_at",
                "sortType": "asc",
                "pageSize": 50,
                "page": 2,
                "creatorWallet": "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM",
                "mintAddress": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
                "status": "Active"
            }"#;

            let request: PoolListRequest = serde_json::from_str(json).unwrap();

            assert_eq!(request.pool_type, Some("concentrated".to_string()));
            assert_eq!(request.pool_sort_field, Some("created_at".to_string()));
            assert_eq!(request.sort_type, Some("asc".to_string()));
            assert_eq!(request.page_size, Some(50));
            assert_eq!(request.page, Some(2));
            assert_eq!(
                request.creator_wallet,
                Some("9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM".to_string())
            );
            assert_eq!(
                request.mint_address,
                Some("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string())
            );
            assert_eq!(request.status, Some("Active".to_string()));
        }

        #[test]
        fn test_pool_list_request_edge_cases() {
            // Test with minimum valid values
            let request = PoolListRequest {
                page_size: Some(1),
                page: Some(1),
                ..Default::default()
            };
            assert!(request.validate().is_ok());

            // Test with maximum valid values
            let request = PoolListRequest {
                page_size: Some(100),
                page: Some(u64::MAX),
                ..Default::default()
            };
            assert!(request.validate().is_ok());
        }

        #[test]
        fn test_pool_list_request_optional_fields() {
            // Test with all optional fields as None
            let request = PoolListRequest {
                pool_type: None,
                pool_sort_field: None,
                sort_type: None,
                page_size: None,
                page: None,
                creator_wallet: None,
                mint_address: None,
                status: None,
            };
            assert!(request.validate().is_ok());
        }

    #[test]
    fn test_pagination_meta_creation() {
        let pagination = PaginationMeta {
            current_page: 2,
            page_size: 20,
            total_count: 150,
            total_pages: 8,
            has_next: true,
            has_prev: true,
        };

        assert_eq!(pagination.current_page, 2);
        assert_eq!(pagination.page_size, 20);
        assert_eq!(pagination.total_count, 150);
        assert_eq!(pagination.total_pages, 8);
        assert!(pagination.has_next);
        assert!(pagination.has_prev);
    }

    #[test]
    fn test_filter_summary_creation() {
        let type_counts = vec![
            TypeCount {
                pool_type: "concentrated".to_string(),
                count: 100,
            },
            TypeCount {
                pool_type: "standard".to_string(),
                count: 50,
            },
        ];

        let filter_summary = FilterSummary {
            pool_type: Some("concentrated".to_string()),
            sort_field: "created_at".to_string(),
            sort_direction: "desc".to_string(),
            type_counts,
        };

        assert_eq!(filter_summary.pool_type, Some("concentrated".to_string()));
        assert_eq!(filter_summary.sort_field, "created_at");
        assert_eq!(filter_summary.sort_direction, "desc");
        assert_eq!(filter_summary.type_counts.len(), 2);
        assert_eq!(filter_summary.type_counts[0].pool_type, "concentrated");
        assert_eq!(filter_summary.type_counts[0].count, 100);
        assert_eq!(filter_summary.type_counts[1].pool_type, "standard");
        assert_eq!(filter_summary.type_counts[1].count, 50);
    }

    #[test]
    fn test_pool_list_response_creation() {
        let pools = vec![]; // Empty for test
        let pagination = PaginationMeta {
            current_page: 1,
            page_size: 20,
            total_count: 0,
            total_pages: 0,
            has_next: false,
            has_prev: false,
        };
        let filters = FilterSummary {
            pool_type: None,
            sort_field: "default".to_string(),
            sort_direction: "desc".to_string(),
            type_counts: vec![],
        };

        let response = PoolListResponse {
            pools,
            pagination,
            filters,
        };

        assert_eq!(response.pools.len(), 0);
        assert_eq!(response.pagination.current_page, 1);
        assert_eq!(response.filters.sort_field, "default");
    }

    #[test]
    fn test_type_count_serialization() {
        let type_count = TypeCount {
            pool_type: "concentrated".to_string(),
            count: 42,
        };

        let json = serde_json::to_string(&type_count).unwrap();
        let deserialized: TypeCount = serde_json::from_str(&json).unwrap();

        assert_eq!(type_count.pool_type, deserialized.pool_type);
        assert_eq!(type_count.count, deserialized.count);
    }

}
    */
