pub mod refer_controller;
pub mod reward_controller;
pub mod user_controller;

use axum::routing::{get, Router};

pub async fn health() -> &'static str {
    "Server is running! 🚀"
}

pub fn app() -> Router {
    Router::new()
        .route("/", get(health))
        .nest("/user", user_controller::UserController::app())
        .nest("/refer", refer_controller::ReferController::app())
        .nest("/reward", reward_controller::RewardController::app())
}
