use std::collections::HashMap;

use crate::{
    dtos::solana_dto::{ApiResponse, ErrorResponse},
    services::Services,
};
use axum::{
    extract::{Extension, Query},
    http::StatusCode,
    response::Json,
    routing::get,
    Router,
};
use database::clmm_pool::model::PoolListRequest;
use tracing::{error, info};

pub struct ClmmPoolQueryController;

impl ClmmPoolQueryController {
    pub fn routes() -> Router {
        Router::new()
            .route("/info", get(get_pool_by_address))
            .route("/by-mint", get(get_pools_by_mint))
            .route("/by-creator", get(get_pools_by_creator))
            .route("/query", get(query_pools))
            .route("/statistics", get(get_pool_statistics))
            .route("/key/ids", get(get_pools_key_by_ids))
    }
}

/// 根据池子地址查询池子信息
///
/// # 查询参数
///
/// - `pool_address`: 池子地址
///
/// # 响应示例
///
/// ```json
/// {
///   "success": true,
///   "data": {
///     "pool_address": "池子地址",
///     "mint0": { "mint_address": "代币0地址", "decimals": 9 },
///     "mint1": { "mint_address": "代币1地址", "decimals": 6 },
///     "price_info": { "initial_price": 100.0, "current_price": 105.0 },
///     "status": "Active",
///     "created_at": 1640995200
///   }
/// }
/// ```
#[utoipa::path(
    get,
    path = "/api/v1/solana/pool/info",
    params(
        ("pool_address" = String, Query, description = "池子地址")
    ),
    responses(
        (status = 200, description = "查询成功", body = ApiResponse<Option<database::clmm_pool::ClmmPool>>),
        (status = 400, description = "参数错误", body = ApiResponse<ErrorResponse>),
        (status = 500, description = "查询失败", body = ApiResponse<ErrorResponse>)
    ),
    tag = "CLMM池子查询"
)]
pub async fn get_pool_by_address(
    Extension(services): Extension<Services>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<ApiResponse<Option<database::clmm_pool::ClmmPool>>>, (StatusCode, Json<ApiResponse<ErrorResponse>>)> {
    let pool_address = params.get("pool_address").ok_or_else(|| {
        let error_response = ErrorResponse::new("MISSING_PARAMETER", "缺少pool_address参数");
        (StatusCode::BAD_REQUEST, Json(ApiResponse::error(error_response)))
    })?;

    info!("🔍 接收到查询池子信息请求: {}", pool_address);

    match services.solana.get_pool_by_address(&pool_address).await {
        Ok(pool_info) => {
            if pool_info.is_some() {
                info!("✅ 查询池子信息成功");
            } else {
                info!("⚠️ 未找到池子信息");
            }
            Ok(Json(ApiResponse::success(pool_info)))
        }
        Err(e) => {
            error!("❌ 查询池子信息失败: {:?}", e);
            let error_response = ErrorResponse::new("QUERY_POOL_ERROR", &format!("查询池子信息失败: {}", e));
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error(error_response)),
            ))
        }
    }
}

/// 根据代币Mint查询池子列表
///
/// # 查询参数
///
/// - `mint_address`: 代币Mint地址
///
/// # 响应示例
///
/// ```json
/// {
///   "success": true,
///   "data": [
///     {
///       "pool_address": "池子地址",
///       "mint0": { "mint_address": "代币0地址", "decimals": 9 },
///       "mint1": { "mint_address": "代币1地址", "decimals": 6 },
///       "price_info": { "initial_price": 100.0, "current_price": 105.0 },
///       "status": "Active"
///     }
///   ]
/// }
/// ```
#[utoipa::path(
    get,
    path = "/api/v1/solana/pool/by-mint",
    params(
        ("mint_address" = String, Query, description = "代币Mint地址")
    ),
    responses(
        (status = 200, description = "查询成功", body = ApiResponse<Vec<database::clmm_pool::ClmmPool>>),
        (status = 400, description = "参数错误", body = ApiResponse<ErrorResponse>),
        (status = 500, description = "查询失败", body = ApiResponse<ErrorResponse>)
    ),
    tag = "CLMM池子查询"
)]
pub async fn get_pools_by_mint(
    Extension(services): Extension<Services>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<ApiResponse<Vec<database::clmm_pool::ClmmPool>>>, (StatusCode, Json<ApiResponse<ErrorResponse>>)> {
    let mint_address = params.get("mint_address").ok_or_else(|| {
        let error_response = ErrorResponse::new("MISSING_PARAMETER", "缺少mint_address参数");
        (StatusCode::BAD_REQUEST, Json(ApiResponse::error(error_response)))
    })?;

    info!("🔍 接收到根据Mint查询池子列表请求: {}", mint_address);

    match services.solana.get_pools_by_mint(&mint_address, None).await {
        Ok(pools) => {
            info!("✅ 查询池子列表成功，找到{}个池子", pools.len());
            Ok(Json(ApiResponse::success(pools)))
        }
        Err(e) => {
            error!("❌ 查询池子列表失败: {:?}", e);
            let error_response = ErrorResponse::new("QUERY_POOLS_BY_MINT_ERROR", &format!("查询池子列表失败: {}", e));
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error(error_response)),
            ))
        }
    }
}

