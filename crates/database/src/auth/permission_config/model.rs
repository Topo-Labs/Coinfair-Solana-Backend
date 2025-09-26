use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};

/// 全局Solana权限配置数据库模型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalSolanaPermissionConfigModel {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    /// 配置类型标识（用于区分全局配置）
    pub config_type: String, // 固定为 "global"
    /// 全局读取权限开关
    pub global_read_enabled: bool,
    /// 全局写入权限开关
    pub global_write_enabled: bool,
    /// 默认读取权限策略（JSON字符串）
    pub default_read_policy: String,
    /// 默认写入权限策略（JSON字符串）
    pub default_write_policy: String,
    /// 紧急停用开关
    pub emergency_shutdown: bool,
    /// 维护模式开关
    pub maintenance_mode: bool,
    /// 配置版本
    pub version: u64,
    /// 最后更新时间
    pub last_updated: u64,
    /// 更新者
    pub updated_by: String,
    /// 创建时间
    pub created_at: u64,
}

impl Default for GlobalSolanaPermissionConfigModel {
    fn default() -> Self {
        let now = chrono::Utc::now().timestamp() as u64;
        Self {
            id: None,
            config_type: "global".to_string(),
            global_read_enabled: true,
            global_write_enabled: true,
            default_read_policy: r#"{"RequirePermission":"ReadPool"}"#.to_string(),
            default_write_policy: r#"{"RequirePermission":"CreatePosition"}"#.to_string(),
            emergency_shutdown: false,
            maintenance_mode: false,
            version: 1,
            last_updated: now,
            updated_by: "system".to_string(),
            created_at: now,
        }
    }
}

/// API权限配置数据库模型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolanaApiPermissionConfigModel {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    /// API端点路径
    pub endpoint: String,
    /// API名称/描述
    pub name: String,
    /// API分类
    pub category: String,
    /// 读取权限策略（JSON字符串）
    pub read_policy: String,
    /// 写入权限策略（JSON字符串）
    pub write_policy: String,
    /// 是否启用
    pub enabled: bool,
    /// 创建时间
    pub created_at: u64,
    /// 更新时间
    pub updated_at: u64,
}

impl SolanaApiPermissionConfigModel {
    /// 创建新的API权限配置
    pub fn new(endpoint: String, name: String, category: String, read_policy: String, write_policy: String) -> Self {
        let now = chrono::Utc::now().timestamp() as u64;
        Self {
            id: None,
            endpoint,
            name,
            category,
            read_policy,
            write_policy,
            enabled: true,
            created_at: now,
            updated_at: now,
        }
    }

    /// 更新配置
    pub fn update(&mut self) {
        self.updated_at = chrono::Utc::now().timestamp() as u64;
    }
}

/// 权限配置操作日志模型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionConfigLogModel {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    /// 操作类型 (create, update, delete, global_toggle, emergency_shutdown)
    pub operation_type: String,
    /// 目标类型 (global, api_config)
    pub target_type: String,
    /// 目标标识（对于API配置是endpoint，对于全局配置是"global"）
    pub target_id: String,
    /// 操作前的配置（JSON字符串）
    pub before_config: Option<String>,
    /// 操作后的配置（JSON字符串）
    pub after_config: Option<String>,
    /// 操作者用户ID
    pub operator_id: String,
    /// 操作者钱包地址
    pub operator_wallet: Option<String>,
    /// 操作时间
    pub operation_time: u64,
    /// 操作原因/备注
    pub reason: Option<String>,
    /// 客户端IP
    pub client_ip: Option<String>,
}

impl PermissionConfigLogModel {
    /// 创建新的操作日志
    pub fn new(operation_type: String, target_type: String, target_id: String, operator_id: String) -> Self {
        Self {
            id: None,
            operation_type,
            target_type,
            target_id,
            before_config: None,
            after_config: None,
            operator_id,
            operator_wallet: None,
            operation_time: chrono::Utc::now().timestamp() as u64,
            reason: None,
            client_ip: None,
        }
    }

    /// 设置配置变更信息
    pub fn with_config_change(mut self, before: Option<String>, after: Option<String>) -> Self {
        self.before_config = before;
        self.after_config = after;
        self
    }

    /// 设置操作者信息
    pub fn with_operator_info(mut self, wallet: Option<String>, ip: Option<String>) -> Self {
        self.operator_wallet = wallet;
        self.client_ip = ip;
        self
    }

    /// 设置操作原因
    pub fn with_reason(mut self, reason: String) -> Self {
        self.reason = Some(reason);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_global_config_creation() {
        let config = GlobalSolanaPermissionConfigModel::default();

        assert_eq!(config.config_type, "global");
        assert!(config.global_read_enabled);
        assert!(config.global_write_enabled);
        assert!(!config.emergency_shutdown);
        assert!(!config.maintenance_mode);
        assert_eq!(config.version, 1);
        assert_eq!(config.updated_by, "system");
    }

    #[test]
    fn test_api_config_creation() {
        let config = SolanaApiPermissionConfigModel::new(
            "/api/v1/solana/swap".to_string(),
            "代币交换".to_string(),
            "交换".to_string(),
            r#"{"Allow":null}"#.to_string(),
            r#"{"RequirePermission":"CreatePosition"}"#.to_string(),
        );

        assert_eq!(config.endpoint, "/api/v1/solana/swap");
        assert_eq!(config.name, "代币交换");
        assert_eq!(config.category, "交换");
        assert!(config.enabled);
        assert!(config.created_at > 0);
        assert!(config.updated_at > 0);
    }

    #[test]
    fn test_permission_log_creation() {
        let log = PermissionConfigLogModel::new(
            "update".to_string(),
            "api_config".to_string(),
            "/api/v1/solana/swap".to_string(),
            "admin_user".to_string(),
        )
        .with_config_change(
            Some(r#"{"enabled":true}"#.to_string()),
            Some(r#"{"enabled":false}"#.to_string()),
        )
        .with_operator_info(Some("wallet123".to_string()), Some("127.0.0.1".to_string()))
        .with_reason("权限调整".to_string());

        assert_eq!(log.operation_type, "update");
        assert_eq!(log.target_type, "api_config");
        assert_eq!(log.target_id, "/api/v1/solana/swap");
        assert_eq!(log.operator_id, "admin_user");
        assert!(log.before_config.is_some());
        assert!(log.after_config.is_some());
        assert_eq!(log.operator_wallet, Some("wallet123".to_string()));
        assert_eq!(log.client_ip, Some("127.0.0.1".to_string()));
        assert_eq!(log.reason, Some("权限调整".to_string()));
    }

    #[test]
    fn test_api_config_update() {
        let mut config = SolanaApiPermissionConfigModel::new(
            "/test".to_string(),
            "Test".to_string(),
            "Test".to_string(),
            "{}".to_string(),
            "{}".to_string(),
        );

        let old_updated_at = config.updated_at;

        // 手动设置一个更早的时间戳来确保测试能够通过
        config.updated_at = old_updated_at.saturating_sub(1);
        let very_old_updated_at = config.updated_at;

        config.update();

        // 现在应该有明确的时间差异
        assert!(config.updated_at > very_old_updated_at);
        // 确保更新后的时间戳大于或等于原始时间戳
        assert!(config.updated_at >= old_updated_at);
    }
}
