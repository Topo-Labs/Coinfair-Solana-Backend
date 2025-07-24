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
pub mod solana;
pub mod solana_service;
pub mod user_service;

use crate::services::{
    refer_service::{DynReferService, ReferService},
    reward_service::{DynRewardService, RewardService},
    solana::{DynSolanaService, SolanaService},
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
    pub solana: DynSolanaService,
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

                let repository = Arc::new(db.clone());
                let user = Arc::new(UserService::new(repository.clone())) as DynUserService;
                let refer = Arc::new(ReferService::new(repository.clone())) as DynReferService;
                let reward = Arc::new(RewardService::new(repository.clone())) as DynRewardService;
                
                // 创建带数据库的SolanaService
                let solana = match SolanaService::with_database(db) {
                    Ok(service) => Arc::new(service) as DynSolanaService,
                    Err(e) => {
                        tracing::warn!("Failed to create SolanaService with database: {}, using default", e);
                        Arc::new(SolanaService::default()) as DynSolanaService
                    }
                };

                info!("🧠 Services initialized with default configuration");

                Self { user, refer, reward, solana }
            }
        }
    }

    /// 从环境变量创建Services (生产环境推荐)
    pub fn from_env(db: Database) -> Result<Self, Box<dyn std::error::Error>> {
        let repository = Arc::new(db.clone());

        let user = Arc::new(UserService::new(repository.clone())) as DynUserService;
        let refer = Arc::new(ReferService::new(repository.clone())) as DynReferService;
        let reward = Arc::new(RewardService::new(repository.clone())) as DynRewardService;
        
        // 创建带数据库的SolanaService
        let solana = Arc::new(SolanaService::with_database(db)?) as DynSolanaService;

        info!("🧠 initializing services from environment...");

        Ok(Self { user, refer, reward, solana })
    }
}
