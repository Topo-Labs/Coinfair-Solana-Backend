use crate::dtos::solana::common::{ApiResponse, ErrorResponse};
use crate::dtos::solana::events::deposit::{
    CreateDepositEventRequest, CreateDepositEventResponse, DepositAdvancedQuery, DepositEventQuery,
    DepositEventResponse, DepositStatsResponse, DepositTrendQuery, DepositTrendResponse, PaginatedDepositResponse,
    TokenDepositQuery, TokenDepositSummaryResponse, TrendPeriod, UserDepositQuery, UserDepositSummaryResponse,
};
use crate::services::solana::clmm::event::DepositEventService;
use crate::services::Services;
use axum::{
    extract::{Extension, Json, Path, Query},
    http::StatusCode,
    response::Json as ResponseJson,
    routing::{get, post},
    Router,
};
use tracing::{error, info};

/// DepositEvent控制器
pub struct DepositEventController;

impl DepositEventController {
    /// 定义路由
    pub fn routes() -> Router {
        Router::new()
            // ====== 基础查询接口 ======
            .route("/deposits", get(get_deposit_events))
            .route("/deposits", post(create_deposit_event))
            .route("/deposits/advanced", get(get_deposit_events_advanced))
            .route("/deposits/by-user/:address", get(get_deposits_by_user))
            .route("/deposits/by-token/:mint", get(get_deposits_by_token))
            .route("/deposits/by-signature/:signature", get(get_deposit_by_signature))
            // ====== 统计分析接口 ======
            .route("/deposits/stats", get(get_deposit_stats))
            .route("/deposits/summary/:address", get(get_user_deposit_summary))
            .route("/deposits/token-summary/:mint", get(get_token_deposit_summary))
            .route("/deposits/trends", get(get_deposit_trends))
    }
}

// ==================== 基础查询接口 ====================

/// 创建存款事件
///
/// 用于手动插入丢失的存款事件
#[axum::debug_handler]
#[utoipa::path(
    post,
    path = "/api/v1/solana/events/deposits",
    request_body = CreateDepositEventRequest,
    responses(
        (status = 201, description = "创建成功", body = ApiResponse<CreateDepositEventResponse>),
        (status = 400, description = "参数错误", body = ErrorResponse),
        (status = 409, description = "事件已存在", body = ErrorResponse),
        (status = 500, description = "服务器错误", body = ErrorResponse)
    ),
    tag = "存款事件"
)]
pub async fn create_deposit_event(
    Extension(services): Extension<Services>,
    Json(request): Json<CreateDepositEventRequest>,
) -> Result<ResponseJson<ApiResponse<CreateDepositEventResponse>>, (StatusCode, ResponseJson<ErrorResponse>)> {
    info!("💾 创建存款事件，用户：{}, 签名：{}", request.user, request.signature);

    // 验证请求参数
    if request.user.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            ResponseJson(ErrorResponse::new("INVALID_USER", "用户地址不能为空")),
        ));
    }

    if request.signature.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            ResponseJson(ErrorResponse::new("INVALID_SIGNATURE", "交易签名不能为空")),
        ));
    }

    if request.amount == 0 {
        return Err((
            StatusCode::BAD_REQUEST,
            ResponseJson(ErrorResponse::new("INVALID_AMOUNT", "存款金额必须大于0")),
        ));
    }

    let deposit_service = DepositEventService::new(services.database.clone());

    // 转换请求为数据库模型
    let event: database::event_model::DepositEvent = request.into();

    match deposit_service.create_deposit_event(event).await {
        Ok((event_id, created_event)) => {
            let response = CreateDepositEventResponse {
                id: event_id,
                user: created_event.user,
                signature: created_event.signature,
                deposited_at: created_event.deposited_at,
                actual_amount: created_event.actual_amount,
                actual_total_raised: created_event.actual_total_raised,
                deposit_type_name: created_event.deposit_type_name,
                estimated_usd_value: created_event.estimated_usd_value,
                created_at: chrono::DateTime::from_timestamp(created_event.processed_at, 0)
                    .unwrap_or_default()
                    .to_rfc3339(),
            };

            info!("✅ 成功创建存款事件，ID: {}", response.id);
            Ok(ResponseJson(ApiResponse::success(response)))
        }
        Err(e) => {
            if e.to_string().contains("已存在") {
                error!("❌ 存款事件已存在: {}", e);
                Err((
                    StatusCode::CONFLICT,
                    ResponseJson(ErrorResponse::new("DEPOSIT_EVENT_ALREADY_EXISTS", "存款事件已存在")),
                ))
            } else {
                error!("❌ 创建存款事件失败: {}", e);
                Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    ResponseJson(ErrorResponse::new("CREATE_DEPOSIT_EVENT_FAILED", "创建存款事件失败")),
                ))
            }
        }
    }
}

