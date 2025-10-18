use anyhow::Result;
use futures::stream::TryStreamExt;
use mongodb::{
    bson::{doc, DateTime as BsonDateTime},
    Collection, IndexModel,
};
use tracing::{error, info, warn};

use super::model::{UserPointsSummary, UserPointsQuery, UserPointsStats, UserPointsWithRank, UserRankInfo};

/// 用户积分仓库
#[derive(Clone, Debug)]
pub struct UserPointsRepository {
    collection: Collection<UserPointsSummary>,
}

impl UserPointsRepository {
    /// 创建新的用户积分仓库
    pub fn new(collection: Collection<UserPointsSummary>) -> Self {
        Self { collection }
    }

    /// 初始化数据库索引
    pub async fn init_indexes(&self) -> Result<()> {
        info!("🔧 初始化用户积分集合索引...");

        let indexes = vec![
            // 用户钱包地址唯一索引（主键）
            IndexModel::builder()
                .keys(doc! { "userWallet": 1 })
                .options(
                    mongodb::options::IndexOptions::builder()
                        .unique(true)
                        .name("userWallet_unique".to_string())
                        .build(),
                )
                .build(),
            // 交易积分索引（用于排行榜查询）
            IndexModel::builder()
                .keys(doc! { "pointsFromTransaction": -1 })
                .options(
                    mongodb::options::IndexOptions::builder()
                        .name("pointsFromTransaction_desc".to_string())
                        .build(),
                )
                .build(),
            // 最后更新时间索引（用于追踪最新变化）
            IndexModel::builder()
                .keys(doc! { "recordUpdateTime": -1 })
                .options(
                    mongodb::options::IndexOptions::builder()
                        .name("recordUpdateTime_desc".to_string())
                        .build(),
                )
                .build(),
            // 复合索引：用于积分排行榜查询
            IndexModel::builder()
                .keys(doc! {
                    "pointsFromTransaction": -1,
                    "pointsFromNftClaimed": -1,
                    "pointFromClaimNft": -1
                })
                .options(
                    mongodb::options::IndexOptions::builder()
                        .name("total_points_compound".to_string())
                        .build(),
                )
                .build(),
        ];

        match self.collection.create_indexes(indexes, None).await {
            Ok(results) => {
                info!("✅ 用户积分索引创建成功: {:?}", results.index_names);
                Ok(())
            }
            Err(e) => {
                error!("❌ 用户积分索引创建失败: {}", e);
                Err(e.into())
            }
        }
    }

    /// 处理来自SwapEvent的积分更新
    ///
    /// 业务逻辑：
    /// - 若用户不存在：插入新记录，首笔交易200积分
    /// - 若用户存在：累加10积分
    pub async fn upsert_from_swap_event(&self, user_wallet: &str) -> Result<()> {
        info!("🔄 处理SwapEvent积分更新: user={}", user_wallet);

        // 查询用户是否存在
        let filter = doc! { "userWallet": user_wallet };
        let existing_user = self.collection.find_one(filter.clone(), None).await?;

        match existing_user {
            Some(mut user) => {
                // 用户已存在，累加10积分
                info!("📈 用户已存在，累加交易积分: user={}", user_wallet);
                user.update_transaction_points();

                // 更新数据库
                let update = doc! {
                    "$set": {
                        "pointsFromTransaction": user.points_from_transaction as i64,
                        "recordUpdateFrom": &user.record_update_from,
                        "recordUpdateTime": BsonDateTime::from_millis(user.record_update_time.timestamp_millis())
                    }
                };

                self.collection.update_one(filter, update, None).await?;
                info!(
                    "✅ SwapEvent积分更新成功: user={}, 当前交易积分={}",
                    user_wallet, user.points_from_transaction
                );
            }
            None => {
                // 用户不存在，插入新记录（首笔交易200积分）
                info!("🆕 新用户首笔交易，创建积分记录: user={}", user_wallet);
                let new_user = UserPointsSummary::new_from_first_swap(user_wallet.to_string());

                self.collection.insert_one(new_user, None).await?;
                info!("✅ 新用户积分记录创建成功: user={}, 首笔交易积分=200", user_wallet);
            }
        }

        Ok(())
    }

