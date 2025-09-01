use crate::event_model::{
    ClmmPoolEvent, DepositEvent, LaunchEvent, MigrationStatus, NftClaimEvent, RewardDistributionEvent,
};
use chrono::Utc;
use futures_util::TryStreamExt;
use mongodb::bson::doc;
use mongodb::options::IndexOptions;
use mongodb::{Collection, IndexModel};
use tracing::info;
use utils::AppResult;

/// 池子事件仓库
#[derive(Debug, Clone)]
pub struct ClmmPoolEventRepository {
    collection: Collection<ClmmPoolEvent>,
}

impl ClmmPoolEventRepository {
    pub fn new(collection: Collection<ClmmPoolEvent>) -> Self {
        Self { collection }
    }

    /// 初始化索引
    pub async fn init_indexes(&self) -> AppResult<()> {
        // 创建复合索引：池子地址 + 签名（唯一）
        let pool_signature_index = IndexModel::builder()
            .keys(doc! {
                "pool_address": 1,
                "signature": 1
            })
            .options(IndexOptions::builder().unique(true).build())
            .build();

        // 创建时间戳索引
        let created_at_index = IndexModel::builder().keys(doc! { "created_at": -1 }).build();

        // 创建创建者索引
        let creator_index = IndexModel::builder().keys(doc! { "creator": 1 }).build();

        // 创建代币对索引
        let token_pair_index = IndexModel::builder()
            .keys(doc! {
                "token_a_mint": 1,
                "token_b_mint": 1
            })
            .build();

        let indexes = vec![pool_signature_index, created_at_index, creator_index, token_pair_index];

        self.collection.create_indexes(indexes, None).await?;
        info!("✅ ClmmPoolEvent数据库索引初始化完成");
        Ok(())
    }

    /// 插入池子创建事件
    pub async fn insert_pool_event(&self, mut event: ClmmPoolEvent) -> AppResult<String> {
        event.updated_at = Utc::now().timestamp();

        let result = self.collection.insert_one(event, None).await?;

        Ok(result.inserted_id.as_object_id().unwrap().to_hex())
    }

    /// 根据池子地址查找事件
    pub async fn find_by_pool_address(&self, pool_address: &str) -> AppResult<Option<ClmmPoolEvent>> {
        let filter = doc! { "pool_address": pool_address };
        let result = self.collection.find_one(filter, None).await?;
        Ok(result)
    }

    /// 根据创建者查找所有池子事件
    pub async fn find_by_creator(&self, creator: &str) -> AppResult<Vec<ClmmPoolEvent>> {
        let filter = doc! { "creator": creator };
        let cursor = self.collection.find(filter, None).await?;

        let events: Vec<ClmmPoolEvent> = cursor.try_collect().await?;

        Ok(events)
    }

    /// 根据代币对查找池子事件
    pub async fn find_by_token_pair(&self, token_a: &str, token_b: &str) -> AppResult<Vec<ClmmPoolEvent>> {
        let filter = doc! {
            "$or": [
                {
                    "token_a_mint": token_a,
                    "token_b_mint": token_b
                },
                {
                    "token_a_mint": token_b,
                    "token_b_mint": token_a
                }
            ]
        };

        let cursor = self.collection.find(filter, None).await?;

        let events: Vec<ClmmPoolEvent> = cursor.try_collect().await?;

        Ok(events)
    }

    /// 获取池子事件统计
    pub async fn get_pool_stats(&self) -> AppResult<PoolEventStats> {
        // 统计总数
        let total_pools = self.collection.count_documents(doc! {}, None).await? as u64;

        // 统计今日新增池子
        let today_start = Utc::now()
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp();
        let today_new_pools = self
            .collection
            .count_documents(doc! { "created_at": { "$gte": today_start } }, None)
            .await? as u64;

        // 统计不同费率的池子数量
        let fee_rate_pipeline = vec![
            doc! {
                "$group": {
                    "_id": "$fee_rate",
                    "count": { "$sum": 1 }
                }
            },
            doc! {
                "$sort": { "count": -1 }
            },
        ];

        let mut cursor = self.collection.aggregate(fee_rate_pipeline, None).await?;

        let mut fee_rate_distribution = Vec::new();
        while let Some(doc) = cursor.try_next().await? {
            if let (Some(fee_rate), Some(count)) = (doc.get_i32("_id").ok(), doc.get_i32("count").ok()) {
                fee_rate_distribution.push((fee_rate as u32, count as u64));
            }
        }

        Ok(PoolEventStats {
            total_pools,
            today_new_pools,
            fee_rate_distribution,
        })
    }
}

/// NFT领取事件仓库
#[derive(Debug, Clone)]
pub struct NftClaimEventRepository {
    collection: Collection<NftClaimEvent>,
}

impl NftClaimEventRepository {
    pub fn new(collection: Collection<NftClaimEvent>) -> Self {
        Self { collection }
    }

