pub mod clmm;
pub mod cpmm;
pub mod statics;

use crate::auth::SolanaMiddlewareBuilder;
use axum::{middleware, Extension, Router};
use std::sync::Arc;
use clmm::{clmm_config_controller, clmm_pool_create, clmm_pool_query, deposit_event_controller, event_controller, launch_event_controller, launch_migration_controller, liquidity_line_controller, nft_controller, position_controller, referral_controller, static_config_controller, swap_controller, swap_v2_controller, swap_v3_controller, token_controller};
use cpmm::{pool_create_controller, cpmm_swap_controller, cpmm_config_controller, deposit_controller, withdraw_controller, lp_change_event_controller};

pub struct SolanaController;

impl SolanaController {
    pub fn app() -> Router {
        Router::new()
            // 公开信息路由 - 使用可选权限检查
            .nest("/main", Self::public_info_routes())
            .nest("/mint", Self::mint_info_routes())
            // 查询路由 - 使用可选权限检查
            .nest("/pools", Self::query_routes())
            // 事件查询路由 - 使用可选权限检查
            .nest("/events", Self::event_routes())
            // 交易路由 - 使用强制权限检查
            .merge(Self::trading_routes())
            // 仓位管理路由 - 使用强制权限检查
            .nest("/position", Self::position_routes())
            // NFT推荐路由 - 使用强制权限检查
            .nest("/nft", Self::nft_routes())
            // 池子管理路由 - 使用强制权限检查和特定权限
            .nest("/pool", Self::pool_management_routes())
            // 流动性管理路由 - 存款、提款等操作
            .nest("/liquidity", Self::liquidity_management_routes())
    }

    /// 公开信息路由 - 版本、配置等基础信息
    fn public_info_routes() -> Router {
        Router::new()
            .route("/version", axum::routing::get(static_config_controller::get_version))
            .route("/auto-fee", axum::routing::get(static_config_controller::get_auto_fee))
            .route("/rpcs", axum::routing::get(static_config_controller::get_rpcs))
            .route(
                "/chain-time",
                axum::routing::get(static_config_controller::get_chain_time),
            )
            .route("/info", axum::routing::get(static_config_controller::get_info))
            .nest("/clmm-config", clmm_config_controller::ClmmConfigController::routes())
            .nest("/cpmm-config", cpmm_config_controller::CpmmConfigController::routes())
            .layer(middleware::from_fn(Self::apply_solana_optional_auth))
    }

    /// 代币信息路由
    fn mint_info_routes() -> Router {
        Router::new()
            // 新的代币管理路由（接管所有mint相关路由）
            .merge(token_controller::TokenController::routes())
            .layer(middleware::from_fn(Self::apply_solana_optional_auth))
    }

    /// 查询路由 - 池子信息、流动性数据等
    fn query_routes() -> Router {
        Router::new()
            // pools/info路由 - 池子基础信息
            .nest(
                "/info",
                Router::new()
                    .route("/list", axum::routing::get(clmm_pool_query::get_pool_list))
                    .route("/mint", axum::routing::get(clmm_pool_query::get_pools_by_mint_pair))
                    .route("/ids", axum::routing::get(clmm_pool_query::get_pools_by_ids)),
            )
            // pools/key路由 - 池子密钥信息
            .nest(
                "/key",
                Router::new().route("/ids", axum::routing::get(clmm_pool_query::get_pools_key_by_ids)),
            )
            // pools/line路由 - 流动性线图
            .nest("/line", liquidity_line_controller::LiquidityLineController::routes())
            .layer(middleware::from_fn(Self::apply_solana_optional_auth))
    }

    /// 事件查询路由 - NFT领取和奖励分发事件、Launch事件、存款事件、LP变更事件
    fn event_routes() -> Router {
        Router::new()
            // 基础事件路由 - NFT领取和奖励分发事件
            .merge(event_controller::EventController::routes())
            // 存款事件路由
            .merge(deposit_event_controller::DepositEventController::routes())
            // Launch事件路由
            .nest("/launch", launch_event_controller::LaunchEventController::routes())
            // LP变更事件路由
            .nest("/cpmm", lp_change_event_controller::lp_change_event_routes())
            .layer(middleware::from_fn(Self::apply_solana_optional_auth))
    }

    /// 交易路由 - 交换操作
    fn trading_routes() -> Router {
        Router::new()
            // 合并CLMM swap相关路由
            .merge(swap_controller::SwapController::routes())
            .merge(swap_v2_controller::SwapV2Controller::routes())
            .merge(swap_v3_controller::SwapV3Controller::routes())
            // 合并CPMM swap相关路由
            .merge(cpmm_swap_controller::CpmmSwapController::routes())
            .layer(middleware::from_fn(Self::apply_solana_auth))
    }

    /// 仓位管理路由 - 开仓、平仓、增减流动性等
    fn position_routes() -> Router {
        position_controller::PositionController::routes().layer(middleware::from_fn(Self::apply_solana_auth))
    }

    /// NFT推荐路由 - NFT铸造等
    fn nft_routes() -> Router {
        Router::new()
            .merge(nft_controller::NftController::routes())
            .nest("/referral", referral_controller::ReferralController::routes())
            .layer(middleware::from_fn(Self::apply_solana_auth))
    }

    /// 池子管理路由 - 创建池子等高级操作
    fn pool_management_routes() -> Router {
        Router::new()
            .merge(clmm_pool_create::ClmmPoolCreateController::routes())
            .merge(pool_create_controller::CpmmPoolCreateController::routes())
            .merge(clmm_pool_query::ClmmPoolQueryController::routes())
            // 添加发射迁移路由
            .nest(
                "/launch-migration",
                launch_migration_controller::LaunchMigrationController::routes(),
            )
            .layer(middleware::from_fn(Self::apply_solana_auth))
    }

    /// 流动性管理路由 - 存款、提款等流动性操作
    fn liquidity_management_routes() -> Router {
        Router::new()
            .nest("/cpmm",
                Router::new()
                    .merge(deposit_controller::CpmmDepositController::routes())
                    .merge(withdraw_controller::CpmmWithdrawController::routes())
            )
            .layer(middleware::from_fn(Self::apply_solana_auth))
    }

    /// 应用Solana权限检查中间件（强制认证）
    async fn apply_solana_auth(
        Extension(solana_middleware): Extension<Arc<SolanaMiddlewareBuilder>>,
        request: axum::extract::Request,
        next: axum::middleware::Next,
    ) -> Result<axum::response::Response, axum::http::StatusCode> {
        let middleware_fn = solana_middleware.solana_auth();
        middleware_fn(request, next).await
    }

    /// 应用Solana可选权限检查中间件（允许匿名访问但检查权限）
    async fn apply_solana_optional_auth(
        Extension(solana_middleware): Extension<Arc<SolanaMiddlewareBuilder>>,
        request: axum::extract::Request,
        next: axum::middleware::Next,
    ) -> Result<axum::response::Response, axum::http::StatusCode> {
        let middleware_fn = solana_middleware.solana_optional_auth();
        middleware_fn(request, next).await
    }
}
