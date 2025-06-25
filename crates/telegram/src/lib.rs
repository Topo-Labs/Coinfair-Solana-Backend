// Telegram: 向用户提供数据查询，消息通知，奖励发放等功能

mod hope;
mod idle;
mod types;
mod utils;

// use crate::hope::schema as hope_schema;
// pub use crate::{idle::*, types::*};
use server::services::Services;
use std::sync::Arc;
use teloxide::{
    dispatching::dialogue::{self, InMemStorage},
    prelude::*,
};
use tracing::info;

#[derive(Clone)]
pub struct HopeBot {
    pub services: Arc<Services>,
    pub bot: Arc<Bot>,
}

impl HopeBot {
    pub fn new(token: String, services: Services) -> Self {
        Self {
            services: Arc::new(services),
            bot: Arc::new(Bot::new(token)),
        }
    }

    pub fn default(services: Services) -> Self {
        let token = "8182733161:AAFiI1DobchXrivL2DjkbPbESqNCULQ3q4U".to_string();
        Self {
            services: Arc::new(services),
            bot: Arc::new(Bot::new(token)),
        }
    }

    // pub async fn run(&self) {
    //     let schema_tree = dialogue::enter::<Update, InMemStorage<BotState>, BotState, _>()
    //         .branch(
    //             dptree::filter(|state: BotState| matches!(state, BotState::Hope(_)))
    //                 .branch(hope_schema()),
    //         )
    //         .branch(idle_schema());

    //     let this = self.clone();

    //     tokio::spawn(async move {
    //         Dispatcher::builder(this.bot.clone(), schema_tree)
    //             .dependencies(dptree::deps![
    //                 this.services.clone(),
    //                 InMemStorage::<BotState>::new() //smart_money_state
    //             ])
    //             .enable_ctrlc_handler()
    //             .build()
    //             .dispatch()
    //             .await;
    //     });

    //     info!("🤖 MoveBot running ...");
    // }

    pub async fn run(&self) {
        info!("🤖 TODO: MoveBot running ...");
    }
}
