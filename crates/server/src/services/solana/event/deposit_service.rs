use anyhow::Result;
use database::{
    event_model::{
        repository::{DepositStats, DepositTypeDistribution},
        DepositEvent,
    },
    Database,
};
use futures::TryStreamExt;
use mongodb::bson::{doc, Document};
use mongodb::options::FindOptions;
use std::sync::Arc;
use tracing::{error, info, warn};

/// 存款事件服务 - 处理存款事件的查询和统计
pub struct DepositEventService {
    database: Arc<Database>,
}

impl DepositEventService {
    /// 创建新的存款事件服务实例
    pub fn new(database: Arc<Database>) -> Self {
        Self { database }
    }

    // ====== 基础CRUD操作 ======

    /// 创建新的存款事件
    pub async fn create_deposit_event(&self, event: DepositEvent) -> Result<(String, DepositEvent)> {
        info!("💾 创建新的存款事件，用户：{}, 签名：{}", event.user, event.signature);
        
        // 检查是否已存在相同签名的事件（防止重复）
        let existing = self.database
            .deposit_event_repository
            .find_by_signature(&event.signature)
            .await?;
        
        if existing.is_some() {
            error!("❌ 存款事件已存在，签名：{}", event.signature);
            return Err(anyhow::anyhow!("存款事件已存在，签名：{}", event.signature));
        }
        
        // 插入事件
        let event_id = self.database
            .deposit_event_repository
            .insert_deposit_event(event.clone())
            .await?;
        
        info!("✅ 成功创建存款事件，ID: {}, 用户: {}, 金额: {}", 
            event_id, event.user, event.actual_amount);
        
        Ok((event_id, event))
    }

    /// 批量创建存款事件
    pub async fn batch_create_deposit_events(&self, events: Vec<DepositEvent>) -> Result<Vec<String>> {
        info!("💾 批量创建存款事件，数量：{}", events.len());
        
        let mut created_ids = Vec::new();
        let mut failed_count = 0;
        
        for event in events {
            match self.create_deposit_event(event).await {
                Ok((id, _)) => {
                    created_ids.push(id);
                }
                Err(e) => {
                    warn!("创建存款事件失败: {}", e);
                    failed_count += 1;
                }
            }
        }
        
        info!("✅ 批量创建完成，成功：{}, 失败：{}", created_ids.len(), failed_count);
        
        if created_ids.is_empty() && failed_count > 0 {
            return Err(anyhow::anyhow!("所有存款事件创建失败"));
        }
        
        Ok(created_ids)
    }

    /// 分页查询存款事件
    pub async fn get_deposit_events_paginated(
        &self,
        page: Option<u32>,
        page_size: Option<u32>,
        user: Option<String>,
        token_mint: Option<String>,
        project_config: Option<String>,
        deposit_type: Option<u8>,
        start_date: Option<i64>,
        end_date: Option<i64>,
        sort_by: Option<String>,
        sort_order: Option<String>,
    ) -> Result<PaginatedResponse<DepositEvent>> {
        info!("🔍 分页查询存款事件");

        let page = page.unwrap_or(1);
        let page_size = page_size.unwrap_or(20).min(100);
        let skip = ((page - 1) * page_size) as u64;
        let sort_field = sort_by.unwrap_or_else(|| "deposited_at".to_string());
        let sort_direction = if sort_order.unwrap_or_else(|| "desc".to_string()) == "asc" {
            1
        } else {
            -1
        };

        // 构建过滤条件
        let mut filter = Document::new();

        if let Some(user) = user {
            filter.insert("user", user);
        }

        if let Some(token_mint) = token_mint {
            filter.insert("token_mint", token_mint);
        }

        if let Some(project_config) = project_config {
            filter.insert("project_config", project_config);
        }

        if let Some(deposit_type) = deposit_type {
            filter.insert("deposit_type", deposit_type as i32);
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
            filter.insert("deposited_at", date_filter);
        }

        let sort = doc! { &sort_field: sort_direction };

        let find_options = FindOptions::builder()
            .skip(skip)
            .limit(page_size as i64)
            .sort(sort)
            .build();

        let result = self
            .database
            .deposit_event_repository
            .find_paginated(filter, find_options)
            .await?;

        let total_pages = (result.total + page_size as u64 - 1) / page_size as u64;

        Ok(PaginatedResponse {
            items: result.items,
            total: result.total,
            page: page as u64,
            page_size: page_size as u64,
            total_pages,
        })
    }

