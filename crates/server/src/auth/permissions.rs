use crate::auth::{Permission, UserTier};
use std::collections::{HashMap, HashSet};
use serde::{Deserialize, Serialize};

/// 权限管理器
pub struct PermissionManager {
    role_permissions: HashMap<String, HashSet<Permission>>,
    tier_permissions: HashMap<UserTier, HashSet<Permission>>,
    endpoint_permissions: HashMap<String, Vec<Permission>>,
}

impl PermissionManager {
    pub fn new() -> Self {
        let mut manager = Self {
            role_permissions: HashMap::new(),
            tier_permissions: HashMap::new(),
            endpoint_permissions: HashMap::new(),
        };
        
        manager.initialize_default_permissions();
        manager.initialize_endpoint_permissions();
        manager
    }

    /// 初始化默认的用户等级权限
    fn initialize_default_permissions(&mut self) {
        // Basic用户权限
        let basic_permissions: HashSet<Permission> = vec![
            Permission::ReadUser,
            Permission::ReadPool,
            Permission::ReadPosition,
            Permission::ReadReward,
        ].into_iter().collect();

        // Premium用户权限（包含Basic权限+创建权限）
        let mut premium_permissions = basic_permissions.clone();
        premium_permissions.extend(vec![
            Permission::CreateUser,
            Permission::CreatePosition,
        ]);

        // VIP用户权限（包含Premium权限+管理部分奖励）
        let mut vip_permissions = premium_permissions.clone();
        vip_permissions.extend(vec![
            Permission::CreatePool,
            Permission::ManageReward,
        ]);

        // Admin用户权限（所有权限）
        let admin_permissions = vec![
            Permission::ReadUser,
            Permission::ReadPool,
            Permission::ReadPosition,
            Permission::ReadReward,
            Permission::CreateUser,
            Permission::CreatePool,
            Permission::CreatePosition,
            Permission::ManageReward,
            Permission::AdminConfig,
            Permission::SystemMonitor,
            Permission::UserManagement,
        ].into_iter().collect();

        self.tier_permissions.insert(UserTier::Basic, basic_permissions);
        self.tier_permissions.insert(UserTier::Premium, premium_permissions);
        self.tier_permissions.insert(UserTier::VIP, vip_permissions);
        self.tier_permissions.insert(UserTier::Admin, admin_permissions);
    }

    /// 初始化API端点权限要求
    fn initialize_endpoint_permissions(&mut self) {
        // 用户相关端点
        self.endpoint_permissions.insert(
            "/api/v1/user/user/*".to_string(),
            vec![Permission::ReadUser]
        );
        self.endpoint_permissions.insert(
            "/api/v1/user/mock_users".to_string(),
            vec![Permission::CreateUser]
        );

        // 池子相关端点
        self.endpoint_permissions.insert(
            "/api/v1/solana/pools/*".to_string(),
            vec![Permission::ReadPool]
        );
        self.endpoint_permissions.insert(
            "/api/v1/solana/pools/create".to_string(),
            vec![Permission::CreatePool]
        );

        // 仓位相关端点
        self.endpoint_permissions.insert(
            "/api/v1/solana/positions/*".to_string(),
            vec![Permission::ReadPosition]
        );
        self.endpoint_permissions.insert(
            "/api/v1/solana/positions/create".to_string(),
            vec![Permission::CreatePosition]
        );

        // 奖励相关端点
        self.endpoint_permissions.insert(
            "/api/v1/reward/*".to_string(),
            vec![Permission::ReadReward]
        );
        self.endpoint_permissions.insert(
            "/api/v1/reward/manage".to_string(),
            vec![Permission::ManageReward]
        );

        // 管理端点
        self.endpoint_permissions.insert(
            "/api/v1/admin/config".to_string(),
            vec![Permission::AdminConfig]
        );
        self.endpoint_permissions.insert(
            "/api/v1/admin/monitor".to_string(),
            vec![Permission::SystemMonitor]
        );
        self.endpoint_permissions.insert(
            "/api/v1/admin/users".to_string(),
            vec![Permission::UserManagement]
        );

        // 交换相关端点（需要创建仓位权限）
        self.endpoint_permissions.insert(
            "/api/v1/solana/swap".to_string(),
            vec![Permission::CreatePosition]
        );
        self.endpoint_permissions.insert(
            "/api/v1/solana/swap/estimate".to_string(),
            vec![Permission::ReadPool]
        );
    }

