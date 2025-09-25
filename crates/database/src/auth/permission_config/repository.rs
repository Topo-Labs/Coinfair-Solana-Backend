use crate::auth::permission_config::model::{
    GlobalSolanaPermissionConfigModel, PermissionConfigLogModel, SolanaApiPermissionConfigModel,
};
use anyhow::{anyhow, Result};
use mongodb::{
    bson::{doc, oid::ObjectId, Document},
    options::FindOptions,
    Collection,
};
use std::collections::HashMap;
use tracing::{error, info, warn};

/// 全局权限配置仓库
#[derive(Clone, Debug)]
pub struct GlobalPermissionConfigRepository {
    collection: Collection<GlobalSolanaPermissionConfigModel>,
}

impl GlobalPermissionConfigRepository {
    pub fn new(collection: Collection<GlobalSolanaPermissionConfigModel>) -> Self {
        Self { collection }
    }

    /// 获取全局配置（单例模式）- 简化方法
    pub async fn find_global_config(&self) -> Result<Vec<GlobalSolanaPermissionConfigModel>> {
        let filter = doc! { "config_type": "global" };

        match self.collection.find_one(filter, None).await? {
            Some(config) => Ok(vec![config]),
            None => {
                // 如果不存在，创建默认配置
                info!("未找到全局权限配置，创建默认配置");
                let default_config = GlobalSolanaPermissionConfigModel::default();
                self.create_global_config(default_config.clone()).await?;
                Ok(vec![default_config])
            }
        }
    }

    /// 保存或更新全局配置
    pub async fn upsert_global_config(&self, config: GlobalSolanaPermissionConfigModel) -> Result<()> {
        let filter = doc! { "config_type": "global" };
        let update_doc = doc! {
            "$set": {
                "global_read_enabled": config.global_read_enabled,
                "global_write_enabled": config.global_write_enabled,
                "default_read_policy": &config.default_read_policy,
                "default_write_policy": &config.default_write_policy,
                "emergency_shutdown": config.emergency_shutdown,
                "maintenance_mode": config.maintenance_mode,
                "version": config.version as i64,
                "last_updated": config.last_updated as i64,
                "updated_by": &config.updated_by,
            }
        };

        let options = mongodb::options::UpdateOptions::builder().upsert(true).build();

        let result = self.collection.update_one(filter, update_doc, options).await?;

        if result.matched_count > 0 || result.upserted_id.is_some() {
            info!("✅ 全局权限配置保存成功");
            Ok(())
        } else {
            Err(anyhow!("Failed to upsert global permission config"))
        }
    }

    /// 创建全局配置
    pub async fn create_global_config(&self, config: GlobalSolanaPermissionConfigModel) -> Result<ObjectId> {
        let result = self.collection.insert_one(config, None).await?;

        if let Some(object_id) = result.inserted_id.as_object_id() {
            info!("✅ 全局权限配置创建成功: {}", object_id);
            Ok(object_id)
        } else {
            Err(anyhow!("Failed to extract ObjectId from insert result"))
        }
    }

    /// 更新全局配置
    pub async fn update_global_config(&self, config: GlobalSolanaPermissionConfigModel) -> Result<()> {
        let filter = doc! { "config_type": "global" };
        let update_doc = doc! {
            "$set": {
                "global_read_enabled": config.global_read_enabled,
                "global_write_enabled": config.global_write_enabled,
                "default_read_policy": &config.default_read_policy,
                "default_write_policy": &config.default_write_policy,
                "emergency_shutdown": config.emergency_shutdown,
                "maintenance_mode": config.maintenance_mode,
                "version": config.version as i64,
                "last_updated": config.last_updated as i64,
                "updated_by": &config.updated_by,
            }
        };

        let result = self.collection.update_one(filter, update_doc, None).await?;

        if result.matched_count > 0 {
            info!("✅ 全局权限配置更新成功");
            Ok(())
        } else {
            Err(anyhow!("Global permission config not found for update"))
        }
    }

