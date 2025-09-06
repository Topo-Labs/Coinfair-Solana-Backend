use super::model::*;
use chrono::Utc;
use futures::TryStreamExt;
use mongodb::{
    bson::{doc, Document},
    options::{FindOptions, IndexOptions, UpdateOptions},
    Collection, IndexModel,
};
use tracing::info;
use utils::AppResult;

/// 代币信息数据库操作接口
#[derive(Clone, Debug)]
pub struct TokenInfoRepository {
    collection: Collection<TokenInfo>,
}

impl TokenInfoRepository {
    /// 创建新的仓库实例
    pub fn new(collection: Collection<TokenInfo>) -> Self {
        Self { collection }
    }

    /// 获取集合引用（用于直接数据库操作）
    pub fn get_collection(&self) -> &Collection<TokenInfo> {
        &self.collection
    }

    /// 初始化数据库索引
    pub async fn init_indexes(&self) -> AppResult<()> {
        let indexes = vec![
            // 代币地址唯一索引 (主键)
            IndexModel::builder()
                .keys(doc! { "address": 1 })
                .options(IndexOptions::builder().unique(true).build())
                .build(),
            // 符号索引 (常用查询)
            IndexModel::builder().keys(doc! { "symbol": 1 }).build(),
            // 名称索引 (常用查询)
            IndexModel::builder().keys(doc! { "name": 1 }).build(),
            // 状态索引 (活跃代币查询)
            IndexModel::builder().keys(doc! { "status": 1 }).build(),
            // 数据来源索引
            IndexModel::builder().keys(doc! { "source": 1 }).build(),
            // 验证状态索引
            IndexModel::builder().keys(doc! { "verification": 1 }).build(),
            // 日交易量索引 (排序用)
            IndexModel::builder().keys(doc! { "daily_volume": -1 }).build(),
            // 创建时间索引 (排序用)
            IndexModel::builder().keys(doc! { "created_at": -1 }).build(),
            // 推送时间索引
            IndexModel::builder().keys(doc! { "push_time": -1 }).build(),
            // 更新时间索引
            IndexModel::builder().keys(doc! { "updated_at": -1 }).build(),
            // 标签索引 (多值字段)
            IndexModel::builder().keys(doc! { "tags": 1 }).build(),
            // 复合索引 - 状态和创建时间 (常用组合查询)
            IndexModel::builder()
                .keys(doc! {
                    "status": 1,
                    "created_at": -1
                })
                .build(),
            // 复合索引 - 验证状态和日交易量 (白名单高交易量代币)
            IndexModel::builder()
                .keys(doc! {
                    "verification": 1,
                    "daily_volume": -1
                })
                .build(),
            // 文本搜索索引 (名称、符号、地址搜索)
            IndexModel::builder()
                .keys(doc! {
                    "name": "text",
                    "symbol": "text",
                    "address": "text"
                })
                .options(
                    IndexOptions::builder()
                        .weights(doc! {
                            "symbol": 10,
                            "name": 5,
                            "address": 1
                        })
                        .build(),
                )
                .build(),
            // 扩展字段索引 - 项目状态过滤优化
            IndexModel::builder()
                .keys(doc! { "extensions.project_state": 1 })
                .build(),
            // 扩展字段索引 - 创建者过滤优化
            IndexModel::builder().keys(doc! { "extensions.creator": 1 }).build(),
        ];

        self.collection.create_indexes(indexes, None).await?;
        info!("✅ TokenInfo数据库索引初始化完成");
        Ok(())
    }