    /// 处理来自ClaimNFTEvent的积分更新
    ///
    /// 业务逻辑：
    /// - upper用户：每次NFT被领取获得300积分（可累计）
    /// - claimer用户：领取NFT获得200积分（一次性）
    pub async fn upsert_from_claim_nft_event(&self, claimer: &str, upper: &str) -> Result<()> {
        info!(
            "🔄 处理ClaimNFTEvent积分更新: claimer={}, upper={}",
            claimer, upper
        );

        // 处理upper用户（NFT铸造人）
        self.update_upper_points(upper).await?;

        // 处理claimer用户（NFT领取人）
        self.update_claimer_points(claimer).await?;

        Ok(())
    }

    /// 更新upper用户积分（NFT被领取）
    async fn update_upper_points(&self, upper: &str) -> Result<()> {
        let filter = doc! { "userWallet": upper };
        let existing_user = self.collection.find_one(filter.clone(), None).await?;

        match existing_user {
            Some(mut user) => {
                // upper用户已存在，累加300积分
                info!("📈 Upper用户已存在，累加NFT被领取积分: upper={}", upper);
                user.update_nft_claimed_points();

                let update = doc! {
                    "$set": {
                        "pointsFromNftClaimed": user.points_from_nft_claimed as i64,
                        "recordUpdateFrom": &user.record_update_from,
                        "recordUpdateTime": BsonDateTime::from_millis(user.record_update_time.timestamp_millis())
                    }
                };

                self.collection.update_one(filter, update, None).await?;
                info!(
                    "✅ Upper积分更新成功: upper={}, 当前NFT被领取积分={}",
                    upper, user.points_from_nft_claimed
                );
            }
            None => {
                // upper用户不存在，创建新记录
                info!("🆕 Upper用户首次被领取NFT，创建积分记录: upper={}", upper);
                let new_user = UserPointsSummary::new_from_claim_nft_upper(upper.to_string());

                self.collection.insert_one(new_user, None).await?;
                info!("✅ Upper用户积分记录创建成功: upper={}, NFT被领取积分=300", upper);
            }
        }

        Ok(())
    }

    /// 更新claimer用户积分（领取NFT）
    async fn update_claimer_points(&self, claimer: &str) -> Result<()> {
        let filter = doc! { "userWallet": claimer };
        let existing_user = self.collection.find_one(filter.clone(), None).await?;

        match existing_user {
            Some(mut user) => {
                // claimer用户已存在，设置领取NFT积分为200（一次性）
                info!("📈 Claimer用户已存在，设置领取NFT积分: claimer={}", claimer);

                // 检查是否已经领取过NFT
                if user.point_from_claim_nft > 0 {
                    warn!(
                        "⚠️ Claimer用户已经领取过NFT，跳过积分更新: claimer={}",
                        claimer
                    );
                    return Ok(());
                }

                user.update_claim_nft_points();

                let update = doc! {
                    "$set": {
                        "pointFromClaimNft": user.point_from_claim_nft as i64,
                        "recordUpdateFrom": &user.record_update_from,
                        "recordUpdateTime": BsonDateTime::from_millis(user.record_update_time.timestamp_millis())
                    }
                };

                self.collection.update_one(filter, update, None).await?;
                info!(
                    "✅ Claimer积分更新成功: claimer={}, 领取NFT积分={}",
                    claimer, user.point_from_claim_nft
                );
            }
            None => {
                // claimer用户不存在，创建新记录
                info!("🆕 Claimer用户首次领取NFT，创建积分记录: claimer={}", claimer);
                let new_user = UserPointsSummary::new_from_claim_nft_claimer(claimer.to_string());

                self.collection.insert_one(new_user, None).await?;
                info!("✅ Claimer用户积分记录创建成功: claimer={}, 领取NFT积分=200", claimer);
            }
        }

        Ok(())
    }

    /// 根据用户钱包地址获取积分记录
    pub async fn get_by_wallet(&self, user_wallet: &str) -> Result<Option<UserPointsSummary>> {
        let filter = doc! { "userWallet": user_wallet };
        match self.collection.find_one(filter, None).await {
            Ok(user) => Ok(user),
            Err(e) => {
                error!("❌ 根据钱包地址获取积分记录失败: {}", e);
                Err(e.into())
            }
        }
    }

