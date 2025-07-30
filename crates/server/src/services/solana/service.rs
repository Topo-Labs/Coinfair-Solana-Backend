// Main SolanaService coordinator that delegates to specialized services

use crate::dtos::solana_dto::{
    BalanceResponse, CalculateLiquidityRequest, CalculateLiquidityResponse, ComputeSwapV2Request, CreateClassicAmmPoolAndSendTransactionResponse,
    CreateClassicAmmPoolRequest, CreateClassicAmmPoolResponse, CreatePoolAndSendTransactionResponse, CreatePoolRequest, CreatePoolResponse,
    DecreaseLiquidityAndSendTransactionResponse, DecreaseLiquidityRequest, DecreaseLiquidityResponse,
    GetUserPositionsRequest, IncreaseLiquidityAndSendTransactionResponse, IncreaseLiquidityRequest, IncreaseLiquidityResponse, NewPoolListResponse, 
    OpenPositionAndSendTransactionResponse, OpenPositionRequest, OpenPositionResponse, PositionInfo, PriceQuoteRequest,
    PriceQuoteResponse, SwapComputeV2Data, SwapRequest, SwapResponse, TransactionData, TransactionSwapV2Request, UserPositionsResponse, WalletInfo,
};

use super::amm_pool::AmmPoolService;
use super::clmm_pool::ClmmPoolService;
use super::config::{ClmmConfigService, ClmmConfigServiceTrait};
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
    config_service: ClmmConfigService,
}

impl SolanaService {
    /// Create a new SolanaService with default configuration (deprecated - use with_database instead)
    pub fn new() -> Result<Self> {
        // 这个方法已弃用，因为需要数据库实例
        Err(anyhow::anyhow!("请使用 with_database 方法创建 SolanaService"))
    }

    /// Create a new SolanaService with custom configuration (deprecated - use with_database instead)
    pub fn with_config(_app_config: ::utils::AppConfig) -> Result<Self> {
        // 这个方法已弃用，因为需要数据库实例
        Err(anyhow::anyhow!("请使用 with_database 方法创建 SolanaService"))
    }

    /// Create a new SolanaService with database integration
    pub fn with_database(database: database::Database) -> Result<Self> {
        let shared_context = Arc::new(SharedContext::new()?);

        Ok(Self {
            swap_service: SwapService::new(shared_context.clone()),
            position_service: PositionService::with_database(shared_context.clone(), Arc::new(database.clone())),
            clmm_pool_service: ClmmPoolService::new(shared_context.clone(), &database),
            amm_pool_service: AmmPoolService::new(shared_context.clone()),
            config_service: ClmmConfigService::new(
                Arc::new(database),
                shared_context.rpc_client.clone(),
            ),
            shared_context,
        })
    }
}

