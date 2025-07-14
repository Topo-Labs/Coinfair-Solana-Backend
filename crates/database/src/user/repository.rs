use crate::{user::model::User, Database};
use async_trait::async_trait;
use chrono::Utc;
use futures::stream::StreamExt;
use mongodb::{
    bson::doc,
    results::{InsertManyResult, InsertOneResult},
    Cursor,
};
use std::{collections::HashSet, sync::Arc};
use utils::{AppError, AppResult};

pub type DynUserRepository = Arc<dyn UserRepositoryTrait + Send + Sync>;

// 主要用于Service中，表示提供了该Trait功能
#[async_trait]
pub trait UserRepositoryTrait {
    // 创建用户(不会由前端调用，仅仅来自链上监听事件)
    async fn create_user(
        &self,
        address: &str,
        amount: f64,
        price: f64,
    ) -> AppResult<InsertOneResult>;

    // 获取活动开始后的新用户
    async fn get_user(&self, address: &str) -> AppResult<Option<User>>;

    async fn create_users(&self, users: Vec<User>) -> AppResult<InsertManyResult>;
}

#[async_trait]
impl UserRepositoryTrait for Database {
    async fn create_user(
        &self,
        address: &str,
        amount: f64,
        price: f64,
    ) -> AppResult<InsertOneResult> {
        let existing_user = self
            .users
            .find_one(doc! { "address": address.to_lowercase()}, None)
            .await?;

        if existing_user.is_some() {
            return Err(AppError::Conflict(format!(
                "Valid User with address: {} already exists.",
                address
            )));
        }

        let new_doc = User {
            id: None,
            address: address.to_string().to_lowercase(),
            amount: amount.floor().to_string(),
            price: format!("{:.20}", price),
            timestamp: Utc::now().timestamp() as u64,
        };

        let user = self.users.insert_one(new_doc, None).await?;

        Ok(user)
    }

    async fn get_user(&self, address: &str) -> AppResult<Option<User>> {
        let filter = doc! {"address": address};
        let user_detail = self.users.find_one(filter, None).await?;

        Ok(user_detail)
    }

    async fn create_users(&self, users: Vec<User>) -> AppResult<InsertManyResult> {
        // Step 1: Deduplicate `refers` based on the `lower` field
        let mut seen_users = HashSet::new();
        let unique_users: Vec<User> = users
            .into_iter()
            .filter(|user| seen_users.insert(user.address.clone().to_lowercase()))
            .collect();

        // Step 2: Extract all unique `lower` addresses
        let users: Vec<String> = unique_users
            .iter()
            .map(|user| user.address.clone().to_lowercase())
            .collect();

        // Step 3: Query the database for existing `lower` addresses
        let cursor: Cursor<User> = self
            .users
            .find(doc! { "address": { "$in": users }}, None)
            .await?;

        // Step 4: Collect all existing lowers from the cursor
        let mut existing_users: HashSet<String> = HashSet::new();

        let mut cursor = cursor;
        while let Some(doc) = cursor.next().await {
            match doc {
                Ok(d) => {
                    existing_users.insert(d.address); // Insert `lower` into the set
                }
                Err(_) => continue, // Ignore error and continue with next document
            }
        }

        // Step 5: Filter out already existing lowers
        let users_to_insert: Vec<User> = unique_users
            .into_iter()
            .filter(|user| !existing_users.contains(&user.address)) // Keep only new `lower`
            .collect();

        if users_to_insert.is_empty() {
            return Err(AppError::Conflict("All users already exist.".to_string()));
        }

        // Step 6: Insert the remaining `Refer` documents
        let result = self.users.insert_many(users_to_insert, None).await?;

        Ok(result)
    }
}