    /// 获取用户等级的默认权限
    pub fn get_tier_permissions(&self, tier: &UserTier) -> HashSet<Permission> {
        self.tier_permissions.get(tier).cloned().unwrap_or_default()
    }

    /// 检查用户等级是否有指定权限
    pub fn tier_has_permission(&self, tier: &UserTier, permission: &Permission) -> bool {
        self.tier_permissions
            .get(tier)
            .map(|perms| perms.contains(permission))
            .unwrap_or(false)
    }

    /// 获取端点需要的权限
    pub fn get_endpoint_permissions(&self, endpoint: &str) -> Vec<Permission> {
        // 首先尝试精确匹配
        if let Some(permissions) = self.endpoint_permissions.get(endpoint) {
            return permissions.clone();
        }

        // 尝试模式匹配（支持通配符）
        for (pattern, permissions) in &self.endpoint_permissions {
            if self.matches_pattern(endpoint, pattern) {
                return permissions.clone();
            }
        }

        // 默认返回空权限（公开端点）
        vec![]
    }

    /// 检查端点模式匹配
    fn matches_pattern(&self, endpoint: &str, pattern: &str) -> bool {
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

    /// 添加自定义角色权限
    pub fn add_role_permissions(&mut self, role: String, permissions: HashSet<Permission>) {
        self.role_permissions.insert(role, permissions);
    }

    /// 获取角色权限
    pub fn get_role_permissions(&self, role: &str) -> HashSet<Permission> {
        self.role_permissions.get(role).cloned().unwrap_or_default()
    }

    /// 合并多个权限集合
    pub fn merge_permissions(&self, permission_sets: Vec<HashSet<Permission>>) -> HashSet<Permission> {
        let mut merged = HashSet::new();
        for set in permission_sets {
            merged.extend(set);
        }
        merged
    }

    /// 检查权限层级关系
    pub fn is_permission_hierarchy_valid(&self, user_permissions: &HashSet<Permission>, required: &Permission) -> bool {
        // 管理员权限可以访问所有资源
        if user_permissions.contains(&Permission::AdminConfig) ||
           user_permissions.contains(&Permission::SystemMonitor) ||
           user_permissions.contains(&Permission::UserManagement) {
            return true;
        }

        // 检查是否直接拥有所需权限
        if user_permissions.contains(required) {
            return true;
        }

        // 检查权限层级关系
        match required {
            Permission::ReadUser | Permission::ReadPool | Permission::ReadPosition | Permission::ReadReward => {
                // 读权限可以被对应的写权限覆盖
                match required {
                    Permission::ReadUser => user_permissions.contains(&Permission::CreateUser),
                    Permission::ReadPool => user_permissions.contains(&Permission::CreatePool),
                    Permission::ReadPosition => user_permissions.contains(&Permission::CreatePosition),
                    Permission::ReadReward => user_permissions.contains(&Permission::ManageReward),
                    _ => false,
                }
            }
            _ => false,
        }
    }
}

/// 权限策略定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionPolicy {
    pub name: String,
    pub description: String,
    pub permissions: Vec<Permission>,
    pub conditions: Vec<PolicyCondition>,
}

/// 权限策略条件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PolicyCondition {
    /// 时间限制（Unix时间戳）
    TimeRange { start: u64, end: u64 },
    /// IP地址限制
    IpWhitelist { ips: Vec<String> },
    /// 用户等级限制
    MinUserTier { tier: UserTier },
    /// 钱包地址限制
    WalletWhitelist { addresses: Vec<String> },
    /// 请求频率限制
    RateLimit { requests_per_minute: u32 },
}

/// 高级权限管理器
pub struct AdvancedPermissionManager {
    base_manager: PermissionManager,
    policies: HashMap<String, PermissionPolicy>,
}

impl AdvancedPermissionManager {
    pub fn new() -> Self {
        Self {
            base_manager: PermissionManager::new(),
            policies: HashMap::new(),
        }
    }

    /// 添加权限策略
    pub fn add_policy(&mut self, policy: PermissionPolicy) {
        self.policies.insert(policy.name.clone(), policy);
    }

    /// 检查用户是否满足策略条件
    pub fn check_policy_conditions(
        &self,
        conditions: &[PolicyCondition],
        user_tier: &UserTier,
        user_wallet: &Option<String>,
        client_ip: &str,
    ) -> bool {
        for condition in conditions {
            if !self.check_single_condition(condition, user_tier, user_wallet, client_ip) {
                return false;
            }
        }
        true
    }

