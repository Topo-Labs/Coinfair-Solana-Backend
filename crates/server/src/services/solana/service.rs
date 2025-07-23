// Main SolanaService coordinator that delegates to specialized services

use crate::dtos::solana_dto::{
    BalanceResponse, CalculateLiquidityRequest, CalculateLiquidityResponse, ComputeSwapV2Request, CreateClassicAmmPoolAndSendTransactionResponse,
    CreateClassicAmmPoolRequest, CreateClassicAmmPoolResponse, CreatePoolAndSendTransactionResponse, CreatePoolRequest, CreatePoolResponse,
    GetUserPositionsRequest, OpenPositionAndSendTransactionResponse, OpenPositionRequest, OpenPositionResponse, PositionInfo, PriceQuoteRequest,
    PriceQuoteResponse, SwapComputeV2Data, SwapRequest, SwapResponse, TransactionData, TransactionSwapV2Request, UserPositionsResponse, WalletInfo,
};

use super::amm_pool::AmmPoolService;
use super::clmm_pool::ClmmPoolService;
use super::position::PositionService;
use super::shared::{SharedContext, SolanaHelpers};
use super::swap::SwapService;

use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;

pub type DynSolanaService = Arc<dyn SolanaServiceTrait + Send + Sync>;

/// Main SolanaService struct that coordinates all specialized services
#[allow(dead_code)]
pub struct SolanaService {
    shared_context: Arc<SharedContext>,
    swap_service: SwapService,
    position_service: PositionService,
    clmm_pool_service: ClmmPoolService,
    amm_pool_service: AmmPoolService,
}

impl SolanaService {
    /// Create a new SolanaService with default configuration
    pub fn new() -> Result<Self> {
        let shared_context = Arc::new(SharedContext::new()?);

        Ok(Self {
            swap_service: SwapService::new(shared_context.clone()),
            position_service: PositionService::new(shared_context.clone()),
            clmm_pool_service: ClmmPoolService::new(shared_context.clone()),
            amm_pool_service: AmmPoolService::new(shared_context.clone()),
            shared_context,
        })
    }

    /// Create a new SolanaService with custom configuration
    pub fn with_config(app_config: ::utils::AppConfig) -> Result<Self> {
        let shared_context = Arc::new(SharedContext::with_config(app_config)?);

        Ok(Self {
            swap_service: SwapService::new(shared_context.clone()),
            position_service: PositionService::new(shared_context.clone()),
            clmm_pool_service: ClmmPoolService::new(shared_context.clone()),
            amm_pool_service: AmmPoolService::new(shared_context.clone()),
            shared_context,
        })
    }
}

impl Default for SolanaService {
    fn default() -> Self {
        Self::new().expect("Failed to create default SolanaService")
    }
}

/// Trait defining all Solana service operations
#[async_trait]
pub trait SolanaServiceTrait {
    // Basic operations
    async fn swap_tokens(&self, request: SwapRequest) -> Result<SwapResponse>;
    async fn get_balance(&self) -> Result<BalanceResponse>;
    async fn get_price_quote(&self, request: PriceQuoteRequest) -> Result<PriceQuoteResponse>;
    async fn get_wallet_info(&self) -> Result<WalletInfo>;
    async fn health_check(&self) -> Result<String>;

    // SwapV2 operations
    async fn compute_swap_v2_base_in(&self, params: ComputeSwapV2Request) -> Result<SwapComputeV2Data>;
    async fn compute_swap_v2_base_out(&self, params: ComputeSwapV2Request) -> Result<SwapComputeV2Data>;
    async fn build_swap_v2_transaction_base_in(&self, request: TransactionSwapV2Request) -> Result<TransactionData>;
    async fn build_swap_v2_transaction_base_out(&self, request: TransactionSwapV2Request) -> Result<TransactionData>;

    // Position operations
    async fn open_position(&self, request: OpenPositionRequest) -> Result<OpenPositionResponse>;
    async fn open_position_and_send_transaction(&self, request: OpenPositionRequest) -> Result<OpenPositionAndSendTransactionResponse>;
    async fn calculate_liquidity(&self, request: CalculateLiquidityRequest) -> Result<CalculateLiquidityResponse>;
    async fn get_user_positions(&self, request: GetUserPositionsRequest) -> Result<UserPositionsResponse>;
    async fn get_position_info(&self, position_key: String) -> Result<PositionInfo>;
    async fn check_position_exists(
        &self,
        pool_address: String,
        tick_lower: i32,
        tick_upper: i32,
        wallet_address: Option<String>,
    ) -> Result<Option<PositionInfo>>;

