// use crate::{match_callback, match_state, BotState};
// // use chain::CHAINS_INFO;
// // use engine::strategies::smart_money::state::State;
// use ethers::types::Address;
// use handler::*;
// // use std::sync::Arc;
// use teloxide::{dispatching::UpdateHandler, prelude::*, utils::command::BotCommands};
// // use tokio::sync::RwLock;

// pub mod handler;
// pub mod keyboard;
// // pub mod listener;
// pub mod message;
// // pub mod state;
// // pub mod tx;

// #[derive(Clone, Default, Debug)]
// pub enum CoinfairState {
//     #[default]
//     Start,
//     ReceiveChainId,
//     // UserAddress, ContractAddress, TokenAddress
//     ReceiveAddress {
//         chain_id: u32,
//     },
// }

// #[derive(BotCommands, Clone, Debug)]
// #[command(description = "Commands:", rename_rule = "lowercase")]
// pub enum Command {
//     #[command(description = "Display all commands")]
//     List,
//     #[command(description = "Subscribe to receive notifications of token transfers")]
//     Subscribe,
//     #[command(
//         description = "Unsubscribe of token transfer, by passing in the id. Ids can be obtained in the /subs command"
//     )]
//     Unsubscribe(u32),
//     #[command(description = "Display all current token subscriptions")]
//     Subs,
//     #[command(description = "Cancel susbscription process")]
//     Cancel,
// }

// // // NOTE: 以后每个Bot的feature模块，搞清楚要提供给MoveBot什么
// // // smart_money: state实例， schema()即handler(), ChatState类型
// // pub async fn setup_smart_money(
// //     bot: Bot,
// // ) -> (
// //     Arc<RwLock<State>>,
// //     UpdateHandler<Box<dyn std::error::Error + Send + Sync + 'static>>,
// // ) {
// //     // 2. 构造初始化的State
// //     // TODO: 思考——每个功能模块所依赖的内存数据，其实例是在该功能模块内部创建，然后向外暴露；还是将类型提供给Global，有后者创建实例
// //     //     - 如果每个功能模块，有自己的注入逻辑，那么就放在该模块内部创建实例
// //
// //     // let state = Arc::new(RwLock::new(State::new()));
// //     let state = Arc::new(RwLock::new(State::init().await));
// //
// //     // 3. 对所有支持的链，每条链开启一个线程，并注入对应的链，State, Bot
// //     // TODO: 链生态单独抽取出来
// //     for chain in CHAINS_INFO.values() {
// //         let bot_clone = bot.clone();
// //         let state_clone = state.clone();
// //         tokio::spawn(async move {
// //             listener::listener(bot_clone, chain, state_clone).await;
// //         });
// //     }
// //
// //     // NOTE: 每个功能模块，需要向外提供三个参数
// //     // - state: 内部需要维护的数据结构
// //     // - schema: 处理逻辑
// //     // - dialogue_state: 对话状态机（提供类型）
// //     (state, schema())
// // }

// pub fn schema() -> UpdateHandler<Box<dyn std::error::Error + Send + Sync + 'static>> {
//     let message_handler = Update::filter_message()
//         .branch(
//             dptree::filter(|state: BotState| {
//                 matches!(
//                     state,
//                     BotState::Coinfair(CoinfairState::ReceiveAddress { chain_id })
//                 )
//             })
//             .endpoint(receive_token_address),
//         )
//         // NOTE: 无效状态 (此处没有case!所定义的状态，所以会进入invalid_state)
//         .branch(dptree::endpoint(invalid_message));

//     let callback_query_handler = Update::filter_callback_query()
//         .branch(
//             dptree::filter(|state: BotState| {
//                 matches!(state, BotState::Coinfair(CoinfairState::Start))
//             })
//             .branch(match_callback!("hope_token_info", hope_token_info))
//             .branch(match_callback!("buy", buy))
//             .branch(match_callback!("sell", sell))
//             .branch(match_callback!("dashboard", dashboard)),
//         )
//         .branch(
//             dptree::filter(|state: BotState| {
//                 matches!(state, BotState::Coinfair(CoinfairState::Start))
//             })
//             .branch(match_callback!("back", back_home)),
//         )
//         .branch(
//             dptree::filter(|state: BotState| {
//                 matches!(
//                     state,
//                     BotState::Coinfair(CoinfairState::ReceiveAddress { .. })
//                 )
//             })
//             .branch(match_callback!("back", back_smartmoney_receive_chain)),
//         )
//         .branch(
//             dptree::filter(|state: BotState| {
//                 matches!(state, BotState::Coinfair(CoinfairState::ReceiveChainId))
//             })
//             .branch(match_callback!("back", back_smartmoney_home)),
//         )
//         .branch(match_state!(
//             BotState::Coinfair(CoinfairState::ReceiveChainId),
//             receive_chain_id
//         ))
//         .branch(
//             dptree::filter(|state: BotState| matches!(state, BotState::Idle))
//                 .branch(match_callback!("Foundry Anvil", invalid_callback)),
//         );

//     dptree::entry()
//         .branch(message_handler)
//         .branch(callback_query_handler)
// }
