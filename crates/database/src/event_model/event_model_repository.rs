use super::{
    repository::{
        ClmmPoolEventRepository, DepositEventRepository, LaunchEventRepository, NftClaimEventRepository,
        RewardDistributionEventRepository,
    },
    ClmmPoolEvent, DepositEvent, LaunchEvent, NftClaimEvent, RewardDistributionEvent, TokenCreationEvent,
};
use mongodb::{
    bson::doc,
    options::{ClientOptions, FindOptions},
    Client, Database,
};
use std::{collections::HashSet, sync::Arc};
use tracing::{error, info};
use utils::AppResult;

/// 统一的事件模型仓库
///
/// 提供对所有事件模型的统一访问接口，支持回填服务的需求
#[derive(Debug, Clone)]
pub struct EventModelRepository {
    database: Arc<Database>,
    pub launch_event_repo: LaunchEventRepository,
    pub clmm_pool_event_repo: ClmmPoolEventRepository,
    pub nft_claim_event_repo: NftClaimEventRepository,
    pub reward_distribution_event_repo: RewardDistributionEventRepository,
    pub deposit_event_repo: DepositEventRepository,
}

impl EventModelRepository {
    /// 创建新的事件模型仓库
    pub async fn new(mongo_uri: &str, database_name: &str) -> AppResult<Self> {
        // 连接MongoDB
        let client_options = ClientOptions::parse(mongo_uri).await?;
        let client = Client::with_options(client_options)?;
        let database = Arc::new(client.database(database_name));

        // 创建各个子仓库
        let launch_event_repo = LaunchEventRepository::new(database.collection("LaunchEvent"));
        let clmm_pool_event_repo = ClmmPoolEventRepository::new(database.collection("ClmmPoolEvent"));
        let nft_claim_event_repo = NftClaimEventRepository::new(database.collection("NftClaimEvent"));
        let reward_distribution_event_repo =
            RewardDistributionEventRepository::new(database.collection("RewardDistributionEvent"));
        let deposit_event_repo = DepositEventRepository::new(database.collection("DepositEvent"));

        Ok(Self {
            database,
            launch_event_repo,
            clmm_pool_event_repo,
            nft_claim_event_repo,
            reward_distribution_event_repo,
            deposit_event_repo,
        })
    }

    /// 获取最新的LaunchEvent签名 (用于回填服务)
    pub async fn get_latest_launch_event(&self) -> AppResult<Option<LaunchEvent>> {
        let options = FindOptions::builder().sort(doc! { "slot": -1, "signature": -1 }).limit(1).build();

        let mut cursor = self
            .database
            .collection::<LaunchEvent>("LaunchEvent")
            .find(doc! {}, options)
            .await?;

        if cursor.advance().await? {
            let event = cursor.deserialize_current()?;
            Ok(Some(event))
        } else {
            Ok(None)
        }
    }

    /// 获取最老的LaunchEvent签名 (用于回填服务)
    pub async fn get_oldest_launch_event(&self) -> AppResult<Option<LaunchEvent>> {
        let options = FindOptions::builder().sort(doc! { "slot": 1, "signature": 1 }).limit(1).build();

        let mut cursor = self
            .database
            .collection::<LaunchEvent>("LaunchEvent")
            .find(doc! {}, options)
            .await?;

        if cursor.advance().await? {
            let event = cursor.deserialize_current()?;
            Ok(Some(event))
        } else {
            Ok(None)
        }
    }

    /// 获取最新的TokenCreationEvent签名 (用于回填服务)
    pub async fn get_latest_token_creation_event(&self) -> AppResult<Option<TokenCreationEvent>> {
        
        let options = FindOptions::builder().sort(doc! { "slot": -1, "signature": -1 }).limit(1).build();

        let mut cursor = self
            .database
            .collection::<TokenCreationEvent>("TokenCreationEvent")
            .find(doc! {}, options)
            .await?;

        if cursor.advance().await? {
            let event = cursor.deserialize_current()?;
            Ok(Some(event))
        } else {
            Ok(None)
        }
    }