    // CLMM Pool operations
    async fn create_pool(&self, request: CreatePoolRequest) -> Result<CreatePoolResponse>;
    async fn create_pool_and_send_transaction(&self, request: CreatePoolRequest) -> Result<CreatePoolAndSendTransactionResponse>;

    // AMM Pool operations
    async fn create_classic_amm_pool(&self, request: CreateClassicAmmPoolRequest) -> Result<CreateClassicAmmPoolResponse>;
    async fn create_classic_amm_pool_and_send_transaction(
        &self,
        request: CreateClassicAmmPoolRequest,
    ) -> Result<CreateClassicAmmPoolAndSendTransactionResponse>;
}

/// Implementation of SolanaServiceTrait that delegates to specialized services
#[async_trait]
impl SolanaServiceTrait for SolanaService {
    // Swap operations - delegate to swap_service
    async fn swap_tokens(&self, request: SwapRequest) -> Result<SwapResponse> {
        self.swap_service.swap_tokens(request).await
    }

    async fn get_price_quote(&self, request: PriceQuoteRequest) -> Result<PriceQuoteResponse> {
        self.swap_service.get_price_quote(request).await
    }

    async fn compute_swap_v2_base_in(&self, params: ComputeSwapV2Request) -> Result<SwapComputeV2Data> {
        self.swap_service.compute_swap_v2_base_in(params).await
    }

    async fn compute_swap_v2_base_out(&self, params: ComputeSwapV2Request) -> Result<SwapComputeV2Data> {
        self.swap_service.compute_swap_v2_base_out(params).await
    }

    async fn build_swap_v2_transaction_base_in(&self, request: TransactionSwapV2Request) -> Result<TransactionData> {
        self.swap_service.build_swap_v2_transaction_base_in(request).await
    }

    async fn build_swap_v2_transaction_base_out(&self, request: TransactionSwapV2Request) -> Result<TransactionData> {
        self.swap_service.build_swap_v2_transaction_base_out(request).await
    }

    // Position operations - delegate to position_service
    async fn open_position(&self, request: OpenPositionRequest) -> Result<OpenPositionResponse> {
        self.position_service.open_position(request).await
    }

    async fn open_position_and_send_transaction(&self, request: OpenPositionRequest) -> Result<OpenPositionAndSendTransactionResponse> {
        self.position_service.open_position_and_send_transaction(request).await
    }

    async fn calculate_liquidity(&self, request: CalculateLiquidityRequest) -> Result<CalculateLiquidityResponse> {
        self.position_service.calculate_liquidity(request).await
    }

    async fn get_user_positions(&self, request: GetUserPositionsRequest) -> Result<UserPositionsResponse> {
        self.position_service.get_user_positions(request).await
    }

    async fn get_position_info(&self, position_key: String) -> Result<PositionInfo> {
        self.position_service.get_position_info(position_key).await
    }

    // CLMM Pool operations - delegate to clmm_pool_service
    async fn create_pool(&self, request: CreatePoolRequest) -> Result<CreatePoolResponse> {
        self.clmm_pool_service.create_pool(request).await
    }

    async fn create_pool_and_send_transaction(&self, request: CreatePoolRequest) -> Result<CreatePoolAndSendTransactionResponse> {
        self.clmm_pool_service.create_pool_and_send_transaction(request).await
    }

    // AMM Pool operations - delegate to amm_pool_service
    async fn create_classic_amm_pool(&self, request: CreateClassicAmmPoolRequest) -> Result<CreateClassicAmmPoolResponse> {
        self.amm_pool_service.create_classic_amm_pool(request).await
    }

    async fn create_classic_amm_pool_and_send_transaction(
        &self,
        request: CreateClassicAmmPoolRequest,
    ) -> Result<CreateClassicAmmPoolAndSendTransactionResponse> {
        self.amm_pool_service.create_classic_amm_pool_and_send_transaction(request).await
    }

    // Basic utility operations - delegate to SolanaHelpers
    async fn get_balance(&self) -> Result<BalanceResponse> {
        SolanaHelpers::get_balance(&self.shared_context).await
    }

    async fn get_wallet_info(&self) -> Result<WalletInfo> {
        SolanaHelpers::get_wallet_info(&self.shared_context).await
    }

    async fn health_check(&self) -> Result<String> {
        SolanaHelpers::health_check(&self.shared_context).await
    }

    async fn check_position_exists(
        &self,
        pool_address: String,
        tick_lower: i32,
        tick_upper: i32,
        wallet_address: Option<String>,
    ) -> Result<Option<PositionInfo>> {
        self.position_service
            .check_position_exists(pool_address, tick_lower, tick_upper, wallet_address)
            .await
    }
}
