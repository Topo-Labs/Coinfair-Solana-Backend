use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Coinfair Solana Backend API",
        description = "基于 Rust 和 Axum 的区块链推荐奖励系统 API 文档",
        version = "1.0.0",
        contact(
            name = "API Support",
            email = "support@coinfair.xyz"
        )
    ),
    paths(
        // System health check
        crate::api::health,
        // Refer endpoints
        crate::api::refer_controller::get_upper,
        crate::api::refer_controller::get_uppers,
        crate::api::refer_controller::create_refers,
        // Reward endpoints
        crate::api::reward_controller::set_reward,
        crate::api::reward_controller::set_rewards,
        crate::api::reward_controller::get_reward,
        crate::api::reward_controller::get_rewards_by_day,
        crate::api::reward_controller::get_all_rewards,
        crate::api::reward_controller::set_all_rewards,
        crate::api::reward_controller::get_rank_rewards,
        crate::api::reward_controller::list_rewards_by_address,
        crate::api::reward_controller::mock_rewards,
        // User endpoints
        crate::api::user_controller::user,
        crate::api::user_controller::mock_users,
        // Solana endpoints
        crate::api::solana_controller::swap_tokens,
        crate::api::solana_controller::get_balance,
        crate::api::solana_controller::get_price_quote,
        crate::api::solana_controller::get_wallet_info,
        crate::api::solana_controller::health_check,
        // Solana SwapV2 endpoints
        crate::api::solana_controller::compute_swap_v2_base_in,
        crate::api::solana_controller::compute_swap_v2_base_out,
        crate::api::solana_controller::transaction_swap_v2_base_in,
        crate::api::solana_controller::transaction_swap_v2_base_out,
        // Solana OpenPosition endpoints
        crate::api::solana_controller::open_position,
        crate::api::solana_controller::calculate_liquidity,
        crate::api::solana_controller::get_user_positions,
        crate::api::solana_controller::get_position_info,
        crate::api::solana_controller::check_position_exists,
        // Static endpoints
        crate::api::static_controller::get_version,
        crate::api::static_controller::get_auto_fee,
        crate::api::static_controller::get_rpcs,
        crate::api::static_controller::get_chain_time,
        crate::api::static_controller::get_mint_list,
        crate::api::static_controller::get_mint_price,
    ),
    components(
        schemas(
            // Database models
            database::refer::model::Refer,
            database::reward::model::Reward,
            database::reward::model::RewardItem,
            database::reward::model::RewardItemWithTime,
            database::user::model::User,
            // DTOs
            crate::dtos::refer_dto::SetRefersDto,
            crate::dtos::reward_dto::SetRewardDto,
            crate::dtos::reward_dto::SetRewardsDto,
            crate::dtos::reward_dto::MockRewardsDto,
            crate::dtos::user_dto::SetUsersDto,
            // Solana DTOs
            crate::dtos::solana_dto::SwapRequest,
            crate::dtos::solana_dto::SwapResponse,
            crate::dtos::solana_dto::BalanceResponse,
            crate::dtos::solana_dto::PriceQuoteRequest,
            crate::dtos::solana_dto::PriceQuoteResponse,
            crate::dtos::solana_dto::WalletInfo,
            crate::dtos::solana_dto::ErrorResponse,
            crate::dtos::solana_dto::ApiResponse<crate::dtos::solana_dto::SwapResponse>,
            crate::dtos::solana_dto::ApiResponse<crate::dtos::solana_dto::BalanceResponse>,
            crate::dtos::solana_dto::ApiResponse<crate::dtos::solana_dto::PriceQuoteResponse>,
            crate::dtos::solana_dto::ApiResponse<crate::dtos::solana_dto::WalletInfo>,
            crate::dtos::solana_dto::ApiResponse<String>,
            crate::dtos::solana_dto::ApiResponse<crate::dtos::solana_dto::ErrorResponse>,
            crate::dtos::solana_dto::TransactionStatus,
            // Solana SwapV2 DTOs
            crate::dtos::solana_dto::ComputeSwapV2Request,
            crate::dtos::solana_dto::SwapComputeData,
            crate::dtos::solana_dto::SwapComputeV2Data,
            crate::dtos::solana_dto::TransferFeeInfo,
            crate::dtos::solana_dto::RoutePlan,
            crate::dtos::solana_dto::TransactionSwapRequest,
            crate::dtos::solana_dto::TransactionSwapV2Request,
            crate::dtos::solana_dto::TransactionData,
            crate::dtos::solana_dto::RaydiumResponse<crate::dtos::solana_dto::SwapComputeData>,
            crate::dtos::solana_dto::RaydiumResponse<crate::dtos::solana_dto::SwapComputeV2Data>,
            crate::dtos::solana_dto::RaydiumResponse<Vec<crate::dtos::solana_dto::TransactionData>>,
            crate::dtos::solana_dto::RaydiumErrorResponse,
            // Solana OpenPosition DTOs
            crate::dtos::solana_dto::OpenPositionRequest,
            crate::dtos::solana_dto::OpenPositionResponse,
            crate::dtos::solana_dto::CalculateLiquidityRequest,
            crate::dtos::solana_dto::CalculateLiquidityResponse,
            crate::dtos::solana_dto::GetUserPositionsRequest,
            crate::dtos::solana_dto::UserPositionsResponse,
            crate::dtos::solana_dto::PositionInfo,
            // Static DTOs
            crate::dtos::static_dto::VersionConfig,
            crate::dtos::static_dto::AutoFeeConfig,
            crate::dtos::static_dto::DefaultFeeConfig,
            crate::dtos::static_dto::RpcConfig,
            crate::dtos::static_dto::RpcNode,
            crate::dtos::static_dto::ChainTimeConfig,
            crate::dtos::static_dto::MintListResponse,
            crate::dtos::static_dto::TokenInfo,
            crate::dtos::static_dto::MintPriceResponse,
            crate::dtos::static_dto::PriceData,
            crate::dtos::static_dto::ApiResponse<crate::dtos::static_dto::VersionConfig>,
            crate::dtos::static_dto::ApiResponse<crate::dtos::static_dto::AutoFeeConfig>,
            crate::dtos::static_dto::ApiResponse<crate::dtos::static_dto::RpcConfig>,
            crate::dtos::static_dto::ApiResponse<crate::dtos::static_dto::ChainTimeConfig>,
            crate::dtos::static_dto::ApiResponse<crate::dtos::static_dto::MintListResponse>,
            crate::dtos::static_dto::ApiResponse<crate::dtos::static_dto::MintPriceResponse>,
        )
    ),
    tags(
        (name = "系统状态", description = "系统健康检查和状态监控"),
        (name = "refer", description = "推荐关系管理"),
        (name = "reward", description = "奖励系统"),
        (name = "user", description = "用户管理"),
        (name = "Solana交换", description = "Solana 代币交换相关接口"),
        (name = "SwapV2兼容接口", description = "SwapV2 兼容接口，支持转账费"),
        (name = "Solana流动性", description = "Solana 流动性位置管理接口"),
        (name = "系统配置", description = "系统配置相关接口"),
        (name = "代币信息", description = "代币信息相关接口")
    )
)]
pub struct ApiDoc;
