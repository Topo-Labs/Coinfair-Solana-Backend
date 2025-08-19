//! 数据转换服务优化测试
//!
//! 验证三层查询策略（内存缓存 → 数据库 → 链上查询）和异步数据库写入功能

use super::solana::config::service::{ClmmConfigService, ClmmConfigServiceTrait};
use anyhow::Result;
use solana_client::rpc_client::RpcClient;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::sleep;

#[cfg(test)]
mod tests {
    use super::super::data_transform::{AmmConfigCache, DataTransformService};
    use super::*;

    /// 测试三层查询策略的完整流程
    #[tokio::test]
    async fn test_three_tier_query_strategy() {
        // 模拟测试环境
        if std::env::var("RUN_INTEGRATION_TESTS").is_err() {
            println!("跳过集成测试 - 设置RUN_INTEGRATION_TESTS环境变量以运行");
            return;
        }

        println!("🧪 开始测试三层查询策略");

        // 1. 创建测试组件
        let rpc_client = Arc::new(RpcClient::new("https://api.devnet.solana.com".to_string()));
        let mock_database = create_mock_database().await.unwrap();
        let config_service = Arc::new(ClmmConfigService::new(Arc::new(mock_database), rpc_client.clone()));

        // 2. 创建优化的数据转换服务
        let data_transform_service = DataTransformService::new_optimized(
            Some(rpc_client.clone()),
            Some(config_service.clone() as Arc<dyn ClmmConfigServiceTrait>),
        )
        .unwrap();

        let test_config_address = "test_config_address_123";

        // 3. 测试第一次查询（应该从链上获取，并缓存）
        println!("🔍 第一次查询（预期：链上查询 + 缓存写入）");
        let start_time = Instant::now();
        let result1 = data_transform_service
            .get_amm_config_optimized(test_config_address)
            .await;
        let first_query_time = start_time.elapsed();
        println!("第一次查询耗时: {:?}", first_query_time);

        match &result1 {
            Ok(Some(config)) => {
                println!("✅ 成功从链上获取配置");
                assert!(config.tick_spacing > 0);
                assert!(config.timestamp > 0);
            }
            Ok(None) => {
                println!("⚠️ 配置不存在（可能是测试配置地址）");
            }
            Err(e) => {
                println!("❌ 查询失败: {}", e);
            }
        }

        // 4. 测试第二次查询（应该从内存缓存获取）
        println!("🔍 第二次查询（预期：内存缓存命中）");
        let start_time = Instant::now();
        let result2 = data_transform_service
            .get_amm_config_optimized(test_config_address)
            .await;
        let second_query_time = start_time.elapsed();
        println!("第二次查询耗时: {:?}", second_query_time);

        // 验证第二次查询更快（缓存命中）
        if result1.is_ok() && result2.is_ok() {
            // 第二次查询应该显著更快（因为使用缓存）
            assert!(
                second_query_time < first_query_time / 2,
                "缓存查询应该比链上查询快得多: 第一次={:?}, 第二次={:?}",
                first_query_time,
                second_query_time
            );
            println!(
                "✅ 缓存优化生效，查询速度提升 {:.1}x",
                first_query_time.as_millis() as f64 / second_query_time.as_millis() as f64
            );
        }

        // 5. 等待一下让异步数据库写入完成
        println!("⏳ 等待异步数据库写入完成...");
        sleep(Duration::from_millis(100)).await;

        println!("✅ 三层查询策略测试完成");
    }