/// 根据创建者查询池子列表
///
/// # 查询参数
///
/// - `creator_address`: 创建者地址
/// - `limit` (可选): 返回数量限制，默认100
/// - `offset` (可选): 分页偏移量，默认0
///
/// # 响应示例
///
/// ```json
/// {
///   "success": true,
///   "data": [
///     {
///       "pool_address": "池子地址",
///       "mint0": { "mint_address": "代币0地址", "decimals": 9 },
///       "mint1": { "mint_address": "代币1地址", "decimals": 6 },
///       "price_info": { "initial_price": 100.0, "current_price": 105.0 },
///       "status": "Active",
///       "created_at": 1640995200
///     }
///   ]
/// }
/// ```
#[utoipa::path(
    get,
    path = "/api/v1/solana/pool/by-creator",
    params(
        ("creator_address" = String, Query, description = "创建者地址"),
        ("limit" = Option<u32>, Query, description = "返回数量限制"),
        ("offset" = Option<u32>, Query, description = "分页偏移量")
    ),
    responses(
        (status = 200, description = "查询成功", body = ApiResponse<Vec<database::clmm_pool::ClmmPool>>),
        (status = 400, description = "参数错误", body = ApiResponse<ErrorResponse>),
        (status = 500, description = "查询失败", body = ApiResponse<ErrorResponse>)
    ),
    tag = "CLMM池子查询"
)]
pub async fn get_pools_by_creator(
    Extension(services): Extension<Services>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<ApiResponse<Vec<database::clmm_pool::ClmmPool>>>, (StatusCode, Json<ApiResponse<ErrorResponse>>)> {
    let creator_address = params.get("creator_address").ok_or_else(|| {
        let error_response = ErrorResponse::new("MISSING_PARAMETER", "缺少creator_address参数");
        (StatusCode::BAD_REQUEST, Json(ApiResponse::error(error_response)))
    })?;

    let limit = params.get("limit").and_then(|v| v.parse::<u32>().ok()).unwrap_or(100);

    info!("🔍 接收到根据创建者查询池子列表请求");
    info!("  创建者: {}", creator_address);
    info!("  限制: {}", limit);

    match services
        .solana
        .get_pools_by_creator(&creator_address, Some(limit as i64))
        .await
    {
        Ok(pools) => {
            info!("✅ 查询池子列表成功，找到{}个池子", pools.len());
            Ok(Json(ApiResponse::success(pools)))
        }
        Err(e) => {
            error!("❌ 查询池子列表失败: {:?}", e);
            let error_response =
                ErrorResponse::new("QUERY_POOLS_BY_CREATOR_ERROR", &format!("查询池子列表失败: {}", e));
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error(error_response)),
            ))
        }
    }
}