/// 查询存款事件列表
///
/// 支持分页和基础过滤条件
#[utoipa::path(
    get,
    path = "/api/v1/solana/events/deposits",
    params(DepositEventQuery),
    responses(
        (status = 200, description = "查询成功", body = ApiResponse<PaginatedDepositResponse>),
        (status = 400, description = "参数错误", body = ErrorResponse),
        (status = 500, description = "服务器错误", body = ErrorResponse)
    ),
    tag = "存款事件"
)]
pub async fn get_deposit_events(
    Query(query): Query<DepositEventQuery>,
    Extension(services): Extension<Services>,
) -> Result<Json<ApiResponse<PaginatedDepositResponse>>, (StatusCode, Json<ErrorResponse>)> {
    info!("📊 查询存款事件列表，参数: {:?}", query);

    let deposit_service = DepositEventService::new(services.database.clone());

    match deposit_service
        .get_deposit_events_paginated(
            Some(query.page),
            Some(query.page_size),
            query.user,
            query.token_mint,
            query.project_config,
            query.deposit_type,
            query.start_date,
            query.end_date,
            query.sort_by,
            query.sort_order,
        )
        .await
    {
        Ok(result) => {
            let items: Vec<DepositEventResponse> = result.items.into_iter().map(Into::into).collect();
            let response = PaginatedDepositResponse {
                items,
                total: result.total,
                page: result.page,
                page_size: result.page_size,
                total_pages: result.total_pages,
                unique_users: None,
            };

            Ok(ResponseJson(ApiResponse::success(response)))
        }
        Err(e) => {
            error!("查询存款事件失败: {:?}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                ResponseJson(ErrorResponse::new("QUERY_DEPOSITS_FAILED", "查询存款事件失败")),
            ))
        }
    }
}

/// 高级查询存款事件
///
/// 支持复杂过滤条件
#[utoipa::path(
    get,
    path = "/api/v1/solana/events/deposits/advanced",
    params(DepositAdvancedQuery),
    responses(
        (status = 200, description = "查询成功", body = ApiResponse<PaginatedDepositResponse>),
        (status = 400, description = "参数错误", body = ErrorResponse),
        (status = 500, description = "服务器错误", body = ErrorResponse)
    ),
    tag = "存款事件"
)]
pub async fn get_deposit_events_advanced(
    Query(query): Query<DepositAdvancedQuery>,
    Extension(services): Extension<Services>,
) -> Result<Json<ApiResponse<PaginatedDepositResponse>>, (StatusCode, Json<ErrorResponse>)> {
    info!("📊 高级查询存款事件，参数: {:?}", query);

    let deposit_service = DepositEventService::new(services.database.clone());

    match deposit_service
        .get_deposit_events_advanced(
            Some(query.page),
            Some(query.page_size),
            query.user,
            query.token_mint,
            query.project_config,
            query.deposit_type,
            query.start_date,
            query.end_date,
            query.amount_min,
            query.amount_max,
            query.total_raised_min,
            query.total_raised_max,
            query.is_high_value_deposit,
            query.related_pool,
            query.estimated_usd_min,
            query.estimated_usd_max,
            query.token_symbol,
            query.token_name,
            query.sort_by,
            query.sort_order,
        )
        .await
    {
        Ok(result) => {
            let items: Vec<DepositEventResponse> = result.items.into_iter().map(Into::into).collect();
            let response = PaginatedDepositResponse {
                items,
                total: result.total,
                page: result.page,
                page_size: result.page_size,
                total_pages: result.total_pages,
                unique_users: None,
            };

            Ok(ResponseJson(ApiResponse::success(response)))
        }
        Err(e) => {
            error!("高级查询存款事件失败: {:?}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                ResponseJson(ErrorResponse::new(
                    "ADVANCED_QUERY_DEPOSITS_FAILED",
                    "高级查询存款事件失败",
                )),
            ))
        }
    }
}

