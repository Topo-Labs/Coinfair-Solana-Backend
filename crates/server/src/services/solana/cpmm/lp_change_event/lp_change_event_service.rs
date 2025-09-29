use crate::dtos::solana::cpmm::lp::lp_change_event::{
    CreateLpChangeEventRequest, LpChangeEventResponse, LpChangeEventsPageResponse, QueryLpChangeEventsRequest,
};
use crate::services::solana::cpmm::lp_change_event::lp_change_event_error::LpChangeEventError;
use anyhow::Result;
use database::cpmm::lp_change_event::{model::LpChangeEvent, repository::LpChangeEventRepository};
use database::Database;
use mongodb::bson::oid::ObjectId;
use mongodb::bson::{doc, Document};
use mongodb::options::FindOptions;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

/// LP变更事件服务
#[derive(Clone, Debug)]
pub struct LpChangeEventService {
    database: Arc<Database>,
}

impl LpChangeEventService {
    /// 创建新的服务实例
    pub fn new(database: Arc<Database>) -> Self {
        Self { database }
    }

    /// 获取Repository实例
    fn get_repository(&self) -> &LpChangeEventRepository {
        &self.database.lp_change_event_repository
    }

    /// 创建新的LP变更事件
    pub async fn create_event(
        &self,
        request: CreateLpChangeEventRequest,
    ) -> Result<LpChangeEventResponse, LpChangeEventError> {
        info!("创建LP变更事件，signature: {}", request.signature);

        // 检查事件是否已存在（通过signature去重）
        match self.get_repository().find_by_signature(&request.signature).await {
            Ok(Some(_)) => {
                warn!("事件已存在，signature: {}", request.signature);
                return Err(LpChangeEventError::EventAlreadyExists(format!(
                    "signature: {}",
                    request.signature
                )));
            }
            Ok(None) => {
                debug!("signature检查通过，可以创建事件");
            }
            Err(e) => {
                error!("检查事件是否存在时发生错误: {}", e);
                return Err(LpChangeEventError::DatabaseError(e));
            }
        }

        // 转换为数据模型
        let event_model = request.to_model();
        let signature = event_model.signature.clone(); // 保存signature用于错误消息

        // 插入事件
        match self.get_repository().insert(event_model).await {
            Ok(created_event) => {
                info!("LP变更事件创建成功，ID: {:?}", created_event.id);
                Ok(LpChangeEventResponse::from(created_event))
            }
            Err(e) => {
                error!("创建LP变更事件失败: {}", e);
                if e.to_string().contains("duplicate key") {
                    Err(LpChangeEventError::EventAlreadyExists(format!(
                        "signature: {}",
                        signature
                    )))
                } else {
                    Err(LpChangeEventError::DatabaseError(e))
                }
            }
        }
    }

    /// 根据ID获取事件
    pub async fn get_event_by_id(&self, id: &str) -> Result<LpChangeEventResponse, LpChangeEventError> {
        debug!("根据ID查询事件: {}", id);

        // 解析ObjectId
        let object_id = ObjectId::parse_str(id)
            .map_err(|_| LpChangeEventError::ValidationError(format!("无效的事件ID格式: {}", id)))?;

        // 查询事件
        match self.get_repository().find_by_id(&object_id).await {
            Ok(Some(event)) => {
                debug!("事件查询成功，ID: {}", id);
                Ok(LpChangeEventResponse::from(event))
            }
            Ok(None) => {
                debug!("事件未找到，ID: {}", id);
                Err(LpChangeEventError::EventNotFound(id.to_string()))
            }
            Err(e) => {
                error!("查询事件失败: {}", e);
                Err(LpChangeEventError::DatabaseError(e))
            }
        }
    }

