// use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup, WebAppInfo};

// pub fn keyboard_invite_code() -> InlineKeyboardMarkup {
//     InlineKeyboardMarkup::default().append_row(vec![InlineKeyboardButton::callback(
//         "🟢 Verify Invitation Code",
//         "invite_code",
//     )])
// }

// pub fn keyboard_home() -> InlineKeyboardMarkup {
//     InlineKeyboardMarkup::default()
//         .append_row(vec![
//             InlineKeyboardButton::callback("Coinfair 🔥", "coinfair"),
//             InlineKeyboardButton::callback("Smart Money 🧠", "smart_money"),
//         ])
//         // .append_row(vec![InlineKeyboardButton::callback("Others 🏗️", "others")])
//         // .append_row(vec![InlineKeyboardButton::web_app(
//         //     "🕌 Community Support",
//         //     WebAppInfo {
//         //         url: reqwest::Url::parse("https://t.me/Coinfair_Global").unwrap(),
//         //     },
//         // )])
//         .append_row(vec![InlineKeyboardButton::url(
//             "🕌 Community Support",
//             reqwest::Url::parse("https://t.me/Coinfair_Global").unwrap(),
//         )])
// }
