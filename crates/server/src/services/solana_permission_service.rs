use crate::auth::{
    AuthUser, GlobalSolanaPermissionConfig, SolanaApiAction, SolanaApiPermissionConfig, SolanaPermissionManager,
    SolanaPermissionPolicy,
};
use anyhow::Result;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tracing::{error, info, warn};

/// Solana 权限服务接口
#[async_trait::async_trait]
pub trait SolanaPermissionServiceTrait: Send + Sync {
    /// 类型转换支持
    fn as_any(&self) -> &dyn std::any::Any;

    /// 检查API权限
    async fn check_api_permission(
        &self,
        endpoint: &str,
        action: &SolanaApiAction,
        auth_user: &AuthUser,
    ) -> Result<(), String>;

    /// 更新全局配置
    async fn update_global_config(&self, config: GlobalSolanaPermissionConfig) -> Result<()>;

    /// 获取全局配置
    async fn get_global_config(&self) -> Result<GlobalSolanaPermissionConfig>;

    /// 更新API配置
    async fn update_api_config(&self, endpoint: String, config: SolanaApiPermissionConfig) -> Result<()>;

    /// 批量更新API配置
    async fn batch_update_api_configs(&self, configs: HashMap<String, SolanaApiPermissionConfig>) -> Result<()>;

    /// 获取所有API配置
    async fn get_all_api_configs(&self) -> Result<HashMap<String, SolanaApiPermissionConfig>>;

    /// 获取特定API配置
    async fn get_api_config(&self, endpoint: &str) -> Result<Option<SolanaApiPermissionConfig>>;

    /// 一键启用/禁用全局读取权限
    async fn toggle_global_read(&self, enabled: bool) -> Result<()>;

    /// 一键启用/禁用全局写入权限
    async fn toggle_global_write(&self, enabled: bool) -> Result<()>;

    /// 紧急停用所有Solana API
    async fn emergency_shutdown(&self, shutdown: bool) -> Result<()>;

    /// 切换维护模式
    async fn toggle_maintenance_mode(&self, maintenance: bool) -> Result<()>;

    /// 重载权限配置
    async fn reload_configuration(&self) -> Result<()>;

    /// 获取权限配置统计信息
    async fn get_permission_stats(&self) -> Result<PermissionStats>;
}

/// 权限统计信息
#[derive(Debug, Clone)]
pub struct PermissionStats {
    /// 总API数量
    pub total_apis: usize,
    /// 启用的API数量
    pub enabled_apis: usize,
    /// 禁用的API数量
    pub disabled_apis: usize,
    /// 全局读取权限状态
    pub global_read_enabled: bool,
    /// 全局写入权限状态
    pub global_write_enabled: bool,
    /// 紧急停用状态
    pub emergency_shutdown: bool,
    /// 维护模式状态
    pub maintenance_mode: bool,
    /// 配置版本
    pub config_version: u64,
    /// 最后更新时间
    pub last_updated: u64,
}

/// Solana 权限服务实现
#[derive(Clone)]
pub struct SolanaPermissionService {
    /// 权限管理器（使用读写锁保护）
    manager: Arc<RwLock<SolanaPermissionManager>>,
    /// 数据库引用（用于持久化配置）
    database: Option<Arc<database::Database>>,
}

impl SolanaPermissionService {
    /// 创建新的权限服务
    pub fn new() -> Self {
        Self {
            manager: Arc::new(RwLock::new(SolanaPermissionManager::new())),
            database: None,
        }
    }

    /// 创建带数据库的权限服务
    pub fn with_database(database: Arc<database::Database>) -> Self {
        Self {
            manager: Arc::new(RwLock::new(SolanaPermissionManager::new())),
            database: Some(database),
        }
    }

    /// 异步初始化权限服务（从数据库加载配置）
    pub async fn init_from_database(&self) -> Result<()> {
        if self.database.is_some() {
            info!("🔄 从数据库初始化权限配置...");
            self.load_from_database().await?;
            info!("✅ 权限服务数据库初始化完成");
        } else {
            info!("⚠️ 权限服务未连接数据库，使用默认内存配置");
        }
        Ok(())
    }

