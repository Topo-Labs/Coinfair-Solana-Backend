use crate::{
    config::EventListenerConfig,
    error::Result,
    metrics::MetricsCollector,
    parser::EventParserRegistry,
    recovery::{
        backfill_handler::{BackfillEventConfig, BackfillEventRegistry},
        backfill_task_context::BackfillTaskContext,
        checkpoint_persistence::CheckpointPersistence,
        scan_record_persistence::ScanRecordPersistence,
    },
    BatchWriter,
};
use solana_client::rpc_client::RpcClient;
use solana_sdk::commitment_config::CommitmentConfig;
use std::{sync::Arc, time::Duration};
use tracing::{error, info, warn};

/// 通用回填服务管理器
///
/// 支持多种事件类型的回填，使用事件处理器策略模式
/// 负责从历史数据中回填丢失的事件，并将其适配为RpcLogsResponse格式
/// 复用现有的解析、处理、持久化流程
#[allow(dead_code)]
pub struct BackfillManager {
    config: Arc<EventListenerConfig>,
    rpc_client: Arc<RpcClient>,
    parser_registry: Arc<EventParserRegistry>,
    batch_writer: Arc<BatchWriter>,
    metrics: Arc<MetricsCollector>,
    checkpoint_persistence: Arc<CheckpointPersistence>,
    scan_record_persistence: Arc<ScanRecordPersistence>,
    /// 事件处理器注册中心
    event_registry: Arc<BackfillEventRegistry>,
    /// 事件配置列表
    event_configs: Vec<BackfillEventConfig>,
    /// 默认检查间隔
    default_check_interval: Duration,
}

impl BackfillManager {
    /// 创建新的通用回填管理器
    ///
    /// 注意：索引初始化由Database::init_permission_indexes()处理，无需在此重复创建
    pub fn new(
        config: &EventListenerConfig,
        parser_registry: Arc<EventParserRegistry>,
        batch_writer: Arc<BatchWriter>,
        metrics: Arc<MetricsCollector>,
        checkpoint_persistence: Arc<CheckpointPersistence>,
        scan_record_persistence: Arc<ScanRecordPersistence>,
        event_configs: Vec<BackfillEventConfig>,
        default_check_interval_secs: u64,
    ) -> Self {
        let config = Arc::new(config.clone());
        let rpc_client = Arc::new(RpcClient::new_with_commitment(
            &config.solana.rpc_url,
            CommitmentConfig::confirmed(),
        ));

        let event_registry = Arc::new(BackfillEventRegistry::new());
        info!("🔧 回填管理器配置了 {} 种事件类型", event_configs.len());

        for event_config in &event_configs {
            info!(
                "📋 配置事件类型: {} (程序ID: {}, 启用: {})",
                event_config.event_type, event_config.program_id, event_config.enabled
            );
        }

        let manager = Self {
            config,
            rpc_client,
            parser_registry,
            batch_writer,
            metrics,
            checkpoint_persistence,
            scan_record_persistence,
            event_registry,
            event_configs,
            default_check_interval: Duration::from_secs(default_check_interval_secs),
        };

        info!("✅ 回填管理器初始化完成，ParserKey配置已在注册表构造时设置");

        manager
    }

    /// 启动多事件回填服务
    pub async fn start(&self) -> Result<()> {
        info!("🔄 启动通用回填服务，支持 {} 种事件类型", self.event_configs.len());

        // 启动每种事件类型的回填任务
        let mut handles = Vec::new();

        for event_config in &self.event_configs {
            if !event_config.enabled {
                info!("⏸️ 跳过已禁用的事件类型: {}", event_config.event_type);
                continue;
            }

            let config = event_config.clone();
            let task_context = self.create_task_context();

            let handle = tokio::spawn(async move { task_context.start_event_backfill_loop(config).await });

            handles.push(handle);
        }

        if handles.is_empty() {
            warn!("⚠️ 没有启用的事件类型，回填服务将退出");
            return Ok(());
        }

        info!("🚀 已启动 {} 个事件回填任务", handles.len());

        // 等待所有任务完成（实际上应该永远运行）
        for handle in handles {
            if let Err(e) = handle.await {
                error!("❌ 回填任务异常终止: {}", e);
            }
        }

        Ok(())
    }