/// 查询池子列表（支持多种过滤条件）
///
/// # 查询参数
///
/// - `pool_address` (可选): 池子地址
/// - `mint_address` (可选): 代币Mint地址
/// - `creator_wallet` (可选): 创建者钱包地址
/// - `status` (可选): 池子状态 (Created/Active/Paused/Closed)
/// - `min_price` (可选): 最低价格
/// - `max_price` (可选): 最高价格
/// - `start_time` (可选): 开始时间
/// - `end_time` (可选): 结束时间
/// - `page` (可选): 页码
/// - `limit` (可选): 每页数量
/// - `sort_by` (可选): 排序字段
/// - `sort_order` (可选): 排序方向
///
/// # 响应示例
///
/// ```json
/// {
///   "success": true,
///   "data": [
///     {
///       "pool_address": "池子地址",
///       "mint0": { "mint_address": "代币0地址", "decimals": 9 },
///       "mint1": { "mint_address": "代币1地址", "decimals": 6 },
///       "price_info": { "initial_price": 100.0, "current_price": 105.0 },
///       "status": "Active",
///       "tvl": 1000000.0,
///       "volume_24h": 50000.0
///     }
///   ]
/// }
/// ```
#[utoipa::path(
    get,
    path = "/api/v1/solana/pool/query",
    params(
        ("pool_address" = Option<String>, Query, description = "池子地址"),
        ("mint_address" = Option<String>, Query, description = "代币Mint地址"),
        ("creator_wallet" = Option<String>, Query, description = "创建者钱包地址"),
        ("status" = Option<String>, Query, description = "池子状态"),
        ("min_price" = Option<f64>, Query, description = "最低价格"),
        ("max_price" = Option<f64>, Query, description = "最高价格"),
        ("start_time" = Option<u64>, Query, description = "开始时间"),
        ("end_time" = Option<u64>, Query, description = "结束时间"),
        ("page" = Option<u64>, Query, description = "页码"),
        ("limit" = Option<u64>, Query, description = "每页数量"),
        ("sort_by" = Option<String>, Query, description = "排序字段"),
        ("sort_order" = Option<String>, Query, description = "排序方向")
    ),
    responses(
        (status = 200, description = "查询成功", body = ApiResponse<Vec<database::clmm_pool::ClmmPool>>),
        (status = 400, description = "参数错误", body = ApiResponse<ErrorResponse>),
        (status = 500, description = "查询失败", body = ApiResponse<ErrorResponse>)
    ),
    tag = "CLMM池子查询"
)]
pub async fn query_pools(
    Extension(services): Extension<Services>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<ApiResponse<Vec<database::clmm_pool::ClmmPool>>>, (StatusCode, Json<ApiResponse<ErrorResponse>>)> {
    info!("🔍 执行复杂池子查询");

    // 构建查询参数
    let query_params = database::clmm_pool::PoolQueryParams {
        pool_address: params.get("pool_address").cloned(),
        mint_address: params.get("mint_address").cloned(),
        creator_wallet: params.get("creator_wallet").cloned(),
        status: params.get("status").and_then(|s| match s.as_str() {
            "Created" => Some(database::clmm_pool::PoolStatus::Created),
            "Active" => Some(database::clmm_pool::PoolStatus::Active),
            "Paused" => Some(database::clmm_pool::PoolStatus::Paused),
            "Closed" => Some(database::clmm_pool::PoolStatus::Closed),
            _ => None,
        }),
        min_price: params.get("min_price").and_then(|s| s.parse().ok()),
        max_price: params.get("max_price").and_then(|s| s.parse().ok()),
        start_time: params.get("start_time").and_then(|s| s.parse().ok()),
        end_time: params.get("end_time").and_then(|s| s.parse().ok()),
        page: params.get("page").and_then(|s| s.parse().ok()),
        limit: params.get("limit").and_then(|s| s.parse().ok()),
        sort_by: params.get("sort_by").cloned(),
        sort_order: params.get("sort_order").cloned(),
    };

    match services.solana.query_pools(&query_params).await {
        Ok(pools) => {
            info!("✅ 查询完成，找到 {} 个池子", pools.len());
            Ok(Json(ApiResponse::success(pools)))
        }
        Err(e) => {
            error!("❌ 复杂查询失败: {}", e);
            let error_response = ErrorResponse::new("QUERY_POOLS_FAILED", &format!("复杂查询失败: {}", e));
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error(error_response)),
            ))
        }
    }
}