    /// 获取最老的TokenCreationEvent签名 (用于回填服务)
    pub async fn get_oldest_token_creation_event(&self) -> AppResult<Option<TokenCreationEvent>> {
        
        let options = FindOptions::builder().sort(doc! { "slot": 1, "signature": 1 }).limit(1).build();

        let mut cursor = self
            .database
            .collection::<TokenCreationEvent>("TokenCreationEvent")
            .find(doc! {}, options)
            .await?;

        if cursor.advance().await? {
            let event = cursor.deserialize_current()?;
            Ok(Some(event))
        } else {
            Ok(None)
        }
    }

    /// 获取最老的DepositEvent签名 (用于回填服务)
    pub async fn get_oldest_deposit_event(&self) -> AppResult<Option<DepositEvent>> {
        let options = FindOptions::builder().sort(doc! { "slot": 1, "signature": 1 }).limit(1).build();

        let mut cursor = self
            .database
            .collection::<DepositEvent>("DepositEvent")
            .find(doc! {}, options)
            .await?;

        if cursor.advance().await? {
            let event = cursor.deserialize_current()?;
            Ok(Some(event))
        } else {
            Ok(None)
        }
    }

    /// 获取最老的NftClaimEvent签名 (用于回填服务)
    pub async fn get_oldest_nft_claim_event(&self) -> AppResult<Option<NftClaimEvent>> {
        let options = FindOptions::builder().sort(doc! { "slot": 1, "signature": 1 }).limit(1).build();

        let mut cursor = self
            .database
            .collection::<NftClaimEvent>("NftClaimEvent")
            .find(doc! {}, options)
            .await?;

        if cursor.advance().await? {
            let event = cursor.deserialize_current()?;
            Ok(Some(event))
        } else {
            Ok(None)
        }
    }

    /// 获取最老的ClmmPoolEvent签名 (用于回填服务)
    pub async fn get_oldest_clmm_pool_event(&self) -> AppResult<Option<ClmmPoolEvent>> {
        let options = FindOptions::builder().sort(doc! { "slot": 1, "signature": 1 }).limit(1).build();

        let mut cursor = self
            .database
            .collection::<ClmmPoolEvent>("ClmmPoolEvent")
            .find(doc! {}, options)
            .await?;

        if cursor.advance().await? {
            let event = cursor.deserialize_current()?;
            Ok(Some(event))
        } else {
            Ok(None)
        }
    }

    /// 获取最老的ReferralRewardEvent签名 (用于回填服务)
    /// 推荐奖励事件是 RewardDistributionEvent 中 is_referral_reward=true 的记录
    pub async fn get_oldest_referral_reward_event(&self) -> AppResult<Option<RewardDistributionEvent>> {
        let filter = doc! { "is_referral_reward": true };
        let options = FindOptions::builder().sort(doc! { "slot": 1, "signature": 1 }).limit(1).build();

        let mut cursor = self
            .database
            .collection::<RewardDistributionEvent>("RewardDistributionEvent")
            .find(filter, options)
            .await?;

        if cursor.advance().await? {
            let event = cursor.deserialize_current()?;
            Ok(Some(event))
        } else {
            Ok(None)
        }
    }

