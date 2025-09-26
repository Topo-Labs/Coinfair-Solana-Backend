use crate::auth::{require_admin, AuthUser};
use crate::dtos::statics::static_dto::{ApiResponse, MintListResponse, MintPriceResponse, PriceData, TokenIdResponse};
use crate::services::Services;
use axum::{
    extract::{Extension, Path, Query},
    middleware,
    response::Json,
    routing::{get, post},
    Router,
};
use database::clmm::token_info::{TokenListQuery, TokenListResponse, TokenPushRequest, TokenPushResponse};
use serde::Deserialize;
use tracing::{info, warn};
use utils::AppResult;
use utoipa::{IntoParams, ToSchema};

/// 代币搜索查询参数
#[derive(Debug, Deserialize, ToSchema, IntoParams)]
pub struct TokenSearchQuery {
    /// 搜索关键词
    pub keyword: String,
    /// 返回结果数量限制 (默认20，最大100)
    pub limit: Option<i64>,
}

/// 查询参数结构体
#[derive(Debug, Deserialize)]
pub struct MintPriceQuery {
    pub mints: String,
}

/// 代币地址路径参数
#[derive(Debug, Deserialize, ToSchema)]
pub struct TokenAddressPath {
    /// 代币地址
    pub address: String,
}

/// Token 控制器 - 处理代币相关的 HTTP 请求
pub struct TokenController;

impl TokenController {
    /// 创建代币管理路由
    pub fn routes() -> Router {
        // 公共路由（无需权限）
        let public_routes = Router::new()
            // 代币推送接口
            .route("/push", post(push_token))
            // 查询接口
            .route("/list", get(get_token_list))
            .route("/query", get(query_tokens))
            .route("/search", get(search_tokens))
            .route("/trending", get(get_trending_tokens))
            .route("/new", get(get_new_tokens))
            .route("/stats", get(get_token_stats))
            .route("/info/:address", get(get_token_by_address))
            .route("/price", get(get_mint_price))
            .route("/ids", get(get_tokens_by_ids));
        // 管理员路由（需要管理员权限）
        let admin_routes = Router::new()
            .route("/status/:address", post(update_token_status))
            .route("/verification/:address", post(update_token_verification))
            .route("/delete/:address", post(delete_token))
            .layer(middleware::from_fn(require_admin));

        // 合并路由
        public_routes.nest("/admin", admin_routes)
    }
}

/// 推送代币信息（Upsert操作）
///
/// 接收来自meme币发射平台或其他外部系统的代币数据推送，支持创建新代币或更新现有代币信息。
/// 系统会自动检测代币是否已存在，如不存在则创建，如已存在则更新相关信息。
///
/// # 请求体
///
/// ```json
/// {
///   "address": "So11111111111111111111111111111111111111112",
///   "program_id": "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",
///   "name": "Wrapped SOL",
///   "symbol": "WSOL",
///   "decimals": 9,
///   "logo_uri": "https://raw.githubusercontent.com/solana-labs/token-list/main/assets/mainnet/So11111111111111111111111111111111111111112/logo.png",
///   "tags": ["defi", "wrapped"],
///   "daily_volume": 50000000.0,
///   "source": "external_push"
/// }
/// ```
///
/// # 响应示例
///
/// ```json
/// {
///   "id": "uuid-string",
///   "success": true,
///   "data": {
///     "id": "代币内部ID",
///     "operation": "created",
///     "address": "So11111111111111111111111111111111111111112",
///     "updated_fields": [],
///     "verification_status": "未验证",
///     "status": "活跃",
///     "timestamp": 1640995200
///   }
/// }
/// ```
#[utoipa::path(
    post,
    path = "/api/v1/solana/mint/push",
    request_body = TokenPushRequest,
    responses(
        (status = 200, description = "代币推送成功", body = ApiResponse<TokenPushResponse>),
        (status = 400, description = "请求数据验证失败"),
        (status = 500, description = "服务器内部错误")
    ),
    tag = "代币管理"
)]
pub async fn push_token(
    Extension(services): Extension<Services>,
    Json(request): Json<TokenPushRequest>,
) -> AppResult<Json<ApiResponse<TokenPushResponse>>> {
    info!("📥 接收代币推送请求: {}", request.address);

    // 验证请求数据
    let _ = validator::Validate::validate(&request)
        .map_err(|e| utils::AppError::BadRequest(format!("请求数据验证失败: {}", e)))?;

    // 处理推送
    let response = services.token.handle_external_push(request).await?;

    Ok(Json(ApiResponse::success(response)))
}

