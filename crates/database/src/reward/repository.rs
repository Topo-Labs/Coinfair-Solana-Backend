use crate::{
    reward::model::{Reward, RewardItem, RewardItemWithTime},
    Database,
};
use anyhow::Context;
use async_trait::async_trait;
use chrono::{NaiveDate, TimeZone, Utc};
// use futures::stream::TryStreamExt;
use mongodb::{
    bson::{doc, DateTime},
    results::{InsertManyResult, InsertOneResult, UpdateResult},
};
use std::cmp::Ordering;
use std::{collections::hash_map::HashMap, sync::Arc};
use tokio_stream::StreamExt;
use tracing::info;
use utils::{AppError, AppResult};

pub type DynRewardRepository = Arc<dyn RewardRepositoryTrait + Send + Sync>;

#[async_trait]
pub trait RewardRepositoryTrait {
    // 创建奖励
    async fn create_reward(&self, user_address: &str, rewards: Vec<RewardItem>) -> AppResult<InsertOneResult>;

    // 将某笔奖励设置为已发放
    async fn set_reward(&self, user_address: &str) -> AppResult<UpdateResult>;

    // 批量设置奖励(用于项目方确认奖励已发放)
    async fn set_rewards(&self, users: Vec<String>) -> AppResult<UpdateResult>;

    // 获取某个用户所触发的奖励
    async fn get_reward(&self, user_address: &str) -> AppResult<Option<Reward>>;

    // 获取某一天的所有奖励
    async fn get_rewards_by_day(&self, day: &str) -> AppResult<Vec<RewardItem>>;

    // 获取所有尚未发放的奖励
    async fn get_all_rewards(&self) -> AppResult<Vec<RewardItem>>;

    // 设置所有尚未发放的奖励为已发放
    async fn set_all_rewards(&self) -> AppResult<UpdateResult>;

    // 获取奖励榜单
    async fn get_rank_rewards(&self) -> AppResult<Vec<RewardItem>>;

    // 获取某个地址的奖励列表
    async fn list_rewards_by_address(&self, address: &str) -> AppResult<Vec<RewardItemWithTime>>;

    async fn mock_rewards(&self, rewards: Vec<Reward>) -> AppResult<InsertManyResult>;

    // async fn delete_user(&self, id: &str) -> AppResult<DeleteResult>;
    //
    // async fn get_all_users(&self) -> AppResult<Vec<User>>;
}

#[async_trait]
impl RewardRepositoryTrait for Database {
    async fn create_reward(&self, user_address: &str, rewards: Vec<RewardItem>) -> AppResult<InsertOneResult> {
        let existing_user = self.rewards.find_one(doc! { "user_address": user_address.to_lowercase()}, None).await?;

        if existing_user.is_some() {
            return Err(AppError::Conflict(format!("Reward of User with address: {} already exists.", user_address)));
        }

        let new_doc = Reward {
            is_rewarded: false,
            user_address: user_address.to_string().to_lowercase(),
            rewards,
            timestamp: Utc::now().timestamp() as u64,
        };

        let reward = self.rewards.insert_one(new_doc, None).await?;

        Ok(reward)
    }

    async fn set_reward(&self, user_address: &str) -> AppResult<UpdateResult> {
        let filter = doc! {"user_address": user_address.to_lowercase()};
        let update = doc! {
            "$set":
                {
                    "is_rewarded": true,
                },
        };

        let updated_doc = self.users.update_one(filter, update, None).await?;

        Ok(updated_doc)
    }

    async fn set_rewards(&self, users: Vec<String>) -> AppResult<UpdateResult> {
        let filter = doc! {"user_address": {"$in": users}};
        let update = doc! {
            "$set":
                {
                    "is_rewarded": true,
                },
        };

        let updated_doc = self.users.update_many(filter, update, None).await?;

        Ok(updated_doc)
    }

    async fn get_reward(&self, user_address: &str) -> AppResult<Option<Reward>> {
        let filter = doc! {"user_address": user_address.to_lowercase()};
        let reward = self.rewards.find_one(filter, None).await?;

        Ok(reward)
    }