    /// 创建任务上下文
    fn create_task_context(&self) -> BackfillTaskContext {
        BackfillTaskContext {
            config: Arc::clone(&self.config),
            rpc_client: Arc::clone(&self.rpc_client),
            parser_registry: Arc::clone(&self.parser_registry),
            batch_writer: Arc::clone(&self.batch_writer),
            metrics: Arc::clone(&self.metrics),
            checkpoint_persistence: Arc::clone(&self.checkpoint_persistence),
            scan_record_persistence: Arc::clone(&self.scan_record_persistence),
            event_registry: Arc::clone(&self.event_registry),
            default_check_interval: self.default_check_interval,
        }
    }

    /// 获取事件配置（用于测试和调试）
    pub fn get_event_configs(&self) -> &[BackfillEventConfig] {
        &self.event_configs
    }

    /// 获取已启用的事件配置
    pub fn get_enabled_event_configs(&self) -> Vec<&BackfillEventConfig> {
        self.event_configs.iter().filter(|config| config.enabled).collect()
    }

    /// 检查是否支持某种事件类型
    pub fn supports_event_type(&self, event_type: &str) -> bool {
        self.event_registry.supports_event_type(event_type)
    }

    /// 获取支持的事件类型列表
    pub fn get_supported_event_types(&self) -> Vec<String> {
        self.event_registry.get_registered_event_types()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_sdk::{pubkey::Pubkey, signature::Signature};
    use std::str::FromStr;

    #[test]
    fn test_signature_parsing() {
        let test_sig = "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d1V4KAEJrGrMn3RcYfP6oK3pVt4K7yWxNvPvx9eT5NqC";
        let signature = Signature::from_str(test_sig);
        assert!(signature.is_ok());
    }

    #[test]
    fn test_pubkey_parsing() {
        let test_pubkey = "FA1RJDDXysgwg5Gm3fJXWxt26JQzPkAzhTA114miqNUX";
        let pubkey = Pubkey::from_str(test_pubkey);
        assert!(pubkey.is_ok());
    }

    #[test]
    fn test_backfill_manager_supports_multiple_events() {
        // Mock配置（仅用于测试结构）
        let program_id_1 = Pubkey::from_str("AZxHQhxgjENmx8x9CQ8r86Eodo8Qg6H9wYiuRqbonaoH").unwrap();
        let program_id_2 = Pubkey::from_str("FA1RJDDXysgwg5Gm3fJXWxt26JQzPkAzhTA114miqNUX").unwrap();

        let event_configs = vec![
            BackfillEventConfig::new("LaunchEvent", program_id_1).with_check_interval(300),
            BackfillEventConfig::new("TokenCreationEvent", program_id_2).with_check_interval(600),
        ];

        // 验证配置创建
        assert_eq!(event_configs.len(), 2);
        assert_eq!(event_configs[0].event_type, "LaunchEvent");
        assert_eq!(event_configs[1].event_type, "TokenCreationEvent");
        assert!(event_configs[0].enabled);
        assert!(event_configs[1].enabled);
    }

    #[test]
    fn test_event_registry_functionality() {
        let registry = BackfillEventRegistry::new();

        // 测试默认注册的处理器
        assert!(registry.supports_event_type("LaunchEvent"));
        assert!(registry.supports_event_type("TokenCreationEvent"));
        assert!(registry.supports_event_type("DepositEvent"));
        assert!(registry.supports_event_type("ClaimNFTEvent"));
        assert!(registry.supports_event_type("PoolCreatedEvent"));
        assert!(registry.supports_event_type("ReferralRewardEvent"));
        assert!(registry.supports_event_type("InitPoolEvent"));
        assert!(registry.supports_event_type("LpChangeEvent"));
        assert!(!registry.supports_event_type("UnsupportedEvent"));

        let event_types = registry.get_registered_event_types();
        assert_eq!(event_types.len(), 8);
        assert!(event_types.contains(&"LaunchEvent".to_string()));
        assert!(event_types.contains(&"TokenCreationEvent".to_string()));
        assert!(event_types.contains(&"DepositEvent".to_string()));
        assert!(event_types.contains(&"ClaimNFTEvent".to_string()));
        assert!(event_types.contains(&"PoolCreatedEvent".to_string()));
        assert!(event_types.contains(&"ReferralRewardEvent".to_string()));
        assert!(event_types.contains(&"InitPoolEvent".to_string()));
        assert!(event_types.contains(&"LpChangeEvent".to_string()));
    }
}