    /// 获取配置版本
    pub async fn get_config_version(&self) -> Result<u64> {
        let configs = self.find_global_config().await?;
        if let Some(config) = configs.first() {
            Ok(config.version)
        } else {
            Ok(1) // 默认版本
        }
    }
}

/// API权限配置仓库
#[derive(Clone, Debug)]
pub struct ApiPermissionConfigRepository {
    collection: Collection<SolanaApiPermissionConfigModel>,
}

impl ApiPermissionConfigRepository {
    pub fn new(collection: Collection<SolanaApiPermissionConfigModel>) -> Self {
        Self { collection }
    }

    /// 创建索引
    pub async fn init_indexes(&self) -> Result<()> {
        use mongodb::options::IndexOptions;
        use mongodb::IndexModel;

        let endpoint_index = IndexModel::builder()
            .keys(doc! { "endpoint": 1 })
            .options(IndexOptions::builder().unique(true).build())
            .build();

        let category_index = IndexModel::builder().keys(doc! { "category": 1, "enabled": 1 }).build();

        let updated_at_index = IndexModel::builder().keys(doc! { "updated_at": -1 }).build();

        self.collection
            .create_indexes(vec![endpoint_index, category_index, updated_at_index], None)
            .await?;

        info!("✅ API权限配置索引创建完成");
        Ok(())
    }

    /// 创建API配置
    pub async fn create_api_config(&self, config: SolanaApiPermissionConfigModel) -> Result<ObjectId> {
        let result = self.collection.insert_one(config.clone(), None).await?;

        if let Some(object_id) = result.inserted_id.as_object_id() {
            info!("✅ API权限配置创建成功: {} -> {}", config.endpoint, object_id);
            Ok(object_id)
        } else {
            Err(anyhow!("Failed to extract ObjectId from insert result"))
        }
    }

    /// 根据端点获取API配置
    pub async fn get_api_config_by_endpoint(&self, endpoint: &str) -> Result<Option<SolanaApiPermissionConfigModel>> {
        let filter = doc! { "endpoint": endpoint };
        Ok(self.collection.find_one(filter, None).await?)
    }

    /// 获取所有API配置 - 简化方法
    pub async fn find_all_api_configs(&self) -> Result<Vec<SolanaApiPermissionConfigModel>> {
        let mut cursor = self.collection.find(None, None).await?;
        let mut configs = Vec::new();

        while cursor.advance().await? {
            configs.push(cursor.deserialize_current()?);
        }

        Ok(configs)
    }

    /// 保存或更新API配置
    pub async fn upsert_api_config(&self, config: SolanaApiPermissionConfigModel) -> Result<()> {
        let filter = doc! { "endpoint": &config.endpoint };
        let update_doc = doc! {
            "$set": {
                "name": &config.name,
                "category": &config.category,
                "read_policy": &config.read_policy,
                "write_policy": &config.write_policy,
                "enabled": config.enabled,
                "updated_at": config.updated_at as i64,
            },
            "$setOnInsert": {
                "endpoint": &config.endpoint,
                "created_at": config.created_at as i64,
            }
        };

        let options = mongodb::options::UpdateOptions::builder().upsert(true).build();

        let result = self.collection.update_one(filter, update_doc, options).await?;

        if result.matched_count > 0 || result.upserted_id.is_some() {
            info!("✅ API权限配置保存成功: {}", config.endpoint);
            Ok(())
        } else {
            Err(anyhow!("Failed to upsert API config for endpoint: {}", config.endpoint))
        }
    }

    /// 根据分类获取API配置
    pub async fn get_api_configs_by_category(&self, category: &str) -> Result<Vec<SolanaApiPermissionConfigModel>> {
        let filter = doc! { "category": category };
        let mut cursor = self.collection.find(filter, None).await?;
        let mut configs = Vec::new();

        while cursor.advance().await? {
            configs.push(cursor.deserialize_current()?);
        }

        Ok(configs)
    }