    /// 推送代币信息 (upsert操作)
    pub async fn push_token(&self, request: TokenPushRequest) -> AppResult<TokenPushResponse> {
        let now = Utc::now();

        // 检查是否已存在
        let existing = self.find_by_address(&request.address).await?;

        let (operation, token_info) = if let Some(mut existing_token) = existing {
            // 更新现有记录
            existing_token.update_from_push_request(request.clone());
            ("updated".to_string(), existing_token)
        } else {
            // 创建新记录
            ("created".to_string(), TokenInfo::from_push_request(request.clone()))
        };

        // 执行upsert操作
        let filter = doc! { "address": &request.address };
        let update = doc! {
            "$set": mongodb::bson::to_bson(&token_info)?
        };
        let options = UpdateOptions::builder().upsert(true).build();

        let result = self.collection.update_one(filter, update, options).await?;

        let success = result.upserted_id.is_some() || result.modified_count > 0;
        let message = if success {
            format!("Token {} successfully {}", request.address, operation)
        } else {
            format!("Failed to {} token {}", operation, request.address)
        };

        Ok(TokenPushResponse {
            success,
            address: request.address,
            operation,
            message,
            timestamp: now,
        })
    }

    /// 根据地址查询代币信息
    pub async fn find_by_address(&self, address: &str) -> AppResult<Option<TokenInfo>> {
        let filter = doc! { "address": address };
        Ok(self.collection.find_one(filter, None).await?)
    }

    /// 根据符号查询代币信息
    pub async fn find_by_symbol(&self, symbol: &str) -> AppResult<Vec<TokenInfo>> {
        let filter = doc! { "symbol": symbol };
        let options = FindOptions::builder()
            .sort(doc! { "daily_volume": -1 })
            .limit(10)
            .build();

        let mut cursor = self.collection.find(filter, options).await?;
        let mut tokens = Vec::new();

        while cursor.advance().await? {
            tokens.push(cursor.deserialize_current()?);
        }

        Ok(tokens)
    }

    /// 查询代币列表 (带分页和过滤)
    pub async fn query_tokens(&self, query: &TokenListQuery) -> AppResult<TokenListResponse> {
        let mut filter = Document::new();

        // 构建查询条件
        if let Some(status) = &query.status {
            filter.insert("status", mongodb::bson::to_bson(status)?);
        }

        if let Some(source) = &query.source {
            filter.insert("source", mongodb::bson::to_bson(source)?);
        }

        if let Some(verification) = &query.verification {
            filter.insert("verification", mongodb::bson::to_bson(verification)?);
        }

        // 标签过滤 (支持多个标签，用逗号分隔)
        if let Some(tags_str) = &query.tags {
            let tags: Vec<String> = tags_str
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();

            if !tags.is_empty() {
                filter.insert("tags", doc! { "$in": tags });
            }
        }

        // 交易量范围过滤
        if query.min_volume.is_some() || query.max_volume.is_some() {
            let mut volume_filter = Document::new();
            if let Some(min_volume) = query.min_volume {
                volume_filter.insert("$gte", min_volume);
            }
            if let Some(max_volume) = query.max_volume {
                volume_filter.insert("$lte", max_volume);
            }
            filter.insert("daily_volume", volume_filter);
        }

        // 搜索关键词 (使用文本搜索)
        if let Some(search) = &query.search {
            if !search.trim().is_empty() {
                filter.insert("$text", doc! { "$search": search });
            }
        }

        // 项目状态过滤 (从extensions.project_state字段过滤)
        if let Some(project_state) = &query.project_state {
            filter.insert("extensions.project_state", mongodb::bson::to_bson(project_state)?);
        }

        // 创建者过滤 (从extensions.creator字段过滤)
        if let Some(creator) = &query.creator {
            if !creator.trim().is_empty() {
                filter.insert("extensions.creator", creator);
            }
        }

        // 地址过滤 (支持多个地址，用逗号分隔)
        if let Some(addresses_str) = &query.addresses {
            let addresses: Vec<String> = addresses_str
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty() && s.len() >= 32 && s.len() <= 44) // 验证地址长度
                .collect();

            if !addresses.is_empty() {
                filter.insert("address", doc! { "$in": addresses });
            }
        }

