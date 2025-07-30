pub mod clmm_config_controller;
pub mod clmm_pool_create;
pub mod clmm_pool_query;
pub mod cpmm_pool_create;
pub mod liquidity_line_controller;
pub mod position_controller;
pub mod static_config_controller;
pub mod swap_controller;
pub mod swap_v2_controller;

use axum::Router;

pub struct SolanaController;

impl SolanaController {
    pub fn app() -> Router {
        Router::new()
            // 直接合并swap相关路由
            .merge(swap_controller::SwapController::routes())
            .merge(swap_v2_controller::SwapV2Controller::routes())
            // position路由嵌套在/position下
            .nest("/position", position_controller::PositionController::routes())
            // pool路由嵌套在/pool下
            .nest(
                "/pool",
                Router::new()
                    .merge(clmm_pool_create::ClmmPoolCreateController::routes())
                    .merge(cpmm_pool_create::CpmmPoolCreateController::routes())
                    .merge(clmm_pool_query::ClmmPoolQueryController::routes()),
            )
            // pools/info路由
            .nest(
                "/pools/info",
                Router::new()
                    .route("/list", axum::routing::get(clmm_pool_query::get_pool_list))
                    .route("/mint", axum::routing::get(clmm_pool_query::get_pools_by_mint_pair))
                    .route("/ids", axum::routing::get(clmm_pool_query::get_pools_by_ids)),
            )
            // pools/key路由 - 池子密钥信息
            .nest(
                "/pools/key",
                Router::new()
                    .route("/ids", axum::routing::get(clmm_pool_query::get_pools_key_by_ids)),
            )
            // pools/line路由 - 流动性线图
            .nest("/pools/line", liquidity_line_controller::LiquidityLineController::routes())
            // CLMM配置路由
            .nest("/main/clmm-config", Router::new().merge(clmm_config_controller::ClmmConfigController::routes()))
            // 静态配置路由
            .route("/main/version", axum::routing::get(static_config_controller::get_version))
            .route("/main/auto-fee", axum::routing::get(static_config_controller::get_auto_fee))
            .route("/main/rpcs", axum::routing::get(static_config_controller::get_rpcs))
            .route("/main/chain-time", axum::routing::get(static_config_controller::get_chain_time))
            .route("/mint/list", axum::routing::get(static_config_controller::get_mint_list))
            .route("/main/info", axum::routing::get(static_config_controller::get_info))
        // .route("/main/clmm-config", axum::routing::get(clmm_config_controller::ClmmConfigController::routes()))
    }
}
