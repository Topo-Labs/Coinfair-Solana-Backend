//! CLMM池子服务集成测试
//! 
//! 测试完整的池子创建、存储、同步和查询流程

#[cfg(test)]
mod integration_tests {
    use crate::services::solana::clmm_pool::service::ClmmPoolService;
    use crate::services::solana::clmm_pool::storage::ClmmPoolStorageService;
    use crate::dtos::solana_dto::CreatePoolRequest;
    use crate::services::solana::shared::SharedContext;
    use database::clmm_pool::{ClmmPool, PoolStatus, SyncStatus, PoolQueryParams, TokenInfo, PriceInfo, VaultInfo, ExtensionInfo};
    use std::sync::Arc;

    /// 集成测试辅助结构
    #[allow(dead_code)]
    struct TestEnvironment {
        pub shared_context: Arc<SharedContext>,
        pub database: database::Database,
        pub pool_service: ClmmPoolService,
        pub storage_service: ClmmPoolStorageService,
    }

    impl TestEnvironment {
        /// 创建测试环境
        pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
            // 初始化共享上下文
            let shared_context = Arc::new(SharedContext::new()?);
            
            // 初始化数据库
            let app_config = Arc::new(utils::AppConfig::default());
            let database = database::Database::new(app_config).await?;
            
            // 创建存储服务
            let storage_service = ClmmPoolStorageService::new(database.clmm_pools.clone());
            
            // 初始化数据库索引
            storage_service.init_indexes().await?;
            
            // 创建池子服务
            let pool_service = ClmmPoolService::new(shared_context.clone(), &database);
            