    /// 从数据库加载配置
    pub async fn load_from_database(&self) -> Result<()> {
        if let Some(db) = &self.database {
            info!("🔄 从数据库加载权限配置...");

            // 1. 加载全局配置
            if let Ok(global_configs) = db.global_permission_repository.find_global_config().await {
                if let Some(global_config_model) = global_configs.first() {
                    let global_config = self.convert_model_to_global_config(global_config_model)?;
                    let mut manager = self
                        .manager
                        .write()
                        .map_err(|e| anyhow::anyhow!("获取权限管理器写锁失败: {}", e))?;
                    manager.update_global_config(global_config);
                    info!("📥 已加载全局权限配置，版本: {}", global_config_model.version);
                }
            }

            // 2. 加载API配置
            if let Ok(api_config_models) = db.api_permission_repository.find_all_api_configs().await {
                let config_count = api_config_models.len();
                let mut api_configs = std::collections::HashMap::new();

                for model in api_config_models {
                    let api_config = self.convert_model_to_api_config(&model)?;
                    api_configs.insert(model.endpoint.clone(), api_config);
                }

                if !api_configs.is_empty() {
                    let mut manager = self
                        .manager
                        .write()
                        .map_err(|e| anyhow::anyhow!("获取权限管理器写锁失败: {}", e))?;
                    manager.batch_update_api_configs(api_configs);
                    info!("📥 已加载{}个API权限配置", config_count);
                }
            }

            info!("✅ 权限配置加载完成");
        } else {
            warn!("⚠️ 未配置数据库，使用默认权限配置");
        }
        Ok(())
    }

    /// 保存配置到数据库
    async fn save_to_database(&self) -> Result<()> {
        if let Some(db) = &self.database {
            // 1. 保存全局配置（先获取数据再释放锁）
            let global_config = {
                let manager = self
                    .manager
                    .read()
                    .map_err(|e| anyhow::anyhow!("获取权限管理器读锁失败: {}", e))?;
                manager.get_global_config().clone()
            };

            let global_config_model = self.convert_global_config_to_model(&global_config)?;

            if let Err(e) = db
                .global_permission_repository
                .upsert_global_config(global_config_model)
                .await
            {
                error!("保存全局权限配置失败: {}", e);
                return Err(anyhow::anyhow!("保存全局权限配置失败: {}", e));
            }

            // 2. 保存API配置（先获取数据再释放锁）
            let api_configs = {
                let manager = self
                    .manager
                    .read()
                    .map_err(|e| anyhow::anyhow!("获取权限管理器读锁失败: {}", e))?;
                manager.get_all_api_configs().clone()
            };

            for (endpoint, config) in api_configs {
                let api_config_model = self.convert_api_config_to_model(&config)?;
                if let Err(e) = db.api_permission_repository.upsert_api_config(api_config_model).await {
                    error!("保存API权限配置失败 [{}]: {}", endpoint, e);
                    // 继续保存其他配置，不中断整个流程
                }
            }

            info!("💾 权限配置已保存到数据库");
        }
        Ok(())
    }

    /// 转换数据库模型到全局配置
    fn convert_model_to_global_config(
        &self,
        model: &database::permission_config::GlobalSolanaPermissionConfigModel,
    ) -> Result<GlobalSolanaPermissionConfig> {
        let default_read_policy: SolanaPermissionPolicy = serde_json::from_str(&model.default_read_policy)
            .map_err(|e| anyhow::anyhow!("解析默认读取策略失败: {}", e))?;
        let default_write_policy: SolanaPermissionPolicy = serde_json::from_str(&model.default_write_policy)
            .map_err(|e| anyhow::anyhow!("解析默认写入策略失败: {}", e))?;

        Ok(GlobalSolanaPermissionConfig {
            global_read_enabled: model.global_read_enabled,
            global_write_enabled: model.global_write_enabled,
            default_read_policy,
            default_write_policy,
            emergency_shutdown: model.emergency_shutdown,
            maintenance_mode: model.maintenance_mode,
            version: model.version,
            last_updated: model.last_updated,
            updated_by: model.updated_by.clone(),
        })
    }

    /// 转换数据库模型到API配置
    fn convert_model_to_api_config(
        &self,
        model: &database::permission_config::SolanaApiPermissionConfigModel,
    ) -> Result<SolanaApiPermissionConfig> {
        let read_policy: SolanaPermissionPolicy =
            serde_json::from_str(&model.read_policy).map_err(|e| anyhow::anyhow!("解析读取策略失败: {}", e))?;
        let write_policy: SolanaPermissionPolicy =
            serde_json::from_str(&model.write_policy).map_err(|e| anyhow::anyhow!("解析写入策略失败: {}", e))?;

        Ok(SolanaApiPermissionConfig {
            endpoint: model.endpoint.clone(),
            name: model.name.clone(),
            category: model.category.clone(),
            read_policy,
            write_policy,
            enabled: model.enabled,
            created_at: model.created_at,
            updated_at: model.updated_at,
        })
    }

