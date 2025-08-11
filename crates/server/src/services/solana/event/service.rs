use anyhow::Result;
use database::{
    event_model::{
        repository::{NftClaimStats, RewardStats},
        NftClaimEvent, RewardDistributionEvent,
    },
    Database,
};
use futures::TryStreamExt;
use mongodb::bson::{doc, Document};
use mongodb::options::FindOptions;
use std::sync::Arc;
use tracing::info;

/// 事件服务 - 处理NFT领取和奖励分发事件的查询
pub struct EventService {
    database: Arc<Database>,
}

impl EventService {
    /// 创建新的事件服务实例
    pub fn new(database: Arc<Database>) -> Self {
        Self { database }
    }

    // ==================== NFT领取事件查询 ====================

    /// 根据领取者地址查询NFT领取事件
    pub async fn get_nft_claim_events_by_claimer(
        &self,
        claimer: &str,
        page: Option<u64>,
        page_size: Option<u64>,
        sort_by: Option<String>,
        sort_order: Option<String>,
    ) -> Result<PaginatedResponse<NftClaimEvent>> {
        info!("🔍 查询领取者 {} 的NFT领取事件", claimer);

        let page = page.unwrap_or(1);
        let page_size = page_size.unwrap_or(20).min(100); // 最大100条
        let skip = (page - 1) * page_size;
        let sort_field = sort_by.unwrap_or_else(|| "claimed_at".to_string());
        let sort_direction = if sort_order.unwrap_or_else(|| "desc".to_string()) == "asc" { 1 } else { -1 };

        // 构建查询条件
        let filter = doc! { "claimer": claimer };

        // 构建排序
        let sort = doc! { &sort_field: sort_direction };

        // 构建查询选项
        let find_options = FindOptions::builder().skip(skip).limit(page_size as i64).sort(sort).build();

        // 查询总数
        let total = self.database.nft_claim_events.count_documents(filter.clone(), None).await? as u64;

        // 查询数据
        let cursor = self.database.nft_claim_events.find(filter, find_options).await?;

        let items: Vec<NftClaimEvent> = cursor.try_collect().await?;

        let total_pages = (total + page_size - 1) / page_size;

        Ok(PaginatedResponse {
            items,
            total,
            page,
            page_size,
            total_pages,
        })
    }

    /// 根据NFT mint地址查询领取事件
    pub async fn get_nft_claim_events_by_nft_mint(&self, nft_mint: &str, page: Option<u64>, page_size: Option<u64>) -> Result<PaginatedResponse<NftClaimEvent>> {
        info!("🔍 查询NFT {} 的领取事件", nft_mint);

        let page = page.unwrap_or(1);
        let page_size = page_size.unwrap_or(20).min(100);
        let skip = (page - 1) * page_size;

        let filter = doc! { "nft_mint": nft_mint };
        let sort = doc! { "claimed_at": -1 };

        let find_options = FindOptions::builder().skip(skip).limit(page_size as i64).sort(sort).build();

        let total = self.database.nft_claim_events.count_documents(filter.clone(), None).await? as u64;

        let cursor = self.database.nft_claim_events.find(filter, find_options).await?;

        let items: Vec<NftClaimEvent> = cursor.try_collect().await?;

        let total_pages = (total + page_size - 1) / page_size;

        Ok(PaginatedResponse {
            items,
            total,
            page,
            page_size,
            total_pages,
        })
    }

