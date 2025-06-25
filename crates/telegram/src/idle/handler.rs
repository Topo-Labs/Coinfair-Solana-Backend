// use super::{keyboard::*, message::*};
// use crate::{types::*, utils::*};
// use std::path::PathBuf;
// use teloxide::{
//     prelude::*,
//     types::{InputFile, Message, MessageKind},
// };

// static MAIN_PNG: &str = "./asset/main.png";

// pub async fn handler_invite_code(
//     bot: Bot,
//     q: CallbackQuery,
//     dialogue: MainDialogue,
// ) -> HandlerResult {
//     bot.answer_callback_query(q.id).await?;

//     bot.send_photo(dialogue.chat_id(), InputFile::file(PathBuf::from(MAIN_PNG)))
//         .caption(message_home())
//         .reply_markup(keyboard_home())
//         .await?;

//     bot.edit_message_reply_markup(dialogue.chat_id(), q.message.as_ref().unwrap().id)
//         .await?;

//     Ok(())
// }

// pub async fn handler_home(bot: Bot, q: CallbackQuery, dialogue: MainDialogue) -> HandlerResult {
//     bot.answer_callback_query(q.id).await?;

//     dialogue.update(BotState::Idle).await?;

//     bot.send_photo(dialogue.chat_id(), InputFile::file(PathBuf::from(MAIN_PNG)))
//         .caption(message_home())
//         .reply_markup(keyboard_home())
//         .await?;

//     Ok(())
// }

// pub async fn invalid_state(bot: Bot, msg: Message) -> HandlerResult {
//     bot.send_message(
//         msg.chat.id,
//         "Unable to handle the smart money message. Type /help to see the usage.",
//     )
//     .await?;
//     Ok(())
// }

// pub async fn handler_start(bot: Bot, msg: Message) -> HandlerResult {
//     println!("user_id: {:#?}", user_id(&msg));

//     // TODO:
//     // 从数据库查询该user_id是否已经被邀请。如果尚未绑定邀请码，则发送Welcome组件；否则发送Main组件

//     bot.send_photo(msg.chat.id, InputFile::file(PathBuf::from(MAIN_PNG)))
//         .caption(message_welcome(user_name_from_message(msg.clone())))
//         .reply_markup(keyboard_invite_code())
//         .await?;

//     Ok(())
// }

// pub async fn handler_portfolio(bot: Bot, msg: Message) -> HandlerResult {
//     bot.send_message(msg.chat.id, "🏗️ List your Portfolio in web")
//         .await?;

//     Ok(())
// }

// pub async fn handler_others(bot: Bot, q: CallbackQuery, dialogue: MainDialogue) -> HandlerResult {
//     bot.answer_callback_query(q.id).await?;

//     bot.send_message(dialogue.chat_id(), "🏗️ Others is under developing")
//         .await?;

//     Ok(())
// }

// pub async fn handler_dashboard(bot: Bot, msg: Message) -> HandlerResult {
//     bot.send_message(msg.chat.id, "🏗️ Show your Dashboard in web")
//         .await?;

//     Ok(())
// }

// //----------------------------------------------------------------

// pub async fn handler_coinfair_hope(bot: Bot, msg: Message) -> HandlerResult {
//     println!("Show $HOPE info");

//     bot.send_message(msg.chat.id, "Show $HOPE info").await?;

//     Ok(())
// }

// pub async fn handler_coinfair_buy(bot: Bot, msg: Message, dialogue: MainDialogue) -> HandlerResult {
//     println!("Buy $HOPE with xxx $BNB");

//     bot.send_message(dialogue.chat_id(), "Buy $HOPE with xxx $BNB")
//         .await?;

//     Ok(())
// }

// pub async fn handler_coinfair_sell(
//     bot: Bot,
//     msg: Message,
//     dialogue: MainDialogue,
// ) -> HandlerResult {
//     println!("Sell xxx $HOPE for $BNB");

//     bot.send_message(dialogue.chat_id(), "Sell xxx $HOPE for $BNB")
//         .await?;

//     Ok(())
// }

// pub async fn handler_coinfair_dashboard(
//     bot: Bot,
//     msg: Message,
//     dialogue: MainDialogue,
// ) -> HandlerResult {
//     println!("Sell xxx $HOPE for $BNB");

//     bot.send_message(dialogue.chat_id(), "Sell xxx $HOPE for $BNB")
//         .await?;

//     Ok(())
// }