    /// 根据signature获取事件
    pub async fn get_event_by_signature(&self, signature: &str) -> Result<LpChangeEventResponse, LpChangeEventError> {
        debug!("根据signature查询事件: {}", signature);

        if signature.is_empty() {
            return Err(LpChangeEventError::ValidationError("signature不能为空".to_string()));
        }

        match self.get_repository().find_by_signature(signature).await {
            Ok(Some(event)) => {
                debug!("事件查询成功，signature: {}", signature);
                Ok(LpChangeEventResponse::from(event))
            }
            Ok(None) => {
                debug!("事件未找到，signature: {}", signature);
                Err(LpChangeEventError::EventNotFound(signature.to_string()))
            }
            Err(e) => {
                error!("查询事件失败: {}", e);
                Err(LpChangeEventError::DatabaseError(e))
            }
        }
    }

    /// 分页查询事件
    pub async fn query_events(
        &self,
        request: QueryLpChangeEventsRequest,
    ) -> Result<LpChangeEventsPageResponse, LpChangeEventError> {
        info!("分页查询LP变更事件，参数: {:?}", request);

        // 验证分页参数
        let page = request.get_page();
        let page_size = request.get_page_size();
        let skip = request.get_skip();

        if page_size > 100 {
            return Err(LpChangeEventError::PaginationError("每页大小不能超过100".to_string()));
        }

        // 构建查询过滤器
        let filter = self.build_query_filter(&request)?;

        // 构建查询选项
        let find_options = FindOptions::builder()
            .sort(doc! { "created_at": -1 }) // 按创建时间倒序
            .skip(skip)
            .limit(page_size as i64)
            .build();

        // 执行查询
        let events_result = self
            .get_repository()
            .find_with_filter(filter.clone(), find_options)
            .await;
        let total_result = self.get_repository().count_with_filter(filter).await;

        match (events_result, total_result) {
            (Ok(events), Ok(total)) => {
                debug!("查询成功，返回{}条记录，总计{}条", events.len(), total);
                Ok(LpChangeEventsPageResponse::new(events, total, page, page_size))
            }
            (Err(e), _) | (_, Err(e)) => {
                error!("查询事件失败: {}", e);
                Err(LpChangeEventError::DatabaseError(e))
            }
        }
    }

    /// 删除事件（管理员功能）
    pub async fn delete_event(&self, id: &str) -> Result<bool, LpChangeEventError> {
        info!("删除LP变更事件，ID: {}", id);

        // 解析ObjectId
        let object_id = ObjectId::parse_str(id)
            .map_err(|_| LpChangeEventError::ValidationError(format!("无效的事件ID格式: {}", id)))?;

        // 检查事件是否存在
        match self.get_repository().find_by_id(&object_id).await {
            Ok(Some(_)) => {
                debug!("事件存在，可以删除");
            }
            Ok(None) => {
                return Err(LpChangeEventError::EventNotFound(id.to_string()));
            }
            Err(e) => {
                error!("检查事件是否存在时发生错误: {}", e);
                return Err(LpChangeEventError::DatabaseError(e));
            }
        }

        // 执行删除
        match self.get_repository().delete_by_id(&object_id).await {
            Ok(deleted) => {
                if deleted {
                    info!("事件删除成功，ID: {}", id);
                    Ok(true)
                } else {
                    warn!("事件删除失败（可能已被删除），ID: {}", id);
                    Err(LpChangeEventError::EventNotFound(id.to_string()))
                }
            }
            Err(e) => {
                error!("删除事件失败: {}", e);
                Err(LpChangeEventError::DatabaseError(e))
            }
        }
    }

    /// 批量创建事件（供事件监听器使用）
    pub async fn bulk_create_events(&self, events: Vec<LpChangeEvent>) -> Result<usize, LpChangeEventError> {
        info!("批量创建LP变更事件，数量: {}", events.len());

        if events.is_empty() {
            return Ok(0);
        }

        match self.get_repository().bulk_insert(events).await {
            Ok(inserted_count) => {
                info!("批量创建事件成功，插入{}条记录", inserted_count);
                Ok(inserted_count)
            }
            Err(e) => {
                error!("批量创建事件失败: {}", e);
                Err(LpChangeEventError::DatabaseError(e))
            }
        }
    }

