//! Solana权限服务专门测试模块
//! 重点测试一键启停和精细化权限配置功能

use crate::auth::{AuthUser, Permission, SolanaApiAction, SolanaApiPermissionConfig, SolanaPermissionPolicy, UserTier};
use crate::services::solana_permission_service::{SolanaPermissionService, SolanaPermissionServiceTrait};
use std::collections::{HashMap, HashSet};
use tokio;

/// 创建测试用的认证用户
fn create_test_auth_user(tier: UserTier, permissions: Vec<Permission>) -> AuthUser {
    let mut perm_set = HashSet::new();
    for perm in permissions {
        perm_set.insert(perm);
    }

    AuthUser {
        user_id: format!("user_{:?}", tier),
        wallet_address: Some("test_wallet_address".to_string()),
        tier,
        permissions: perm_set,
    }
}

#[cfg(test)]
mod solana_permission_service_tests {
    use super::*;

    #[tokio::test]
    async fn test_service_creation() {
        let service = SolanaPermissionService::new();
        let stats = service.get_permission_stats().await.unwrap();

        assert!(stats.total_apis > 0);
        assert!(stats.global_read_enabled);
        assert!(stats.global_write_enabled);
        assert!(!stats.emergency_shutdown);
        assert!(!stats.maintenance_mode);
    }

    #[tokio::test]
    async fn test_global_read_permission_toggle() {
        let service = SolanaPermissionService::new();
        let auth_user = create_test_auth_user(UserTier::Basic, vec![Permission::ReadPool]);

        // 初始状态应该允许读取
        let result = service.check_api_permission("/api/v1/solana/pools/info/list", &SolanaApiAction::Read, &auth_user).await;
        assert!(result.is_ok());

        // 一键关闭全局读取权限
        service.toggle_global_read(false).await.unwrap();

        // 所有读取操作都应该被拒绝
        let result = service.check_api_permission("/api/v1/solana/pools/info/list", &SolanaApiAction::Read, &auth_user).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("全局读取权限已关闭"));

        // 验证配置已更新
        let config = service.get_global_config().await.unwrap();
        assert!(!config.global_read_enabled);

        // 一键重新开启全局读取权限
        service.toggle_global_read(true).await.unwrap();

        // 读取操作应该恢复正常
        let result = service.check_api_permission("/api/v1/solana/pools/info/list", &SolanaApiAction::Read, &auth_user).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_global_write_permission_toggle() {
        let service = SolanaPermissionService::new();
        let auth_user = create_test_auth_user(UserTier::Basic, vec![Permission::CreatePosition]);

        // 初始状态应该允许写入
        let result = service.check_api_permission("/api/v1/solana/swap", &SolanaApiAction::Write, &auth_user).await;
        assert!(result.is_ok());

        // 一键关闭全局写入权限
        service.toggle_global_write(false).await.unwrap();

        // 所有写入操作都应该被拒绝
        let result = service.check_api_permission("/api/v1/solana/swap", &SolanaApiAction::Write, &auth_user).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("全局写入权限已关闭"));

        // 读取操作应该仍然可用
        let result = service.check_api_permission("/api/v1/solana/pools/info/list", &SolanaApiAction::Read, &auth_user).await;
        assert!(result.is_ok());

        // 验证配置已更新
        let config = service.get_global_config().await.unwrap();
        assert!(!config.global_write_enabled);
        assert!(config.global_read_enabled); // 读取权限应该不受影响

        // 一键重新开启全局写入权限
        service.toggle_global_write(true).await.unwrap();

        // 写入操作应该恢复正常
        let result = service.check_api_permission("/api/v1/solana/swap", &SolanaApiAction::Write, &auth_user).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_fine_grained_api_permission_configuration() {
        let service = SolanaPermissionService::new();

        // 创建一个自定义API配置
        let custom_config = SolanaApiPermissionConfig {
            endpoint: "/api/v1/solana/custom".to_string(),
            name: "自定义API".to_string(),
            category: "测试".to_string(),
            read_policy: SolanaPermissionPolicy::RequireMinTier(UserTier::Premium), // 读取需要Premium
            write_policy: SolanaPermissionPolicy::RequirePermissionAndTier(Permission::CreatePosition, UserTier::VIP), // 写入需要VIP+权限
            enabled: true,
            created_at: chrono::Utc::now().timestamp() as u64,
            updated_at: chrono::Utc::now().timestamp() as u64,
        };

        // 更新API配置
        service.update_api_config("/api/v1/solana/custom".to_string(), custom_config).await.unwrap();

        // 测试不同用户等级对该API的访问权限

        // Basic用户 - 应该无法读取
        let basic_user = create_test_auth_user(UserTier::Basic, vec![Permission::ReadPool]);
        let result = service.check_api_permission("/api/v1/solana/custom", &SolanaApiAction::Read, &basic_user).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("用户等级不足"));

