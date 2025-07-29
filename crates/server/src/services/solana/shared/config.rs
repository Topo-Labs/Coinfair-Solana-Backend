use super::types::constants::{DEFAULT_RAYDIUM_PROGRAM_ID, USDC_MINT_STANDARD};
use ::utils::AppConfig;
use anyhow::Result;
use ::utils::solana::{RaydiumApiClient, RaydiumSwap, SolanaClient, SwapConfig, SwapV2Service};
use ::utils::solana::swap_services::SwapV2InstructionBuilder;
use solana_client::rpc_client::RpcClient;
use std::sync::Arc;
use tracing::info;

/// ConfigurationManager handles all configuration-related operations
/// and provides centralized access to RPC clients and API clients
pub struct ConfigurationManager {
    app_config: AppConfig,
}

impl ConfigurationManager {
    /// Create a new ConfigurationManager
    pub fn new(app_config: AppConfig) -> Self {
        Self { app_config }
    }

    /// Create ConfigurationManager with default configuration
    pub fn default() -> Self {
        Self {
            app_config: AppConfig::default(),
        }
    }

    /// Get unified configuration for read-only operations
    pub fn get_config(&self) -> Result<SwapConfig> {
        info!("🔍 Loading Solana configuration...");

        let rpc_url = std::env::var("RPC_URL").unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".to_string());
        let amm_program_id = std::env::var("RAYDIUM_PROGRAM_ID").unwrap_or_else(|_| DEFAULT_RAYDIUM_PROGRAM_ID.to_string());

        let config = SwapConfig {
            rpc_url: rpc_url.clone(),
            private_key: "".to_string(),
            amm_program_id: amm_program_id.clone(),
            openbook_program_id: "".to_string(),
            usdc_mint: USDC_MINT_STANDARD.to_string(),
            sol_usdc_pool_id: "".to_string(),
        };

        info!("✅ Solana configuration loaded successfully (read-only mode)");
        info!("  RPC URL: {}", config.rpc_url);
        info!("  Raydium Program ID: {}", config.amm_program_id);
        Ok(config)
    }

    /// Get complete configuration including private key
    pub fn get_config_with_private_key(&self) -> Result<SwapConfig> {
        info!("🔍 Loading complete Solana configuration (including private key)...");

        let rpc_url = self.app_config.rpc_url.clone();
        let amm_program_id = self.app_config.raydium_program_id.clone();
        let private_key = self
            .app_config
            .private_key
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Private key not configured, please check PRIVATE_KEY in .env.development file"))?
            .clone();

        let config = SwapConfig {
            rpc_url: rpc_url.clone(),
            private_key,
            amm_program_id: amm_program_id.clone(),
            openbook_program_id: "".to_string(),
            usdc_mint: USDC_MINT_STANDARD.to_string(),
            sol_usdc_pool_id: "".to_string(),
        };

        info!("✅ Complete Solana configuration loaded successfully");
        info!("  RPC URL: {}", config.rpc_url);
        info!("  Raydium Program ID: {}", config.amm_program_id);
        Ok(config)
    }

    /// Initialize RPC client
    pub fn create_rpc_client(&self) -> Arc<RpcClient> {
        let rpc_url = self.app_config.rpc_url.clone();
        Arc::new(RpcClient::new(rpc_url))
    }

    /// Initialize RPC client with custom URL
    pub fn create_rpc_client_with_url(rpc_url: &str) -> Arc<RpcClient> {
        Arc::new(RpcClient::new(rpc_url.to_string()))
    }

    /// Initialize API client
    pub fn create_api_client(&self) -> RaydiumApiClient {
        RaydiumApiClient::new()
    }

    /// Initialize SwapV2 service
    pub fn create_swap_v2_service(&self) -> SwapV2Service {
        SwapV2Service::new(&self.app_config.rpc_url)
    }

    /// Initialize SwapV2 service with custom URL
    pub fn create_swap_v2_service_with_url(rpc_url: &str) -> SwapV2Service {
        SwapV2Service::new(rpc_url)
    }

    /// Initialize SwapV2 instruction builder
    pub fn create_swap_v2_builder(&self) -> Result<SwapV2InstructionBuilder> {
        let rpc_url = &self.app_config.rpc_url;
        let raydium_program_id = &self.app_config.raydium_program_id;

        SwapV2InstructionBuilder::new(rpc_url, raydium_program_id, 0).map_err(|e| anyhow::anyhow!("Failed to create SwapV2InstructionBuilder: {}", e))
    }

