use crate::{refer::model::Refer, Database};
use async_trait::async_trait;
use chrono::prelude::Utc;
use futures::stream::StreamExt;
use mongodb::{
    bson::doc,
    results::{InsertManyResult, InsertOneResult},
    Cursor,
};
use std::{collections::HashSet, sync::Arc};
use utils::{AppError, AppResult};

pub type DynReferRepository = Arc<dyn ReferRepositoryTrait + Send + Sync>;

// 主要用于Service中，表示提供了该Trait功能
#[async_trait]
pub trait ReferRepositoryTrait {
    // 插入某个上下级关系(监听链上)
    async fn create_refer(&self, lower: &str, upper: &str) -> AppResult<InsertOneResult>;

    // 批量插入上下级关系(api调用)
    // TODO: 权限控制
    async fn create_refers(&self, refers: Vec<Refer>) -> AppResult<InsertManyResult>;

    // 获取某个地址的上级()
    async fn get_upper(&self, address: &str) -> AppResult<Option<String>>;

    // 获取某个地址的上级&上上级
    async fn get_uppers(&self, address: &str) -> AppResult<Vec<String>>;

    async fn get_user(&self, lower: &str) -> AppResult<Option<Refer>>;
    // // 获取某个地址的所有下级
    // async fn get_lowers(&self, id: &str) -> AppResult<Option<User>>;

    // // 获取某个地址的所有下级和下下级
    // async fn get_lowers_chain(&self, email: &str) -> AppResult<Option<User>>;
}

#[async_trait]
impl ReferRepositoryTrait for Database {
    async fn create_refer(&self, lower: &str, upper: &str) -> AppResult<InsertOneResult> {
        let existing_lower = self
            .refers
            .find_one(doc! { "lower": lower.to_lowercase()}, None)
            .await?;

        if existing_lower.is_some() {
            return Err(AppError::Conflict(format!(
                "Lower with address: {} already exists.",
                lower
            )));
        }

        let new_doc = Refer {
            lower: lower.to_string().to_lowercase(),
            upper: upper.to_string().to_lowercase(),
            timestamp: Utc::now().timestamp() as u64,
        };

        let refer = self.refers.insert_one(new_doc, None).await?;

        Ok(refer)
    }

    // TODO: 待简化
    async fn create_refers(&self, refers: Vec<Refer>) -> AppResult<InsertManyResult> {
        // Step 1: Deduplicate `refers` based on the `lower` field
        let mut seen_lowers = HashSet::new();
        let unique_refers: Vec<Refer> = refers
            .into_iter()
            .filter(|refer| seen_lowers.insert(refer.lower.clone().to_lowercase())) // Only keep unique `lower`
            .collect();

        // Step 2: Extract all unique `lower` addresses
        let lowers: Vec<String> = unique_refers
            .iter()
            .map(|refer| refer.lower.clone().to_lowercase())
            .collect();

        // Step 3: Query the database for existing `lower` addresses
        let cursor: Cursor<Refer> = self
            .refers
            .find(doc! { "lower": { "$in": lowers }}, None)
            .await?;

        // Step 4: Collect all existing lowers from the cursor
        let mut existing_lowers: HashSet<String> = HashSet::new();

        let mut cursor = cursor;
        while let Some(doc) = cursor.next().await {
            match doc {
                Ok(d) => {
                    existing_lowers.insert(d.lower); // Insert `lower` into the set
                }
                Err(_) => continue, // Ignore error and continue with next document
            }
        }

        // Step 5: Filter out already existing lowers
        let refers_to_insert: Vec<Refer> = unique_refers
            .into_iter()
            .filter(|refer| !existing_lowers.contains(&refer.lower)) // Keep only new `lower`
            .collect();

        if refers_to_insert.is_empty() {
            return Err(AppError::Conflict("All refers already exist.".to_string()));
        }

        // Step 6: Insert the remaining `Refer` documents
        let result = self.refers.insert_many(refers_to_insert, None).await?;

        Ok(result)
    }

    async fn get_upper(&self, lower: &str) -> AppResult<Option<String>> {
        let filter = doc! {"lower": lower};
        let refer = self.refers.find_one(filter, None).await?;

        Ok(refer.map(|r| r.upper))
    }

    async fn get_uppers(&self, lower: &str) -> AppResult<Vec<String>> {
        let mut result = Vec::new();
        let mut current_lower = lower.to_string().to_lowercase();

        // 获取上级和上上级(再高的级别就不获取了)
        for _ in 0..2 {
            if let Some(upper) = self.get_upper(&current_lower).await? {
                result.push(upper.clone());
                current_lower = upper.to_lowercase();
            } else {
                break;
            }
        }

        Ok(result)
    }

    async fn get_user(&self, lower: &str) -> AppResult<Option<Refer>> {
        let filter = doc! {"lower": lower};
        let refer = self.refers.find_one(filter, None).await?;

        Ok(refer)
    }
}
