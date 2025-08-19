use crate::{dtos::refer_dto::SetRefersDto, extractors::validation_extractor::ValidationExtractor, services::Services};
use axum::{
    extract::Path,
    routing::{get, post},
    Extension, Json, Router,
};
use mongodb::results::InsertManyResult;
use utils::{AppError, AppResult};

/// 获取上级推荐人
#[utoipa::path(
    get,
    path = "/api/v1/refer/upper/{address}",
    tag = "refer",
    params(
        ("address" = String, Path, description = "用户钱包地址")
    ),
    responses(
        (status = 200, description = "成功返回上级推荐人地址", body = String),
        (status = 404, description = "未找到上级推荐人")
    )
)]
pub async fn get_upper(
    Extension(services): Extension<Services>,
    Path(address): Path<String>,
) -> AppResult<Json<String>> {
    match services.refer.get_upper(address.to_string()).await? {
        Some(upper) => Ok(Json(upper)),
        None => Err(AppError::NotFound(format!("Upper of address {} not found.", address))),
    }
}

/// 获取所有上级推荐人
#[utoipa::path(
    get,
    path = "/api/v1/refer/uppers/{address}",
    tag = "refer",
    params(
        ("address" = String, Path, description = "用户钱包地址")
    ),
    responses(
        (status = 200, description = "成功返回完整推荐链", body = Vec<String>)
    )
)]
pub async fn get_uppers(
    Extension(services): Extension<Services>,
    Path(address): Path<String>,
) -> AppResult<Json<Vec<String>>> {
    let uppers = services.refer.get_uppers(address.to_string()).await?;

    Ok(Json(uppers))
}

/// 批量创建推荐关系
#[utoipa::path(
    post,
    path = "/api/v1/refer/refers",
    tag = "refer",
    request_body = SetRefersDto,
    responses(
        (status = 200, description = "成功创建推荐关系"),
        (status = 400, description = "请求参数错误")
    )
)]
pub async fn create_refers(
    Extension(services): Extension<Services>,
    ValidationExtractor(req): ValidationExtractor<SetRefersDto>,
) -> AppResult<Json<InsertManyResult>> {
    let refers = services.refer.create_refers(req.refers).await?;

    Ok(Json(refers))
}

pub struct ReferController;
impl ReferController {
    pub fn app() -> Router {
        Router::new()
            .route("/refer/upper/:address", get(get_upper))
            .route("/refer/uppers/:address", get(get_uppers))
            .route("/refer/refers", post(create_refers))
    }
}
