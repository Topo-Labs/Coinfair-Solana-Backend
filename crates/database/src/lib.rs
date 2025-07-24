////////////////////////////////////////////////////////////////////////
//
// 1. 每个Domain(Entity)单独一个文件夹
// 2. 每个Domain由两部分组成:
//    - model: 定义Schema
//    - repository: 实际的数据库底层操作
//
//////////////////////////////////////////////////////////////////////

use mongodb::{Client, Collection}; // 源码中集成了mongodb，因此数据是直接存储在这个程序中的(此处的是driver还是mongodb本身?)
use std::sync::Arc;
use tracing::info;
use utils::{AppConfig, AppResult};

pub mod refer;
use refer::model::Refer;

pub mod user;
use user::model::User;

pub mod reward;
use reward::model::Reward;

pub mod clmm_pool;
use clmm_pool::model::ClmmPool;

#[derive(Clone, Debug)]
pub struct Database {
    pub refers: Collection<Refer>,
    pub users: Collection<User>,
    pub rewards: Collection<Reward>,
    pub clmm_pools: Collection<ClmmPool>,
}

impl Database {
    pub async fn new(config: Arc<AppConfig>) -> AppResult<Self> {
        let client = Client::with_uri_str(&config.mongo_uri).await?;

        // let db = match &config.cargo_env {
        //     CargoEnv::Development => {
        //         client.database(&config.mongo_db_test)
        //     }
        //     CargoEnv::Production => {
        //         client.database(&config.mongo_db)
        //     }
        // };

        let db: mongodb::Database = client.database(&config.mongo_db);

        let refers = db.collection("Refer");
        let users = db.collection("User");
        let rewards = db.collection("Reward");
        let clmm_pools = db.collection("ClmmPool");

        info!("🧱 database({:#}) connected.", &config.mongo_db);

        Ok(Database { refers, users, rewards, clmm_pools })
    }
}