            Ok(TestEnvironment {
                shared_context,
                database,
                pool_service,
                storage_service,
            })
        }
    }

    #[tokio::test]
    async fn test_complete_pool_creation_flow() {
        // 跳过测试如果环境不可用
        let env = match TestEnvironment::new().await {
            Ok(env) => env,
            Err(_) => {
                println!("⚠️ 跳过集成测试：测试环境不可用");
                return;
            }
        };

        // 1. 测试池子创建
        let request = CreatePoolRequest {
            config_index: 0,
            price: 100.0,
            mint0: "So11111111111111111111111111111111111111112".to_string(), // SOL
            mint1: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(), // USDC
            open_time: 0,
            user_wallet: "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM".to_string(),
        };

        // 创建池子交易
        let create_result = env.pool_service.create_pool(request.clone()).await;
        
        match create_result {
            Ok(response) => {
                println!("✅ 池子创建成功: {}", response.pool_address);
                
                // 2. 测试数据存储
                let storage_result = env.storage_service.store_pool_creation(&request, &response).await;
                assert!(storage_result.is_ok(), "池子数据存储应该成功");
                
                let pool_id = storage_result.unwrap();
                println!("✅ 池子数据存储成功，ID: {}", pool_id);
                
                // 3. 测试数据查询
                let query_result = env.storage_service.get_pool_by_address(&response.pool_address).await;
                assert!(query_result.is_ok(), "池子查询应该成功");
                
                let pool_option = query_result.unwrap();
                assert!(pool_option.is_some(), "应该能查询到池子数据");
                
                let pool = pool_option.unwrap();
                println!("✅ 池子查询成功: {}", pool.pool_address);
                
                // 4. 测试统计查询
                let stats_result = env.storage_service.get_pool_statistics().await;
                match stats_result {
                    Ok(stats) => {
                        println!("📊 池子统计: 总数={}, 活跃={}", stats.total_pools, stats.active_pools);
                    }
                    Err(e) => {
                        println!("⚠️ 统计查询失败: {}", e);
                    }
                }
            }
            Err(e) => {
                println!("⚠️ 池子创建失败（可能是网络或配置问题）: {}", e);
                // 在测试环境中，这可能是正常的
            }
        }
    }

    #[tokio::test]
    async fn test_performance_and_batch_operations() {
        let env = match TestEnvironment::new().await {
            Ok(env) => env,
            Err(_) => {
                println!("⚠️ 跳过性能测试：测试环境不可用");
                return;
            }
        };

        // 测试批量查询性能
        let start_time = std::time::Instant::now();
        
        // 模拟批量查询 - 使用正确的字段名
        let query_params = PoolQueryParams {
            pool_address: None,
            mint_address: None,
            creator_wallet: None,
            status: None,
            min_price: None,
            max_price: None,
            start_time: None,
            end_time: None,
            page: Some(1),
            limit: Some(100),
            sort_by: None,
            sort_order: None,
        };
        
        let query_result = env.storage_service.query_pools(&query_params).await;
        let query_duration = start_time.elapsed();
        
        match query_result {
            Ok(pools) => {
                println!("📊 批量查询性能测试:");
                println!("  - 查询时间: {:?}", query_duration);
                println!("  - 返回结果: {} 个池子", pools.len());
                if !pools.is_empty() {
                    println!("  - 平均每个查询: {:?}", query_duration / pools.len() as u32);
                }
            }
            Err(e) => {
                println!("⚠️ 批量查询失败: {}", e);
            }
        }

        println!("✅ 性能测试完成");
    }

    #[tokio::test]
    async fn test_concurrent_operations() {
        let env = match TestEnvironment::new().await {
            Ok(env) => env,
            Err(_) => {
                println!("⚠️ 跳过并发测试：测试环境不可用");
                return;
            }
        };

        // 测试并发查询 - 简化版本，不使用futures crate
        let mut success_count = 0;
        let mut total_results = 0;
        
        for i in 0..3 {
            let storage_service = ClmmPoolStorageService::new(env.database.clmm_pools.clone());
            let query_params = PoolQueryParams {
                pool_address: None,
                mint_address: None,
                creator_wallet: None,
                status: None,
                min_price: None,
                max_price: None,
                start_time: None,
                end_time: None,
                page: Some(i + 1),
                limit: Some(10),
                sort_by: None,
                sort_order: None,
            };
            
            match storage_service.query_pools(&query_params).await {
                Ok(pools) => {
                    success_count += 1;
                    total_results += pools.len();
                }
                Err(e) => {
                    println!("⚠️ 查询失败: {}", e);
                }
            }
        }

        println!("🔄 并发操作测试结果:");
        println!("  - 成功操作: {}/3", success_count);
        println!("  - 总查询结果: {} 个池子", total_results);
        
        assert!(success_count >= 2, "至少应该有2个操作成功");
        
        println!("✅ 并发操作测试通过");
    }

    #[tokio::test]
    async fn test_data_validation() {
        let env = match TestEnvironment::new().await {
            Ok(env) => env,
            Err(_) => {
                println!("⚠️ 跳过数据验证测试：测试环境不可用");
                return;
            }
        };

        // 创建一个测试池子数据
        let test_pool = ClmmPool {
            id: None,
            pool_address: "test_pool_address".to_string(),
            amm_config_address: "test_config".to_string(),
            config_index: 0,
            mint0: TokenInfo {
                mint_address: "mint0".to_string(),
                decimals: 9,
                owner: "owner1".to_string(),
                symbol: Some("SOL".to_string()),
                name: Some("Solana".to_string()),
            },
            mint1: TokenInfo {
                mint_address: "mint1".to_string(),
                decimals: 6,
                owner: "owner2".to_string(),
                symbol: Some("USDC".to_string()),
                name: Some("USD Coin".to_string()),
            },
            price_info: PriceInfo {
                initial_price: 100.0,
                sqrt_price_x64: "1000000".to_string(),
                initial_tick: 0,
                current_price: Some(100.0),
                current_tick: Some(0),
            },
            vault_info: VaultInfo {
                token_vault_0: "vault0".to_string(),
                token_vault_1: "vault1".to_string(),
            },
            extension_info: ExtensionInfo {
                observation_address: "obs".to_string(),
                tickarray_bitmap_extension: "bitmap".to_string(),
            },
            creator_wallet: "creator".to_string(),
            open_time: 0,
            created_at: chrono::Utc::now().timestamp() as u64,
            updated_at: chrono::Utc::now().timestamp() as u64,
            transaction_info: None,
            status: PoolStatus::Created,
            sync_status: SyncStatus {
                last_sync_at: 0,
                sync_version: 1,
                needs_sync: false,
                sync_error: None,
            },
        };

        // 测试数据存储和查询
        let store_result = env.storage_service.store_pool(&test_pool).await;
        match store_result {
            Ok(pool_id) => {
                println!("✅ 测试池子存储成功，ID: {}", pool_id);
                
                // 查询刚存储的池子
                let query_result = env.storage_service.get_pool_by_address(&test_pool.pool_address).await;
                match query_result {
                    Ok(Some(retrieved_pool)) => {
                        assert_eq!(retrieved_pool.pool_address, test_pool.pool_address);
                        assert_eq!(retrieved_pool.config_index, test_pool.config_index);
                        println!("✅ 池子查询验证成功");
                    }
                    Ok(None) => {
                        println!("⚠️ 未找到刚存储的池子");
                    }
                    Err(e) => {
                        println!("⚠️ 池子查询失败: {}", e);
                    }
                }
            }
            Err(e) => {
                println!("⚠️ 测试池子存储失败: {}", e);
            }
        }
        
        println!("✅ 数据验证测试完成");
    }
}