/// 获取池子统计信息
///
/// 返回所有池子的聚合统计数据。
///
/// # 响应示例
///
/// ```json
/// {
///   "success": true,
///   "data": {
///     "total_pools": 150,
///     "active_pools": 120,
///     "total_tvl": 50000000.0,
///     "total_volume_24h": 2000000.0,
///     "top_pools_by_tvl": [
///       {
///         "pool_address": "池子地址",
///         "tvl": 5000000.0,
///         "mint0": "SOL",
///         "mint1": "USDC"
///       }
///     ]
///   }
/// }
/// ```
#[utoipa::path(
    get,
    path = "/api/v1/solana/pool/statistics",
    responses(
        (status = 200, description = "查询成功", body = ApiResponse<database::clmm_pool::PoolStats>),
        (status = 500, description = "查询失败", body = ApiResponse<ErrorResponse>)
    ),
    tag = "CLMM池子查询"
)]
pub async fn get_pool_statistics(
    Extension(services): Extension<Services>,
) -> Result<Json<ApiResponse<database::clmm_pool::PoolStats>>, (StatusCode, Json<ApiResponse<ErrorResponse>>)> {
    info!("📊 接收到获取池子统计信息请求");

    match services.solana.get_pool_statistics().await {
        Ok(stats) => {
            info!("✅ 获取池子统计信息成功");
            info!("  总池子数: {}", stats.total_pools);
            info!("  活跃池子数: {}", stats.active_pools);
            Ok(Json(ApiResponse::success(stats)))
        }
        Err(e) => {
            error!("❌ 获取池子统计信息失败: {:?}", e);
            let error_response = ErrorResponse::new("GET_POOL_STATS_ERROR", &format!("获取池子统计信息失败: {}", e));
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error(error_response)),
            ))
        }
    }
}

/// 获取池子列表
///
/// 返回符合查询条件的池子列表，支持代币对过滤和分页。
///
/// # 查询参数
///
/// - `mint0` (可选): 代币0地址
/// - `mint1` (可选): 代币1地址
/// - `type` (可选): 池子类型，值为 `raydium` 或 `all`
/// - `page` (可选): 页码，默认1
/// - `limit` (可选): 每页数量，默认20
///
/// # 响应示例
///
/// ```json
/// {
///   "status": 200,
///   "message": "success",
///   "data": [
///     {
///       "pool_address": "池子地址",
///       "mint0": "So11111111111111111111111111111111111111112",
///       "mint1": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
///       "mint0_symbol": "SOL",
///       "mint1_symbol": "USDC",
///       "mint0_decimal": 9,
///       "mint1_decimal": 6,
///       "amm_config": "AMM配置地址",
///       "current_price": 100.5,
///       "tvl": 1000000.0,
///       "volume_24h": 50000.0,
///       "fee_24h": 150.0,
///       "apr": 15.5,
///       "status": "Active",
///       "created_at": 1640995200
///     }
///   ],
///   "pagination": {
///     "total": 100,
///     "page": 1,
///     "limit": 20,
///     "total_pages": 5
///   }
/// }
/// ```
#[utoipa::path(
    get,
    path = "/api/v1/solana/pools/info/list",
    params(
        ("mint0" = Option<String>, Query, description = "代币0地址"),
        ("mint1" = Option<String>, Query, description = "代币1地址"),
        ("type" = Option<String>, Query, description = "池子类型"),
        ("page" = Option<u32>, Query, description = "页码"),
        ("limit" = Option<u32>, Query, description = "每页数量")
    ),
    responses(
        (status = 200, description = "查询成功", body = crate::dtos::solana_dto::NewPoolListResponse),
        (status = 400, description = "参数错误", body = crate::dtos::solana_dto::NewPoolListResponse),
        (status = 500, description = "查询失败", body = crate::dtos::solana_dto::NewPoolListResponse)
    ),
    tag = "CLMM池子查询"
)]
pub async fn get_pool_list(
    Extension(services): Extension<Services>,
    Query(params): Query<PoolListRequest>,
) -> Result<
    Json<crate::dtos::solana_dto::NewPoolListResponse>,
    (StatusCode, Json<crate::dtos::solana_dto::NewPoolListResponse>),
> {
    info!("🔍 接收到获取池子列表请求");
    if let Some(ref mint_address) = params.mint_address {
        info!("  Mint地址: {}", mint_address);
    }
    if let Some(ref pool_type) = params.pool_type {
        info!("  类型: {}", pool_type);
    }
    info!(
        "  页码: {}, 限制: {}",
        params.page.unwrap_or(1),
        params.page_size.unwrap_or(20)
    );

    match services.solana.query_pools_with_new_format(&params).await {
        Ok(response) => {
            info!("✅ 池子列表查询成功，返回{}个池子", response.data.data.len());
            Ok(Json(response))
        }
        Err(e) => {
            error!("❌ 池子列表查询失败: {:?}", e);
            let error_response = crate::dtos::solana_dto::NewPoolListResponse {
                id: uuid::Uuid::new_v4().to_string(),
                success: false,
                data: crate::dtos::solana_dto::PoolListData {
                    count: 0,
                    data: vec![],
                    has_next_page: false,
                },
            };
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error_response)))
        }
    }
}

