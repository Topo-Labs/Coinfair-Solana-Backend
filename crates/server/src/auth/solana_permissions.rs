use crate::auth::{Permission, UserTier};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use utoipa::ToSchema;

/// Solana API 权限操作类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, ToSchema)]
pub enum SolanaApiAction {
    /// 读取操作
    Read,
    /// 写入操作  
    Write,
}

/// Solana API 权限策略
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub enum SolanaPermissionPolicy {
    /// 允许访问
    Allow,
    /// 拒绝访问
    Deny,
    /// 需要特定权限
    RequirePermission(Permission),
    /// 需要最低用户等级
    RequireMinTier(UserTier),
    /// 需要权限和等级
    RequirePermissionAndTier(Permission, UserTier),
}

/// Solana API 端点权限配置
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SolanaApiPermissionConfig {
    /// API 端点路径
    pub endpoint: String,
    /// API 名称/描述
    pub name: String,
    /// API 分类
    pub category: String,
    /// 读取权限策略
    pub read_policy: SolanaPermissionPolicy,
    /// 写入权限策略
    pub write_policy: SolanaPermissionPolicy,
    /// 是否启用（个别控制）
    pub enabled: bool,
    /// 创建时间
    pub created_at: u64,
    /// 更新时间
    pub updated_at: u64,
}

/// 全局 Solana 权限控制配置
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct GlobalSolanaPermissionConfig {
    /// 全局读取权限开关
    pub global_read_enabled: bool,
    /// 全局写入权限开关
    pub global_write_enabled: bool,
    /// 默认读取权限策略
    pub default_read_policy: SolanaPermissionPolicy,
    /// 默认写入权限策略
    pub default_write_policy: SolanaPermissionPolicy,
    /// 紧急停用开关（优先级最高）
    pub emergency_shutdown: bool,
    /// 维护模式（只允许管理员访问）
    pub maintenance_mode: bool,
    /// 配置版本
    pub version: u64,
    /// 最后更新时间
    pub last_updated: u64,
    /// 更新者
    pub updated_by: String,
}

impl Default for GlobalSolanaPermissionConfig {
    fn default() -> Self {
        Self {
            global_read_enabled: true,
            global_write_enabled: true,
            default_read_policy: SolanaPermissionPolicy::RequirePermission(Permission::ReadPool),
            default_write_policy: SolanaPermissionPolicy::RequirePermission(Permission::CreatePosition),
            emergency_shutdown: false,
            maintenance_mode: false,
            version: 1,
            last_updated: chrono::Utc::now().timestamp() as u64,
            updated_by: "system".to_string(),
        }
    }
}

/// Solana 权限配置管理器
#[derive(Debug, Clone)]
pub struct SolanaPermissionManager {
    /// 全局配置
    global_config: GlobalSolanaPermissionConfig,
    /// API 端点权限配置映射
    api_configs: HashMap<String, SolanaApiPermissionConfig>,
    /// 权限策略缓存
    policy_cache: HashMap<String, (SolanaPermissionPolicy, SolanaPermissionPolicy)>,
}

impl SolanaPermissionManager {
    /// 创建新的权限管理器
    pub fn new() -> Self {
        let mut manager = Self {
            global_config: GlobalSolanaPermissionConfig::default(),
            api_configs: HashMap::new(),
            policy_cache: HashMap::new(),
        };

        // 初始化默认API配置
        manager.initialize_default_api_configs();
        manager.rebuild_cache();
        manager
    }

