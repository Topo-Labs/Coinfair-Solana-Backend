use teloxide::{
    prelude::*,
    types::{MessageKind, User},
};
pub fn user_name_from_callback(query: CallbackQuery) -> String {
    let user: User = query.from;

    if let Some(username) = &user.username {
        username.to_string()
    } else {
        "".to_string()
    }
}

pub fn user_name_from_message(message: Message) -> String {
    if let Some(user) = message.from() {
        if let Some(username) = &user.username {
            println!("Username: @{}", username);
            username.to_string()
        } else {
            println!("User does not have a username.");
            "".to_string()
        }
    } else {
        println!("Message has no sender information.");
        "".to_string()
    }
}

pub fn user_id(msg: &Message) -> Option<UserId> {
    if let MessageKind::Common(common) = &msg.kind {
        if let Some(user) = &common.from {
            return Some(user.id);
        }
    }
    None
}
