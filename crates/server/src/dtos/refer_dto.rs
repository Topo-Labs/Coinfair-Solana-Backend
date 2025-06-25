use database::refer::model::Refer;
use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Clone, Serialize, Deserialize, Debug, Validate, Default)]
pub struct SetRefersDto {
    pub refers: Vec<Refer>,
}