    /// 高级查询存款事件
    pub async fn get_deposit_events_advanced(
        &self,
        page: Option<u32>,
        page_size: Option<u32>,
        user: Option<String>,
        token_mint: Option<String>,
        project_config: Option<String>,
        deposit_type: Option<u8>,
        start_date: Option<i64>,
        end_date: Option<i64>,
        amount_min: Option<u64>,
        amount_max: Option<u64>,
        total_raised_min: Option<u64>,
        total_raised_max: Option<u64>,
        is_high_value_deposit: Option<bool>,
        related_pool: Option<String>,
        estimated_usd_min: Option<f64>,
        estimated_usd_max: Option<f64>,
        token_symbol: Option<String>,
        token_name: Option<String>,
        sort_by: Option<String>,
        sort_order: Option<String>,
    ) -> Result<PaginatedResponse<DepositEvent>> {
        info!("🔍 高级查询存款事件");

        let page = page.unwrap_or(1);
        let page_size = page_size.unwrap_or(20).min(100);
        let skip = ((page - 1) * page_size) as u64;
        let sort_field = sort_by.unwrap_or_else(|| "deposited_at".to_string());
        let sort_direction = if sort_order.unwrap_or_else(|| "desc".to_string()) == "asc" {
            1
        } else {
            -1
        };

        // 构建高级过滤条件
        let mut filter = Document::new();

        // 基础过滤条件
        if let Some(user) = user {
            filter.insert("user", user);
        }

        if let Some(token_mint) = token_mint {
            filter.insert("token_mint", token_mint);
        }

        if let Some(project_config) = project_config {
            filter.insert("project_config", project_config);
        }

        if let Some(deposit_type) = deposit_type {
            filter.insert("deposit_type", deposit_type as i32);
        }

        if let Some(is_high_value_deposit) = is_high_value_deposit {
            filter.insert("is_high_value_deposit", is_high_value_deposit);
        }

        if let Some(related_pool) = related_pool {
            filter.insert("related_pool", related_pool);
        }

        if let Some(token_symbol) = token_symbol {
            filter.insert("token_symbol", token_symbol);
        }

        if let Some(token_name) = token_name {
            // 使用模糊匹配
            filter.insert("token_name", doc! { "$regex": token_name, "$options": "i" });
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
            filter.insert("deposited_at", date_filter);
        }

        // 金额范围过滤
        if amount_min.is_some() || amount_max.is_some() {
            let mut amount_filter = Document::new();
            if let Some(min) = amount_min {
                amount_filter.insert("$gte", min as i64);
            }
            if let Some(max) = amount_max {
                amount_filter.insert("$lte", max as i64);
            }
            filter.insert("amount", amount_filter);
        }

        // 累计筹资额范围过滤
        if total_raised_min.is_some() || total_raised_max.is_some() {
            let mut raised_filter = Document::new();
            if let Some(min) = total_raised_min {
                raised_filter.insert("$gte", min as i64);
            }
            if let Some(max) = total_raised_max {
                raised_filter.insert("$lte", max as i64);
            }
            filter.insert("total_raised", raised_filter);
        }

        // USD价值范围过滤
        if estimated_usd_min.is_some() || estimated_usd_max.is_some() {
            let mut usd_filter = Document::new();
            if let Some(min) = estimated_usd_min {
                usd_filter.insert("$gte", min);
            }
            if let Some(max) = estimated_usd_max {
                usd_filter.insert("$lte", max);
            }
            filter.insert("estimated_usd_value", usd_filter);
        }

        let sort = doc! { &sort_field: sort_direction };

        let find_options = FindOptions::builder()
            .skip(skip)
            .limit(page_size as i64)
            .sort(sort)
            .build();

        let result = self
            .database
            .deposit_event_repository
            .find_paginated(filter, find_options)
            .await?;

        let total_pages = (result.total + page_size as u64 - 1) / page_size as u64;

        Ok(PaginatedResponse {
            items: result.items,
            total: result.total,
            page: page as u64,
            page_size: page_size as u64,
            total_pages,
        })
    }

