//! Solana权限系统集成测试
//! 测试完整的权限管理流程，包括与数据库、API控制器的集成

use crate::auth::{
    AuthConfig, AuthUser, JwtManager, Permission, SolanaApiAction, SolanaApiPermissionConfig, SolanaPermissionPolicy,
    UserTier,
};
use crate::services::solana_permission_service::{SolanaPermissionService, SolanaPermissionServiceTrait};
use std::collections::{HashMap, HashSet};
use tokio;

/// 集成测试用的配置
fn integration_test_config() -> AuthConfig {
    AuthConfig {
        jwt_secret: "integration_test_jwt_secret_key".to_string(),
        jwt_expires_in_hours: 24,
        solana_auth_message_ttl: 300,
        redis_url: None, // 使用内存存储进行测试
        rate_limit_redis_prefix: "integration_test:ratelimit".to_string(),
        auth_disabled: false,
    }
}

/// 创建测试认证用户
fn create_auth_user(user_id: &str, tier: UserTier, permissions: Vec<Permission>) -> AuthUser {
    let mut perm_set = HashSet::new();
    for perm in permissions {
        perm_set.insert(perm);
    }

    AuthUser {
        user_id: user_id.to_string(),
        wallet_address: Some(format!("wallet_for_{}", user_id)),
        tier,
        permissions: perm_set,
    }
}

#[cfg(test)]
mod permission_service_integration_tests {
    use super::*;

    #[tokio::test]
    async fn test_complete_permission_workflow() {
        // 完整权限管理工作流测试
        let service = SolanaPermissionService::new();

        // 1. 初始状态验证
        let initial_stats = service.get_permission_stats().await.unwrap();
        assert!(initial_stats.total_apis > 0);
        assert!(initial_stats.global_read_enabled);
        assert!(initial_stats.global_write_enabled);
        assert!(!initial_stats.emergency_shutdown);

        // 2. 创建不同角色的用户
        let basic_user = create_auth_user("basic_001", UserTier::Basic, vec![Permission::ReadPool]);
        let premium_user = create_auth_user(
            "premium_001",
            UserTier::Premium,
            vec![Permission::ReadPool, Permission::CreatePosition],
        );
        let vip_user = create_auth_user(
            "vip_001",
            UserTier::VIP,
            vec![
                Permission::ReadPool,
                Permission::CreatePosition,
                Permission::ReadPosition,
            ],
        );
        let admin_user = create_auth_user("admin_001", UserTier::Admin, vec![]);

        // 3. 测试默认权限配置
        // 基础用户应该可以读取池子信息
        let result = service
            .check_api_permission("/api/v1/solana/pools/info/list", &SolanaApiAction::Read, &basic_user)
            .await;
        assert!(result.is_ok());

        // 基础用户应该可以进行交换（有CreatePosition权限和Basic等级要求）
        let result = service
            .check_api_permission("/api/v1/solana/swap", &SolanaApiAction::Write, &basic_user)
            .await;
        assert!(result.is_err()); // basic_user没有CreatePosition权限

        // Premium用户应该可以交换
        let result = service
            .check_api_permission("/api/v1/solana/swap", &SolanaApiAction::Write, &premium_user)
            .await;
        assert!(result.is_ok());

        // 4. 测试精细化权限配置
        let custom_api_config = SolanaApiPermissionConfig {
            endpoint: "/api/v1/solana/advanced-trading".to_string(),
            name: "高级交易".to_string(),
            category: "高级功能".to_string(),
            read_policy: SolanaPermissionPolicy::RequireMinTier(UserTier::Premium),
            write_policy: SolanaPermissionPolicy::RequirePermissionAndTier(Permission::CreatePosition, UserTier::VIP),
            enabled: true,
            created_at: chrono::Utc::now().timestamp() as u64,
            updated_at: chrono::Utc::now().timestamp() as u64,
        };

        service
            .update_api_config("/api/v1/solana/advanced-trading".to_string(), custom_api_config)
            .await
            .unwrap();

        // 验证配置生效
        let result = service
            .check_api_permission("/api/v1/solana/advanced-trading", &SolanaApiAction::Read, &basic_user)
            .await;
        assert!(result.is_err()); // Basic用户无法读取

        let result = service
            .check_api_permission("/api/v1/solana/advanced-trading", &SolanaApiAction::Read, &premium_user)
            .await;
        assert!(result.is_ok()); // Premium用户可以读取

        let result = service
            .check_api_permission(
                "/api/v1/solana/advanced-trading",
                &SolanaApiAction::Write,
                &premium_user,
            )
            .await;
        assert!(result.is_err()); // Premium用户无法写入（需要VIP）

        let result = service
            .check_api_permission("/api/v1/solana/advanced-trading", &SolanaApiAction::Write, &vip_user)
            .await;
        assert!(result.is_ok()); // VIP用户可以写入

        // 5. 测试全局权限控制
        service.toggle_global_write(false).await.unwrap();

        // 所有写入操作都应该被禁止
        let result = service
            .check_api_permission("/api/v1/solana/swap", &SolanaApiAction::Write, &vip_user)
            .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("全局写入权限已关闭"));