    /// 初始化索引
    pub async fn init_indexes(&self) -> AppResult<()> {
        // 创建复合索引：NFT地址 + 签名（唯一）
        let nft_signature_index = IndexModel::builder()
            .keys(doc! {
                "nft_mint": 1,
                "signature": 1
            })
            .options(IndexOptions::builder().unique(true).build())
            .build();

        // 创建领取者索引
        let claimer_index = IndexModel::builder().keys(doc! { "claimer": 1 }).build();

        // 创建时间戳索引
        let claimed_at_index = IndexModel::builder().keys(doc! { "claimed_at": -1 }).build();

        // 创建等级索引
        let tier_index = IndexModel::builder().keys(doc! { "tier": 1 }).build();

        // 创建推荐人索引（支持推荐人地址过滤）
        let referrer_index = IndexModel::builder().keys(doc! { "referrer": 1 }).build();

        // 创建has_referrer索引（支持是否有推荐人过滤）
        let has_referrer_index = IndexModel::builder().keys(doc! { "has_referrer": 1 }).build();

        // 创建复合索引：推荐人 + 时间戳（优化推荐人历史查询）
        let referrer_claimed_at_index = IndexModel::builder()
            .keys(doc! {
                "referrer": 1,
                "claimed_at": -1
            })
            .build();

        // 创建奖励金额索引（支持金额范围过滤）
        let claim_amount_index = IndexModel::builder().keys(doc! { "claim_amount": 1 }).build();

        // 创建领取类型索引
        let claim_type_index = IndexModel::builder().keys(doc! { "claim_type": 1 }).build();

        // 创建紧急领取索引
        let emergency_claim_index = IndexModel::builder().keys(doc! { "is_emergency_claim": 1 }).build();

        // 创建池子地址索引
        let pool_address_index = IndexModel::builder().keys(doc! { "pool_address": 1 }).build();

        // 创建代币mint索引
        let token_mint_index = IndexModel::builder().keys(doc! { "token_mint": 1 }).build();

        // 创建奖励倍率索引
        let reward_multiplier_index = IndexModel::builder().keys(doc! { "reward_multiplier": 1 }).build();

        let indexes = vec![
            nft_signature_index,
            claimer_index,
            claimed_at_index,
            tier_index,
            referrer_index,
            has_referrer_index,
            referrer_claimed_at_index,
            claim_amount_index,
            claim_type_index,
            emergency_claim_index,
            pool_address_index,
            token_mint_index,
            reward_multiplier_index,
        ];

        self.collection.create_indexes(indexes, None).await?;

        info!("✅ NftClaimEvent数据库索引初始化完成（包含高级查询优化索引）");
        Ok(())
    }

    /// 插入NFT领取事件
    pub async fn insert_nft_claim_event(&self, mut event: NftClaimEvent) -> AppResult<String> {
        event.updated_at = Utc::now().timestamp();

        let result = self.collection.insert_one(event, None).await?;

        Ok(result.inserted_id.as_object_id().unwrap().to_hex())
    }

    /// 根据NFT地址查找事件
    pub async fn find_by_nft_mint(&self, nft_mint: &str) -> AppResult<Vec<NftClaimEvent>> {
        let filter = doc! { "nft_mint": nft_mint };
        let cursor = self.collection.find(filter, None).await?;

        let events: Vec<NftClaimEvent> = cursor.try_collect().await?;

        Ok(events)
    }

    /// 根据领取者查找所有领取事件
    pub async fn find_by_claimer(&self, claimer: &str) -> AppResult<Vec<NftClaimEvent>> {
        let filter = doc! { "claimer": claimer };
        let cursor = self.collection.find(filter, None).await?;

        let events: Vec<NftClaimEvent> = cursor.try_collect().await?;

        Ok(events)
    }

    /// 获取NFT领取统计
    pub async fn get_nft_claim_stats(&self) -> AppResult<NftClaimStats> {
        // 统计总领取次数
        let total_claims = self.collection.count_documents(doc! {}, None).await? as u64;

        // 统计今日领取次数
        let today_start = Utc::now()
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp();
        let today_claims = self
            .collection
            .count_documents(doc! { "claimed_at": { "$gte": today_start } }, None)
            .await? as u64;

        // 统计等级分布
        let tier_pipeline = vec![
            doc! {
                "$group": {
                    "_id": "$tier",
                    "count": { "$sum": 1 },
                    "total_amount": { "$sum": "$claim_amount" }
                }
            },
            doc! {
                "$sort": { "_id": 1 }
            },
        ];

        let mut cursor = self.collection.aggregate(tier_pipeline, None).await?;

        let mut tier_distribution = Vec::new();
        while let Some(doc) = cursor.try_next().await? {
            if let (Some(tier), Some(count), Some(total_amount)) = (
                doc.get_i32("_id").ok(),
                doc.get_i32("count").ok(),
                doc.get_i64("total_amount").ok(),
            ) {
                tier_distribution.push((tier as u8, count as u64, total_amount as u64));
            }
        }

        Ok(NftClaimStats {
            total_claims,
            today_claims,
            tier_distribution,
        })
    }
}

/// 奖励分发事件仓库
#[derive(Debug, Clone)]
pub struct RewardDistributionEventRepository {
    collection: Collection<RewardDistributionEvent>,
}

impl RewardDistributionEventRepository {
    pub fn new(collection: Collection<RewardDistributionEvent>) -> Self {
        Self { collection }
    }

