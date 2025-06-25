// use crate::HandlerResult;
// use teloxide::{prelude::*, utils::command::BotCommands};

// // BotState::Idle状态下支持的Commands.
// #[derive(BotCommands, Clone)]
// #[command(rename_rule = "lowercase", description = "——— MoveBot ———")]
// pub enum Command {
//     #[command(description = "📝 Show help information")]
//     Help,

//     #[command(description = "🛸 Enter MoveBot's world")]
//     Start,

//     #[command(description = "💵 Monitor & follow the Smart Money flow")]
//     SmartMoney,

//     #[command(description = "🏛️ List the portfolio")]
//     Portfolio,

//     #[command(description = "📈 Show the statistics")]
//     Dashboard,
// }

// // BotState::Idle状态下支持的Commands.
// #[derive(BotCommands, Clone)]
// #[command(rename_rule = "lowercase", description = "——— Coinfair & $HOPE ———")]
// pub enum CommandCoinfair {
//     #[command(description = "👀 Show $HOPE info")]
//     HOPE,

//     #[command(description = "🟢 Buy $HOPE")]
//     Buy,

//     #[command(description = "🔴 Sell $HOPE")]
//     Sell,

//     #[command(description = "📈 Show the Profit & Loss")]
//     Dashboard,

//     #[command(description = "📝 Help information")]
//     Help,
// }

// pub async fn handler_help(bot: Bot, msg: Message) -> HandlerResult {
//     bot.send_message(msg.chat.id, Command::descriptions().to_string())
//         .await?;

//     Ok(())
// }

// pub async fn handler_coinfair_help(bot: Bot, msg: Message) -> HandlerResult {
//     bot.send_message(msg.chat.id, CommandCoinfair::descriptions().to_string())
//         .await?;

//     Ok(())
// }

// #[derive(BotCommands, Clone)]
// #[command(rename_rule = "snake_case", description = "——— Admin ———")]
// pub enum AdminCommand {
//     #[command(description = "📝 Show admin help")]
//     Help,

//     // 查看某个用户的详情
//     #[command(description = "💵 Show user info")]
//     User(String),

//     // 查看当前总体业务详情
//     #[command(description = "📈 Show all statistics")]
//     Static,
// }
