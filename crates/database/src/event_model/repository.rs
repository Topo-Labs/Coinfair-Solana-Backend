use crate::event_model::{ClmmPoolEvent, NftClaimEvent, RewardDistributionEvent};
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
