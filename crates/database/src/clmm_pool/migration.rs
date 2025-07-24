use mongodb::{
    bson::{doc, Document},
    options::IndexOptions,
    Collection, Database, IndexModel,
};
use tracing::{error, info, warn};
use utils::AppResult;

/// Database migration for pool type enhancement
pub struct PoolTypeMigration;

impl PoolTypeMigration {
    /// Apply the migration - add pool_type field to existing documents
    pub async fn migrate_up(&self, db: &Database) -> AppResult<()> {
        info!("🔄 开始执行池子类型字段迁移 (migrate_up)...");

        let collection: Collection<Document> = db.collection("ClmmPool");

        // Check if migration is needed by counting documents without pool_type field
        let filter_without_pool_type = doc! { "pool_type": { "$exists": false } };
        let count_without_pool_type = collection.count_documents(filter_without_pool_type.clone(), None).await?;

        if count_without_pool_type == 0 {
            info!("✅ 迁移已完成，所有池子记录都已包含 pool_type 字段");
            return Ok(());
        }

        info!("📊 发现 {} 个池子记录需要添加 pool_type 字段", count_without_pool_type);

        // Add pool_type field to existing documents
        // Default to "concentrated" for backward compatibility
        let update = doc! {
            "$set": {
                "pool_type": "concentrated", // Default to concentrated
                "updated_at": chrono::Utc::now().timestamp() as f64
            }
        };

        let result = collection.update_many(filter_without_pool_type, update, None).await?;
        info!("✅ 成功更新 {} 个池子记录，添加了 pool_type 字段", result.modified_count);

        // Create index on pool_type field for performance
        self.create_pool_type_indexes(&collection).await?;

        info!("🎉 池子类型字段迁移完成！");
        Ok(())
    }

    /// Rollback the migration - remove pool_type field from documents
    pub async fn migrate_down(&self, db: &Database) -> AppResult<()> {
        warn!("⚠️  开始执行池子类型字段迁移回滚 (migrate_down)...");

        let collection: Collection<Document> = db.collection("ClmmPool");

        // Check if rollback is needed
        let filter_with_pool_type = doc! { "pool_type": { "$exists": true } };
        let count_with_pool_type = collection.count_documents(filter_with_pool_type.clone(), None).await?;

        if count_with_pool_type == 0 {
            info!("✅ 回滚已完成，所有池子记录都已移除 pool_type 字段");
            return Ok(());
        }

        info!("📊 发现 {} 个池子记录需要移除 pool_type 字段", count_with_pool_type);

        // Remove pool_type field from all documents
        let update = doc! {
            "$unset": {
                "pool_type": ""
            },
            "$set": {
                "updated_at": chrono::Utc::now().timestamp() as f64
            }
        };

        let result = collection.update_many(doc! {}, update, None).await?;
        info!("✅ 成功从 {} 个池子记录中移除了 pool_type 字段", result.modified_count);

        // Drop pool_type related indexes
        self.drop_pool_type_indexes(&collection).await?;

        warn!("🔄 池子类型字段迁移回滚完成！");
        Ok(())
    }

    /// Create indexes related to pool_type field
    async fn create_pool_type_indexes(&self, collection: &Collection<Document>) -> AppResult<()> {
        info!("🔧 创建池子类型相关索引...");

        let indexes = vec![
            // Single index on pool_type field
            IndexModel::builder()
                .keys(doc! { "pool_type": 1 })
                .options(IndexOptions::builder().name("pool_type_1".to_string()).build())
                .build(),
            // Compound index on pool_type and created_at for efficient filtering and sorting
            IndexModel::builder()
                .keys(doc! {
                    "pool_type": 1,
                    "created_at": -1
                })
                .options(IndexOptions::builder().name("pool_type_1_created_at_-1".to_string()).build())
                .build(),
        ];

        match collection.create_indexes(indexes, None).await {
            Ok(_) => {
                info!("✅ 池子类型索引创建成功");
                Ok(())
            }
            Err(e) => {
                error!("❌ 池子类型索引创建失败: {:?}", e);
                Err(utils::AppError::InternalServerErrorWithContext(format!("索引创建失败: {}", e)))
            }
        }
    }

    /// Drop indexes related to pool_type field
    async fn drop_pool_type_indexes(&self, collection: &Collection<Document>) -> AppResult<()> {
        info!("🗑️  删除池子类型相关索引...");

        let index_names = vec!["pool_type_1", "pool_type_1_created_at_-1"];

        for index_name in index_names {
            match collection.drop_index(index_name, None).await {
                Ok(_) => {
                    info!("✅ 成功删除索引: {}", index_name);
                }
                Err(e) => {
                    // Log warning but don't fail the migration if index doesn't exist
                    warn!("⚠️  删除索引 {} 时出现警告: {:?}", index_name, e);
                }
            }
        }

        info!("🗑️  池子类型索引删除完成");
        Ok(())
    }