    /// 分页查询所有NFT领取事件（支持过滤）
    pub async fn get_nft_claim_events_paginated(
        &self,
        page: Option<u64>,
        page_size: Option<u64>,
        tier: Option<u8>,
        has_referrer: Option<bool>,
        start_date: Option<i64>,
        end_date: Option<i64>,
        sort_by: Option<String>,
        sort_order: Option<String>,
    ) -> Result<PaginatedResponse<NftClaimEvent>> {
        info!("🔍 分页查询NFT领取事件");

        let page = page.unwrap_or(1);
        let page_size = page_size.unwrap_or(20).min(100);
        let skip = (page - 1) * page_size;
        let sort_field = sort_by.unwrap_or_else(|| "claimed_at".to_string());
        let sort_direction = if sort_order.unwrap_or_else(|| "desc".to_string()) == "asc" { 1 } else { -1 };

        // 构建过滤条件
        let mut filter = Document::new();

        if let Some(tier) = tier {
            filter.insert("tier", tier as i32);
        }

        if let Some(has_referrer) = has_referrer {
            filter.insert("has_referrer", has_referrer);
        }

        // 日期范围过滤
        if start_date.is_some() || end_date.is_some() {
            let mut date_filter = Document::new();
            if let Some(start) = start_date {
                date_filter.insert("$gte", start);
            }
            if let Some(end) = end_date {
                date_filter.insert("$lte", end);
            }
            filter.insert("claimed_at", date_filter);
        }

        let sort = doc! { &sort_field: sort_direction };

        let find_options = FindOptions::builder().skip(skip).limit(page_size as i64).sort(sort).build();

        let total = self.database.nft_claim_events.count_documents(filter.clone(), None).await? as u64;

        let cursor = self.database.nft_claim_events.find(filter, find_options).await?;

        let items: Vec<NftClaimEvent> = cursor.try_collect().await?;

        let total_pages = (total + page_size - 1) / page_size;

        Ok(PaginatedResponse {
            items,
            total,
            page,
            page_size,
            total_pages,
        })
    }

    /// 高级分页查询所有NFT领取事件（支持复杂过滤条件）
    pub async fn get_nft_claim_events_advanced(
        &self,
        page: Option<u64>,
        page_size: Option<u64>,
        tier: Option<u8>,
        has_referrer: Option<bool>,
        start_date: Option<i64>,
        end_date: Option<i64>,
        sort_by: Option<String>,
        sort_order: Option<String>,
        referrer: Option<String>,
        claimer: Option<String>,
        nft_mint: Option<String>,
        claim_amount_min: Option<u64>,
        claim_amount_max: Option<u64>,
        claim_type: Option<u8>,
        is_emergency_claim: Option<bool>,
        pool_address: Option<String>,
        token_mint: Option<String>,
        reward_multiplier_min: Option<u16>,
        reward_multiplier_max: Option<u16>,
    ) -> Result<PaginatedResponse<NftClaimEvent>> {
        info!("🔍 高级分页查询NFT领取事件");

        let page = page.unwrap_or(1);
        let page_size = page_size.unwrap_or(20).min(100);
        let skip = (page - 1) * page_size;
        let sort_field = sort_by.unwrap_or_else(|| "claimed_at".to_string());
        let sort_direction = if sort_order.unwrap_or_else(|| "desc".to_string()) == "asc" { 1 } else { -1 };

        // 构建高级过滤条件
        let mut filter = Document::new();

        // 基础过滤条件
        if let Some(tier) = tier {
            filter.insert("tier", tier as i32);
        }

        if let Some(has_referrer) = has_referrer {
            filter.insert("has_referrer", has_referrer);
        }

        // 日期范围过滤
        if start_date.is_some() || end_date.is_some() {
            let mut date_filter = Document::new();
            if let Some(start) = start_date {
                date_filter.insert("$gte", start);
            }
            if let Some(end) = end_date {
                date_filter.insert("$lte", end);
            }
            filter.insert("claimed_at", date_filter);
        }

        // 高级过滤条件
        if let Some(referrer) = referrer {
            filter.insert("referrer", referrer);
        }

        if let Some(claimer) = claimer {
            filter.insert("claimer", claimer);
        }

        if let Some(nft_mint) = nft_mint {
            filter.insert("nft_mint", nft_mint);
        }

        if let Some(pool_address) = pool_address {
            filter.insert("pool_address", pool_address);
        }

        if let Some(token_mint) = token_mint {
            filter.insert("token_mint", token_mint);
        }

        if let Some(claim_type) = claim_type {
            filter.insert("claim_type", claim_type as i32);
        }

        if let Some(is_emergency_claim) = is_emergency_claim {
            filter.insert("is_emergency_claim", is_emergency_claim);
        }

        // 奖励金额范围过滤
        if claim_amount_min.is_some() || claim_amount_max.is_some() {
            let mut amount_filter = Document::new();
            if let Some(min) = claim_amount_min {
                amount_filter.insert("$gte", min as i64);
            }
            if let Some(max) = claim_amount_max {
                amount_filter.insert("$lte", max as i64);
            }
            filter.insert("claim_amount", amount_filter);
        }

        // 奖励倍率范围过滤
        if reward_multiplier_min.is_some() || reward_multiplier_max.is_some() {
            let mut multiplier_filter = Document::new();
            if let Some(min) = reward_multiplier_min {
                multiplier_filter.insert("$gte", min as i32);
            }
            if let Some(max) = reward_multiplier_max {
                multiplier_filter.insert("$lte", max as i32);
            }
            filter.insert("reward_multiplier", multiplier_filter);
        }

        let sort = doc! { &sort_field: sort_direction };

        let find_options = FindOptions::builder().skip(skip).limit(page_size as i64).sort(sort).build();

        let total = self.database.nft_claim_events.count_documents(filter.clone(), None).await? as u64;

        let cursor = self.database.nft_claim_events.find(filter, find_options).await?;

        let items: Vec<NftClaimEvent> = cursor.try_collect().await?;

        let total_pages = (total + page_size - 1) / page_size;

        Ok(PaginatedResponse {
            items,
            total,
            page,
            page_size,
            total_pages,
        })
    }