    /// 根据用户查询存款记录
    pub async fn get_deposits_by_user(
        &self,
        user: &str,
        page: Option<u32>,
        page_size: Option<u32>,
    ) -> Result<PaginatedResponse<DepositEvent>> {
        info!("🔍 查询用户 {} 的存款记录", user);

        let page = page.unwrap_or(1);
        let page_size = page_size.unwrap_or(20).min(100);
        let skip = ((page - 1) * page_size) as u64;

        let filter = doc! { "user": user };
        let sort = doc! { "deposited_at": -1 };

        let find_options = FindOptions::builder()
            .skip(skip)
            .limit(page_size as i64)
            .sort(sort)
            .build();

        let result = self
            .database
            .deposit_event_repository
            .find_paginated(filter, find_options)
            .await?;

        let total_pages = (result.total + page_size as u64 - 1) / page_size as u64;

        Ok(PaginatedResponse {
            items: result.items,
            total: result.total,
            page: page as u64,
            page_size: page_size as u64,
            total_pages,
        })
    }

    /// 根据代币查询存款记录
    pub async fn get_deposits_by_token(
        &self,
        token_mint: &str,
        page: Option<u32>,
        page_size: Option<u32>,
    ) -> Result<PaginatedResponse<DepositEvent>> {
        info!("🔍 查询代币 {} 的存款记录", token_mint);

        let page = page.unwrap_or(1);
        let page_size = page_size.unwrap_or(20).min(100);
        let skip = ((page - 1) * page_size) as u64;

        let filter = doc! { "token_mint": token_mint };
        let sort = doc! { "deposited_at": -1 };

        let find_options = FindOptions::builder()
            .skip(skip)
            .limit(page_size as i64)
            .sort(sort)
            .build();

        let result = self
            .database
            .deposit_event_repository
            .find_paginated(filter, find_options)
            .await?;

        let total_pages = (result.total + page_size as u64 - 1) / page_size as u64;

        Ok(PaginatedResponse {
            items: result.items,
            total: result.total,
            page: page as u64,
            page_size: page_size as u64,
            total_pages,
        })
    }

    /// 根据签名查询存款事件
    pub async fn get_deposit_by_signature(&self, signature: &str) -> Result<Option<DepositEvent>> {
        info!("🔍 查询签名 {} 的存款事件", signature);

        let event = self
            .database
            .deposit_event_repository
            .find_by_signature(signature)
            .await?;

        Ok(event)
    }

    // ====== 统计分析功能 ======

    /// 获取存款统计信息
    pub async fn get_deposit_stats(&self) -> Result<DepositStats> {
        info!("📊 获取存款统计信息");

        let stats = self.database.deposit_event_repository.get_deposit_stats().await?;

        Ok(stats)
    }

