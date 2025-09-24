// Main SolanaService coordinator that delegates to specialized services

use super::clmm::config::{ClmmConfigService, ClmmConfigServiceTrait};
use super::clmm::pool::ClmmPoolService;
use super::cpmm::AmmPoolService;
use crate::services::solana::clmm::launch_migration::LaunchMigrationService;
use crate::services::solana::clmm::liquidity_line::LiquidityLineService;
use crate::services::solana::clmm::nft::NftService;
use crate::services::solana::clmm::position::PositionService;
use crate::services::solana::clmm::referral::ReferralService;
use super::shared::{SharedContext, SolanaHelpers};
use crate::services::solana::clmm::swap::SwapService;

use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;

use crate::dtos::solana::common::{TransactionData, WalletInfo};
use crate::dtos::solana::launch::{
    LaunchMigrationAndSendTransactionResponse, LaunchMigrationRequest, LaunchMigrationResponse, LaunchMigrationStats,
};
use crate::dtos::solana::nft::claim::{ClaimNftAndSendTransactionResponse, ClaimNftRequest, ClaimNftResponse};
use crate::dtos::solana::nft::mint::{MintNftAndSendTransactionResponse, MintNftRequest, MintNftResponse};
use crate::dtos::solana::pool::creation::{
    CreateClassicAmmPoolAndSendTransactionResponse, CreateClassicAmmPoolRequest, CreateClassicAmmPoolResponse,
    CreatePoolAndSendTransactionResponse, CreatePoolRequest, CreatePoolResponse,
};
use crate::dtos::solana::pool::info::PoolKeyResponse;
use crate::dtos::solana::pool::liquidity_line::{PoolLiquidityLineData, PoolLiquidityLineRequest};
use crate::dtos::solana::pool::listing::{NewPoolListResponse, NewPoolListResponse2};
use crate::dtos::solana::position::liquidity::{
    DecreaseLiquidityAndSendTransactionResponse, DecreaseLiquidityRequest, DecreaseLiquidityResponse,
    IncreaseLiquidityAndSendTransactionResponse, IncreaseLiquidityRequest, IncreaseLiquidityResponse,
};
use crate::dtos::solana::position::open_position::{
    CalculateLiquidityRequest, CalculateLiquidityResponse, GetUserPositionsRequest,
    OpenPositionAndSendTransactionResponse, OpenPositionRequest, OpenPositionResponse, PositionInfo,
    UserPositionsResponse,
};
use crate::dtos::solana::swap::basic::{
    BalanceResponse, PriceQuoteRequest, PriceQuoteResponse, SwapRequest, SwapResponse,
};
use crate::dtos::solana::swap::raydium::{ComputeSwapV2Request, SwapComputeV2Data, TransactionSwapV2Request};
use crate::dtos::solana::swap::swap_v3::{
    ComputeSwapV3Request, SwapComputeV3Data, SwapV3AndSendTransactionResponse, TransactionSwapV3Request,
};
use crate::dtos::static_dto::{
    ClmmConfig, ClmmConfigResponse, CreateAmmConfigAndSendTransactionResponse, CreateAmmConfigRequest,
    CreateAmmConfigResponse, SaveClmmConfigRequest, SaveClmmConfigResponse,
};
use crate::services::data_transform::DataTransformService;
use tokio::sync::Mutex;

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
    liquidity_line_service: LiquidityLineService,
    pub launch_migration: LaunchMigrationService,
    pub nft: NftService,
    pub referral: ReferralService,
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
        let config_service = ClmmConfigService::new(Arc::new(database.clone()), shared_context.rpc_client.clone());
        let config_service_arc = Arc::new(config_service);

        // 创建优化版本的 DataTransformService，注入 ClmmConfigService
        let optimized_transform_service = DataTransformService::new_optimized(
            Some(shared_context.rpc_client.clone()),
            Some(config_service_arc.clone() as Arc<dyn ClmmConfigServiceTrait>),
        )?;

        // 创建新的 SharedContext 实例，使用优化的 DataTransformService
        let optimized_shared_context = SharedContext {
            rpc_client: shared_context.rpc_client.clone(),
            app_config: shared_context.app_config.clone(), // 保持原有配置
            swap_config: shared_context.swap_config.clone(),
            raydium_swap: shared_context.raydium_swap.clone(),
            api_client: shared_context.api_client.clone(),
            swap_v2_service: shared_context.swap_v2_service.clone(),
            swap_v2_builder: shared_context.swap_v2_builder.clone(),
            config_manager: shared_context.config_manager.clone(), // 保持原有配置
            data_transform_service: Arc::new(Mutex::new(optimized_transform_service)),
        };
        let optimized_shared_context = Arc::new(optimized_shared_context);

        Ok(Self {
            swap_service: SwapService::new(optimized_shared_context.clone()),
            position_service: PositionService::with_database(
                optimized_shared_context.clone(),
                Arc::new(database.clone()),
            ),
            clmm_pool_service: ClmmPoolService::new(
                optimized_shared_context.clone(),
                &database,
                config_service_arc.clone(),
            ),
            amm_pool_service: AmmPoolService::new(optimized_shared_context.clone()),
            config_service: ClmmConfigService::new(
                Arc::new(database.clone()),
                optimized_shared_context.rpc_client.clone(),
            ),
            liquidity_line_service: LiquidityLineService::new(
                optimized_shared_context.rpc_client.clone(),
                Arc::new(database.clone()),
            ),
            launch_migration: LaunchMigrationService::new(optimized_shared_context.clone(), &database),
            nft: NftService::new(optimized_shared_context.clone()),
            referral: ReferralService::new(optimized_shared_context.clone()),
            shared_context: optimized_shared_context,
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
    /// 支持类型转换的方法，用于downcasting
    fn as_any(&self) -> &dyn std::any::Any;

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

    // SwapV3 operations (支持推荐系统)
    async fn compute_swap_v3_base_in(&self, params: ComputeSwapV3Request) -> Result<SwapComputeV3Data>;
    async fn compute_swap_v3_base_out(&self, params: ComputeSwapV3Request) -> Result<SwapComputeV3Data>;
    async fn build_swap_v3_transaction_base_in(&self, request: TransactionSwapV3Request) -> Result<TransactionData>;
    async fn build_swap_v3_transaction_base_out(&self, request: TransactionSwapV3Request) -> Result<TransactionData>;
    // SwapV3 testing operations (本地签名测试方法)
    async fn build_and_send_transaction_swap_v3_transaction_base_in(
        &self,
        request: TransactionSwapV3Request,
    ) -> Result<SwapV3AndSendTransactionResponse>;
    async fn build_and_send_transaction_swap_v3_transaction_base_out(
        &self,
        request: TransactionSwapV3Request,
    ) -> Result<SwapV3AndSendTransactionResponse>;

    // Position operations
    async fn open_position(&self, request: OpenPositionRequest) -> Result<OpenPositionResponse>;
    async fn open_position_and_send_transaction(
        &self,
        request: OpenPositionRequest,
    ) -> Result<OpenPositionAndSendTransactionResponse>;
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
    async fn increase_liquidity_and_send_transaction(
        &self,
        request: IncreaseLiquidityRequest,
    ) -> Result<IncreaseLiquidityAndSendTransactionResponse>;

    // DecreaseLiquidity operations
    async fn decrease_liquidity(&self, request: DecreaseLiquidityRequest) -> Result<DecreaseLiquidityResponse>;
    async fn decrease_liquidity_and_send_transaction(
        &self,
        request: DecreaseLiquidityRequest,
    ) -> Result<DecreaseLiquidityAndSendTransactionResponse>;

    // CLMM Pool operations
    async fn create_pool(&self, request: CreatePoolRequest) -> Result<CreatePoolResponse>;
    async fn create_pool_and_send_transaction(
        &self,
        request: CreatePoolRequest,
    ) -> Result<CreatePoolAndSendTransactionResponse>;

    // CLMM Pool query operations
    async fn get_pool_by_address(&self, pool_address: &str) -> Result<Option<database::clmm_pool::ClmmPool>>;
    async fn get_pools_by_mint(
        &self,
        mint_address: &str,
        limit: Option<i64>,
    ) -> Result<Vec<database::clmm_pool::ClmmPool>>;
    async fn get_pools_by_creator(
        &self,
        creator_wallet: &str,
        limit: Option<i64>,
    ) -> Result<Vec<database::clmm_pool::ClmmPool>>;
    async fn query_pools(
        &self,
        params: &database::clmm_pool::PoolQueryParams,
    ) -> Result<Vec<database::clmm_pool::ClmmPool>>;
    async fn get_pool_statistics(&self) -> Result<database::clmm_pool::PoolStats>;
    async fn query_pools_with_pagination(
        &self,
        params: &database::clmm_pool::model::PoolListRequest,
    ) -> Result<database::clmm_pool::model::PoolListResponse>;

    // New method for the expected response format
    async fn query_pools_with_new_format(
        &self,
        params: &database::clmm_pool::model::PoolListRequest,
    ) -> Result<NewPoolListResponse>;

    async fn query_pools_with_new_format2(
        &self,
        params: &database::clmm_pool::model::PoolListRequest,
    ) -> Result<NewPoolListResponse2>;
    // Pool key operations - NEW
    async fn get_pools_key_by_ids(&self, pool_ids: Vec<String>) -> Result<PoolKeyResponse>;

    // AMM Pool operations
    async fn create_classic_amm_pool(
        &self,
        request: CreateClassicAmmPoolRequest,
    ) -> Result<CreateClassicAmmPoolResponse>;
    async fn create_classic_amm_pool_and_send_transaction(
        &self,
        request: CreateClassicAmmPoolRequest,
    ) -> Result<CreateClassicAmmPoolAndSendTransactionResponse>;

    // CLMM Pool sync operations
    async fn start_clmm_pool_sync(&self) -> Result<()>;

    // CLMM Config operations
    async fn get_clmm_configs(&self) -> Result<ClmmConfigResponse>;
    async fn sync_clmm_configs_from_chain(&self) -> Result<u64>;
    async fn save_clmm_config(&self, config: ClmmConfig) -> Result<String>;
    async fn save_clmm_config_from_request(&self, request: SaveClmmConfigRequest) -> Result<SaveClmmConfigResponse>;

    /// 创建新的AMM配置（构建交易）
    async fn create_amm_config(&self, request: CreateAmmConfigRequest) -> Result<CreateAmmConfigResponse>;

    /// 创建新的AMM配置并发送交易（用于测试）
    async fn create_amm_config_and_send_transaction(
        &self,
        request: CreateAmmConfigRequest,
    ) -> Result<CreateAmmConfigAndSendTransactionResponse>;

    // Liquidity line operations
    async fn get_pool_liquidity_line(&self, request: &PoolLiquidityLineRequest) -> Result<PoolLiquidityLineData>;

    // NFT operations
    async fn mint_nft(&self, request: MintNftRequest) -> Result<MintNftResponse>;
    async fn mint_nft_and_send_transaction(&self, request: MintNftRequest)
        -> Result<MintNftAndSendTransactionResponse>;

    // Claim NFT operations
    async fn claim_nft(&self, request: ClaimNftRequest) -> Result<ClaimNftResponse>;
    async fn claim_nft_and_send_transaction(
        &self,
        request: ClaimNftRequest,
    ) -> Result<ClaimNftAndSendTransactionResponse>;

    // Launch Migration operations
    async fn launch_migration(&self, request: LaunchMigrationRequest) -> Result<LaunchMigrationResponse>;
    async fn launch_migration_and_send_transaction(
        &self,
        request: LaunchMigrationRequest,
    ) -> Result<LaunchMigrationAndSendTransactionResponse>;

    // Launch Migration query operations
    async fn get_user_launch_history(
        &self,
        creator_wallet: &str,
        page: u64,
        limit: u64,
    ) -> Result<Vec<database::clmm_pool::model::ClmmPool>>;

    async fn get_user_launch_history_count(&self, creator_wallet: &str) -> Result<u64>;

    async fn get_launch_stats(&self) -> Result<LaunchMigrationStats>;
}

/// Implementation of SolanaServiceTrait that delegates to specialized services
#[async_trait]
impl SolanaServiceTrait for SolanaService {
    /// 支持类型转换的方法，用于downcasting
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    // Swap operations - delegate to swap_service
    async fn swap_tokens(&self, request: SwapRequest) -> Result<SwapResponse> {
        self.swap_service.swap_tokens(request).await
    }

    // Basic utility operations - delegate to SolanaHelpers
    async fn get_balance(&self) -> Result<BalanceResponse> {
        SolanaHelpers::get_balance(&self.shared_context).await
    }

    async fn get_price_quote(&self, request: PriceQuoteRequest) -> Result<PriceQuoteResponse> {
        self.swap_service.get_price_quote(request).await
    }

    async fn get_wallet_info(&self) -> Result<WalletInfo> {
        SolanaHelpers::get_wallet_info(&self.shared_context).await
    }

    async fn health_check(&self) -> Result<String> {
        SolanaHelpers::health_check(&self.shared_context).await
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

    // SwapV3 operations - delegate to swap_service
    async fn compute_swap_v3_base_in(&self, params: ComputeSwapV3Request) -> Result<SwapComputeV3Data> {
        self.swap_service.compute_swap_v3_base_in(params).await
    }

    async fn compute_swap_v3_base_out(&self, params: ComputeSwapV3Request) -> Result<SwapComputeV3Data> {
        self.swap_service.compute_swap_v3_base_out(params).await
    }

    async fn build_swap_v3_transaction_base_in(&self, request: TransactionSwapV3Request) -> Result<TransactionData> {
        self.swap_service.build_swap_v3_transaction_base_in(request).await
    }

    async fn build_swap_v3_transaction_base_out(&self, request: TransactionSwapV3Request) -> Result<TransactionData> {
        self.swap_service.build_swap_v3_transaction_base_out(request).await
    }

    // SwapV3 testing operations - delegate to swap_service
    async fn build_and_send_transaction_swap_v3_transaction_base_in(
        &self,
        request: TransactionSwapV3Request,
    ) -> Result<SwapV3AndSendTransactionResponse> {
        self.swap_service
            .build_and_send_transaction_swap_v3_transaction_base_in(request)
            .await
    }

    async fn build_and_send_transaction_swap_v3_transaction_base_out(
        &self,
        request: TransactionSwapV3Request,
    ) -> Result<SwapV3AndSendTransactionResponse> {
        self.swap_service
            .build_and_send_transaction_swap_v3_transaction_base_out(request)
            .await
    }

    // Position operations - delegate to position_service
    async fn open_position(&self, request: OpenPositionRequest) -> Result<OpenPositionResponse> {
        self.position_service.open_position(request).await
    }

    async fn open_position_and_send_transaction(
        &self,
        request: OpenPositionRequest,
    ) -> Result<OpenPositionAndSendTransactionResponse> {
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

    async fn increase_liquidity_and_send_transaction(
        &self,
        request: IncreaseLiquidityRequest,
    ) -> Result<IncreaseLiquidityAndSendTransactionResponse> {
        self.position_service
            .increase_liquidity_and_send_transaction(request)
            .await
    }

    // DecreaseLiquidity operations - delegate to position_service
    async fn decrease_liquidity(&self, request: DecreaseLiquidityRequest) -> Result<DecreaseLiquidityResponse> {
        self.position_service.decrease_liquidity(request).await
    }

    async fn decrease_liquidity_and_send_transaction(
        &self,
        request: DecreaseLiquidityRequest,
    ) -> Result<DecreaseLiquidityAndSendTransactionResponse> {
        self.position_service
            .decrease_liquidity_and_send_transaction(request)
            .await
    }

    // CLMM Pool operations - delegate to clmm_pool_service
    async fn create_pool(&self, request: CreatePoolRequest) -> Result<CreatePoolResponse> {
        self.clmm_pool_service.create_pool(request).await
    }

    async fn create_pool_and_send_transaction(
        &self,
        request: CreatePoolRequest,
    ) -> Result<CreatePoolAndSendTransactionResponse> {
        self.clmm_pool_service.create_pool_and_send_transaction(request).await
    }

    // CLMM Pool query operations - delegate to clmm_pool_service
    async fn get_pool_by_address(&self, pool_address: &str) -> Result<Option<database::clmm_pool::ClmmPool>> {
        self.clmm_pool_service.get_pool_by_address(pool_address).await
    }

    async fn get_pools_by_mint(
        &self,
        mint_address: &str,
        limit: Option<i64>,
    ) -> Result<Vec<database::clmm_pool::ClmmPool>> {
        self.clmm_pool_service.get_pools_by_mint(mint_address, limit).await
    }

    async fn get_pools_by_creator(
        &self,
        creator_wallet: &str,
        limit: Option<i64>,
    ) -> Result<Vec<database::clmm_pool::ClmmPool>> {
        self.clmm_pool_service.get_pools_by_creator(creator_wallet, limit).await
    }

    async fn query_pools(
        &self,
        params: &database::clmm_pool::PoolQueryParams,
    ) -> Result<Vec<database::clmm_pool::ClmmPool>> {
        self.clmm_pool_service.query_pools(params).await
    }

    async fn get_pool_statistics(&self) -> Result<database::clmm_pool::PoolStats> {
        self.clmm_pool_service.get_pool_statistics().await
    }

    async fn query_pools_with_pagination(
        &self,
        params: &database::clmm_pool::model::PoolListRequest,
    ) -> Result<database::clmm_pool::model::PoolListResponse> {
        self.clmm_pool_service.query_pools_with_pagination(params).await
    }

    async fn query_pools_with_new_format(
        &self,
        params: &database::clmm_pool::model::PoolListRequest,
    ) -> Result<NewPoolListResponse> {
        // 先获取传统格式的响应
        let old_response = self.clmm_pool_service.query_pools_with_pagination(params).await?;

        // 使用共享的数据转换服务（包含持久化缓存）
        let mut transform_service = self.shared_context.data_transform_service.lock().await;
        let new_response = transform_service
            .transform_pool_list_response(old_response, params)
            .await?;

        Ok(new_response)
    }

    async fn query_pools_with_new_format2(
        &self,
        params: &database::clmm_pool::model::PoolListRequest,
    ) -> Result<NewPoolListResponse2> {
        // 先获取传统格式的响应
        let old_response = self.clmm_pool_service.query_pools_with_pagination(params).await?;

        // 使用共享的数据转换服务（包含持久化缓存）
        let mut transform_service = self.shared_context.data_transform_service.lock().await;
        let new_response = transform_service
            .transform_pool_list_response2(old_response, params)
            .await?;

        Ok(new_response)
    }

    // Pool key operations - NEW
    async fn get_pools_key_by_ids(&self, pool_ids: Vec<String>) -> Result<PoolKeyResponse> {
        // 使用共享服务来获取池子密钥信息
        self.clmm_pool_service.get_pools_key_by_ids(pool_ids).await
    }

    // AMM Pool operations - delegate to amm_pool_service
    async fn create_classic_amm_pool(
        &self,
        request: CreateClassicAmmPoolRequest,
    ) -> Result<CreateClassicAmmPoolResponse> {
        self.amm_pool_service.create_classic_amm_pool(request).await
    }

    async fn create_classic_amm_pool_and_send_transaction(
        &self,
        request: CreateClassicAmmPoolRequest,
    ) -> Result<CreateClassicAmmPoolAndSendTransactionResponse> {
        self.amm_pool_service
            .create_classic_amm_pool_and_send_transaction(request)
            .await
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

    async fn save_clmm_config_from_request(
        &self,
        request: crate::dtos::static_dto::SaveClmmConfigRequest,
    ) -> Result<crate::dtos::static_dto::SaveClmmConfigResponse> {
        self.config_service.save_clmm_config_from_request(request).await
    }

    async fn create_amm_config(
        &self,
        request: crate::dtos::static_dto::CreateAmmConfigRequest,
    ) -> Result<crate::dtos::static_dto::CreateAmmConfigResponse> {
        self.config_service.create_amm_config(request).await
    }

    async fn create_amm_config_and_send_transaction(
        &self,
        request: crate::dtos::static_dto::CreateAmmConfigRequest,
    ) -> Result<crate::dtos::static_dto::CreateAmmConfigAndSendTransactionResponse> {
        self.config_service
            .create_amm_config_and_send_transaction(request)
            .await
    }

    // Liquidity line operations - delegate to liquidity_line_service
    async fn get_pool_liquidity_line(&self, request: &PoolLiquidityLineRequest) -> Result<PoolLiquidityLineData> {
        self.liquidity_line_service.get_pool_liquidity_line(request).await
    }

    // NFT operations - delegate to nft service
    async fn mint_nft(&self, request: MintNftRequest) -> Result<MintNftResponse> {
        self.nft.mint_nft(request).await
    }

    async fn mint_nft_and_send_transaction(
        &self,
        request: MintNftRequest,
    ) -> Result<MintNftAndSendTransactionResponse> {
        self.nft.mint_nft_and_send_transaction(request).await
    }

    // Claim NFT operations - delegate to nft service
    async fn claim_nft(&self, request: ClaimNftRequest) -> Result<ClaimNftResponse> {
        self.nft.claim_nft(request).await
    }

    async fn claim_nft_and_send_transaction(
        &self,
        request: ClaimNftRequest,
    ) -> Result<ClaimNftAndSendTransactionResponse> {
        self.nft.claim_nft_and_send_transaction(request).await
    }

    // Launch Migration operations - delegate to launch_migration service
    async fn launch_migration(&self, request: LaunchMigrationRequest) -> Result<LaunchMigrationResponse> {
        self.launch_migration.launch(request).await
    }

    async fn launch_migration_and_send_transaction(
        &self,
        request: LaunchMigrationRequest,
    ) -> Result<LaunchMigrationAndSendTransactionResponse> {
        self.launch_migration.launch_and_send_transaction(request).await
    }

    // Launch Migration query operations - delegate to launch_migration service
    async fn get_user_launch_history(
        &self,
        creator_wallet: &str,
        page: u64,
        limit: u64,
    ) -> Result<Vec<database::clmm_pool::model::ClmmPool>> {
        self.launch_migration
            .get_user_launch_history(creator_wallet, page, limit)
            .await
    }

    async fn get_user_launch_history_count(&self, creator_wallet: &str) -> Result<u64> {
        self.launch_migration
            .get_user_launch_history_count(creator_wallet)
            .await
    }

    async fn get_launch_stats(&self) -> Result<LaunchMigrationStats> {
        self.launch_migration.get_launch_stats().await
    }
}
