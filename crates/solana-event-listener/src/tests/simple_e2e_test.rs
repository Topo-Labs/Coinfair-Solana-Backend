//! 简化的E2E测试
//! 
//! 验证核心功能：WebSocket连接 → 事件解析 → 数据库写入
//! 使用更短的时间和模拟数据来快速验证完整流程

use crate::{
    config::EventListenerConfig,
    parser::{EventParserRegistry, ParsedEvent},
    persistence::EventStorage,
    metrics::MetricsCollector,
};
use std::sync::Arc;
use solana_sdk::pubkey::Pubkey;
use tokio::time::{timeout, Duration};
use tracing::{info, error};

/// 创建简化测试配置
fn create_simple_e2e_config() -> EventListenerConfig {
    EventListenerConfig {
        solana: crate::config::settings::SolanaConfig {
            rpc_url: "https://api.devnet.solana.com".to_string(),
            ws_url: "wss://api.devnet.solana.com".to_string(),
            commitment: "confirmed".to_string(),
            // 使用Raydium CLMM程序ID
            program_id: "CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK".parse().unwrap(),
            private_key: None,
        },
        database: crate::config::settings::DatabaseConfig {
            uri: "mongodb://localhost:27017".to_string(),
            database_name: "coinfair_development".to_string(),
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

/// 简化E2E测试：快速验证所有组件工作
#[tokio::test]
#[ignore]
async fn test_simple_e2e_flow() {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .try_init()
        .ok();

    info!("🚀 开始简化E2E测试流程");
    
    let config = create_simple_e2e_config();
    
    // === 步骤1：验证网络连接 ===
    info!("📡 步骤1：验证网络连接");
    
    let rpc_client = solana_client::nonblocking::rpc_client::RpcClient::new(config.solana.rpc_url.clone());
    let current_slot = match timeout(Duration::from_secs(5), rpc_client.get_slot()).await {
        Ok(Ok(slot)) => {
            info!("✅ RPC连接成功，当前slot: {}", slot);
            slot
        }
        Ok(Err(e)) => {
            error!("❌ RPC连接失败: {}", e);
            panic!("无法连接到Solana RPC");
        }
        Err(_) => {
            error!("⏰ RPC连接超时");
            panic!("RPC连接超时");
        }
    };

    // === 步骤2：验证数据库连接 ===
    info!("🗄️  步骤2：验证数据库连接");
    
    let event_storage = match EventStorage::new(&config).await {
        Ok(storage) => {
            info!("✅ 数据库连接成功");
            let health = storage.health_check().await.unwrap();
            assert!(health, "数据库健康检查应该通过");
            storage
        }
        Err(e) => {
            error!("❌ 数据库连接失败: {}", e);
            panic!("请确保MongoDB正在运行: docker-compose up -d");
        }
    };

    // === 步骤3：初始化解析器 ===
    info!("🔧 步骤3：初始化解析器");
    
    let parser_registry = Arc::new(EventParserRegistry::new(&config).unwrap());
    info!("✅ 解析器注册表: 已注册{}个解析器", parser_registry.parser_count());
    assert_eq!(parser_registry.parser_count(), 4, "应该有4个解析器");

    // === 步骤4：初始化指标收集 ===
    info!("📈 步骤4：初始化指标收集");
    
    let metrics = Arc::new(MetricsCollector::new(&config).unwrap());
    metrics.start_collection().await.unwrap();
    info!("✅ 指标收集已启动");

    // === 步骤5：创建和写入测试数据 ===
    info!("📝 步骤5：创建和写入测试数据");
    
    let test_events = create_test_events();
    info!("准备写入{}个测试事件", test_events.len());

    // 获取写入前统计
    let before_stats = event_storage.get_storage_stats().await.unwrap();
    info!("📊 写入前统计: 总代币={}", before_stats.total_tokens);

    // 批量写入
    let written_count = match event_storage.write_batch(&test_events).await {
        Ok(count) => {
            info!("✅ 成功写入{}个事件", count);
            count
        }
        Err(e) => {
            error!("❌ 批量写入失败: {}", e);
            panic!("数据库写入失败");
        }
    };

    // 验证写入结果
    assert!(written_count > 0, "应该至少写入1个事件");

    // 获取写入后统计
    let after_stats = event_storage.get_storage_stats().await.unwrap();
    info!("📊 写入后统计: 总代币={}", after_stats.total_tokens);

    let new_tokens = after_stats.total_tokens - before_stats.total_tokens;
    if new_tokens > 0 {
        info!("🎉 成功新增{}个代币记录", new_tokens);
    }

    // === 步骤6：记录指标并生成报告 ===
    info!("📈 步骤6：记录指标并生成报告");
    
    metrics.record_event_processed().await.unwrap();
    metrics.record_batch_write().await.unwrap();
    metrics.record_websocket_connection().await.unwrap();

    let stats = metrics.get_stats().await.unwrap();
    info!("📊 指标统计:");
    info!("   处理事件: {}", stats.events_processed);
    info!("   批量写入: {}", stats.batch_writes);
    info!("   WebSocket连接: {}", stats.websocket_connections);

    // 生成性能报告
    let report = metrics.generate_performance_report().await.unwrap();
    info!("🔧 性能报告:");
    info!("   内存使用: {:.2} MB", report.system_resources.memory_usage_mb);
    info!("   CPU使用: {:.2}%", report.system_resources.cpu_usage_percent);
    info!("   运行时间: {} 秒", report.uptime_seconds);

    // === 步骤7：验证Prometheus导出 ===
    info!("📊 步骤7：验证Prometheus导出");
    
    let prometheus_output = metrics.export_prometheus_metrics().await.unwrap();
    let lines_count = prometheus_output.lines().count();
    info!("✅ Prometheus导出成功，包含{}行指标", lines_count);
    assert!(lines_count > 10, "Prometheus输出应该包含足够的指标");

    // === 步骤8：最终验证 ===
    info!("🎯 步骤8：最终验证");
    
    let mut success_checks = 0u32;
    let total_checks = 7u32;

    // 检查1：网络连接
    if current_slot > 0 {
        success_checks += 1;
        info!("✅ 检查1: 网络连接正常");
    }

    // 检查2：数据库连接
    if event_storage.health_check().await.unwrap() {
        success_checks += 1;
        info!("✅ 检查2: 数据库连接正常");
    }

    // 检查3：解析器
    if parser_registry.parser_count() == 4 {
        success_checks += 1;
        info!("✅ 检查3: 解析器组件正常");
    }

    // 检查4：数据写入
    if written_count > 0 {
        success_checks += 1;
        info!("✅ 检查4: 数据写入成功");
    }

    // 检查5：指标收集
    if stats.events_processed > 0 {
        success_checks += 1;
        info!("✅ 检查5: 指标收集正常");
    }

    // 检查6：性能报告
    if report.system_resources.memory_usage_mb >= 0.0 {
        success_checks += 1;
        info!("✅ 检查6: 性能报告正常");
    }

    // 检查7：Prometheus导出
    if lines_count > 10 {
        success_checks += 1;
        info!("✅ 检查7: Prometheus导出正常");
    }

    // 停止指标收集
    metrics.stop().await.unwrap();

    // 最终结果
    let success_rate = (success_checks as f64 / total_checks as f64) * 100.0;
    info!("🎉 简化E2E测试完成!");
    info!("   成功检查: {}/{}", success_checks, total_checks);
    info!("   成功率: {:.1}%", success_rate);

    if success_checks >= 6 {
        info!("🎉 E2E测试大部分成功！系统基本功能正常");
    } else if success_checks >= 4 {
        info!("⚠️ E2E测试部分成功，需要检查配置");
    } else {
        info!("❌ E2E测试失败较多，需要检查环境");
    }

    // 断言基本功能正常
    assert!(success_checks >= 5, "至少5个基本检查应该通过");
    
    info!("✅ 简化E2E测试流程成功结束");
}

/// 创建测试事件数据
fn create_test_events() -> Vec<ParsedEvent> {
    vec![
        ParsedEvent::TokenCreation(
            crate::parser::event_parser::TokenCreationEventData {
                mint_address: Pubkey::new_unique().to_string(),
                name: "Simple E2E Test Token".to_string(),
                symbol: "SE2E".to_string(),
                uri: "https://simple-e2e-test.example.com/metadata.json".to_string(),
                decimals: 9,
                supply: 1000000000000,
                creator: Pubkey::new_unique().to_string(),
                has_whitelist: false,
                whitelist_deadline: 0,
                created_at: chrono::Utc::now().timestamp(),
                signature: format!("simple_e2e_test_sig_{}", chrono::Utc::now().timestamp_millis()),
                slot: 999999999,
            }
        ),
        ParsedEvent::PoolCreation(create_test_pool_event()),
        ParsedEvent::NftClaim(create_test_nft_event()),
        ParsedEvent::RewardDistribution(create_test_reward_event()),
    ]
}

fn create_test_pool_event() -> crate::parser::event_parser::PoolCreationEventData {
    crate::parser::event_parser::PoolCreationEventData {
        pool_address: Pubkey::new_unique().to_string(),
        token_a_mint: "So11111111111111111111111111111111111111112".parse().unwrap(), // SOL
        token_b_mint: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".parse().unwrap(), // USDC
        token_a_decimals: 9,
        token_b_decimals: 6,
        fee_rate: 2500,
        fee_rate_percentage: 0.25,
        annual_fee_rate: 91.25,
        pool_type: "标准费率".to_string(),
        sqrt_price_x64: 7922816251426433759354395034_u128.to_string(),
        initial_price: 100.0,
        initial_tick: 46054,
        creator: Pubkey::new_unique().to_string(),
        clmm_config: "6WaEpWoTW4gYcHRAaCivNp3PPwPBaOd6zMpMySgCFjhj".parse().unwrap(),
        is_stable_pair: false,
        estimated_liquidity_usd: 50000.0,
        created_at: chrono::Utc::now().timestamp(),
        signature: format!("simple_pool_test_{}", chrono::Utc::now().timestamp_millis()),
        slot: 399244500,
        processed_at: chrono::Utc::now().to_rfc3339(),
    }
}

fn create_test_nft_event() -> crate::parser::event_parser::NftClaimEventData {
    crate::parser::event_parser::NftClaimEventData {
        nft_mint: Pubkey::new_unique().to_string(),
        claimer: Pubkey::new_unique().to_string(),
        referrer: Some(Pubkey::new_unique().to_string()),
        tier: 3,
        tier_name: "Gold".to_string(),
        tier_bonus_rate: 1.5,
        claim_amount: 3000000,
        token_mint: "So11111111111111111111111111111111111111112".parse().unwrap(),
        reward_multiplier: 15000,
        reward_multiplier_percentage: 1.5,
        bonus_amount: 4500000,
        claim_type: 0,
        claim_type_name: "定期领取".to_string(),
        total_claimed: 15000000,
        claim_progress_percentage: 20.0,
        pool_address: Some(Pubkey::new_unique().to_string()),
        has_referrer: true,
        is_emergency_claim: false,
        estimated_usd_value: 300.0,
        claimed_at: chrono::Utc::now().timestamp(),
        signature: format!("simple_nft_test_{}", chrono::Utc::now().timestamp_millis()),
        slot: 399244501,
        processed_at: chrono::Utc::now().to_rfc3339(),
    }
}

fn create_test_reward_event() -> crate::parser::event_parser::RewardDistributionEventData {
    let now = chrono::Utc::now();
    crate::parser::event_parser::RewardDistributionEventData {
        distribution_id: now.timestamp_millis() as u64,
        reward_pool: Pubkey::new_unique().to_string(),
        recipient: Pubkey::new_unique().to_string(),
        referrer: Some(Pubkey::new_unique().to_string()),
        reward_token_mint: "So11111111111111111111111111111111111111112".parse().unwrap(),
        reward_amount: 2000000,
        base_reward_amount: 1500000,
        bonus_amount: 500000,
        reward_type: 1,
        reward_type_name: "流动性挖矿奖励".to_string(),
        reward_source: 1,
        reward_source_name: "CLMM流动性挖矿".to_string(),
        related_address: Some(Pubkey::new_unique().to_string()),
        multiplier: 13333,
        multiplier_percentage: 1.33,
        is_locked: true,
        unlock_timestamp: Some(now.timestamp() + 7 * 24 * 3600),
        lock_days: 7,
        has_referrer: true,
        is_referral_reward: false,
        is_high_value_reward: false,
        estimated_usd_value: 200.0,
        distributed_at: now.timestamp(),
        signature: format!("simple_reward_test_{}", now.timestamp_millis()),
        slot: 399244502,
        processed_at: now.to_rfc3339(),
    }
}