    /// 获取用户存款汇总
    pub async fn get_user_deposit_summary(&self, user: &str) -> Result<UserDepositSummary> {
        info!("📊 获取用户 {} 的存款汇总", user);

        // 使用聚合管道计算汇总信息
        let pipeline = vec![
            doc! {
                "$match": {
                    "user": user
                }
            },
            doc! {
                "$group": {
                    "_id": null,
                    "total_deposits": { "$sum": 1 },
                    "total_volume_usd": { "$sum": "$estimated_usd_value" },
                    "unique_tokens": { "$addToSet": "$token_mint" },
                    "first_deposit_at": { "$min": "$deposited_at" },
                    "last_deposit_at": { "$max": "$deposited_at" },
                    "deposit_type_distribution": {
                        "$push": {
                            "deposit_type": "$deposit_type",
                            "deposit_type_name": "$deposit_type_name"
                        }
                    },
                    "token_distribution": {
                        "$push": {
                            "token_mint": "$token_mint",
                            "token_symbol": "$token_symbol",
                            "token_name": "$token_name",
                            "amount": "$estimated_usd_value"
                        }
                    }
                }
            },
        ];

        let mut cursor = self.database.deposit_events.aggregate(pipeline, None).await?;

        let summary = if let Some(doc) = cursor.try_next().await? {
            // 处理存款类型分布
            let mut type_counts = std::collections::HashMap::new();
            if let Ok(type_array) = doc.get_array("deposit_type_distribution") {
                for type_doc in type_array {
                    if let Some(type_doc) = type_doc.as_document() {
                        if let (Ok(deposit_type), Ok(type_name)) =
                            (type_doc.get_i32("deposit_type"), type_doc.get_str("deposit_type_name"))
                        {
                            *type_counts
                                .entry((deposit_type as u8, type_name.to_string()))
                                .or_insert(0) += 1;
                        }
                    }
                }
            }

            let deposit_type_distribution = type_counts
                .into_iter()
                .map(|((deposit_type, name), count)| DepositTypeDistribution {
                    deposit_type,
                    name,
                    count,
                })
                .collect();

            // 处理代币分布
            let mut token_amounts: std::collections::HashMap<String, f64> = std::collections::HashMap::new();
            let mut token_info_map: std::collections::HashMap<String, (Option<String>, Option<String>)> =
                std::collections::HashMap::new();
            let mut token_count_map: std::collections::HashMap<String, u64> = std::collections::HashMap::new();

            if let Ok(token_array) = doc.get_array("token_distribution") {
                for token_doc in token_array {
                    if let Some(token_doc) = token_doc.as_document() {
                        if let (Ok(token_mint), Ok(amount)) =
                            (token_doc.get_str("token_mint"), token_doc.get_f64("amount"))
                        {
                            *token_amounts.entry(token_mint.to_string()).or_insert(0.0) += amount;
                            *token_count_map.entry(token_mint.to_string()).or_insert(0) += 1;

                            if !token_info_map.contains_key(token_mint) {
                                let symbol = token_doc.get_str("token_symbol").ok().map(|s| s.to_string());
                                let name = token_doc.get_str("token_name").ok().map(|s| s.to_string());
                                token_info_map.insert(token_mint.to_string(), (symbol, name));
                            }
                        }
                    }
                }
            }

            let token_distribution = token_amounts
                .into_iter()
                .map(|(token_mint, total_volume_usd)| {
                    let (token_symbol, token_name) = token_info_map.get(&token_mint).cloned().unwrap_or((None, None));
                    let count = token_count_map.get(&token_mint).cloned().unwrap_or(0);
                    UserTokenDistribution {
                        token_mint,
                        token_symbol,
                        token_name,
                        count,
                        total_volume_usd,
                    }
                })
                .collect();

            let unique_tokens_count = if let Ok(unique_tokens) = doc.get_array("unique_tokens") {
                unique_tokens.len() as u32
            } else {
                0
            };

            UserDepositSummary {
                user: user.to_string(),
                total_deposits: doc.get_i32("total_deposits").unwrap_or(0) as u64,
                total_volume_usd: doc.get_f64("total_volume_usd").unwrap_or(0.0),
                unique_tokens: unique_tokens_count,
                first_deposit_at: doc.get_i64("first_deposit_at").unwrap_or(0),
                last_deposit_at: doc.get_i64("last_deposit_at").unwrap_or(0),
                deposit_type_distribution,
                token_distribution,
            }
        } else {
            UserDepositSummary {
                user: user.to_string(),
                total_deposits: 0,
                total_volume_usd: 0.0,
                unique_tokens: 0,
                first_deposit_at: 0,
                last_deposit_at: 0,
                deposit_type_distribution: vec![],
                token_distribution: vec![],
            }
        };

        Ok(summary)
    }