    /// 获取NFT领取统计信息
    pub async fn get_nft_claim_stats(&self) -> Result<NftClaimStats> {
        info!("📊 获取NFT领取统计信息");

        let stats = self.database.nft_claim_event_repository.get_nft_claim_stats().await?;

        Ok(stats)
    }

    // ==================== 奖励分发事件查询 ====================

    /// 根据接收者地址查询奖励分发事件
    pub async fn get_reward_events_by_recipient(
        &self,
        recipient: &str,
        page: Option<u64>,
        page_size: Option<u64>,
        is_locked: Option<bool>,
        reward_type: Option<u8>,
        sort_by: Option<String>,
        sort_order: Option<String>,
    ) -> Result<PaginatedResponse<RewardDistributionEvent>> {
        info!("🔍 查询接收者 {} 的奖励分发事件", recipient);

        let page = page.unwrap_or(1);
        let page_size = page_size.unwrap_or(20).min(100);
        let skip = (page - 1) * page_size;
        let sort_field = sort_by.unwrap_or_else(|| "distributed_at".to_string());
        let sort_direction = if sort_order.unwrap_or_else(|| "desc".to_string()) == "asc" { 1 } else { -1 };

        // 构建查询条件
        let mut filter = doc! { "recipient": recipient };

        if let Some(is_locked) = is_locked {
            filter.insert("is_locked", is_locked);
        }

        if let Some(reward_type) = reward_type {
            filter.insert("reward_type", reward_type as i32);
        }

        let sort = doc! { &sort_field: sort_direction };

        let find_options = FindOptions::builder().skip(skip).limit(page_size as i64).sort(sort).build();

        let total = self.database.reward_distribution_events.count_documents(filter.clone(), None).await? as u64;

        let cursor = self.database.reward_distribution_events.find(filter, find_options).await?;

        let items: Vec<RewardDistributionEvent> = cursor.try_collect().await?;

        let total_pages = (total + page_size - 1) / page_size;

        Ok(PaginatedResponse {
            items,
            total,
            page,
            page_size,
            total_pages,
        })
    }

    /// 根据分发ID查询奖励事件
    pub async fn get_reward_event_by_distribution_id(&self, distribution_id: u64) -> Result<Option<RewardDistributionEvent>> {
        info!("🔍 查询分发ID {} 的奖励事件", distribution_id);

        let event = self.database.reward_distribution_event_repository.find_by_distribution_id(distribution_id).await?;

        Ok(event)
    }