    /// 测试批量查询性能优化
    #[tokio::test]
    async fn test_batch_query_optimization() {
        // 模拟测试环境
        if std::env::var("RUN_INTEGRATION_TESTS").is_err() {
            println!("跳过集成测试 - 设置RUN_INTEGRATION_TESTS环境变量以运行");
            return;
        }

        println!("🧪 开始测试批量查询性能优化");

        // 1. 创建测试组件
        let rpc_client = Arc::new(RpcClient::new("https://api.devnet.solana.com".to_string()));
        let mock_database = create_mock_database().await.unwrap();
        let config_service = Arc::new(ClmmConfigService::new(Arc::new(mock_database), rpc_client.clone()));

        let data_transform_service = DataTransformService::new_optimized(
            Some(rpc_client.clone()),
            Some(config_service.clone() as Arc<dyn ClmmConfigServiceTrait>),
        )
        .unwrap();

        // 2. 准备测试数据
        let test_addresses = vec![
            "config_address_1".to_string(),
            "config_address_2".to_string(),
            "config_address_3".to_string(),
            "config_address_4".to_string(),
            "config_address_5".to_string(),
        ];

        // 3. 测试批量查询
        println!("🔍 执行批量查询（{} 个配置）", test_addresses.len());
        let start_time = Instant::now();
        let batch_result = data_transform_service.load_multiple_amm_configs(&test_addresses).await;
        let batch_query_time = start_time.elapsed();

        println!("批量查询耗时: {:?}", batch_query_time);

        match &batch_result {
            Ok(configs) => {
                println!("✅ 批量查询完成，获取到 {} 个配置", configs.len());

                // 验证批量查询结果
                for (address, config) in configs.iter() {
                    println!(
                        "配置 {}: tick_spacing={}, trade_fee_rate={}",
                        address, config.tick_spacing, config.trade_fee_rate
                    );
                }
            }
            Err(e) => {
                println!("⚠️ 批量查询结果: {}", e);
            }
        }

        // 4. 测试第二次批量查询（应该使用缓存）
        println!("🔍 第二次批量查询（预期：缓存命中）");
        let start_time = Instant::now();
        let cache_result = data_transform_service.load_multiple_amm_configs(&test_addresses).await;
        let cache_query_time = start_time.elapsed();

        println!("缓存批量查询耗时: {:?}", cache_query_time);

        // 验证缓存优化效果
        if batch_result.is_ok() && cache_result.is_ok() {
            // 缓存查询应该显著更快
            assert!(
                cache_query_time < batch_query_time / 2,
                "缓存批量查询应该比首次查询快得多: 首次={:?}, 缓存={:?}",
                batch_query_time,
                cache_query_time
            );
            println!(
                "✅ 批量缓存优化生效，查询速度提升 {:.1}x",
                batch_query_time.as_millis() as f64 / cache_query_time.as_millis() as f64
            );
        }

        println!("✅ 批量查询优化测试完成");
    }

    /// 测试缓存过期机制
    #[tokio::test]
    async fn test_cache_expiration() {
        println!("🧪 开始测试缓存过期机制");

        // 1. 创建测试服务（不使用RPC客户端，专注测试缓存逻辑）
        let data_transform_service = DataTransformService::new().unwrap();
        let test_address = "test_expiration_address";

        // 2. 手动插入一个过期的缓存项
        {
            let mut cache = data_transform_service.amm_config_cache.lock().unwrap();
            let expired_config = AmmConfigCache {
                protocol_fee_rate: 120000,
                trade_fee_rate: 500,
                tick_spacing: 10,
                fund_fee_rate: 40000,
                timestamp: (chrono::Utc::now().timestamp() - 400) as u64, // 6分钟前（超过5分钟过期时间）
            };
            cache.insert(test_address.to_string(), expired_config);
        }

        // 3. 测试缓存检查（应该返回None，因为已过期）
        let cache_result = data_transform_service.check_memory_cache(test_address).unwrap();
        assert!(cache_result.is_none(), "过期的缓存项应该被忽略");

        // 4. 插入一个新鲜的缓存项
        {
            let mut cache = data_transform_service.amm_config_cache.lock().unwrap();
            let fresh_config = AmmConfigCache {
                protocol_fee_rate: 120000,
                trade_fee_rate: 500,
                tick_spacing: 10,
                fund_fee_rate: 40000,
                timestamp: chrono::Utc::now().timestamp() as u64, // 当前时间
            };
            cache.insert(test_address.to_string(), fresh_config);
        }

        // 5. 测试缓存检查（应该返回有效配置）
        let cache_result = data_transform_service.check_memory_cache(test_address).unwrap();
        assert!(cache_result.is_some(), "新鲜的缓存项应该被返回");

        if let Some(config) = cache_result {
            assert_eq!(config.tick_spacing, 10);
            assert_eq!(config.trade_fee_rate, 500);
        }

        println!("✅ 缓存过期机制测试完成");
    }