    /// Check migration status - returns true if migration has been applied
    pub async fn is_migrated(&self, db: &Database) -> AppResult<bool> {
        let collection: Collection<Document> = db.collection("ClmmPool");

        // Check if any documents exist without pool_type field
        let filter_without_pool_type = doc! { "pool_type": { "$exists": false } };
        let count_without_pool_type = collection.count_documents(filter_without_pool_type, None).await?;

        // Migration is complete if no documents are missing the pool_type field
        Ok(count_without_pool_type == 0)
    }

    /// Get migration statistics
    pub async fn get_migration_stats(&self, db: &Database) -> AppResult<MigrationStats> {
        let collection: Collection<Document> = db.collection("ClmmPool");

        // Count total documents
        let total_count = collection.count_documents(doc! {}, None).await?;

        // Count documents with pool_type field
        let with_pool_type = collection.count_documents(doc! { "pool_type": { "$exists": true } }, None).await?;

        // Count documents without pool_type field
        let without_pool_type = if total_count >= with_pool_type { total_count - with_pool_type } else { 0 };

        // Count by pool type
        let concentrated_count = collection.count_documents(doc! { "pool_type": "concentrated" }, None).await?;

        let standard_count = collection.count_documents(doc! { "pool_type": "standard" }, None).await?;

        Ok(MigrationStats {
            total_pools: total_count,
            pools_with_type: with_pool_type,
            pools_without_type: without_pool_type,
            concentrated_pools: concentrated_count,
            standard_pools: standard_count,
            migration_complete: without_pool_type == 0,
        })
    }

    /// Validate pool_type values in the database
    pub async fn validate_pool_types(&self, db: &Database) -> AppResult<ValidationResult> {
        let collection: Collection<Document> = db.collection("ClmmPool");

        // Find documents with invalid pool_type values
        let invalid_filter = doc! {
            "pool_type": {
                "$exists": true,
                "$nin": ["concentrated", "standard"]
            }
        };

        let invalid_count = collection.count_documents(invalid_filter, None).await?;

        // Find documents with null pool_type
        let null_filter = doc! { "pool_type": { "$type": "null" } };
        let null_count = collection.count_documents(null_filter, None).await?;

        Ok(ValidationResult {
            invalid_pool_types: invalid_count,
            null_pool_types: null_count,
            is_valid: invalid_count == 0 && null_count == 0,
        })
    }
}

/// Migration statistics
#[derive(Debug, Clone)]
pub struct MigrationStats {
    /// Total number of pool documents
    pub total_pools: u64,
    /// Number of pools with pool_type field
    pub pools_with_type: u64,
    /// Number of pools without pool_type field
    pub pools_without_type: u64,
    /// Number of concentrated pools
    pub concentrated_pools: u64,
    /// Number of standard pools
    pub standard_pools: u64,
    /// Whether migration is complete
    pub migration_complete: bool,
}

/// Pool type validation result
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// Number of pools with invalid pool_type values
    pub invalid_pool_types: u64,
    /// Number of pools with null pool_type values
    pub null_pool_types: u64,
    /// Whether all pool types are valid
    pub is_valid: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_migration_stats_creation() {
        let stats = MigrationStats {
            total_pools: 100,
            pools_with_type: 80,
            pools_without_type: 20,
            concentrated_pools: 60,
            standard_pools: 20,
            migration_complete: false,
        };

        assert_eq!(stats.total_pools, 100);
        assert_eq!(stats.pools_with_type, 80);
        assert_eq!(stats.pools_without_type, 20);
        assert_eq!(stats.concentrated_pools, 60);
        assert_eq!(stats.standard_pools, 20);
        assert!(!stats.migration_complete);
    }

    #[test]
    fn test_validation_result_creation() {
        let result = ValidationResult {
            invalid_pool_types: 5,
            null_pool_types: 2,
            is_valid: false,
        };

        assert_eq!(result.invalid_pool_types, 5);
        assert_eq!(result.null_pool_types, 2);
        assert!(!result.is_valid);

        let valid_result = ValidationResult {
            invalid_pool_types: 0,
            null_pool_types: 0,
            is_valid: true,
        };

        assert_eq!(valid_result.invalid_pool_types, 0);
        assert_eq!(valid_result.null_pool_types, 0);
        assert!(valid_result.is_valid);
    }

    #[test]
    fn test_pool_type_migration_struct_creation() {
        let migration = PoolTypeMigration;
        // Just verify the struct can be created
        assert!(std::mem::size_of_val(&migration) == 0); // Zero-sized struct
    }

    // Note: Integration tests that require MongoDB connection should be run separately
    // with a test database. The migration methods are designed to work with real MongoDB
    // instances and include proper error handling and logging.
}