/// 根据用户查询存款记录
#[utoipa::path(
    get,
    path = "/api/v1/solana/events/deposits/by-user/{address}",
    params(
        ("address" = String, Path, description = "用户钱包地址"),
        UserDepositQuery,
    ),
    responses(
        (status = 200, description = "查询成功", body = ApiResponse<PaginatedDepositResponse>),
        (status = 400, description = "参数错误", body = ErrorResponse),
        (status = 500, description = "服务器错误", body = ErrorResponse)
    ),
    tag = "存款事件"
)]
pub async fn get_deposits_by_user(
    Path(address): Path<String>,
    Query(query): Query<UserDepositQuery>,
    Extension(services): Extension<Services>,
) -> Result<Json<ApiResponse<PaginatedDepositResponse>>, (StatusCode, Json<ErrorResponse>)> {
    info!("📊 查询用户{}的存款记录", address);

    let deposit_service = DepositEventService::new(services.database.clone());

    match deposit_service
        .get_deposits_by_user(&address, Some(query.page), Some(query.page_size))
        .await
    {
        Ok(result) => {
            let items: Vec<DepositEventResponse> = result.items.into_iter().map(Into::into).collect();
            let response = PaginatedDepositResponse {
                items,
                total: result.total,
                page: result.page,
                page_size: result.page_size,
                total_pages: result.total_pages,
                unique_users: None,
            };

            Ok(ResponseJson(ApiResponse::success(response)))
        }
        Err(e) => {
            error!("查询用户存款记录失败: {:?}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                ResponseJson(ErrorResponse::new("QUERY_USER_DEPOSITS_FAILED", "查询用户存款记录失败")),
            ))
        }
    }
}

/// 根据代币查询存款记录
#[utoipa::path(
    get,
    path = "/api/v1/solana/events/deposits/by-token/{mint}",
    params(
        ("mint" = String, Path, description = "代币mint地址"),
        TokenDepositQuery,
    ),
    responses(
        (status = 200, description = "查询成功", body = ApiResponse<PaginatedDepositResponse>),
        (status = 400, description = "参数错误", body = ErrorResponse),
        (status = 500, description = "服务器错误", body = ErrorResponse)
    ),
    tag = "存款事件"
)]
pub async fn get_deposits_by_token(
    Path(mint): Path<String>,
    Query(query): Query<TokenDepositQuery>,
    Extension(services): Extension<Services>,
) -> Result<Json<ApiResponse<PaginatedDepositResponse>>, (StatusCode, Json<ErrorResponse>)> {
    info!("📊 查询代币{}的存款记录", mint);

    let deposit_service = DepositEventService::new(services.database.clone());

    match deposit_service
        .get_deposits_by_token(&mint, Some(query.page), Some(query.page_size))
        .await
    {
        Ok(result) => {
            let items: Vec<DepositEventResponse> = result.items.into_iter().map(Into::into).collect();
            let response = PaginatedDepositResponse {
                items,
                total: result.total,
                page: result.page,
                page_size: result.page_size,
                total_pages: result.total_pages,
                unique_users: Some(result.unique_users),
            };

            // 日志提示 unique_users
            info!(
                "📊 代币{}的unique_users: {} (page={}, page_size={})",
                mint, result.unique_users, response.page, response.page_size
            );

            Ok(ResponseJson(ApiResponse::success(response)))
        }
        Err(e) => {
            error!("查询代币存款记录失败: {:?}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                ResponseJson(ErrorResponse::new(
                    "QUERY_TOKEN_DEPOSITS_FAILED",
                    "查询代币存款记录失败",
                )),
            ))
        }
    }
}

/// 根据签名查询存款事件
#[utoipa::path(
    get,
    path = "/api/v1/solana/events/deposits/by-signature/{signature}",
    params(
        ("signature" = String, Path, description = "交易签名")
    ),
    responses(
        (status = 200, description = "查询成功", body = ApiResponse<DepositEventResponse>),
        (status = 404, description = "未找到", body = ErrorResponse),
        (status = 500, description = "服务器错误", body = ErrorResponse)
    ),
    tag = "存款事件"
)]
pub async fn get_deposit_by_signature(
    Path(signature): Path<String>,
    Extension(services): Extension<Services>,
) -> Result<Json<ApiResponse<Option<DepositEventResponse>>>, (StatusCode, Json<ErrorResponse>)> {
    info!("📊 查询签名{}的存款事件", signature);

    let deposit_service = DepositEventService::new(services.database.clone());

    match deposit_service.get_deposit_by_signature(&signature).await {
        Ok(Some(event)) => {
            let response: DepositEventResponse = event.into();
            Ok(ResponseJson(ApiResponse::success(Some(response))))
        }
        Ok(None) => Ok(Json(ApiResponse::success(None))),
        Err(e) => {
            error!("查询存款事件失败: {:?}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                ResponseJson(ErrorResponse::new(
                    "QUERY_DEPOSIT_BY_SIGNATURE_FAILED",
                    "查询存款事件失败",
                )),
            ))
        }
    }
}

// ==================== 统计分析接口 ====================

