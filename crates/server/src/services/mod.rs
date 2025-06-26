////////////////////////////////////////////////////////////////////////
//
// 1. 每个Domain(Entity)单独一个文件夹
// 2. 每个Domain由两部分组成:
//    - model: 定义Schema
//    - repository: 实际的数据库底层操作
//
//////////////////////////////////////////////////////////////////////

pub mod refer_service;
pub mod reward_service;
pub mod user_service;

use crate::services::{
    refer_service::{DynReferService, ReferService},
    reward_service::{DynRewardService, RewardService},
    user_service::{DynUserService, UserService},
};
use database::Database;
use std::sync::Arc;
use tracing::info;

#[derive(Clone)]
pub struct Services {
    pub user: DynUserService,
    pub refer: DynReferService,
    pub reward: DynRewardService,
}

impl Services {
    pub fn new(db: Database) -> Self {
        // 优先尝试从环境变量创建，否则使用默认配置
        match Self::from_env(db.clone()) {
            Ok(services) => {
                info!("🧠 Services initialized from environment variables");
                services
            }
            Err(e) => {
                tracing::warn!("Failed to initialize from environment: {}, using default config", e);
                
                let repository = Arc::new(db);
                let user = Arc::new(UserService::new(repository.clone())) as DynUserService;
                let refer = Arc::new(ReferService::new(repository.clone())) as DynReferService;
                let reward = Arc::new(RewardService::new(repository.clone())) as DynRewardService;
                
                // 使用默认配置和临时密钥
                let dummy_keypair = Keypair::new();
                let swap = Arc::new(SwapService::new(
                    "https://api.devnet.solana.com".to_string(),
                    "CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK".to_string(),
                    0.01, // 1% slippage tolerance
                    0,    // AMM config index
                    dummy_keypair,
                ).expect("Failed to create swap service")) as DynSwapService;

                info!("🧠 Services initialized with default configuration");

                Self {
                    user,
                    refer,
                    reward,
                }
            }
        }
    }
    
    /// 从环境变量创建Services (生产环境推荐)
    pub fn from_env(db: Database) -> Result<Self, Box<dyn std::error::Error>> {
        let repository = Arc::new(db);

        let user = Arc::new(UserService::new(repository.clone())) as DynUserService;
        let refer = Arc::new(ReferService::new(repository.clone())) as DynReferService;
        let reward = Arc::new(RewardService::new(repository.clone())) as DynRewardService;
        
        // 从环境变量中读取配置
        let rpc_url = std::env::var("SOLANA_RPC_URL")
            .unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".to_string());
        
        let raydium_program_id = std::env::var("RAYDIUM_V3_PROGRAM_ID")
            .unwrap_or_else(|_| "CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK".to_string());
        
        let slippage_tolerance: f64 = std::env::var("SLIPPAGE_TOLERANCE")
            .unwrap_or_else(|_| "0.005".to_string())
            .parse()
            .unwrap_or(0.005);
        
        let amm_config_index: u16 = std::env::var("AMM_CONFIG_INDEX")
            .unwrap_or_else(|_| "0".to_string())
            .parse()
            .unwrap_or(0);
        
        // 从环境变量中读取私钥 (base58格式)
        let payer_secret_key = std::env::var("PAYER_SECRET_KEY")
            .unwrap_or_else(|_| {
                tracing::warn!("PAYER_SECRET_KEY not found, using dummy keypair");
                // 返回一个dummy keypair的base58编码
                bs58::encode(&Keypair::new().to_bytes()).into_string()
            });
        
        let keypair_bytes = bs58::decode(&payer_secret_key)
            .into_vec()
            .map_err(|e| format!("Failed to decode payer secret key: {}", e))?;
        
        let swap = Arc::new(SwapService::from_config(
            rpc_url,
            raydium_program_id,
            slippage_tolerance,
            amm_config_index,
            keypair_bytes,
        ).map_err(|e| format!("Failed to create swap service: {:?}", e))?
        ) as DynSwapService;

        info!("🧠 initializing services from environment...");

        Ok(Self {
            user,
            refer,
            reward,
        })
    }
}