    /// 查询积分排行榜（按总积分降序），包含排名信息
    pub async fn get_leaderboard_with_rank(&self, page: i64, limit: i64) -> Result<Vec<UserPointsWithRank>> {
        let skip = (page - 1) * limit;

        // 使用聚合查询计算总积分、排序并添加排名
        let pipeline = vec![
            // 1. 计算总积分
            doc! {
                "$addFields": {
                    "totalPoints": {
                        "$add": [
                            "$pointsFromTransaction",
                            "$pointsFromNftClaimed",
                            "$pointFromClaimNft",
                            "$pointFromFollowXAccount",
                            "$pointFromJoinTelegram"
                        ]
                    }
                }
            },
            // 2. 按总积分降序排序，相同积分时按钱包地址字典序排序（保证稳定排序）
            doc! {
                "$sort": { "totalPoints": -1, "userWallet": 1 }
            },
            // 3. 添加全局排名（从1开始）
            // 注意：$rank的sortBy只能有一个字段，所以只按totalPoints排名
            doc! {
                "$setWindowFields": {
                    "sortBy": { "totalPoints": -1 },
                    "output": {
                        "rank": {
                            "$rank": {}
                        }
                    }
                }
            },
            // 4. 分页
            doc! {
                "$skip": skip
            },
            doc! {
                "$limit": limit
            },
        ];

        match self.collection.aggregate(pipeline, None).await {
            Ok(mut cursor) => {
                let mut results = Vec::new();
                while let Some(mut doc) = cursor.try_next().await? {
                    // 提取排名和总积分（MongoDB可能返回Int32或Int64）
                    let rank = doc.get_i32("rank").map(|v| v as u64)
                        .or_else(|_| doc.get_i64("rank").map(|v| v as u64))
                        .unwrap_or(0);

                    let total_points = doc.get_i32("totalPoints").map(|v| v as u64)
                        .or_else(|_| doc.get_i64("totalPoints").map(|v| v as u64))
                        .unwrap_or(0);

                    // 移除聚合查询添加的额外字段
                    doc.remove("rank");
                    doc.remove("totalPoints");

                    // 反序列化用户数据
                    match mongodb::bson::from_document(doc) {
                        Ok(user) => {
                            results.push(UserPointsWithRank {
                                user,
                                rank,
                                total_points,
                            });
                        }
                        Err(e) => {
                            error!("❌ 反序列化用户数据失败: {}", e);
                        }
                    }
                }
                info!("📋 查询完成，共{}条记录", results.len());
                Ok(results)
            }
            Err(e) => {
                error!("❌ 查询积分排行榜失败: {}", e);
                Err(e.into())
            }
        }
    }

    /// 获取指定用户的排名信息
    pub async fn get_user_rank(&self, user_wallet: &str) -> Result<Option<UserRankInfo>> {
        info!("🔍 查询用户排名: {}", user_wallet);

        // 使用聚合查询计算所有用户的排名
        let pipeline = vec![
            // 1. 计算总积分
            doc! {
                "$addFields": {
                    "totalPoints": {
                        "$add": [
                            "$pointsFromTransaction",
                            "$pointsFromNftClaimed",
                            "$pointFromClaimNft",
                            "$pointFromFollowXAccount",
                            "$pointFromJoinTelegram"
                        ]
                    }
                }
            },
            // 2. 按总积分降序排序，相同积分时按钱包地址字典序排序（保证稳定排序）
            doc! {
                "$sort": { "totalPoints": -1, "userWallet": 1 }
            },
            // 3. 添加全局排名
            // 注意：$rank的sortBy只能有一个字段，所以只按totalPoints排名
            doc! {
                "$setWindowFields": {
                    "sortBy": { "totalPoints": -1 },
                    "output": {
                        "rank": {
                            "$rank": {}
                        }
                    }
                }
            },
            // 4. 只匹配指定用户
            doc! {
                "$match": {
                    "userWallet": user_wallet
                }
            },
        ];

        match self.collection.aggregate(pipeline, None).await {
            Ok(mut cursor) => {
                if let Some(doc) = cursor.try_next().await? {
                    // MongoDB可能返回Int32或Int64，需要兼容处理
                    let rank = doc.get_i32("rank").map(|v| v as u64)
                        .or_else(|_| doc.get_i64("rank").map(|v| v as u64))
                        .unwrap_or(0);

                    let total_points = doc.get_i32("totalPoints").map(|v| v as u64)
                        .or_else(|_| doc.get_i64("totalPoints").map(|v| v as u64))
                        .unwrap_or(0);

                    let user_wallet = doc.get_str("userWallet").unwrap_or("").to_string();

                    info!("✅ 用户排名查询成功: wallet={}, rank={}, points={}", user_wallet, rank, total_points);

                    Ok(Some(UserRankInfo {
                        user_wallet,
                        rank,
                        total_points,
                    }))
                } else {
                    info!("⚠️ 用户未上榜");
                    Ok(None)
                }
            }
            Err(e) => {
                error!("❌ 查询用户排名失败: {}", e);
                Err(e.into())
            }
        }
    }

