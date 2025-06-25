// use crate::dtos::user_dto::GetUserDto;

use async_trait::async_trait;
use database::user::{model::User, repository::DynUserRepository};
use mongodb::results::{InsertManyResult, InsertOneResult};
// use mongodb::results::InsertOneResult;
use std::sync::Arc;
// use tracing::{error, info};
use utils::AppResult;

pub type DynUserService = Arc<dyn UserServiceTrait + Send + Sync>;

#[async_trait]
pub trait UserServiceTrait {
    async fn get_user(&self, address: String) -> AppResult<Option<User>>;
    async fn create_user(
        &self,
        address: String,
        amount: f64,
        price: f64,
    ) -> AppResult<InsertOneResult>;
    async fn create_users(&self, users: Vec<User>) -> AppResult<InsertManyResult>;
}

#[derive(Clone)]
pub struct UserService {
    repository: DynUserRepository,
}

impl UserService {
    pub fn new(repository: DynUserRepository) -> Self {
        Self { repository }
    }
}

#[async_trait]
impl UserServiceTrait for UserService {
    async fn get_user(&self, address: String) -> AppResult<Option<User>> {
        // let address = request.address.unwrap();

        let user = self.repository.get_user(&address).await?;

        Ok(user)
    }

    async fn create_user(
        &self,
        address: String,
        amount: f64,
        price: f64,
    ) -> AppResult<InsertOneResult> {
        let user = self.repository.create_user(&address, amount, price).await?;

        Ok(user)
    }

    async fn create_users(&self, users: Vec<User>) -> AppResult<InsertManyResult> {
        let users = self.repository.create_users(users).await?;

        Ok(users)
    }
}
