// use crate::{
//     base::invalid_state,
//     coinfair::{handler::handler_coinfair_callback, CoinfairState},
//     match_callback,
//     smart_money::handler::{handler_smart_money, handler_smart_money_callback},
//     types::*,
// };
// use command::*;
// use handler::*;
// use teloxide::{
//     dispatching::{
//         dialogue::{self, InMemStorage},
//         UpdateFilterExt, UpdateHandler,
//     },
//     prelude::*,
// };

// mod command;
// pub mod handler;
// pub mod keyboard;
// mod message;

// pub fn idle_schema() -> UpdateHandler<Box<dyn std::error::Error + Send + Sync + 'static>> {
//     use dptree::case;
//     let command_handler = teloxide::filter_command::<Command, _>().branch(
//         case![BotState::Idle]
//             .branch(case![Command::Help].endpoint(handler_help))
//             .branch(case![Command::Start].endpoint(handler_start))
//             .branch(case![Command::SmartMoney].endpoint(handler_smart_money))
//             .branch(case![Command::Portfolio].endpoint(handler_portfolio))
//             .branch(case![Command::Dashboard].endpoint(handler_dashboard)),
//     );

//     // let command_idle_handler = teloxide::filter_command::<CommandCoinfair, _>().branch(
//     //     case![BotState::Idle]
//     //         .branch(case![CommandCoinfair::HOPE].endpoint(handler_coinfair_hope))
//     //         .branch(case![CommandCoinfair::Buy].endpoint(handler_coinfair_buy))
//     //         .branch(case![CommandCoinfair::Sell].endpoint(handler_coinfair_sell))
//     //         .branch(case![CommandCoinfair::Dashboard].endpoint(handler_coinfair_dashboard))
//     //         .branch(case![CommandCoinfair::Help].endpoint(handler_coinfair_help)),
//     // );

//     // 不挂载前提状态，即所有状态下都可以处理
//     let command_coinfair_handler = teloxide::filter_command::<CommandCoinfair, _>()
//         .branch(case![CommandCoinfair::HOPE].endpoint(handler_coinfair_hope))
//         .branch(case![CommandCoinfair::Buy].endpoint(handler_coinfair_buy))
//         .branch(case![CommandCoinfair::Sell].endpoint(handler_coinfair_sell))
//         .branch(case![CommandCoinfair::Dashboard].endpoint(handler_coinfair_dashboard))
//         .branch(case![CommandCoinfair::Help].endpoint(handler_coinfair_help));

//     let message_handler = Update::filter_message()
//         .branch(command_handler)
//         // .branch(command_idle_handler)
//         .branch(command_coinfair_handler)
//         .branch(dptree::endpoint(invalid_state));

//     let callback_query_handler = Update::filter_callback_query().branch(
//         case![BotState::Idle]
//             .branch(match_callback!("invite_code", handler_invite_code))
//             .branch(match_callback!("coinfair", handler_coinfair_callback))
//             .branch(match_callback!("smart_money", handler_smart_money_callback))
//             .branch(match_callback!("others", handler_others))
//             .branch(match_callback!("home", handler_home)),
//     );
//     // FIXME: Idle状态，点击其它query提示进入 /start

//     dialogue::enter::<Update, InMemStorage<BotState>, _, _>()
//         .branch(message_handler)
//         .branch(callback_query_handler)
// }
