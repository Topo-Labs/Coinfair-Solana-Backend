use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;
use thiserror::Error;

/// LP变更事件服务错误类型
#[derive(Error, Debug)]
pub enum LpChangeEventError {
    #[error("数据库错误: {0}")]
    DatabaseError(#[from] anyhow::Error),

    #[error("事件未找到: {0}")]
    EventNotFound(String),

    #[error("事件已存在: {0}")]
    EventAlreadyExists(String),

    #[error("数据验证错误: {0}")]
    ValidationError(String),

    #[error("分页参数错误: {0}")]
    PaginationError(String),

    #[error("查询参数错误: {0}")]
    QueryParameterError(String),

    #[error("权限不足")]
    PermissionDenied,

    #[error("内部服务器错误: {0}")]
    InternalServerError(String),
}

impl IntoResponse for LpChangeEventError {
    fn into_response(self) -> Response {
        let (status, error_message, error_code) = match self {
            LpChangeEventError::DatabaseError(ref e) => {
                tracing::error!("数据库错误: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, "数据库操作失败", "DATABASE_ERROR")
            }
            LpChangeEventError::EventNotFound(ref msg) => {
                (StatusCode::NOT_FOUND, msg.as_str(), "EVENT_NOT_FOUND")
            }
            LpChangeEventError::EventAlreadyExists(ref msg) => {
                (StatusCode::CONFLICT, msg.as_str(), "EVENT_ALREADY_EXISTS")
            }
            LpChangeEventError::ValidationError(ref msg) => {
                (StatusCode::BAD_REQUEST, msg.as_str(), "VALIDATION_ERROR")
            }
            LpChangeEventError::PaginationError(ref msg) => {
                (StatusCode::BAD_REQUEST, msg.as_str(), "PAGINATION_ERROR")
            }
            LpChangeEventError::QueryParameterError(ref msg) => {
                (StatusCode::BAD_REQUEST, msg.as_str(), "QUERY_PARAMETER_ERROR")
            }
            LpChangeEventError::PermissionDenied => {
                (StatusCode::FORBIDDEN, "权限不足", "PERMISSION_DENIED")
            }
            LpChangeEventError::InternalServerError(ref msg) => {
                tracing::error!("内部服务器错误: {}", msg);
                (StatusCode::INTERNAL_SERVER_ERROR, msg.as_str(), "INTERNAL_SERVER_ERROR")
            }
        };

        let body = Json(json!({
            "error": {
                "code": error_code,
                "message": error_message,
                "details": format!("{}", self)
            },
            "success": false
        }));

        (status, body).into_response()
    }
}