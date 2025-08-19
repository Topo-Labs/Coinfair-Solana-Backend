//! 权限配置热加载功能测试
//! 测试权限配置的自动重载和手动重载功能

use crate::auth::{AuthUser, Permission, SolanaApiAction, UserTier};
use crate::services::solana_permission_service::{SolanaPermissionService, SolanaPermissionServiceTrait};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

/// 创建测试用的认证用户
fn create_test_user() -> AuthUser {
    let mut permissions = HashSet::new();
    permissions.insert(Permission::ReadPool);
    permissions.insert(Permission::CreatePosition);

    AuthUser {
        user_id: "hot_reload_test_user".to_string(),
        wallet_address: Some("hot_reload_test_wallet".to_string()),
        tier: UserTier::Basic,
        permissions,
    }
}

#[cfg(test)]
mod hot_reload_tests {
    use super::*;

    #[tokio::test]
    async fn test_manual_config_reload() {
        let service = SolanaPermissionService::new();
        let user = create_test_user();

        // 1. 测试初始状态
        let result = service
            .check_api_permission("/api/v1/solana/swap", &SolanaApiAction::Write, &user)
            .await;
        assert!(result.is_ok());

        // 2. 禁用全局写入权限
        service.toggle_global_write(false).await.unwrap();

        let result = service
            .check_api_permission("/api/v1/solana/swap", &SolanaApiAction::Write, &user)
            .await;
        assert!(result.is_err());

        // 3. 手动重载配置（模拟从数据库重载，但由于没有真实数据库，这里主要测试方法调用）
        let reload_result = service.reload_configuration().await;
        assert!(reload_result.is_ok());

        // 4. 验证重载后状态（没有数据库，配置不会改变）
        let result = service
            .check_api_permission("/api/v1/solana/swap", &SolanaApiAction::Write, &user)
            .await;
        assert!(result.is_err()); // 仍然被禁用，因为没有真实的数据库重载
    }