    /// 初始化索引
    pub async fn init_indexes(&self) -> AppResult<()> {
        // 创建复合索引：分发ID + 签名（唯一）
        let distribution_signature_index = IndexModel::builder()
            .keys(doc! {
                "distribution_id": 1,
                "signature": 1
            })
            .options(IndexOptions::builder().unique(true).build())
            .build();

        // 创建接收者索引
        let recipient_index = IndexModel::builder().keys(doc! { "recipient": 1 }).build();

        // 创建时间戳索引
        let distributed_at_index = IndexModel::builder().keys(doc! { "distributed_at": -1 }).build();

        // 创建奖励类型索引
        let reward_type_index = IndexModel::builder().keys(doc! { "reward_type": 1 }).build();

        // 创建锁定状态索引
        let locked_index = IndexModel::builder().keys(doc! { "is_locked": 1 }).build();

        // ==================== 高级查询优化索引 ====================

        // 创建推荐人索引（支持推荐人地址过滤）
        let referrer_index = IndexModel::builder().keys(doc! { "referrer": 1 }).build();

        // 创建复合索引：推荐人 + 时间戳（优化推荐人历史查询）
        let referrer_distributed_at_index = IndexModel::builder()
            .keys(doc! {
                "referrer": 1,
                "distributed_at": -1
            })
            .build();

        // 创建奖励代币mint索引
        let reward_token_mint_index = IndexModel::builder().keys(doc! { "reward_token_mint": 1 }).build();

        // 创建奖励金额索引（支持金额范围过滤）
        let reward_amount_index = IndexModel::builder().keys(doc! { "reward_amount": 1 }).build();

        // 创建分发ID范围索引（已有distribution_signature_index包含了distribution_id）

        // 创建奖励池地址索引
        let reward_pool_index = IndexModel::builder().keys(doc! { "reward_pool": 1 }).build();

        // 创建has_referrer索引（支持是否有推荐人过滤）
        let has_referrer_index = IndexModel::builder().keys(doc! { "has_referrer": 1 }).build();

        // 创建is_referral_reward索引
        let is_referral_reward_index = IndexModel::builder().keys(doc! { "is_referral_reward": 1 }).build();

        // 创建高价值奖励索引
        let is_high_value_reward_index = IndexModel::builder().keys(doc! { "is_high_value_reward": 1 }).build();

        // 创建锁定天数索引
        let lock_days_index = IndexModel::builder().keys(doc! { "lock_days": 1 }).build();

        // 创建奖励倍率索引
        let multiplier_index = IndexModel::builder().keys(doc! { "multiplier": 1 }).build();

        // 创建相关地址索引
        let related_address_index = IndexModel::builder().keys(doc! { "related_address": 1 }).build();

        // 创建预估USD价值索引
        let estimated_usd_value_index = IndexModel::builder().keys(doc! { "estimated_usd_value": 1 }).build();

        // 创建奖励来源索引
        let reward_source_index = IndexModel::builder().keys(doc! { "reward_source": 1 }).build();

        // 创建复合索引：接收者 + 时间戳（优化用户历史查询）
        let recipient_distributed_at_index = IndexModel::builder()
            .keys(doc! {
                "recipient": 1,
                "distributed_at": -1
            })
            .build();

        let indexes = vec![
            distribution_signature_index,
            recipient_index,
            distributed_at_index,
            reward_type_index,
            locked_index,
            // 高级查询索引
            referrer_index,
            referrer_distributed_at_index,
            reward_token_mint_index,
            reward_amount_index,
            reward_pool_index,
            has_referrer_index,
            is_referral_reward_index,
            is_high_value_reward_index,
            lock_days_index,
            multiplier_index,
            related_address_index,
            estimated_usd_value_index,
            reward_source_index,
            recipient_distributed_at_index,
        ];

        self.collection.create_indexes(indexes, None).await?;

        info!("✅ RewardDistributionEvent数据库索引初始化完成（包含高级查询优化索引）");
        Ok(())
    }

    /// 插入奖励分发事件
    pub async fn insert_reward_event(&self, mut event: RewardDistributionEvent) -> AppResult<String> {
        event.updated_at = Utc::now().timestamp();

        let result = self.collection.insert_one(event, None).await?;

        Ok(result.inserted_id.as_object_id().unwrap().to_hex())
    }

    /// 根据接收者查找所有奖励事件
    pub async fn find_by_recipient(&self, recipient: &str) -> AppResult<Vec<RewardDistributionEvent>> {
        let filter = doc! { "recipient": recipient };
        let cursor = self.collection.find(filter, None).await?;

        let events: Vec<RewardDistributionEvent> = cursor.try_collect().await?;

        Ok(events)
    }

    /// 根据分发ID查找事件
    pub async fn find_by_distribution_id(&self, distribution_id: i64) -> AppResult<Option<RewardDistributionEvent>> {
        let filter = doc! { "distribution_id": distribution_id };
        self.collection.find_one(filter, None).await.map_err(Into::into)
    }

    /// 获取奖励分发统计
    pub async fn get_reward_stats(&self) -> AppResult<RewardStats> {
        // 统计总分发次数
        let total_distributions = self.collection.count_documents(doc! {}, None).await? as u64;

        // 统计今日分发次数
        let today_start = Utc::now()
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp();
        let today_distributions = self
            .collection
            .count_documents(doc! { "distributed_at": { "$gte": today_start } }, None)
            .await? as u64;

        // 统计锁定中的奖励
        let locked_rewards = self
            .collection
            .count_documents(doc! { "is_locked": true }, None)
            .await? as u64;

        // 统计奖励类型分布
        let reward_type_pipeline = vec![
            doc! {
                "$group": {
                    "_id": "$reward_type",
                    "count": { "$sum": 1 },
                    "total_amount": { "$sum": "$reward_amount" }
                }
            },
            doc! {
                "$sort": { "count": -1 }
            },
        ];

        let mut cursor = self.collection.aggregate(reward_type_pipeline, None).await?;

        let mut reward_type_distribution = Vec::new();
        while let Some(doc) = cursor.try_next().await? {
            if let (Some(reward_type), Some(count), Some(total_amount)) = (
                doc.get_i32("_id").ok(),
                doc.get_i32("count").ok(),
                doc.get_i64("total_amount").ok(),
            ) {
                reward_type_distribution.push((reward_type as u8, count as u64, total_amount as u64));
            }
        }

        Ok(RewardStats {
            total_distributions,
            today_distributions,
            locked_rewards,
            reward_type_distribution,
        })
    }
}

/// 池子事件统计
#[derive(Debug, Clone)]
pub struct PoolEventStats {
    pub total_pools: u64,
    pub today_new_pools: u64,
    pub fee_rate_distribution: Vec<(u32, u64)>, // (费率, 数量)
}

/// NFT领取统计
#[derive(Debug, Clone)]
pub struct NftClaimStats {
    pub total_claims: u64,
    pub today_claims: u64,
    pub tier_distribution: Vec<(u8, u64, u64)>, // (等级, 数量, 总金额)
}

/// 奖励分发统计
#[derive(Debug, Clone)]
pub struct RewardStats {
    pub total_distributions: u64,
    pub today_distributions: u64,
    pub locked_rewards: u64,
    pub reward_type_distribution: Vec<(u8, u64, u64)>, // (奖励类型, 数量, 总金额)
}