        // 读取操作应该仍然正常
        let result = service
            .check_api_permission("/api/v1/solana/pools/info/list", &SolanaApiAction::Read, &basic_user)
            .await;
        assert!(result.is_ok());

        // 6. 测试紧急停用
        service.emergency_shutdown(true).await.unwrap();

        // 所有操作都应该被禁止，包括管理员
        let result = service
            .check_api_permission("/api/v1/solana/pools/info/list", &SolanaApiAction::Read, &admin_user)
            .await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "系统紧急停用中");

        // 7. 恢复正常状态
        service.emergency_shutdown(false).await.unwrap();
        service.toggle_global_write(true).await.unwrap();

        // 验证恢复正常
        let result = service
            .check_api_permission("/api/v1/solana/swap", &SolanaApiAction::Write, &premium_user)
            .await;
        assert!(result.is_ok());

        // 8. 验证统计信息
        let final_stats = service.get_permission_stats().await.unwrap();
        assert!(final_stats.total_apis > initial_stats.total_apis); // 添加了一个新API
        assert!(final_stats.global_read_enabled);
        assert!(final_stats.global_write_enabled);
        assert!(!final_stats.emergency_shutdown);
    }

    #[tokio::test]
    async fn test_batch_configuration_management() {
        let service = SolanaPermissionService::new();

        // 创建多个API配置进行批量管理
        let mut batch_configs = HashMap::new();

        // 交易相关API - 不同等级限制
        batch_configs.insert(
            "/api/v1/solana/trade/spot".to_string(),
            SolanaApiPermissionConfig {
                endpoint: "/api/v1/solana/trade/spot".to_string(),
                name: "现货交易".to_string(),
                category: "交易".to_string(),
                read_policy: SolanaPermissionPolicy::Allow,
                write_policy: SolanaPermissionPolicy::RequireMinTier(UserTier::Basic),
                enabled: true,
                created_at: chrono::Utc::now().timestamp() as u64,
                updated_at: chrono::Utc::now().timestamp() as u64,
            },
        );

        batch_configs.insert(
            "/api/v1/solana/trade/margin".to_string(),
            SolanaApiPermissionConfig {
                endpoint: "/api/v1/solana/trade/margin".to_string(),
                name: "保证金交易".to_string(),
                category: "交易".to_string(),
                read_policy: SolanaPermissionPolicy::RequireMinTier(UserTier::Premium),
                write_policy: SolanaPermissionPolicy::RequireMinTier(UserTier::Premium),
                enabled: true,
                created_at: chrono::Utc::now().timestamp() as u64,
                updated_at: chrono::Utc::now().timestamp() as u64,
            },
        );

        batch_configs.insert(
            "/api/v1/solana/trade/futures".to_string(),
            SolanaApiPermissionConfig {
                endpoint: "/api/v1/solana/trade/futures".to_string(),
                name: "期货交易".to_string(),
                category: "交易".to_string(),
                read_policy: SolanaPermissionPolicy::RequireMinTier(UserTier::VIP),
                write_policy: SolanaPermissionPolicy::RequirePermissionAndTier(
                    Permission::CreatePosition,
                    UserTier::VIP,
                ),
                enabled: true,
                created_at: chrono::Utc::now().timestamp() as u64,
                updated_at: chrono::Utc::now().timestamp() as u64,
            },
        );

        // 管理功能API - 仅管理员
        batch_configs.insert(
            "/api/v1/solana/admin/pool-management".to_string(),
            SolanaApiPermissionConfig {
                endpoint: "/api/v1/solana/admin/pool-management".to_string(),
                name: "池子管理".to_string(),
                category: "管理".to_string(),
                read_policy: SolanaPermissionPolicy::RequireMinTier(UserTier::Admin),
                write_policy: SolanaPermissionPolicy::RequireMinTier(UserTier::Admin),
                enabled: true,
                created_at: chrono::Utc::now().timestamp() as u64,
                updated_at: chrono::Utc::now().timestamp() as u64,
            },
        );

        // 批量更新
        service.batch_update_api_configs(batch_configs).await.unwrap();

        // 创建测试用户
        let basic_user = create_auth_user("basic_batch", UserTier::Basic, vec![Permission::CreatePosition]);
        let premium_user = create_auth_user("premium_batch", UserTier::Premium, vec![Permission::CreatePosition]);
        let vip_user = create_auth_user("vip_batch", UserTier::VIP, vec![Permission::CreatePosition]);
        let admin_user = create_auth_user("admin_batch", UserTier::Admin, vec![]);

        // 验证现货交易权限
        let result = service
            .check_api_permission("/api/v1/solana/trade/spot", &SolanaApiAction::Read, &basic_user)
            .await;
        assert!(result.is_ok());
        let result = service
            .check_api_permission("/api/v1/solana/trade/spot", &SolanaApiAction::Write, &basic_user)
            .await;
        assert!(result.is_ok());

        // 验证保证金交易权限
        let result = service
            .check_api_permission("/api/v1/solana/trade/margin", &SolanaApiAction::Read, &basic_user)
            .await;
        assert!(result.is_err()); // Basic用户无法访问
        let result = service
            .check_api_permission("/api/v1/solana/trade/margin", &SolanaApiAction::Read, &premium_user)
            .await;
        assert!(result.is_ok()); // Premium用户可以访问

        // 验证期货交易权限
        let result = service
            .check_api_permission("/api/v1/solana/trade/futures", &SolanaApiAction::Read, &premium_user)
            .await;
        assert!(result.is_err()); // Premium用户无法访问
        let result = service
            .check_api_permission("/api/v1/solana/trade/futures", &SolanaApiAction::Write, &vip_user)
            .await;
        assert!(result.is_ok()); // VIP用户可以访问

        // 验证管理功能权限
        let result = service
            .check_api_permission(
                "/api/v1/solana/admin/pool-management",
                &SolanaApiAction::Read,
                &vip_user,
            )
            .await;
        assert!(result.is_err()); // VIP用户无法访问管理功能
        let result = service
            .check_api_permission(
                "/api/v1/solana/admin/pool-management",
                &SolanaApiAction::Write,
                &admin_user,
            )
            .await;
        assert!(result.is_ok()); // 管理员可以访问

        // 验证配置统计
        let _stats = service.get_permission_stats().await.unwrap();
        let all_configs = service.get_all_api_configs().await.unwrap();
        assert!(all_configs.contains_key("/api/v1/solana/trade/spot"));
        assert!(all_configs.contains_key("/api/v1/solana/trade/margin"));
        assert!(all_configs.contains_key("/api/v1/solana/trade/futures"));
        assert!(all_configs.contains_key("/api/v1/solana/admin/pool-management"));
    }

    #[tokio::test]
    async fn test_api_disable_and_enable_workflow() {
        let service = SolanaPermissionService::new();
        let user = create_auth_user("test_disable", UserTier::VIP, vec![Permission::CreatePosition]);

        // 创建一个测试API
        let test_config = SolanaApiPermissionConfig {
            endpoint: "/api/v1/solana/test-disable".to_string(),
            name: "测试禁用功能".to_string(),
            category: "测试".to_string(),
            read_policy: SolanaPermissionPolicy::Allow,
            write_policy: SolanaPermissionPolicy::Allow,
            enabled: true,
            created_at: chrono::Utc::now().timestamp() as u64,
            updated_at: chrono::Utc::now().timestamp() as u64,
        };

        service
            .update_api_config("/api/v1/solana/test-disable".to_string(), test_config)
            .await
            .unwrap();

        // 验证API正常工作
        let result = service
            .check_api_permission("/api/v1/solana/test-disable", &SolanaApiAction::Read, &user)
            .await;
        assert!(result.is_ok());
        let result = service
            .check_api_permission("/api/v1/solana/test-disable", &SolanaApiAction::Write, &user)
            .await;
        assert!(result.is_ok());

        // 禁用API
        let disabled_config = SolanaApiPermissionConfig {
            endpoint: "/api/v1/solana/test-disable".to_string(),
            name: "测试禁用功能".to_string(),
            category: "测试".to_string(),
            read_policy: SolanaPermissionPolicy::Allow,
            write_policy: SolanaPermissionPolicy::Allow,
            enabled: false, // 禁用
            created_at: chrono::Utc::now().timestamp() as u64,
            updated_at: chrono::Utc::now().timestamp() as u64,
        };

        service
            .update_api_config("/api/v1/solana/test-disable".to_string(), disabled_config)
            .await
            .unwrap();

        // 验证API被禁用
        let result = service
            .check_api_permission("/api/v1/solana/test-disable", &SolanaApiAction::Read, &user)
            .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("已禁用"));

        let result = service
            .check_api_permission("/api/v1/solana/test-disable", &SolanaApiAction::Write, &user)
            .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("已禁用"));

        // 重新启用API
        let enabled_config = SolanaApiPermissionConfig {
            endpoint: "/api/v1/solana/test-disable".to_string(),
            name: "测试禁用功能".to_string(),
            category: "测试".to_string(),
            read_policy: SolanaPermissionPolicy::Allow,
            write_policy: SolanaPermissionPolicy::Allow,
            enabled: true, // 重新启用
            created_at: chrono::Utc::now().timestamp() as u64,
            updated_at: chrono::Utc::now().timestamp() as u64,
        };

        service
            .update_api_config("/api/v1/solana/test-disable".to_string(), enabled_config)
            .await
            .unwrap();

        // 验证API恢复正常
        let result = service
            .check_api_permission("/api/v1/solana/test-disable", &SolanaApiAction::Read, &user)
            .await;
        assert!(result.is_ok());
        let result = service
            .check_api_permission("/api/v1/solana/test-disable", &SolanaApiAction::Write, &user)
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_permission_hierarchy_and_override() {
        let service = SolanaPermissionService::new();

        let basic_user = create_auth_user("hierarchy_basic", UserTier::Basic, vec![Permission::CreatePosition]);
        let admin_user = create_auth_user("hierarchy_admin", UserTier::Admin, vec![]);

        // 设置一个高要求的API配置
        let strict_config = SolanaApiPermissionConfig {
            endpoint: "/api/v1/solana/strict-api".to_string(),
            name: "严格API".to_string(),
            category: "测试".to_string(),
            read_policy: SolanaPermissionPolicy::RequirePermissionAndTier(Permission::AdminConfig, UserTier::VIP),
            write_policy: SolanaPermissionPolicy::RequirePermissionAndTier(Permission::AdminConfig, UserTier::VIP),
            enabled: true,
            created_at: chrono::Utc::now().timestamp() as u64,
            updated_at: chrono::Utc::now().timestamp() as u64,
        };

        service
            .update_api_config("/api/v1/solana/strict-api".to_string(), strict_config)
            .await
            .unwrap();

        // Basic用户应该无法访问
        let result = service
            .check_api_permission("/api/v1/solana/strict-api", &SolanaApiAction::Read, &basic_user)
            .await;
        assert!(result.is_err());

        // 管理员应该可以访问（无视权限和等级要求）
        let result = service
            .check_api_permission("/api/v1/solana/strict-api", &SolanaApiAction::Read, &admin_user)
            .await;
        assert!(result.is_ok());
        let result = service
            .check_api_permission("/api/v1/solana/strict-api", &SolanaApiAction::Write, &admin_user)
            .await;
        assert!(result.is_ok());

        // 测试全局权限覆盖
        service.toggle_global_read(false).await.unwrap();

        // 即使是管理员也应该受全局权限限制
        let result = service
            .check_api_permission("/api/v1/solana/strict-api", &SolanaApiAction::Read, &admin_user)
            .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("全局读取权限已关闭"));

        // 但紧急停用是最高优先级
        service.toggle_global_read(true).await.unwrap();
        service.emergency_shutdown(true).await.unwrap();

        // 即使是管理员也无法在紧急停用时访问
        let result = service
            .check_api_permission("/api/v1/solana/strict-api", &SolanaApiAction::Read, &admin_user)
            .await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "系统紧急停用中");
    }

    #[tokio::test]
    async fn test_concurrent_permission_operations() {
        use std::sync::Arc;
        use tokio::task::JoinSet;

        let service = Arc::new(SolanaPermissionService::new());
        let mut join_set = JoinSet::new();

        // 并发进行多种权限操作
        for i in 0..10 {
            let service_clone = Arc::clone(&service);
            let user = create_auth_user(
                &format!("concurrent_user_{}", i),
                UserTier::Premium,
                vec![Permission::CreatePosition],
            );

            join_set.spawn(async move {
                // 并发进行权限检查
                let result = service_clone
                    .check_api_permission("/api/v1/solana/swap", &SolanaApiAction::Write, &user)
                    .await;
                result.is_ok()
            });
        }

        // 同时进行配置更新
        for i in 0..5 {
            let service_clone = Arc::clone(&service);
            join_set.spawn(async move {
                let config = SolanaApiPermissionConfig {
                    endpoint: format!("/api/v1/solana/concurrent-test-{}", i),
                    name: format!("并发测试API {}", i),
                    category: "测试".to_string(),
                    read_policy: SolanaPermissionPolicy::Allow,
                    write_policy: SolanaPermissionPolicy::RequireMinTier(UserTier::Basic),
                    enabled: true,
                    created_at: chrono::Utc::now().timestamp() as u64,
                    updated_at: chrono::Utc::now().timestamp() as u64,
                };

                let result = service_clone
                    .update_api_config(format!("/api/v1/solana/concurrent-test-{}", i), config)
                    .await;
                result.is_ok()
            });
        }

        // 等待所有任务完成
        let mut success_count = 0;
        while let Some(result) = join_set.join_next().await {
            if let Ok(success) = result {
                if success {
                    success_count += 1;
                }
            }
        }

        // 应该有大部分操作成功
        assert!(success_count > 10);

        // 验证配置更新成功
        let all_configs = service.get_all_api_configs().await.unwrap();
        for i in 0..5 {
            let endpoint = format!("/api/v1/solana/concurrent-test-{}", i);
            assert!(all_configs.contains_key(&endpoint));
        }
    }
}

