//! 核心功能验证测试
//!
//! 验证E2E流程的每个组件都能正常工作，不依赖外部配置文件

use solana_client::nonblocking::rpc_client::RpcClient;
use tokio::time::{timeout, Duration};

/// 验证Solana网络连接
#[tokio::test]
async fn test_solana_network_connection() {
    let rpc_client = RpcClient::new("https://api.devnet.solana.com".to_string());

    match timeout(Duration::from_secs(10), rpc_client.get_slot()).await {
        Ok(Ok(slot)) => {
            println!("✅ Solana网络连接成功，当前slot: {}", slot);
            assert!(slot > 0, "slot应该大于0");
        }
        Ok(Err(e)) => {
            println!("❌ Solana RPC连接失败: {}", e);
            panic!("无法连接到Solana devnet");
        }
        Err(_) => {
            println!("⏰ Solana连接超时");
            panic!("连接超时");
        }
    }
}

/// 验证解析器注册功能
#[test]
fn test_parser_registry() {
    use crate::config::EventListenerConfig;
    use crate::parser::EventParserRegistry;

    let config = EventListenerConfig {
        solana: crate::config::settings::SolanaConfig {
            rpc_url: "https://api.devnet.solana.com".to_string(),
            ws_url: "wss://api.devnet.solana.com".to_string(),
            commitment: "confirmed".to_string(),
            program_ids: vec![solana_sdk::pubkey::Pubkey::new_unique()],
            private_key: None,
        },
        database: crate::config::settings::DatabaseConfig {
            uri: "mongodb://localhost:27017".to_string(),
            database_name: "test_db".to_string(),
            max_connections: 10,
            min_connections: 2,
        },
        listener: crate::config::settings::ListenerConfig {
            batch_size: 10,
            sync_interval_secs: 5,
            max_retries: 3,
            retry_delay_ms: 500,
            signature_cache_size: 100,
            checkpoint_save_interval_secs: 10,
            backoff: crate::config::settings::BackoffConfig::default(),
            batch_write: crate::config::settings::BatchWriteConfig::default(),
        },
        monitoring: crate::config::settings::MonitoringConfig {
            metrics_interval_secs: 5,
            enable_performance_monitoring: true,
            health_check_interval_secs: 10,
        },
        backfill: None,
    };
    let registry = EventParserRegistry::new(&config).unwrap();
    let parser_count = registry.parser_count();

    println!("✅ 解析器注册表创建成功，包含{}个解析器", parser_count);
    assert_eq!(parser_count, 6, "应该有6个解析器");

    let parsers = registry.get_registered_parsers();
    for (parser_type, discriminator) in parsers {
        println!("   - 解析器: {} -> {:?}", parser_type, discriminator);
    }
}

/// 验证指标收集器功能
#[tokio::test]
async fn test_metrics_collector() {
    use crate::config::EventListenerConfig;
    use crate::metrics::MetricsCollector;

    let config = EventListenerConfig {
        solana: crate::config::settings::SolanaConfig {
            rpc_url: "https://api.devnet.solana.com".to_string(),
            ws_url: "wss://api.devnet.solana.com".to_string(),
            commitment: "confirmed".to_string(),
            program_ids: vec![solana_sdk::pubkey::Pubkey::new_unique()],
            private_key: None,
        },
        database: crate::config::settings::DatabaseConfig {
            uri: "mongodb://localhost:27017".to_string(),
            database_name: "test_db".to_string(),
            max_connections: 10,
            min_connections: 2,
        },
        listener: crate::config::settings::ListenerConfig {
            batch_size: 10,
            sync_interval_secs: 5,
            max_retries: 3,
            retry_delay_ms: 500,
            signature_cache_size: 100,
            checkpoint_save_interval_secs: 10,
            backoff: crate::config::settings::BackoffConfig::default(),
            batch_write: crate::config::settings::BatchWriteConfig::default(),
        },
        monitoring: crate::config::settings::MonitoringConfig {
            metrics_interval_secs: 5,
            enable_performance_monitoring: true,
            health_check_interval_secs: 10,
        },
        backfill: None,
    };
    let collector = MetricsCollector::new(&config).unwrap();

    // 启动收集
    collector.start_collection().await.unwrap();

    // 记录一些指标
    collector.record_event_processed().await.unwrap();
    collector.record_batch_write().await.unwrap();
    collector.record_websocket_connection().await.unwrap();

    // 获取统计
    let stats = collector.get_stats().await.unwrap();
    println!("✅ 指标收集器工作正常:");
    println!("   处理事件: {}", stats.events_processed);
    println!("   批量写入: {}", stats.batch_writes);
    println!("   WebSocket连接: {}", stats.websocket_connections);

    assert_eq!(stats.events_processed, 1);
    assert_eq!(stats.batch_writes, 1);
    assert_eq!(stats.websocket_connections, 1);

    // 生成报告
    let report = collector.generate_performance_report().await.unwrap();
    println!("✅ 性能报告生成成功:");
    println!("   内存使用: {:.2} MB", report.system_resources.memory_usage_mb);
    println!("   CPU使用: {:.2}%", report.system_resources.cpu_usage_percent);

    assert!(report.system_resources.memory_usage_mb >= 0.0);
    assert!(report.system_resources.cpu_usage_percent >= 0.0);

    // 导出Prometheus指标
    let prometheus_output = collector.export_prometheus_metrics().await.unwrap();
    let lines_count = prometheus_output.lines().count();
    println!("✅ Prometheus导出成功，包含{}行指标", lines_count);
    assert!(lines_count > 5, "Prometheus输出应该包含足够的指标");

    // 停止收集
    collector.stop().await.unwrap();
}