    /// 更新API配置
    pub async fn update_api_config(&self, endpoint: &str, config: SolanaApiPermissionConfigModel) -> Result<()> {
        let filter = doc! { "endpoint": endpoint };
        let update_doc = doc! {
            "$set": {
                "name": &config.name,
                "category": &config.category,
                "read_policy": &config.read_policy,
                "write_policy": &config.write_policy,
                "enabled": config.enabled,
                "updated_at": config.updated_at as i64,
            }
        };

        let result = self.collection.update_one(filter, update_doc, None).await?;

        if result.matched_count > 0 {
            info!("✅ API权限配置更新成功: {}", endpoint);
            Ok(())
        } else {
            warn!("API权限配置未找到: {}", endpoint);
            Err(anyhow!("API config not found for endpoint: {}", endpoint))
        }
    }

    /// 批量更新API配置
    pub async fn batch_update_api_configs(
        &self,
        configs: HashMap<String, SolanaApiPermissionConfigModel>,
    ) -> Result<usize> {
        let mut updated_count = 0;

        for (endpoint, config) in configs {
            match self.update_api_config(&endpoint, config).await {
                Ok(_) => updated_count += 1,
                Err(e) => {
                    error!("批量更新失败 {}: {}", endpoint, e);
                    // 继续处理其他配置，不中断批量操作
                }
            }
        }

        info!("✅ 批量更新完成，成功更新 {} 个配置", updated_count);
        Ok(updated_count)
    }

    /// 删除API配置
    pub async fn delete_api_config(&self, endpoint: &str) -> Result<()> {
        let filter = doc! { "endpoint": endpoint };
        let result = self.collection.delete_one(filter, None).await?;

        if result.deleted_count > 0 {
            info!("✅ API权限配置删除成功: {}", endpoint);
            Ok(())
        } else {
            warn!("API权限配置未找到: {}", endpoint);
            Err(anyhow!("API config not found for endpoint: {}", endpoint))
        }
    }

    /// 获取启用的API配置数量
    pub async fn count_enabled_configs(&self) -> Result<u64> {
        let filter = doc! { "enabled": true };
        Ok(self.collection.count_documents(filter, None).await?)
    }

    /// 获取总API配置数量
    pub async fn count_total_configs(&self) -> Result<u64> {
        Ok(self.collection.count_documents(None, None).await?)
    }

    /// 获取配置统计信息
    pub async fn get_config_stats(&self) -> Result<ApiConfigStats> {
        let total_configs = self.count_total_configs().await?;
        let enabled_configs = self.count_enabled_configs().await?;
        let disabled_configs = total_configs - enabled_configs;

        // 获取分类统计
        let pipeline = vec![doc! {
            "$group": {
                "_id": "$category",
                "count": { "$sum": 1 }
            }
        }];

        let mut cursor = self.collection.aggregate(pipeline, None).await?;
        let mut category_stats = HashMap::new();

        while cursor.advance().await? {
            let doc = cursor.current();
            if let (Ok(category), Ok(count)) = (doc.get_str("_id"), doc.get_i32("count")) {
                category_stats.insert(category.to_string(), count as u64);
            }
        }

        Ok(ApiConfigStats {
            total_configs,
            enabled_configs,
            disabled_configs,
            category_stats,
        })
    }
}

/// 权限配置日志仓库
#[derive(Clone, Debug)]
pub struct PermissionConfigLogRepository {
    collection: Collection<PermissionConfigLogModel>,
}

impl PermissionConfigLogRepository {
    pub fn new(collection: Collection<PermissionConfigLogModel>) -> Self {
        Self { collection }
    }

    /// 创建索引
    pub async fn init_indexes(&self) -> Result<()> {
        use mongodb::IndexModel;

        let operator_index = IndexModel::builder()
            .keys(doc! { "operator_id": 1, "operation_time": -1 })
            .build();

        let target_index = IndexModel::builder()
            .keys(doc! { "target_type": 1, "target_id": 1, "operation_time": -1 })
            .build();

        let time_index = IndexModel::builder().keys(doc! { "operation_time": -1 }).build();

        self.collection
            .create_indexes(vec![operator_index, target_index, time_index], None)
            .await?;

        info!("✅ 权限配置日志索引创建完成");
        Ok(())
    }