/// LaunchEvent仓库
#[derive(Debug, Clone)]
pub struct LaunchEventRepository {
    collection: Collection<LaunchEvent>,
}

impl LaunchEventRepository {
    pub fn new(collection: Collection<LaunchEvent>) -> Self {
        Self { collection }
    }

    /// 初始化索引
    pub async fn init_indexes(&self) -> AppResult<()> {
        // 唯一索引：签名（防止重复）
        let signature_index = IndexModel::builder()
            .keys(doc! { "signature": 1 })
            .options(IndexOptions::builder().unique(true).build())
            .build();

        // 用户钱包索引（支持用户历史查询）
        let user_wallet_index = IndexModel::builder().keys(doc! { "user_wallet": 1 }).build();

        // meme代币索引
        let meme_token_index = IndexModel::builder().keys(doc! { "meme_token_mint": 1 }).build();

        // 时间索引（支持时间范围查询）
        let launched_at_index = IndexModel::builder().keys(doc! { "launched_at": -1 }).build();

        // 迁移状态索引（支持状态过滤）
        let migration_status_index = IndexModel::builder().keys(doc! { "migration_status": 1 }).build();

        // 复合索引：状态+时间（优化迁移任务查询）
        let status_time_index = IndexModel::builder()
            .keys(doc! {
                "migration_status": 1,
                "launched_at": -1
            })
            .build();

        let indexes = vec![
            signature_index,
            user_wallet_index,
            meme_token_index,
            launched_at_index,
            migration_status_index,
            status_time_index,
        ];

        self.collection.create_indexes(indexes, None).await?;
        info!("✅ LaunchEvent数据库索引初始化完成");
        Ok(())
    }

    /// 插入Launch事件
    pub async fn insert_launch_event(&self, mut event: LaunchEvent) -> AppResult<String> {
        event.updated_at = Utc::now().timestamp();

        let result = self.collection.insert_one(event, None).await?;

        Ok(result.inserted_id.as_object_id().unwrap().to_hex())
    }

    /// 根据签名查找事件
    pub async fn find_by_signature(&self, signature: &str) -> AppResult<Option<LaunchEvent>> {
        let filter = doc! { "signature": signature };
        let result = self.collection.find_one(filter, None).await?;
        Ok(result)
    }

    /// 更新迁移状态
    pub async fn update_migration_status(
        &self,
        signature: &str,
        status: MigrationStatus,
        pool_address: Option<String>,
        error: Option<String>,
    ) -> AppResult<bool> {
        let status_str = match status {
            MigrationStatus::Pending => "pending",
            MigrationStatus::Success => "success",
            MigrationStatus::Failed => "failed",
            MigrationStatus::Retrying => "retrying",
        };

        let mut update_doc = doc! {
            "$set": {
                "migration_status": status_str,
                "updated_at": Utc::now().timestamp()
            }
        };

        // 如果是成功状态，设置池子地址和完成时间
        if matches!(status, MigrationStatus::Success) {
            if let Some(pool_addr) = pool_address {
                update_doc
                    .get_document_mut("$set")
                    .unwrap()
                    .insert("migrated_pool_address", pool_addr);
                update_doc
                    .get_document_mut("$set")
                    .unwrap()
                    .insert("migration_completed_at", Utc::now().timestamp());
            }
        }

        // 如果是失败状态，设置错误信息并递增重试次数
        if matches!(status, MigrationStatus::Failed) {
            if let Some(err) = error {
                update_doc
                    .get_document_mut("$set")
                    .unwrap()
                    .insert("migration_error", err);
            }
            update_doc.insert("$inc", doc! { "migration_retry_count": 1 });
        }

        let filter = doc! { "signature": signature };
        let result = self.collection.update_one(filter, update_doc, None).await?;

        Ok(result.modified_count > 0)
    }

    /// 查找待迁移的事件
    pub async fn find_pending_migrations(&self) -> AppResult<Vec<LaunchEvent>> {
        let filter = doc! { "migration_status": "pending" };
        let cursor = self.collection.find(filter, None).await?;

        let events: Vec<LaunchEvent> = cursor.try_collect().await?;

        Ok(events)
    }

    /// 查找需要重试的失败事件
    pub async fn find_failed_migrations_for_retry(&self) -> AppResult<Vec<LaunchEvent>> {
        let filter = doc! {
            "migration_status": "failed",
            "migration_retry_count": { "$lt": 3 } // 最多重试3次
        };
        let cursor = self.collection.find(filter, None).await?;

        let events: Vec<LaunchEvent> = cursor.try_collect().await?;

        Ok(events)
    }

    /// 统计总Launch数量
    pub async fn count_total_launches(&self) -> AppResult<u64> {
        let count = self.collection.count_documents(doc! {}, None).await?;
        Ok(count)
    }

    /// 获取迁移成功率
    pub async fn get_migration_success_rate(&self) -> AppResult<f64> {
        let total_count = self.collection.count_documents(doc! {}, None).await?;

        if total_count == 0 {
            return Ok(0.0);
        }

        let success_count = self
            .collection
            .count_documents(doc! { "migration_status": "success" }, None)
            .await?;

        let success_rate = (success_count as f64) / (total_count as f64) * 100.0;
        Ok(success_rate)
    }

    /// 获取待迁移事件数量
    pub async fn count_pending_migrations(&self) -> AppResult<u64> {
        let count = self
            .collection
            .count_documents(doc! { "migration_status": "pending" }, None)
            .await?;
        Ok(count)
    }

    /// 获取成功迁移事件数量
    pub async fn count_success_migrations(&self) -> AppResult<u64> {
        let count = self
            .collection
            .count_documents(doc! { "migration_status": "success" }, None)
            .await?;
        Ok(count)
    }