        // Premium用户 - 应该可以读取但无法写入
        let premium_user = create_test_auth_user(UserTier::Premium, vec![Permission::CreatePosition]);
        let result = service.check_api_permission("/api/v1/solana/custom", &SolanaApiAction::Read, &premium_user).await;
        assert!(result.is_ok());

        let result = service.check_api_permission("/api/v1/solana/custom", &SolanaApiAction::Write, &premium_user).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("用户等级不足"));

        // VIP用户有权限 - 应该可以读取和写入
        let vip_user = create_test_auth_user(UserTier::VIP, vec![Permission::CreatePosition]);
        let result = service.check_api_permission("/api/v1/solana/custom", &SolanaApiAction::Read, &vip_user).await;
        assert!(result.is_ok());

        let result = service.check_api_permission("/api/v1/solana/custom", &SolanaApiAction::Write, &vip_user).await;
        assert!(result.is_ok());

        // VIP用户无权限 - 应该可以读取但无法写入
        let vip_user_no_perm = create_test_auth_user(UserTier::VIP, vec![Permission::ReadPool]);
        let result = service.check_api_permission("/api/v1/solana/custom", &SolanaApiAction::Read, &vip_user_no_perm).await;
        assert!(result.is_ok());

        let result = service.check_api_permission("/api/v1/solana/custom", &SolanaApiAction::Write, &vip_user_no_perm).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("缺少必需权限"));
    }

    #[tokio::test]
    async fn test_batch_api_configuration() {
        let service = SolanaPermissionService::new();

        // 批量配置多个API的权限策略
        let mut configs = HashMap::new();

        // 配置交换API为只允许VIP用户写入
        configs.insert(
            "/api/v1/solana/swap".to_string(),
            SolanaApiPermissionConfig {
                endpoint: "/api/v1/solana/swap".to_string(),
                name: "代币交换".to_string(),
                category: "交换".to_string(),
                read_policy: SolanaPermissionPolicy::Allow,                          // 读取无限制
                write_policy: SolanaPermissionPolicy::RequireMinTier(UserTier::VIP), // 写入需要VIP
                enabled: true,
                created_at: chrono::Utc::now().timestamp() as u64,
                updated_at: chrono::Utc::now().timestamp() as u64,
            },
        );

        // 配置仓位API为需要特定权限
        configs.insert(
            "/api/v1/solana/position/open".to_string(),
            SolanaApiPermissionConfig {
                endpoint: "/api/v1/solana/position/open".to_string(),
                name: "开仓".to_string(),
                category: "仓位".to_string(),
                read_policy: SolanaPermissionPolicy::RequirePermission(Permission::ReadPosition),
                write_policy: SolanaPermissionPolicy::RequirePermissionAndTier(Permission::CreatePosition, UserTier::Premium),
                enabled: true,
                created_at: chrono::Utc::now().timestamp() as u64,
                updated_at: chrono::Utc::now().timestamp() as u64,
            },
        );

        // 配置池子创建API为管理员专用
        configs.insert(
            "/api/v1/solana/pool/create/clmm".to_string(),
            SolanaApiPermissionConfig {
                endpoint: "/api/v1/solana/pool/create/clmm".to_string(),
                name: "创建CLMM池".to_string(),
                category: "池子".to_string(),
                read_policy: SolanaPermissionPolicy::RequireMinTier(UserTier::VIP),
                write_policy: SolanaPermissionPolicy::RequireMinTier(UserTier::Admin), // 只有管理员可以创建
                enabled: true,
                created_at: chrono::Utc::now().timestamp() as u64,
                updated_at: chrono::Utc::now().timestamp() as u64,
            },
        );

        // 批量更新配置
        service.batch_update_api_configs(configs).await.unwrap();

        // 验证配置生效
        let basic_user = create_test_auth_user(UserTier::Basic, vec![Permission::ReadPool]);
        let premium_user = create_test_auth_user(UserTier::Premium, vec![Permission::CreatePosition, Permission::ReadPosition]);
        let vip_user = create_test_auth_user(UserTier::VIP, vec![Permission::CreatePosition, Permission::ReadPosition]);
        let admin_user = create_test_auth_user(UserTier::Admin, vec![]);

        // 测试交换API - Basic用户可以读取但不能写入
        let result = service.check_api_permission("/api/v1/solana/swap", &SolanaApiAction::Read, &basic_user).await;
        assert!(result.is_ok());

        let result = service.check_api_permission("/api/v1/solana/swap", &SolanaApiAction::Write, &basic_user).await;
        assert!(result.is_err());

        // VIP用户可以写入
        let result = service.check_api_permission("/api/v1/solana/swap", &SolanaApiAction::Write, &vip_user).await;
        assert!(result.is_ok());

        // 测试仓位API - Premium用户有权限可以操作
        let result = service.check_api_permission("/api/v1/solana/position/open", &SolanaApiAction::Read, &premium_user).await;
        assert!(result.is_ok());

        let result = service.check_api_permission("/api/v1/solana/position/open", &SolanaApiAction::Write, &premium_user).await;
        assert!(result.is_ok());

        // Basic用户无权限读取仓位
        let result = service.check_api_permission("/api/v1/solana/position/open", &SolanaApiAction::Read, &basic_user).await;
        assert!(result.is_err());

        // 测试池子创建API - 只有管理员可以写入
        let result = service.check_api_permission("/api/v1/solana/pool/create/clmm", &SolanaApiAction::Write, &vip_user).await;
        assert!(result.is_err());

        let result = service.check_api_permission("/api/v1/solana/pool/create/clmm", &SolanaApiAction::Write, &admin_user).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_emergency_shutdown_override() {
        let service = SolanaPermissionService::new();
        let admin_user = create_test_auth_user(UserTier::Admin, vec![]);

        // 正常情况下管理员应该可以访问
        let result = service.check_api_permission("/api/v1/solana/swap", &SolanaApiAction::Write, &admin_user).await;
        assert!(result.is_ok());

        // 紧急停用 - 连管理员也无法访问
        service.emergency_shutdown(true).await.unwrap();

        let result = service.check_api_permission("/api/v1/solana/swap", &SolanaApiAction::Write, &admin_user).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "系统紧急停用中");

        // 恢复服务
        service.emergency_shutdown(false).await.unwrap();

        let result = service.check_api_permission("/api/v1/solana/swap", &SolanaApiAction::Write, &admin_user).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_maintenance_mode_admin_only() {
        let service = SolanaPermissionService::new();
        let regular_user = create_test_auth_user(UserTier::VIP, vec![Permission::CreatePosition]);
        let admin_user = create_test_auth_user(UserTier::Admin, vec![]);

        // 开启维护模式
        service.toggle_maintenance_mode(true).await.unwrap();

        // 普通用户（即使是VIP）应该被拒绝
        let result = service.check_api_permission("/api/v1/solana/pools/info/list", &SolanaApiAction::Read, &regular_user).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("系统维护模式"));

        // 管理员应该可以访问
        let result = service.check_api_permission("/api/v1/solana/pools/info/list", &SolanaApiAction::Read, &admin_user).await;
        assert!(result.is_ok());

        // 关闭维护模式
        service.toggle_maintenance_mode(false).await.unwrap();

        // 普通用户应该恢复访问
        let result = service.check_api_permission("/api/v1/solana/pools/info/list", &SolanaApiAction::Read, &regular_user).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_permission_priority_hierarchy() {
        let service = SolanaPermissionService::new();
        let user = create_test_auth_user(UserTier::VIP, vec![Permission::CreatePosition]);

        // 1. 正常情况 - 所有权限检查都通过
        let result = service.check_api_permission("/api/v1/solana/swap", &SolanaApiAction::Write, &user).await;
        assert!(result.is_ok());

        // 2. 禁用API - 即使其他条件满足也被拒绝
        let disabled_config = SolanaApiPermissionConfig {
            endpoint: "/api/v1/solana/swap".to_string(),
            name: "代币交换".to_string(),
            category: "交换".to_string(),
            read_policy: SolanaPermissionPolicy::Allow,
            write_policy: SolanaPermissionPolicy::Allow,
            enabled: false, // 禁用API
            created_at: chrono::Utc::now().timestamp() as u64,
            updated_at: chrono::Utc::now().timestamp() as u64,
        };
        service.update_api_config("/api/v1/solana/swap".to_string(), disabled_config).await.unwrap();

        let result = service.check_api_permission("/api/v1/solana/swap", &SolanaApiAction::Write, &user).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("已禁用"));

        // 重新启用API
        let enabled_config = SolanaApiPermissionConfig {
            endpoint: "/api/v1/solana/swap".to_string(),
            name: "代币交换".to_string(),
            category: "交换".to_string(),
            read_policy: SolanaPermissionPolicy::Allow,
            write_policy: SolanaPermissionPolicy::Allow,
            enabled: true,
            created_at: chrono::Utc::now().timestamp() as u64,
            updated_at: chrono::Utc::now().timestamp() as u64,
        };
        service.update_api_config("/api/v1/solana/swap".to_string(), enabled_config).await.unwrap();

        // 3. 全局写入权限关闭 - 优先于API级别配置
        service.toggle_global_write(false).await.unwrap();

        let result = service.check_api_permission("/api/v1/solana/swap", &SolanaApiAction::Write, &user).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("全局写入权限已关闭"));

        // 重新开启全局写入权限
        service.toggle_global_write(true).await.unwrap();

        // 4. 紧急停用 - 最高优先级
        service.emergency_shutdown(true).await.unwrap();

        let result = service.check_api_permission("/api/v1/solana/swap", &SolanaApiAction::Write, &user).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "系统紧急停用中");
    }

    #[tokio::test]
    async fn test_permission_statistics() {
        let service = SolanaPermissionService::new();

        // 获取初始统计
        let initial_stats = service.get_permission_stats().await.unwrap();
        assert!(initial_stats.total_apis > 0);
        assert_eq!(initial_stats.enabled_apis + initial_stats.disabled_apis, initial_stats.total_apis);

        // 添加一些API配置
        let mut configs = HashMap::new();
        configs.insert(
            "/api/v1/solana/test1".to_string(),
            SolanaApiPermissionConfig {
                endpoint: "/api/v1/solana/test1".to_string(),
                name: "测试API1".to_string(),
                category: "测试".to_string(),
                read_policy: SolanaPermissionPolicy::Allow,
                write_policy: SolanaPermissionPolicy::Deny,
                enabled: true,
                created_at: chrono::Utc::now().timestamp() as u64,
                updated_at: chrono::Utc::now().timestamp() as u64,
            },
        );
        configs.insert(
            "/api/v1/solana/test2".to_string(),
            SolanaApiPermissionConfig {
                endpoint: "/api/v1/solana/test2".to_string(),
                name: "测试API2".to_string(),
                category: "测试".to_string(),
                read_policy: SolanaPermissionPolicy::Allow,
                write_policy: SolanaPermissionPolicy::Deny,
                enabled: false, // 禁用
                created_at: chrono::Utc::now().timestamp() as u64,
                updated_at: chrono::Utc::now().timestamp() as u64,
            },
        );

        service.batch_update_api_configs(configs).await.unwrap();

        // 获取更新后的统计
        let updated_stats = service.get_permission_stats().await.unwrap();
        assert_eq!(updated_stats.total_apis, initial_stats.total_apis + 2);
        assert_eq!(updated_stats.enabled_apis, initial_stats.enabled_apis + 1);
        assert_eq!(updated_stats.disabled_apis, initial_stats.disabled_apis + 1);

        // 测试全局权限状态
        service.toggle_global_read(false).await.unwrap();
        service.toggle_maintenance_mode(true).await.unwrap();

        let final_stats = service.get_permission_stats().await.unwrap();
        assert!(!final_stats.global_read_enabled);
        assert!(final_stats.global_write_enabled);
        assert!(final_stats.maintenance_mode);
        assert!(!final_stats.emergency_shutdown);
    }

    #[tokio::test]
    async fn test_configuration_reload() {
        let service = SolanaPermissionService::new();

        // 修改一些配置
        service.toggle_global_read(false).await.unwrap();
        service.toggle_maintenance_mode(true).await.unwrap();

        let config_before = service.get_global_config().await.unwrap();
        assert!(!config_before.global_read_enabled);
        assert!(config_before.maintenance_mode);

        // 测试配置重载（这里主要测试方法调用成功）
        let result = service.reload_configuration().await;
        assert!(result.is_ok());

        // 在实际应用中，重载会从数据库恢复配置
        // 这里我们只验证方法调用成功
    }
}

