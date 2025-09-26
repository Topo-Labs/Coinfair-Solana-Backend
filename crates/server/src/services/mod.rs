////////////////////////////////////////////////////////////////////////
//
// 1. 每个Domain(Entity)单独一个文件夹
// 2. 每个Domain由两部分组成:
//    - model: 定义Schema
//    - repository: 实际的数据库底层操作
//
//////////////////////////////////////////////////////////////////////

pub mod position_storage;
pub mod solana;
pub mod user;
pub mod docs;
pub mod middleware;

use crate::services::{
    solana::clmm::launch_event::LaunchEventService,
    solana::{DynSolanaService, SolanaService},
};
use database::Database;
use std::sync::Arc;
use tracing::{error, info, warn};
use database::clmm::clmm_pool::PoolTypeMigration;
use database::clmm::position::repository::PositionRepositoryTrait;
use user::user_service::{DynUserService, UserService};
use self::solana::auth::solana_permission_service::{DynSolanaPermissionService, SolanaPermissionService};
use self::solana::clmm::refer::refer_service::{DynReferService, ReferService};
use self::solana::clmm::reward::reward_service::{DynRewardService, RewardService};
use self::solana::clmm::token::token_service::TokenService;

#[derive(Clone)]
pub struct Services {
    pub user: DynUserService,
    pub refer: DynReferService,
    pub reward: DynRewardService,
    pub solana: DynSolanaService,
    pub solana_permission: DynSolanaPermissionService,
    pub token: Arc<TokenService>,
    pub launch_event: Arc<LaunchEventService>,
    pub database: Arc<Database>,
}

impl Services {
    pub fn new(db: Database) -> Self {
        // 优先尝试从环境变量创建，否则使用默认配置
        match Self::from_env(db.clone()) {
            Ok(mut services) => {
                info!("🧠 Services initialized from environment variables");

                // 初始化数据库服务（包括运行迁移）
                if let Err(e) = tokio::task::block_in_place(|| {
                    tokio::runtime::Handle::current().block_on(services.init_database_service())
                }) {
                    error!("❌ 数据库服务初始化失败: {}", e);
                    warn!("⚠️ 继续启动服务，但某些功能可能不可用");
                }

                services
            }
            Err(e) => {
                tracing::warn!("Failed to initialize from environment: {}, using default config", e);

                let database = Arc::new(db.clone());
                let user = Arc::new(UserService::new(database.clone())) as DynUserService;
                let refer = Arc::new(ReferService::new(database.clone())) as DynReferService;
                let reward = Arc::new(RewardService::new(database.clone())) as DynRewardService;

                // 创建带数据库的SolanaService
                let solana = match SolanaService::with_database(db.clone()) {
                    Ok(service) => Arc::new(service) as DynSolanaService,
                    Err(e) => {
                        tracing::warn!("Failed to create SolanaService with database: {}, using default", e);
                        Arc::new(SolanaService::default()) as DynSolanaService
                    }
                };

                // 创建权限服务
                let solana_permission =
                    Arc::new(SolanaPermissionService::with_database(database.clone())) as DynSolanaPermissionService;

                // 创建代币服务
                let token = Arc::new(TokenService::new(database.clone()));

                // 创建Launch事件服务
                let launch_event = Arc::new(LaunchEventService::new(database.clone()));

                let mut services = Self {
                    user,
                    refer,
                    reward,
                    solana,
                    solana_permission,
                    token,
                    launch_event,
                    database,
                };

                // 初始化数据库服务（包括运行迁移）
                if let Err(e) = tokio::task::block_in_place(|| {
                    tokio::runtime::Handle::current().block_on(services.init_database_service())
                }) {
                    error!("❌ 数据库服务初始化失败: {}", e);
                    warn!("⚠️ 继续启动服务，但某些功能可能不可用");
                }

                info!("🧠 Services initialized with default configuration");
                services
            }
        }
    }

    /// 从环境变量创建Services (生产环境推荐)
    pub fn from_env(db: Database) -> Result<Self, Box<dyn std::error::Error>> {
        let database = Arc::new(db.clone());

        let user = Arc::new(UserService::new(database.clone())) as DynUserService;
        let refer = Arc::new(ReferService::new(database.clone())) as DynReferService;
        let reward = Arc::new(RewardService::new(database.clone())) as DynRewardService;

        // 创建带数据库的SolanaService
        let solana = Arc::new(SolanaService::with_database(db)?) as DynSolanaService;

        // 创建权限服务
        let solana_permission =
            Arc::new(SolanaPermissionService::with_database(database.clone())) as DynSolanaPermissionService;

        // 创建代币服务
        let token = Arc::new(TokenService::new(database.clone()));

        // 创建Launch事件服务
        let launch_event = Arc::new(LaunchEventService::new(database.clone()));

        info!("🧠 initializing services from environment...");

        Ok(Self {
            user,
            refer,
            reward,
            solana,
            solana_permission,
            token,
            launch_event,
            database,
        })
    }

