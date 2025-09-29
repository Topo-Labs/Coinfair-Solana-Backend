use crate::dtos::solana::cpmm::pool::init_pool_event::{
    CreateInitPoolEventRequest, InitPoolEventResponse, InitPoolEventsPageResponse, QueryInitPoolEventsRequest,
    UserPoolStats,
};
use crate::services::solana::cpmm::init_pool_event::init_pool_event_error::InitPoolEventError;
use anyhow::Result;
use database::cpmm::init_pool_event::model::InitPoolEvent;
use database::Database;
use mongodb::bson::{doc, oid::ObjectId, Document};
use mongodb::options::FindOptions;
use std::str::FromStr;
use std::sync::Arc;
use tracing::{debug, info, warn};

pub struct InitPoolEventService {
    db: Arc<Database>,
}

impl InitPoolEventService {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    pub async fn create_event(&self, request: CreateInitPoolEventRequest) -> Result<InitPoolEventResponse> {
        info!("🏗️ 创建池子初始化事件: pool_id={}", request.pool_id);

        // 检查pool_id是否已存在
        if let Ok(Some(_)) = self
            .db
            .init_pool_event_repository
            .find_by_pool_id(&request.pool_id)
            .await
        {
            warn!("⚠️ 池子已存在: {}", request.pool_id);
            return Err(InitPoolEventError::DuplicatePoolId(request.pool_id).into());
        }

        // 检查signature是否已存在
        if let Ok(Some(_)) = self
            .db
            .init_pool_event_repository
            .find_by_signature(&request.signature)
            .await
        {
            warn!("⚠️ 事件signature已存在: {}", request.signature);
            return Err(InitPoolEventError::DuplicateSignature(request.signature).into());
        }

        let event: InitPoolEvent = request.into();
        let created_event = self.db.init_pool_event_repository.insert(event).await?;

        info!("✅ 池子初始化事件创建成功: pool_id={}", created_event.pool_id);
        Ok(created_event.into())
    }

    pub async fn get_event_by_id(&self, id: &str) -> Result<InitPoolEventResponse> {
        debug!("🔍 根据ID查询池子初始化事件: {}", id);

        let object_id = ObjectId::from_str(id).map_err(|_| InitPoolEventError::EventNotFound)?;

        let event = self
            .db
            .init_pool_event_repository
            .find_by_id(&object_id)
            .await?
            .ok_or(InitPoolEventError::EventNotFound)?;

        Ok(event.into())
    }

    pub async fn get_event_by_pool_id(&self, pool_id: &str) -> Result<InitPoolEventResponse> {
        debug!("🔍 根据pool_id查询池子初始化事件: {}", pool_id);

        let event = self
            .db
            .init_pool_event_repository
            .find_by_pool_id(pool_id)
            .await?
            .ok_or(InitPoolEventError::EventNotFound)?;

        Ok(event.into())
    }

    pub async fn get_event_by_signature(&self, signature: &str) -> Result<InitPoolEventResponse> {
        debug!("🔍 根据signature查询池子初始化事件: {}", signature);

        let event = self
            .db
            .init_pool_event_repository
            .find_by_signature(signature)
            .await?
            .ok_or(InitPoolEventError::EventNotFound)?;

        Ok(event.into())
    }

    pub async fn query_events(&self, request: QueryInitPoolEventsRequest) -> Result<InitPoolEventsPageResponse> {
        debug!("🔍 查询池子初始化事件列表");

        let mut filter = Document::new();

        // 处理多个pool_id（英文逗号分隔）
        if let Some(pool_ids) = &request.pool_ids {
            let ids: Vec<String> = pool_ids
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();

            if !ids.is_empty() {
                filter.insert("pool_id", doc! { "$in": ids });
            }
        }

        // 根据池子创建者过滤
        if let Some(pool_creator) = &request.pool_creator {
            filter.insert("pool_creator", pool_creator);
        }

        // 根据LP mint过滤
        if let Some(lp_mint) = &request.lp_mint {
            filter.insert("lp_mint", lp_mint);
        }

        // 根据token_0_mint过滤
        if let Some(token_0_mint) = &request.token_0_mint {
            filter.insert("token_0_mint", token_0_mint);
        }

        // 根据token_1_mint过滤
        if let Some(token_1_mint) = &request.token_1_mint {
            filter.insert("token_1_mint", token_1_mint);
        }

        // 时间范围过滤
        if request.start_time.is_some() || request.end_time.is_some() {
            let mut time_filter = Document::new();
            if let Some(start) = request.start_time {
                // 将 chrono::DateTime 转换为 BSON DateTime
                let bson_datetime = mongodb::bson::DateTime::from_system_time(start.into());
                time_filter.insert("$gte", bson_datetime);
            }
            if let Some(end) = request.end_time {
                // 将 chrono::DateTime 转换为 BSON DateTime
                let bson_datetime = mongodb::bson::DateTime::from_system_time(end.into());
                time_filter.insert("$lte", bson_datetime);
            }
            filter.insert("created_at", time_filter);
        }

        // 分页参数
        let page = request.page.unwrap_or(1).max(1);
        let page_size = request.page_size.unwrap_or(20).min(100);
        let skip = (page - 1) * page_size;

        let options = FindOptions::builder()
            .sort(doc! { "created_at": -1 })
            .skip(skip)
            .limit(page_size as i64)
            .build();

        // 查询数据和总数
        let events = self
            .db
            .init_pool_event_repository
            .find_with_filter(filter.clone(), options)
            .await?;

        let total = self.db.init_pool_event_repository.count_with_filter(filter).await?;

        let total_pages = (total + page_size - 1) / page_size;

        let response_events: Vec<InitPoolEventResponse> = events.into_iter().map(|event| event.into()).collect();

        Ok(InitPoolEventsPageResponse {
            data: response_events,
            total,
            page,
            page_size,
            total_pages,
        })
    }

    pub async fn get_user_pool_stats(&self, pool_creator: &str) -> Result<UserPoolStats> {
        debug!("📊 获取用户池子创建统计: {}", pool_creator);

        // 使用Repository层的聚合查询方法，一次查询获取所有统计数据
        let stats = self
            .db
            .init_pool_event_repository
            .get_user_pool_stats(pool_creator)
            .await?;

        // 转换为Service层的UserPoolStats（注意这里需要类型转换）
        Ok(UserPoolStats {
            total_pools_created: stats.total_pools_created,
            first_pool_created_at: stats.first_pool_created_at,
            latest_pool_created_at: stats.latest_pool_created_at,
        })
    }

    pub async fn delete_event(&self, id: &str) -> Result<bool> {
        info!("🗑️ 删除池子初始化事件: {}", id);

        let object_id = ObjectId::from_str(id).map_err(|_| InitPoolEventError::EventNotFound)?;

        let deleted = self.db.init_pool_event_repository.delete_by_id(&object_id).await?;

        if deleted {
            info!("✅ 池子初始化事件删除成功: {}", id);
        } else {
            warn!("⚠️ 池子初始化事件不存在: {}", id);
        }

        Ok(deleted)
    }
}