/// 获取代币列表（兼容现有静态接口格式）
///
/// 返回与现有静态接口相同格式的代币列表，包含黑名单和白名单。
/// 此接口保持向后兼容，适用于现有前端调用。
///
/// # 查询参数
///
/// - `blacklist`: 是否返回黑名单代币（可选）
/// - `whitelist`: 是否返回白名单代币（可选）
///
/// # 响应示例
///
/// ```json
/// {
///   "id": "uuid-string",
///   "success": true,
///   "data": {
///     "mintList": {
///       "blacklist": [],
///       "whitelist": [
///         {
///           "address": "So11111111111111111111111111111111111111112",
///           "chainId": 101,
///           "decimals": 9,
///           "name": "Wrapped SOL",
///           "symbol": "WSOL",
///           "logoURI": "https://raw.githubusercontent.com/solana-labs/token-list/main/assets/mainnet/So11111111111111111111111111111111111111112/logo.png",
///           "tags": ["defi", "wrapped"]
///         }
///       ]
///     },
///     "count": 1
///   }
/// }
/// ```
#[utoipa::path(
    get,
    path = "/api/v1/solana/mint/list",
    params(TokenListQuery),
    responses(
        (status = 200, description = "获取代币列表成功", body = ApiResponse<MintListResponse>),
        (status = 500, description = "服务器内部错误")
    ),
    tag = "代币查询"
)]
pub async fn get_token_list(
    Extension(services): Extension<Services>,
    Query(query): Query<TokenListQuery>,
) -> AppResult<Json<ApiResponse<MintListResponse>>> {
    info!("📋 获取代币列表");

    let response = services.token.get_token_list(Some(query)).await?;

    Ok(Json(ApiResponse::success(response)))
}