    /// 初始化默认的 Solana API 权限配置
    fn initialize_default_api_configs(&mut self) {
        let apis = vec![
            // 交换相关 API
            (
                "/api/v1/solana/swap",
                "代币交换",
                "交换",
                SolanaPermissionPolicy::RequirePermission(Permission::ReadPool),
                SolanaPermissionPolicy::RequirePermissionAndTier(Permission::CreatePosition, UserTier::Basic),
            ),
            ("/api/v1/solana/quote", "价格报价", "交换", SolanaPermissionPolicy::Allow, SolanaPermissionPolicy::Deny),
            ("/api/v1/solana/balance", "余额查询", "查询", SolanaPermissionPolicy::Allow, SolanaPermissionPolicy::Deny),
            // 仓位相关 API
            (
                "/api/v1/solana/position/open",
                "开仓",
                "仓位",
                SolanaPermissionPolicy::RequirePermission(Permission::ReadPosition),
                SolanaPermissionPolicy::RequirePermissionAndTier(Permission::CreatePosition, UserTier::Premium),
            ),
            (
                "/api/v1/solana/position/open-and-send-transaction",
                "开仓并发送交易",
                "仓位",
                SolanaPermissionPolicy::RequirePermission(Permission::ReadPosition),
                SolanaPermissionPolicy::RequirePermissionAndTier(Permission::CreatePosition, UserTier::Premium),
            ),
            (
                "/api/v1/solana/position/increase-liquidity",
                "增加流动性",
                "仓位",
                SolanaPermissionPolicy::RequirePermission(Permission::ReadPosition),
                SolanaPermissionPolicy::RequirePermissionAndTier(Permission::CreatePosition, UserTier::Basic),
            ),
            (
                "/api/v1/solana/position/decrease-liquidity",
                "减少流动性",
                "仓位",
                SolanaPermissionPolicy::RequirePermission(Permission::ReadPosition),
                SolanaPermissionPolicy::RequirePermissionAndTier(Permission::CreatePosition, UserTier::Basic),
            ),
            (
                "/api/v1/solana/position/list",
                "仓位列表",
                "查询",
                SolanaPermissionPolicy::RequirePermission(Permission::ReadPosition),
                SolanaPermissionPolicy::Deny,
            ),
            (
                "/api/v1/solana/position/info",
                "仓位信息",
                "查询",
                SolanaPermissionPolicy::RequirePermission(Permission::ReadPosition),
                SolanaPermissionPolicy::Deny,
            ),
            // 池子相关 API
            (
                "/api/v1/solana/pool/create/clmm",
                "创建CLMM池",
                "池子",
                SolanaPermissionPolicy::RequirePermission(Permission::ReadPool),
                SolanaPermissionPolicy::RequirePermissionAndTier(Permission::CreatePool, UserTier::VIP),
            ),
            (
                "/api/v1/solana/pool/create/cpmm",
                "创建CPMM池",
                "池子",
                SolanaPermissionPolicy::RequirePermission(Permission::ReadPool),
                SolanaPermissionPolicy::RequirePermissionAndTier(Permission::CreatePool, UserTier::VIP),
            ),
            (
                "/api/v1/solana/pools/info/list",
                "池子列表",
                "查询",
                SolanaPermissionPolicy::Allow,
                SolanaPermissionPolicy::Deny,
            ),
            (
                "/api/v1/solana/pools/info/mint",
                "按代币对查询池子",
                "查询",
                SolanaPermissionPolicy::Allow,
                SolanaPermissionPolicy::Deny,
            ),
            (
                "/api/v1/solana/pools/info/ids",
                "按ID查询池子",
                "查询",
                SolanaPermissionPolicy::Allow,
                SolanaPermissionPolicy::Deny,
            ),
            (
                "/api/v1/solana/pools/key/ids",
                "池子密钥信息",
                "查询",
                SolanaPermissionPolicy::RequirePermission(Permission::ReadPool),
                SolanaPermissionPolicy::Deny,
            ),
            // 流动性相关 API
            (
                "/api/v1/solana/pools/line/*",
                "流动性分布图",
                "查询",
                SolanaPermissionPolicy::Allow,
                SolanaPermissionPolicy::Deny,
            ),
            // 配置相关 API
            (
                "/api/v1/solana/main/clmm-config/*",
                "CLMM配置",
                "配置",
                SolanaPermissionPolicy::Allow,
                SolanaPermissionPolicy::RequirePermissionAndTier(Permission::AdminConfig, UserTier::Admin),
            ),
            // 静态配置 API
            (
                "/api/v1/solana/main/version",
                "版本信息",
                "配置",
                SolanaPermissionPolicy::Allow,
                SolanaPermissionPolicy::Deny,
            ),
            (
                "/api/v1/solana/main/auto-fee",
                "自动手续费",
                "配置",
                SolanaPermissionPolicy::Allow,
                SolanaPermissionPolicy::Deny,
            ),
            ("/api/v1/solana/main/rpcs", "RPC信息", "配置", SolanaPermissionPolicy::Allow, SolanaPermissionPolicy::Deny),
            (
                "/api/v1/solana/main/chain-time",
                "链时间",
                "配置",
                SolanaPermissionPolicy::Allow,
                SolanaPermissionPolicy::Deny,
            ),
            ("/api/v1/solana/mint/list", "代币列表", "配置", SolanaPermissionPolicy::Allow, SolanaPermissionPolicy::Deny),
        ];

        let now = chrono::Utc::now().timestamp() as u64;

        for (endpoint, name, category, read_policy, write_policy) in apis {
            let config = SolanaApiPermissionConfig {
                endpoint: endpoint.to_string(),
                name: name.to_string(),
                category: category.to_string(),
                read_policy,
                write_policy,
                enabled: true,
                created_at: now,
                updated_at: now,
            };

            self.api_configs.insert(endpoint.to_string(), config);
        }
    }