impl Default for SolanaService {
    fn default() -> Self {
        // 默认实现已弃用，应该使用 with_database 方法
        panic!("SolanaService::default() 已弃用，请使用 SolanaService::with_database() 方法")
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
    
    // IncreaseLiquidity operations
    async fn increase_liquidity(&self, request: IncreaseLiquidityRequest) -> Result<IncreaseLiquidityResponse>;
    async fn increase_liquidity_and_send_transaction(&self, request: IncreaseLiquidityRequest) -> Result<IncreaseLiquidityAndSendTransactionResponse>;
    
    // DecreaseLiquidity operations
    async fn decrease_liquidity(&self, request: DecreaseLiquidityRequest) -> Result<DecreaseLiquidityResponse>;
    async fn decrease_liquidity_and_send_transaction(&self, request: DecreaseLiquidityRequest) -> Result<DecreaseLiquidityAndSendTransactionResponse>;

    // CLMM Pool operations
    async fn create_pool(&self, request: CreatePoolRequest) -> Result<CreatePoolResponse>;
    async fn create_pool_and_send_transaction(&self, request: CreatePoolRequest) -> Result<CreatePoolAndSendTransactionResponse>;

    // CLMM Pool query operations
    async fn get_pool_by_address(&self, pool_address: &str) -> Result<Option<database::clmm_pool::ClmmPool>>;
    async fn get_pools_by_mint(&self, mint_address: &str, limit: Option<i64>) -> Result<Vec<database::clmm_pool::ClmmPool>>;
    async fn get_pools_by_creator(&self, creator_wallet: &str, limit: Option<i64>) -> Result<Vec<database::clmm_pool::ClmmPool>>;
    async fn query_pools(&self, params: &database::clmm_pool::PoolQueryParams) -> Result<Vec<database::clmm_pool::ClmmPool>>;
    async fn get_pool_statistics(&self) -> Result<database::clmm_pool::PoolStats>;
    async fn query_pools_with_pagination(&self, params: &database::clmm_pool::model::PoolListRequest) -> Result<database::clmm_pool::model::PoolListResponse>;
    
    // New method for the expected response format
    async fn query_pools_with_new_format(&self, params: &database::clmm_pool::model::PoolListRequest) -> Result<NewPoolListResponse>;

    // AMM Pool operations
    async fn create_classic_amm_pool(&self, request: CreateClassicAmmPoolRequest) -> Result<CreateClassicAmmPoolResponse>;
    async fn create_classic_amm_pool_and_send_transaction(
        &self,
        request: CreateClassicAmmPoolRequest,
    ) -> Result<CreateClassicAmmPoolAndSendTransactionResponse>;
    
    // CLMM Pool sync operations
    async fn start_clmm_pool_sync(&self) -> Result<()>;
    
    // CLMM Config operations
    async fn get_clmm_configs(&self) -> Result<crate::dtos::static_dto::ClmmConfigResponse>;
    async fn sync_clmm_configs_from_chain(&self) -> Result<u64>;
    async fn save_clmm_config(&self, config: crate::dtos::static_dto::ClmmConfig) -> Result<String>;
    async fn save_clmm_config_from_request(&self, request: crate::dtos::static_dto::SaveClmmConfigRequest) -> Result<crate::dtos::static_dto::SaveClmmConfigResponse>;
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

    // CLMM Pool query operations - delegate to clmm_pool_service
    async fn get_pool_by_address(&self, pool_address: &str) -> Result<Option<database::clmm_pool::ClmmPool>> {
        self.clmm_pool_service.get_pool_by_address(pool_address).await
    }

    async fn get_pools_by_mint(&self, mint_address: &str, limit: Option<i64>) -> Result<Vec<database::clmm_pool::ClmmPool>> {
        self.clmm_pool_service.get_pools_by_mint(mint_address, limit).await
    }

    async fn get_pools_by_creator(&self, creator_wallet: &str, limit: Option<i64>) -> Result<Vec<database::clmm_pool::ClmmPool>> {
        self.clmm_pool_service.get_pools_by_creator(creator_wallet, limit).await
    }

    async fn query_pools(&self, params: &database::clmm_pool::PoolQueryParams) -> Result<Vec<database::clmm_pool::ClmmPool>> {
        self.clmm_pool_service.query_pools(params).await
    }

    async fn get_pool_statistics(&self) -> Result<database::clmm_pool::PoolStats> {
        self.clmm_pool_service.get_pool_statistics().await
    }

    async fn query_pools_with_pagination(&self, params: &database::clmm_pool::model::PoolListRequest) -> Result<database::clmm_pool::model::PoolListResponse> {
        self.clmm_pool_service.query_pools_with_pagination(params).await
    }
    
    async fn query_pools_with_new_format(&self, params: &database::clmm_pool::model::PoolListRequest) -> Result<NewPoolListResponse> {
        use crate::services::data_transform::DataTransformService;
        
        // 先获取传统格式的响应
        let old_response = self.clmm_pool_service.query_pools_with_pagination(params).await?;
        
        // 使用数据转换服务转换为新格式
        let mut transform_service = DataTransformService::new()?;
        let new_response = transform_service.transform_pool_list_response(old_response, params).await?;
        
        Ok(new_response)
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

    // IncreaseLiquidity operations - delegate to position_service
    async fn increase_liquidity(&self, request: IncreaseLiquidityRequest) -> Result<IncreaseLiquidityResponse> {
        self.position_service.increase_liquidity(request).await
    }

    async fn increase_liquidity_and_send_transaction(&self, request: IncreaseLiquidityRequest) -> Result<IncreaseLiquidityAndSendTransactionResponse> {
        self.position_service.increase_liquidity_and_send_transaction(request).await
    }

    // DecreaseLiquidity operations - delegate to position_service
    async fn decrease_liquidity(&self, request: DecreaseLiquidityRequest) -> Result<DecreaseLiquidityResponse> {
        self.position_service.decrease_liquidity(request).await
    }

    async fn decrease_liquidity_and_send_transaction(&self, request: DecreaseLiquidityRequest) -> Result<DecreaseLiquidityAndSendTransactionResponse> {
        self.position_service.decrease_liquidity_and_send_transaction(request).await
    }

    // CLMM Pool sync operations - delegate to clmm_pool_service
    async fn start_clmm_pool_sync(&self) -> Result<()> {
        self.clmm_pool_service.start_auto_sync().await
    }
    
    // CLMM Config operations - delegate to config_service
    async fn get_clmm_configs(&self) -> Result<crate::dtos::static_dto::ClmmConfigResponse> {
        self.config_service.get_clmm_configs().await
    }
    
    async fn sync_clmm_configs_from_chain(&self) -> Result<u64> {
        self.config_service.sync_clmm_configs_from_chain().await
    }
    
    async fn save_clmm_config(&self, config: crate::dtos::static_dto::ClmmConfig) -> Result<String> {
        self.config_service.save_clmm_config(config).await
    }
    
    async fn save_clmm_config_from_request(&self, request: crate::dtos::static_dto::SaveClmmConfigRequest) -> Result<crate::dtos::static_dto::SaveClmmConfigResponse> {
        self.config_service.save_clmm_config_from_request(request).await
    }
}
