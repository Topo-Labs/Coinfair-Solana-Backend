pub mod solana;
pub mod auth;
pub mod user;

use axum::routing::{get, Router};
use auth::{auth_controller, dev_auth_controller, permission_management_controller};
use user::user_controller;
use crate::api::solana::statics::static_controller;
use self::solana::clmm::{refer_controller, reward_controller};

/// 系统健康检查
///
/// 返回服务器运行状态
///
/// # 响应
///
/// 返回简单的状态消息字符串
#[utoipa::path(
    get,
    path = "/api/v1/",
    responses(
        (status = 200, description = "服务器运行正常", body = String)
    ),
    tag = "系统状态"
)]
pub async fn health() -> &'static str {
    "Server is running! 🚀"
}

pub fn app() -> Router {
    Router::new()
        .route("/health", get(health))
        .nest("/user", user_controller::UserController::app())
        .nest("/refer", refer_controller::ReferController::app())
        .nest("/reward", reward_controller::RewardController::app())
        .nest("/solana", solana::SolanaController::app())
        .nest("/mint", static_controller::StaticController::app())
        .nest("", auth_controller::AuthController::app())
        .nest(
            "/admin/permissions",
            permission_management_controller::PermissionManagementController::routes(),
        )
        .nest("", dev_auth_controller::DevAuthController::routes())
}
