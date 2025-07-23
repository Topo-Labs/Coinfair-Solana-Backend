use ::utils::AppConfig;
use anyhow::Result;
use solana::raydium_api::RaydiumApiClient;
use solana::{RaydiumSwap, SwapConfig, SwapV2InstructionBuilder, SwapV2Service};
use solana_client::rpc_client::RpcClient;
use std::sync::Arc;
use tokio::sync::Mutex;
use utils::ServiceHelpers;

pub mod config;
pub mod helpers;
pub mod types;

use config::{ClientFactory, ConfigurationManager};

/// SharedContext contains all shared resources and configuration
/// that are used across different service modules
pub struct SharedContext {
    pub rpc_client: Arc<RpcClient>,
    pub app_config: AppConfig,
    pub swap_config: SwapConfig,
    pub raydium_swap: Arc<Mutex<Option<RaydiumSwap>>>,
    pub api_client: Arc<RaydiumApiClient>,
    pub swap_v2_service: Arc<SwapV2Service>,
    pub swap_v2_builder: Arc<SwapV2InstructionBuilder>,
    pub config_manager: ConfigurationManager,
}

impl SharedContext {
    /// Create a new SharedContext with default configuration
    pub fn new() -> Result<Self> {
        let app_config = AppConfig::default();
        let config_manager = ConfigurationManager::default();

        // Use ClientFactory to create all clients from environment
        let (rpc_client, api_client, swap_v2_service, swap_v2_builder) = ClientFactory::create_clients_from_env()?;

        let swap_config = config_manager.get_config()?;

        Ok(Self {
            rpc_client,
            app_config,
            swap_config,
            raydium_swap: Arc::new(Mutex::new(None)),
            api_client: Arc::new(api_client),
            swap_v2_service: Arc::new(swap_v2_service),
            swap_v2_builder: Arc::new(swap_v2_builder),
            config_manager,
        })
    }

    /// Create SharedContext with custom configuration
    pub fn with_config(app_config: AppConfig) -> Result<Self> {
        // Create clients using the configuration manager
        let rpc_client = Arc::new(RpcClient::new(app_config.rpc_url.clone()));
        let api_client = RaydiumApiClient::new();
        let swap_v2_service = SwapV2Service::new(&app_config.rpc_url);
        let swap_v2_builder = SwapV2InstructionBuilder::new(&app_config.rpc_url, &app_config.raydium_program_id, 0)
            .map_err(|e| anyhow::anyhow!("Failed to create SwapV2InstructionBuilder: {}", e))?;

        let config_manager = ConfigurationManager::new(app_config);

        // Use ConfigurationManager to get proper configuration
        let swap_config = if config_manager.has_private_key() {
            config_manager.get_config_with_private_key()?
        } else {
            config_manager.get_config()?
        };

        Ok(Self {
            rpc_client,
            app_config: AppConfig::default(), // Use default for now since we have the config_manager
            swap_config,
            raydium_swap: Arc::new(Mutex::new(None)),
            api_client: Arc::new(api_client),
            swap_v2_service: Arc::new(swap_v2_service),
            swap_v2_builder: Arc::new(swap_v2_builder),
            config_manager,
        })
    }

    /// Initialize the Raydium swap service if not already initialized
    pub async fn initialize_raydium(&self) -> Result<()> {
        let mut raydium_guard = self.raydium_swap.lock().await;
        if raydium_guard.is_none() {
            tracing::info!("Initializing Raydium swap service...");

            // Create SolanaClient
            let client = solana::SolanaClient::new(&self.swap_config)?;

            // Create RaydiumSwap instance
            match RaydiumSwap::new(client, &self.swap_config) {
                Ok(raydium_swap) => {
                    *raydium_guard = Some(raydium_swap);
                    tracing::info!("✅ Raydium swap service initialized successfully");
                }
                Err(e) => {
                    tracing::error!("❌ Failed to initialize Raydium swap service: {:?}", e);
                    return Err(anyhow::anyhow!("Failed to initialize Raydium swap service: {}", e));
                }
            }
        }
        Ok(())
    }

    /// Ensure Raydium service is available
    pub async fn ensure_raydium_available(&self) -> Result<()> {
        self.initialize_raydium().await?;
        let raydium_guard = self.raydium_swap.lock().await;
        if raydium_guard.is_none() {
            Err(anyhow::anyhow!("Raydium swap service not initialized"))
        } else {
            Ok(())
        }
    }

    /// Get wallet address from private key
    pub async fn get_wallet_address_from_private_key(&self) -> String {
        if let Some(raydium) = self.raydium_swap.lock().await.as_ref() {
            // Get wallet address through RaydiumSwap
            match raydium.get_wallet_pubkey() {
                Ok(pubkey) => pubkey.to_string(),
                Err(_) => "Unable to get wallet address".to_string(),
            }
        } else if let Some(private_key) = &self.app_config.private_key {
            // If private key is configured but raydium is not initialized, show first 8 chars as identifier
            format!("{}...(private key configured)", &private_key[..8.min(private_key.len())])
        } else {
            "Private key not configured".to_string()
        }
    }

    /// Get configuration through ConfigurationManager
    pub fn get_config(&self) -> Result<SwapConfig> {
        self.config_manager.get_config()
    }

    /// Get configuration with private key through ConfigurationManager
    pub fn get_config_with_private_key(&self) -> Result<SwapConfig> {
        self.config_manager.get_config_with_private_key()
    }

    /// Check if private key is available
    pub fn has_private_key(&self) -> bool {
        self.config_manager.has_private_key()
    }

    /// Validate configuration
    pub fn validate_config(&self) -> Result<()> {
        self.config_manager.validate_config()
    }

    /// Create a new Raydium swap instance using ConfigurationManager
    pub fn create_raydium_swap(&self) -> Result<RaydiumSwap> {
        self.config_manager.create_raydium_swap()
    }

    /// Create ServiceHelpers instance server::services::solana::shared::helpers
    pub fn create_service_helpers(&self) -> ServiceHelpers {
        ServiceHelpers::new(&self.rpc_client)
    }
}