    /// 重建权限策略缓存
    fn rebuild_cache(&mut self) {
        self.policy_cache.clear();
        for (endpoint, config) in &self.api_configs {
            self.policy_cache.insert(endpoint.clone(), (config.read_policy.clone(), config.write_policy.clone()));
        }
    }

    /// 检查 API 权限（增强版 - 明确全局优先级）
    pub fn check_api_permission(
        &self,
        endpoint: &str,
        action: &SolanaApiAction,
        user_permissions: &std::collections::HashSet<Permission>,
        user_tier: &UserTier,
    ) -> Result<(), String> {
        // 1. 检查紧急停用开关（最高优先级）
        if self.global_config.emergency_shutdown {
            return Err("系统紧急停用中".to_string());
        }

        // 2. 检查维护模式
        if self.global_config.maintenance_mode && *user_tier != UserTier::Admin {
            return Err("系统维护模式，仅管理员可访问".to_string());
        }

        // 3. 检查全局开关（优先级高于具体API配置）
        let global_permission_result = self.check_global_permission(action);
        if let Err(global_error) = global_permission_result {
            // 全局权限被拒绝，直接返回，不检查局部配置
            return Err(global_error);
        }

        // 4. 如果全局权限允许，检查具体 API 配置和权限策略
        let policy = if let Some(config) = self.get_api_config(endpoint) {
            // 检查具体API是否被禁用
            if !config.enabled {
                return Err(format!("API {} 已禁用", endpoint));
            }

            match action {
                SolanaApiAction::Read => &config.read_policy,
                SolanaApiAction::Write => &config.write_policy,
            }
        } else {
            // 使用默认策略
            match action {
                SolanaApiAction::Read => &self.global_config.default_read_policy,
                SolanaApiAction::Write => &self.global_config.default_write_policy,
            }
        };

        // 5. 检查具体权限策略
        self.check_permission_policy(policy, user_permissions, user_tier)
    }

    /// 检查全局权限开关
    fn check_global_permission(&self, action: &SolanaApiAction) -> Result<(), String> {
        match action {
            SolanaApiAction::Read => {
                if !self.global_config.global_read_enabled {
                    return Err("全局读取权限已关闭".to_string());
                }
            }
            SolanaApiAction::Write => {
                if !self.global_config.global_write_enabled {
                    return Err("全局写入权限已关闭".to_string());
                }
            }
        }
        Ok(())
    }

    /// 检查权限策略
    pub fn check_permission_policy(&self, policy: &SolanaPermissionPolicy, user_permissions: &std::collections::HashSet<Permission>, user_tier: &UserTier) -> Result<(), String> {
        match policy {
            SolanaPermissionPolicy::Allow => Ok(()),
            SolanaPermissionPolicy::Deny => Err("操作被拒绝".to_string()),
            SolanaPermissionPolicy::RequirePermission(required_perm) => {
                if user_permissions.contains(required_perm) || *user_tier == UserTier::Admin {
                    Ok(())
                } else {
                    Err(format!("缺少必需权限: {:?}", required_perm))
                }
            }
            SolanaPermissionPolicy::RequireMinTier(min_tier) => {
                if self.user_tier_meets_minimum(user_tier, min_tier) {
                    Ok(())
                } else {
                    Err(format!("用户等级不足，需要至少: {:?}", min_tier))
                }
            }
            SolanaPermissionPolicy::RequirePermissionAndTier(required_perm, min_tier) => {
                if *user_tier == UserTier::Admin {
                    return Ok(());
                }

                if !user_permissions.contains(required_perm) {
                    return Err(format!("缺少必需权限: {:?}", required_perm));
                }

                if !self.user_tier_meets_minimum(user_tier, min_tier) {
                    return Err(format!("用户等级不足，需要至少: {:?}", min_tier));
                }

                Ok(())
            }
        }
    }