    /// 获取失败迁移事件数量
    pub async fn count_failed_migrations(&self) -> AppResult<u64> {
        let count = self
            .collection
            .count_documents(doc! { "migration_status": "failed" }, None)
            .await?;
        Ok(count)
    }

    /// 获取重试中迁移事件数量
    pub async fn count_retrying_migrations(&self) -> AppResult<u64> {
        let count = self
            .collection
            .count_documents(doc! { "migration_status": "retrying" }, None)
            .await?;
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event_model::PairType;
    use chrono::Utc;

    fn create_test_launch_event() -> LaunchEvent {
        LaunchEvent {
            id: None,
            meme_token_mint: "So11111111111111111111111111111111111111112".to_string(),
            base_token_mint: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(),
            user_wallet: "8S2bcP66WehuF6cHryfZ7vfFpQWaUhYyAYSy5U3gX4Fy".to_string(),
            config_index: 1,
            initial_price: 0.001,
            tick_lower_price: 0.0005,
            tick_upper_price: 0.002,
            meme_token_amount: 1000000,
            base_token_amount: 1000,
            max_slippage_percent: 1.0,
            with_metadata: true,
            open_time: 0,
            launched_at: Utc::now().timestamp(),
            migration_status: "pending".to_string(),
            migrated_pool_address: None,
            migration_completed_at: None,
            migration_error: None,
            migration_retry_count: 0,
            total_liquidity_usd: 1000.0,
            pair_type: "MemeToUsdc".to_string(),
            price_range_width_percent: 300.0,
            is_high_value_launch: true,
            signature: "test_signature_123".to_string(),
            slot: 12345,
            processed_at: Utc::now().timestamp(),
            updated_at: Utc::now().timestamp(),
        }
    }

    #[test]
    fn test_launch_event_creation() {
        let event = create_test_launch_event();
        assert_eq!(event.meme_token_mint, "So11111111111111111111111111111111111111112");
        assert_eq!(event.base_token_mint, "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
        assert_eq!(event.user_wallet, "8S2bcP66WehuF6cHryfZ7vfFpQWaUhYyAYSy5U3gX4Fy");
        assert_eq!(event.migration_status, "pending");
        assert_eq!(event.migration_retry_count, 0);
    }

    #[test]
    fn test_migration_status_enum() {
        let pending = MigrationStatus::Pending;
        let success = MigrationStatus::Success;
        let failed = MigrationStatus::Failed;
        let retrying = MigrationStatus::Retrying;

        // 测试序列化
        let pending_json = serde_json::to_string(&pending).unwrap();
        let success_json = serde_json::to_string(&success).unwrap();
        let failed_json = serde_json::to_string(&failed).unwrap();
        let retrying_json = serde_json::to_string(&retrying).unwrap();

        assert_eq!(pending_json, "\"Pending\"");
        assert_eq!(success_json, "\"Success\"");
        assert_eq!(failed_json, "\"Failed\"");
        assert_eq!(retrying_json, "\"Retrying\"");

        // 测试反序列化
        let pending_from_json: MigrationStatus = serde_json::from_str(&pending_json).unwrap();
        let success_from_json: MigrationStatus = serde_json::from_str(&success_json).unwrap();
        let failed_from_json: MigrationStatus = serde_json::from_str(&failed_json).unwrap();
        let retrying_from_json: MigrationStatus = serde_json::from_str(&retrying_json).unwrap();

        assert!(matches!(pending_from_json, MigrationStatus::Pending));
        assert!(matches!(success_from_json, MigrationStatus::Success));
        assert!(matches!(failed_from_json, MigrationStatus::Failed));
        assert!(matches!(retrying_from_json, MigrationStatus::Retrying));
    }

    #[test]
    fn test_pair_type_enum() {
        let pair_types = vec![
            PairType::MemeToSol,
            PairType::MemeToUsdc,
            PairType::MemeToUsdt,
            PairType::MemeToOther,
        ];

        for pair_type in pair_types {
            let json = serde_json::to_string(&pair_type).unwrap();
            let from_json: PairType = serde_json::from_str(&json).unwrap();

            match pair_type {
                PairType::MemeToSol => assert!(matches!(from_json, PairType::MemeToSol)),
                PairType::MemeToUsdc => assert!(matches!(from_json, PairType::MemeToUsdc)),
                PairType::MemeToUsdt => assert!(matches!(from_json, PairType::MemeToUsdt)),
                PairType::MemeToOther => assert!(matches!(from_json, PairType::MemeToOther)),
            }
        }
    }

    #[test]
    fn test_launch_event_serialization() {
        let event = create_test_launch_event();

        // 测试序列化
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("So11111111111111111111111111111111111111112"));
        assert!(json.contains("test_signature_123"));
        assert!(json.contains("pending"));

        // 测试反序列化
        let from_json: LaunchEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(from_json.meme_token_mint, event.meme_token_mint);
        assert_eq!(from_json.signature, event.signature);
        assert_eq!(from_json.migration_status, event.migration_status);
    }
}

/// 存款事件仓库
#[derive(Debug, Clone)]
pub struct DepositEventRepository {
    collection: Collection<DepositEvent>,
}

impl DepositEventRepository {
    pub fn new(collection: Collection<DepositEvent>) -> Self {
        Self { collection }
    }

