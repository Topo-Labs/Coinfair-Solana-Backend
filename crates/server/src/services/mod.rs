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
        let repository = Arc::new(db);

        let user = Arc::new(UserService::new(repository.clone())) as DynUserService;
        let refer = Arc::new(ReferService::new(repository.clone())) as DynReferService;
        let reward = Arc::new(RewardService::new(repository.clone())) as DynRewardService;

        info!("🧠 initializing services...");

        Self {
            user,
            refer,
            reward,
        }
    }
}