    /// 检查单个策略条件
    fn check_single_condition(
        &self,
        condition: &PolicyCondition,
        user_tier: &UserTier,
        user_wallet: &Option<String>,
        client_ip: &str,
    ) -> bool {
        match condition {
            PolicyCondition::TimeRange { start, end } => {
                let now = chrono::Utc::now().timestamp() as u64;
                now >= *start && now <= *end
            }
            PolicyCondition::IpWhitelist { ips } => {
                ips.contains(&client_ip.to_string())
            }
            PolicyCondition::MinUserTier { tier } => {
                self.tier_meets_minimum(user_tier, tier)
            }
            PolicyCondition::WalletWhitelist { addresses } => {
                if let Some(wallet) = user_wallet {
                    addresses.contains(wallet)
                } else {
                    false
                }
            }
            PolicyCondition::RateLimit { requests_per_minute: _ } => {
                // 速率限制检查应该在速率限制中间件中处理
                true
            }
        }
    }

    /// 检查用户等级是否满足最低要求
    fn tier_meets_minimum(&self, user_tier: &UserTier, min_tier: &UserTier) -> bool {
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

    /// 获取基础权限管理器的引用
    pub fn base(&self) -> &PermissionManager {
        &self.base_manager
    }

    /// 获取策略
    pub fn get_policy(&self, name: &str) -> Option<&PermissionPolicy> {
        self.policies.get(name)
    }
}

impl Default for PermissionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for AdvancedPermissionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_manager_initialization() {
        let manager = PermissionManager::new();
        
        // 测试Basic用户权限
        let basic_perms = manager.get_tier_permissions(&UserTier::Basic);
        assert!(basic_perms.contains(&Permission::ReadUser));
        assert!(!basic_perms.contains(&Permission::CreateUser));

        // 测试Admin用户权限
        let admin_perms = manager.get_tier_permissions(&UserTier::Admin);
        assert!(admin_perms.contains(&Permission::AdminConfig));
        assert!(admin_perms.contains(&Permission::SystemMonitor));
    }

    #[test]
    fn test_endpoint_permission_matching() {
        let manager = PermissionManager::new();
        
        // 测试精确匹配
        let perms = manager.get_endpoint_permissions("/api/v1/user/mock_users");
        assert_eq!(perms, vec![Permission::CreateUser]);

        // 测试通配符匹配
        let perms = manager.get_endpoint_permissions("/api/v1/user/user/0x123");
        assert_eq!(perms, vec![Permission::ReadUser]);
    }

    #[test]
    fn test_permission_hierarchy() {
        let manager = PermissionManager::new();
        
        let mut user_permissions = HashSet::new();
        user_permissions.insert(Permission::CreateUser);
        
        // 创建权限应该包含读取权限
        assert!(manager.is_permission_hierarchy_valid(&user_permissions, &Permission::ReadUser));
        
        // 但不应该包含其他权限
        assert!(!manager.is_permission_hierarchy_valid(&user_permissions, &Permission::CreatePool));
    }

    #[test]
    fn test_advanced_permission_policies() {
        let mut manager = AdvancedPermissionManager::new();
        
        let policy = PermissionPolicy {
            name: "vip_trading".to_string(),
            description: "VIP trading permissions".to_string(),
            permissions: vec![Permission::CreatePosition, Permission::ManageReward],
            conditions: vec![
                PolicyCondition::MinUserTier { tier: UserTier::VIP },
                PolicyCondition::RateLimit { requests_per_minute: 100 },
            ],
        };
        
        manager.add_policy(policy);
        
        let retrieved_policy = manager.get_policy("vip_trading");
        assert!(retrieved_policy.is_some());
        assert_eq!(retrieved_policy.unwrap().permissions.len(), 2);
    }

    #[test]
    fn test_policy_conditions() {
        let manager = AdvancedPermissionManager::new();
        
        let conditions = vec![
            PolicyCondition::MinUserTier { tier: UserTier::Premium },
            PolicyCondition::IpWhitelist { ips: vec!["127.0.0.1".to_string()] },
        ];
        
        // 测试满足条件的情况
        assert!(manager.check_policy_conditions(
            &conditions,
            &UserTier::VIP,
            &Some("test_wallet".to_string()),
            "127.0.0.1"
        ));
        
        // 测试不满足条件的情况
        assert!(!manager.check_policy_conditions(
            &conditions,
            &UserTier::Basic,
            &Some("test_wallet".to_string()),
            "127.0.0.1"
        ));
    }
}