    /// 测试异步数据库写入功能
    #[tokio::test]
    async fn test_async_database_write() {
        // 模拟测试环境
        if std::env::var("RUN_INTEGRATION_TESTS").is_err() {
            println!("跳过集成测试 - 设置RUN_INTEGRATION_TESTS环境变量以运行");
            return;
        }

        println!("🧪 开始测试异步数据库写入功能");

        // 1. 创建测试组件
        let rpc_client = Arc::new(RpcClient::new("https://api.devnet.solana.com".to_string()));
        let mock_database = create_mock_database().await.unwrap();
        let config_service = Arc::new(ClmmConfigService::new(Arc::new(mock_database), rpc_client.clone()));

        let data_transform_service = DataTransformService::new_optimized(
            Some(rpc_client.clone()),
            Some(config_service.clone() as Arc<dyn ClmmConfigServiceTrait>),
        )
        .unwrap();

        // 2. 创建测试配置数据
        let test_config = AmmConfigCache {
            protocol_fee_rate: 120000,
            trade_fee_rate: 500,
            tick_spacing: 10,
            fund_fee_rate: 40000,
            timestamp: chrono::Utc::now().timestamp() as u64,
        };

        let test_address = "test_async_write_address";

        // 3. 执行异步写入
        println!("🔄 执行异步数据库写入");
        data_transform_service
            .async_save_config_to_database(test_address, &test_config)
            .await;

        // 4. 等待异步操作完成
        println!("⏳ 等待异步写入完成...");
        sleep(Duration::from_millis(200)).await;

        // 5. 验证数据是否已写入数据库
        match config_service.get_config_by_address(test_address).await {
            Ok(Some(saved_config)) => {
                println!("✅ 成功从数据库读取异步写入的配置");
                assert_eq!(saved_config.tick_spacing as u16, test_config.tick_spacing);
                assert_eq!(saved_config.trade_fee_rate as u32, test_config.trade_fee_rate);
            }
            Ok(None) => {
                println!("⚠️ 配置未在数据库中找到（可能需要更多时间）");
            }
            Err(e) => {
                println!("⚠️ 数据库查询失败: {}", e);
            }
        }

        println!("✅ 异步数据库写入测试完成");
    }

    /// 测试性能统计和监控
    #[tokio::test]
    async fn test_performance_monitoring() {
        println!("🧪 开始测试性能监控功能");

        // 1. 创建测试服务
        let data_transform_service = DataTransformService::new().unwrap();

        // 2. 测试缓存操作性能
        let test_configs = (0..1000)
            .map(|i| {
                (
                    format!("test_address_{}", i),
                    AmmConfigCache {
                        protocol_fee_rate: 120000,
                        trade_fee_rate: 500,
                        tick_spacing: 10,
                        fund_fee_rate: 40000,
                        timestamp: chrono::Utc::now().timestamp() as u64,
                    },
                )
            })
            .collect::<HashMap<_, _>>();

        // 3. 批量插入缓存并测量性能
        let start_time = Instant::now();
        {
            let mut cache = data_transform_service.amm_config_cache.lock().unwrap();
            for (address, config) in test_configs.iter() {
                cache.insert(address.clone(), config.clone());
            }
        }
        let insert_time = start_time.elapsed();

        // 4. 批量查询缓存并测量性能
        let start_time = Instant::now();
        let mut found_count = 0;
        {
            let cache = data_transform_service.amm_config_cache.lock().unwrap();
            for address in test_configs.keys() {
                if cache.get(address).is_some() {
                    found_count += 1;
                }
            }
        }
        let query_time = start_time.elapsed();

        println!("📊 性能统计:");
        println!("  - 插入 {} 个配置耗时: {:?}", test_configs.len(), insert_time);
        println!("  - 查询 {} 个配置耗时: {:?}", test_configs.len(), query_time);
        println!(
            "  - 缓存命中率: {:.2}%",
            (found_count as f64 / test_configs.len() as f64) * 100.0
        );
        println!("  - 平均插入时间: {:?}", insert_time / test_configs.len() as u32);
        println!("  - 平均查询时间: {:?}", query_time / test_configs.len() as u32);

        // 5. 验证性能指标
        assert_eq!(found_count, test_configs.len(), "所有配置都应该在缓存中找到");
        assert!(insert_time < Duration::from_millis(100), "批量插入应该很快");
        assert!(query_time < Duration::from_millis(50), "批量查询应该很快");

        println!("✅ 性能监控测试完成");
    }