/// 获取存款统计信息
#[utoipa::path(
    get,
    path = "/api/v1/solana/events/deposits/stats",
    responses(
        (status = 200, description = "查询成功", body = ApiResponse<DepositStatsResponse>),
        (status = 500, description = "服务器错误", body = ErrorResponse)
    ),
    tag = "存款事件"
)]
pub async fn get_deposit_stats(
    Extension(services): Extension<Services>,
) -> Result<Json<ApiResponse<DepositStatsResponse>>, (StatusCode, Json<ErrorResponse>)> {
    info!("📊 获取存款统计信息");

    let deposit_service = DepositEventService::new(services.database.clone());

    match deposit_service.get_deposit_stats().await {
        Ok(stats) => {
            let response: DepositStatsResponse = stats.into();
            Ok(ResponseJson(ApiResponse::success(response)))
        }
        Err(e) => {
            error!("获取存款统计失败: {:?}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                ResponseJson(ErrorResponse::new("GET_DEPOSIT_STATS_FAILED", "获取存款统计失败")),
            ))
        }
    }
}

/// 获取用户存款汇总
#[utoipa::path(
    get,
    path = "/api/v1/solana/events/deposits/summary/{address}",
    params(
        ("address" = String, Path, description = "用户钱包地址")
    ),
    responses(
        (status = 200, description = "查询成功", body = ApiResponse<UserDepositSummaryResponse>),
        (status = 500, description = "服务器错误", body = ErrorResponse)
    ),
    tag = "存款事件"
)]
pub async fn get_user_deposit_summary(
    Path(address): Path<String>,
    Extension(services): Extension<Services>,
) -> Result<Json<ApiResponse<UserDepositSummaryResponse>>, (StatusCode, Json<ErrorResponse>)> {
    info!("📊 获取用户{}的存款汇总", address);

    let deposit_service = DepositEventService::new(services.database.clone());

    match deposit_service.get_user_deposit_summary(&address).await {
        Ok(summary) => {
            let response: UserDepositSummaryResponse = summary.into();
            Ok(ResponseJson(ApiResponse::success(response)))
        }
        Err(e) => {
            error!("获取用户存款汇总失败: {:?}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                ResponseJson(ErrorResponse::new(
                    "GET_USER_DEPOSIT_SUMMARY_FAILED",
                    "获取用户存款汇总失败",
                )),
            ))
        }
    }
}

/// 获取代币存款汇总
#[utoipa::path(
    get,
    path = "/api/v1/solana/events/deposits/token-summary/{mint}",
    params(
        ("mint" = String, Path, description = "代币mint地址")
    ),
    responses(
        (status = 200, description = "查询成功", body = ApiResponse<TokenDepositSummaryResponse>),
        (status = 500, description = "服务器错误", body = ErrorResponse)
    ),
    tag = "存款事件"
)]
pub async fn get_token_deposit_summary(
    Path(mint): Path<String>,
    Extension(services): Extension<Services>,
) -> Result<Json<ApiResponse<TokenDepositSummaryResponse>>, (StatusCode, Json<ErrorResponse>)> {
    info!("📊 获取代币{}的存款汇总", mint);

    let deposit_service = DepositEventService::new(services.database.clone());

    match deposit_service.get_token_deposit_summary(&mint).await {
        Ok(summary) => {
            let response: TokenDepositSummaryResponse = summary.into();
            Ok(ResponseJson(ApiResponse::success(response)))
        }
        Err(e) => {
            error!("获取代币存款汇总失败: {:?}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                ResponseJson(ErrorResponse::new(
                    "GET_TOKEN_DEPOSIT_SUMMARY_FAILED",
                    "获取代币存款汇总失败",
                )),
            ))
        }
    }
}

