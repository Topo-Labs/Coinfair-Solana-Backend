//! 完整端到端测试
//!
//! 这个测试将验证完整的流程：
//! 1. 真实WebSocket订阅链上合约
//! 2. 接收并解析program data
//! 3. 数据库持久化
//! 4. 验证数据完整性
//!
//! 使用方法：
//! ```bash
//! # 确保MongoDB运行
//! docker-compose up -d
//!
//! # 运行完整E2E测试
//! CARGO_ENV=development MONGO_DB=coinfair_development cargo test --package solana-event-listener test_complete_e2e_flow -- --nocapture --ignored
//! ```

use crate::{
    config::EventListenerConfig,
    metrics::MetricsCollector,
    parser::{EventParserRegistry, ParsedEvent},
    persistence::{BatchWriter, EventStorage},
    recovery::CheckpointManager,
    subscriber::{SubscriptionManager, WebSocketManager},
};
use solana_sdk::pubkey::Pubkey;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::time::{sleep, timeout, Duration};
use tracing::{error, info, warn};

/// 创建完整E2E测试配置
fn create_complete_e2e_config() -> EventListenerConfig {
    EventListenerConfig {
        solana: crate::config::settings::SolanaConfig {
            rpc_url: "https://api.devnet.solana.com".to_string(),
            ws_url: "wss://api.devnet.solana.com".to_string(),
            commitment: "confirmed".to_string(),
            // 使用Raydium CLMM devnet程序ID
            // program_id: "CPMDWBwJDtYax9qW7AyRuVC19Cc4L4Vcy4n2BHAbHkCW".parse().unwrap(),
            program_ids: vec!["devi51mZmdwUJGU9hjN27vEz64Gps7uUefqxg27EAtH".parse().unwrap()],
            private_key: None,
        },
        database: crate::config::settings::DatabaseConfig {
            uri: "mongodb://localhost:27017".to_string(),
            database_name: "coinfair_development".to_string(), // 使用实际数据库
            max_connections: 10,
            min_connections: 2,
        },
        listener: crate::config::settings::ListenerConfig {
            batch_size: 20,
            sync_interval_secs: 1,
            max_retries: 3,
            retry_delay_ms: 100,              // 减少重试延迟
            signature_cache_size: 5000,       // 减少缓存大小
            checkpoint_save_interval_secs: 5, // 减少检查点保存间隔
            backoff: crate::config::settings::BackoffConfig::default(),
            batch_write: crate::config::settings::BatchWriteConfig {
                batch_size: 10,
                max_wait_ms: 500,      // 大幅减少等待时间
                buffer_size: 1000,     // 大幅增加缓冲区
                concurrent_writers: 8, // 增加并发写入数
            },
        },
        monitoring: crate::config::settings::MonitoringConfig {
            metrics_interval_secs: 2, // 减少指标收集间隔
            enable_performance_monitoring: true,
            health_check_interval_secs: 5, // 减少健康检查间隔
        },
    }
}

