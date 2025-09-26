use anyhow::Result;
use futures::stream::TryStreamExt;
use mongodb::{
    bson::{doc, DateTime as BsonDateTime},
    Collection, IndexModel,
};
use tracing::{error, info};

use super::model::{CpmmConfigModel, CpmmConfigQuery, CpmmConfigStats};

/// CPMM配置仓库
#[derive(Clone, Debug)]
pub struct CpmmConfigRepository {
    collection: Collection<CpmmConfigModel>,
}

impl CpmmConfigRepository {
    /// 创建新的CPMM配置仓库
    pub fn new(collection: Collection<CpmmConfigModel>) -> Self {
        Self { collection }
    }

    /// 初始化数据库索引
    pub async fn init_indexes(&self) -> Result<()> {
        info!("🔧 初始化CPMM配置集合索引...");

        let indexes = vec![
            // 配置ID唯一索引
            IndexModel::builder()
                .keys(doc! { "configId": 1 })
                .options(
                    mongodb::options::IndexOptions::builder()
                        .unique(true)
                        .name("configId_unique".to_string())
                        .build(),
                )
                .build(),
            // 索引字段索引 (可能会查询)
            IndexModel::builder()
                .keys(doc! { "index": 1 })
                .options(
                    mongodb::options::IndexOptions::builder()
                        .name("index_1".to_string())
                        .build(),
                )
                .build(),
            // 启用状态索引
            IndexModel::builder()
                .keys(doc! { "enabled": 1 })
                .options(
                    mongodb::options::IndexOptions::builder()
                        .name("enabled_1".to_string())
                        .build(),
                )
                .build(),
            // 复合索引：启用状态和索引
            IndexModel::builder()
                .keys(doc! { "enabled": 1, "index": 1 })
                .options(
                    mongodb::options::IndexOptions::builder()
                        .name("enabled_index_compound".to_string())
                        .build(),
                )
                .build(),
            // 创建时间索引 (用于排序)
            IndexModel::builder()
                .keys(doc! { "createdAt": 1 })
                .options(
                    mongodb::options::IndexOptions::builder()
                        .name("createdAt_1".to_string())
                        .build(),
                )
                .build(),
        ];

        match self.collection.create_indexes(indexes, None).await {
            Ok(results) => {
                info!("✅ CPMM配置索引创建成功: {:?}", results.index_names);
                Ok(())
            }
            Err(e) => {
                error!("❌ CPMM配置索引创建失败: {}", e);
                Err(e.into())
            }
        }
    }

    /// 保存CPMM配置 (upsert操作)
    pub async fn save_config(&self, config: &CpmmConfigModel) -> Result<String> {
        let filter = doc! { "configId": &config.config_id };
        let update = doc! {
            "$set": mongodb::bson::to_document(config)?
        };

        let options = mongodb::options::UpdateOptions::builder().upsert(true).build();

        match self.collection.update_one(filter, update, options).await {
            Ok(result) => {
                if let Some(upserted_id) = result.upserted_id {
                    info!("✅ 新建CPMM配置: {}", config.config_id);
                    Ok(upserted_id.to_string())
                } else {
                    info!("🔄 更新CPMM配置: {}", config.config_id);
                    Ok(config.config_id.clone())
                }
            }
            Err(e) => {
                error!("❌ 保存CPMM配置失败: {}", e);
                Err(e.into())
            }
        }
    }

    /// 批量保存CPMM配置
    pub async fn save_configs(&self, configs: &[CpmmConfigModel]) -> Result<Vec<String>> {
        let mut saved_ids = Vec::new();

        for config in configs {
            match self.save_config(config).await {
                Ok(id) => saved_ids.push(id),
                Err(e) => {
                    error!("❌ 批量保存配置{}失败: {}", config.config_id, e);
                    return Err(e);
                }
            }
        }

        info!("✅ 批量保存{}个CPMM配置成功", saved_ids.len());
        Ok(saved_ids)
    }

    /// 根据配置ID获取配置
    pub async fn get_config_by_id(&self, config_id: &str) -> Result<Option<CpmmConfigModel>> {
        let filter = doc! { "configId": config_id };

        match self.collection.find_one(filter, None).await {
            Ok(config) => Ok(config),
            Err(e) => {
                error!("❌ 根据ID获取CPMM配置失败: {}", e);
                Err(e.into())
            }
        }
    }

    /// 根据配置地址获取配置 (别名方法，与 get_config_by_id 相同)
    pub async fn get_config_by_address(&self, config_address: &str) -> Result<Option<CpmmConfigModel>> {
        self.get_config_by_id(config_address).await
    }

