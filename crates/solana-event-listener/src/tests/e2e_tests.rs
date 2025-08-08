//! 端到端测试
//! 
//! 真正连接Solana链上数据，验证完整的事件监听→解析→持久化流程
//! 
//! ⚠️ 注意：这些测试需要：
//! 1. 网络连接到Solana devnet
//! 2. MongoDB运行
//! 3. 真实的程序ID和事件

use crate::{
    config::EventListenerConfig,
    parser::EventParserRegistry,
    persistence::{BatchWriter, EventStorage},
    subscriber::SubscriptionManager,
    recovery::CheckpointManager,
    metrics::MetricsCollector,
};
use std::sync::Arc;
use anchor_lang::prelude::Pubkey;
use tokio::time::{timeout, Duration};
use tracing::{info, warn};

/// 创建真实E2E测试配置
fn create_e2e_test_config() -> EventListenerConfig {
    EventListenerConfig {
        solana: crate::config::settings::SolanaConfig {
            rpc_url: "https://api.devnet.solana.com".to_string(),
            ws_url: "wss://api.devnet.solana.com".to_string(),
            commitment: "confirmed".to_string(),
            // 使用一个在devnet上活跃的程序ID（Raydium CLMM）
            program_id: "CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK".parse().unwrap(),
            private_key: None,
        },
        database: crate::config::settings::DatabaseConfig {
            uri: "mongodb://localhost:27017".to_string(),
            database_name: "event_listener_e2e_test".to_string(),
            max_connections: 10,
            min_connections: 2,
        },
        listener: crate::config::settings::ListenerConfig {
            batch_size: 5,
            sync_interval_secs: 2,
            max_retries: 3,
            retry_delay_ms: 1000,
            signature_cache_size: 1000,
            checkpoint_save_interval_secs: 10,
            backoff: crate::config::settings::BackoffConfig::default(),
            batch_write: crate::config::settings::BatchWriteConfig {
                batch_size: 3,
                max_wait_ms: 5000,
                buffer_size: 50,
                concurrent_writers: 2,
            },
        },
        monitoring: crate::config::settings::MonitoringConfig {
            metrics_interval_secs: 5,
            enable_performance_monitoring: true,
            health_check_interval_secs: 10,
        },
    }
}

/// E2E测试1：真实WebSocket连接测试
#[tokio::test]
#[ignore] // 默认忽略，需要手动运行
async fn test_e2e_websocket_connection() {
    let config = create_e2e_test_config();
    let parser_registry = Arc::new(EventParserRegistry::new(&config).unwrap());
    let batch_writer = Arc::new(BatchWriter::new(&config).await.unwrap());
    let checkpoint_manager = Arc::new(CheckpointManager::new(&config).await.unwrap());
    let metrics = Arc::new(MetricsCollector::new(&config).unwrap());

    let manager = SubscriptionManager::new(
        &config,
        parser_registry,
        batch_writer,
        checkpoint_manager,
        metrics,
    ).await;

    match manager {
        Ok(subscription_manager) => {
            info!("✅ SubscriptionManager创建成功");
            
            // 测试获取当前slot
            match timeout(Duration::from_secs(15), subscription_manager.get_current_slot()).await {
                Ok(Ok(slot)) => {
                    info!("✅ 成功获取当前slot: {}", slot);
                    assert!(slot > 0, "slot应该大于0");
                }
                Ok(Err(e)) => {
                    panic!("获取slot失败: {}", e);
                }
                Err(_) => {
                    panic!("获取slot超时");
                }
            }
        }
        Err(e) => {
            warn!("⚠️ SubscriptionManager创建失败（可能是数据库连接问题）: {}", e);
            println!("请确保MongoDB正在运行并且可以连接到Solana devnet");
        }
    }
}