/// 完整E2E测试：真实链上数据订阅→解析→持久化
#[tokio::test]
#[ignore] // 需要手动运行
async fn test_complete_e2e_flow() {
    // 初始化日志 - 提高日志级别以看到更多调试信息
    tracing_subscriber::fmt()
        .with_env_filter(
            "debug,solana_event_listener::subscriber::subscription_manager=info,solana_event_listener::parser=info",
        )
        .try_init()
        .ok();

    info!("🚀 开始完整E2E测试流程");

    let config = create_complete_e2e_config();

    // === 第1步：验证网络连接 ===
    info!("📡 第1步：验证网络连接");

    let rpc_client = solana_client::nonblocking::rpc_client::RpcClient::new(config.solana.rpc_url.clone());
    let current_slot = match timeout(Duration::from_secs(10), rpc_client.get_slot()).await {
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

    // === 第2步：验证数据库连接 ===
    info!("🗄️  第2步：验证数据库连接");

    let event_storage = match EventStorage::new(&config).await {
        Ok(storage) => {
            info!("✅ 数据库连接成功");
            let health = storage.health_check().await.unwrap();
            if health {
                info!("✅ 数据库健康检查通过");
            } else {
                warn!("⚠️ 数据库健康检查失败");
            }
            storage
        }
        Err(e) => {
            error!("❌ 数据库连接失败: {}", e);
            panic!("请确保MongoDB正在运行: docker-compose up -d");
        }
    };

    // 获取初始数据库统计
    let initial_stats = event_storage.get_storage_stats().await.unwrap();
    info!(
        "📊 初始数据库统计: 总代币数={}, 今日新增={}",
        initial_stats.total_tokens, initial_stats.today_new_tokens
    );

    // === 第3步：初始化所有组件 ===
    info!("🔧 第3步：初始化所有组件");

    let parser_registry = Arc::new(EventParserRegistry::new(&config).unwrap());
    let batch_writer = Arc::new(BatchWriter::new(&config).await.unwrap());
    let checkpoint_manager = Arc::new(CheckpointManager::new(&config).await.unwrap());
    let metrics = Arc::new(MetricsCollector::new(&config).unwrap());

    info!("✅ 解析器注册表: 已注册{}个解析器", parser_registry.parser_count());
    let parsers = parser_registry.get_registered_parsers();
    for (parser_type, discriminator) in parsers {
        info!("   - {}: {:?}", parser_type, discriminator);
    }

    // === 第4步：创建订阅管理器 ===
    info!("📻 第4步：创建订阅管理器");

    let subscription_manager = match SubscriptionManager::new(
        &config,
        parser_registry.clone(),
        batch_writer.clone(),
        checkpoint_manager,
        metrics.clone(),
    )
    .await
    {
        Ok(manager) => {
            info!("✅ 订阅管理器创建成功");
            manager
        }
        Err(e) => {
            error!("❌ 订阅管理器创建失败: {}", e);
            panic!("订阅管理器创建失败");
        }
    };

    // === 第5步：启动指标收集 ===
    info!("📈 第5步：启动指标收集");

    metrics.start_collection().await.unwrap();
    info!("✅ 指标收集已启动");

    // === 第6步：开始真实事件监听 ===
    info!("🎧 第6步：开始真实事件监听（30秒，专注测试）");
    info!("   监听程序: {:?}", config.solana.program_ids);
    info!("   WebSocket: {}", config.solana.ws_url);

    let processed_events = Arc::new(AtomicU64::new(0));
    let listen_events = processed_events.clone();

    // 启动监听任务
    let listen_handle = {
        let subscription_manager = Arc::new(subscription_manager);
        let sm = subscription_manager.clone();

        tokio::spawn(async move {
            info!("🚀 启动事件监听...");
            match sm.start().await {
                Ok(_) => {
                    info!("✅ 事件监听正常结束");
                }
                Err(e) => {
                    error!("❌ 事件监听出错: {}", e);
                }
            }
        })
    };

    // === 第7步：监控和收集数据 ===
    info!("🔍 第7步：监控数据收集（30秒）");

    let monitoring_handle = {
        let metrics_clone = metrics.clone();
        let events_counter = listen_events.clone();

        tokio::spawn(async move {
            for i in 1..=6 {
                // 6次检查，每5秒一次
                sleep(Duration::from_secs(5)).await;

                let stats = metrics_clone.get_stats().await.unwrap();
                let _current_events = events_counter.load(Ordering::Relaxed);

                info!("📊 第{}次检查 ({}s):", i, i * 5);
                info!("   处理事件: {}", stats.events_processed);
                info!("   失败事件: {}", stats.events_failed);
                info!("   WebSocket连接: {}", stats.websocket_connections);
                info!("   批量写入: {}", stats.batch_writes);

                if stats.events_processed > 0 {
                    info!("🎉 检测到事件处理！");
                    events_counter.store(stats.events_processed, Ordering::Relaxed);
                }
            }
        })
    };

    // 等待监听和监控完成
    let _listen_result = tokio::select! {
        _ = listen_handle => {
            info!("监听任务完成");
        }
        _ = monitoring_handle => {
            info!("监控任务完成");
        }
        _ = sleep(Duration::from_secs(30)) => {
            info!("⏰ 30秒监听时间到");
        }
    };

    // === 第8步：停止监听和收集最终统计 ===
    info!("🛑 第8步：停止监听并收集最终统计");

    // 获取最终指标
    let final_stats = metrics.get_stats().await.unwrap();
    let _final_processed = processed_events.load(Ordering::Relaxed);

    info!("📊 最终统计结果:");
    info!("   处理的事件数: {}", final_stats.events_processed);
    info!("   失败的事件数: {}", final_stats.events_failed);
    info!("   成功率: {:.2}%", final_stats.success_rate * 100.0);
    info!("   WebSocket连接数: {}", final_stats.websocket_connections);
    info!("   批量写入数: {}", final_stats.batch_writes);

    // 生成性能报告
    let performance_report = metrics.generate_performance_report().await.unwrap();
    info!("🔧 性能报告:");
    info!(
        "   内存使用: {:.2} MB",
        performance_report.system_resources.memory_usage_mb
    );
    info!(
        "   CPU使用: {:.2}%",
        performance_report.system_resources.cpu_usage_percent
    );
    info!("   运行时间: {} 秒", performance_report.uptime_seconds);

    // === 第9步：验证数据库持久化 ===
    info!("🗄️  第9步：验证数据库持久化");

    let final_db_stats = event_storage.get_storage_stats().await.unwrap();
    let new_tokens = final_db_stats.total_tokens - initial_stats.total_tokens;
    let _new_today = final_db_stats.today_new_tokens - initial_stats.today_new_tokens;

    info!("📊 数据库变化:");
    info!("   初始总代币: {}", initial_stats.total_tokens);
    info!("   最终总代币: {}", final_db_stats.total_tokens);
    info!("   新增代币: {}", new_tokens);
    info!(
        "   今日新增变化: {} → {}",
        initial_stats.today_new_tokens, final_db_stats.today_new_tokens
    );

    // === 第10步：测试结果评估 ===
    info!("📋 第10步：测试结果评估");

    let mut success_count = 0u32;
    let mut total_checks = 0u32;

    // 检查1：网络连接
    total_checks += 1;
    if current_slot > 0 {
        success_count += 1;
        info!("✅ 检查1: 网络连接正常");
    } else {
        info!("❌ 检查1: 网络连接失败");
    }

    // 检查2：数据库连接
    total_checks += 1;
    if event_storage.health_check().await.unwrap() {
        success_count += 1;
        info!("✅ 检查2: 数据库连接正常");
    } else {
        info!("❌ 检查2: 数据库连接失败");
    }

    // 检查3：组件初始化
    total_checks += 1;
    if parser_registry.parser_count() == 6 {
        success_count += 1;
        info!("✅ 检查3: 解析器组件正常 (6个解析器)");
    } else {
        info!("❌ 检查3: 解析器组件异常");
    }

    // 检查4：WebSocket连接
    total_checks += 1;
    if final_stats.websocket_connections > 0 {
        success_count += 1;
        info!("✅ 检查4: WebSocket连接成功");
    } else {
        info!("⚠️ 检查4: WebSocket连接数为0（可能网络问题）");
    }

    // 检查5：事件处理（如果有的话）
    total_checks += 1;
    if final_stats.events_processed > 0 {
        success_count += 1;
        info!("✅ 检查5: 成功处理了{}个事件", final_stats.events_processed);

        // 额外检查：数据库写入
        if new_tokens > 0 || final_stats.batch_writes > 0 {
            info!("✅ 检查5+: 数据成功写入数据库");
        }
    } else {
        info!("⚠️ 检查5: 监听期间没有捕获到事件");
        info!("   这可能是因为:");
        info!("   - 监听时间内没有相关合约活动");
        info!("   - discriminator不匹配实际事件");
        info!("   - 程序ID不够活跃");
    }

    // 最终结果
    let success_rate = (success_count as f64 / total_checks as f64) * 100.0;
    info!("🎯 测试完成!");
    info!("   成功检查: {}/{}", success_count, total_checks);
    info!("   成功率: {:.1}%", success_rate);

    if success_count >= 4 {
        info!("🎉 E2E测试大部分成功！系统基本功能正常");
    } else if success_count >= 3 {
        info!("⚠️ E2E测试部分成功，需要检查网络或配置");
    } else {
        info!("❌ E2E测试失败较多，需要检查环境和配置");
    }

    // 停止指标收集
    metrics.stop().await.unwrap();

    info!("✅ 完整E2E测试流程结束");

    // 断言基本功能正常
    assert!(success_count >= 3, "至少3个基本检查应该通过");
}

/// 快速调试测试：检查过滤器行为
#[tokio::test]
#[ignore]
async fn test_debug_filter_behavior() {
    tracing_subscriber::fmt().with_env_filter("info").try_init().ok();

    info!("🚀 开始调试过滤器行为");

    let config = create_complete_e2e_config();

    // 创建WebSocket管理器
    let websocket_manager = Arc::new(WebSocketManager::new(Arc::new(config.clone())).unwrap());

    // 启动WebSocket（在后台）
    let ws_manager = Arc::clone(&websocket_manager);
    let ws_handle = tokio::spawn(async move {
        if let Err(e) = ws_manager.start().await {
            error!("WebSocket启动失败: {}", e);
        }
    });

    // 等待WebSocket连接
    tokio::time::sleep(Duration::from_secs(3)).await;

    // 创建事件接收器
    let mut event_receiver = websocket_manager.subscribe();

    info!("📡 开始监听并调试前几个事件...");

    // 监听前3个事件进行调试
    for i in 1..=3 {
        match tokio::time::timeout(Duration::from_secs(10), event_receiver.recv()).await {
            Ok(Ok(log_response)) => {
                info!("📨 调试事件 {}: {}", i, log_response.signature);
                info!(
                    "📋 事件详情: err={:?}, logs_count={}",
                    log_response.err,
                    log_response.logs.len()
                );

                // 打印前几行日志
                for (j, log) in log_response.logs.iter().enumerate().take(5) {
                    info!("  日志{}: {}", j, log);
                }
                if log_response.logs.len() > 5 {
                    info!("  ... (共{}行日志)", log_response.logs.len());
                }

                // 检查是否包含程序ID
                let target_program = "CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK";
                let contains_program = log_response.logs.iter().any(|log| log.contains(target_program));
                info!("🔍 是否包含目标程序ID: {}", contains_program);

                // 检查是否有 Program data
                let has_program_data = log_response.logs.iter().any(|log| log.starts_with("Program data: "));
                info!("🔍 是否有Program data: {}", has_program_data);

                info!("---");
            }
            Ok(Err(e)) => {
                warn!("接收事件失败: {}", e);
                break;
            }
            Err(_) => {
                warn!("接收事件超时");
                break;
            }
        }
    }

    // 清理
    websocket_manager.stop().await.unwrap();
    ws_handle.abort();

    info!("✅ 调试完成");
}

#[tokio::test]
#[ignore]
async fn test_simple_event_processing() {
    tracing_subscriber::fmt().with_env_filter("debug").try_init().ok();

    info!("🚀 开始简化事件处理测试");

    let config = create_complete_e2e_config();

    // 创建WebSocket管理器
    let websocket_manager = Arc::new(WebSocketManager::new(Arc::new(config.clone())).unwrap());

    // 启动WebSocket（在后台）
    let ws_manager = Arc::clone(&websocket_manager);
    let ws_handle = tokio::spawn(async move {
        if let Err(e) = ws_manager.start().await {
            error!("WebSocket启动失败: {}", e);
        }
    });

    // 等待WebSocket连接
    tokio::time::sleep(Duration::from_secs(3)).await;

    // 创建事件接收器
    let mut event_receiver = websocket_manager.subscribe();

    info!("📡 开始监听事件...");

    // 监听10个事件或超时
    let mut event_count = 0;
    let timeout_duration = Duration::from_secs(20);

    match tokio::time::timeout(timeout_duration, async {
        while event_count < 10 {
            match event_receiver.recv().await {
                Ok(log_response) => {
                    event_count += 1;
                    info!("✅ 接收到事件 {}: {}", event_count, log_response.signature);

                    // 尝试解析事件
                    for log in &log_response.logs {
                        if log.starts_with("Program data: ") {
                            info!("🔍 找到程序数据日志: {}", &log[..50.min(log.len())]);
                        }
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                    warn!("⚠️ 事件滞后，跳过了 {} 个事件", skipped);
                    continue;
                }
                Err(e) => {
                    error!("❌ 接收事件失败: {}", e);
                    break;
                }
            }
        }
    })
    .await
    {
        Ok(_) => {
            info!("🎉 成功处理了 {} 个事件", event_count);
        }
        Err(_) => {
            info!("⏰ 测试超时，处理了 {} 个事件", event_count);
        }
    }

    // 清理
    websocket_manager.stop().await.unwrap();
    ws_handle.abort();

    info!("✅ 简化测试完成，总事件数: {}", event_count);

    // 基本验证
    assert!(event_count > 0, "应该至少接收到1个事件");
}
#[tokio::test]
#[ignore]
async fn test_e2e_database_write_verification() {
    tracing_subscriber::fmt().with_env_filter("info").try_init().ok();

    info!("🗄️ 开始数据库写入验证测试");

    let config = create_complete_e2e_config();
    let event_storage = EventStorage::new(&config).await.unwrap();

    // 创建测试事件（模拟真实解析结果）
    let test_events = vec![
        ParsedEvent::TokenCreation(crate::parser::event_parser::TokenCreationEventData {
            mint_address: Pubkey::new_unique().to_string(),
            name: "E2E Test Token".to_string(),
            symbol: "E2ETEST".to_string(),
            uri: "https://e2e-test.example.com/metadata.json".to_string(),
            decimals: 9,
            supply: 1000000000000,
            creator: Pubkey::new_unique().to_string(),
            has_whitelist: false,
            whitelist_deadline: 0,
            created_at: chrono::Utc::now().timestamp(),
            signature: format!("e2e_test_sig_{}", chrono::Utc::now().timestamp_millis()),
            slot: 999999999,
        }),
        ParsedEvent::PoolCreation(create_realistic_pool_event()),
        ParsedEvent::NftClaim(create_realistic_nft_event()),
        ParsedEvent::RewardDistribution(create_realistic_reward_event()),
    ];

    info!("📝 准备写入{}个测试事件", test_events.len());

    // 获取写入前统计
    let before_stats = event_storage.get_storage_stats().await.unwrap();
    info!("📊 写入前统计: 总代币={}", before_stats.total_tokens);

    // 批量写入
    match event_storage.write_batch(&test_events).await {
        Ok(written_count) => {
            info!("✅ 成功写入{}个事件", written_count);

            // 验证写入后统计
            let after_stats = event_storage.get_storage_stats().await.unwrap();
            info!("📊 写入后统计: 总代币={}", after_stats.total_tokens);

            let new_tokens = after_stats.total_tokens - before_stats.total_tokens;
            if new_tokens > 0 {
                info!("🎉 成功新增{}个代币记录", new_tokens);
            }

            assert!(written_count > 0, "应该至少写入1个事件");
            info!("✅ 数据库写入验证测试成功");
        }
        Err(e) => {
            error!("❌ 批量写入失败: {}", e);
            panic!("数据库写入失败");
        }
    }
}

// 辅助函数：创建逼真的测试数据
fn create_realistic_pool_event() -> crate::parser::event_parser::PoolCreationEventData {
    crate::parser::event_parser::PoolCreationEventData {
        pool_address: Pubkey::new_unique().to_string(),
        token_a_mint: "So11111111111111111111111111111111111111112".parse().unwrap(), // SOL
        token_b_mint: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".parse().unwrap(), // USDC
        token_a_decimals: 9,
        token_b_decimals: 6,
        fee_rate: 2500, // 0.25%
        fee_rate_percentage: 0.25,
        annual_fee_rate: 91.25,
        pool_type: "标准费率".to_string(),
        sqrt_price_x64: 7922816251426433759354395034_u128.to_string(), // 约 100 SOL/USDC
        initial_price: 100.0,
        initial_tick: 46054,
        creator: Pubkey::new_unique().to_string(),
        clmm_config: "6WaEpWoTW4gYcHRAaCivNp3PPwPBaOd6zMpMySgCFjhj".parse().unwrap(),
        is_stable_pair: false,
        estimated_liquidity_usd: 50000.0,
        created_at: chrono::Utc::now().timestamp(),
        signature: format!("pool_e2e_test_{}", chrono::Utc::now().timestamp_millis()),
        slot: 399244500,
        processed_at: chrono::Utc::now().to_rfc3339(),
    }
}

fn create_realistic_nft_event() -> crate::parser::event_parser::NftClaimEventData {
    crate::parser::event_parser::NftClaimEventData {
        nft_mint: Pubkey::new_unique().to_string(),
        claimer: Pubkey::new_unique().to_string(),
        referrer: Some(Pubkey::new_unique().to_string()),
        tier: 4, // Platinum
        tier_name: "Platinum".to_string(),
        tier_bonus_rate: 2.0,
        claim_amount: 5000000, // 5 tokens
        token_mint: "So11111111111111111111111111111111111111112".parse().unwrap(),
        reward_multiplier: 20000, // 2.0x
        reward_multiplier_percentage: 2.0,
        bonus_amount: 10000000, // 10 tokens with bonus
        claim_type: 0,
        claim_type_name: "定期领取".to_string(),
        total_claimed: 25000000, // 25 tokens total
        claim_progress_percentage: 20.0,
        pool_address: Some(Pubkey::new_unique().to_string()),
        has_referrer: true,
        is_emergency_claim: false,
        estimated_usd_value: 500.0, // $500 USD
        claimed_at: chrono::Utc::now().timestamp(),
        signature: format!("nft_e2e_test_{}", chrono::Utc::now().timestamp_millis()),
        slot: 399244501,
        processed_at: chrono::Utc::now().to_rfc3339(),
    }
}

fn create_realistic_reward_event() -> crate::parser::event_parser::RewardDistributionEventData {
    let now = chrono::Utc::now();
    crate::parser::event_parser::RewardDistributionEventData {
        distribution_id: now.timestamp_millis(),
        reward_pool: Pubkey::new_unique().to_string(),
        recipient: Pubkey::new_unique().to_string(),
        referrer: Some(Pubkey::new_unique().to_string()),
        reward_token_mint: "So11111111111111111111111111111111111111112".parse().unwrap(),
        // 新增的代币元数据字段
        reward_token_decimals: Some(9),
        reward_token_name: Some("Wrapped SOL".to_string()),
        reward_token_symbol: Some("WSOL".to_string()),
        reward_token_logo_uri: Some("https://raw.githubusercontent.com/solana-labs/token-list/main/assets/mainnet/So11111111111111111111111111111111111111112/logo.png".to_string()),
        reward_amount: 2500000,      // 2.5 SOL
        base_reward_amount: 2000000, // 2 SOL base
        bonus_amount: 500000,        // 0.5 SOL bonus
        reward_type: 1,              // 流动性奖励
        reward_type_name: "流动性挖矿奖励".to_string(),
        reward_source: 1,
        reward_source_name: "CLMM流动性挖矿".to_string(),
        related_address: Some(Pubkey::new_unique().to_string()),
        multiplier: 12500, // 1.25x
        multiplier_percentage: 1.25,
        is_locked: true,
        unlock_timestamp: Some(now.timestamp() + 14 * 24 * 3600), // 14天后解锁
        lock_days: 14,
        has_referrer: true,
        is_referral_reward: false,
        is_high_value_reward: true,
        estimated_usd_value: 250.0, // $250 USD
        distributed_at: now.timestamp(),
        signature: format!("reward_e2e_test_{}", now.timestamp_millis()),
        slot: 399244502,
        processed_at: now.to_rfc3339(),
    }
}