    /// 初始化数据库索引
    pub async fn init_indexes(&self) -> AppResult<()> {
        // 1. 唯一索引：防止重复处理
        let signature_index = IndexModel::builder()
            .keys(doc! { "signature": 1 })
            .options(IndexOptions::builder().unique(true).build())
            .build();

        // 2. 用户历史查询优化
        let user_deposited_at_index = IndexModel::builder()
            .keys(doc! {
                "user": 1,
                "deposited_at": -1
            })
            .build();

        // 3. 代币查询优化
        let token_deposited_at_index = IndexModel::builder()
            .keys(doc! {
                "token_mint": 1,
                "deposited_at": -1
            })
            .build();

        // 4. 项目配置查询优化
        let project_deposited_at_index = IndexModel::builder()
            .keys(doc! {
                "project_config": 1,
                "deposited_at": -1
            })
            .build();

        // 5. 时间范围查询
        let deposited_at_index = IndexModel::builder().keys(doc! { "deposited_at": -1 }).build();

        // 6-7. 金额范围查询
        let amount_index = IndexModel::builder().keys(doc! { "amount": 1 }).build();

        let total_raised_index = IndexModel::builder().keys(doc! { "total_raised": 1 }).build();

        // 8-11. 业务查询索引
        let deposit_type_index = IndexModel::builder().keys(doc! { "deposit_type": 1 }).build();

        let high_value_index = IndexModel::builder().keys(doc! { "is_high_value_deposit": 1 }).build();

        let related_pool_index = IndexModel::builder().keys(doc! { "related_pool": 1 }).build();

        // 12-13. 复合查询索引
        let token_type_deposited_index = IndexModel::builder()
            .keys(doc! {
                "token_mint": 1,
                "deposit_type": 1,
                "deposited_at": -1
            })
            .build();

        let project_user_deposited_index = IndexModel::builder()
            .keys(doc! {
                "project_config": 1,
                "user": 1,
                "deposited_at": -1
            })
            .build();

        let indexes = vec![
            signature_index,
            user_deposited_at_index,
            token_deposited_at_index,
            project_deposited_at_index,
            deposited_at_index,
            amount_index,
            total_raised_index,
            deposit_type_index,
            high_value_index,
            related_pool_index,
            token_type_deposited_index,
            project_user_deposited_index,
        ];

        self.collection.create_indexes(indexes, None).await?;
        info!("✅ DepositEvent数据库索引初始化完成");
        Ok(())
    }

    /// 插入存款事件
    pub async fn insert_deposit_event(&self, mut event: DepositEvent) -> AppResult<String> {
        event.updated_at = Utc::now().timestamp();

        let result = self.collection.insert_one(event, None).await?;

        Ok(result.inserted_id.as_object_id().unwrap().to_hex())
    }

    /// 查找所有记录（用于调试）
    pub async fn find_all(&self) -> AppResult<Vec<DepositEvent>> {
        let cursor = self.collection.find(doc! {}, None).await?;
        let items: Vec<DepositEvent> = cursor.try_collect().await?;
        Ok(items)
    }

    /// 分页查询（支持多种过滤条件）
    pub async fn find_paginated(
        &self,
        filter: mongodb::bson::Document,
        options: mongodb::options::FindOptions,
    ) -> AppResult<PaginatedResult<DepositEvent>> {
        // 查询总数
        let total = self.collection.count_documents(filter.clone(), None).await?;

        // 执行分页查询
        let cursor = self.collection.find(filter, options).await?;
        let items: Vec<DepositEvent> = cursor.try_collect().await?;

        Ok(PaginatedResult { items, total })
    }

    /// 统计查询
    pub async fn get_deposit_stats(&self) -> AppResult<DepositStats> {
        // 统计总存款数
        let total_deposits = self.collection.count_documents(doc! {}, None).await?;

        // 统计今日存款数
        let today_start = Utc::now()
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp();
        let today_deposits = self
            .collection
            .count_documents(doc! { "deposited_at": { "$gte": today_start } }, None)
            .await?;

        // 统计独特用户数
        let unique_users_pipeline = vec![doc! { "$group": { "_id": "$user", "count": { "$sum": 1 } } }];
        let mut unique_users_cursor = self.collection.aggregate(unique_users_pipeline, None).await?;
        let mut unique_users = 0u64;
        while let Some(_doc) = unique_users_cursor.try_next().await? {
            unique_users += 1;
        }

        // 统计独特代币数
        let unique_tokens_pipeline = vec![doc! { "$group": { "_id": "$token_mint", "count": { "$sum": 1 } } }];
        let mut unique_tokens_cursor = self.collection.aggregate(unique_tokens_pipeline, None).await?;
        let mut unique_tokens = 0u64;
        while let Some(_doc) = unique_tokens_cursor.try_next().await? {
            unique_tokens += 1;
        }

        // 统计总美元交易量
        let total_volume_pipeline =
            vec![doc! { "$group": { "_id": null, "total": { "$sum": "$estimated_usd_value" } } }];
        let mut volume_cursor = self.collection.aggregate(total_volume_pipeline, None).await?;
        let total_volume_usd = if let Some(doc) = volume_cursor.try_next().await? {
            doc.get_f64("total").unwrap_or(0.0)
        } else {
            0.0
        };

        // 统计今日美元交易量
        let today_volume_pipeline = vec![
            doc! { "$match": { "deposited_at": { "$gte": today_start } } },
            doc! { "$group": { "_id": null, "total": { "$sum": "$estimated_usd_value" } } },
        ];
        let mut today_volume_cursor = self.collection.aggregate(today_volume_pipeline, None).await?;
        let today_volume_usd = if let Some(doc) = today_volume_cursor.try_next().await? {
            doc.get_f64("total").unwrap_or(0.0)
        } else {
            0.0
        };

        // 统计存款类型分布
        let deposit_type_pipeline = vec![
            doc! {
                "$group": {
                    "_id": "$deposit_type",
                    "count": { "$sum": 1 },
                    "name": { "$first": "$deposit_type_name" }
                }
            },
            doc! { "$sort": { "_id": 1 } },
        ];
        let mut type_cursor = self.collection.aggregate(deposit_type_pipeline, None).await?;
        let mut deposit_type_distribution = Vec::new();
        while let Some(doc) = type_cursor.try_next().await? {
            if let (Some(deposit_type), Some(count), Some(name)) = (
                doc.get_i32("_id").ok(),
                doc.get_i64("count").ok(),
                doc.get_str("name").ok(),
            ) {
                deposit_type_distribution.push(DepositTypeDistribution {
                    deposit_type: deposit_type as u8,
                    name: name.to_string(),
                    count: count as u64,
                });
            }
        }

        // 统计代币分布（前10）
        let token_distribution_pipeline = vec![
            doc! {
                "$group": {
                    "_id": "$token_mint",
                    "count": { "$sum": 1 },
                    "total_amount": { "$sum": "$estimated_usd_value" },
                    "symbol": { "$first": "$token_symbol" },
                    "name": { "$first": "$token_name" }
                }
            },
            doc! { "$sort": { "count": -1 } },
            doc! { "$limit": 10 },
        ];
        let mut token_cursor = self.collection.aggregate(token_distribution_pipeline, None).await?;
        let mut token_distribution = Vec::new();
        while let Some(doc) = token_cursor.try_next().await? {
            if let (Some(mint), Some(count), Some(total_amount)) = (
                doc.get_str("_id").ok(),
                doc.get_i64("count").ok(),
                doc.get_f64("total_amount").ok(),
            ) {
                token_distribution.push(TokenDistribution {
                    token_mint: mint.to_string(),
                    token_symbol: doc.get_str("symbol").ok().map(|s| s.to_string()),
                    token_name: doc.get_str("name").ok().map(|s| s.to_string()),
                    count: count as u64,
                    total_volume_usd: total_amount,
                });
            }
        }

        Ok(DepositStats {
            total_deposits,
            today_deposits,
            unique_users,
            unique_tokens,
            total_volume_usd,
            today_volume_usd,
            deposit_type_distribution,
            token_distribution,
        })
    }