    /// Initialize SwapV2 instruction builder with custom parameters
    pub fn create_swap_v2_builder_with_params(rpc_url: &str, raydium_program_id: &str, amm_config_index: u16) -> Result<SwapV2InstructionBuilder> {
        SwapV2InstructionBuilder::new(rpc_url, raydium_program_id, amm_config_index)
            .map_err(|e| anyhow::anyhow!("Failed to create SwapV2InstructionBuilder: {}", e))
    }

    /// Initialize Solana client
    pub fn create_solana_client(&self) -> Result<SolanaClient> {
        let config = self.get_config_with_private_key()?;
        SolanaClient::new(&config).map_err(|e| anyhow::anyhow!("Failed to create SolanaClient: {}", e))
    }

    /// Initialize Raydium swap service
    pub fn create_raydium_swap(&self) -> Result<RaydiumSwap> {
        let config = self.get_config_with_private_key()?;
        let client = self.create_solana_client()?;

        RaydiumSwap::new(client, &config).map_err(|e| anyhow::anyhow!("Failed to create RaydiumSwap: {}", e))
    }

    /// Get app configuration
    pub fn get_app_config(&self) -> &AppConfig {
        &self.app_config
    }

    /// Update app configuration
    pub fn update_app_config(&mut self, new_config: AppConfig) {
        self.app_config = new_config;
    }

    /// Validate configuration
    pub fn validate_config(&self) -> Result<()> {
        // Validate RPC URL
        if self.app_config.rpc_url.is_empty() {
            return Err(anyhow::anyhow!("RPC URL is not configured"));
        }

        // Validate Raydium program ID
        if self.app_config.raydium_program_id.is_empty() {
            return Err(anyhow::anyhow!("Raydium program ID is not configured"));
        }

        // Validate private key if required for write operations
        if self.app_config.private_key.is_none() {
            info!("⚠️ Private key not configured - only read-only operations available");
        }

        info!("✅ Configuration validation passed");
        Ok(())
    }

    /// Check if private key is available
    pub fn has_private_key(&self) -> bool {
        self.app_config.private_key.is_some()
    }

    /// Get environment-specific configuration
    pub fn get_env_config() -> Result<SwapConfig> {
        let rpc_url = std::env::var("RPC_URL").unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".to_string());
        let amm_program_id = std::env::var("RAYDIUM_PROGRAM_ID").unwrap_or_else(|_| DEFAULT_RAYDIUM_PROGRAM_ID.to_string());
        let private_key = std::env::var("PRIVATE_KEY").unwrap_or_default();

        Ok(SwapConfig {
            rpc_url,
            private_key,
            amm_program_id,
            openbook_program_id: "".to_string(),
            usdc_mint: USDC_MINT_STANDARD.to_string(),
            sol_usdc_pool_id: "".to_string(),
        })
    }
}

/// ClientFactory provides centralized client creation
pub struct ClientFactory;

impl ClientFactory {
    /// Create all necessary clients for the service
    pub fn create_all_clients(app_config: AppConfig) -> Result<(Arc<RpcClient>, RaydiumApiClient, SwapV2Service, SwapV2InstructionBuilder)> {
        let config_manager = ConfigurationManager::new(app_config);

        let rpc_client = config_manager.create_rpc_client();
        let api_client = config_manager.create_api_client();
        let swap_v2_service = config_manager.create_swap_v2_service();
        let swap_v2_builder = config_manager.create_swap_v2_builder()?;

        Ok((rpc_client, api_client, swap_v2_service, swap_v2_builder))
    }

    /// Create clients with environment variables
    pub fn create_clients_from_env() -> Result<(Arc<RpcClient>, RaydiumApiClient, SwapV2Service, SwapV2InstructionBuilder)> {
        let rpc_url = std::env::var("RPC_URL").unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".to_string());
        let raydium_program_id = std::env::var("RAYDIUM_PROGRAM_ID").unwrap_or_else(|_| DEFAULT_RAYDIUM_PROGRAM_ID.to_string());

        let rpc_client = Arc::new(RpcClient::new(rpc_url.clone()));
        let api_client = RaydiumApiClient::new();
        let swap_v2_service = SwapV2Service::new(&rpc_url);
        let swap_v2_builder =
            SwapV2InstructionBuilder::new(&rpc_url, &raydium_program_id, 0).map_err(|e| anyhow::anyhow!("Failed to create SwapV2InstructionBuilder: {}", e))?;

        Ok((rpc_client, api_client, swap_v2_service, swap_v2_builder))
    }
}