    /// 检查用户等级是否满足最低要求
    pub fn user_tier_meets_minimum(&self, user_tier: &UserTier, min_tier: &UserTier) -> bool {
        let tier_level = match user_tier {
            UserTier::Basic => 0,
            UserTier::Premium => 1,
            UserTier::VIP => 2,
            UserTier::Admin => 3,
        };

        let min_level = match min_tier {
            UserTier::Basic => 0,
            UserTier::Premium => 1,
            UserTier::VIP => 2,
            UserTier::Admin => 3,
        };

        tier_level >= min_level
    }

    /// 获取 API 配置
    pub fn get_api_config(&self, endpoint: &str) -> Option<&SolanaApiPermissionConfig> {
        // 首先尝试精确匹配
        if let Some(config) = self.api_configs.get(endpoint) {
            return Some(config);
        }

        // 尝试模式匹配
        for (pattern, config) in &self.api_configs {
            if self.matches_endpoint_pattern(endpoint, pattern) {
                return Some(config);
            }
        }

        None
    }

    /// 端点模式匹配
    fn matches_endpoint_pattern(&self, endpoint: &str, pattern: &str) -> bool {
        if pattern.ends_with("/*") {
            let prefix = &pattern[..pattern.len() - 2];
            endpoint.starts_with(prefix)
        } else if pattern.contains("*") {
            // 简单的通配符匹配
            let parts: Vec<&str> = pattern.split('*').collect();
            if parts.len() == 2 {
                endpoint.starts_with(parts[0]) && endpoint.ends_with(parts[1])
            } else {
                false
            }
        } else {
            endpoint == pattern
        }
    }

    /// 更新全局配置
    pub fn update_global_config(&mut self, config: GlobalSolanaPermissionConfig) {
        self.global_config = config;
        self.global_config.version += 1;
        self.global_config.last_updated = chrono::Utc::now().timestamp() as u64;
    }

    /// 更新 API 配置
    pub fn update_api_config(&mut self, endpoint: String, config: SolanaApiPermissionConfig) {
        let mut updated_config = config;
        updated_config.updated_at = chrono::Utc::now().timestamp() as u64;
        self.api_configs.insert(endpoint.clone(), updated_config.clone());

        // 更新缓存
        self.policy_cache.insert(endpoint, (updated_config.read_policy, updated_config.write_policy));
    }

    /// 批量更新 API 配置
    pub fn batch_update_api_configs(&mut self, configs: HashMap<String, SolanaApiPermissionConfig>) {
        let now = chrono::Utc::now().timestamp() as u64;

        for (endpoint, mut config) in configs {
            config.updated_at = now;
            self.api_configs.insert(endpoint.clone(), config.clone());

            // 更新缓存
            self.policy_cache.insert(endpoint, (config.read_policy, config.write_policy));
        }
    }

    /// 获取全局配置
    pub fn get_global_config(&self) -> &GlobalSolanaPermissionConfig {
        &self.global_config
    }

    /// 获取所有 API 配置
    pub fn get_all_api_configs(&self) -> &HashMap<String, SolanaApiPermissionConfig> {
        &self.api_configs
    }

    /// 一键启用/禁用所有读取权限
    pub fn toggle_global_read(&mut self, enabled: bool) {
        self.global_config.global_read_enabled = enabled;
        self.global_config.version += 1;
        self.global_config.last_updated = chrono::Utc::now().timestamp() as u64;
    }

    /// 一键启用/禁用所有写入权限
    pub fn toggle_global_write(&mut self, enabled: bool) {
        self.global_config.global_write_enabled = enabled;
        self.global_config.version += 1;
        self.global_config.last_updated = chrono::Utc::now().timestamp() as u64;
    }

    /// 紧急停用所有 Solana API
    pub fn emergency_shutdown(&mut self, shutdown: bool) {
        self.global_config.emergency_shutdown = shutdown;
        self.global_config.version += 1;
        self.global_config.last_updated = chrono::Utc::now().timestamp() as u64;
    }