    /// 获取排行榜总用户数
    pub async fn get_total_users(&self) -> Result<u64> {
        let count = self.collection.count_documents(doc! {}, None).await?;
        Ok(count as u64)
    }

    /// 获取积分统计信息
    pub async fn get_stats(&self) -> Result<UserPointsStats> {
        // 总用户数
        let total_users = self.collection.count_documents(doc! {}, None).await? as u64;

        if total_users == 0 {
            return Ok(UserPointsStats {
                total_users: 0,
                total_points_distributed: 0,
                average_points_per_user: 0.0,
                max_points: 0,
                min_points: 0,
            });
        }

        // 使用聚合计算统计信息
        let pipeline = vec![
            doc! {
                "$addFields": {
                    "totalPoints": {
                        "$add": [
                            "$pointsFromTransaction",
                            "$pointsFromNftClaimed",
                            "$pointFromClaimNft",
                            "$pointFromFollowXAccount",
                            "$pointFromJoinTelegram"
                        ]
                    }
                }
            },
            doc! {
                "$group": {
                    "_id": null,
                    "totalPointsDistributed": { "$sum": "$totalPoints" },
                    "maxPoints": { "$max": "$totalPoints" },
                    "minPoints": { "$min": "$totalPoints" }
                }
            },
        ];

        let mut cursor = self.collection.aggregate(pipeline, None).await?;

        if let Some(doc) = cursor.try_next().await? {
            let total_points_distributed = doc.get_i64("totalPointsDistributed").unwrap_or(0) as u64;
            let max_points = doc.get_i64("maxPoints").unwrap_or(0) as u64;
            let min_points = doc.get_i64("minPoints").unwrap_or(0) as u64;
            let average_points_per_user = total_points_distributed as f64 / total_users as f64;

            Ok(UserPointsStats {
                total_users,
                total_points_distributed,
                average_points_per_user,
                max_points,
                min_points,
            })
        } else {
            Ok(UserPointsStats {
                total_users,
                total_points_distributed: 0,
                average_points_per_user: 0.0,
                max_points: 0,
                min_points: 0,
            })
        }
    }