#[cfg(test)]
mod auth_and_permission_integration_tests {
    use super::*;

    #[tokio::test]
    async fn test_jwt_and_permission_integration() {
        let config = integration_test_config();
        let jwt_manager = JwtManager::new(config);
        let permission_service = SolanaPermissionService::new();

        // 创建不同等级的JWT token
        let basic_token = jwt_manager
            .generate_token(
                "integration_basic_user",
                Some("basic_wallet_address"),
                vec!["read:pool".to_string()],
                UserTier::Basic,
            )
            .unwrap();

        let premium_token = jwt_manager
            .generate_token(
                "integration_premium_user",
                Some("premium_wallet_address"),
                vec!["read:pool".to_string(), "create:position".to_string()],
                UserTier::Premium,
            )
            .unwrap();

        // 验证token并提取用户信息
        let basic_claims = jwt_manager.verify_token(&basic_token).unwrap();
        let premium_claims = jwt_manager.verify_token(&premium_token).unwrap();

        // 将JWT claims转换为AuthUser
        let basic_auth_user = AuthUser {
            user_id: basic_claims.sub.clone(),
            wallet_address: basic_claims.wallet.clone(),
            tier: basic_claims.tier.clone(),
            permissions: {
                let mut perms = HashSet::new();
                perms.insert(Permission::ReadPool); // 模拟权限转换
                perms
            },
        };

        let premium_auth_user = AuthUser {
            user_id: premium_claims.sub.clone(),
            wallet_address: premium_claims.wallet.clone(),
            tier: premium_claims.tier.clone(),
            permissions: {
                let mut perms = HashSet::new();
                perms.insert(Permission::ReadPool);
                perms.insert(Permission::CreatePosition);
                perms
            },
        };

        // 测试权限检查
        let result = permission_service
            .check_api_permission(
                "/api/v1/solana/pools/info/list",
                &SolanaApiAction::Read,
                &basic_auth_user,
            )
            .await;
        assert!(result.is_ok());

        let result = permission_service
            .check_api_permission("/api/v1/solana/swap", &SolanaApiAction::Write, &basic_auth_user)
            .await;
        assert!(result.is_err()); // Basic用户缺少CreatePosition权限

        let result = permission_service
            .check_api_permission("/api/v1/solana/swap", &SolanaApiAction::Write, &premium_auth_user)
            .await;
        assert!(result.is_ok()); // Premium用户有足够权限

        // 测试token刷新后的权限保持
        let refreshed_token = jwt_manager.refresh_token(&premium_token).unwrap();
        let refreshed_claims = jwt_manager.verify_token(&refreshed_token).unwrap();

        assert_eq!(refreshed_claims.tier, UserTier::Premium);
        assert_eq!(refreshed_claims.sub, "integration_premium_user");
        assert_ne!(refreshed_token, premium_token); // token应该不同
    }