/// 查询代币列表（新格式，支持分页和高级筛选）
///
/// 支持分页、筛选、多字段排序等高级查询功能，返回详细的代币信息和统计数据。
/// 适用于需要高级查询功能的新版本前端。
///
/// # 查询参数
///
/// - `page`: 页码（从1开始，默认1）
/// - `pageSize`: 每页数量（默认100，最大1000）
/// - `sortBy`: 排序字段，支持多字段排序
///   - 单字段：`created_at`, `daily_volume`, `name`, `symbol`, `updated_at`, `push_time`
///   - 多字段：用逗号分隔，如 `daily_volume,created_at`
/// - `sortOrder`: 排序方向，支持多方向排序
///   - 单方向：`asc` 或 `desc`（默认desc）
///   - 多方向：用逗号分隔，如 `desc,asc`
///   - 如果字段数多于方向数，多余字段使用默认方向(desc)
/// - `status`: 代币状态筛选
/// - `verification`: 验证状态筛选
/// - `tags`: 标签筛选（逗号分隔）
/// - `search`: 搜索关键词（匹配名称、符号、地址）
/// - `minVolume`: 最小日交易量过滤
/// - `maxVolume`: 最大日交易量过滤
/// - `source`: 数据来源过滤
/// - `projectState`: 项目状态过滤（从extensions.project_state字段过滤）
/// - `creator`: 创建者过滤（从extensions.creator字段过滤）
/// - `addresses`: 地址过滤（支持多个地址，用逗号分隔）
/// - `participate`: 参与者过滤（钱包地址，查询该地址参与过的代币众筹活动）
///
/// # 项目状态过滤示例
///
/// ```
/// # 过滤正在进行中的项目
/// projectState=2
///
/// # 过滤已完成的项目
/// projectState=4
///
/// # 结合其他过滤条件
/// status=active&projectState=4&minVolume=1000
/// ```
///
/// # 创建者过滤示例
///
/// ```
/// # 按特定创建者过滤
/// creator=8S2bcP66WehuF6cHryfZ7vfFpQWaUhYyAYSy5U3gX4Fy
///
/// # 结合状态和创建者过滤
/// status=active&creator=8S2bcP66WehuF6cHryfZ7vfFpQWaUhYyAYSy5U3gX4Fy&minVolume=1000
/// ```
///
/// # 地址过滤示例
///
/// ```
/// # 查询单个地址
/// addresses=So11111111111111111111111111111111111111112
///
/// # 查询多个地址（逗号分隔）
/// addresses=So11111111111111111111111111111111111111112,EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v
///
/// # 结合其他过滤条件
/// status=active&addresses=So11111111111111111111111111111111111111112,EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v&minVolume=1000
/// ```
///
/// # 参与者过滤示例
///
/// ```
/// # 查询特定钱包参与过的代币众筹活动
/// participate=8S2bcP66WehuF6cHryfZ7vfFpQWaUhYyAYSy5U3gX4Fy
///
/// # 结合其他过滤条件
/// participate=8S2bcP66WehuF6cHryfZ7vfFpQWaUhYyAYSy5U3gX4Fy&status=active&minVolume=1000
/// ```
///
/// # 多字段排序示例
///
/// ```
/// # 先按交易量降序，再按创建时间升序
/// sortBy=daily_volume,created_at&sortOrder=desc,asc
///
/// # 先按验证状态降序，再按名称升序，最后按创建时间降序
/// sortBy=verification,name,created_at&sortOrder=desc,asc,desc
/// ```
///
/// # 响应示例
///
/// ```json
/// {
///   "id": "uuid-string",
///   "success": true,
///   "data": {
///     "mintList": [
///       {
///         "address": "So11111111111111111111111111111111111111112",
///         "name": "Wrapped SOL",
///         "symbol": "WSOL",
///         "decimals": 9,
///         "logo_uri": "https://...",
///         "daily_volume": 50000000.0,
///         "tags": ["defi", "wrapped"],
///         "created_at": "2024-01-01T00:00:00Z"
///       }
///     ],
///     "pagination": {
///       "current_page": 1,
///       "total_pages": 5,
///       "page_size": 100,
///       "total_count": 500,
///       "has_next": true,
///       "has_prev": false
///     }
///   }
/// }
/// ```
#[utoipa::path(
    get,
    path = "/api/v1/solana/mint/query",
    params(TokenListQuery),
    responses(
        (status = 200, description = "查询代币列表成功", body = ApiResponse<TokenListResponse>),
        (status = 400, description = "查询参数验证失败"),
        (status = 500, description = "服务器内部错误")
    ),
    tag = "代币查询"
)]
pub async fn query_tokens(
    Extension(services): Extension<Services>,
    Query(query): Query<TokenListQuery>,
) -> AppResult<Json<ApiResponse<TokenListResponse>>> {
    info!("🔍 查询代币列表");

    // 验证查询参数
    let _ = validator::Validate::validate(&query)
        .map_err(|e| utils::AppError::BadRequest(format!("查询参数验证失败: {}", e)))?;

    let response = services.token.query_tokens(query).await?;

    Ok(Json(ApiResponse::success(response)))
}

/// 根据地址获取代币详细信息
///
/// 通过代币地址查询特定代币的详细信息，包括基础信息、交易统计、验证状态等。
///
/// # 路径参数
///
/// - `address`: 代币合约地址（如：So11111111111111111111111111111111111111112）
///
/// # 响应示例
///
/// ## 成功响应（代币存在）
///
/// ```json
/// {
///   "address": "So11111111111111111111111111111111111111112",
///   "chainId": 101,
///   "decimals": 9,
///   "name": "Wrapped SOL",
///   "symbol": "WSOL",
///   "logoURI": "https://raw.githubusercontent.com/solana-labs/token-list/main/assets/mainnet/So11111111111111111111111111111111111111112/logo.png",
///   "tags": ["defi", "wrapped"],
///   "extensions": {
///     "website": "https://solana.com",
///     "bridgeContract": "wormhole"
///   }
/// }
/// ```
///
/// ## 代币不存在
///
/// ```json
/// null
/// ```
#[utoipa::path(
    get,
    path = "/api/v1/solana/mint/info/{address}",
    params(
        ("address" = String, Path, description = "代币地址")
    ),
    responses(
        (status = 200, description = "代币信息获取成功"),
        (status = 404, description = "代币不存在"),
        (status = 400, description = "代币地址格式错误"),
        (status = 500, description = "服务器内部错误")
    ),
    tag = "代币查询"
)]
pub async fn get_token_by_address(
    Extension(services): Extension<Services>,
    Path(address): Path<String>,
) -> AppResult<Json<Option<crate::dtos::statics::static_dto::TokenInfo>>> {
    info!("🔍 查询代币信息: {}", address);

    // 验证地址格式
    services.token.validate_token_address(&address)?;

    let token = services.token.get_token_by_address(&address).await?;

    Ok(Json(token))
}