    /// 转换全局配置到数据库模型
    fn convert_global_config_to_model(
        &self,
        config: &GlobalSolanaPermissionConfig,
    ) -> Result<database::permission_config::GlobalSolanaPermissionConfigModel> {
        let default_read_policy = serde_json::to_string(&config.default_read_policy)
            .map_err(|e| anyhow::anyhow!("序列化默认读取策略失败: {}", e))?;
        let default_write_policy = serde_json::to_string(&config.default_write_policy)
            .map_err(|e| anyhow::anyhow!("序列化默认写入策略失败: {}", e))?;

        Ok(database::permission_config::GlobalSolanaPermissionConfigModel {
            id: None,
            config_type: "global".to_string(),
            global_read_enabled: config.global_read_enabled,
            global_write_enabled: config.global_write_enabled,
            default_read_policy,
            default_write_policy,
            emergency_shutdown: config.emergency_shutdown,
            maintenance_mode: config.maintenance_mode,
            version: config.version,
            last_updated: config.last_updated,
            updated_by: config.updated_by.clone(),
            created_at: chrono::Utc::now().timestamp() as u64,
        })
    }

    /// 转换API配置到数据库模型
    fn convert_api_config_to_model(
        &self,
        config: &SolanaApiPermissionConfig,
    ) -> Result<database::permission_config::SolanaApiPermissionConfigModel> {
        let read_policy =
            serde_json::to_string(&config.read_policy).map_err(|e| anyhow::anyhow!("序列化读取策略失败: {}", e))?;
        let write_policy =
            serde_json::to_string(&config.write_policy).map_err(|e| anyhow::anyhow!("序列化写入策略失败: {}", e))?;

        Ok(database::permission_config::SolanaApiPermissionConfigModel {
            id: None,
            endpoint: config.endpoint.clone(),
            name: config.name.clone(),
            category: config.category.clone(),
            read_policy,
            write_policy,
            enabled: config.enabled,
            created_at: config.created_at,
            updated_at: config.updated_at,
        })
    }

    /// 启用配置热重载功能
    pub async fn enable_hot_reload(&self, reload_interval_seconds: u64) -> Result<()> {
        if self.database.is_none() {
            return Err(anyhow::anyhow!("热重载需要数据库支持"));
        }

        let service_clone = self.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(reload_interval_seconds));

            loop {
                interval.tick().await;

                if let Err(e) = service_clone.load_from_database().await {
                    error!("热重载权限配置失败: {}", e);
                } else {
                    info!("🔄 权限配置热重载完成");
                }
            }
        });

        info!("🚀 权限配置热重载已启用，间隔: {}秒", reload_interval_seconds);
        Ok(())
    }

    /// 手动触发配置重载
    pub async fn reload_from_database(&self) -> Result<()> {
        info!("🔄 手动触发权限配置重载...");
        self.load_from_database().await?;
        info!("✅ 权限配置手动重载完成");
        Ok(())
    }

    /// 监听配置变更通知（基于文件系统监控或消息队列）
    pub async fn setup_config_change_listener(&self) -> Result<()> {
        // 可以实现基于文件系统监控或消息队列的配置变更通知
        // 这里提供一个基础的实现框架

        info!("🎧 设置权限配置变更监听器...");

        // TODO: 可以集成 notify crate 进行文件监控
        // TODO: 可以集成 Redis pub/sub 进行实时通知
        // TODO: 可以集成 webhook 进行远程通知

        info!("✅ 权限配置变更监听器设置完成");
        Ok(())
    }

    /// 验证权限配置的有效性
    fn validate_config(&self, config: &GlobalSolanaPermissionConfig) -> Result<()> {
        if config.version == 0 {
            return Err(anyhow::anyhow!("配置版本不能为0"));
        }

        if config.updated_by.is_empty() {
            return Err(anyhow::anyhow!("更新者不能为空"));
        }

        Ok(())
    }

    /// 记录权限检查日志
    fn log_permission_check(
        &self,
        endpoint: &str,
        action: &SolanaApiAction,
        user_id: &str,
        result: &Result<(), String>,
    ) {
        match result {
            Ok(_) => {
                info!("✅ 权限检查通过: 用户={} 端点={} 操作={:?}", user_id, endpoint, action);
            }
            Err(error) => {
                warn!(
                    "❌ 权限检查失败: 用户={} 端点={} 操作={:?} 原因={}",
                    user_id, endpoint, action, error
                );
            }
        }
    }
}

