use crate::error::{EventListenerError, Result};
use async_trait::async_trait;
use database::event_model::event_model_repository::EventModelRepository;
use solana_sdk::pubkey::Pubkey;
use std::{collections::HashMap, sync::Arc};
use tracing::info;

/// 事件回填处理器特征
///
/// 定义了每种事件类型回填时需要实现的核心方法
#[async_trait]
pub trait EventBackfillHandler: Send + Sync {
    /// 获取事件类型名称（用于日志和检查点标识）
    fn event_type_name(&self) -> &'static str;

    /// 获取目标集合名称
    fn collection_name(&self) -> &'static str;

    /// 获取最老事件的签名（仅用于初次启动时确定回填范围的起始点）
    async fn get_oldest_event_signature(&self, repo: &EventModelRepository) -> Result<String>;

    /// 检查签名是否已存在于目标集合中
    async fn signature_exists(&self, repo: &EventModelRepository, signature: &str) -> Result<bool>;

    /// 获取检查点事件名称（用于检查点标识，默认为事件类型名称）
    fn checkpoint_event_name(&self) -> String {
        self.event_type_name().to_lowercase()
    }
}

/// LaunchEvent回填处理器
#[derive(Debug, Clone)]
pub struct LaunchEventHandler;

#[async_trait]
impl EventBackfillHandler for LaunchEventHandler {
    fn event_type_name(&self) -> &'static str {
        "LaunchEvent"
    }

    fn collection_name(&self) -> &'static str {
        "LaunchEvent"
    }

    async fn get_oldest_event_signature(&self, repo: &EventModelRepository) -> Result<String> {
        match repo.get_oldest_launch_event().await {
            Ok(Some(launch)) => Ok(launch.signature),
            Ok(None) => {
                info!("⚠️ 没有找到LaunchEvent，使用零签名");
                Ok("1111111111111111111111111111111111111111111111111111111111111111".to_string())
            }
            Err(e) => Err(EventListenerError::Unknown(format!("获取最老LaunchEvent失败: {}", e))),
        }
    }

    async fn signature_exists(&self, repo: &EventModelRepository, signature: &str) -> Result<bool> {
        // 检查LaunchEvent集合中是否存在该签名
        use mongodb::bson::doc;
        let collection = repo
            .get_database()
            .collection::<mongodb::bson::Document>(self.collection_name());
        let filter = doc! { "signature": signature };

        match collection.count_documents(filter, None).await {
            Ok(count) => Ok(count > 0),
            Err(e) => Err(EventListenerError::Unknown(format!(
                "检查LaunchEvent签名存在性失败: {}",
                e
            ))),
        }
    }
}

/// TokenCreationEvent回填处理器
#[derive(Debug, Clone)]
pub struct TokenCreationEventHandler;

#[async_trait]
impl EventBackfillHandler for TokenCreationEventHandler {
    fn event_type_name(&self) -> &'static str {
        "TokenCreationEvent"
    }

    fn collection_name(&self) -> &'static str {
        "TokenCreationEvent"
    }

    async fn get_oldest_event_signature(&self, repo: &EventModelRepository) -> Result<String> {
        match repo.get_oldest_token_creation_event().await {
            Ok(Some(token)) => Ok(token.signature),
            Ok(None) => {
                info!("⚠️ 没有找到TokenCreationEvent，使用零签名");
                Ok("1111111111111111111111111111111111111111111111111111111111111111".to_string())
            }
            Err(e) => Err(EventListenerError::Unknown(format!(
                "获取最老TokenCreationEvent失败: {}",
                e
            ))),
        }
    }

    async fn signature_exists(&self, repo: &EventModelRepository, signature: &str) -> Result<bool> {
        // 检查TokenCreationEvent集合中是否存在该签名
        use mongodb::bson::doc;
        let collection = repo
            .get_database()
            .collection::<mongodb::bson::Document>(self.collection_name());
        let filter = doc! { "signature": signature };

        match collection.count_documents(filter, None).await {
            Ok(count) => Ok(count > 0),
            Err(e) => Err(EventListenerError::Unknown(format!(
                "检查TokenCreationEvent签名存在性失败: {}",
                e
            ))),
        }
    }
}