/// 搜索代币（全文搜索）
///
/// 支持通过名称、符号、地址等关键词进行模糊搜索。
/// 使用MongoDB文本索引实现高效的全文搜索，权重设置为：symbol:10, name:5, address:1。
///
/// # 查询参数
///
/// - `keyword`: 搜索关键词（必填，支持部分匹配）
/// - `limit`: 返回结果数量限制（可选，默认10，最大100）
///
/// # 使用示例
///
/// - 搜索SOL: `/search?keyword=SOL&limit=5`
/// - 搜索USDC: `/search?keyword=USDC`
/// - 搜索地址: `/search?keyword=So11111111111111111111111111111111111111112`
///
/// # 响应示例
///
/// ```json
/// [
///   {
///     "address": "So11111111111111111111111111111111111111112",
///     "chainId": 101,
///     "decimals": 9,
///     "name": "Wrapped SOL",
///     "symbol": "WSOL",
///     "logoURI": "https://raw.githubusercontent.com/solana-labs/token-list/main/assets/mainnet/So11111111111111111111111111111111111111112/logo.png",
///     "tags": ["defi", "wrapped"]
///   },
///   {
///     "address": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
///     "chainId": 101,
///     "decimals": 6,
///     "name": "USD Coin",
///     "symbol": "USDC",
///     "logoURI": "https://raw.githubusercontent.com/solana-labs/token-list/main/assets/mainnet/EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v/logo.png",
///     "tags": ["stablecoin"]
///   }
/// ]
/// ```
#[utoipa::path(
    get,
    path = "/api/v1/solana/mint/search",
    params(TokenSearchQuery),
    responses(
        (status = 200, description = "搜索结果"),
        (status = 400, description = "搜索参数错误"),
        (status = 500, description = "服务器内部错误")
    ),
    tag = "代币查询"
)]
pub async fn search_tokens(
    Extension(services): Extension<Services>,
    Query(query): Query<TokenSearchQuery>,
) -> AppResult<Json<Vec<crate::dtos::statics::static_dto::TokenInfo>>> {
    info!("🔍 搜索代币: {}", query.keyword);

    // 验证搜索参数
    if query.keyword.trim().is_empty() {
        return Err(utils::AppError::BadRequest("搜索关键词不能为空".to_string()));
    }

    if let Some(limit) = query.limit {
        if limit <= 0 || limit > 100 {
            return Err(utils::AppError::BadRequest("限制数量必须在1-100之间".to_string()));
        }
    }

    let tokens = services.token.search_tokens(&query.keyword, query.limit).await?;

    Ok(Json(tokens))
}