    /// 初始化数据库服务，包括运行迁移和配置
    pub async fn init_database_service(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        info!("🔧 初始化数据库服务...");

        // 1. 运行池子类型迁移
        // self.run_pool_type_migration().await?;

        // 2. 初始化CLMM池子存储服务索引
        self.init_clmm_pool_indexes().await?;

        // 3. 初始化Position存储服务索引
        self.init_position_indexes().await?;

        // 4. 初始化权限配置索引
        self.init_permission_indexes().await?;

        // 5. 初始化TokenInfo索引
        self.init_token_info_indexes().await?;

        // 6. 初始化默认权限配置
        self.init_default_permission_config().await?;

        // 7. 初始化权限服务（从数据库加载配置）
        self.init_permission_service().await?;

        // 8. 应用默认分页配置
        // self.apply_default_pagination_config().await?;

        // 9. 验证数据库健康状态
        match self.get_database_health().await {
            Ok(health) => {
                info!("🏥 数据库健康检查:");
                info!("  状态: {}", if health.is_healthy { "健康" } else { "异常" });
                info!("  响应时间: {}ms", health.response_time_ms);
                info!("  总池子数: {}", health.total_pools);
                info!("  活跃池子数: {}", health.active_pools);

                if !health.is_healthy {
                    warn!("⚠️ 数据库健康检查发现问题: {:?}", health.issues);
                }
            }
            Err(e) => {
                warn!("⚠️ 数据库健康检查失败: {}", e);
            }
        }

        info!("✅ 数据库服务初始化完成");
        Ok(())
    }

    /// 初始化TokenInfo索引
    async fn init_token_info_indexes(&self) -> Result<(), Box<dyn std::error::Error>> {
        match self.database.token_info_repository.init_indexes().await {
            Ok(_) => {
                info!("✅ TokenInfo数据库索引初始化完成");
                Ok(())
            }
            Err(e) => {
                error!("❌ TokenInfo数据库索引初始化失败: {}", e);
                Err(format!("TokenInfo索引初始化失败: {}", e).into())
            }
        }
    }

    /// 初始化权限配置索引
    async fn init_permission_indexes(&self) -> Result<(), Box<dyn std::error::Error>> {
        info!("🔧 初始化权限配置数据库索引...");

        match self.database.init_repository_indexes().await {
            Ok(_) => {
                info!("✅ 权限配置数据库索引初始化完成");
                Ok(())
            }
            Err(e) => {
                error!("❌ 权限配置数据库索引初始化失败: {}", e);
                Err(format!("权限索引初始化失败: {}", e).into())
            }
        }
    }

    /// 初始化权限服务
    async fn init_permission_service(&self) -> Result<(), Box<dyn std::error::Error>> {
        info!("🔧 初始化权限服务...");

        // 将权限服务向下转型为具体类型以调用 init_from_database 方法
        if let Some(concrete_service) = self
            .solana_permission
            .as_any()
            .downcast_ref::<SolanaPermissionService>()
        {
            match concrete_service.init_from_database().await {
                Ok(_) => {
                    info!("✅ 权限服务初始化完成");
                    Ok(())
                }
                Err(e) => {
                    error!("❌ 权限服务初始化失败: {}", e);
                    Err(format!("权限服务初始化失败: {}", e).into())
                }
            }
        } else {
            // 如果不是具体类型，说明是测试环境或其他实现，跳过数据库初始化
            info!("⚠️ 权限服务非数据库实现，跳过数据库初始化");
            Ok(())
        }
    }

    /// 初始化默认权限配置
    async fn init_default_permission_config(&self) -> Result<(), Box<dyn std::error::Error>> {
        info!("🔧 初始化默认权限配置...");

        match self.database.init_default_permission_config().await {
            Ok(_) => {
                info!("✅ 默认权限配置初始化完成");
                Ok(())
            }
            Err(e) => {
                error!("❌ 默认权限配置初始化失败: {}", e);
                Err(format!("权限配置初始化失败: {}", e).into())
            }
        }
    }

