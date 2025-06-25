use crate::{
    dtos::user_dto::SetUsersDto, extractors::validation_extractor::ValidationExtractor,
    services::Services,
};
use axum::{
    extract::Path,
    routing::{get, post},
    Extension, Json, Router,
};
use database::user::model::User;
use mongodb::results::InsertManyResult;
use utils::{AppError, AppResult};

pub struct UserController;
impl UserController {
    pub fn app() -> Router {
        Router::new()
            .route("/user/:address", get(Self::user))
            .route("/mock_users", post(Self::mock_users))
    }

    pub async fn user(
        Extension(services): Extension<Services>,
        Path(address): Path<String>,
    ) -> AppResult<Json<User>> {
        match services.user.get_user(address.to_string()).await? {
            Some(user) => Ok(Json(user)),
            None => Err(AppError::NotFound(format!(
                "New User with address {} not found.",
                address
            ))),
        }
    }

    pub async fn mock_users(
        Extension(services): Extension<Services>,
        ValidationExtractor(req): ValidationExtractor<SetUsersDto>,
    ) -> AppResult<Json<InsertManyResult>> {
        let users = services.user.create_users(req.users).await?;

        Ok(Json(users))
    }
}