    /// 根据多个LP mint查询事件列表（用于新的query-lp-mint接口）
    pub async fn query_events_by_lp_mints(
        &self,
        lp_mints: Vec<String>,
        limit: Option<i64>,
    ) -> Result<Vec<LpChangeEvent>, LpChangeEventError> {
        info!("根据LP mints查询事件，mint数量: {}, 限制: {:?}", lp_mints.len(), limit);

        if lp_mints.is_empty() {
            return Ok(vec![]);
        }

        // 限制一次查询的LP mint数量，防止过大查询
        if lp_mints.len() > 100 {
            return Err(LpChangeEventError::QueryParameterError(
                "一次查询的LP mint数量不能超过100个".to_string(),
            ));
        }

        match self.get_repository().find_by_lp_mints(lp_mints, limit).await {
            Ok(events) => {
                info!("根据LP mints查询事件成功，返回{}条记录", events.len());
                Ok(events)
            }
            Err(e) => {
                error!("根据LP mints查询事件失败: {}", e);
                Err(LpChangeEventError::DatabaseError(e))
            }
        }
    }

    /// 构建查询过滤器
    fn build_query_filter(&self, request: &QueryLpChangeEventsRequest) -> Result<Document, LpChangeEventError> {
        let mut filter = doc! {};

        // 用户钱包过滤
        if let Some(user_wallet) = &request.user_wallet {
            if !user_wallet.is_empty() {
                filter.insert("user_wallet", user_wallet);
            }
        }

        // 池子ID过滤
        if let Some(pool_id) = &request.pool_id {
            if !pool_id.is_empty() {
                filter.insert("pool_id", pool_id);
            }
        }

        // LP mint过滤（支持多个）
        if let Some(lp_mints) = request.parse_lp_mints() {
            if !lp_mints.is_empty() {
                if lp_mints.len() == 1 {
                    filter.insert("lp_mint", &lp_mints[0]);
                } else {
                    filter.insert("lp_mint", doc! { "$in": lp_mints });
                }
            }
        }

        // 变更类型过滤
        if let Some(change_type) = request.change_type {
            if change_type <= 2 {
                filter.insert("change_type", change_type as i32);
            } else {
                return Err(LpChangeEventError::QueryParameterError(format!(
                    "无效的变更类型: {}",
                    change_type
                )));
            }
        }

        // 时间范围过滤
        let mut time_filter = doc! {};
        if let Some(start_time) = request.start_time {
            let millis = start_time.timestamp_millis();
            time_filter.insert("$gte", mongodb::bson::DateTime::from_millis(millis));
        }
        if let Some(end_time) = request.end_time {
            let millis = end_time.timestamp_millis();
            time_filter.insert("$lte", mongodb::bson::DateTime::from_millis(millis));
        }
        if !time_filter.is_empty() {
            filter.insert("created_at", time_filter);
        }

        debug!("构建的查询过滤器: {:?}", filter);
        Ok(filter)
    }

    /// 获取用户的事件统计信息
    pub async fn get_user_event_stats(&self, user_wallet: &str) -> Result<UserEventStats, LpChangeEventError> {
        debug!("获取用户事件统计信息: {}", user_wallet);

        if user_wallet.is_empty() {
            return Err(LpChangeEventError::ValidationError("用户钱包地址不能为空".to_string()));
        }

        let filter = doc! { "user_wallet": user_wallet };

        // 获取总数量
        let total_events = self
            .get_repository()
            .count_with_filter(filter.clone())
            .await
            .map_err(LpChangeEventError::DatabaseError)?;

        // 按类型统计
        let deposit_filter = doc! { "user_wallet": user_wallet, "change_type": 0 };
        let withdraw_filter = doc! { "user_wallet": user_wallet, "change_type": 1 };
        let initialize_filter = doc! { "user_wallet": user_wallet, "change_type": 2 };

        let deposit_count = self
            .get_repository()
            .count_with_filter(deposit_filter)
            .await
            .map_err(LpChangeEventError::DatabaseError)?;
        let withdraw_count = self
            .get_repository()
            .count_with_filter(withdraw_filter)
            .await
            .map_err(LpChangeEventError::DatabaseError)?;
        let initialize_count = self
            .get_repository()
            .count_with_filter(initialize_filter)
            .await
            .map_err(LpChangeEventError::DatabaseError)?;

        Ok(UserEventStats {
            user_wallet: user_wallet.to_string(),
            total_events,
            deposit_count,
            withdraw_count,
            initialize_count,
        })
    }
}