/// 根据多个池子地址获取池子列表
///
/// 返回指定池子地址列表的详细信息。
///
/// # 查询参数
///
/// - `ids`: 多个池子地址，用逗号分隔
/// - `type` (可选): 池子类型，值为 `raydium` 或 `all`
/// - `page` (可选): 页码，默认1
/// - `limit` (可选): 每页数量，默认20
///
/// # 响应示例
///
/// ```json
/// {
///   "id": "uuid",
///   "success": true,
///   "data": {
///     "count": 3,
///     "data": [
///       {
///         "pool_address": "EWsjgXuVrcAESbAyBo6Q2JCuuAdotBhp8g7Qhvf8GNek",
///         "mint0": "So11111111111111111111111111111111111111112",
///         "mint1": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
///         "mint0_symbol": "SOL",
///         "mint1_symbol": "USDC",
///         "current_price": 100.5,
///         "tvl": 1000000.0,
///         "volume_24h": 50000.0,
///         "status": "Active"
///       }
///     ],
///     "has_next_page": false
///   }
/// }
/// ```
#[utoipa::path(
    get,
    path = "/api/v1/solana/pools/info/ids",
    params(
        ("ids" = String, Query, description = "多个池子地址，用逗号分隔"),
        ("type" = Option<String>, Query, description = "池子类型"),
        ("page" = Option<u32>, Query, description = "页码"),
        ("limit" = Option<u32>, Query, description = "每页数量")
    ),
    responses(
        (status = 200, description = "查询成功", body = crate::dtos::solana_dto::NewPoolListResponse),
        (status = 400, description = "参数错误", body = crate::dtos::solana_dto::NewPoolListResponse),
        (status = 500, description = "查询失败", body = crate::dtos::solana_dto::NewPoolListResponse)
    ),
    tag = "CLMM池子查询"
)]
pub async fn get_pools_by_ids(
    Extension(services): Extension<Services>,
    Query(params): Query<PoolListRequest>,
) -> Result<
    Json<crate::dtos::solana_dto::NewPoolListResponse2>,
    (StatusCode, Json<crate::dtos::solana_dto::NewPoolListResponse2>),
> {
    info!("🔍 接收到根据IDs查询池子列表请求");
    if let Some(ref ids) = params.ids {
        let ids_count = ids.split(',').filter(|s| !s.trim().is_empty()).count();
        info!("  池子地址数量: {}", ids_count);
        info!("  IDs: {}", ids);
    }
    if let Some(ref pool_type) = params.pool_type {
        info!("  类型: {}", pool_type);
    }
    info!(
        "  页码: {}, 限制: {}",
        params.page.unwrap_or(1),
        params.page_size.unwrap_or(20)
    );

    // 验证必需参数
    let ids = params.ids.clone().ok_or_else(|| {
        let error_response = crate::dtos::solana_dto::NewPoolListResponse2 {
            id: uuid::Uuid::new_v4().to_string(),
            success: false,
            data: vec![],
        };
        (StatusCode::BAD_REQUEST, Json(error_response))
    })?;

    // 验证 ids 参数格式
    if ids.trim().is_empty() {
        let error_response = crate::dtos::solana_dto::NewPoolListResponse2 {
            id: uuid::Uuid::new_v4().to_string(),
            success: false,
            data: vec![],
        };
        return Err((StatusCode::BAD_REQUEST, Json(error_response)));
    }

    // 验证池子地址格式
    let pool_addresses: Vec<&str> = ids.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();

    if pool_addresses.is_empty() {
        let error_response = crate::dtos::solana_dto::NewPoolListResponse2 {
            id: uuid::Uuid::new_v4().to_string(),
            success: false,
            data: vec![],
        };
        return Err((StatusCode::BAD_REQUEST, Json(error_response)));
    }

    // 限制一次查询的池子数量，防止过大查询
    if pool_addresses.len() > 100 {
        let error_response = crate::dtos::solana_dto::NewPoolListResponse2 {
            id: uuid::Uuid::new_v4().to_string(),
            success: false,
            data: vec![],
        };
        return Err((StatusCode::BAD_REQUEST, Json(error_response)));
    }

    // 验证每个地址的格式（基本长度检查）
    for addr in &pool_addresses {
        if addr.len() < 32 || addr.len() > 44 {
            let error_response = crate::dtos::solana_dto::NewPoolListResponse2 {
                id: uuid::Uuid::new_v4().to_string(),
                success: false,
                data: vec![],
            };
            return Err((StatusCode::BAD_REQUEST, Json(error_response)));
        }
    }

    match services.solana.query_pools_with_new_format2(&params).await {
        Ok(response) => {
            info!("✅ 根据IDs查询池子成功，返回{}个池子", response.data.len());
            Ok(Json(response))
        }
        Err(e) => {
            error!("❌ 根据IDs查询池子失败: {:?}", e);
            let error_response = crate::dtos::solana_dto::NewPoolListResponse2 {
                id: uuid::Uuid::new_v4().to_string(),
                success: false,
                data: vec![],
            };
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error_response)))
        }
    }
}

