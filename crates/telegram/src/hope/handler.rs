// // 查询 $HOPE 的信息
// pub async fn hope_token_info(bot: Bot, q: CallbackQuery, dialogue: MainDialogue) -> HandlerResult {
//     bot.answer_callback_query(q.id).await?;

//     // let message_text = r#"Address: `0x123456789abcdef123456789abcdef12345678`(Click Copy)"#;

//     let message_text = r#"Address: `0x123456789abcdef123456789abcdef12345678`"#;

//     let keyboard = KeyboardMarkup::new(vec![
//         // vec![KeyboardButton::new("选项 1").request(), KeyboardButton::new("选项 2")],
//         vec![KeyboardButton::new("选项 1"), KeyboardButton::new("选项 2")],
//         vec![KeyboardButton::new("选项 3"), KeyboardButton::new("选项 4")],
//     ])
//     .input_field_placeholder("Choose the percent of BNB")
//     .resize_keyboard()
//     .one_time_keyboard();

//     bot.send_message(dialogue.chat_id(), message_text)
//         .reply_markup(keyboard)
//         .parse_mode(ParseMode::MarkdownV2)
//         .await?;

//     // // bot.send_message(dialogue.chat_id(), message_chain(None))
//     // //     .reply_markup(keyboard_chains())
//     // //     .await?;
//     //
//     // // bot.edit_message_text(
//     // //     dialogue.chat_id(),
//     // //     q.message.as_ref().unwrap().id,
//     // //     message_chain(None),
//     // // )
//     // // .reply_markup(keyboard_chains())
//     // // .await?;
//     //
//     // bot.edit_message_media(
//     //     dialogue.chat_id(),
//     //     q.message.as_ref().unwrap().id,
//     //     InputMedia::Photo(InputMediaPhoto {
//     //         media: InputFile::file(PathBuf::from(SM_SUBSCRIBE_PNG)),
//     //         caption: Some(message_chain(None)),
//     //         parse_mode: None,
//     //         caption_entities: None,
//     //         has_spoiler: false,
//     //     }),
//     // )
//     // .reply_markup(keyboard_chains())
//     // .await?;
//     //
//     // dialogue
//     //     .update(BotState::Coinfair(CoinfairState::ReceiveChainId))
//     //     .await?;

//     Ok(())
// }

// // 购买 $HOPE
// pub async fn buy(
//     bot: Bot,
//     q: CallbackQuery,
//     dialogue: MainDialogue,
//     state: Arc<Mutex<State>>,
// ) -> HandlerResult {
//     bot.answer_callback_query(q.id).await?;

//     bot.send_message(dialogue.chat_id(), "Buy $HOPE.").await?;

//     // let state = state.lock().await;
//     // let subs = state.get_user_subscriptions_formatted(&dialogue.chat_id());
//     // if let Some(subs) = subs {
//     //     bot.send_message(dialogue.chat_id(), message_subscribe(&subs))
//     //         .await?;
//     // } else {
//     //     bot.send_message(dialogue.chat_id(), format!("You currently have no subs"))
//     //         .await?;
//     // }

//     Ok(())
// }

// // 出售 $HOPE
// pub async fn sell(
//     bot: Bot,
//     q: CallbackQuery,
//     dialogue: MainDialogue,
//     state: Arc<Mutex<State>>,
// ) -> HandlerResult {
//     bot.answer_callback_query(q.id).await?;

//     bot.send_message(dialogue.chat_id(), "Sell $HOPE.").await?;

//     // let state = state.lock().await;
//     // let subs = state.get_user_subscriptions_formatted(&dialogue.chat_id());
//     // if let Some(subs) = subs {
//     //     bot.send_message(dialogue.chat_id(), message_subscribe(&subs))
//     //         .await?;
//     // } else {
//     //     bot.send_message(dialogue.chat_id(), format!("You currently have no subs"))
//     //         .await?;
//     // }

//     Ok(())
// }

// // 查看Dashboard
// pub async fn dashboard(bot: Bot, q: CallbackQuery, dialogue: MainDialogue) -> HandlerResult {
//     bot.answer_callback_query(q.id).await?;

//     bot.send_message(dialogue.chat_id(), "Show profit of all wallets.")
//         .await?;

//     // // bot.send_message(dialogue.chat_id(), message_chain(None))
//     // //     .reply_markup(keyboard_chains())
//     // //     .await?;
//     //
//     // // bot.edit_message_text(
//     // //     dialogue.chat_id(),
//     // //     q.message.as_ref().unwrap().id,
//     // //     message_chain(None),
//     // // )
//     // // .reply_markup(keyboard_chains())
//     // // .await?;
//     //
//     // bot.edit_message_media(
//     //     dialogue.chat_id(),
//     //     q.message.as_ref().unwrap().id,
//     //     InputMedia::Photo(InputMediaPhoto {
//     //         media: InputFile::file(PathBuf::from(SM_SUBSCRIBE_PNG)),
//     //         caption: Some(message_chain(None)),
//     //         parse_mode: None,
//     //         caption_entities: None,
//     //         has_spoiler: false,
//     //     }),
//     // )
//     // .reply_markup(keyboard_chains())
//     // .await?;
//     //
//     // dialogue
//     //     .update(BotState::Coinfair(CoinfairState::ReceiveChainId))
//     //     .await?;

//     Ok(())
// }