/// 获取热门代币（按交易量排序）
///
/// 返回按24小时交易量降序排列的热门代币列表。
/// 适用于首页热门代币展示、交易推荐等场景。
///
/// # 查询参数
///
/// - `limit`: 返回数量限制（可选，默认10，最大100）
///
/// # 响应示例
///
/// ```json
/// [
///   {
///     "address": "So11111111111111111111111111111111111111112",
///     "chainId": 101,
///     "decimals": 9,
///     "name": "Wrapped SOL",
///     "symbol": "WSOL",
///     "logoURI": "https://...",
///     "tags": ["defi", "wrapped"],
///     "extensions": {
///       "dailyVolume": 50000000.0,
///       "priceChange24h": 5.2
///     }
///   }
/// ]
/// ```
#[utoipa::path(
    get,
    path = "/api/v1/solana/mint/trending",
    params(
        ("limit" = Option<i64>, Query, description = "返回数量限制（默认10，最大100）")
    ),
    responses(
        (status = 200, description = "获取热门代币成功"),
        (status = 400, description = "参数错误"),
        (status = 500, description = "服务器内部错误")
    ),
    tag = "代币查询"
)]
pub async fn get_trending_tokens(
    Extension(services): Extension<Services>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> AppResult<Json<Vec<crate::dtos::statics::static_dto::TokenInfo>>> {
    info!("📈 获取热门代币");

    let limit = params
        .get("limit")
        .and_then(|s| s.parse::<i64>().ok())
        .map(|l| {
            if l <= 0 || l > 100 {
                return Err(utils::AppError::BadRequest("限制数量必须在1-100之间".to_string()));
            }
            Ok(l)
        })
        .transpose()?;

    let tokens = services.token.get_trending_tokens(limit).await?;

    Ok(Json(tokens))
}

/// 获取新上线代币（按创建时间排序）
///
/// 返回按创建时间降序排列的新上线代币列表。
/// 适用于新币发现、投资机会展示等场景。
///
/// # 查询参数
///
/// - `limit`: 返回数量限制（可选，默认10，最大100）
///
/// # 响应示例
///
/// ```json
/// [
///   {
///     "address": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
///     "chainId": 101,
///     "decimals": 6,
///     "name": "USD Coin",
///     "symbol": "USDC",
///     "logoURI": "https://...",
///     "tags": ["stablecoin"],
///     "extensions": {
///       "createdAt": "2024-01-01T00:00:00Z",
///       "launchPlatform": "pump.fun"
///     }
///   }
/// ]
/// ```
#[utoipa::path(
    get,
    path = "/api/v1/solana/mint/new",
    params(
        ("limit" = Option<i64>, Query, description = "返回数量限制（默认10，最大100）")
    ),
    responses(
        (status = 200, description = "获取新代币成功"),
        (status = 400, description = "参数错误"),
        (status = 500, description = "服务器内部错误")
    ),
    tag = "代币查询"
)]
pub async fn get_new_tokens(
    Extension(services): Extension<Services>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> AppResult<Json<Vec<crate::dtos::statics::static_dto::TokenInfo>>> {
    info!("🆕 获取新上线代币");

    let limit = params
        .get("limit")
        .and_then(|s| s.parse::<i64>().ok())
        .map(|l| {
            if l <= 0 || l > 100 {
                return Err(utils::AppError::BadRequest("限制数量必须在1-100之间".to_string()));
            }
            Ok(l)
        })
        .transpose()?;

    let tokens = services.token.get_new_tokens(limit).await?;

    Ok(Json(tokens))
}

/// 获取代币统计信息
///
/// 返回系统中代币的统计数据，包括总数、活跃数、验证数等
#[utoipa::path(
    get,
    path = "/api/v1/solana/mint/stats",
    responses(
        (status = 200, description = "统计信息获取成功"),
        (status = 500, description = "服务器内部错误")
    ),
    tag = "代币统计"
)]
pub async fn get_token_stats(
    Extension(services): Extension<Services>,
) -> AppResult<Json<database::clmm::token_info::repository::TokenStats>> {
    info!("📊 获取代币统计信息");

    let stats = services.token.get_token_stats().await?;

    Ok(Json(stats))
}

