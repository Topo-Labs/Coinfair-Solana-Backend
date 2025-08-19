use crate::{dtos::user_dto::SetUsersDto, extractors::validation_extractor::ValidationExtractor, services::Services};
use axum::{
    extract::Path,
    routing::{get, post},
    Extension, Json, Router,
};
use database::user::model::User;
use mongodb::results::InsertManyResult;
use utils::{AppError, AppResult};

/// 获取用户信息
#[utoipa::path(
    get,
    path = "/api/v1/user/user/{address}",
    tag = "user",
    params(
        ("address" = String, Path, description = "用户钱包地址")
    ),
    responses(
        (status = 200, description = "成功返回用户信息", body = User),
        (status = 404, description = "未找到用户")
    )
)]
pub async fn user(Extension(services): Extension<Services>, Path(address): Path<String>) -> AppResult<Json<User>> {
    match services.user.get_user(address.to_string()).await? {
        Some(user) => Ok(Json(user)),
        None => Err(AppError::NotFound(format!(
            "New User with address {} not found.",
            address
        ))),
    }
}

/// 批量创建模拟用户
#[utoipa::path(
    post,
    path = "/api/v1/user/mock_users",
    tag = "user",
    request_body = SetUsersDto,
    responses(
        (status = 200, description = "成功创建模拟用户"),
        (status = 400, description = "请求参数错误")
    )
)]
pub async fn mock_users(
    Extension(services): Extension<Services>,
    ValidationExtractor(req): ValidationExtractor<SetUsersDto>,
) -> AppResult<Json<InsertManyResult>> {
    let users = services.user.create_users(req.users).await?;

    Ok(Json(users))
}

pub struct UserController;
impl UserController {
    pub fn app() -> Router {
        Router::new()
            .route("/user/:address", get(user))
            .route("/mock_users", post(mock_users))
    }
}
