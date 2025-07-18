use database::user::model::User;
use ethers::prelude::*;
use serde::{Deserialize, Serialize};
use validator::Validate;
// #[derive(Clone, Serialize, Deserialize, Debug, Validate, Default)]
// pub struct SignUpUserDto {
//     #[validate(required, length(min = 1))]
//     pub name: Option<String>,
//     #[validate(required, length(min = 1), email(message = "email is invalid"))]
//     pub email: Option<String>,
//     #[validate(required, length(min = 6))]
//     pub password: Option<String>,
// }

#[derive(Clone, Serialize, Deserialize, Debug, Validate, Default)]
pub struct GetUserDto {
    #[validate(required, length(min = 1))]
    pub address: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, Debug, Validate, Default)]
pub struct SetUsersDto {
    pub users: Vec<User>,
}