    /// 运行池子类型迁移，暂时不使用这个迁移，因为是新功能上线，无历史数据
    #[allow(dead_code)]
    async fn run_pool_type_migration(&self) -> Result<(), Box<dyn std::error::Error>> {
        info!("🔄 检查池子类型迁移状态...");

        let migration = PoolTypeMigration;

        // 获取MongoDB数据库实例
        let mongo_client = mongodb::Client::with_uri_str(
            &std::env::var("MONGO_URI").unwrap_or_else(|_| "mongodb://localhost:27017".to_string()),
        )
        .await?;
        let db_name = std::env::var("MONGO_DB").unwrap_or_else(|_| "coinfair".to_string());
        let mongo_db = mongo_client.database(&db_name);

        // 检查迁移状态
        match migration.is_migrated(&mongo_db).await {
            Ok(true) => {
                info!("✅ 池子类型迁移已完成，跳过迁移");
            }
            Ok(false) => {
                info!("🔄 开始执行池子类型迁移...");
                match migration.migrate_up(&mongo_db).await {
                    Ok(_) => {
                        info!("✅ 池子类型迁移执行成功");
                    }
                    Err(e) => {
                        error!("❌ 池子类型迁移执行失败: {}", e);
                        return Err(format!("迁移失败: {}", e).into());
                    }
                }
            }
            Err(e) => {
                error!("❌ 检查迁移状态失败: {}", e);
                warn!("⚠️ 尝试执行迁移...");

                // 即使检查失败，也尝试执行迁移（迁移本身有幂等性保护）
                match migration.migrate_up(&mongo_db).await {
                    Ok(_) => {
                        info!("✅ 池子类型迁移执行成功");
                    }
                    Err(e) => {
                        error!("❌ 池子类型迁移执行失败: {}", e);
                        return Err(format!("迁移失败: {}", e).into());
                    }
                }
            }
        }

        // 获取迁移统计信息
        match migration.get_migration_stats(&mongo_db).await {
            Ok(stats) => {
                info!("📊 迁移统计信息:");
                info!("  总池子数: {}", stats.total_pools);
                info!("  已迁移池子数: {}", stats.pools_with_type);
                info!("  未迁移池子数: {}", stats.pools_without_type);
                info!("  集中流动性池子数: {}", stats.concentrated_pools);
                info!("  标准池子数: {}", stats.standard_pools);
                info!("  迁移完成状态: {}", stats.migration_complete);
            }
            Err(e) => {
                warn!("⚠️ 获取迁移统计信息失败: {}", e);
            }
        }

        Ok(())
    }

    /// 初始化CLMM池子存储服务索引
    async fn init_clmm_pool_indexes(&self) -> Result<(), Box<dyn std::error::Error>> {
        info!("🔧 初始化CLMM池子数据库索引...");

        // 直接使用数据库连接来初始化索引
        let repository = database::clmm::clmm_pool::ClmmPoolRepository::new(self.database.clmm_pools.clone());

        match repository.init_indexes().await {
            Ok(_) => {
                info!("✅ CLMM池子数据库索引初始化完成");
            }
            Err(e) => {
                error!("❌ CLMM池子数据库索引初始化失败: {}", e);
                return Err(format!("索引初始化失败: {}", e).into());
            }
        }

        Ok(())
    }

    /// 初始化Position存储服务索引
    async fn init_position_indexes(&self) -> Result<(), Box<dyn std::error::Error>> {
        info!("🔧 初始化Position数据库索引...");

        // 使用Database实例直接调用init_indexes方法
        match self.database.as_ref().init_indexes().await {
            Ok(_) => {
                info!("✅ Position数据库索引初始化完成");
            }
            Err(e) => {
                error!("❌ Position数据库索引初始化失败: {}", e);
                return Err(format!("Position索引初始化失败: {}", e).into());
            }
        }

        Ok(())
    }

    /// 应用默认分页配置 目前没有使用这个全局配置
    #[allow(dead_code)]
    async fn apply_default_pagination_config(&self) -> Result<(), Box<dyn std::error::Error>> {
        info!("⚙️ 应用默认分页配置...");

        // 设置默认分页配置
        let default_config = DatabaseServiceConfig {
            default_page_size: 20,
            max_page_size: 100,
            default_sort_field: "created_at".to_string(),
            default_sort_direction: "desc".to_string(),
            enable_query_logging: true,
            query_timeout_seconds: 30,
        };

        info!("📋 默认分页配置:");
        info!("  默认页大小: {}", default_config.default_page_size);
        info!("  最大页大小: {}", default_config.max_page_size);
        info!("  默认排序字段: {}", default_config.default_sort_field);
        info!("  默认排序方向: {}", default_config.default_sort_direction);
        info!("  启用查询日志: {}", default_config.enable_query_logging);
        info!("  查询超时时间: {}秒", default_config.query_timeout_seconds);

        // 在实际应用中，这些配置可以存储在配置文件或环境变量中
        // 这里我们只是记录配置信息，实际的分页逻辑在repository中实现

        info!("✅ 默认分页配置应用完成");
        Ok(())
    }