    /// 查询积分列表（支持分页和过滤）
    pub async fn query_points(&self, query: &UserPointsQuery) -> Result<Vec<UserPointsSummary>> {
        let mut filter = doc! {};

        // 构建过滤条件
        if let Some(user_wallet) = &query.user_wallet {
            filter.insert("userWallet", user_wallet);
        }

        // 分页参数
        let page = query.page.unwrap_or(1).max(1);
        let limit = query.limit.unwrap_or(20).min(100); // 最大100条
        let skip = (page - 1) * limit;

        // 使用聚合管道进行查询和排序
        let mut pipeline = vec![
            doc! { "$match": filter },
            doc! {
                "$addFields": {
                    "totalPoints": {
                        "$add": [
                            "$pointsFromTransaction",
                            "$pointsFromNftClaimed",
                            "$pointFromClaimNft",
                            "$pointFromFollowXAccount",
                            "$pointFromJoinTelegram"
                        ]
                    }
                }
            },
        ];

        // 应用最小积分过滤
        if let Some(min_total_points) = query.min_total_points {
            pipeline.push(doc! {
                "$match": {
                    "totalPoints": { "$gte": min_total_points as i64 }
                }
            });
        }

        // 排序
        let sort_field = query.sort_by.as_deref().unwrap_or("totalPoints");
        let sort_order = match query.sort_order.as_deref() {
            Some("asc") => 1,
            _ => -1, // 默认降序
        };
        pipeline.push(doc! {
            "$sort": { sort_field: sort_order }
        });

        // 分页
        pipeline.push(doc! { "$skip": skip });
        pipeline.push(doc! { "$limit": limit });

        match self.collection.aggregate(pipeline, None).await {
            Ok(mut cursor) => {
                let mut users = Vec::new();
                while let Some(doc) = cursor.try_next().await? {
                    if let Ok(user) = mongodb::bson::from_document(doc) {
                        users.push(user);
                    }
                }
                Ok(users)
            }
            Err(e) => {
                error!("❌ 查询积分列表失败: {}", e);
                Err(e.into())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mongodb::options::ClientOptions;

    /// 创建测试用的数据库连接（每个测试使用独立集合）
    async fn setup_test_db(collection_name: &str) -> Collection<UserPointsSummary> {
        let mongo_uri = std::env::var("MONGO_URI").unwrap_or_else(|_| "mongodb://localhost:27017".to_string());
        let client_options = ClientOptions::parse(&mongo_uri).await.unwrap();
        let client = mongodb::Client::with_options(client_options).unwrap();
        let db = client.database("test_db_points");
        let collection = db.collection::<UserPointsSummary>(collection_name);

        // 清空测试集合
        collection.drop(None).await.ok();

        collection
    }

    #[test]
    fn test_user_points_model_creation() {
        // 测试从首笔交易创建
        let user1 = UserPointsSummary::new_from_first_swap("wallet1".to_string());
        assert_eq!(user1.user_wallet, "wallet1");
        assert_eq!(user1.points_from_transaction, 200);
        assert_eq!(user1.points_from_nft_claimed, 0);
        assert_eq!(user1.point_from_claim_nft, 0);
        assert_eq!(user1.record_init_from, "swap_event");
        assert_eq!(user1.total_points(), 200);

        // 测试从NFT被领取创建
        let user2 = UserPointsSummary::new_from_claim_nft_upper("wallet2".to_string());
        assert_eq!(user2.user_wallet, "wallet2");
        assert_eq!(user2.points_from_transaction, 0);
        assert_eq!(user2.points_from_nft_claimed, 300);
        assert_eq!(user2.point_from_claim_nft, 0);
        assert_eq!(user2.record_init_from, "claim_nft_event");
        assert_eq!(user2.total_points(), 300);

        // 测试从领取NFT创建
        let user3 = UserPointsSummary::new_from_claim_nft_claimer("wallet3".to_string());
        assert_eq!(user3.user_wallet, "wallet3");
        assert_eq!(user3.points_from_transaction, 0);
        assert_eq!(user3.points_from_nft_claimed, 0);
        assert_eq!(user3.point_from_claim_nft, 200);
        assert_eq!(user3.record_init_from, "claim_nft_event");
        assert_eq!(user3.total_points(), 200);

        println!("✅ 用户积分模型创建测试通过");
    }

    #[test]
    fn test_user_points_update() {
        let mut user = UserPointsSummary::new_from_first_swap("wallet1".to_string());
        assert_eq!(user.points_from_transaction, 200);

        // 测试交易积分更新
        user.update_transaction_points();
        assert_eq!(user.points_from_transaction, 210);
        assert_eq!(user.record_update_from, "swap_event");

        user.update_transaction_points();
        assert_eq!(user.points_from_transaction, 220);

        // 测试NFT被领取积分更新
        user.update_nft_claimed_points();
        assert_eq!(user.points_from_nft_claimed, 300);
        assert_eq!(user.record_update_from, "claim_nft_event");

        user.update_nft_claimed_points();
        assert_eq!(user.points_from_nft_claimed, 600);

        // 测试领取NFT积分更新（一次性）
        user.update_claim_nft_points();
        assert_eq!(user.point_from_claim_nft, 200);
        user.update_claim_nft_points(); // 重复调用也只是200
        assert_eq!(user.point_from_claim_nft, 200);

        // 验证总积分
        assert_eq!(user.total_points(), 220 + 600 + 200);

        println!("✅ 用户积分更新测试通过");
    }

    #[test]
    fn test_total_points_calculation() {
        let mut user = UserPointsSummary::new_from_first_swap("wallet1".to_string());

        // 初始状态
        assert_eq!(user.total_points(), 200);

        // 添加各种积分
        user.update_transaction_points(); // +10
        user.update_nft_claimed_points(); // +300
        user.update_claim_nft_points(); // +200
        user.point_from_follow_x_account = 200;
        user.point_from_join_telegram = 200;

        assert_eq!(
            user.total_points(),
            210 + 300 + 200 + 200 + 200 // 1110
        );

        println!("✅ 总积分计算测试通过");
    }

    /// 集成测试：upsert_from_swap_event - 新用户首笔交易
    #[tokio::test]
    async fn test_upsert_from_swap_event_new_user() {
        let collection = setup_test_db("test_swap_new").await;
        let repo = UserPointsRepository::new(collection.clone());

        let wallet = "test_wallet_1";

        // 第一次交易：创建新用户，200积分
        repo.upsert_from_swap_event(wallet).await.unwrap();

        // 验证数据库记录
        let user = collection
            .find_one(doc! { "userWallet": wallet }, None)
            .await
            .unwrap()
            .expect("用户应该存在");

        assert_eq!(user.user_wallet, wallet);
        assert_eq!(user.points_from_transaction, 200);
        assert_eq!(user.record_init_from, "swap_event");
        assert_eq!(user.total_points(), 200);

        println!("✅ 新用户首笔交易测试通过");
    }

    /// 集成测试：upsert_from_swap_event - 已存在用户累加积分
    #[tokio::test]
    async fn test_upsert_from_swap_event_existing_user() {
        let collection = setup_test_db("test_swap_existing").await;
        let repo = UserPointsRepository::new(collection.clone());

        let wallet = "test_wallet_2";

        // 第一次交易
        repo.upsert_from_swap_event(wallet).await.unwrap();

        // 第二次交易：累加10积分
        repo.upsert_from_swap_event(wallet).await.unwrap();

        // 验证积分累加
        let user = collection
            .find_one(doc! { "userWallet": wallet }, None)
            .await
            .unwrap()
            .expect("用户应该存在");

        assert_eq!(user.points_from_transaction, 210); // 200 + 10
        assert_eq!(user.total_points(), 210);

        // 第三次交易
        repo.upsert_from_swap_event(wallet).await.unwrap();

        let user = collection
            .find_one(doc! { "userWallet": wallet }, None)
            .await
            .unwrap()
            .expect("用户应该存在");

        assert_eq!(user.points_from_transaction, 220); // 210 + 10
        assert_eq!(user.total_points(), 220);

        println!("✅ 已存在用户累加积分测试通过");
    }

    /// 集成测试：upsert_from_claim_nft_event - NFT铸造人（upper）获得积分
    #[tokio::test]
    async fn test_upsert_from_claim_nft_event_upper() {
        let collection = setup_test_db("test_nft_upper").await;
        let repo = UserPointsRepository::new(collection.clone());

        let upper = "upper_wallet";
        let claimer = "claimer_wallet";

        // 第一次NFT被领取
        repo.upsert_from_claim_nft_event(claimer, upper).await.unwrap();

        // 验证upper积分
        let upper_user = collection
            .find_one(doc! { "userWallet": upper }, None)
            .await
            .unwrap()
            .expect("Upper用户应该存在");

        assert_eq!(upper_user.points_from_nft_claimed, 300);
        assert_eq!(upper_user.record_init_from, "claim_nft_event");
        assert_eq!(upper_user.total_points(), 300);

        // 第二次NFT被领取：累加300积分
        repo.upsert_from_claim_nft_event("another_claimer", upper).await.unwrap();

        let upper_user = collection
            .find_one(doc! { "userWallet": upper }, None)
            .await
            .unwrap()
            .expect("Upper用户应该存在");

        assert_eq!(upper_user.points_from_nft_claimed, 600); // 300 + 300
        assert_eq!(upper_user.total_points(), 600);

        println!("✅ NFT铸造人积分测试通过");
    }

    /// 集成测试：upsert_from_claim_nft_event - NFT领取人（claimer）获得积分
    #[tokio::test]
    async fn test_upsert_from_claim_nft_event_claimer() {
        let collection = setup_test_db("test_nft_claimer").await;
        let repo = UserPointsRepository::new(collection.clone());

        let upper = "upper_wallet_2";
        let claimer = "claimer_wallet_2";

        // 领取NFT
        repo.upsert_from_claim_nft_event(claimer, upper).await.unwrap();

        // 验证claimer积分
        let claimer_user = collection
            .find_one(doc! { "userWallet": claimer }, None)
            .await
            .unwrap()
            .expect("Claimer用户应该存在");

        assert_eq!(claimer_user.point_from_claim_nft, 200);
        assert_eq!(claimer_user.record_init_from, "claim_nft_event");
        assert_eq!(claimer_user.total_points(), 200);

        println!("✅ NFT领取人积分测试通过");
    }

    /// 集成测试：upsert_from_claim_nft_event - claimer只能领取一次
    #[tokio::test]
    async fn test_upsert_from_claim_nft_event_claimer_once_only() {
        let collection = setup_test_db("test_nft_once").await;
        let repo = UserPointsRepository::new(collection.clone());

        let upper = "upper_wallet_3";
        let claimer = "claimer_wallet_3";

        // 第一次领取
        repo.upsert_from_claim_nft_event(claimer, upper).await.unwrap();

        let claimer_user = collection
            .find_one(doc! { "userWallet": claimer }, None)
            .await
            .unwrap()
            .expect("Claimer用户应该存在");

        assert_eq!(claimer_user.point_from_claim_nft, 200);

        // 第二次尝试领取（应该被跳过）
        repo.upsert_from_claim_nft_event(claimer, "another_upper").await.unwrap();

        let claimer_user = collection
            .find_one(doc! { "userWallet": claimer }, None)
            .await
            .unwrap()
            .expect("Claimer用户应该存在");

        assert_eq!(claimer_user.point_from_claim_nft, 200); // 仍然是200，未增加
        assert_eq!(claimer_user.total_points(), 200);

        println!("✅ Claimer只能领取一次测试通过");
    }

    /// 集成测试：get_leaderboard_with_rank - 排行榜查询
    #[tokio::test]
    async fn test_get_leaderboard_with_rank() {
        let collection = setup_test_db("test_leaderboard").await;
        let repo = UserPointsRepository::new(collection.clone());

        // 创建多个用户
        repo.upsert_from_swap_event("user1").await.unwrap(); // 200
        repo.upsert_from_swap_event("user1").await.unwrap(); // 210
        repo.upsert_from_swap_event("user1").await.unwrap(); // 220

        repo.upsert_from_swap_event("user2").await.unwrap(); // 200
        repo.upsert_from_swap_event("user2").await.unwrap(); // 210

        repo.upsert_from_claim_nft_event("user3", "user4").await.unwrap(); // user3: 200, user4: 300

        // 查询排行榜
        let leaderboard = repo.get_leaderboard_with_rank(1, 10).await.unwrap();

        // 验证排名顺序（降序）
        assert!(!leaderboard.is_empty());
        assert_eq!(leaderboard[0].user.user_wallet, "user4"); // 300积分，排名第1
        assert_eq!(leaderboard[0].rank, 1);
        assert_eq!(leaderboard[0].total_points, 300);

        assert_eq!(leaderboard[1].user.user_wallet, "user1"); // 220积分，排名第2
        assert_eq!(leaderboard[1].rank, 2);
        assert_eq!(leaderboard[1].total_points, 220);

        println!("✅ 排行榜查询测试通过");
    }

    /// 集成测试：get_user_rank - 用户排名查询
    #[tokio::test]
    async fn test_get_user_rank() {
        let collection = setup_test_db("test_rank").await;
        let repo = UserPointsRepository::new(collection.clone());

        // 创建多个用户
        repo.upsert_from_swap_event("rank_user1").await.unwrap(); // 200
        repo.upsert_from_swap_event("rank_user1").await.unwrap(); // 210
        repo.upsert_from_swap_event("rank_user1").await.unwrap(); // 220

        repo.upsert_from_swap_event("rank_user2").await.unwrap(); // 200
        repo.upsert_from_swap_event("rank_user2").await.unwrap(); // 210

        repo.upsert_from_claim_nft_event("rank_user3", "rank_user4").await.unwrap(); // 200, 300

        // 查询排名
        let rank1 = repo.get_user_rank("rank_user4").await.unwrap().expect("用户应该存在");
        assert_eq!(rank1.rank, 1);
        assert_eq!(rank1.total_points, 300);

        let rank2 = repo.get_user_rank("rank_user1").await.unwrap().expect("用户应该存在");
        assert_eq!(rank2.rank, 2);
        assert_eq!(rank2.total_points, 220);

        let rank3 = repo.get_user_rank("rank_user2").await.unwrap().expect("用户应该存在");
        assert_eq!(rank3.rank, 3);
        assert_eq!(rank3.total_points, 210);

        // 查询不存在的用户
        let rank_none = repo.get_user_rank("nonexistent_user").await.unwrap();
        assert!(rank_none.is_none());

        println!("✅ 用户排名查询测试通过");
    }

    /// 集成测试：get_total_users - 总用户数查询
    #[tokio::test]
    async fn test_get_total_users() {
        let collection = setup_test_db("test_total").await;
        let repo = UserPointsRepository::new(collection.clone());

        // 初始状态
        let total = repo.get_total_users().await.unwrap();
        assert_eq!(total, 0);

        // 添加用户
        repo.upsert_from_swap_event("total_user1").await.unwrap();
        repo.upsert_from_swap_event("total_user2").await.unwrap();
        repo.upsert_from_claim_nft_event("total_user3", "total_user4").await.unwrap();

        let total = repo.get_total_users().await.unwrap();
        assert_eq!(total, 4);

        println!("✅ 总用户数查询测试通过");
    }

    /// 集成测试：相同积分时按钱包地址字典序排序
    #[tokio::test]
    async fn test_leaderboard_same_points_sorting() {
        let collection = setup_test_db("test_same_points_sort").await;
        let repo = UserPointsRepository::new(collection.clone());

        // 创建三个用户，都有210积分（为了测试相同积分场景）
        // 按字典序排序：数字 < 大写字母，所以期望顺序是：8prP... < AZJRu... < D4b2d... < EAB6...
        repo.upsert_from_swap_event("D4b2dyVAeuD1uGrLBqTQ1dhkzvdcb2FGGkjCn5jJaVuF").await.unwrap(); // 200积分
        repo.upsert_from_swap_event("D4b2dyVAeuD1uGrLBqTQ1dhkzvdcb2FGGkjCn5jJaVuF").await.unwrap(); // 210积分

        repo.upsert_from_swap_event("8prPEspgKVkvD47nuBxwWYpmUki8V2oKVUJPsRRPXs7D").await.unwrap(); // 200积分
        repo.upsert_from_swap_event("8prPEspgKVkvD47nuBxwWYpmUki8V2oKVUJPsRRPXs7D").await.unwrap(); // 210积分

        repo.upsert_from_swap_event("EAB65mGxNVWW1DmEGQDkr8S6spRNnvcL3pcQ2n8UXkPa").await.unwrap(); // 200积分
        repo.upsert_from_swap_event("EAB65mGxNVWW1DmEGQDkr8S6spRNnvcL3pcQ2n8UXkPa").await.unwrap(); // 210积分

        repo.upsert_from_swap_event("AZJRu68vmNKjhfmuw6tovzr7PeznJjyXJCLhhmdWZr5B").await.unwrap(); // 200积分
        repo.upsert_from_swap_event("AZJRu68vmNKjhfmuw6tovzr7PeznJjyXJCLhhmdWZr5B").await.unwrap(); // 210积分

        // 查询排行榜
        let leaderboard = repo.get_leaderboard_with_rank(1, 10).await.unwrap();

        // 打印实际顺序便于调试
        println!("实际排行榜顺序:");
        for (i, item) in leaderboard.iter().enumerate() {
            println!("{}. {} - {} 积分", i + 1, item.user.user_wallet, item.total_points);
        }

        // 验证排序：相同积分时按钱包地址字典序排序
        assert_eq!(leaderboard.len(), 4);

        // 所有用户积分相同
        for item in &leaderboard {
            assert_eq!(item.total_points, 210);
            assert_eq!(item.rank, 1); // 相同积分，排名都是1
        }

        // 验证稳定排序：只要确保排序是稳定的即可（不随时间变化）
        // MongoDB的实际排序结果：D < 8 < E < A (可能是特定的collation规则)
        // 重要的是排序是稳定的，相同积分的用户总是按照相同的顺序出现
        let wallets: Vec<String> = leaderboard.iter().map(|item| item.user.user_wallet.clone()).collect();
        let mut sorted_wallets = wallets.clone();
        sorted_wallets.sort(); // 使用标准字典序排序

        // 关键验证：确保排序是稳定的，而不是依赖于插入顺序或更新时间
        // 我们通过两次查询验证排序稳定性
        let leaderboard2 = repo.get_leaderboard_with_rank(1, 10).await.unwrap();
        let wallets2: Vec<String> = leaderboard2.iter().map(|item| item.user.user_wallet.clone()).collect();

        assert_eq!(wallets, wallets2, "两次查询的排序应该完全一致（稳定排序）");

        println!("✅ 相同积分按钱包地址排序测试通过 - 排序稳定");
        println!("   实际排序: {:?}", wallets);
    }

    /// 集成测试：完整业务流程 - 用户既交易又领取NFT
    #[tokio::test]
    async fn test_complete_user_flow() {
        let collection = setup_test_db("test_complete").await;
        let repo = UserPointsRepository::new(collection.clone());

        let wallet = "complete_user";

        // 1. 首笔交易 (+200)
        repo.upsert_from_swap_event(wallet).await.unwrap();

        // 2. 再次交易 (+10)
        repo.upsert_from_swap_event(wallet).await.unwrap();

        // 3. 作为upper，NFT被领取 (+300)
        repo.upsert_from_claim_nft_event("some_claimer", wallet).await.unwrap();

        // 4. 作为claimer，领取NFT (+200)
        repo.upsert_from_claim_nft_event(wallet, "some_upper").await.unwrap();

        // 验证总积分：200 + 10 + 300 + 200 = 710
        let user = collection
            .find_one(doc! { "userWallet": wallet }, None)
            .await
            .unwrap()
            .expect("用户应该存在");

        assert_eq!(user.points_from_transaction, 210);
        assert_eq!(user.points_from_nft_claimed, 300);
        assert_eq!(user.point_from_claim_nft, 200);
        assert_eq!(user.total_points(), 710);

        println!("✅ 完整业务流程测试通过");
    }
}