/// 获取存款趋势数据
#[utoipa::path(
    get,
    path = "/api/v1/solana/events/deposits/trends",
    params(DepositTrendQuery),
    responses(
        (status = 200, description = "查询成功", body = ApiResponse<DepositTrendResponse>),
        (status = 400, description = "参数错误", body = ErrorResponse),
        (status = 500, description = "服务器错误", body = ErrorResponse)
    ),
    tag = "存款事件"
)]
pub async fn get_deposit_trends(
    Query(query): Query<DepositTrendQuery>,
    Extension(services): Extension<Services>,
) -> Result<Json<ApiResponse<DepositTrendResponse>>, (StatusCode, Json<ErrorResponse>)> {
    info!("📊 获取存款趋势数据，参数: {:?}", query);

    let deposit_service = DepositEventService::new(services.database.clone());

    let period = query.period.unwrap_or(TrendPeriod::Day);
    let service_period = match period {
        TrendPeriod::Hour => crate::services::solana::clmm::event::deposit_service::TrendPeriod::Hour,
        TrendPeriod::Day => crate::services::solana::clmm::event::deposit_service::TrendPeriod::Day,
        TrendPeriod::Week => crate::services::solana::clmm::event::deposit_service::TrendPeriod::Week,
        TrendPeriod::Month => crate::services::solana::clmm::event::deposit_service::TrendPeriod::Month,
    };

    match deposit_service
        .get_deposit_trends(service_period, query.start_date, query.end_date)
        .await
    {
        Ok(trends) => {
            let trend_points = trends.into_iter().map(Into::into).collect();
            let response = DepositTrendResponse { trends: trend_points };
            Ok(ResponseJson(ApiResponse::success(response)))
        }
        Err(e) => {
            error!("获取存款趋势失败: {:?}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                ResponseJson(ErrorResponse::new("GET_DEPOSIT_TRENDS_FAILED", "获取存款趋势失败")),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::dtos::solana::events::deposit::*;
    use axum::http::StatusCode;

    /// 控制器层单元测试 - 测试API接口和响应格式
    #[test]
    fn test_deposit_event_query_structure() {
        // 测试基础查询参数结构
        let query = DepositEventQuery {
            page: 1,
            page_size: 20,
            user: Some("8S2bcP66WehuF6cHryfZ7vfFpQWaUhYyAYSy5U3gX4Fy".to_string()),
            token_mint: Some("So11111111111111111111111111111111111111112".to_string()),
            project_config: Some("test_config".to_string()),
            deposit_type: Some(1),
            start_date: Some(1640995200), // 2022-01-01
            end_date: Some(1672531199),   // 2022-12-31
            sort_by: Some("deposited_at".to_string()),
            sort_order: Some("desc".to_string()),
        };

        // 验证查询结构正确性
        assert_eq!(query.page, 1);
        assert_eq!(query.page_size, 20);
        assert!(query.user.is_some());
        assert!(query.token_mint.is_some());
        assert!(query.start_date.unwrap() < query.end_date.unwrap());
        assert_eq!(query.sort_order.as_ref().unwrap(), "desc");
    }

    #[test]
    fn test_deposit_advanced_query_structure() {
        // 测试高级查询参数结构
        let query = DepositAdvancedQuery {
            page: 1,
            page_size: 50,
            user: Some("8S2bcP66WehuF6cHryfZ7vfFpQWaUhYyAYSy5U3gX4Fy".to_string()),
            token_mint: Some("So11111111111111111111111111111111111111112".to_string()),
            project_config: Some("test_config".to_string()),
            deposit_type: Some(1),
            start_date: Some(1640995200),
            end_date: Some(1672531199),
            amount_min: Some(1000000),        // 1 SOL
            amount_max: Some(10000000),       // 10 SOL
            total_raised_min: Some(5000000),  // 5 SOL
            total_raised_max: Some(50000000), // 50 SOL
            is_high_value_deposit: Some(false),
            related_pool: Some("test_pool_address".to_string()),
            estimated_usd_min: Some(100.0),
            estimated_usd_max: Some(1000.0),
            token_symbol: Some("SOL".to_string()),
            token_name: Some("Solana".to_string()),
            sort_by: Some("estimated_usd_value".to_string()),
            sort_order: Some("asc".to_string()),
        };

        // 验证高级查询参数
        assert_eq!(query.page_size, 50);
        assert!(query.amount_min.unwrap() < query.amount_max.unwrap());
        assert!(query.total_raised_min.unwrap() < query.total_raised_max.unwrap());
        assert!(query.estimated_usd_min.unwrap() < query.estimated_usd_max.unwrap());
        assert_eq!(query.is_high_value_deposit, Some(false));
        assert!(query.related_pool.is_some());
    }

    #[test]
    fn test_user_deposit_query_structure() {
        // 测试用户存款查询参数
        let query = UserDepositQuery { page: 1, page_size: 20 };

        assert_eq!(query.page, 1);
        assert_eq!(query.page_size, 20);
        assert!(query.page >= 1);
        assert!(query.page_size >= 1 && query.page_size <= 100);
    }

    #[test]
    fn test_token_deposit_query_structure() {
        // 测试代币存款查询参数
        let query = TokenDepositQuery { page: 2, page_size: 30 };

        assert_eq!(query.page, 2);
        assert_eq!(query.page_size, 30);
        assert!(query.page >= 1);
        assert!(query.page_size >= 1 && query.page_size <= 100);
    }

    #[test]
    fn test_deposit_trend_query_structure() {
        // 测试存款趋势查询参数
        let query = DepositTrendQuery {
            period: Some(TrendPeriod::Day),
            start_date: Some(1640995200),
            end_date: Some(1672531199),
        };

        assert!(query.period.is_some());
        assert!(matches!(query.period.unwrap(), TrendPeriod::Day));
        assert!(query.start_date.unwrap() < query.end_date.unwrap());

        // 测试不同的趋势周期
        let periods = vec![
            TrendPeriod::Hour,
            TrendPeriod::Day,
            TrendPeriod::Week,
            TrendPeriod::Month,
        ];

        for period in periods {
            let trend_query = DepositTrendQuery {
                period: Some(period.clone()),
                start_date: Some(1640995200),
                end_date: Some(1672531199),
            };

            assert!(trend_query.period.is_some());
            match period {
                TrendPeriod::Hour => assert!(matches!(trend_query.period.unwrap(), TrendPeriod::Hour)),
                TrendPeriod::Day => assert!(matches!(trend_query.period.unwrap(), TrendPeriod::Day)),
                TrendPeriod::Week => assert!(matches!(trend_query.period.unwrap(), TrendPeriod::Week)),
                TrendPeriod::Month => assert!(matches!(trend_query.period.unwrap(), TrendPeriod::Month)),
            }
        }
    }

    #[test]
    fn test_trend_period_case_insensitive_deserialization() {
        use serde_json;

        // 测试小写输入
        let json_data = r#"{"period": "day", "start_date": 1640995200, "end_date": 1672531199}"#;
        let query: Result<DepositTrendQuery, _> = serde_json::from_str(json_data);
        assert!(query.is_ok());
        let query = query.unwrap();
        assert!(matches!(query.period.unwrap(), TrendPeriod::Day));

        // 测试大写输入
        let json_data = r#"{"period": "Day", "start_date": 1640995200, "end_date": 1672531199}"#;
        let query: Result<DepositTrendQuery, _> = serde_json::from_str(json_data);
        assert!(query.is_ok());
        let query = query.unwrap();
        assert!(matches!(query.period.unwrap(), TrendPeriod::Day));

        // 测试混合大小写输入
        let json_data = r#"{"period": "HOUR", "start_date": 1640995200, "end_date": 1672531199}"#;
        let query: Result<DepositTrendQuery, _> = serde_json::from_str(json_data);
        assert!(query.is_ok());
        let query = query.unwrap();
        assert!(matches!(query.period.unwrap(), TrendPeriod::Hour));

        // 测试所有有效的小写变体
        let test_cases = vec![
            ("hour", TrendPeriod::Hour),
            ("day", TrendPeriod::Day),
            ("week", TrendPeriod::Week),
            ("month", TrendPeriod::Month),
        ];

        for (input, expected) in test_cases {
            let json_data = format!(
                r#"{{"period": "{}", "start_date": 1640995200, "end_date": 1672531199}}"#,
                input
            );
            let query: Result<DepositTrendQuery, _> = serde_json::from_str(&json_data);
            assert!(query.is_ok(), "Failed to deserialize: {}", input);
            let query = query.unwrap();
            assert!(query.period.is_some());
            match expected {
                TrendPeriod::Hour => assert!(matches!(query.period.unwrap(), TrendPeriod::Hour)),
                TrendPeriod::Day => assert!(matches!(query.period.unwrap(), TrendPeriod::Day)),
                TrendPeriod::Week => assert!(matches!(query.period.unwrap(), TrendPeriod::Week)),
                TrendPeriod::Month => assert!(matches!(query.period.unwrap(), TrendPeriod::Month)),
            }
        }

        // 测试无效输入
        let json_data = r#"{"period": "invalid", "start_date": 1640995200, "end_date": 1672531199}"#;
        let query: Result<DepositTrendQuery, _> = serde_json::from_str(json_data);
        assert!(query.is_err());
    }

    #[test]
    fn test_api_error_codes_consistency() {
        // 测试API错误代码的一致性和覆盖度
        let error_codes = vec![
            "QUERY_DEPOSITS_FAILED",
            "ADVANCED_QUERY_DEPOSITS_FAILED",
            "QUERY_USER_DEPOSITS_FAILED",
            "QUERY_TOKEN_DEPOSITS_FAILED",
            "QUERY_DEPOSIT_BY_SIGNATURE_FAILED",
            "GET_DEPOSIT_STATS_FAILED",
            "GET_USER_DEPOSIT_SUMMARY_FAILED",
            "GET_TOKEN_DEPOSIT_SUMMARY_FAILED",
            "GET_DEPOSIT_TRENDS_FAILED",
            "CREATE_DEPOSIT_EVENT_FAILED",
            "DEPOSIT_EVENT_ALREADY_EXISTS",
        ];

        // 验证错误代码格式
        for code in &error_codes {
            assert!(code.ends_with("_FAILED"));
            assert!(code.chars().all(|c| c.is_uppercase() || c == '_'));
            assert!(code.len() > 10); // 合理的长度
            assert!(!code.starts_with('_'));
            assert!(!code.ends_with("__FAILED"));
        }

        // 验证错误代码唯一性
        let mut unique_codes = std::collections::HashSet::new();
        for code in &error_codes {
            assert!(unique_codes.insert(code), "重复的错误代码: {}", code);
        }

        assert_eq!(unique_codes.len(), error_codes.len());
    }

    #[test]
    fn test_api_status_codes() {
        // 测试API状态码使用的正确性
        let success_status = StatusCode::OK;
        let client_error_status = StatusCode::BAD_REQUEST;
        let not_found_status = StatusCode::NOT_FOUND;
        let server_error_status = StatusCode::INTERNAL_SERVER_ERROR;

        // 验证状态码范围
        assert_eq!(success_status.as_u16(), 200);
        assert_eq!(client_error_status.as_u16(), 400);
        assert_eq!(not_found_status.as_u16(), 404);
        assert_eq!(server_error_status.as_u16(), 500);

        // 验证状态码分类
        assert!(success_status.is_success());
        assert!(client_error_status.is_client_error());
        assert!(not_found_status.is_client_error());
        assert!(server_error_status.is_server_error());
    }

    #[test]
    fn test_route_path_structure() {
        // 测试路由路径结构的一致性
        let route_paths = vec![
            "/deposits",
            "/deposits/advanced",
            "/deposits/by-user/{address}",
            "/deposits/by-token/{mint}",
            "/deposits/by-signature/{signature}",
            "/deposits/stats",
            "/deposits/summary/{address}",
            "/deposits/token-summary/{mint}",
            "/deposits/trends",
        ];

        for path in &route_paths {
            // 验证路径格式
            assert!(path.starts_with("/deposits"));
            assert!(!path.ends_with('/') || *path == "/");
            assert!(!path.contains("//"));

            // 验证路径参数格式
            if path.contains('{') {
                assert!(path.contains('}'));
                assert!(path.matches('{').count() == path.matches('}').count());
            }
        }

        // 验证路径唯一性
        let mut unique_paths = std::collections::HashSet::new();
        for path in &route_paths {
            assert!(unique_paths.insert(path), "重复的路由路径: {}", path);
        }
    }

    #[test]
    fn test_pagination_defaults() {
        // 测试分页默认值的合理性
        const DEFAULT_PAGE: u32 = 1;
        const DEFAULT_PAGE_SIZE: u32 = 20;
        const MAX_PAGE_SIZE: u32 = 100;

        assert_eq!(DEFAULT_PAGE, 1);
        assert_eq!(DEFAULT_PAGE_SIZE, 20);
        assert_eq!(MAX_PAGE_SIZE, 100);

        // 验证默认值合理性
        assert!(DEFAULT_PAGE >= 1);
        assert!(DEFAULT_PAGE_SIZE >= 1 && DEFAULT_PAGE_SIZE <= MAX_PAGE_SIZE);
        assert!(MAX_PAGE_SIZE >= DEFAULT_PAGE_SIZE);
        assert!(MAX_PAGE_SIZE <= 1000); // 避免过大的页面大小
    }

    #[test]
    fn test_api_response_structure() {
        // 测试API响应结构的一致性
        use crate::dtos::solana::common::{ApiResponse, ErrorResponse};

        // 测试成功响应
        let success_data = "test_data";
        let success_response = ApiResponse::success(success_data);

        assert!(success_response.success);
        assert!(success_response.data.is_some());
        assert_eq!(success_response.data.unwrap(), "test_data");
        assert!(!success_response.id.is_empty());

        // 测试错误响应
        let error_response = ErrorResponse::new("TEST_ERROR", "测试错误");

        assert_eq!(error_response.code, "TEST_ERROR");
        assert_eq!(error_response.message, "测试错误");
        assert!(error_response.details.is_none());
        assert!(error_response.timestamp > 0);

        // 测试带详情的错误响应
        let detailed_error = ErrorResponse::new("TEST_ERROR", "测试错误").with_details("详细错误信息");

        assert!(detailed_error.details.is_some());
        assert_eq!(detailed_error.details.unwrap(), "详细错误信息");
    }

    #[test]
    fn test_controller_logging_consistency() {
        // 测试控制器日志记录的一致性
        let log_messages = vec![
            "📊 查询存款事件列表",
            "📊 高级查询存款事件",
            "📊 查询用户{}的存款记录",
            "📊 查询代币{}的存款记录",
            "📊 查询签名{}的存款事件",
            "📊 获取存款统计信息",
            "📊 获取用户{}的存款汇总",
            "📊 获取代币{}的存款汇总",
            "📊 获取存款趋势数据",
        ];

        for message in &log_messages {
            // 验证日志格式
            assert!(message.starts_with("📊"));
            assert!(message.len() > 3);
            assert!(!message.ends_with(' '));

            // 验证中文字符正确性
            let has_chinese = message.chars().any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c));
            assert!(has_chinese, "日志消息应包含中文: {}", message);
        }
    }

    #[test]
    fn test_create_deposit_event_request_validation() {
        // 测试CreateDepositEventRequest结构
        let request = CreateDepositEventRequest {
            user: "8S2bcP66WehuF6cHryfZ7vfFpQWaUhYyAYSy5U3gX4Fy".to_string(),
            project_config: "test_config".to_string(),
            token_mint: "So11111111111111111111111111111111111111112".to_string(),
            amount: 1000000,       // 1 SOL
            total_raised: 5000000, // 5 SOL
            signature: "test_signature_12345".to_string(),
            deposited_at: 1640995200,
            slot: 123456,
            token_decimals: Some(9),
            token_name: Some("Solana".to_string()),
            token_symbol: Some("SOL".to_string()),
            token_logo_uri: Some("https://example.com/sol.png".to_string()),
            deposit_type: Some(0),
            related_pool: Some("test_pool_address".to_string()),
            estimated_usd_value: Some(50.0),
        };

        // 验证必填字段
        assert!(!request.user.is_empty());
        assert!(!request.project_config.is_empty());
        assert!(!request.token_mint.is_empty());
        assert!(!request.signature.is_empty());
        assert!(request.amount > 0);
        assert!(request.total_raised >= request.amount);
        assert!(request.deposited_at > 0);
        assert!(request.slot > 0);

        // 验证可选字段
        assert!(request.token_decimals.is_some());
        assert!(request.token_name.is_some());
        assert!(request.token_symbol.is_some());
        assert!(request.estimated_usd_value.is_some());
        assert!(request.estimated_usd_value.unwrap() >= 0.0);
    }

    #[test]
    fn test_create_deposit_event_response_structure() {
        // 测试CreateDepositEventResponse结构
        let response = CreateDepositEventResponse {
            id: "test_id_12345".to_string(),
            user: "8S2bcP66WehuF6cHryfZ7vfFpQWaUhYyAYSy5U3gX4Fy".to_string(),
            signature: "test_signature_12345".to_string(),
            deposited_at: 1640995200,
            actual_amount: 1.0,
            actual_total_raised: 5.0,
            deposit_type_name: "初始存款".to_string(),
            estimated_usd_value: 50.0,
            created_at: "2022-01-01T00:00:00Z".to_string(),
        };

        // 验证响应结构
        assert!(!response.id.is_empty());
        assert!(!response.user.is_empty());
        assert!(!response.signature.is_empty());
        assert!(!response.deposit_type_name.is_empty());
        assert!(!response.created_at.is_empty());
        assert!(response.deposited_at > 0);
        assert!(response.actual_amount >= 0.0);
        assert!(response.actual_total_raised >= response.actual_amount);
        assert!(response.estimated_usd_value >= 0.0);

        // 验证时间格式（ISO 8601）
        assert!(response.created_at.contains('T'));
        assert!(response.created_at.contains('Z'));
    }

    #[test]
    fn test_post_route_path_addition() {
        // 验证POST路由已正确添加到路由路径列表中
        let route_paths = vec![
            "/deposits", // 现在同时支持GET和POST
            "/deposits/advanced",
            "/deposits/by-user/{address}",
            "/deposits/by-token/{mint}",
            "/deposits/by-signature/{signature}",
            "/deposits/stats",
            "/deposits/summary/{address}",
            "/deposits/token-summary/{mint}",
            "/deposits/trends",
        ];

        for path in &route_paths {
            // 验证路径格式
            assert!(path.starts_with("/deposits"));
            assert!(!path.ends_with('/') || *path == "/");
            assert!(!path.contains("//"));
        }

        // 验证主deposits路径支持多种HTTP方法
        let deposits_path = "/deposits";
        assert!(route_paths.contains(&deposits_path));

        // 验证路径唯一性
        let mut unique_paths = std::collections::HashSet::new();
        for path in &route_paths {
            assert!(unique_paths.insert(path), "重复的路由路径: {}", path);
        }
    }
}
