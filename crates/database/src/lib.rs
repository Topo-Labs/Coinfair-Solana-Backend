////////////////////////////////////////////////////////////////////////
//
// 1. 每个Domain(Entity)单独一个文件夹
// 2. 每个Domain由两部分组成:
//    - model: 定义Schema
//    - repository: 实际的数据库底层操作
//
//////////////////////////////////////////////////////////////////////



use std::sync::Arc;
use tracing::info;
use mongodb::{Client, Collection}; // 源码中集成了mongodb，因此数据是直接存储在这个程序中的(此处的是driver还是mongodb本身?)
use utils::{AppConfig, AppResult, CargoEnv};

pub mod refer;
use refer::model::Refer;

pub mod user;
use user::model::User;

pub mod reward;
use reward::model::Reward;

#[derive(Clone, Debug)]
pub struct Database {
    pub refers: Collection<Refer>,
    pub users: Collection<User>, 
    pub rewards: Collection<Reward>, 
}

impl Database {
    pub async fn new(config: Arc<AppConfig>) -> AppResult<Self> {
        let client = Client::with_uri_str(&config.mongo_uri).await?;

        let db = match &config.cargo_env {
            CargoEnv::Development => {
                client.database(&config.mongo_db_test)
            }
            CargoEnv::Production => {
                client.database(&config.mongo_db)
            }
        };
        let refers = db.collection("Refer");  
        let users = db.collection("User");    
        let rewards = db.collection("Reward");

        info!("🧱 database({:#}) connected.", match &config.cargo_env {
            CargoEnv::Development => {
                &config.mongo_db_test
            }
            CargoEnv::Production => {
                &config.mongo_db
            }
        });


        Ok(Database { refers, users, rewards })
    }
}