        // 参与者过滤 (根据钱包地址查询参与过的众筹代币)
        // 注意：此逻辑在service层实现，这里repository层不直接处理participate参数

        // 获取总数用于分页
        let total_count = self.collection.count_documents(filter.clone(), None).await?;

        // 构建排序文档 - 支持多字段排序
        let sort_params = query.parse_sort_params();
        let mut sort_doc = Document::new();

        for (field, direction) in sort_params {
            sort_doc.insert(field, direction);
        }

        // 如果没有任何有效的排序字段，使用默认排序
        if sort_doc.is_empty() {
            sort_doc.insert("created_at", -1);
        }

        // 计算分页参数
        let page = query.page.unwrap_or(1);
        let page_size = query.page_size.unwrap_or(100);
        let skip = (page - 1) * page_size;

        // 构建查询选项
        let options = FindOptions::builder()
            .sort(sort_doc)
            .skip(skip)
            .limit(page_size as i64)
            .build();

        // 执行查询
        let mut cursor = self.collection.find(filter, options).await?;
        let mut tokens = Vec::new();

        while cursor.advance().await? {
            tokens.push(cursor.deserialize_current()?);
        }

        // 构建响应
        let total_pages = if total_count == 0 {
            0
        } else {
            (total_count + page_size - 1) / page_size
        };

        let pagination = PaginationInfo {
            current_page: page,
            page_size,
            total_count,
            total_pages,
            has_next: page < total_pages,
            has_prev: page > 1,
        };

        // 构建黑名单和白名单
        let (blacklist, white_list) = self.build_lists(&tokens).await?;

        // 构建统计信息
        let stats = self.build_filter_stats().await?;

        // 转换为DTO格式
        let mint_list: Vec<StaticTokenInfo> = tokens.into_iter().map(|t| t.to_static_dto()).collect();