/// 根据代币对获取池子列表
///
/// 返回包含指定代币对的所有池子信息。
///
/// # 查询参数
///
/// - `mint0`: 代币0地址
/// - `mint1`: 代币1地址
/// - `type` (可选): 池子类型，值为 `raydium` 或 `all`
/// - `page` (可选): 页码，默认1
/// - `limit` (可选): 每页数量，默认20
///
/// # 响应示例
///
/// ```json
/// {
///   "status": 200,
///   "message": "success",
///   "data": [
///     {
///       "pool_address": "池子地址",
///       "mint0": "So11111111111111111111111111111111111111112",
///       "mint1": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
///       "mint0_symbol": "SOL",
///       "mint1_symbol": "USDC",
///       "mint0_decimal": 9,
///       "mint1_decimal": 6,
///       "amm_config": "AMM配置地址",
///       "current_price": 100.5,
///       "tvl": 1000000.0,
///       "volume_24h": 50000.0,
///       "fee_24h": 150.0,
///       "apr": 15.5,
///       "status": "Active",
///       "created_at": 1640995200
///     }
///   ],
///   "pagination": {
///     "total": 10,
///     "page": 1,
///     "limit": 20,
///     "total_pages": 1
///   }
/// }
/// ```
#[utoipa::path(
    get,
    path = "/api/v1/solana/pools/info/mint",
    params(
        ("mint0" = String, Query, description = "代币0地址"),
        ("mint1" = String, Query, description = "代币1地址"),
        ("type" = Option<String>, Query, description = "池子类型"),
        ("page" = Option<u32>, Query, description = "页码"),
        ("limit" = Option<u32>, Query, description = "每页数量")
    ),
    responses(
        (status = 200, description = "查询成功", body = crate::dtos::solana_dto::NewPoolListResponse),
        (status = 400, description = "参数错误", body = crate::dtos::solana_dto::NewPoolListResponse),
        (status = 500, description = "查询失败", body = crate::dtos::solana_dto::NewPoolListResponse)
    ),
    tag = "CLMM池子查询"
)]
pub async fn get_pools_by_mint_pair(
    Extension(services): Extension<Services>,
    Query(params): Query<PoolListRequest>,
) -> Result<
    Json<crate::dtos::solana_dto::NewPoolListResponse>,
    (StatusCode, Json<crate::dtos::solana_dto::NewPoolListResponse>),