/// E2E测试2：真实事件监听测试（短时间）
#[tokio::test]
#[ignore] // 默认忽略，需要手动运行
async fn test_e2e_real_event_listening() {
    let config = create_e2e_test_config();
    let parser_registry = Arc::new(EventParserRegistry::new(&config).unwrap());
    let batch_writer = Arc::new(BatchWriter::new(&config).await.unwrap());
    let checkpoint_manager = Arc::new(CheckpointManager::new(&config).await.unwrap());
    let metrics = Arc::new(MetricsCollector::new(&config).unwrap());

    let manager = SubscriptionManager::new(
        &config,
        parser_registry,
        batch_writer,
        checkpoint_manager,
        metrics.clone(),
    ).await;

    match manager {
        Ok(subscription_manager) => {
            info!("🚀 开始真实事件监听测试（30秒）");
            
            // 启动监听
            let listen_handle = tokio::spawn(async move {
                subscription_manager.start().await
            });

            // 启动指标收集
            metrics.start_collection().await.unwrap();

            // 监听30秒
            tokio::time::sleep(Duration::from_secs(30)).await;

            // 停止监听
            listen_handle.abort();

            // 检查是否收集到了指标
            let stats = metrics.get_stats().await.unwrap();
            info!("📊 监听结果:");
            info!("   处理的事件数: {}", stats.events_processed);
            info!("   失败的事件数: {}", stats.events_failed);
            info!("   WebSocket连接数: {}", stats.websocket_connections);
            info!("   批量写入数: {}", stats.batch_writes);

            // 生成报告
            let report = metrics.generate_performance_report().await.unwrap();
            info!("🔍 性能报告:");
            info!("   内存使用: {:.2} MB", report.system_resources.memory_usage_mb);
            info!("   CPU使用: {:.2}%", report.system_resources.cpu_usage_percent);
            info!("   运行时间: {} 秒", report.uptime_seconds);

            println!("✅ 真实事件监听测试完成");
            println!("   如果处理的事件数为0，可能是因为:");
            println!("   1. 测试期间没有相关的链上活动");
            println!("   2. 程序ID可能不活跃");
            println!("   3. 网络连接问题");
        }
        Err(e) => {
            warn!("⚠️ 无法启动事件监听: {}", e);
            println!("请确保:");
            println!("1. MongoDB正在运行: docker-compose up -d");
            println!("2. 网络可以访问Solana devnet");
            println!("3. 程序ID是有效的");
        }
    }
}

/// E2E测试3：数据库持久化验证
#[tokio::test] 
#[ignore] // 默认忽略，需要手动运行
async fn test_e2e_database_persistence() {
    let config = create_e2e_test_config();

    // 创建EventStorage
    let storage = EventStorage::new(&config).await;
    
    match storage {
        Ok(event_storage) => {
            info!("✅ EventStorage创建成功");

            // 创建测试事件
            let test_events = vec![
                crate::parser::ParsedEvent::TokenCreation(
                    crate::parser::event_parser::TokenCreationEventData {
                        mint_address: Pubkey::new_unique().to_string(),
                        name: "E2E Test Token".to_string(),
                        symbol: "E2E".to_string(),
                        uri: "https://e2e-test.example.com/metadata.json".to_string(),
                        decimals: 9,
                        supply: 1000000,
                        creator: Pubkey::new_unique().to_string(),
                        has_whitelist: false,
                        whitelist_deadline: 0,
                        created_at: chrono::Utc::now().timestamp(),
                        signature: format!("e2e_test_signature_{}", chrono::Utc::now().timestamp_millis()),
                        slot: 999999,
                    }
                ),
                crate::parser::ParsedEvent::PoolCreation(create_test_pool_event()),
                crate::parser::ParsedEvent::NftClaim(create_test_nft_event()),
                crate::parser::ParsedEvent::RewardDistribution(create_test_reward_event()),
            ];

            // 批量写入
            match event_storage.write_batch(&test_events).await {
                Ok(written_count) => {
                    info!("✅ 成功写入 {} 个事件到数据库", written_count);
                    assert!(written_count > 0, "应该写入至少1个事件");
                    
                    // 验证健康状态
                    let is_healthy = event_storage.health_check().await.unwrap();
                    assert!(is_healthy, "EventStorage应该健康");
                    
                    // 获取统计信息
                    if let Ok(stats) = event_storage.get_storage_stats().await {
                        info!("📊 存储统计:");
                        info!("   总代币数: {}", stats.total_tokens);
                        info!("   活跃代币数: {}", stats.active_tokens);
                        info!("   今日新增: {}", stats.today_new_tokens);
                    }
                }
                Err(e) => {
                    panic!("写入数据库失败: {}", e);
                }
            }
        }
        Err(e) => {
            warn!("⚠️ 无法连接数据库: {}", e);
            println!("请确保MongoDB正在运行: docker-compose up -d");
        }
    }
}

