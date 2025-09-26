use crate::dtos::statics::static_dto::{ApiResponse, TokenIdResponse};
use crate::services::Services;
use axum::{
    extract::{Extension, Query},
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use tracing::info;
use utils::AppResult;
use utoipa::{IntoParams, ToSchema};

pub struct StaticController;

impl StaticController {
    pub fn app() -> Router {
        Router::new().route("/ids", get(get_tokens_by_ids))
    }
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
    path = "/api/v1/mint/ids",
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