    /// 切换维护模式
    pub fn toggle_maintenance_mode(&mut self, maintenance: bool) {
        self.global_config.maintenance_mode = maintenance;
        self.global_config.version += 1;
        self.global_config.last_updated = chrono::Utc::now().timestamp() as u64;
    }
}

impl Default for SolanaPermissionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn test_solana_permission_manager_creation() {
        let manager = SolanaPermissionManager::new();

        assert!(manager.global_config.global_read_enabled);
        assert!(manager.global_config.global_write_enabled);
        assert!(!manager.global_config.emergency_shutdown);
        assert!(!manager.api_configs.is_empty());
    }

    #[test]
    fn test_global_permission_toggle() {
        let mut manager = SolanaPermissionManager::new();

        // 测试全局读取权限开关
        manager.toggle_global_read(false);
        assert!(!manager.global_config.global_read_enabled);

        // 测试全局写入权限开关
        manager.toggle_global_write(false);
        assert!(!manager.global_config.global_write_enabled);
    }

    #[test]
    fn test_emergency_shutdown() {
        let mut manager = SolanaPermissionManager::new();
        let mut user_perms = HashSet::new();
        user_perms.insert(Permission::ReadPool);
        // 先恢复使用，如果当前状态处于紧急停用状态
        manager.emergency_shutdown(false);

        // 正常情况下应该允许
        let result = manager.check_api_permission("/api/v1/solana/pools/info/list", &SolanaApiAction::Read, &user_perms, &UserTier::Basic);
        assert!(result.is_ok());

        // 紧急停用后应该拒绝
        manager.emergency_shutdown(true);
        let result = manager.check_api_permission("/api/v1/solana/pools/info/list", &SolanaApiAction::Read, &user_perms, &UserTier::Basic);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "系统紧急停用中");
    }

    #[test]
    fn test_maintenance_mode() {
        let mut manager = SolanaPermissionManager::new();
        let mut user_perms = HashSet::new();
        user_perms.insert(Permission::ReadPool);

        // 开启维护模式
        manager.toggle_maintenance_mode(true);

        // 普通用户应该被拒绝
        let result = manager.check_api_permission("/api/v1/solana/pools/info/list", &SolanaApiAction::Read, &user_perms, &UserTier::Basic);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "系统维护模式，仅管理员可访问");

        // 管理员应该可以访问
        let result = manager.check_api_permission("/api/v1/solana/pools/info/list", &SolanaApiAction::Read, &user_perms, &UserTier::Admin);
        assert!(result.is_ok());
    }

    #[test]
    fn test_permission_policy_checking() {
        let manager = SolanaPermissionManager::new();
        let mut basic_perms = HashSet::new();
        basic_perms.insert(Permission::ReadPool);

        let mut premium_perms = HashSet::new();
        premium_perms.insert(Permission::ReadPool);
        premium_perms.insert(Permission::CreatePosition);

        // 测试读取权限 - 应该允许 (Allow policy)
        let result = manager.check_api_permission("/api/v1/solana/pools/info/list", &SolanaApiAction::Read, &basic_perms, &UserTier::Basic);
        assert!(result.is_ok());

        // 测试交换写入权限 - 需要 CreatePosition 权限和 Basic 等级
        let result = manager.check_api_permission(
            "/api/v1/solana/swap",
            &SolanaApiAction::Write,
            &basic_perms, // 缺少 CreatePosition 权限
            &UserTier::Basic,
        );
        assert!(result.is_err());

        // 有正确权限应该成功
        let result = manager.check_api_permission(
            "/api/v1/solana/swap",
            &SolanaApiAction::Write,
            &premium_perms, // 有 CreatePosition 权限
            &UserTier::Basic,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_endpoint_pattern_matching() {
        let manager = SolanaPermissionManager::new();

        // 测试通配符匹配
        assert!(manager.matches_endpoint_pattern("/api/v1/solana/pools/line/position", "/api/v1/solana/pools/line/*"));
        assert!(manager.matches_endpoint_pattern("/api/v1/solana/main/clmm-config/list", "/api/v1/solana/main/clmm-config/*"));

        // 测试精确匹配
        assert!(manager.matches_endpoint_pattern("/api/v1/solana/swap", "/api/v1/solana/swap"));

        // 测试不匹配
        assert!(!manager.matches_endpoint_pattern("/api/v1/solana/swap", "/api/v1/solana/position/*"));
    }
}
