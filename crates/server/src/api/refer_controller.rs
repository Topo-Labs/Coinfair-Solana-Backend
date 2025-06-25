use crate::{
    dtos::refer_dto::SetRefersDto, extractors::validation_extractor::ValidationExtractor,
    services::Services,
};
use axum::{
    extract::Path,
    routing::{get, post},
    Extension, Json, Router,
};
use mongodb::results::InsertManyResult;
use utils::{AppError, AppResult};

pub struct ReferController;
impl ReferController {
    pub fn app() -> Router {
        Router::new()
            .route("/refer/upper/:address", get(Self::upper))
            .route("/refer/uppers/:address", get(Self::uppers))
            .route("/refer/refers", post(Self::refers))
    }

    pub async fn upper(
        Extension(services): Extension<Services>,
        Path(address): Path<String>,
    ) -> AppResult<Json<String>> {
        match services.refer.get_upper(address.to_string()).await? {
            Some(upper) => Ok(Json(upper)),
            None => Err(AppError::NotFound(format!(
                "Upper of address {} not found.",
                address
            ))),
        }
    }

    pub async fn uppers(
        Extension(services): Extension<Services>,
        Path(address): Path<String>,
    ) -> AppResult<Json<Vec<String>>> {
        let uppers = services.refer.get_uppers(address.to_string()).await?;

        Ok(Json(uppers))
    }

    pub async fn refers(
        Extension(services): Extension<Services>,
        ValidationExtractor(req): ValidationExtractor<SetRefersDto>,
    ) -> AppResult<Json<InsertManyResult>> {
        let refers = services.refer.create_refers(req.refers).await?;

        Ok(Json(refers))
    }
}