    /// 分页查询所有奖励分发事件（支持过滤）
    pub async fn get_reward_events_paginated(
        &self,
        page: Option<u64>,
        page_size: Option<u64>,
        is_locked: Option<bool>,
        reward_type: Option<u8>,
        reward_source: Option<u8>,
        is_referral_reward: Option<bool>,
        start_date: Option<i64>,
        end_date: Option<i64>,
        sort_by: Option<String>,
        sort_order: Option<String>,
    ) -> Result<PaginatedResponse<RewardDistributionEvent>> {
        info!("🔍 分页查询奖励分发事件");

        let page = page.unwrap_or(1);
        let page_size = page_size.unwrap_or(20).min(100);
        let skip = (page - 1) * page_size;
        let sort_field = sort_by.unwrap_or_else(|| "distributed_at".to_string());
        let sort_direction = if sort_order.unwrap_or_else(|| "desc".to_string()) == "asc" { 1 } else { -1 };

        // 构建过滤条件
        let mut filter = Document::new();

        if let Some(is_locked) = is_locked {
            filter.insert("is_locked", is_locked);
        }

        if let Some(reward_type) = reward_type {
            filter.insert("reward_type", reward_type as i32);
        }

        if let Some(reward_source) = reward_source {
            filter.insert("reward_source", reward_source as i32);
        }

        if let Some(is_referral_reward) = is_referral_reward {
            filter.insert("is_referral_reward", is_referral_reward);
        }

        // 日期范围过滤
        if start_date.is_some() || end_date.is_some() {
            let mut date_filter = Document::new();
            if let Some(start) = start_date {
                date_filter.insert("$gte", start);
            }
            if let Some(end) = end_date {
                date_filter.insert("$lte", end);
            }
            filter.insert("distributed_at", date_filter);
        }

        let sort = doc! { &sort_field: sort_direction };

        let find_options = FindOptions::builder().skip(skip).limit(page_size as i64).sort(sort).build();

        let total = self.database.reward_distribution_events.count_documents(filter.clone(), None).await? as u64;

        let cursor = self.database.reward_distribution_events.find(filter, find_options).await?;

        let items: Vec<RewardDistributionEvent> = cursor.try_collect().await?;

        let total_pages = (total + page_size - 1) / page_size;

        Ok(PaginatedResponse {
            items,
            total,
            page,
            page_size,
            total_pages,
        })
    }

    /// 获取奖励分发统计信息
    pub async fn get_reward_stats(&self) -> Result<RewardStats> {
        info!("📊 获取奖励分发统计信息");

        let stats = self.database.reward_distribution_event_repository.get_reward_stats().await?;

        Ok(stats)
    }

    /// 获取用户的奖励汇总信息
    pub async fn get_user_reward_summary(&self, recipient: &str) -> Result<UserRewardSummary> {
        info!("📊 获取用户 {} 的奖励汇总信息", recipient);

        // 使用聚合管道计算汇总信息
        let pipeline = vec![
            doc! {
                "$match": {
                    "recipient": recipient
                }
            },
            doc! {
                "$group": {
                    "_id": null,
                    "total_rewards": { "$sum": 1 },
                    "total_amount": { "$sum": "$reward_amount" },
                    "locked_amount": {
                        "$sum": {
                            "$cond": [
                                { "$eq": ["$is_locked", true] },
                                "$reward_amount",
                                0
                            ]
                        }
                    },
                    "unlocked_amount": {
                        "$sum": {
                            "$cond": [
                                { "$eq": ["$is_locked", false] },
                                "$reward_amount",
                                0
                            ]
                        }
                    },
                    "referral_rewards": {
                        "$sum": {
                            "$cond": [
                                { "$eq": ["$is_referral_reward", true] },
                                1,
                                0
                            ]
                        }
                    },
                    "referral_amount": {
                        "$sum": {
                            "$cond": [
                                { "$eq": ["$is_referral_reward", true] },
                                "$reward_amount",
                                0
                            ]
                        }
                    }
                }
            },
        ];

        let mut cursor = self.database.reward_distribution_events.aggregate(pipeline, None).await?;

        let summary = if let Some(doc) = cursor.try_next().await? {
            UserRewardSummary {
                recipient: recipient.to_string(),
                total_rewards: doc.get_i32("total_rewards").unwrap_or(0) as u64,
                total_amount: doc.get_i64("total_amount").unwrap_or(0) as u64,
                locked_amount: doc.get_i64("locked_amount").unwrap_or(0) as u64,
                unlocked_amount: doc.get_i64("unlocked_amount").unwrap_or(0) as u64,
                referral_rewards: doc.get_i32("referral_rewards").unwrap_or(0) as u64,
                referral_amount: doc.get_i64("referral_amount").unwrap_or(0) as u64,
            }
        } else {
            // 没有数据时返回空汇总
            UserRewardSummary {
                recipient: recipient.to_string(),
                total_rewards: 0,
                total_amount: 0,
                locked_amount: 0,
                unlocked_amount: 0,
                referral_rewards: 0,
                referral_amount: 0,
            }
        };

        Ok(summary)
    }