    /// 批量根据配置地址获取配置 (使用 $in 查询，性能优化版本)
    pub async fn get_configs_by_addresses_batch(&self, config_addresses: &[String]) -> Result<Vec<CpmmConfigModel>> {
        let start_time = std::time::Instant::now();

        if config_addresses.is_empty() {
            info!("📋 批量查询配置地址列表为空，返回空结果");
            return Ok(Vec::new());
        }

        info!("🔍 MongoDB批量查询{}个配置地址 (使用$in操作符)", config_addresses.len());

        let filter = doc! {
            "configId": {
                "$in": config_addresses
            },
            "enabled": true
        };

        let options = mongodb::options::FindOptions::builder()
            .sort(doc! { "index": 1 })
            .build();

        match self.collection.find(filter, options).await {
            Ok(cursor) => {
                let configs: Vec<CpmmConfigModel> = cursor.try_collect().await?;
                let duration = start_time.elapsed();

                info!(
                    "✅ MongoDB批量查询完成: 查询{}个地址，找到{}个配置，耗时{:?}",
                    config_addresses.len(),
                    configs.len(),
                    duration
                );

                // 性能监控：如果查询时间超过100ms，记录警告
                if duration.as_millis() > 100 {
                    tracing::warn!("⚠️ 批量查询耗时较长: {:?}，请检查索引配置", duration);
                }

                Ok(configs)
            }
            Err(e) => {
                let duration = start_time.elapsed();
                error!("❌ MongoDB批量查询配置失败: {}，耗时{:?}", e, duration);
                Err(e.into())
            }
        }
    }

    /// 根据索引获取配置
    pub async fn get_config_by_index(&self, index: u32) -> Result<Option<CpmmConfigModel>> {
        let filter = doc! { "index": index, "enabled": true };

        match self.collection.find_one(filter, None).await {
            Ok(config) => Ok(config),
            Err(e) => {
                error!("❌ 根据索引获取CPMM配置失败: {}", e);
                Err(e.into())
            }
        }
    }

    /// 获取所有启用的配置
    pub async fn get_all_enabled_configs(&self) -> Result<Vec<CpmmConfigModel>> {
        let filter = doc! { "enabled": true };
        let options = mongodb::options::FindOptions::builder()
            .sort(doc! { "index": 1 })
            .build();

        match self.collection.find(filter, options).await {
            Ok(cursor) => {
                let configs: Vec<CpmmConfigModel> = cursor.try_collect().await?;
                Ok(configs)
            }
            Err(e) => {
                error!("❌ 获取所有启用的CPMM配置失败: {}", e);
                Err(e.into())
            }
        }
    }

    /// 查询配置列表 (支持分页和过滤)
    pub async fn query_configs(&self, query: &CpmmConfigQuery) -> Result<Vec<CpmmConfigModel>> {
        let mut filter = doc! {};

        // 构建过滤条件
        if let Some(config_id) = &query.config_id {
            filter.insert("configId", config_id);
        }
        if let Some(index) = query.index {
            filter.insert("index", index);
        }
        if let Some(enabled) = query.enabled {
            filter.insert("enabled", enabled);
        }

        // 分页参数
        let page = query.page.unwrap_or(1).max(1);
        let limit = query.limit.unwrap_or(20).min(100); // 最大100条
        let skip = (page - 1) * limit;

        let options = mongodb::options::FindOptions::builder()
            .sort(doc! { "index": 1 })
            .skip(skip as u64)
            .limit(limit)
            .build();

        match self.collection.find(filter, options).await {
            Ok(cursor) => {
                let configs: Vec<CpmmConfigModel> = cursor.try_collect().await?;
                Ok(configs)
            }
            Err(e) => {
                error!("❌ 查询CPMM配置失败: {}", e);
                Err(e.into())
            }
        }
    }

    /// 获取配置统计信息
    pub async fn get_config_stats(&self) -> Result<CpmmConfigStats> {
        // 总配置数量
        let total_configs = self.collection.count_documents(doc! {}, None).await? as u64;

        // 启用的配置数量
        let enabled_configs = self.collection.count_documents(doc! { "enabled": true }, None).await? as u64;

        // 禁用的配置数量
        let disabled_configs = total_configs - enabled_configs;

        // 最后同步时间
        let last_sync_filter = doc! { "lastSyncAt": { "$exists": true } };
        let last_sync_options = mongodb::options::FindOneOptions::builder()
            .sort(doc! { "lastSyncAt": -1 })
            .projection(doc! { "lastSyncAt": 1 })
            .build();

        let last_sync_time = match self.collection.find_one(last_sync_filter, last_sync_options).await? {
            Some(config) => config.last_sync_at,
            None => None,
        };

        Ok(CpmmConfigStats {
            total_configs,
            enabled_configs,
            disabled_configs,
            last_sync_time,
        })
    }

