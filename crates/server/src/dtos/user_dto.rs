use database::user::model::User;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::Validate;

#[derive(Clone, Serialize, Deserialize, Debug, Validate, Default, ToSchema)]
pub struct GetUserDto {
    #[validate(required, length(min = 1))]
    pub address: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, Debug, Validate, Default, ToSchema)]
pub struct SetUsersDto {
    pub users: Vec<User>,
}