    // 获取某一天应该发放的奖励
    async fn get_rewards_by_day(&self, day: &str) -> AppResult<Vec<RewardItem>> {
        let naive_date = NaiveDate::parse_from_str(day, "%Y-%m-%d").context("Invalid date format")?;

        let start_of_day = Utc.from_utc_datetime(&naive_date.and_hms_opt(0, 0, 0).unwrap());
        let end_of_day = Utc.from_utc_datetime(&naive_date.and_hms_opt(23, 59, 59).unwrap());

        let filter = doc! {
            "timestamp": {
                "$gte": DateTime::from_millis(start_of_day.timestamp_millis()),
                "$lte": DateTime::from_millis(end_of_day.timestamp_millis())
            },
            "is_rewarded": false
        };

        let mut cursor = self.rewards.find(filter, None).await?;

        let mut reward_map: HashMap<String, f64> = HashMap::new();

        while let Some(reward) = cursor.try_next().await? {
            for item in reward.rewards {
                let entry = reward_map.entry(item.address).or_insert(0.0);
                *entry += item.amount;
            }
        }

        let result: Vec<RewardItem> = reward_map.into_iter().map(|(address, amount)| RewardItem { address, amount }).collect();

        Ok(result)
    }

    // 获取所有应该发放的奖励（截止今天）
    async fn get_all_rewards(&self) -> AppResult<Vec<RewardItem>> {
        let filter = doc! {
            "is_rewarded": false
        };

        let mut cursor = self.rewards.find(filter, None).await?;

        let mut reward_map: HashMap<String, f64> = HashMap::new();

        while let Some(reward) = cursor.try_next().await? {
            for item in reward.rewards {
                let entry = reward_map.entry(item.address).or_insert(0.0);
                *entry += item.amount;
            }
        }

        let result: Vec<RewardItem> = reward_map.into_iter().map(|(address, amount)| RewardItem { address, amount }).collect();

        Ok(result)
    }

    // 设置所有待发放奖励为已发放
    async fn set_all_rewards(&self) -> AppResult<UpdateResult> {
        let filter = doc! {"is_rewarded": false};
        let update = doc! {
            "$set":
                {
                    "is_rewarded": true,
                },
        };

        info!("set_all_rewards in repo.rs");
        let updated_doc = self.rewards.update_many(filter, update, None).await?;

        Ok(updated_doc)
    }

    // 获取奖励榜单
    async fn get_rank_rewards(&self) -> AppResult<Vec<RewardItem>> {
        let filter = doc! {};

        let mut cursor = self.rewards.find(filter, None).await?;

        let mut reward_map: HashMap<String, f64> = HashMap::new();

        while let Some(reward) = cursor.try_next().await? {
            for item in reward.rewards {
                let entry = reward_map.entry(item.address).or_insert(0.0);
                *entry += item.amount;
            }
        }

        let mut result: Vec<RewardItem> = reward_map.into_iter().map(|(address, amount)| RewardItem { address, amount }).collect();

        result.sort_by(|a, b| b.amount.partial_cmp(&a.amount).unwrap_or(Ordering::Equal));
        Ok(result)
    }

    // 获取某个地址的奖励列表
    async fn list_rewards_by_address(&self, address: &str) -> AppResult<Vec<RewardItemWithTime>> {
        let filter = doc! {
            "rewards.address": address // 查询 `rewards` 数组中包含 `address`
        };

        let mut cursor = self.rewards.find(filter, None).await?;

        let mut reward_items = Vec::new();

        while let Some(reward) = cursor.try_next().await? {
            for item in reward.rewards.iter().filter(|r| r.address == address) {
                reward_items.push(RewardItemWithTime {
                    address: item.address.clone(),
                    amount: item.amount,
                    timestamp: reward.timestamp,
                    user_address: reward.user_address.clone(),
                });
            }
        }

        // **按 timestamp 逆序排序**
        reward_items.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        Ok(reward_items)
    }

    async fn mock_rewards(&self, rewards: Vec<Reward>) -> AppResult<InsertManyResult> {
        let result = self.rewards.insert_many(rewards, None).await?;

        Ok(result)
    }
}