        Ok(TokenListResponse {
            mint_list,
            blacklist,
            white_list,
            pagination,
            stats,
        })
    }

    /// 构建黑名单和白名单
    async fn build_lists(&self, tokens: &[TokenInfo]) -> AppResult<(Vec<String>, Vec<String>)> {
        let blacklist: Vec<String> = tokens
            .iter()
            .filter(|t| t.is_blacklisted())
            .map(|t| t.address.clone())
            .collect();

        let white_list: Vec<String> = tokens
            .iter()
            .filter(|t| t.is_whitelisted())
            .map(|t| t.address.clone())
            .collect();

        Ok((blacklist, white_list))
    }

    /// 构建过滤器统计信息
    async fn build_filter_stats(&self) -> AppResult<FilterStats> {
        // 按状态统计
        let status_pipeline = vec![doc! {
            "$group": {
                "_id": "$status",
                "count": { "$sum": 1 }
            }
        }];

        let mut status_cursor = self.collection.aggregate(status_pipeline, None).await?;
        let mut status_counts = Vec::new();

        while status_cursor.advance().await? {
            let doc = status_cursor.current();
            if let (Ok(status_str), Ok(count)) = (doc.get_str("_id"), doc.get_i64("count")) {
                if let Ok(status) = serde_json::from_str::<TokenStatus>(&format!("\"{}\"", status_str)) {
                    status_counts.push(StatusCount {
                        status,
                        count: count as u64,
                    });
                }
            }
        }

        // 按数据来源统计
        let source_pipeline = vec![doc! {
            "$group": {
                "_id": "$source",
                "count": { "$sum": 1 }
            }
        }];

        let mut source_cursor = self.collection.aggregate(source_pipeline, None).await?;
        let mut source_counts = Vec::new();

        while source_cursor.advance().await? {
            let doc = source_cursor.current();
            if let (Ok(source_str), Ok(count)) = (doc.get_str("_id"), doc.get_i64("count")) {
                if let Ok(source) = serde_json::from_str::<DataSource>(&format!("\"{}\"", source_str)) {
                    source_counts.push(SourceCount {
                        source,
                        count: count as u64,
                    });
                }
            }
        }

        // 按验证状态统计
        let verification_pipeline = vec![doc! {
            "$group": {
                "_id": "$verification",
                "count": { "$sum": 1 }
            }
        }];

        let mut verification_cursor = self.collection.aggregate(verification_pipeline, None).await?;
        let mut verification_counts = Vec::new();

        while verification_cursor.advance().await? {
            let doc = verification_cursor.current();
            if let (Ok(verification_str), Ok(count)) = (doc.get_str("_id"), doc.get_i64("count")) {
                if let Ok(verification) =
                    serde_json::from_str::<VerificationStatus>(&format!("\"{}\"", verification_str))
                {
                    verification_counts.push(VerificationCount {
                        verification,
                        count: count as u64,
                    });
                }
            }
        }

        // 按标签统计 (Top 10)
        let tag_pipeline = vec![
            doc! { "$unwind": "$tags" },
            doc! {
                "$group": {
                    "_id": "$tags",
                    "count": { "$sum": 1 }
                }
            },
            doc! { "$sort": { "count": -1 } },
            doc! { "$limit": 10 },
        ];

        let mut tag_cursor = self.collection.aggregate(tag_pipeline, None).await?;
        let mut tag_counts = Vec::new();

        while tag_cursor.advance().await? {
            let doc = tag_cursor.current();
            if let (Ok(tag), Ok(count)) = (doc.get_str("_id"), doc.get_i64("count")) {
                tag_counts.push(TagCount {
                    tag: tag.to_string(),
                    count: count as u64,
                });
            }
        }

        Ok(FilterStats {
            status_counts,
            source_counts,
            verification_counts,
            tag_counts,
        })
    }

    /// 更新代币信息
    pub async fn update_token(&self, address: &str, update_doc: Document) -> AppResult<bool> {
        let filter = doc! { "address": address };
        let mut update = update_doc;
        update.insert("updated_at", mongodb::bson::to_bson(&Utc::now())?);

        let update_doc = doc! { "$set": update };
        let result = self.collection.update_one(filter, update_doc, None).await?;

        Ok(result.modified_count > 0)
    }

    /// 更新代币状态
    pub async fn update_token_status(&self, address: &str, status: TokenStatus) -> AppResult<bool> {
        let filter = doc! { "address": address };
        let update = doc! {
            "$set": {
                "status": mongodb::bson::to_bson(&status)?,
                "updated_at": mongodb::bson::to_bson(&Utc::now())?
            }
        };

        let result = self.collection.update_one(filter, update, None).await?;
        Ok(result.modified_count > 0)
    }

    /// 更新代币验证状态
    pub async fn update_token_verification(&self, address: &str, verification: VerificationStatus) -> AppResult<bool> {
        let filter = doc! { "address": address };
        let update = doc! {
            "$set": {
                "verification": mongodb::bson::to_bson(&verification)?,
                "updated_at": mongodb::bson::to_bson(&Utc::now())?
            }
        };

        let result = self.collection.update_one(filter, update, None).await?;
        Ok(result.modified_count > 0)
    }

    /// 批量更新代币交易量
    pub async fn batch_update_volumes(&self, volume_updates: &[(String, f64)]) -> AppResult<u64> {
        let mut updated_count = 0;

        for (address, volume) in volume_updates {
            let filter = doc! { "address": address };
            let update = doc! {
                "$set": {
                    "daily_volume": volume,
                    "updated_at": mongodb::bson::to_bson(&Utc::now())?
                }
            };

            let result = self.collection.update_one(filter, update, None).await?;
            if result.modified_count > 0 {
                updated_count += 1;
            }
        }

        Ok(updated_count)
    }

    /// 删除代币信息 (谨慎使用)
    pub async fn delete_token(&self, address: &str) -> AppResult<bool> {
        let filter = doc! { "address": address };
        let result = self.collection.delete_one(filter, None).await?;
        Ok(result.deleted_count > 0)
    }

    /// 获取代币统计信息
    pub async fn get_token_stats(&self) -> AppResult<TokenStats> {
        // 总代币数量
        let total_tokens = self.collection.count_documents(doc! {}, None).await?;

        // 活跃代币数量
        let active_tokens = self
            .collection
            .count_documents(doc! { "status": "active" }, None)
            .await?;

        // 已验证代币数量
        let verified_tokens = self
            .collection
            .count_documents(
                doc! {
                    "verification": {
                        "$in": ["verified", "community", "strict"]
                    }
                },
                None,
            )
            .await?;

        // 今日新增代币数量
        let today_start = Utc::now().date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc();
        let today_new_tokens = self
            .collection
            .count_documents(
                doc! {
                    "created_at": {
                        "$gte": mongodb::bson::to_bson(&today_start)?
                    }
                },
                None,
            )
            .await?;

        Ok(TokenStats {
            total_tokens,
            active_tokens,
            verified_tokens,
            today_new_tokens,
        })
    }

    /// 搜索代币 (支持模糊搜索)
    pub async fn search_tokens(&self, keyword: &str, limit: Option<i64>) -> AppResult<Vec<TokenInfo>> {
        if keyword.trim().is_empty() {
            return Ok(Vec::new());
        }

        // 优先使用文本搜索
        let text_filter = doc! {
            "$text": { "$search": keyword },
            "status": "active"
        };

        let text_options = FindOptions::builder()
            .sort(doc! {
                "score": { "$meta": "textScore" },
                "daily_volume": -1
            })
            .limit(limit.unwrap_or(20))
            .build();

        let mut cursor = self.collection.find(text_filter, text_options).await?;
        let mut tokens = Vec::new();

        while cursor.advance().await? {
            tokens.push(cursor.deserialize_current()?);
        }

        // 如果文本搜索结果不足，使用正则表达式补充搜索
        if tokens.len() < (limit.unwrap_or(20) as usize) {
            let remaining_limit = (limit.unwrap_or(20) as usize) - tokens.len();
            let existing_addresses: Vec<String> = tokens.iter().map(|t| t.address.clone()).collect();

            let regex_filter = doc! {
                "$and": [
                    {
                        "$or": [
                            { "name": { "$regex": keyword, "$options": "i" } },
                            { "symbol": { "$regex": keyword, "$options": "i" } },
                            { "address": { "$regex": keyword, "$options": "i" } }
                        ]
                    },
                    { "status": "active" },
                    { "address": { "$nin": existing_addresses } }
                ]
            };

            let regex_options = FindOptions::builder()
                .sort(doc! { "daily_volume": -1 })
                .limit(remaining_limit as i64)
                .build();

            let mut regex_cursor = self.collection.find(regex_filter, regex_options).await?;

            while regex_cursor.advance().await? {
                tokens.push(regex_cursor.deserialize_current()?);
            }
        }

        Ok(tokens)
    }

    /// 获取热门代币 (按交易量排序)
    pub async fn get_trending_tokens(&self, limit: Option<i64>) -> AppResult<Vec<TokenInfo>> {
        let filter = doc! {
            "status": "active",
            "daily_volume": { "$gt": 0.0 }
        };

        let options = FindOptions::builder()
            .sort(doc! { "daily_volume": -1 })
            .limit(limit.unwrap_or(50))
            .build();

        let mut cursor = self.collection.find(filter, options).await?;
        let mut tokens = Vec::new();

        while cursor.advance().await? {
            tokens.push(cursor.deserialize_current()?);
        }

        Ok(tokens)
    }

    /// 获取新上线代币 (按创建时间排序)
    pub async fn get_new_tokens(&self, limit: Option<i64>) -> AppResult<Vec<TokenInfo>> {
        let filter = doc! { "status": "active" };

        let options = FindOptions::builder()
            .sort(doc! { "created_at": -1 })
            .limit(limit.unwrap_or(50))
            .build();

        let mut cursor = self.collection.find(filter, options).await?;
        let mut tokens = Vec::new();

        while cursor.advance().await? {
            tokens.push(cursor.deserialize_current()?);
        }

        Ok(tokens)
    }

    /// 根据地址列表批量查询代币信息
    pub async fn find_by_addresses(&self, addresses: &[String]) -> AppResult<Vec<TokenInfo>> {
        if addresses.is_empty() {
            return Ok(Vec::new());
        }

        let filter = doc! {
            "address": {
                "$in": addresses
            }
        };

        let options = FindOptions::builder()
            .sort(doc! { "daily_volume": -1 }) // 按交易量降序排序
            .build();

        let mut cursor = self.collection.find(filter, options).await?;
        let mut tokens = Vec::new();

        while let Some(token) = cursor.try_next().await? {
            tokens.push(token);
        }

        Ok(tokens)
    }
}