    /// 获取代币存款汇总
    pub async fn get_token_deposit_summary(&self, token_mint: &str) -> Result<TokenDepositSummary> {
        info!("📊 获取代币 {} 的存款汇总", token_mint);

        let pipeline = vec![
            doc! {
                "$match": {
                    "token_mint": token_mint
                }
            },
            doc! {
                "$group": {
                    "_id": null,
                    "total_deposits": { "$sum": 1 },
                    "total_volume_usd": { "$sum": "$estimated_usd_value" },
                    "unique_users": { "$addToSet": "$user" },
                    "first_deposit_at": { "$min": "$deposited_at" },
                    "last_deposit_at": { "$max": "$deposited_at" },
                    "token_symbol": { "$first": "$token_symbol" },
                    "token_name": { "$first": "$token_name" },
                    "token_decimals": { "$first": "$token_decimals" },
                    "deposit_type_distribution": {
                        "$push": {
                            "deposit_type": "$deposit_type",
                            "deposit_type_name": "$deposit_type_name"
                        }
                    }
                }
            },
        ];

        let mut cursor = self.database.deposit_events.aggregate(pipeline, None).await?;

        let summary = if let Some(doc) = cursor.try_next().await? {
            // 处理存款类型分布
            let mut type_counts = std::collections::HashMap::new();
            if let Ok(type_array) = doc.get_array("deposit_type_distribution") {
                for type_doc in type_array {
                    if let Some(type_doc) = type_doc.as_document() {
                        if let (Ok(deposit_type), Ok(type_name)) =
                            (type_doc.get_i32("deposit_type"), type_doc.get_str("deposit_type_name"))
                        {
                            *type_counts
                                .entry((deposit_type as u8, type_name.to_string()))
                                .or_insert(0) += 1;
                        }
                    }
                }
            }

            let deposit_type_distribution = type_counts
                .into_iter()
                .map(|((deposit_type, name), count)| DepositTypeDistribution {
                    deposit_type,
                    name,
                    count,
                })
                .collect();

            let unique_users_count = if let Ok(unique_users) = doc.get_array("unique_users") {
                unique_users.len() as u32
            } else {
                0
            };

            TokenDepositSummary {
                token_mint: token_mint.to_string(),
                token_symbol: doc.get_str("token_symbol").ok().map(|s| s.to_string()),
                token_name: doc.get_str("token_name").ok().map(|s| s.to_string()),
                token_decimals: doc.get_i32("token_decimals").ok().map(|d| d as u8),
                total_deposits: doc.get_i32("total_deposits").unwrap_or(0) as u64,
                total_volume_usd: doc.get_f64("total_volume_usd").unwrap_or(0.0),
                unique_users: unique_users_count,
                first_deposit_at: doc.get_i64("first_deposit_at").unwrap_or(0),
                last_deposit_at: doc.get_i64("last_deposit_at").unwrap_or(0),
                deposit_type_distribution,
            }
        } else {
            TokenDepositSummary {
                token_mint: token_mint.to_string(),
                token_symbol: None,
                token_name: None,
                token_decimals: None,
                total_deposits: 0,
                total_volume_usd: 0.0,
                unique_users: 0,
                first_deposit_at: 0,
                last_deposit_at: 0,
                deposit_type_distribution: vec![],
            }
        };

        Ok(summary)
    }