    #[tokio::test]
    async fn test_hot_reload_without_database() {
        let service = SolanaPermissionService::new();

        // 测试没有数据库时启用热重载应该返回错误
        let result = service.enable_hot_reload(30).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("热重载需要数据库支持"));
    }

    #[tokio::test]
    async fn test_config_change_listener_setup() {
        let service = SolanaPermissionService::new();

        // 测试配置变更监听器设置
        let result = service.setup_config_change_listener().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_permission_stats_consistency() {
        let service = SolanaPermissionService::new();

        // 获取初始统计
        let initial_stats = service.get_permission_stats().await.unwrap();
        assert!(initial_stats.total_apis > 0);
        assert!(initial_stats.global_read_enabled);
        assert!(initial_stats.global_write_enabled);
        assert!(!initial_stats.emergency_shutdown);
        assert!(!initial_stats.maintenance_mode);

        // 修改配置
        service.toggle_global_read(false).await.unwrap();
        service.toggle_maintenance_mode(true).await.unwrap();

        // 获取更新后的统计
        let updated_stats = service.get_permission_stats().await.unwrap();
        assert!(!updated_stats.global_read_enabled);
        assert!(updated_stats.global_write_enabled);
        assert!(updated_stats.maintenance_mode);
        assert!(!updated_stats.emergency_shutdown);

        // 统计数据应该一致
        assert_eq!(initial_stats.total_apis, updated_stats.total_apis);
        assert_eq!(initial_stats.enabled_apis, updated_stats.enabled_apis);
    }

    #[tokio::test]
    async fn test_concurrent_config_access() {
        let service = Arc::new(SolanaPermissionService::new());
        let user = create_test_user();

        // 创建多个并发任务
        let mut handles = Vec::new();

        // 启动读取任务
        for i in 0..5 {
            let service_clone = Arc::clone(&service);
            let user_clone = user.clone();

            let handle = tokio::spawn(async move {
                for j in 0..10 {
                    let result = service_clone
                        .check_api_permission("/api/v1/solana/pools/info/list", &SolanaApiAction::Read, &user_clone)
                        .await;

                    if result.is_err() {
                        println!("读取任务 {} 第 {} 次检查失败: {:?}", i, j, result);
                    }
                }
                true
            });
            handles.push(handle);
        }

        // 启动配置修改任务
        for i in 0..3 {
            let service_clone = Arc::clone(&service);

            let handle = tokio::spawn(async move {
                for j in 0..5 {
                    let enabled = j % 2 == 0;
                    let result = service_clone.toggle_global_read(enabled).await;

                    if result.is_err() {
                        println!("配置任务 {} 第 {} 次修改失败: {:?}", i, j, result);
                        return false;
                    }

                    // 短暂延迟
                    sleep(Duration::from_millis(10)).await;
                }
                true
            });
            handles.push(handle);
        }

        // 等待所有任务完成
        let mut success_count = 0;
        for handle in handles {
            if let Ok(success) = handle.await {
                if success {
                    success_count += 1;
                }
            }
        }

        // 验证大部分任务成功
        assert!(success_count >= 7); // 至少7/8的任务成功
    }

    #[tokio::test]
    async fn test_permission_configuration_persistence_simulation() {
        // 模拟权限配置持久化场景
        let service = SolanaPermissionService::new();
        let user = create_test_user();

        // 1. 记录初始配置
        let _initial_global_config = service.get_global_config().await.unwrap();
        let initial_api_configs = service.get_all_api_configs().await.unwrap();

        // 2. 进行一系列配置变更
        service.toggle_global_write(false).await.unwrap();
        service.toggle_maintenance_mode(true).await.unwrap();

        // 添加自定义API配置
        use crate::auth::{SolanaApiPermissionConfig, SolanaPermissionPolicy};
        let custom_config = SolanaApiPermissionConfig {
            endpoint: "/api/v1/solana/hot-reload-test".to_string(),
            name: "热重载测试API".to_string(),
            category: "测试".to_string(),
            read_policy: SolanaPermissionPolicy::Allow,
            write_policy: SolanaPermissionPolicy::RequireMinTier(UserTier::VIP),
            enabled: true,
            created_at: chrono::Utc::now().timestamp() as u64,
            updated_at: chrono::Utc::now().timestamp() as u64,
        };

        service
            .update_api_config("/api/v1/solana/hot-reload-test".to_string(), custom_config)
            .await
            .unwrap();

        // 3. 验证配置变更生效
        let result = service
            .check_api_permission("/api/v1/solana/swap", &SolanaApiAction::Write, &user)
            .await;
        assert!(result.is_err()); // 全局写入被禁用

        // 注意：在维护模式下，Basic用户无法访问，所以我们用Admin用户测试
        let mut admin_permissions = HashSet::new();
        admin_permissions.insert(Permission::ReadPool);
        admin_permissions.insert(Permission::CreatePosition);

        let admin_user = AuthUser {
            user_id: "admin_test_user".to_string(),
            wallet_address: Some("admin_test_wallet".to_string()),
            tier: UserTier::Admin,
            permissions: admin_permissions,
        };

        let result = service
            .check_api_permission("/api/v1/solana/hot-reload-test", &SolanaApiAction::Read, &admin_user)
            .await;
        assert!(result.is_ok()); // 管理员可以在维护模式下读取

        let result = service
            .check_api_permission("/api/v1/solana/hot-reload-test", &SolanaApiAction::Write, &admin_user)
            .await;
        assert!(result.is_err()); // 即使是管理员，全局写入权限被禁用时也无法写入

        // 4. 模拟重载配置
        let reload_result = service.reload_configuration().await;
        assert!(reload_result.is_ok());

        // 5. 验证配置统计
        let final_stats = service.get_permission_stats().await.unwrap();
        assert!(final_stats.total_apis > initial_api_configs.len());
        assert!(!final_stats.global_write_enabled);
        assert!(final_stats.maintenance_mode);

        println!("热重载测试完成:");
        println!("  初始API数量: {}", initial_api_configs.len());
        println!("  最终API数量: {}", final_stats.total_apis);
        println!("  全局写入权限: {}", final_stats.global_write_enabled);
        println!("  维护模式: {}", final_stats.maintenance_mode);
    }

    #[tokio::test]
    async fn test_permission_service_cloning() {
        // 测试权限服务的克隆功能（用于热重载后台任务）
        let original_service = SolanaPermissionService::new();
        let user = create_test_user();

        // 修改原始服务的配置
        original_service.toggle_global_read(false).await.unwrap();

        // 克隆服务（模拟热重载任务中的使用）
        let cloned_service = original_service.clone();

        // 验证克隆的服务有相同的配置
        let result1 = original_service
            .check_api_permission("/api/v1/solana/pools/info/list", &SolanaApiAction::Read, &user)
            .await;

        let result2 = cloned_service
            .check_api_permission("/api/v1/solana/pools/info/list", &SolanaApiAction::Read, &user)
            .await;

        // 两个服务应该有相同的行为
        assert_eq!(result1.is_ok(), result2.is_ok());
        if result1.is_err() && result2.is_err() {
            assert_eq!(result1.unwrap_err(), result2.unwrap_err());
        }
    }

    #[tokio::test]
    async fn test_hot_reload_error_handling() {
        let service = SolanaPermissionService::new();

        // 测试各种错误情况的处理

        // 1. 无数据库时的重载
        let result = service.reload_from_database().await;
        assert!(result.is_ok()); // 应该优雅处理无数据库的情况

        // 2. 无数据库时的热重载启用
        let result = service.enable_hot_reload(10).await;
        assert!(result.is_err());

        // 3. 配置监听器设置
        let result = service.setup_config_change_listener().await;
        assert!(result.is_ok()); // 应该成功设置基础框架
    }
}