    #[tokio::test]
    async fn test_end_to_end_permission_flow() {
        // 端到端权限流程测试：从JWT认证到权限检查到API访问
        let config = integration_test_config();
        let jwt_manager = JwtManager::new(config);
        let permission_service = SolanaPermissionService::new();

        // 模拟用户登录流程
        let user_wallet = "end_to_end_test_wallet";
        let user_permissions = vec!["read:pool".to_string(), "create:position".to_string()];
        let user_tier = UserTier::VIP;

        // 1. 生成JWT token（模拟登录成功）
        let jwt_token = jwt_manager
            .generate_token(
                user_wallet,
                Some(user_wallet),
                user_permissions.clone(),
                user_tier.clone(),
            )
            .unwrap();

        // 2. 验证token（模拟中间件验证）
        let claims = jwt_manager.verify_token(&jwt_token).unwrap();
        assert_eq!(claims.tier, user_tier);
        assert_eq!(claims.sub, user_wallet);

        // 3. 构建AuthUser（模拟中间件提取用户信息）
        let auth_user = AuthUser {
            user_id: claims.sub,
            wallet_address: claims.wallet,
            tier: claims.tier,
            permissions: {
                let mut perms = HashSet::new();
                perms.insert(Permission::ReadPool);
                perms.insert(Permission::CreatePosition);
                perms.insert(Permission::ReadPosition); // VIP用户的额外权限
                perms
            },
        };

        // 4. 配置特定API权限（模拟运营配置）
        let advanced_api_config = SolanaApiPermissionConfig {
            endpoint: "/api/v1/solana/advanced-features".to_string(),
            name: "高级功能".to_string(),
            category: "高级".to_string(),
            read_policy: SolanaPermissionPolicy::RequireMinTier(UserTier::Premium),
            write_policy: SolanaPermissionPolicy::RequirePermissionAndTier(Permission::CreatePosition, UserTier::VIP),
            enabled: true,
            created_at: chrono::Utc::now().timestamp() as u64,
            updated_at: chrono::Utc::now().timestamp() as u64,
        };

        permission_service
            .update_api_config("/api/v1/solana/advanced-features".to_string(), advanced_api_config)
            .await
            .unwrap();

        // 5. 进行权限检查（模拟API控制器调用）
        let read_result = permission_service
            .check_api_permission("/api/v1/solana/advanced-features", &SolanaApiAction::Read, &auth_user)
            .await;
        assert!(read_result.is_ok()); // VIP >= Premium

        let write_result = permission_service
            .check_api_permission("/api/v1/solana/advanced-features", &SolanaApiAction::Write, &auth_user)
            .await;
        assert!(write_result.is_ok()); // VIP用户有CreatePosition权限

        // 6. 测试运营干预场景
        // 临时关闭高级功能写入权限
        let disabled_write_config = SolanaApiPermissionConfig {
            endpoint: "/api/v1/solana/advanced-features".to_string(),
            name: "高级功能".to_string(),
            category: "高级".to_string(),
            read_policy: SolanaPermissionPolicy::RequireMinTier(UserTier::Premium),
            write_policy: SolanaPermissionPolicy::Deny, // 临时禁用写入
            enabled: true,
            created_at: chrono::Utc::now().timestamp() as u64,
            updated_at: chrono::Utc::now().timestamp() as u64,
        };

        permission_service
            .update_api_config("/api/v1/solana/advanced-features".to_string(), disabled_write_config)
            .await
            .unwrap();

        // 7. 再次检查权限
        let read_result_after = permission_service
            .check_api_permission("/api/v1/solana/advanced-features", &SolanaApiAction::Read, &auth_user)
            .await;
        assert!(read_result_after.is_ok()); // 读取仍然可用

        let write_result_after = permission_service
            .check_api_permission("/api/v1/solana/advanced-features", &SolanaApiAction::Write, &auth_user)
            .await;
        assert!(write_result_after.is_err()); // 写入被禁用
        assert_eq!(write_result_after.unwrap_err(), "操作被拒绝");

        // 8. 测试全局紧急控制
        permission_service.emergency_shutdown(true).await.unwrap();

        let emergency_result = permission_service
            .check_api_permission("/api/v1/solana/pools/info/list", &SolanaApiAction::Read, &auth_user)
            .await;
        assert!(emergency_result.is_err());
        assert_eq!(emergency_result.unwrap_err(), "系统紧急停用中");

        // 9. 恢复服务
        permission_service.emergency_shutdown(false).await.unwrap();

        let recovery_result = permission_service
            .check_api_permission("/api/v1/solana/pools/info/list", &SolanaApiAction::Read, &auth_user)
            .await;
        assert!(recovery_result.is_ok());
    }
}