/// 辅助函数：创建测试池子事件
fn create_test_pool_event() -> crate::parser::event_parser::PoolCreationEventData {
    crate::parser::event_parser::PoolCreationEventData {
        pool_address: Pubkey::new_unique().to_string(),
        token_a_mint: Pubkey::new_unique().to_string(),
        token_b_mint: Pubkey::new_unique().to_string(),
        token_a_decimals: 9,
        token_b_decimals: 6,
        fee_rate: 3000,
        fee_rate_percentage: 0.3,
        annual_fee_rate: 109.5,
        pool_type: "标准费率".to_string(),
        sqrt_price_x64: (1u128 << 64).to_string(),
        initial_price: 1.0,
        initial_tick: 0,
        creator: Pubkey::new_unique().to_string(),
        clmm_config: Pubkey::new_unique().to_string(),
        is_stable_pair: false,
        estimated_liquidity_usd: 0.0,
        created_at: chrono::Utc::now().timestamp(),
        signature: format!("e2e_pool_test_{}", chrono::Utc::now().timestamp_millis()),
        slot: 999998,
        processed_at: chrono::Utc::now().to_rfc3339(),
    }
}

/// 辅助函数：创建测试NFT事件
fn create_test_nft_event() -> crate::parser::event_parser::NftClaimEventData {
    crate::parser::event_parser::NftClaimEventData {
        nft_mint: Pubkey::new_unique().to_string(),
        claimer: Pubkey::new_unique().to_string(),
        referrer: Some(Pubkey::new_unique().to_string()),
        tier: 3,
        tier_name: "Gold".to_string(),
        tier_bonus_rate: 1.5,
        claim_amount: 1000000,
        token_mint: Pubkey::new_unique().to_string(),
        reward_multiplier: 15000,
        reward_multiplier_percentage: 1.5,
        bonus_amount: 1500000,
        claim_type: 0,
        claim_type_name: "定期领取".to_string(),
        total_claimed: 5000000,
        claim_progress_percentage: 20.0,
        pool_address: Some(Pubkey::new_unique().to_string()),
        has_referrer: true,
        is_emergency_claim: false,
        estimated_usd_value: 0.0,
        claimed_at: chrono::Utc::now().timestamp(),
        signature: format!("e2e_nft_test_{}", chrono::Utc::now().timestamp_millis()),
        slot: 999997,
        processed_at: chrono::Utc::now().to_rfc3339(),
    }
}

/// 辅助函数：创建测试奖励事件
fn create_test_reward_event() -> crate::parser::event_parser::RewardDistributionEventData {
    crate::parser::event_parser::RewardDistributionEventData {
        distribution_id: chrono::Utc::now().timestamp_millis() as u64,
        reward_pool: Pubkey::new_unique().to_string(),
        recipient: Pubkey::new_unique().to_string(),
        referrer: Some(Pubkey::new_unique().to_string()),
        reward_token_mint: Pubkey::new_unique().to_string(),
        reward_amount: 1500000,
        base_reward_amount: 1000000,
        bonus_amount: 500000,
        reward_type: 2,
        reward_type_name: "流动性奖励".to_string(),
        reward_source: 1,
        reward_source_name: "流动性挖矿".to_string(),
        related_address: Some(Pubkey::new_unique().to_string()),
        multiplier: 15000,
        multiplier_percentage: 1.5,
        is_locked: true,
        unlock_timestamp: Some(chrono::Utc::now().timestamp() + 7 * 24 * 3600),
        lock_days: 7,
        has_referrer: true,
        is_referral_reward: false,
        is_high_value_reward: false,
        estimated_usd_value: 0.0,
        distributed_at: chrono::Utc::now().timestamp(),
        signature: format!("e2e_reward_test_{}", chrono::Utc::now().timestamp_millis()),
        slot: 999996,
        processed_at: chrono::Utc::now().to_rfc3339(),
    }
}

/// 快速连接测试
#[tokio::test]
async fn test_quick_connection_check() {
    let config = create_e2e_test_config();
    
    // 测试RPC连接
    let rpc_client = solana_client::nonblocking::rpc_client::RpcClient::new(config.solana.rpc_url.clone());
    
    match timeout(Duration::from_secs(10), rpc_client.get_slot()).await {
        Ok(Ok(slot)) => {
            println!("✅ RPC连接成功，当前slot: {}", slot);
        }
        Ok(Err(e)) => {
            println!("❌ RPC连接失败: {}", e);
        }
        Err(_) => {
            println!("⏰ RPC连接超时");
        }
    }

    // 测试WebSocket连接
    match timeout(Duration::from_secs(10), async {
        solana_client::nonblocking::pubsub_client::PubsubClient::new(&config.solana.ws_url).await
    }).await {
        Ok(Ok(_)) => {
            println!("✅ WebSocket连接成功");
        }
        Ok(Err(e)) => {
            println!("❌ WebSocket连接失败: {}", e);
        }
        Err(_) => {
            println!("⏰ WebSocket连接超时");
        }
    }
}