#[cfg(test)]
mod integration_scenario_tests {
    use super::*;

    #[tokio::test]
    async fn test_trading_peak_hour_scenario() {
        // 场景：交易高峰期，限制只有VIP用户可以进行交换操作
        let service = SolanaPermissionService::new();

        // 配置交换API为VIP专用
        let swap_config = SolanaApiPermissionConfig {
            endpoint: "/api/v1/solana/swap".to_string(),
            name: "代币交换".to_string(),
            category: "交换".to_string(),
            read_policy: SolanaPermissionPolicy::Allow,                          // 报价仍然开放
            write_policy: SolanaPermissionPolicy::RequireMinTier(UserTier::VIP), // 只有VIP可以交换
            enabled: true,
            created_at: chrono::Utc::now().timestamp() as u64,
            updated_at: chrono::Utc::now().timestamp() as u64,
        };
        service.update_api_config("/api/v1/solana/swap".to_string(), swap_config).await.unwrap();

        let basic_user = create_test_auth_user(UserTier::Basic, vec![Permission::CreatePosition]);
        let premium_user = create_test_auth_user(UserTier::Premium, vec![Permission::CreatePosition]);
        let vip_user = create_test_auth_user(UserTier::VIP, vec![Permission::CreatePosition]);

        // 所有用户都可以查询价格
        for user in [&basic_user, &premium_user, &vip_user] {
            let result = service.check_api_permission("/api/v1/solana/swap", &SolanaApiAction::Read, user).await;
            assert!(result.is_ok());
        }

        // 只有VIP用户可以执行交换
        let result = service.check_api_permission("/api/v1/solana/swap", &SolanaApiAction::Write, &basic_user).await;
        assert!(result.is_err());

        let result = service.check_api_permission("/api/v1/solana/swap", &SolanaApiAction::Write, &premium_user).await;
        assert!(result.is_err());

        let result = service.check_api_permission("/api/v1/solana/swap", &SolanaApiAction::Write, &vip_user).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_system_maintenance_scenario() {
        // 场景：系统维护期间，禁用所有写操作，保留查询功能
        let service = SolanaPermissionService::new();

        // 关闭全局写入权限
        service.toggle_global_write(false).await.unwrap();

        let vip_user = create_test_auth_user(UserTier::VIP, vec![Permission::CreatePosition]);
        let admin_user = create_test_auth_user(UserTier::Admin, vec![]);

        // 查询功能正常
        let result = service.check_api_permission("/api/v1/solana/pools/info/list", &SolanaApiAction::Read, &vip_user).await;
        assert!(result.is_ok());

        // 所有写入操作被禁用（包括VIP）
        let result = service.check_api_permission("/api/v1/solana/swap", &SolanaApiAction::Write, &vip_user).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("全局写入权限已关闭"));

        // 管理员也受限制
        let result = service.check_api_permission("/api/v1/solana/swap", &SolanaApiAction::Write, &admin_user).await;
        assert!(result.is_err());

        // 维护完成，恢复写入权限
        service.toggle_global_write(true).await.unwrap();

        let result = service.check_api_permission("/api/v1/solana/swap", &SolanaApiAction::Write, &vip_user).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_security_incident_response_scenario() {
        // 场景：发现安全漏洞，立即停用所有Solana API
        let service = SolanaPermissionService::new();
        let admin_user = create_test_auth_user(UserTier::Admin, vec![]);

        // 正常情况
        let result = service.check_api_permission("/api/v1/solana/pools/info/list", &SolanaApiAction::Read, &admin_user).await;
        assert!(result.is_ok());

        // 紧急停用
        service.emergency_shutdown(true).await.unwrap();

        // 所有访问都被阻止（包括管理员）
        let result = service.check_api_permission("/api/v1/solana/pools/info/list", &SolanaApiAction::Read, &admin_user).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "系统紧急停用中");

        // 漏洞修复后恢复服务
        service.emergency_shutdown(false).await.unwrap();

        let result = service.check_api_permission("/api/v1/solana/pools/info/list", &SolanaApiAction::Read, &admin_user).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_feature_gradual_rollout_scenario() {
        // 场景：新功能渐进开放，先VIP，后Premium，最后Basic
        let service = SolanaPermissionService::new();

        let basic_user = create_test_auth_user(UserTier::Basic, vec![Permission::CreatePosition]);
        let premium_user = create_test_auth_user(UserTier::Premium, vec![Permission::CreatePosition]);
        let vip_user = create_test_auth_user(UserTier::VIP, vec![Permission::CreatePosition]);

        // 阶段1：只对VIP开放
        let new_feature_config = SolanaApiPermissionConfig {
            endpoint: "/api/v1/solana/new-feature".to_string(),
            name: "新功能".to_string(),
            category: "实验性".to_string(),
            read_policy: SolanaPermissionPolicy::RequireMinTier(UserTier::VIP),
            write_policy: SolanaPermissionPolicy::RequireMinTier(UserTier::VIP),
            enabled: true,
            created_at: chrono::Utc::now().timestamp() as u64,
            updated_at: chrono::Utc::now().timestamp() as u64,
        };
        service.update_api_config("/api/v1/solana/new-feature".to_string(), new_feature_config).await.unwrap();

        // 只有VIP可以访问
        let result = service.check_api_permission("/api/v1/solana/new-feature", &SolanaApiAction::Read, &basic_user).await;
        assert!(result.is_err());

        let result = service.check_api_permission("/api/v1/solana/new-feature", &SolanaApiAction::Read, &premium_user).await;
        assert!(result.is_err());

        let result = service.check_api_permission("/api/v1/solana/new-feature", &SolanaApiAction::Read, &vip_user).await;
        assert!(result.is_ok());

        // 阶段2：开放给Premium用户
        let updated_config = SolanaApiPermissionConfig {
            endpoint: "/api/v1/solana/new-feature".to_string(),
            name: "新功能".to_string(),
            category: "实验性".to_string(),
            read_policy: SolanaPermissionPolicy::RequireMinTier(UserTier::Premium),
            write_policy: SolanaPermissionPolicy::RequireMinTier(UserTier::Premium),
            enabled: true,
            created_at: chrono::Utc::now().timestamp() as u64,
            updated_at: chrono::Utc::now().timestamp() as u64,
        };
        service.update_api_config("/api/v1/solana/new-feature".to_string(), updated_config).await.unwrap();

        // Premium和VIP都可以访问
        let result = service.check_api_permission("/api/v1/solana/new-feature", &SolanaApiAction::Read, &basic_user).await;
        assert!(result.is_err());

        let result = service.check_api_permission("/api/v1/solana/new-feature", &SolanaApiAction::Read, &premium_user).await;
        assert!(result.is_ok());

        let result = service.check_api_permission("/api/v1/solana/new-feature", &SolanaApiAction::Read, &vip_user).await;
        assert!(result.is_ok());

        // 阶段3：全面开放
        let final_config = SolanaApiPermissionConfig {
            endpoint: "/api/v1/solana/new-feature".to_string(),
            name: "新功能".to_string(),
            category: "稳定".to_string(),
            read_policy: SolanaPermissionPolicy::Allow,
            write_policy: SolanaPermissionPolicy::RequirePermission(Permission::CreatePosition),
            enabled: true,
            created_at: chrono::Utc::now().timestamp() as u64,
            updated_at: chrono::Utc::now().timestamp() as u64,
        };
        service.update_api_config("/api/v1/solana/new-feature".to_string(), final_config).await.unwrap();

        // 所有用户都可以访问
        for user in [&basic_user, &premium_user, &vip_user] {
            let result = service.check_api_permission("/api/v1/solana/new-feature", &SolanaApiAction::Read, user).await;
            assert!(result.is_ok());

            let result = service.check_api_permission("/api/v1/solana/new-feature", &SolanaApiAction::Write, user).await;
            assert!(result.is_ok()); // 所有测试用户都有CreatePosition权限
        }
    }
}
