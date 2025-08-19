use database::refer::model::Refer;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::Validate;

/// 批量设置推荐关系的请求体
#[derive(Clone, Serialize, Deserialize, Debug, Validate, Default, ToSchema)]
pub struct SetRefersDto {
    /// 推荐关系列表
    pub refers: Vec<Refer>,
}