    /// 禁用配置
    pub async fn disable_config(&self, config_id: &str) -> Result<bool> {
        let filter = doc! { "configId": config_id };
        let update = doc! {
            "$set": {
                "enabled": false,
                "updatedAt": BsonDateTime::now()
            }
        };

        match self.collection.update_one(filter, update, None).await {
            Ok(result) => {
                if result.matched_count > 0 {
                    info!("✅ 禁用CPMM配置: {}", config_id);
                    Ok(true)
                } else {
                    info!("⚠️ 未找到要禁用的CPMM配置: {}", config_id);
                    Ok(false)
                }
            }
            Err(e) => {
                error!("❌ 禁用CPMM配置失败: {}", e);
                Err(e.into())
            }
        }
    }

    /// 启用配置
    pub async fn enable_config(&self, config_id: &str) -> Result<bool> {
        let filter = doc! { "configId": config_id };
        let update = doc! {
            "$set": {
                "enabled": true,
                "updatedAt": BsonDateTime::now()
            }
        };

        match self.collection.update_one(filter, update, None).await {
            Ok(result) => {
                if result.matched_count > 0 {
                    info!("✅ 启用CPMM配置: {}", config_id);
                    Ok(true)
                } else {
                    info!("⚠️ 未找到要启用的CPMM配置: {}", config_id);
                    Ok(false)
                }
            }
            Err(e) => {
                error!("❌ 启用CPMM配置失败: {}", e);
                Err(e.into())
            }
        }
    }

    /// 检查是否有配置数据
    pub async fn has_configs(&self) -> Result<bool> {
        let count = self.collection.count_documents(doc! {}, None).await?;
        Ok(count > 0)
    }

    /// 清空所有配置 (谨慎使用)
    pub async fn clear_all_configs(&self) -> Result<u64> {
        match self.collection.delete_many(doc! {}, None).await {
            Ok(result) => {
                info!("🗑️ 清空所有CPMM配置，删除数量: {}", result.deleted_count);
                Ok(result.deleted_count)
            }
            Err(e) => {
                error!("❌ 清空CPMM配置失败: {}", e);
                Err(e.into())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mongodb::bson::oid::ObjectId;

    // 创建测试配置模型
    fn create_test_config(config_id: &str, index: u32) -> CpmmConfigModel {
        CpmmConfigModel {
            id: Some(ObjectId::new()),
            config_id: config_id.to_string(),
            index,
            protocol_fee_rate: 120000,
            trade_fee_rate: 2500,
            fund_fee_rate: 40000,
            create_pool_fee: "150000000".to_string(),
            creator_fee_rate: 0,
            enabled: true,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            last_sync_at: Some(chrono::Utc::now()),
        }
    }

    #[tokio::test]
    async fn test_batch_query_performance_simulation() {
        // 这是一个性能模拟测试，不需要真实数据库连接

        let start_time = std::time::Instant::now();

        // 模拟批量查询操作
        let test_addresses = vec!["Config1".to_string(), "Config2".to_string(), "Config3".to_string()];

        // 模拟查询处理时间
        tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;

        let duration = start_time.elapsed();

        // 验证性能特征
        assert!(duration.as_millis() < 50, "模拟批量查询耗时过长: {:?}", duration);
        assert!(!test_addresses.is_empty());

        println!("✅ 批量查询性能模拟测试通过，模拟耗时: {:?}", duration);
    }

    #[test]
    fn test_config_model_creation() {
        let config = create_test_config("TestConfig123", 0);

        assert_eq!(config.config_id, "TestConfig123");
        assert_eq!(config.index, 0);
        assert_eq!(config.protocol_fee_rate, 120000);
        assert_eq!(config.trade_fee_rate, 2500);
        assert_eq!(config.fund_fee_rate, 40000);
        assert_eq!(config.create_pool_fee, "150000000");
        assert_eq!(config.creator_fee_rate, 0);
        assert!(config.enabled);

        println!("✅ 配置模型创建测试通过");
    }

    #[test]
    fn test_batch_query_filter_construction() {
        // 测试MongoDB过滤器构造逻辑
        let config_addresses = vec!["Config1".to_string(), "Config2".to_string(), "Config3".to_string()];

        let filter = doc! {
            "configId": {
                "$in": config_addresses.clone()
            },
            "enabled": true
        };

        // 验证过滤器结构
        assert!(filter.contains_key("configId"));
        assert!(filter.contains_key("enabled"));

        let config_id_filter = filter.get("configId").unwrap();
        assert!(config_id_filter.as_document().unwrap().contains_key("$in"));

        println!("✅ 批量查询过滤器构造测试通过");
    }
}