    /// 获取用户的NFT领取汇总信息
    pub async fn get_user_nft_claim_summary(&self, claimer: &str) -> Result<UserNftClaimSummary> {
        info!("📊 获取用户 {} 的NFT领取汇总信息", claimer);

        // 使用聚合管道计算汇总信息
        let pipeline = vec![
            doc! {
                "$match": {
                    "claimer": claimer
                }
            },
            doc! {
                "$group": {
                    "_id": null,
                    "total_claims": { "$sum": 1 },
                    "total_claim_amount": { "$sum": "$claim_amount" },
                    "total_bonus_amount": { "$sum": "$bonus_amount" },
                    "claims_with_referrer": {
                        "$sum": {
                            "$cond": [
                                { "$eq": ["$has_referrer", true] },
                                1,
                                0
                            ]
                        }
                    },
                    "tier_distribution": {
                        "$push": {
                            "tier": "$tier",
                            "tier_name": "$tier_name"
                        }
                    }
                }
            },
        ];

        let mut cursor = self.database.nft_claim_events.aggregate(pipeline, None).await?;

        let summary = if let Some(doc) = cursor.try_next().await? {
            // 处理tier分布
            let mut tier_counts = std::collections::HashMap::new();
            if let Ok(tier_array) = doc.get_array("tier_distribution") {
                for tier_doc in tier_array {
                    if let Some(tier_doc) = tier_doc.as_document() {
                        if let Ok(tier) = tier_doc.get_i32("tier") {
                            *tier_counts.entry(tier as u8).or_insert(0) += 1;
                        }
                    }
                }
            }

            UserNftClaimSummary {
                claimer: claimer.to_string(),
                total_claims: doc.get_i32("total_claims").unwrap_or(0) as u64,
                total_claim_amount: doc.get_i64("total_claim_amount").unwrap_or(0) as u64,
                total_bonus_amount: doc.get_i64("total_bonus_amount").unwrap_or(0) as u64,
                claims_with_referrer: doc.get_i32("claims_with_referrer").unwrap_or(0) as u64,
                tier_distribution: tier_counts.into_iter().collect(),
            }
        } else {
            UserNftClaimSummary {
                claimer: claimer.to_string(),
                total_claims: 0,
                total_claim_amount: 0,
                total_bonus_amount: 0,
                claims_with_referrer: 0,
                tier_distribution: vec![],
            }
        };

        Ok(summary)
    }
}

// ==================== 响应结构体定义 ====================

/// 分页响应
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PaginatedResponse<T> {
    pub items: Vec<T>,
    pub total: u64,
    pub page: u64,
    pub page_size: u64,
    pub total_pages: u64,
}

/// 用户奖励汇总
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UserRewardSummary {
    pub recipient: String,
    pub total_rewards: u64,
    pub total_amount: u64,
    pub locked_amount: u64,
    pub unlocked_amount: u64,
    pub referral_rewards: u64,
    pub referral_amount: u64,
}

/// 用户NFT领取汇总
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UserNftClaimSummary {
    pub claimer: String,
    pub total_claims: u64,
    pub total_claim_amount: u64,
    pub total_bonus_amount: u64,
    pub claims_with_referrer: u64,
    pub tier_distribution: Vec<(u8, u32)>,
}