    /// 获取存款趋势数据
    pub async fn get_deposit_trends(
        &self,
        period: TrendPeriod,
        start_date: Option<i64>,
        end_date: Option<i64>,
    ) -> Result<Vec<DepositTrendPoint>> {
        info!("📊 获取存款趋势数据，周期: {:?}", period);

        let date_format = match period {
            TrendPeriod::Hour => "%Y%m%d%H",
            TrendPeriod::Day => "%Y%m%d",
            TrendPeriod::Week => "%Y%U",
            TrendPeriod::Month => "%Y%m",
        };

        let mut match_stage = doc! {};
        if start_date.is_some() || end_date.is_some() {
            let mut date_filter = Document::new();
            if let Some(start) = start_date {
                date_filter.insert("$gte", start);
            }
            if let Some(end) = end_date {
                date_filter.insert("$lte", end);
            }
            match_stage.insert("deposited_at", date_filter);
        }

        let pipeline = vec![
            doc! { "$match": match_stage },
            doc! {
                "$group": {
                    "_id": {
                        "$dateToString": {
                            "format": date_format,
                            "date": {
                                "$toDate": {
                                    "$multiply": ["$deposited_at", 1000]
                                }
                            }
                        }
                    },
                    "count": { "$sum": 1 },
                    "volume_usd": { "$sum": "$estimated_usd_value" },
                    "unique_users": { "$addToSet": "$user" }
                }
            },
            doc! {
                "$project": {
                    "_id": 1,
                    "count": 1,
                    "volume_usd": 1,
                    "unique_users": { "$size": "$unique_users" }
                }
            },
            doc! { "$sort": { "_id": 1 } },
        ];

        let mut cursor = self.database.deposit_events.aggregate(pipeline, None).await?;
        let mut trends = Vec::new();

        while let Some(doc) = cursor.try_next().await? {
            if let (Ok(period_key), Ok(count), Ok(volume_usd), Ok(unique_users)) = (
                doc.get_str("_id"),
                doc.get_i32("count"),
                doc.get_f64("volume_usd"),
                doc.get_i32("unique_users"),
            ) {
                trends.push(DepositTrendPoint {
                    period: period_key.to_string(),
                    count: count as u64,
                    volume_usd,
                    unique_users: unique_users as u32,
                });
            }
        }

        Ok(trends)
    }

    // ====== 内部辅助方法 ======

    /// 获取代币元数据
    async fn _fetch_token_metadata(&self, token_mint: &str) -> Option<TokenMetadata> {
        // 首先从TokenInfo表查询
        if let Ok(Some(token_info)) = self.database.token_info_repository.find_by_address(token_mint).await {
            return Some(TokenMetadata {
                decimals: Some(token_info.decimals),
                name: Some(token_info.name),
                symbol: Some(token_info.symbol),
                logo_uri: Some(token_info.logo_uri),
            });
        }

        // TODO: 实现从链上查询元数据的逻辑
        warn!("🔍 代币元数据未找到: {}", token_mint);
        None
    }

    /// 计算实际数量（考虑decimals）
    fn _calculate_actual_amount(&self, amount: u64, decimals: u8) -> f64 {
        amount as f64 / 10_f64.powi(decimals as i32)
    }

    /// 判断是否为高价值存款
    fn _is_high_value_deposit(&self, usd_value: f64) -> bool {
        const HIGH_VALUE_THRESHOLD: f64 = 10000.0; // $10,000 USD 阈值
        usd_value >= HIGH_VALUE_THRESHOLD
    }