#[async_trait::async_trait]
impl SolanaPermissionServiceTrait for SolanaPermissionService {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn check_api_permission(
        &self,
        endpoint: &str,
        action: &SolanaApiAction,
        auth_user: &AuthUser,
    ) -> Result<(), String> {
        let result = {
            let manager = self
                .manager
                .read()
                .map_err(|e| format!("获取权限管理器读锁失败: {}", e))?;
            manager.check_api_permission(endpoint, action, &auth_user.permissions, &auth_user.tier)
        };

        // 记录权限检查日志
        self.log_permission_check(endpoint, action, &auth_user.user_id, &result);

        result
    }

    async fn update_global_config(&self, config: GlobalSolanaPermissionConfig) -> Result<()> {
        // 验证配置
        self.validate_config(&config)?;

        {
            let mut manager = self
                .manager
                .write()
                .map_err(|e| anyhow::anyhow!("获取权限管理器写锁失败: {}", e))?;
            manager.update_global_config(config);
        }

        // 保存到数据库
        self.save_to_database().await?;

        info!("✅ 全局权限配置已更新");
        Ok(())
    }

    async fn get_global_config(&self) -> Result<GlobalSolanaPermissionConfig> {
        let manager = self
            .manager
            .read()
            .map_err(|e| anyhow::anyhow!("获取权限管理器读锁失败: {}", e))?;
        Ok(manager.get_global_config().clone())
    }

    async fn update_api_config(&self, endpoint: String, config: SolanaApiPermissionConfig) -> Result<()> {
        {
            let mut manager = self
                .manager
                .write()
                .map_err(|e| anyhow::anyhow!("获取权限管理器写锁失败: {}", e))?;
            manager.update_api_config(endpoint.clone(), config);
        }

        // 保存到数据库
        self.save_to_database().await?;

        info!("✅ API权限配置已更新: {}", endpoint);
        Ok(())
    }

    async fn batch_update_api_configs(&self, configs: HashMap<String, SolanaApiPermissionConfig>) -> Result<()> {
        let count = configs.len();

        {
            let mut manager = self
                .manager
                .write()
                .map_err(|e| anyhow::anyhow!("获取权限管理器写锁失败: {}", e))?;
            manager.batch_update_api_configs(configs);
        }

        // 保存到数据库
        self.save_to_database().await?;

        info!("✅ 批量更新{}个API权限配置完成", count);
        Ok(())
    }

    async fn get_all_api_configs(&self) -> Result<HashMap<String, SolanaApiPermissionConfig>> {
        let manager = self
            .manager
            .read()
            .map_err(|e| anyhow::anyhow!("获取权限管理器读锁失败: {}", e))?;
        Ok(manager.get_all_api_configs().clone())
    }

    async fn get_api_config(&self, endpoint: &str) -> Result<Option<SolanaApiPermissionConfig>> {
        let manager = self
            .manager
            .read()
            .map_err(|e| anyhow::anyhow!("获取权限管理器读锁失败: {}", e))?;
        Ok(manager.get_api_config(endpoint).cloned())
    }

    async fn toggle_global_read(&self, enabled: bool) -> Result<()> {
        {
            let mut manager = self
                .manager
                .write()
                .map_err(|e| anyhow::anyhow!("获取权限管理器写锁失败: {}", e))?;
            manager.toggle_global_read(enabled);
        }

        // 保存到数据库
        self.save_to_database().await?;

        info!("✅ 全局读取权限已{}", if enabled { "启用" } else { "禁用" });
        Ok(())
    }

    async fn toggle_global_write(&self, enabled: bool) -> Result<()> {
        {
            let mut manager = self
                .manager
                .write()
                .map_err(|e| anyhow::anyhow!("获取权限管理器写锁失败: {}", e))?;
            manager.toggle_global_write(enabled);
        }

        // 保存到数据库
        self.save_to_database().await?;

        info!("✅ 全局写入权限已{}", if enabled { "启用" } else { "禁用" });
        Ok(())
    }

    async fn emergency_shutdown(&self, shutdown: bool) -> Result<()> {
        {
            let mut manager = self
                .manager
                .write()
                .map_err(|e| anyhow::anyhow!("获取权限管理器写锁失败: {}", e))?;
            manager.emergency_shutdown(shutdown);
        }

        // 保存到数据库
        self.save_to_database().await?;

        if shutdown {
            error!("🚨 紧急停用所有Solana API");
        } else {
            info!("✅ 紧急停用已解除");
        }
        Ok(())
    }