/// 管理员功能：更新代币状态
///
/// 仅限管理员使用，用于更新代币的状态。可用状态包括：
/// - Active: 活跃状态，正常显示和交易
/// - Paused: 暂停状态，暂停交易但保留信息
/// - Deprecated: 弃用状态，不推荐使用
/// - Blacklisted: 黑名单状态，禁止显示和交易
///
/// # 路径参数
///
/// - `address`: 代币合约地址
///
/// # 请求体
///
/// ```json
/// "Active"
/// ```
///
/// # 响应示例
///
/// ```json
/// {
///   "id": "uuid-string",
///   "success": true,
///   "data": true
/// }
/// ```
#[utoipa::path(
    post,
    path = "/api/v1/solana/mint/admin/status/{address}",
    params(
        ("address" = String, Path, description = "代币地址")
    ),
    request_body = TokenStatus,
    responses(
        (status = 200, description = "状态更新成功", body = ApiResponse<bool>),
        (status = 400, description = "代币地址格式错误"),
        (status = 404, description = "代币不存在"),
        (status = 403, description = "权限不足"),
        (status = 500, description = "服务器内部错误")
    ),
    tag = "管理员功能",
    security(
        ("api_key" = [])
    )
)]
pub async fn update_token_status(
    Extension(services): Extension<Services>,
    Extension(user): Extension<AuthUser>,
    Path(address): Path<String>,
    Json(status): Json<database::clmm::token_info::TokenStatus>,
) -> AppResult<Json<ApiResponse<bool>>> {
    warn!(
        "🔄 管理员更新代币状态: {} -> {:?} (操作员: {})",
        address, status, user.user_id
    );

    // 验证地址格式
    services.token.validate_token_address(&address)?;

    let updated = services.token.update_token_status(&address, status).await?;

    if !updated {
        return Err(utils::AppError::NotFound("代币不存在".to_string()));
    }

    Ok(Json(ApiResponse::success(updated)))
}

/// 管理员功能：更新代币验证状态
///
/// 仅限管理员使用，用于更新代币的验证状态（未验证、已验证、社区验证、严格验证）
#[utoipa::path(
    post,
    path = "/api/v1/solana/mint/admin/verification/{address}",
    params(
        ("address" = String, Path, description = "代币地址")
    ),
    request_body = VerificationStatus,
    responses(
        (status = 200, description = "验证状态更新成功", body = ApiResponse<bool>),
        (status = 400, description = "代币地址格式错误"),
        (status = 404, description = "代币不存在"),
        (status = 403, description = "权限不足"),
        (status = 500, description = "服务器内部错误")
    ),
    tag = "管理员功能",
    security(
        ("api_key" = [])
    )
)]
pub async fn update_token_verification(
    Extension(services): Extension<Services>,
    Extension(user): Extension<AuthUser>,
    Path(address): Path<String>,
    Json(verification): Json<database::clmm::token_info::VerificationStatus>,
) -> AppResult<Json<ApiResponse<bool>>> {
    warn!(
        "🔄 管理员更新代币验证状态: {} -> {:?} (操作员: {})",
        address, verification, user.user_id
    );

    // 验证地址格式
    services.token.validate_token_address(&address)?;

    let updated = services.token.update_token_verification(&address, verification).await?;

    if !updated {
        return Err(utils::AppError::NotFound("代币不存在".to_string()));
    }

    Ok(Json(ApiResponse::success(updated)))
}

/// 管理员功能：删除代币（危险操作）
///
/// 仅限超级管理员使用，会永久删除代币信息。
/// ⚠️ 警告：此操作不可逆，请谨慎使用！
///
/// 删除前会检查：
/// - 代币是否存在活跃交易
/// - 是否有用户持仓
/// - 是否为系统关键代币
///
/// # 路径参数
///
/// - `address`: 代币合约地址
///
/// # 响应示例
///
/// ```json
/// {
///   "id": "uuid-string",
///   "success": true,
///   "data": true
/// }
/// ```
#[utoipa::path(
    post,
    path = "/api/v1/solana/mint/admin/delete/{address}",
    params(
        ("address" = String, Path, description = "代币地址")
    ),
    responses(
        (status = 200, description = "删除成功", body = ApiResponse<bool>),
        (status = 400, description = "代币地址格式错误"),
        (status = 404, description = "代币不存在"),
        (status = 403, description = "权限不足"),
        (status = 409, description = "代币有活跃交易，无法删除"),
        (status = 500, description = "服务器内部错误")
    ),
    tag = "管理员功能",
    security(
        ("api_key" = [])
    )
)]
pub async fn delete_token(
    Extension(services): Extension<Services>,
    Extension(user): Extension<AuthUser>,
    Path(address): Path<String>,
) -> AppResult<Json<ApiResponse<bool>>> {
    warn!("🗑️ 管理员删除代币: {} (危险操作，操作员: {})", address, user.user_id);

    // 验证地址格式
    services.token.validate_token_address(&address)?;

    let deleted = services.token.delete_token(&address).await?;

    if !deleted {
        return Err(utils::AppError::NotFound("代币不存在".to_string()));
    }

    Ok(Json(ApiResponse::success(deleted)))
}