/// 用户事件统计信息
#[derive(Debug, Clone)]
pub struct UserEventStats {
    pub user_wallet: String,
    pub total_events: u64,
    pub deposit_count: u64,
    pub withdraw_count: u64,
    pub initialize_count: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[tokio::test]
    async fn test_build_query_filter_basic() {
        let service = create_test_service().await;
        let request = QueryLpChangeEventsRequest {
            user_wallet: Some("test_wallet".to_string()),
            pool_id: Some("test_pool".to_string()),
            lp_mints: None,
            change_type: Some(0),
            start_time: None,
            end_time: None,
            page: None,
            page_size: None,
        };

        let filter = service.build_query_filter(&request).unwrap();
        assert_eq!(filter.get_str("user_wallet").unwrap(), "test_wallet");
        assert_eq!(filter.get_str("pool_id").unwrap(), "test_pool");
        assert_eq!(filter.get_i32("change_type").unwrap(), 0);
    }

    #[tokio::test]
    async fn test_build_query_filter_with_lp_mints() {
        let service = create_test_service().await;
        let request = QueryLpChangeEventsRequest {
            user_wallet: None,
            pool_id: None,
            lp_mints: Some("mint1,mint2,mint3".to_string()),
            change_type: None,
            start_time: None,
            end_time: None,
            page: None,
            page_size: None,
        };

        let filter = service.build_query_filter(&request).unwrap();
        // 多个lp_mint应该使用$in操作符
        assert!(filter.contains_key("lp_mint"));
    }

    #[tokio::test]
    async fn test_build_query_filter_with_time_range() {
        let service = create_test_service().await;
        let start_time = Utc::now() - chrono::Duration::hours(24);
        let end_time = Utc::now();

        let request = QueryLpChangeEventsRequest {
            user_wallet: None,
            pool_id: None,
            lp_mints: None,
            change_type: None,
            start_time: Some(start_time),
            end_time: Some(end_time),
            page: None,
            page_size: None,
        };

        let filter = service.build_query_filter(&request).unwrap();
        assert!(filter.contains_key("created_at"));
    }

    #[tokio::test]
    async fn test_build_query_filter_invalid_change_type() {
        let service = create_test_service().await;
        let request = QueryLpChangeEventsRequest {
            user_wallet: None,
            pool_id: None,
            lp_mints: None,
            change_type: Some(99), // 无效类型
            start_time: None,
            end_time: None,
            page: None,
            page_size: None,
        };

        let result = service.build_query_filter(&request);
        assert!(result.is_err());
        match result.unwrap_err() {
            LpChangeEventError::QueryParameterError(_) => {
                // 预期的错误类型
            }
            _ => panic!("期望QueryParameterError"),
        }
    }