    /// 按代币统计独立用户数（distinct user by token_mint）
    pub async fn count_unique_users_by_token(&self, token_mint: &str) -> AppResult<u64> {
        // 使用聚合实现去重计数，避免 distinct 拉取全部结果到内存
        let pipeline = vec![
            doc! { "$match": { "token_mint": token_mint } },
            doc! { "$group": { "_id": "$user" } },
            doc! { "$count": "count" },
        ];

        let mut cursor = self.collection.aggregate(pipeline, None).await?;
        if let Some(doc) = cursor.try_next().await? {
            if let Ok(v) = doc.get_i64("count") {
                return Ok(v as u64);
            }
            if let Ok(v) = doc.get_i32("count") {
                return Ok(v as u64);
            }
        }
        Ok(0)
    }

    /// 根据签名查询
    pub async fn find_by_signature(&self, signature: &str) -> AppResult<Option<DepositEvent>> {
        let filter = doc! { "signature": signature };
        self.collection.find_one(filter, None).await.map_err(Into::into)
    }

    /// 检查事件是否存在（防重复）
    pub async fn exists_by_signature(&self, signature: &str) -> AppResult<bool> {
        let filter = doc! { "signature": signature };
        let count = self.collection.count_documents(filter, None).await?;
        Ok(count > 0)
    }

    /// 根据用户钱包地址查询参与过的代币列表（去重）
    /// 用于支持按participate参数过滤代币查询
    pub async fn find_participated_tokens_by_user(&self, user_wallet: &str) -> AppResult<Vec<String>> {
        // 使用MongoDB聚合管道实现高效去重
        let pipeline = vec![
            // 1. 匹配指定用户的存款记录
            doc! {
                "$match": {
                    "user": user_wallet
                }
            },
            // 2. 按token_mint分组，去重
            doc! {
                "$group": {
                    "_id": "$token_mint"
                }
            },
            // 3. 重新整理输出格式
            doc! {
                "$project": {
                    "_id": 0,
                    "token_mint": "$_id"
                }
            },
            // 4. 按token_mint排序，确保结果稳定
            doc! {
                "$sort": {
                    "token_mint": 1
                }
            }
        ];

        let mut cursor = self.collection.aggregate(pipeline, None).await?;
        let mut token_mints = Vec::new();

        while let Some(doc) = cursor.try_next().await? {
            if let Ok(token_mint) = doc.get_str("token_mint") {
                token_mints.push(token_mint.to_string());
            }
        }

        Ok(token_mints)
    }
}

/// 分页结果
#[derive(Debug, Clone)]
pub struct PaginatedResult<T> {
    pub items: Vec<T>,
    pub total: u64,
}

/// 存款统计
#[derive(Debug, Clone)]
pub struct DepositStats {
    pub total_deposits: u64,
    pub today_deposits: u64,
    pub unique_users: u64,
    pub unique_tokens: u64,
    pub total_volume_usd: f64,
    pub today_volume_usd: f64,
    pub deposit_type_distribution: Vec<DepositTypeDistribution>,
    pub token_distribution: Vec<TokenDistribution>,
}

/// 存款类型分布
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DepositTypeDistribution {
    pub deposit_type: u8,
    pub name: String,
    pub count: u64,
}

/// 代币分布
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TokenDistribution {
    pub token_mint: String,
    pub token_symbol: Option<String>,
    pub token_name: Option<String>,
    pub count: u64,
    pub total_volume_usd: f64,
}

#[cfg(test)]
mod deposit_tests {
    use super::*;
    use crate::event_model::DepositEvent;
    use chrono::Utc;