/// 获取代币价格
///
/// 根据提供的代币mint地址列表查询价格
///
/// # 查询参数
///
/// - mints: 代币mint地址列表，用逗号分隔
///
/// # 响应示例
///
/// ```json
/// {
///   "id": "fe1955f5-91ba-43c6-8d14-cc0588bb71db",
///   "success": true,
///   "data": {
///     "data": [
///       {
///         "mint": "So11111111111111111111111111111111111111112",
///         "price": "0"
///       }
///     ]
///   }
/// }
/// ```
#[utoipa::path(
    get,
    path = "/api/v1/solana/mint/price",
    params(
        ("mints" = String, Query, description = "代币mint地址列表，用逗号分隔")
    ),
    responses(
        (status = 200, description = "代币价格查询成功", body = ApiResponse<MintPriceResponse>)
    ),
    tag = "代币信息"
)]
pub async fn get_mint_price(Query(params): Query<MintPriceQuery>) -> Json<ApiResponse<MintPriceResponse>> {
    info!("💰 获取代币价格，mints: {}", params.mints);

    let mint_addresses: Vec<&str> = params.mints.split(',').collect();

    let mut price_data = Vec::new();
    for mint in mint_addresses {
        price_data.push(PriceData {
            mint: mint.to_string(),
            price: "0".to_string(), // 按照文档要求，全部返回0
        });
    }

    let response = MintPriceResponse { data: price_data };

    Json(ApiResponse::success(response))
}

/// 代币 ID 查询参数
#[derive(Debug, Deserialize, ToSchema, IntoParams)]
pub struct TokenIdsQuery {
    /// 代币地址列表，用逗号分隔
    pub mints: String,
}

/// 根据地址列表批量获取代币信息
///
/// 根据提供的代币地址列表批量查询代币信息，支持最多50个地址的批量查询。
/// 返回所有找到的代币信息，格式适配前端期望的响应结构。
///
/// # 查询参数
///
/// - mints: 代币地址列表，用逗号分隔
///
/// # 响应示例
///
/// ```json
/// {
///   "id": "uuid-string",
///   "success": true,
///   "data": [
///     {
///       "chainId": 101,
///       "address": "So11111111111111111111111111111111111111112",
///       "programId": "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",
///       "logoURI": "https://raw.githubusercontent.com/solana-labs/token-list/main/assets/mainnet/So11111111111111111111111111111111111111112/logo.png",
///       "symbol": "WSOL",
///       "name": "Wrapped SOL",
///       "decimals": 9,
///       "tags": ["defi", "wrapped"],
///       "extensions": {}
///     }
///   ]
/// }
/// ```
#[utoipa::path(
    get,
    path = "/api/v1/solana/mint/ids",
    params(TokenIdsQuery),
    responses(
        (status = 200, description = "批量查询成功", body = ApiResponse<Vec<TokenIdResponse>>),
        (status = 400, description = "参数错误"),
        (status = 500, description = "服务器内部错误")
    ),
    tag = "代币查询"
)]
pub async fn get_tokens_by_ids(
    Extension(services): Extension<Services>,
    Query(params): Query<TokenIdsQuery>,
) -> AppResult<Json<ApiResponse<Vec<TokenIdResponse>>>> {
    info!("📋 接收批量代币查询请求: {}", params.mints);

    // 解析地址列表
    let addresses: Vec<String> = params
        .mints
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    if addresses.is_empty() {
        return Err(utils::AppError::BadRequest("mints参数不能为空".to_string()));
    }

    // 执行批量查询
    let tokens = services.token.get_tokens_by_addresses(&addresses).await?;

    info!(
        "✅ 批量查询完成: 查询 {} 个地址，找到 {} 个代币",
        addresses.len(),
        tokens.len()
    );

    Ok(Json(ApiResponse::success(tokens)))
}