    // 创建测试用的服务实例
    // 注意：这些是单元测试，主要测试逻辑而非数据库操作
    // 实际的集成测试将在后续阶段使用真实数据库进行
    async fn create_test_service() -> LpChangeEventService {
        // 创建一个简单的mock database用于单元测试
        use mongodb::Client;
        use std::sync::Arc;

        // 对于单元测试，我们主要测试业务逻辑
        // 这里使用一个占位database，实际的数据库测试在集成测试中进行
        let mock_client = Client::with_uri_str("mongodb://mock").await.unwrap();
        let mock_mongodb = mock_client.database("test");

        // 模拟创建Database实例 - 这里需要一个简化版本用于测试
        // 在实际应用中，Database::new()会创建完整的数据库连接
        let mock_database = Database {
            refers: mock_mongodb.collection("Refer"),
            users: mock_mongodb.collection("User"),
            rewards: mock_mongodb.collection("Reward"),
            clmm_pools: mock_mongodb.collection("ClmmPool"),
            clmm_configs: mock_mongodb.collection("ClmmConfig"),
            cpmm_configs: mock_mongodb.collection("CpmmConfig"),
            positions: mock_mongodb.collection("Position"),
            global_permission_configs: mock_mongodb.collection("GlobalSolanaPermissionConfig"),
            api_permission_configs: mock_mongodb.collection("SolanaApiPermissionConfig"),
            permission_config_logs: mock_mongodb.collection("PermissionConfigLog"),
            token_infos: mock_mongodb.collection("TokenInfo"),
            clmm_pool_events: mock_mongodb.collection("ClmmPoolEvent"),
            nft_claim_events: mock_mongodb.collection("NftClaimEvent"),
            reward_distribution_events: mock_mongodb.collection("RewardDistributionEvent"),
            launch_events: mock_mongodb.collection("LaunchEvent"),
            deposit_events: mock_mongodb.collection("DepositEvent"),
            token_creation_events: mock_mongodb.collection("TokenCreationEvent"),
            lp_change_events: mock_mongodb.collection("LpChangeEvent"),
            init_pool_events: mock_mongodb.collection("InitPoolEvent"),
            event_scanner_checkpoints: mock_mongodb.collection("EventScannerCheckpoints"),
            scan_records: mock_mongodb.collection("ScanRecords"),
            clmm_pool_repository: database::clmm::clmm_pool::repository::ClmmPoolRepository::new(
                mock_mongodb.collection("ClmmPool"),
            ),
            cpmm_config_repository: database::cpmm::cpmm_config::repository::CpmmConfigRepository::new(
                mock_mongodb.collection("CpmmConfig"),
            ),
            global_permission_repository:
                database::auth::permission_config::repository::GlobalPermissionConfigRepository::new(
                    mock_mongodb.collection("GlobalSolanaPermissionConfig"),
                ),
            api_permission_repository:
                database::auth::permission_config::repository::ApiPermissionConfigRepository::new(
                    mock_mongodb.collection("SolanaApiPermissionConfig"),
                ),
            permission_log_repository:
                database::auth::permission_config::repository::PermissionConfigLogRepository::new(
                    mock_mongodb.collection("PermissionConfigLog"),
                ),
            token_info_repository: database::clmm::token_info::repository::TokenInfoRepository::new(
                mock_mongodb.collection("TokenInfo"),
            ),
            clmm_pool_event_repository: database::events::event_model::repository::ClmmPoolEventRepository::new(
                mock_mongodb.collection("ClmmPoolEvent"),
            ),
            nft_claim_event_repository: database::events::event_model::repository::NftClaimEventRepository::new(
                mock_mongodb.collection("NftClaimEvent"),
            ),
            reward_distribution_event_repository:
                database::events::event_model::repository::RewardDistributionEventRepository::new(
                    mock_mongodb.collection("RewardDistributionEvent"),
                ),
            launch_event_repository: database::events::event_model::repository::LaunchEventRepository::new(
                mock_mongodb.collection("LaunchEvent"),
            ),
            deposit_event_repository: database::events::event_model::repository::DepositEventRepository::new(
                mock_mongodb.collection("DepositEvent"),
            ),
            token_creation_event_repository:
                database::events::event_model::repository::TokenCreationEventRepository::new(
                    mock_mongodb.collection("TokenCreationEvent"),
                ),
            lp_change_event_repository: database::cpmm::lp_change_event::repository::LpChangeEventRepository::new(
                mock_mongodb.collection("LpChangeEvent"),
            ),
            init_pool_event_repository: database::cpmm::init_pool_event::repository::InitPoolEventRepository::new(
                mock_mongodb.collection("InitPoolEvent"),
            ),
            event_scanner_checkpoint_repository:
                database::events::event_scanner::repository::EventScannerCheckpointRepository::new(
                    mock_mongodb.collection("EventScannerCheckpoints"),
                ),
            scan_record_repository: database::events::event_scanner::repository::ScanRecordRepository::new(
                mock_mongodb.collection("ScanRecords"),
            ),
        };

        LpChangeEventService::new(Arc::new(mock_database))
    }
}