    /// 获取数据库服务健康状态
    pub async fn get_database_health(&self) -> Result<DatabaseHealthStatus, Box<dyn std::error::Error>> {
        info!("🔍 检查数据库服务健康状态...");

        let repository = database::clmm::clmm_pool::ClmmPoolRepository::new(self.database.clmm_pools.clone());

        // 执行基本的数据库操作来检查健康状态
        let start_time = std::time::Instant::now();

        match repository.get_pool_stats().await {
            Ok(stats) => {
                let response_time = start_time.elapsed();

                let health_status = DatabaseHealthStatus {
                    is_healthy: true,
                    response_time_ms: response_time.as_millis() as u64,
                    total_pools: stats.total_pools,
                    active_pools: stats.active_pools,
                    issues: Vec::new(),
                    last_check: chrono::Utc::now().timestamp() as u64,
                };

                info!("✅ 数据库服务健康状态良好");
                info!("  响应时间: {}ms", health_status.response_time_ms);
                info!("  总池子数: {}", health_status.total_pools);
                info!("  活跃池子数: {}", health_status.active_pools);

                Ok(health_status)
            }
            Err(e) => {
                let response_time = start_time.elapsed();

                let health_status = DatabaseHealthStatus {
                    is_healthy: false,
                    response_time_ms: response_time.as_millis() as u64,
                    total_pools: 0,
                    active_pools: 0,
                    issues: vec![format!("数据库查询失败: {}", e)],
                    last_check: chrono::Utc::now().timestamp() as u64,
                };

                error!("❌ 数据库服务健康检查失败: {}", e);
                Ok(health_status)
            }
        }
    }
}

/// 数据库服务配置
#[derive(Debug, Clone)]
pub struct DatabaseServiceConfig {
    /// 默认页大小
    pub default_page_size: u64,
    /// 最大页大小
    pub max_page_size: u64,
    /// 默认排序字段
    pub default_sort_field: String,
    /// 默认排序方向
    pub default_sort_direction: String,
    /// 是否启用查询日志
    pub enable_query_logging: bool,
    /// 查询超时时间（秒）
    pub query_timeout_seconds: u64,
}

/// 数据库健康状态
#[derive(Debug, Clone)]
pub struct DatabaseHealthStatus {
    /// 是否健康
    pub is_healthy: bool,
    /// 响应时间（毫秒）
    pub response_time_ms: u64,
    /// 总池子数
    pub total_pools: u64,
    /// 活跃池子数
    pub active_pools: u64,
    /// 问题列表
    pub issues: Vec<String>,
    /// 最后检查时间
    pub last_check: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_database_service_config_creation() {
        let config = DatabaseServiceConfig {
            default_page_size: 20,
            max_page_size: 100,
            default_sort_field: "created_at".to_string(),
            default_sort_direction: "desc".to_string(),
            enable_query_logging: true,
            query_timeout_seconds: 30,
        };

        assert_eq!(config.default_page_size, 20);
        assert_eq!(config.max_page_size, 100);
        assert_eq!(config.default_sort_field, "created_at");
        assert_eq!(config.default_sort_direction, "desc");
        assert!(config.enable_query_logging);
        assert_eq!(config.query_timeout_seconds, 30);
    }

    #[test]
    fn test_database_health_status_creation() {
        let health_status = DatabaseHealthStatus {
            is_healthy: true,
            response_time_ms: 150,
            total_pools: 100,
            active_pools: 80,
            issues: Vec::new(),
            last_check: 1640995200,
        };

        assert!(health_status.is_healthy);
        assert_eq!(health_status.response_time_ms, 150);
        assert_eq!(health_status.total_pools, 100);
        assert_eq!(health_status.active_pools, 80);
        assert!(health_status.issues.is_empty());
        assert_eq!(health_status.last_check, 1640995200);
    }

    #[test]
    fn test_database_health_status_with_issues() {
        let health_status = DatabaseHealthStatus {
            is_healthy: false,
            response_time_ms: 5000,
            total_pools: 0,
            active_pools: 0,
            issues: vec!["Database connection timeout".to_string(), "Index missing".to_string()],
            last_check: 1640995200,
        };

        assert!(!health_status.is_healthy);
        assert_eq!(health_status.response_time_ms, 5000);
        assert_eq!(health_status.total_pools, 0);
        assert_eq!(health_status.active_pools, 0);
        assert_eq!(health_status.issues.len(), 2);
        assert_eq!(health_status.issues[0], "Database connection timeout");
        assert_eq!(health_status.issues[1], "Index missing");
    }
}