    async fn toggle_maintenance_mode(&self, maintenance: bool) -> Result<()> {
        {
            let mut manager = self
                .manager
                .write()
                .map_err(|e| anyhow::anyhow!("获取权限管理器写锁失败: {}", e))?;
            manager.toggle_maintenance_mode(maintenance);
        }

        // 保存到数据库
        self.save_to_database().await?;

        info!("✅ 维护模式已{}", if maintenance { "开启" } else { "关闭" });
        Ok(())
    }

    /// 重载权限配置
    async fn reload_configuration(&self) -> Result<()> {
        info!("🔄 重载权限配置...");

        // 从数据库重新加载配置
        self.reload_from_database().await?;

        info!("✅ 权限配置重载完成");
        Ok(())
    }

    async fn get_permission_stats(&self) -> Result<PermissionStats> {
        let manager = self
            .manager
            .read()
            .map_err(|e| anyhow::anyhow!("获取权限管理器读锁失败: {}", e))?;

        let global_config = manager.get_global_config();
        let api_configs = manager.get_all_api_configs();

        let total_apis = api_configs.len();
        let enabled_apis = api_configs.values().filter(|config| config.enabled).count();
        let disabled_apis = total_apis - enabled_apis;

        Ok(PermissionStats {
            total_apis,
            enabled_apis,
            disabled_apis,
            global_read_enabled: global_config.global_read_enabled,
            global_write_enabled: global_config.global_write_enabled,
            emergency_shutdown: global_config.emergency_shutdown,
            maintenance_mode: global_config.maintenance_mode,
            config_version: global_config.version,
            last_updated: global_config.last_updated,
        })
    }
}

impl Default for SolanaPermissionService {
    fn default() -> Self {
        Self::new()
    }
}

/// 权限服务的动态引用类型
pub type DynSolanaPermissionService = Arc<dyn SolanaPermissionServiceTrait>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::{Permission, UserTier};
    use std::collections::HashSet;

    fn create_test_auth_user() -> AuthUser {
        let mut permissions = HashSet::new();
        permissions.insert(Permission::ReadPool);
        permissions.insert(Permission::CreatePosition);

        AuthUser {
            user_id: "test_user".to_string(),
            wallet_address: Some("test_wallet".to_string()),
            tier: UserTier::Basic,
            permissions,
        }
    }

    #[tokio::test]
    async fn test_permission_service_creation() {
        let service = SolanaPermissionService::new();
        let stats = service.get_permission_stats().await.unwrap();

        assert!(stats.total_apis > 0);
        assert!(stats.global_read_enabled);
        assert!(stats.global_write_enabled);
        assert!(!stats.emergency_shutdown);
        assert!(!stats.maintenance_mode);
    }

    #[tokio::test]
    async fn test_global_permission_toggle() {
        let service = SolanaPermissionService::new();

        // 测试全局读取权限切换
        service.toggle_global_read(false).await.unwrap();
        let config = service.get_global_config().await.unwrap();
        assert!(!config.global_read_enabled);

        // 测试全局写入权限切换
        service.toggle_global_write(false).await.unwrap();
        let config = service.get_global_config().await.unwrap();
        assert!(!config.global_write_enabled);
    }

    #[tokio::test]
    async fn test_api_permission_check() {
        let service = SolanaPermissionService::new();
        let auth_user = create_test_auth_user();

        // 测试允许的操作
        let result = service
            .check_api_permission("/api/v1/solana/pools/info/list", &SolanaApiAction::Read, &auth_user)
            .await;
        assert!(result.is_ok());

        // 测试需要权限的操作
        let result = service
            .check_api_permission("/api/v1/solana/swap", &SolanaApiAction::Write, &auth_user)
            .await;
        assert!(result.is_ok()); // 用户有 CreatePosition 权限
    }

    #[tokio::test]
    async fn test_emergency_shutdown() {
        let service = SolanaPermissionService::new();
        let auth_user = create_test_auth_user();

        // 正常情况下应该允许
        let result = service
            .check_api_permission("/api/v1/solana/pools/info/list", &SolanaApiAction::Read, &auth_user)
            .await;
        assert!(result.is_ok());

        // 紧急停用后应该拒绝
        service.emergency_shutdown(true).await.unwrap();
        let result = service
            .check_api_permission("/api/v1/solana/pools/info/list", &SolanaApiAction::Read, &auth_user)
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_permission_stats() {
        let service = SolanaPermissionService::new();
        let stats = service.get_permission_stats().await.unwrap();

        assert!(stats.total_apis > 0);
        assert_eq!(stats.enabled_apis + stats.disabled_apis, stats.total_apis);
        assert!(stats.config_version > 0);
        assert!(stats.last_updated > 0);
    }
}
