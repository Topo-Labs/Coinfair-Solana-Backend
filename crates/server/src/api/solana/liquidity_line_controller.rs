use crate::{
    dtos::solana_dto::{LiquidityLineErrorResponse, PoolLiquidityLineRequest, PoolLiquidityLineResponse},
    services::Services,
};
use axum::{
    extract::{Extension, Query},
    http::StatusCode,
    response::Json,
    routing::get,
    Router,
};
use tracing::{error, info, warn};
use uuid::Uuid;

pub struct LiquidityLineController;

impl LiquidityLineController {
    pub fn routes() -> Router {
        Router::new().route("/position", get(get_pool_liquidity_line))
    }
}

/// 获取池子流动性分布线图
///
/// 返回指定池子的流动性分布线图数据，显示不同价格区间的流动性分布情况。
/// 该接口从Solana链上实时获取TickArray数据，计算流动性分布。
///
/// # 查询参数
///
/// - `id`: 池子地址 (必需)
/// - `range`: 查询范围，以当前价格为中心的tick范围 (可选，默认2000)
/// - `max_points`: 最大返回点数 (可选，默认100)
///
/// # 响应格式
///
/// ```json
/// {
///   "id": "7028313c-ef1d-4ebc-a1a2-2ecc665f1fd4",
///   "success": true,
///   "data": {
///     "count": 2,
///     "line": [
///       {
///         "price": 0.006646607793183304,
///         "liquidity": "21689835282",
///         "tick": -119220
///       }
///     ]
///   }
/// }
/// ```
///
/// # 错误响应
///
/// ```json
/// {
///   "id": "7028313c-ef1d-4ebc-a1a2-2ecc665f1fd4",
///   "success": false,
///   "error": "池子不存在或地址无效",
///   "error_code": "POOL_NOT_FOUND"
/// }
/// ```
#[utoipa::path(
    get,
    path = "/api/v1/solana/pools/line/position",
    params(PoolLiquidityLineRequest),
    responses(
        (status = 200, description = "获取流动性线图成功", body = PoolLiquidityLineResponse),
        (status = 400, description = "请求参数无效", body = LiquidityLineErrorResponse),
        (status = 404, description = "池子不存在", body = LiquidityLineErrorResponse),
        (status = 500, description = "服务器内部错误", body = LiquidityLineErrorResponse)
    ),
    tag = "Solana流动性"
)]
pub async fn get_pool_liquidity_line(
    Query(validated_params): Query<PoolLiquidityLineRequest>,
    Extension(services): Extension<Services>,
) -> Result<Json<PoolLiquidityLineResponse>, (StatusCode, Json<LiquidityLineErrorResponse>)> {
    let request_id = Uuid::new_v4().to_string();

    info!("🎯 获取池子流动性线图 - 请求ID: {}", request_id);
    info!("  池子地址: {}", validated_params.id);
    info!("  查询范围: {:?}", validated_params.range);
    info!("  最大点数: {:?}", validated_params.max_points);

    // 创建错误响应的辅助函数
    let create_error_response = |error_msg: &str, error_code: Option<&str>| {
        Json(LiquidityLineErrorResponse {
            id: request_id.clone(),
            success: false,
            error: error_msg.to_string(),
            error_code: error_code.map(|s| s.to_string()),
        })
    };

    // 验证池子地址格式
    if validated_params.id.len() < 32 || validated_params.id.len() > 44 {
        warn!("⚠️ 无效的池子地址长度: {}", validated_params.id);
        return Err((StatusCode::BAD_REQUEST, create_error_response("池子地址格式无效", Some("INVALID_POOL_ADDRESS"))));
    }

    // 调用服务层获取流动性线图数据
    match services.solana.get_pool_liquidity_line(&validated_params).await {
        Ok(liquidity_line_data) => {
            info!("✅ 成功获取流动性线图 - 请求ID: {}, 数据点数: {}", request_id, liquidity_line_data.count);

            let response = PoolLiquidityLineResponse {
                id: request_id,
                success: true,
                data: liquidity_line_data,
            };

            Ok(Json(response))
        }
        Err(e) => {
            error!("❌ 获取流动性线图失败 - 请求ID: {}, 错误: {:?}", request_id, e);

            let (status_code, error_code, error_msg) = match e.to_string().as_str() {
                s if s.contains("pool not found") || s.contains("池子不存在") => (StatusCode::NOT_FOUND, "POOL_NOT_FOUND", "池子不存在或地址无效"),
                s if s.contains("invalid pool address") || s.contains("无效的池子地址") => (StatusCode::BAD_REQUEST, "INVALID_POOL_ADDRESS", "池子地址格式无效"),
                s if s.contains("RPC") || s.contains("network") => (StatusCode::SERVICE_UNAVAILABLE, "RPC_ERROR", "网络连接错误，请稍后重试"),
                s if s.contains("tick array") => (StatusCode::INTERNAL_SERVER_ERROR, "TICK_ARRAY_ERROR", "获取流动性数据失败"),
                _ => (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR", "服务器内部错误"),
            };

            Err((status_code, create_error_response(error_msg, Some(error_code))))
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::dtos::solana_dto::PoolLiquidityLineRequest;
    use validator::Validate;

    #[test]
    fn test_liquidity_line_request_validation() {
        // 测试有效请求
        let valid_request = PoolLiquidityLineRequest {
            id: "EWsjgXuVrcAESbAyBo6Q2JCuuAdotBhp8g7Qhvf8GNek".to_string(),
            range: Some(1000),
            max_points: Some(100),
        };
        assert!(valid_request.validate().is_ok());

        // 测试默认值
        let default_request = PoolLiquidityLineRequest {
            id: "EWsjgXuVrcAESbAyBo6Q2JCuuAdotBhp8g7Qhvf8GNek".to_string(),
            range: None,
            max_points: None,
        };
        assert!(default_request.validate().is_ok());

        // 测试无效地址长度
        let invalid_request = PoolLiquidityLineRequest {
            id: "short".to_string(),
            range: Some(1000),
            max_points: Some(100),
        };
        assert!(invalid_request.validate().is_err());

        // 测试无效范围
        let invalid_range_request = PoolLiquidityLineRequest {
            id: "EWsjgXuVrcAESbAyBo6Q2JCuuAdotBhp8g7Qhvf8GNek".to_string(),
            range: Some(50), // 小于最小值100
            max_points: Some(100),
        };
        assert!(invalid_range_request.validate().is_err());

        // 测试无效最大点数
        let invalid_points_request = PoolLiquidityLineRequest {
            id: "EWsjgXuVrcAESbAyBo6Q2JCuuAdotBhp8g7Qhvf8GNek".to_string(),
            range: Some(1000),
            max_points: Some(5), // 小于最小值10
        };
        assert!(invalid_points_request.validate().is_err());
    }
}