    /// 查询数据库中已存在的签名集合 (用于回填服务去重)
    pub async fn get_existing_signatures(&self, signatures: &[String]) -> AppResult<Vec<String>> {
        if signatures.is_empty() {
            return Ok(Vec::new());
        }

        let mut existing_signatures = Vec::new();

        // 分批查询，避免单次查询条件过大
        const BATCH_SIZE: usize = 100;
        for chunk in signatures.chunks(BATCH_SIZE) {
            let filter = doc! {
                "signature": {
                    "$in": chunk
                }
            };

            let mut all_found = Vec::new();

            // 检查LaunchEvent集合
            if let Ok(mut cursor) = self
                .database
                .collection::<LaunchEvent>("LaunchEvent")
                .find(filter.clone(), None)
                .await
            {
                while cursor.advance().await? {
                    let event = cursor.deserialize_current()?;
                    all_found.push(event.signature);
                }
            }

            // 检查ClmmPoolEvent集合
            if let Ok(mut cursor) = self
                .database
                .collection::<ClmmPoolEvent>("ClmmPoolEvent")
                .find(filter.clone(), None)
                .await
            {
                while cursor.advance().await? {
                    let event = cursor.deserialize_current()?;
                    all_found.push(event.signature);
                }
            }

            // 检查NftClaimEvent集合
            if let Ok(mut cursor) = self
                .database
                .collection::<NftClaimEvent>("NftClaimEvent")
                .find(filter.clone(), None)
                .await
            {
                while cursor.advance().await? {
                    let event = cursor.deserialize_current()?;
                    all_found.push(event.signature);
                }
            }

            // 检查RewardDistributionEvent集合
            if let Ok(mut cursor) = self
                .database
                .collection::<RewardDistributionEvent>("RewardDistributionEvent")
                .find(filter.clone(), None)
                .await
            {
                while cursor.advance().await? {
                    let event = cursor.deserialize_current()?;
                    all_found.push(event.signature);
                }
            }

            // 检查DepositEvent集合
            if let Ok(mut cursor) = self
                .database
                .collection::<DepositEvent>("DepositEvent")
                .find(filter.clone(), None)
                .await
            {
                while cursor.advance().await? {
                    let event = cursor.deserialize_current()?;
                    all_found.push(event.signature);
                }
            }

            existing_signatures.extend(all_found);
        }

        // 去重并返回
        let unique_signatures: HashSet<String> = existing_signatures.into_iter().collect();
        Ok(unique_signatures.into_iter().collect())
    }