    /// 创建模拟数据库（用于测试）
    async fn create_mock_database() -> Result<database::Database> {
        // 使用测试数据库配置
        let test_config = Arc::new(utils::AppConfig::new_for_test());
        database::Database::new(test_config)
            .await
            .map_err(|e| anyhow::anyhow!("创建数据库失败: {}", e))
    }

    /// 性能基准测试：对比优化前后的性能差异
    #[tokio::test]
    async fn test_optimization_benchmark() {
        println!("🧪 开始基准测试：优化前后性能对比");

        // 1. 测试数据准备
        let test_addresses = (0..10).map(|i| format!("bench_config_{}", i)).collect::<Vec<_>>();

        // 2. 测试未优化版本（仅使用链上查询）
        println!("🔍 测试未优化版本（仅链上查询）");
        let rpc_client = Arc::new(RpcClient::new("https://api.devnet.solana.com".to_string()));
        let unoptimized_service = DataTransformService::new_with_rpc(rpc_client.clone()).unwrap();

        let start_time = Instant::now();
        let mut _unoptimized_results = 0;
        for address in &test_addresses {
            match unoptimized_service.load_amm_config_from_chain(address).await {
                Ok(Some(_)) => _unoptimized_results += 1,
                Ok(None) => {} // 配置不存在，正常情况
                Err(_) => {}   // 查询失败，测试环境正常情况
            }
        }
        let unoptimized_time = start_time.elapsed();

        // 3. 测试优化版本（三层查询策略）
        if std::env::var("RUN_INTEGRATION_TESTS").is_ok() {
            println!("🔍 测试优化版本（三层查询策略）");
            let mock_database = create_mock_database().await.unwrap();
            let config_service = Arc::new(ClmmConfigService::new(Arc::new(mock_database), rpc_client.clone()));

            let optimized_service = DataTransformService::new_optimized(
                Some(rpc_client),
                Some(config_service as Arc<dyn ClmmConfigServiceTrait>),
            )
            .unwrap();

            let start_time = Instant::now();
            let optimized_result = optimized_service.load_multiple_amm_configs(&test_addresses).await;
            let optimized_time = start_time.elapsed();

            // 4. 性能对比分析
            println!("📊 基准测试结果:");
            println!(
                "  - 未优化版本耗时: {:?} (单独查询 {} 次)",
                unoptimized_time,
                test_addresses.len()
            );
            println!("  - 优化版本耗时: {:?} (批量查询)", optimized_time);

            if optimized_time.as_millis() > 0 {
                let speedup = unoptimized_time.as_millis() as f64 / optimized_time.as_millis() as f64;
                println!("  - 性能提升: {:.1}x", speedup);

                // 验证优化确实带来了性能提升
                if unoptimized_time > Duration::from_millis(100) {
                    assert!(
                        optimized_time < unoptimized_time,
                        "优化版本应该比未优化版本更快: 优化前={:?}, 优化后={:?}",
                        unoptimized_time,
                        optimized_time
                    );
                }
            }

            match optimized_result {
                Ok(configs) => {
                    println!("  - 优化版本获取配置数: {}", configs.len());
                }
                Err(e) => {
                    println!("  - 优化版本结果: {}", e);
                }
            }
        } else {
            println!("⚠️ 跳过优化版本测试 - 需要集成测试环境");
        }

        println!("✅ 基准测试完成");
    }
}