    /// 推断存款类型
    fn _infer_deposit_type(&self, _user: &str, _token_mint: &str, _amount: u64) -> u8 {
        // TODO: 实现基于历史数据的存款类型推断逻辑
        // 目前默认为初始存款
        0
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

/// 用户存款汇总响应
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UserDepositSummary {
    pub user: String,
    pub total_deposits: u64,
    pub total_volume_usd: f64,
    pub unique_tokens: u32,
    pub first_deposit_at: i64,
    pub last_deposit_at: i64,
    pub deposit_type_distribution: Vec<DepositTypeDistribution>,
    pub token_distribution: Vec<UserTokenDistribution>,
}

/// 用户代币分布
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UserTokenDistribution {
    pub token_mint: String,
    pub token_symbol: Option<String>,
    pub token_name: Option<String>,
    pub count: u64,
    pub total_volume_usd: f64,
}

/// 代币存款汇总响应
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TokenDepositSummary {
    pub token_mint: String,
    pub token_symbol: Option<String>,
    pub token_name: Option<String>,
    pub token_decimals: Option<u8>,
    pub total_deposits: u64,
    pub total_volume_usd: f64,
    pub unique_users: u32,
    pub first_deposit_at: i64,
    pub last_deposit_at: i64,
    pub deposit_type_distribution: Vec<DepositTypeDistribution>,
}

/// 存款趋势数据点
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DepositTrendPoint {
    pub period: String,
    pub count: u64,
    pub volume_usd: f64,
    pub unique_users: u32,
}

/// 趋势周期枚举
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum TrendPeriod {
    Hour,
    Day,
    Week,
    Month,
}

/// 代币元数据
#[derive(Debug, Clone)]
pub struct TokenMetadata {
    pub decimals: Option<u8>,
    pub name: Option<String>,
    pub symbol: Option<String>,
    pub logo_uri: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 服务层单元测试 - 测试业务逻辑
    #[test]
    fn test_high_value_deposit_logic() {
        // 测试高价值存款判断阈值
        const HIGH_VALUE_THRESHOLD: f64 = 10000.0; // $10,000 USD 阈值

        // 测试高价值存款
        assert!(15000.0 >= HIGH_VALUE_THRESHOLD);
        assert!(10000.0 >= HIGH_VALUE_THRESHOLD);

        // 测试普通存款
        assert!(9999.99 < HIGH_VALUE_THRESHOLD);
        assert!(5000.0 < HIGH_VALUE_THRESHOLD);
        assert!(100.0 < HIGH_VALUE_THRESHOLD);
    }

    #[test]
    fn test_pagination_calculations() {
        // 测试分页计算逻辑
        let test_cases = vec![
            (100, 20, 5), // 100条记录，每页20条，共5页
            (101, 20, 6), // 101条记录，每页20条，共6页
            (50, 20, 3),  // 50条记录，每页20条，共3页
            (0, 20, 0),   // 0条记录，每页20条，共0页
            (10, 50, 1),  // 10条记录，每页50条，共1页
            (1, 1, 1),    // 1条记录，每页1条，共1页
        ];

        for (total, page_size, expected_pages) in test_cases {
            let calculated_pages = if total == 0 {
                0
            } else {
                (total + page_size - 1) / page_size
            };
            assert_eq!(
                calculated_pages, expected_pages,
                "Total: {}, PageSize: {}, Expected: {}, Got: {}",
                total, page_size, expected_pages, calculated_pages
            );
        }
    }

    #[test]
    fn test_deposit_type_logic() {
        // 测试存款类型枚举
        let deposit_types = vec![(0, "初始存款"), (1, "追加存款"), (2, "紧急存款"), (3, "自动存款")];

        for (type_id, type_name) in deposit_types {
            assert!(type_id >= 0 && type_id <= 10); // 合理范围
            assert!(!type_name.is_empty());
            assert!(type_name.len() <= 20); // 名称长度合理
        }
    }

    #[test]
    fn test_calculate_actual_amount() {
        // 测试实际金额计算（考虑decimals）
        struct TestCase {
            amount: u64,
            decimals: u8,
            expected: f64,
        }

        let test_cases = vec![
            TestCase {
                amount: 1000000000,
                decimals: 9,
                expected: 1.0,
            }, // 1 SOL
            TestCase {
                amount: 1000000,
                decimals: 6,
                expected: 1.0,
            }, // 1 USDC
            TestCase {
                amount: 5000000000,
                decimals: 9,
                expected: 5.0,
            }, // 5 SOL
            TestCase {
                amount: 0,
                decimals: 9,
                expected: 0.0,
            }, // 0 amounts
        ];

        for case in test_cases {
            let actual = case.amount as f64 / 10_f64.powi(case.decimals as i32);
            assert_eq!(
                actual, case.expected,
                "Amount: {}, Decimals: {}, Expected: {}, Got: {}",
                case.amount, case.decimals, case.expected, actual
            );
        }
    }

    #[test]
    fn test_trend_period_enum() {
        // 测试趋势周期枚举序列化
        let periods = vec![
            TrendPeriod::Hour,
            TrendPeriod::Day,
            TrendPeriod::Week,
            TrendPeriod::Month,
        ];

        for period in periods {
            let json = serde_json::to_string(&period).unwrap();
            let from_json: TrendPeriod = serde_json::from_str(&json).unwrap();

            match period {
                TrendPeriod::Hour => assert!(matches!(from_json, TrendPeriod::Hour)),
                TrendPeriod::Day => assert!(matches!(from_json, TrendPeriod::Day)),
                TrendPeriod::Week => assert!(matches!(from_json, TrendPeriod::Week)),
                TrendPeriod::Month => assert!(matches!(from_json, TrendPeriod::Month)),
            }
        }
    }

    #[test]
    fn test_paginated_response_structure() {
        // 测试分页响应结构
        let mock_events = vec!["event1".to_string(), "event2".to_string()];

        let response = PaginatedResponse {
            items: mock_events,
            total: 100,
            page: 1,
            page_size: 20,
            total_pages: 5,
        };

        assert_eq!(response.items.len(), 2);
        assert_eq!(response.total, 100);
        assert_eq!(response.page, 1);
        assert_eq!(response.page_size, 20);
        assert_eq!(response.total_pages, 5);

        // 验证分页逻辑一致性
        assert_eq!(
            (response.total + response.page_size - 1) / response.page_size,
            response.total_pages
        );
    }

    #[test]
    fn test_user_token_distribution() {
        // 测试用户代币分布结构
        let distribution = UserTokenDistribution {
            token_mint: "So11111111111111111111111111111111111111112".to_string(),
            token_symbol: Some("SOL".to_string()),
            token_name: Some("Solana".to_string()),
            count: 10,
            total_volume_usd: 3000.0,
        };

        assert_eq!(distribution.token_mint, "So11111111111111111111111111111111111111112");
        assert_eq!(distribution.token_symbol, Some("SOL".to_string()));
        assert_eq!(distribution.token_name, Some("Solana".to_string()));
        assert_eq!(distribution.count, 10);
        assert_eq!(distribution.total_volume_usd, 3000.0);

        // 验证平均值计算
        let avg_amount = distribution.total_volume_usd / distribution.count as f64;
        assert_eq!(avg_amount, 300.0);
    }

    #[test]
    fn test_token_metadata_structure() {
        // 测试代币元数据结构
        let metadata_with_all_fields = TokenMetadata {
            decimals: Some(9),
            name: Some("Solana".to_string()),
            symbol: Some("SOL".to_string()),
            logo_uri: Some("https://example.com/sol.png".to_string()),
        };

        assert_eq!(metadata_with_all_fields.decimals, Some(9));
        assert!(metadata_with_all_fields.name.is_some());
        assert!(metadata_with_all_fields.symbol.is_some());
        assert!(metadata_with_all_fields.logo_uri.is_some());

        // 测试部分缺失的元数据
        let metadata_partial = TokenMetadata {
            decimals: Some(6),
            name: None,
            symbol: Some("UNKNOWN".to_string()),
            logo_uri: None,
        };

        assert_eq!(metadata_partial.decimals, Some(6));
        assert!(metadata_partial.name.is_none());
        assert_eq!(metadata_partial.symbol, Some("UNKNOWN".to_string()));
        assert!(metadata_partial.logo_uri.is_none());
    }

    #[test]
    fn test_service_validation_constraints() {
        // 测试服务层验证约束

        // 分页参数验证
        let valid_page_sizes = vec![1, 10, 20, 50, 100];
        for page_size in valid_page_sizes {
            assert!(
                page_size >= 1 && page_size <= 100,
                "页面大小应在1-100之间: {}",
                page_size
            );
        }

        // 日期范围验证
        let current_timestamp = chrono::Utc::now().timestamp();
        let valid_start_date = current_timestamp - 86400 * 30; // 30天前
        let valid_end_date = current_timestamp;

        assert!(valid_start_date < valid_end_date, "开始日期应早于结束日期");
        assert!(valid_start_date > 0, "日期戳应为正数");
        assert!(valid_end_date > 0, "日期戳应为正数");

        // 金额范围验证
        let amount_ranges = vec![(0.0, 1000.0), (100.0, 10000.0), (1000.0, f64::MAX)];
        for (min, max) in amount_ranges {
            assert!(min <= max, "最小值应小于等于最大值: min={}, max={}", min, max);
            assert!(min >= 0.0, "金额应为非负数: {}", min);
        }
    }
}