    /// 检查单个签名是否存在于任何事件集合中
    pub async fn signature_exists(&self, signature: &str) -> AppResult<bool> {
        let filter = doc! { "signature": signature };

        // 依次检查每个集合
        let collections = [
            "LaunchEvent",
            "ClmmPoolEvent",
            "NftClaimEvent",
            "RewardDistributionEvent",
            "DepositEvent",
        ];

        for collection_name in &collections {
            let count = self
                .database
                .collection::<mongodb::bson::Document>(collection_name)
                .count_documents(filter.clone(), None)
                .await?;

            if count > 0 {
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// 获取数据库连接 (供持久化服务使用)
    pub fn get_database(&self) -> Arc<Database> {
        Arc::clone(&self.database)
    }

    /// 获取所有事件集合的签名统计
    pub async fn get_signature_statistics(&self) -> AppResult<SignatureStatistics> {
        let mut stats = SignatureStatistics::default();

        // LaunchEvent统计
        if let Ok(count) = self
            .database
            .collection::<LaunchEvent>("LaunchEvent")
            .count_documents(doc! {}, None)
            .await
        {
            stats.launch_events = count;
        }

        // ClmmPoolEvent统计
        if let Ok(count) = self
            .database
            .collection::<ClmmPoolEvent>("ClmmPoolEvent")
            .count_documents(doc! {}, None)
            .await
        {
            stats.clmm_pool_events = count;
        }

        // NftClaimEvent统计
        if let Ok(count) = self
            .database
            .collection::<NftClaimEvent>("NftClaimEvent")
            .count_documents(doc! {}, None)
            .await
        {
            stats.nft_claim_events = count;
        }

        // RewardDistributionEvent统计
        if let Ok(count) = self
            .database
            .collection::<RewardDistributionEvent>("RewardDistributionEvent")
            .count_documents(doc! {}, None)
            .await
        {
            stats.reward_distribution_events = count;
        }

        // DepositEvent统计
        if let Ok(count) = self
            .database
            .collection::<DepositEvent>("DepositEvent")
            .count_documents(doc! {}, None)
            .await
        {
            stats.deposit_events = count;
        }

        stats.total_events = stats.launch_events
            + stats.clmm_pool_events
            + stats.nft_claim_events
            + stats.reward_distribution_events
            + stats.deposit_events;

        Ok(stats)
    }

    /// 健康检查
    pub async fn is_healthy(&self) -> bool {
        match self.database.list_collection_names(None).await {
            Ok(_) => true,
            Err(e) => {
                error!("❌ EventModelRepository 健康检查失败: {}", e);
                false
            }
        }
    }

    /// 测试数据库连接
    pub async fn test_connection(&self) -> AppResult<()> {
        let _ = self.database.list_collection_names(None).await?;
        info!("✅ EventModelRepository 数据库连接测试通过");
        Ok(())
    }
}

/// 签名统计信息
#[derive(Debug, Clone, Default)]
pub struct SignatureStatistics {
    pub total_events: u64,
    pub launch_events: u64,
    pub clmm_pool_events: u64,
    pub nft_claim_events: u64,
    pub reward_distribution_events: u64,
    pub deposit_events: u64,
}

impl SignatureStatistics {
    /// 获取事件分布百分比
    pub fn get_distribution_percentages(&self) -> EventDistributionPercentages {
        if self.total_events == 0 {
            return EventDistributionPercentages::default();
        }

        let total = self.total_events as f64;

        EventDistributionPercentages {
            launch_events_pct: (self.launch_events as f64 / total) * 100.0,
            clmm_pool_events_pct: (self.clmm_pool_events as f64 / total) * 100.0,
            nft_claim_events_pct: (self.nft_claim_events as f64 / total) * 100.0,
            reward_distribution_events_pct: (self.reward_distribution_events as f64 / total) * 100.0,
            deposit_events_pct: (self.deposit_events as f64 / total) * 100.0,
        }
    }
}

/// 事件分布百分比
#[derive(Debug, Clone, Default)]
pub struct EventDistributionPercentages {
    pub launch_events_pct: f64,
    pub clmm_pool_events_pct: f64,
    pub nft_claim_events_pct: f64,
    pub reward_distribution_events_pct: f64,
    pub deposit_events_pct: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signature_statistics_creation() {
        let mut stats = SignatureStatistics::default();
        assert_eq!(stats.total_events, 0);
        assert_eq!(stats.launch_events, 0);

        stats.launch_events = 10;
        stats.clmm_pool_events = 5;
        stats.nft_claim_events = 3;
        stats.reward_distribution_events = 2;
        stats.deposit_events = 1;
        stats.total_events = 21;

        let percentages = stats.get_distribution_percentages();
        assert!((percentages.launch_events_pct - 47.619047619047619).abs() < 0.001);
        assert!((percentages.clmm_pool_events_pct - 23.809523809523807).abs() < 0.001);
    }

    #[test]
    fn test_empty_statistics() {
        let stats = SignatureStatistics::default();
        let percentages = stats.get_distribution_percentages();

        assert_eq!(percentages.launch_events_pct, 0.0);
        assert_eq!(percentages.clmm_pool_events_pct, 0.0);
        assert_eq!(percentages.nft_claim_events_pct, 0.0);
        assert_eq!(percentages.reward_distribution_events_pct, 0.0);
        assert_eq!(percentages.deposit_events_pct, 0.0);
    }

    #[tokio::test]
    #[ignore] // 需要MongoDB连接
    async fn test_repository_creation() {
        // 这个测试需要真实的MongoDB连接
        // 在实际测试中，应该使用测试数据库
        let mongo_uri = "mongodb://localhost:27017";
        let database_name = "test_backfill_db";

        match EventModelRepository::new(mongo_uri, database_name).await {
            Ok(repo) => {
                assert!(repo.is_healthy().await);
                println!("✅ Repository创建成功");
            }
            Err(e) => {
                println!("⚠️ Repository创建失败（可能没有MongoDB）: {}", e);
            }
        }
    }
}