> {
    info!("🔍 接收到代币对池子查询请求");
    info!("  Mint1: {:?}", params.mint1);
    info!("  Mint2: {:?}", params.mint2);
    info!("  池子类型: {:?}", params.pool_type);
    info!("  排序字段: {:?}", params.pool_sort_field);
    info!("  排序方向: {:?}", params.sort_type);
    info!(
        "  页码: {}, 页大小: {}",
        params.page.unwrap_or(1),
        params.page_size.unwrap_or(20)
    );

    // 验证必需参数
    let mint1 = params.mint1.clone().ok_or_else(|| {
        let error_response = crate::dtos::solana_dto::NewPoolListResponse {
            id: uuid::Uuid::new_v4().to_string(),
            success: false,
            data: crate::dtos::solana_dto::PoolListData {
                count: 0,
                data: vec![],
                has_next_page: false,
            },
        };
        (StatusCode::BAD_REQUEST, Json(error_response))
    })?;

    let mint2 = params.mint2.clone().ok_or_else(|| {
        let error_response = crate::dtos::solana_dto::NewPoolListResponse {
            id: uuid::Uuid::new_v4().to_string(),
            success: false,
            data: crate::dtos::solana_dto::PoolListData {
                count: 0,
                data: vec![],
                has_next_page: false,
            },
        };
        (StatusCode::BAD_REQUEST, Json(error_response))
    })?;

    // 验证mint地址格式
    if mint1.len() < 32 || mint1.len() > 44 {
        let error_response = crate::dtos::solana_dto::NewPoolListResponse {
            id: uuid::Uuid::new_v4().to_string(),
            success: false,
            data: crate::dtos::solana_dto::PoolListData {
                count: 0,
                data: vec![],
                has_next_page: false,
            },
        };
        return Err((StatusCode::BAD_REQUEST, Json(error_response)));
    }

    if mint2.len() < 32 || mint2.len() > 44 {
        let error_response = crate::dtos::solana_dto::NewPoolListResponse {
            id: uuid::Uuid::new_v4().to_string(),
            success: false,
            data: crate::dtos::solana_dto::PoolListData {
                count: 0,
                data: vec![],
                has_next_page: false,
            },
        };
        return Err((StatusCode::BAD_REQUEST, Json(error_response)));
    }

    // 验证两个mint不能相同
    if mint1 == mint2 {
        let error_response = crate::dtos::solana_dto::NewPoolListResponse {
            id: uuid::Uuid::new_v4().to_string(),
            success: false,
            data: crate::dtos::solana_dto::PoolListData {
                count: 0,
                data: vec![],
                has_next_page: false,
            },
        };
        return Err((StatusCode::BAD_REQUEST, Json(error_response)));
    }

    match services.solana.query_pools_with_new_format(&params).await {
        Ok(response) => {
            info!("✅ 代币对池子查询成功，返回{}个池子", response.data.data.len());
            Ok(Json(response))
        }
        Err(e) => {
            error!("❌ 代币对池子查询失败: {:?}", e);
            let error_response = crate::dtos::solana_dto::NewPoolListResponse {
                id: uuid::Uuid::new_v4().to_string(),
                success: false,
                data: crate::dtos::solana_dto::PoolListData {
                    count: 0,
                    data: vec![],
                    has_next_page: false,
                },
            };
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error_response)))
        }
    }
}