    /// 记录操作日志
    pub async fn log_operation(&self, log: PermissionConfigLogModel) -> Result<ObjectId> {
        let result = self.collection.insert_one(log.clone(), None).await?;

        if let Some(object_id) = result.inserted_id.as_object_id() {
            info!("✅ 权限操作日志记录成功: {} -> {}", log.operation_type, object_id);
            Ok(object_id)
        } else {
            Err(anyhow!("Failed to extract ObjectId from log insert result"))
        }
    }

    /// 获取操作日志（分页）
    pub async fn get_operation_logs(
        &self,
        page: u64,
        page_size: u64,
        filter: Option<Document>,
    ) -> Result<(Vec<PermissionConfigLogModel>, u64)> {
        let skip = (page - 1) * page_size;

        let find_options = FindOptions::builder()
            .skip(skip)
            .limit(page_size as i64)
            .sort(doc! { "operation_time": -1 })
            .build();

        let filter_doc = filter.unwrap_or_else(|| doc! {});

        let mut cursor = self.collection.find(filter_doc.clone(), find_options).await?;
        let mut logs = Vec::new();

        while cursor.advance().await? {
            logs.push(cursor.deserialize_current()?);
        }

        let total_count = self.collection.count_documents(filter_doc, None).await?;

        Ok((logs, total_count))
    }

    /// 根据操作者获取日志
    pub async fn get_logs_by_operator(
        &self,
        operator_id: &str,
        limit: Option<i64>,
    ) -> Result<Vec<PermissionConfigLogModel>> {
        let filter = doc! { "operator_id": operator_id };
        let find_options = FindOptions::builder()
            .limit(limit.unwrap_or(100))
            .sort(doc! { "operation_time": -1 })
            .build();

        let mut cursor = self.collection.find(filter, find_options).await?;
        let mut logs = Vec::new();

        while cursor.advance().await? {
            logs.push(cursor.deserialize_current()?);
        }

        Ok(logs)
    }

    /// 根据目标获取日志
    pub async fn get_logs_by_target(
        &self,
        target_type: &str,
        target_id: &str,
        limit: Option<i64>,
    ) -> Result<Vec<PermissionConfigLogModel>> {
        let filter = doc! {
            "target_type": target_type,
            "target_id": target_id
        };
        let find_options = FindOptions::builder()
            .limit(limit.unwrap_or(50))
            .sort(doc! { "operation_time": -1 })
            .build();

        let mut cursor = self.collection.find(filter, find_options).await?;
        let mut logs = Vec::new();

        while cursor.advance().await? {
            logs.push(cursor.deserialize_current()?);
        }

        Ok(logs)
    }

    /// 清理过期日志（保留指定天数）
    pub async fn cleanup_old_logs(&self, retain_days: i64) -> Result<u64> {
        let cutoff_time = chrono::Utc::now().timestamp() - (retain_days * 24 * 60 * 60);
        let filter = doc! { "operation_time": { "$lt": cutoff_time } };

        let result = self.collection.delete_many(filter, None).await?;

        if result.deleted_count > 0 {
            info!("✅ 清理过期权限日志: {} 条", result.deleted_count);
        }

        Ok(result.deleted_count)
    }
}

/// API配置统计信息
#[derive(Debug, Clone)]
pub struct ApiConfigStats {
    pub total_configs: u64,
    pub enabled_configs: u64,
    pub disabled_configs: u64,
    pub category_stats: HashMap<String, u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_config_stats_creation() {
        let mut category_stats = HashMap::new();
        category_stats.insert("交换".to_string(), 5);
        category_stats.insert("查询".to_string(), 10);

        let stats = ApiConfigStats {
            total_configs: 15,
            enabled_configs: 12,
            disabled_configs: 3,
            category_stats,
        };

        assert_eq!(stats.total_configs, 15);
        assert_eq!(stats.enabled_configs, 12);
        assert_eq!(stats.disabled_configs, 3);
        assert_eq!(stats.category_stats.len(), 2);
        assert_eq!(stats.category_stats.get("交换"), Some(&5));
        assert_eq!(stats.category_stats.get("查询"), Some(&10));
    }
}