/// 事件回填处理器注册中心
///
/// 管理所有事件类型的处理器，支持动态注册和查询
pub struct BackfillEventRegistry {
    handlers: HashMap<String, Arc<dyn EventBackfillHandler>>,
}

impl BackfillEventRegistry {
    /// 创建新的注册中心
    pub fn new() -> Self {
        let mut registry = Self {
            handlers: HashMap::new(),
        };

        // 注册默认的事件处理器
        registry.register_default_handlers();
        registry
    }

    /// 注册默认的事件处理器
    fn register_default_handlers(&mut self) {
        self.register_handler("LaunchEvent", Arc::new(LaunchEventHandler));
        self.register_handler("TokenCreationEvent", Arc::new(TokenCreationEventHandler));
    }

    /// 注册事件处理器
    pub fn register_handler(&mut self, event_type: &str, handler: Arc<dyn EventBackfillHandler>) {
        info!("📋 注册事件回填处理器: {}", event_type);
        self.handlers.insert(event_type.to_string(), handler);
    }

    /// 获取事件处理器
    pub fn get_handler(&self, event_type: &str) -> Option<Arc<dyn EventBackfillHandler>> {
        self.handlers.get(event_type).cloned()
    }

    /// 获取所有已注册的事件类型
    pub fn get_registered_event_types(&self) -> Vec<String> {
        self.handlers.keys().cloned().collect()
    }

    /// 检查是否支持某种事件类型
    pub fn supports_event_type(&self, event_type: &str) -> bool {
        self.handlers.contains_key(event_type)
    }

    /// 获取注册的处理器数量
    pub fn handler_count(&self) -> usize {
        self.handlers.len()
    }
}

impl Default for BackfillEventRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// 回填事件配置
///
/// 描述单个事件类型的回填配置
#[derive(Debug, Clone)]
pub struct BackfillEventConfig {
    /// 事件类型名称
    pub event_type: String,
    /// 目标程序ID
    pub program_id: Pubkey,
    /// 是否启用该事件类型的回填
    pub enabled: bool,
    /// 检查间隔（秒）
    pub check_interval_secs: Option<u64>,
}

impl BackfillEventConfig {
    /// 创建新的事件配置
    pub fn new(event_type: &str, program_id: Pubkey) -> Self {
        Self {
            event_type: event_type.to_string(),
            program_id,
            enabled: true,
            check_interval_secs: None,
        }
    }

    /// 设置是否启用
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// 设置检查间隔
    pub fn with_check_interval(mut self, interval_secs: u64) -> Self {
        self.check_interval_secs = Some(interval_secs);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_backfill_event_registry_creation() {
        let registry = BackfillEventRegistry::new();

        // 检查默认处理器是否已注册
        assert!(registry.supports_event_type("LaunchEvent"));
        assert!(registry.supports_event_type("TokenCreationEvent"));
        assert_eq!(registry.handler_count(), 2);

        let event_types = registry.get_registered_event_types();
        assert!(event_types.contains(&"LaunchEvent".to_string()));
        assert!(event_types.contains(&"TokenCreationEvent".to_string()));
    }

    #[test]
    fn test_backfill_event_config_creation() {
        let program_id = Pubkey::from_str("7iEA3rL66H6yCY3PWJNipfys5srz3L6r9QsGPmhnLkA1").unwrap();

        let config = BackfillEventConfig::new("LaunchEvent", program_id)
            .with_enabled(true)
            .with_check_interval(300);

        assert_eq!(config.event_type, "LaunchEvent");
        assert_eq!(config.program_id, program_id);
        assert!(config.enabled);
        assert_eq!(config.check_interval_secs, Some(300));
    }

    #[test]
    fn test_launch_event_handler_properties() {
        let handler = LaunchEventHandler;

        assert_eq!(handler.event_type_name(), "LaunchEvent");
        assert_eq!(handler.collection_name(), "LaunchEvent");
        assert_eq!(handler.checkpoint_event_name(), "launchevent");
    }

    #[test]
    fn test_token_creation_event_handler_properties() {
        let handler = TokenCreationEventHandler;

        assert_eq!(handler.event_type_name(), "TokenCreationEvent");
        assert_eq!(handler.collection_name(), "TokenCreationEvent");
        assert_eq!(handler.checkpoint_event_name(), "tokencreationevent");
    }
}
