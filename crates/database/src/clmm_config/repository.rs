use anyhow::Result;
use futures::stream::TryStreamExt;
use mongodb::{
    bson::{doc, DateTime as BsonDateTime},
    Collection, IndexModel,
};
use tracing::{error, info};

use super::model::{ClmmConfigModel, ClmmConfigQuery, ClmmConfigStats};

/// CLMM配置仓库
#[derive(Clone)]
pub struct ClmmConfigRepository {
    collection: Collection<ClmmConfigModel>,
}

impl ClmmConfigRepository {
    /// 创建新的CLMM配置仓库
    pub fn new(collection: Collection<ClmmConfigModel>) -> Self {
        Self { collection }
    }

    /// 初始化数据库索引
    pub async fn init_indexes(&self) -> Result<()> {
        info!("🔧 初始化CLMM配置集合索引...");

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
                info!("✅ CLMM配置索引创建成功: {:?}", results.index_names);
                Ok(())
            }
            Err(e) => {
                error!("❌ CLMM配置索引创建失败: {}", e);
                Err(e.into())
            }
        }
    }

    /// 保存CLMM配置 (upsert操作)
    pub async fn save_config(&self, config: &ClmmConfigModel) -> Result<String> {
        let filter = doc! { "configId": &config.config_id };
        let update = doc! {
            "$set": mongodb::bson::to_document(config)?
        };

        let options = mongodb::options::UpdateOptions::builder()
            .upsert(true)
            .build();

        match self.collection.update_one(filter, update, options).await {
            Ok(result) => {
                if let Some(upserted_id) = result.upserted_id {
                    info!("✅ 新建CLMM配置: {}", config.config_id);
                    Ok(upserted_id.to_string())
                } else {
                    info!("🔄 更新CLMM配置: {}", config.config_id);
                    Ok(config.config_id.clone())
                }
            }
            Err(e) => {
                error!("❌ 保存CLMM配置失败: {}", e);
                Err(e.into())
            }
        }
    }

    /// 批量保存CLMM配置
    pub async fn save_configs(&self, configs: &[ClmmConfigModel]) -> Result<Vec<String>> {
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

        info!("✅ 批量保存{}个CLMM配置成功", saved_ids.len());
        Ok(saved_ids)
    }

    /// 根据配置ID获取配置
    pub async fn get_config_by_id(&self, config_id: &str) -> Result<Option<ClmmConfigModel>> {
        let filter = doc! { "configId": config_id };

        match self.collection.find_one(filter, None).await {
            Ok(config) => Ok(config),
            Err(e) => {
                error!("❌ 根据ID获取CLMM配置失败: {}", e);
                Err(e.into())
            }
        }
    }

    /// 根据索引获取配置
    pub async fn get_config_by_index(&self, index: u32) -> Result<Option<ClmmConfigModel>> {
        let filter = doc! { "index": index, "enabled": true };

        match self.collection.find_one(filter, None).await {
            Ok(config) => Ok(config),
            Err(e) => {
                error!("❌ 根据索引获取CLMM配置失败: {}", e);
                Err(e.into())
            }
        }
    }

    /// 获取所有启用的配置
    pub async fn get_all_enabled_configs(&self) -> Result<Vec<ClmmConfigModel>> {
        let filter = doc! { "enabled": true };
        let options = mongodb::options::FindOptions::builder()
            .sort(doc! { "index": 1 })
            .build();

        match self.collection.find(filter, options).await {
            Ok(cursor) => {
                let configs: Vec<ClmmConfigModel> = cursor.try_collect().await?;
                Ok(configs)
            }
            Err(e) => {
                error!("❌ 获取所有启用的CLMM配置失败: {}", e);
                Err(e.into())
            }
        }
    }

    /// 查询配置列表 (支持分页和过滤)
    pub async fn query_configs(&self, query: &ClmmConfigQuery) -> Result<Vec<ClmmConfigModel>> {
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
                let configs: Vec<ClmmConfigModel> = cursor.try_collect().await?;
                Ok(configs)
            }
            Err(e) => {
                error!("❌ 查询CLMM配置失败: {}", e);
                Err(e.into())
            }
        }
    }

    /// 获取配置统计信息
    pub async fn get_config_stats(&self) -> Result<ClmmConfigStats> {
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

        Ok(ClmmConfigStats {
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
                    info!("✅ 禁用CLMM配置: {}", config_id);
                    Ok(true)
                } else {
                    info!("⚠️ 未找到要禁用的CLMM配置: {}", config_id);
                    Ok(false)
                }
            }
            Err(e) => {
                error!("❌ 禁用CLMM配置失败: {}", e);
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
                    info!("✅ 启用CLMM配置: {}", config_id);
                    Ok(true)
                } else {
                    info!("⚠️ 未找到要启用的CLMM配置: {}", config_id);
                    Ok(false)
                }
            }
            Err(e) => {
                error!("❌ 启用CLMM配置失败: {}", e);
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
                info!("🗑️ 清空所有CLMM配置，删除数量: {}", result.deleted_count);
                Ok(result.deleted_count)
            }
            Err(e) => {
                error!("❌ 清空CLMM配置失败: {}", e);
                Err(e.into())
            }
        }
    }
}