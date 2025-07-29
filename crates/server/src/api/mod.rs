pub mod refer_controller;
pub mod reward_controller;
pub mod solana;
// pub mod solana_controller;
pub mod static_controller;
pub mod user_controller;

use axum::routing::{get, Router};

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
        .route("/", get(health))
        .nest("/user", user_controller::UserController::app())
        .nest("/refer", refer_controller::ReferController::app())
        .nest("/reward", reward_controller::RewardController::app())
        .nest("/solana", solana::SolanaController::app())
        .nest("/solana/mint", static_controller::StaticController::app())
}