/// 根据多个池子ID获取池子密钥信息
///
/// 返回指定池子ID列表的完整Raydium密钥信息，包含程序ID、代币信息、金库、配置等。
///
/// # 查询参数
///
/// - `ids`: 多个池子地址，用逗号分隔
///
/// # 响应示例
///
/// ```json
/// {
///   "id": "uuid",
///   "success": true,
///   "data": [
///     {
///       "programId": "CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK",
///       "id": "EWsjgXuVrcAESbAyBo6Q2JCuuAdotBhp8g7Qhvf8GNek",
///       "mintA": {
///         "chainId": 101,
///         "address": "CF1Ms9vjvGEiSHqoj1jLadoLNXD9EqtnR6TZp1w8CeHz",
///         "programId": "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",
///         "logoURI": "https://img-v1.raydium.io/icon/CF1Ms9vjvGEiSHqoj1jLadoLNXD9EqtnR6TZp1w8CeHz.png",
///         "symbol": "FAIR",
///         "name": "Coinfair's Coin",
///         "decimals": 9,
///         "tags": ["hasFreeze"],
///         "extensions": {}
///       },
///       "mintB": {
///         "chainId": 101,
///         "address": "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB",
///         "programId": "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",
///         "logoURI": "https://img-v1.raydium.io/icon/Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB.png",
///         "symbol": "USDT",
///         "name": "USDT",
///         "decimals": 6,
///         "tags": ["hasFreeze"],
///         "extensions": {}
///       },
///       "lookupTableAccount": "GSZngJkhWZsKFdXax7AGGaXSemifVnsv5ZaMyzzQVSMt",
///       "openTime": "0",
///       "vault": {
///         "A": "4C3rJaRkP5WMdNtFKeGKeYWndTUrYCrboHTJEComysaw",
///         "B": "GYGAw7n5vwyNM9ykLhQBvxF1FEu81AVgdNAPFZ25HKjw"
///       },
///       "config": {
///         "id": "E64NGkDLLCdQ2yFNPcavaKptrEgmiQaNykUuLC1Qgwyp",
///         "index": 1,
///         "protocolFeeRate": 120000,
///         "tradeFeeRate": 2500,
///         "tickSpacing": 60,
///         "fundFeeRate": 40000,
///         "defaultRange": 0.1,
///         "defaultRangePoint": [0.01, 0.05, 0.1, 0.2, 0.5]
///       },
///       "rewardInfos": [],
///       "observationId": "9EofKmSNgY6s3bb1DgS5DaqXLZXUK6dK7oRJRBRPjh76",
///       "exBitmapAccount": "3HJ1hfiLFjM1Lvt2eD3P4whttuxV2ihKnVADqVPDuX5g"
///     },
///     null
///   ]
/// }
/// ```
#[utoipa::path(
    get,
    path = "/api/v1/solana/pools/key/ids",
    params(
        ("ids" = String, Query, description = "多个池子地址，用逗号分隔")
    ),
    responses(
        (status = 200, description = "查询成功", body = crate::dtos::solana_dto::PoolKeyResponse),
        (status = 400, description = "参数错误", body = crate::dtos::solana_dto::PoolKeyResponse),
        (status = 500, description = "查询失败", body = crate::dtos::solana_dto::PoolKeyResponse)
    ),
    tag = "CLMM池子查询"
)]
pub async fn get_pools_key_by_ids(
    Extension(services): Extension<Services>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<crate::dtos::solana_dto::PoolKeyResponse>, (StatusCode, Json<crate::dtos::solana_dto::PoolKeyResponse>)>
{
    info!("🔍 接收到池子密钥查询请求");

    // 验证必需参数
    let ids = params.get("ids").ok_or_else(|| {
        let error_response = crate::dtos::solana_dto::PoolKeyResponse {
            id: uuid::Uuid::new_v4().to_string(),
            success: false,
            data: vec![],
        };
        (StatusCode::BAD_REQUEST, Json(error_response))
    })?;

    // 验证 ids 参数格式
    if ids.trim().is_empty() {
        let error_response = crate::dtos::solana_dto::PoolKeyResponse {
            id: uuid::Uuid::new_v4().to_string(),
            success: false,
            data: vec![],
        };
        return Err((StatusCode::BAD_REQUEST, Json(error_response)));
    }

    // 解析池子地址
    let pool_addresses: Vec<String> = ids
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    if pool_addresses.is_empty() {
        let error_response = crate::dtos::solana_dto::PoolKeyResponse {
            id: uuid::Uuid::new_v4().to_string(),
            success: false,
            data: vec![],
        };
        return Err((StatusCode::BAD_REQUEST, Json(error_response)));
    }

    // 限制一次查询的池子数量，防止过大查询
    if pool_addresses.len() > 100 {
        let error_response = crate::dtos::solana_dto::PoolKeyResponse {
            id: uuid::Uuid::new_v4().to_string(),
            success: false,
            data: vec![],
        };
        return Err((StatusCode::BAD_REQUEST, Json(error_response)));
    }

    // 验证每个地址的格式（基本长度检查）
    for addr in &pool_addresses {
        if addr.len() < 32 || addr.len() > 44 {
            let error_response = crate::dtos::solana_dto::PoolKeyResponse {
                id: uuid::Uuid::new_v4().to_string(),
                success: false,
                data: vec![],
            };
            return Err((StatusCode::BAD_REQUEST, Json(error_response)));
        }
    }

    info!("  池子地址数量: {}", pool_addresses.len());
    info!("  IDs: {:?}", pool_addresses);

    match services.solana.get_pools_key_by_ids(pool_addresses).await {
        Ok(response) => {
            info!("✅ 池子密钥查询成功，返回{}个结果", response.data.len());
            Ok(Json(response))
        }
        Err(e) => {
            error!("❌ 池子密钥查询失败: {:?}", e);
            let error_response = crate::dtos::solana_dto::PoolKeyResponse {
                id: uuid::Uuid::new_v4().to_string(),
                success: false,
                data: vec![],
            };
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error_response)))
        }
    }
}