/// 代币统计信息
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct TokenStats {
    /// 总代币数量
    pub total_tokens: u64,
    /// 活跃代币数量
    pub active_tokens: u64,
    /// 已验证代币数量
    pub verified_tokens: u64,
    /// 今日新增代币数量
    pub today_new_tokens: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use mongodb::{Client, Database};
    use tokio;

    // Helper function to create test database
    async fn setup_test_db() -> Database {
        let client = Client::with_uri_str("mongodb://localhost:27017")
            .await
            .expect("Failed to connect to MongoDB");
        let db_name = format!("test_token_info_{}", Utc::now().timestamp());
        client.database(&db_name)
    }

    // Helper function to create test token
    fn create_test_token(address: &str, symbol: &str, name: &str) -> TokenInfo {
        TokenInfo::new(
            address.to_string(),
            "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA".to_string(),
            name.to_string(),
            symbol.to_string(),
            9,
            "https://example.com/logo.png".to_string(),
        )
    }

    #[tokio::test]
    async fn test_push_token_create_new() {
        let db = setup_test_db().await;
        let collection = db.collection::<TokenInfo>("token_info");
        let repository = TokenInfoRepository::new(collection.clone());

        let request = TokenPushRequest {
            address: "So11111111111111111111111111111111111111112".to_string(),
            program_id: Some("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA".to_string()),
            name: "Wrapped SOL".to_string(),
            symbol: "WSOL".to_string(),
            decimals: 9,
            logo_uri: "https://example.com/wsol.png".to_string(),
            tags: Some(vec!["defi".to_string(), "wrapped".to_string()]),
            daily_volume: Some(1000000.0),
            freeze_authority: None,
            mint_authority: None,
            permanent_delegate: None,
            minted_at: None,
            extensions: None,
            source: Some(DataSource::ExternalPush),
        };

        let response = repository.push_token(request).await.unwrap();

        assert!(response.success);
        assert_eq!(response.operation, "created");
        assert_eq!(response.address, "So11111111111111111111111111111111111111112");

        // Verify token was created
        let token = repository
            .find_by_address("So11111111111111111111111111111111111111112")
            .await
            .unwrap();
        assert!(token.is_some());
        let token = token.unwrap();
        assert_eq!(token.symbol, "WSOL");
        assert_eq!(token.daily_volume, 1000000.0);

        // Cleanup
        db.drop(None).await.unwrap();
    }

    #[tokio::test]
    async fn test_push_token_update_existing() {
        let db = setup_test_db().await;
        let collection = db.collection::<TokenInfo>("token_info");
        let repository = TokenInfoRepository::new(collection.clone());

        // First create a token
        let mut token = create_test_token("So11111111111111111111111111111111111111112", "WSOL", "Wrapped SOL");
        token.daily_volume = 500000.0;
        collection.insert_one(&token, None).await.unwrap();

        // Now update it
        let request = TokenPushRequest {
            address: "So11111111111111111111111111111111111111112".to_string(),
            program_id: Some("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA".to_string()),
            name: "Wrapped Solana".to_string(), // Changed name
            symbol: "WSOL".to_string(),
            decimals: 9,
            logo_uri: "https://example.com/wsol-new.png".to_string(), // Changed logo
            tags: Some(vec!["defi".to_string(), "wrapped".to_string(), "updated".to_string()]),
            daily_volume: Some(2000000.0), // Changed volume
            freeze_authority: None,
            mint_authority: None,
            permanent_delegate: None,
            minted_at: None,
            extensions: None,
            source: Some(DataSource::ExternalPush),
        };

        let response = repository.push_token(request).await.unwrap();

        assert!(response.success);
        assert_eq!(response.operation, "updated");

        // Verify token was updated
        let updated_token = repository
            .find_by_address("So11111111111111111111111111111111111111112")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(updated_token.name, "Wrapped Solana");
        assert_eq!(updated_token.daily_volume, 2000000.0);
        assert!(updated_token.tags.contains(&"updated".to_string()));

        // Cleanup
        db.drop(None).await.unwrap();
    }

    #[tokio::test]
    async fn test_find_by_symbol() {
        let db = setup_test_db().await;
        let collection = db.collection::<TokenInfo>("token_info");
        let repository = TokenInfoRepository::new(collection.clone());

        // Create test tokens with same symbol
        let mut token1 = create_test_token("address1", "TEST", "Test Token 1");
        token1.daily_volume = 1000.0;
        let mut token2 = create_test_token("address2", "TEST", "Test Token 2");
        token2.daily_volume = 2000.0;
        let token3 = create_test_token("address3", "OTHER", "Other Token");

        collection.insert_many(&[token1, token2, token3], None).await.unwrap();

        let results = repository.find_by_symbol("TEST").await.unwrap();

        assert_eq!(results.len(), 2);
        // Should be sorted by daily_volume descending
        assert_eq!(results[0].address, "address2");
        assert_eq!(results[1].address, "address1");

        // Cleanup
        db.drop(None).await.unwrap();
    }

    #[tokio::test]
    async fn test_query_tokens_with_filters() {
        let db = setup_test_db().await;
        let collection = db.collection::<TokenInfo>("token_info");
        let repository = TokenInfoRepository::new(collection.clone());

        // Create test tokens with different properties
        let mut token1 = create_test_token("address1", "TOKEN1", "Token 1");
        token1.status = TokenStatus::Active;
        token1.verification = VerificationStatus::Verified;
        token1.tags = vec!["defi".to_string(), "gaming".to_string()];
        token1.daily_volume = 1000.0;

        let mut token2 = create_test_token("address2", "TOKEN2", "Token 2");
        token2.status = TokenStatus::Paused;
        token2.verification = VerificationStatus::Unverified;
        token2.tags = vec!["meme".to_string()];
        token2.daily_volume = 500.0;

        let mut token3 = create_test_token("address3", "TOKEN3", "Token 3");
        token3.status = TokenStatus::Active;
        token3.verification = VerificationStatus::Community;
        token3.tags = vec!["defi".to_string()];
        token3.daily_volume = 2000.0;

        collection.insert_many(&[token1, token2, token3], None).await.unwrap();

        // Test status filter
        let query = TokenListQuery {
            status: Some(TokenStatus::Active),
            ..Default::default()
        };

        let result = repository.query_tokens(&query).await.unwrap();
        assert_eq!(result.mint_list.len(), 2);

        // Test verification filter
        let query = TokenListQuery {
            verification: Some(VerificationStatus::Verified),
            ..Default::default()
        };

        let result = repository.query_tokens(&query).await.unwrap();
        assert_eq!(result.mint_list.len(), 1);
        assert_eq!(result.mint_list[0].address, "address1");

        // Test tags filter
        let query = TokenListQuery {
            tags: Some("defi".to_string()),
            ..Default::default()
        };

        let result = repository.query_tokens(&query).await.unwrap();
        assert_eq!(result.mint_list.len(), 2);

        // Test volume range filter
        let query = TokenListQuery {
            min_volume: Some(800.0),
            max_volume: Some(1500.0),
            ..Default::default()
        };

        let result = repository.query_tokens(&query).await.unwrap();
        assert_eq!(result.mint_list.len(), 1);
        assert_eq!(result.mint_list[0].address, "address1");

        // Cleanup
        db.drop(None).await.unwrap();
    }

    #[tokio::test]
    async fn test_search_tokens() {
        let db = setup_test_db().await;
        let collection = db.collection::<TokenInfo>("token_info");
        let repository = TokenInfoRepository::new(collection.clone());

        // Initialize indexes first
        repository.init_indexes().await.unwrap();

        // Create test tokens
        let token1 = create_test_token("So11111111111111111111111111111111111111112", "WSOL", "Wrapped SOL");
        let token2 = create_test_token("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v", "USDC", "USD Coin");
        let token3 = create_test_token("Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB", "USDT", "Tether USD");

        collection.insert_many(&[token1, token2, token3], None).await.unwrap();

        // Wait a bit for potential text indexing
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Test symbol search
        let results = repository.search_tokens("USDC", Some(10)).await.unwrap();
        assert!(!results.is_empty());

        // Test name search
        let results = repository.search_tokens("Wrapped", Some(10)).await.unwrap();
        assert!(!results.is_empty());

        // Test partial address search
        let results = repository.search_tokens("So111", Some(10)).await.unwrap();
        assert!(!results.is_empty());

        // Cleanup
        db.drop(None).await.unwrap();
    }

    #[tokio::test]
    async fn test_update_token_status() {
        let db = setup_test_db().await;
        let collection = db.collection::<TokenInfo>("token_info");
        let repository = TokenInfoRepository::new(collection.clone());

        // Create test token
        let token = create_test_token("address1", "TOKEN1", "Token 1");
        collection.insert_one(&token, None).await.unwrap();

        // Update status
        let result = repository
            .update_token_status("address1", TokenStatus::Paused)
            .await
            .unwrap();
        assert!(result);

        // Verify update
        let updated_token = repository.find_by_address("address1").await.unwrap().unwrap();
        assert_eq!(updated_token.status, TokenStatus::Paused);

        // Cleanup
        db.drop(None).await.unwrap();
    }

    #[tokio::test]
    async fn test_get_trending_tokens() {
        let db = setup_test_db().await;
        let collection = db.collection::<TokenInfo>("token_info");
        let repository = TokenInfoRepository::new(collection.clone());

        // Create test tokens with different volumes
        let mut token1 = create_test_token("address1", "TOKEN1", "Token 1");
        token1.daily_volume = 1000.0;
        let mut token2 = create_test_token("address2", "TOKEN2", "Token 2");
        token2.daily_volume = 3000.0;
        let mut token3 = create_test_token("address3", "TOKEN3", "Token 3");
        token3.daily_volume = 2000.0;
        let mut token4 = create_test_token("address4", "TOKEN4", "Token 4");
        token4.daily_volume = 0.0; // Should be excluded

        collection
            .insert_many([token1, token2, token3, token4], None)
            .await
            .unwrap();

        let trending = repository.get_trending_tokens(Some(10)).await.unwrap();

        assert_eq!(trending.len(), 3); // token4 excluded due to 0 volume
                                       // Should be sorted by volume descending
        assert_eq!(trending[0].address, "address2"); // 3000.0
        assert_eq!(trending[1].address, "address3"); // 2000.0
        assert_eq!(trending[2].address, "address1"); // 1000.0

        // Cleanup
        db.drop(None).await.unwrap();
    }
}
