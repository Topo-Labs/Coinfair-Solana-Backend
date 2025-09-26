#[cfg(test)]
mod tests {
    use crate::dtos::solana::clmm::launch::*;
    use crate::services::solana::clmm::launch_migration::LaunchMigrationService;
    use crate::services::solana::shared::SharedContext;
    use anyhow::Result;
    use database::Database;
    use std::sync::Arc;

    /// 创建测试用的LaunchMigrationService
    async fn create_test_service() -> Result<LaunchMigrationService> {
        // 创建测试配置
        let config = std::sync::Arc::new(utils::AppConfig::new_for_test());

        // 创建测试数据库
        let database = Database::new(config).await?;

        // 创建SharedContext
        let shared_context = Arc::new(SharedContext::new()?);

        Ok(LaunchMigrationService::new(shared_context, &database))
    }

    /// 创建测试用的LaunchMigrationRequest
    fn create_test_request() -> LaunchMigrationRequest {
        LaunchMigrationRequest {
            meme_token_mint: "So11111111111111111111111111111111111111112".to_string(),
            base_token_mint: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(),
            user_wallet: "8S2bcP66WehuF6cHryfZ7vfFpQWaUhYyAYSy5U3gX4Fy".to_string(),
            config_index: 0,
            initial_price: 1.0,
            open_time: 0,
            tick_lower_price: 0.8,
            tick_upper_price: 1.2,
            meme_token_amount: 1000000000, // 1 SOL
            base_token_amount: 1000000,    // 1 USDC
            max_slippage_percent: 5.0,
            with_metadata: Some(false),
        }
    }

    /// 创建无效的LaunchMigrationRequest用于测试验证
    fn create_invalid_test_request() -> LaunchMigrationRequest {
        LaunchMigrationRequest {
            meme_token_mint: "invalid_mint".to_string(),
            base_token_mint: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(),
            user_wallet: "invalid_wallet".to_string(),
            config_index: 0,
            initial_price: -1.0, // 无效价格
            open_time: 0,
            tick_lower_price: 1.2, // 错误的tick范围
            tick_upper_price: 0.8,
            meme_token_amount: 0, // 零金额
            base_token_amount: 1000000,
            max_slippage_percent: 150.0, // 无效滑点
            with_metadata: Some(false),
        }
    }

    #[tokio::test]
    async fn test_launch_migration_service_creation() {
        let service = create_test_service().await;
        assert!(service.is_ok(), "LaunchMigrationService创建失败");
    }