/// 综合验证测试
#[tokio::test]
async fn test_comprehensive_validation() {
    println!("🚀 开始综合验证测试");

    // 测试1: Solana网络连接
    let rpc_client = RpcClient::new("https://api.devnet.solana.com".to_string());
    let slot_result = timeout(Duration::from_secs(5), rpc_client.get_slot()).await;
    let network_ok = slot_result.is_ok() && slot_result.unwrap().is_ok();

    // 测试2: 解析器功能
    let parser_ok = test_parser_creation();

    // 测试3: 指标收集
    let metrics_ok = test_metrics_creation().await;

    // 汇总结果
    let mut success_count = 0;
    let total_tests = 3;

    if network_ok {
        success_count += 1;
        println!("✅ 测试1: Solana网络连接正常");
    } else {
        println!("❌ 测试1: Solana网络连接失败");
    }

    if parser_ok {
        success_count += 1;
        println!("✅ 测试2: 解析器功能正常");
    } else {
        println!("❌ 测试2: 解析器功能异常");
    }

    if metrics_ok {
        success_count += 1;
        println!("✅ 测试3: 指标收集正常");
    } else {
        println!("❌ 测试3: 指标收集异常");
    }

    let success_rate = (success_count as f64 / total_tests as f64) * 100.0;
    println!("🎯 综合验证测试完成:");
    println!("   成功测试: {}/{}", success_count, total_tests);
    println!("   成功率: {:.1}%", success_rate);

    if success_count >= 2 {
        println!("🎉 核心功能验证通过！");
    } else {
        println!("⚠️ 部分功能需要检查");
    }

    assert!(success_count >= 2, "至少2个核心测试应该通过");
}

fn test_parser_creation() -> bool {
    use crate::config::EventListenerConfig;
    use crate::parser::EventParserRegistry;

    let config = EventListenerConfig {
        solana: crate::config::settings::SolanaConfig {
            rpc_url: "https://api.devnet.solana.com".to_string(),
            ws_url: "wss://api.devnet.solana.com".to_string(),
            commitment: "confirmed".to_string(),
            program_ids: vec![solana_sdk::pubkey::Pubkey::new_unique()],
            private_key: None,
        },
        database: crate::config::settings::DatabaseConfig {
            uri: "mongodb://localhost:27017".to_string(),
            database_name: "test_db".to_string(),
            max_connections: 10,
            min_connections: 2,
        },
        listener: crate::config::settings::ListenerConfig {
            batch_size: 10,
            sync_interval_secs: 5,
            max_retries: 3,
            retry_delay_ms: 500,
            signature_cache_size: 100,
            checkpoint_save_interval_secs: 10,
            backoff: crate::config::settings::BackoffConfig::default(),
            batch_write: crate::config::settings::BatchWriteConfig::default(),
        },
        monitoring: crate::config::settings::MonitoringConfig {
            metrics_interval_secs: 5,
            enable_performance_monitoring: true,
            health_check_interval_secs: 10,
        },
        backfill: None,
    };
    EventParserRegistry::new(&config).is_ok()
}

async fn test_metrics_creation() -> bool {
    use crate::config::EventListenerConfig;
    use crate::metrics::MetricsCollector;

    let config = EventListenerConfig {
        solana: crate::config::settings::SolanaConfig {
            rpc_url: "https://api.devnet.solana.com".to_string(),
            ws_url: "wss://api.devnet.solana.com".to_string(),
            commitment: "confirmed".to_string(),
            program_ids: vec![solana_sdk::pubkey::Pubkey::new_unique()],
            private_key: None,
        },
        database: crate::config::settings::DatabaseConfig {
            uri: "mongodb://localhost:27017".to_string(),
            database_name: "test_db".to_string(),
            max_connections: 10,
            min_connections: 2,
        },
        listener: crate::config::settings::ListenerConfig {
            batch_size: 10,
            sync_interval_secs: 5,
            max_retries: 3,
            retry_delay_ms: 500,
            signature_cache_size: 100,
            checkpoint_save_interval_secs: 10,
            backoff: crate::config::settings::BackoffConfig::default(),
            batch_write: crate::config::settings::BatchWriteConfig::default(),
        },
        monitoring: crate::config::settings::MonitoringConfig {
            metrics_interval_secs: 5,
            enable_performance_monitoring: true,
            health_check_interval_secs: 10,
        },
        backfill: None,
    };
    match MetricsCollector::new(&config) {
        Ok(collector) => match collector.start_collection().await {
            Ok(_) => {
                collector.stop().await.unwrap();
                true
            }
            Err(_) => false,
        },
        Err(_) => false,
    }
}