    /// 创建测试用的存款事件
    fn create_test_deposit_event(signature: &str) -> DepositEvent {
        DepositEvent {
            id: None,
            // 核心业务字段
            user: "8S2bcP66WehuF6cHryfZ7vfFpQWaUhYyAYSy5U3gX4Fy".to_string(),
            token_mint: "So11111111111111111111111111111111111111112".to_string(),
            amount: 1000000, // 1 SOL
            project_config: "test_project_config".to_string(),
            total_raised: 5000000, // 5 SOL 总筹资
            deposited_at: Utc::now().timestamp(),

            // 代币元数据
            token_symbol: Some("SOL".to_string()),
            token_name: Some("Solana".to_string()),
            token_decimals: Some(9),
            token_logo_uri: Some("https://example.com/sol.png".to_string()),

            // 业务扩展字段
            deposit_type: 1,
            deposit_type_name: "初始存款".to_string(),
            is_high_value_deposit: false,
            related_pool: Some("test_pool_address".to_string()),
            estimated_usd_value: 100.0, // $100
            actual_amount: 1.0,         // 1.0 SOL (1000000 / 10^9)
            actual_total_raised: 5.0,   // 5.0 SOL

            // 区块链标准字段
            signature: signature.to_string(),
            slot: 12345,
            processed_at: Utc::now().timestamp(),
            updated_at: Utc::now().timestamp(),
        }
    }

    #[test]
    fn test_deposit_event_creation() {
        let event = create_test_deposit_event("test_signature_123");

        // 验证核心字段
        assert_eq!(event.user, "8S2bcP66WehuF6cHryfZ7vfFpQWaUhYyAYSy5U3gX4Fy");
        assert_eq!(event.token_mint, "So11111111111111111111111111111111111111112");
        assert_eq!(event.amount, 1000000);
        assert_eq!(event.deposit_type, 1);
        assert_eq!(event.signature, "test_signature_123");

        // 验证元数据字段
        assert_eq!(event.token_symbol, Some("SOL".to_string()));
        assert_eq!(event.token_name, Some("Solana".to_string()));
        assert_eq!(event.token_decimals, Some(9));

        // 验证业务字段
        assert!(!event.is_high_value_deposit);
        assert_eq!(event.estimated_usd_value, 100.0);
    }

    #[test]
    fn test_deposit_type_distribution_serialization() {
        let distribution = DepositTypeDistribution {
            deposit_type: 1,
            name: "初始存款".to_string(),
            count: 10,
        };

        // 测试序列化
        let json = serde_json::to_string(&distribution).unwrap();
        assert!(json.contains("\"deposit_type\":1"));
        assert!(json.contains("\"name\":\"初始存款\""));
        assert!(json.contains("\"count\":10"));

        // 测试反序列化
        let from_json: DepositTypeDistribution = serde_json::from_str(&json).unwrap();
        assert_eq!(from_json.deposit_type, 1);
        assert_eq!(from_json.name, "初始存款");
        assert_eq!(from_json.count, 10);
    }

    #[test]
    fn test_token_distribution_serialization() {
        let distribution = TokenDistribution {
            token_mint: "So11111111111111111111111111111111111111112".to_string(),
            token_symbol: Some("SOL".to_string()),
            token_name: Some("Solana".to_string()),
            count: 5,
            total_volume_usd: 500.0,
        };

        // 测试序列化
        let json = serde_json::to_string(&distribution).unwrap();
        assert!(json.contains("So11111111111111111111111111111111111111112"));
        assert!(json.contains("\"token_symbol\":\"SOL\""));
        assert!(json.contains("\"total_volume_usd\":500.0"));

        // 测试反序列化
        let from_json: TokenDistribution = serde_json::from_str(&json).unwrap();
        assert_eq!(from_json.token_mint, "So11111111111111111111111111111111111111112");
        assert_eq!(from_json.token_symbol, Some("SOL".to_string()));
        assert_eq!(from_json.total_volume_usd, 500.0);
    }

    #[test]
    fn test_deposit_stats_creation() {
        let stats = DepositStats {
            total_deposits: 100,
            today_deposits: 10,
            unique_users: 50,
            unique_tokens: 20,
            total_volume_usd: 10000.0,
            today_volume_usd: 1000.0,
            deposit_type_distribution: vec![
                DepositTypeDistribution {
                    deposit_type: 1,
                    name: "初始存款".to_string(),
                    count: 60,
                },
                DepositTypeDistribution {
                    deposit_type: 2,
                    name: "追加存款".to_string(),
                    count: 40,
                },
            ],
            token_distribution: vec![TokenDistribution {
                token_mint: "So11111111111111111111111111111111111111112".to_string(),
                token_symbol: Some("SOL".to_string()),
                token_name: Some("Solana".to_string()),
                count: 70,
                total_volume_usd: 7000.0,
            }],
        };

        // 验证基础统计
        assert_eq!(stats.total_deposits, 100);
        assert_eq!(stats.today_deposits, 10);
        assert_eq!(stats.unique_users, 50);
        assert_eq!(stats.unique_tokens, 20);
        assert_eq!(stats.total_volume_usd, 10000.0);
        assert_eq!(stats.today_volume_usd, 1000.0);

        // 验证存款类型分布
        assert_eq!(stats.deposit_type_distribution.len(), 2);
        assert_eq!(stats.deposit_type_distribution[0].deposit_type, 1);
        assert_eq!(stats.deposit_type_distribution[0].name, "初始存款");
        assert_eq!(stats.deposit_type_distribution[0].count, 60);

        // 验证代币分布
        assert_eq!(stats.token_distribution.len(), 1);
        assert_eq!(
            stats.token_distribution[0].token_mint,
            "So11111111111111111111111111111111111111112"
        );
        assert_eq!(stats.token_distribution[0].count, 70);
        assert_eq!(stats.token_distribution[0].total_volume_usd, 7000.0);
    }

    #[test]
    fn test_paginated_result_creation() {
        let events = vec![
            create_test_deposit_event("sig1"),
            create_test_deposit_event("sig2"),
            create_test_deposit_event("sig3"),
        ];

        let result = PaginatedResult {
            items: events,
            total: 3,
        };

        assert_eq!(result.items.len(), 3);
        assert_eq!(result.total, 3);
        assert_eq!(result.items[0].signature, "sig1");
        assert_eq!(result.items[1].signature, "sig2");
        assert_eq!(result.items[2].signature, "sig3");
    }
}