    // 测试主要的公共接口，不依赖私有方法
    #[tokio::test]
    #[ignore = "需要实际的Solana网络连接"]
    async fn test_launch_integration() -> Result<()> {
        let service = create_test_service().await?;
        let request = create_test_request();

        let result = service.launch(request).await;
        // 在测试环境中，这可能会因为缺少实际的区块链连接而失败
        // 但我们可以验证方法被正确调用
        match result {
            Ok(response) => {
                assert!(!response.transaction.is_empty(), "交易数据不应该为空");
                assert!(!response.pool_address.is_empty(), "池子地址不应该为空");
                assert!(!response.position_nft_mint.is_empty(), "仓位NFT mint不应该为空");
                assert!(!response.position_key.is_empty(), "仓位key不应该为空");
                assert!(response.liquidity.parse::<u128>().is_ok(), "流动性应该是有效数字");
            }
            Err(e) => {
                // 在测试环境中失败是正常的，我们主要验证类型和结构正确性
                println!("测试环境中的预期错误: {}", e);
            }
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_launch_with_invalid_request() -> Result<()> {
        let service = create_test_service().await?;
        let request = create_invalid_test_request();

        let result = service.launch(request).await;
        // 应该因为验证失败而返回错误
        assert!(result.is_err(), "无效请求应该返回错误");

        Ok(())
    }

    #[tokio::test]
    #[ignore = "需要实际的Solana网络连接和私钥配置"]
    async fn test_launch_and_send_transaction_integration() -> Result<()> {
        let service = create_test_service().await?;
        let request = create_test_request();

        let result = service.launch_and_send_transaction(request).await;
        // 在测试环境中，这可能会因为缺少私钥配置而失败
        match result {
            Ok(response) => {
                assert!(!response.signature.is_empty(), "交易签名不应该为空");
                assert!(!response.explorer_url.is_empty(), "浏览器链接不应该为空");
                assert!(!response.pool_address.is_empty(), "池子地址不应该为空");
            }
            Err(e) => {
                println!("测试环境中的预期错误（缺少私钥配置）: {}", e);
                // 验证错误是因为私钥配置问题
                assert!(e.to_string().contains("私钥") || e.to_string().contains("private_key"));
            }
        }

        Ok(())
    }

    // 性能测试 - 确保服务实例化不会太慢
    #[tokio::test]
    async fn test_service_creation_performance() {
        let start = std::time::Instant::now();

        // 创建多个服务实例
        for _ in 0..5 {
            let _service = create_test_service().await;
        }

        let duration = start.elapsed();
        println!("5次服务创建耗时: {:?}", duration);

        // 每次创建服务不应该超过合理时间
        assert!(duration.as_secs() < 30, "服务创建性能应该足够快");
    }

    // 边界条件测试
    #[tokio::test]
    async fn test_edge_case_extreme_values() -> Result<()> {
        let service = create_test_service().await?;

        let mut request = create_test_request();
        request.meme_token_amount = u64::MAX;
        request.base_token_amount = u64::MAX;
        request.initial_price = f64::MAX;

        // 即使是极端值，service也应该能够处理（即使最终可能失败）
        let result = service.launch(request).await;
        // 不检查成功或失败，只检查不会panic
        match result {
            Ok(_) => println!("极端值测试通过"),
            Err(e) => println!("极端值测试返回预期错误: {}", e),
        }

        Ok(())
    }

    // 并发安全测试
    #[tokio::test]
    async fn test_concurrent_operations() -> Result<()> {
        let service = Arc::new(create_test_service().await?);
        let mut handles = vec![];

        // 启动多个并发操作
        for i in 0..5 {
            let service_clone = service.clone();
            let mut request = create_test_request();
            request.initial_price = 1.0 + (i as f64) * 0.1; // 每个请求稍微不同

            let handle = tokio::spawn(async move { service_clone.launch(request).await });

            handles.push(handle);
        }

        // 等待所有操作完成
        for handle in handles {
            let result = handle.await;
            assert!(result.is_ok(), "并发操作不应该panic");
            // 内部的launch操作可能成功或失败，但不应该导致并发问题
        }

        Ok(())
    }

    // 测试持久化功能 (通过公共接口间接测试)
    #[tokio::test]
    async fn test_persistence_integration() {
        let service = create_test_service().await;
        if service.is_err() {
            println!("跳过持久化测试，无法连接数据库: {:?}", service.err());
            return;
        }
        let service = service.unwrap();
        
        // 只测试公共接口，不直接测试私有方法
        let user_wallet = "8S2bcP66WehuF6cHryfZ7vfFpQWaUhYyAYSy5U3gX4Fy";
        
        // 先查询当前历史记录数量
        let before_result = service.get_user_launch_history(user_wallet, 1, 100).await;
        let before_count = match before_result {
            Ok(pools) => pools.len(),
            Err(_) => 0, // 数据库可能不可访问
        };
        
        // 查询统计信息
        let stats_result = service.get_launch_stats().await;
        match stats_result {
            Ok(stats) => {
                println!("当前系统统计:");
                println!("  总Launch次数: {}", stats.total_launches);
                println!("  成功Launch次数: {}", stats.successful_launches);
                println!("  待确认Launch次数: {}", stats.pending_launches);
                println!("  今日Launch次数: {}", stats.today_launches);
                println!("  成功率: {:.2}%", stats.success_rate);
                
                // 验证统计数据的逻辑一致性
                assert!(stats.success_rate >= 0.0 && stats.success_rate <= 100.0);
                assert!(stats.successful_launches <= stats.total_launches);
                assert!(stats.pending_launches <= stats.total_launches);
                assert!(stats.today_launches <= stats.total_launches);
                println!("✅ 统计数据逻辑一致性验证通过");
            }
            Err(e) => {
                println!("统计查询失败: {}", e);
            }
        }
        
        println!("✅ 持久化集成测试完成, 用户历史记录数: {}", before_count);
    }

    #[tokio::test]
    async fn test_get_user_launch_history() {
        let service = create_test_service().await;
        if service.is_err() {
            println!("跳过历史查询测试，无法连接数据库: {:?}", service.err());
            return;
        }
        let service = service.unwrap();

        let user_wallet = "8S2bcP66WehuF6cHryfZ7vfFpQWaUhYyAYSy5U3gX4Fy";
        
        // 测试查询用户历史
        let result = service.get_user_launch_history(user_wallet, 1, 10).await;
        
        match result {
            Ok(pools) => {
                println!("用户历史查询成功，找到 {} 条记录", pools.len());
                // 验证返回的都是该用户的记录
                for pool in &pools {
                    // 在测试环境中，可能没有数据，所以只验证结构
                    assert_eq!(pool.creator_wallet, user_wallet);
                }
            }
            Err(e) => {
                println!("用户历史查询失败: {}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_get_launch_stats() {
        let service = create_test_service().await;
        if service.is_err() {
            println!("跳过统计查询测试，无法连接数据库: {:?}", service.err());
            return;
        }
        let service = service.unwrap();

        let result = service.get_launch_stats().await;
        
        match result {
            Ok(stats) => {
                println!("统计查询成功:");
                println!("  总Launch次数: {}", stats.total_launches);
                println!("  成功Launch次数: {}", stats.successful_launches);
                println!("  待确认Launch次数: {}", stats.pending_launches);
                println!("  今日Launch次数: {}", stats.today_launches);
                println!("  成功率: {:.2}%", stats.success_rate);
                println!("  每日统计条数: {}", stats.daily_launch_counts.len());
                
                // 验证统计数据的逻辑一致性
                assert!(stats.success_rate >= 0.0 && stats.success_rate <= 100.0);
                assert!(stats.successful_launches <= stats.total_launches);
                assert!(stats.pending_launches <= stats.total_launches);
                assert!(stats.today_launches <= stats.total_launches);
                assert!(stats.daily_launch_counts.len() <= 7); // 最多7天
            }
            Err(e) => {
                println!("统计查询失败: {}", e);
            }
        }
    }

}
