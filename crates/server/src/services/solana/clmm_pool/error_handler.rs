//! CLMM池子服务异常处理模块
//!
//! 提供统一的错误处理、重试机制和数据一致性保障

use database::clmm_pool::{ClmmPool, PoolStatus};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{error, info, warn};
use utils::AppResult;

/// 错误类型分类
#[derive(Debug, Clone, PartialEq)]
pub enum ErrorCategory {
    /// 网络错误 (可重试)
    Network,
    /// RPC错误 (可重试)
    Rpc,
    /// 数据库错误 (可重试)
    Database,
    /// 验证错误 (不可重试)
    Validation,
    /// 业务逻辑错误 (不可重试)
    Business,
    /// 系统错误 (不可重试)
    System,
}

/// 重试策略配置
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// 最大重试次数
    pub max_retries: u32,
    /// 基础重试间隔 (毫秒)
    pub base_delay_ms: u64,
    /// 指数退避倍数
    pub backoff_multiplier: f64,
    /// 最大重试间隔 (毫秒)
    pub max_delay_ms: u64,
    /// 抖动因子 (0.0-1.0)
    pub jitter_factor: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay_ms: 1000,
            backoff_multiplier: 2.0,
            max_delay_ms: 30000,
            jitter_factor: 0.1,
        }
    }
}

/// 异常处理器
pub struct ErrorHandler {
    retry_config: RetryConfig,
}

impl ErrorHandler {
    /// 创建新的异常处理器
    pub fn new(retry_config: Option<RetryConfig>) -> Self {
        Self {
            retry_config: retry_config.unwrap_or_default(),
        }
    }

    /// 分类错误类型
    pub fn categorize_error(&self, error: &anyhow::Error) -> ErrorCategory {
        let error_msg = error.to_string().to_lowercase();

        // 网络相关错误
        if error_msg.contains("connection")
            || error_msg.contains("timeout")
            || error_msg.contains("network")
            || error_msg.contains("dns")
        {
            return ErrorCategory::Network;
        }

        // RPC相关错误
        if error_msg.contains("rpc")
            || error_msg.contains("solana")
            || error_msg.contains("account not found")
            || error_msg.contains("insufficient funds")
        {
            return ErrorCategory::Rpc;
        }

        // 数据库相关错误
        if error_msg.contains("mongodb")
            || error_msg.contains("database")
            || error_msg.contains("collection")
            || error_msg.contains("bson")
        {
            return ErrorCategory::Database;
        }

        // 验证相关错误
        if error_msg.contains("invalid")
            || error_msg.contains("validation")
            || error_msg.contains("parse")
            || error_msg.contains("format")
        {
            return ErrorCategory::Validation;
        }

        // 业务逻辑错误
        if error_msg.contains("pool already exists")
            || error_msg.contains("insufficient liquidity")
            || error_msg.contains("price out of range")
        {
            return ErrorCategory::Business;
        }

        // 默认为系统错误
        ErrorCategory::System
    }

    /// 判断错误是否可重试
    pub fn is_retryable(&self, error: &anyhow::Error) -> bool {
        match self.categorize_error(error) {
            ErrorCategory::Network | ErrorCategory::Rpc | ErrorCategory::Database => true,
            _ => false,
        }
    }

    /// 计算重试延迟时间
    pub fn calculate_delay(&self, attempt: u32) -> Duration {
        if attempt == 0 {
            return Duration::from_millis(0);
        }

        let base_delay = self.retry_config.base_delay_ms as f64;
        let multiplier = self.retry_config.backoff_multiplier;
        let max_delay = self.retry_config.max_delay_ms as f64;

        // 指数退避
        let delay = base_delay * multiplier.powi((attempt - 1) as i32);
        let delay = delay.min(max_delay);

        // 添加简单的抖动 (使用系统时间作为随机源)
        let jitter_ms = (chrono::Utc::now().timestamp_millis() % 100) as f64;
        let jitter = delay * self.retry_config.jitter_factor * (jitter_ms / 100.0 - 0.5);
        let final_delay = (delay + jitter).max(0.0) as u64;

        Duration::from_millis(final_delay)
    }

    /// 执行带重试的操作
    pub async fn execute_with_retry<F, Fut, T>(&self, operation_name: &str, mut operation: F) -> AppResult<T>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = AppResult<T>>,
    {
        let mut last_error: Option<utils::AppError> = None;

        for attempt in 0..=self.retry_config.max_retries {
            match operation().await {
                Ok(result) => {
                    if attempt > 0 {
                        info!("✅ 操作重试成功: {} (尝试次数: {})", operation_name, attempt + 1);
                    }
                    return Ok(result);
                }
                Err(error) => {
                    let error_msg = error.to_string();
                    let is_retryable = self.is_retryable(&anyhow::anyhow!(error_msg.clone()));

                    if attempt < self.retry_config.max_retries && is_retryable {
                        let delay = self.calculate_delay(attempt + 1);
                        warn!(
                            "⚠️ 操作失败，将重试: {} (尝试 {}/{}) - {} (延迟: {:?})",
                            operation_name,
                            attempt + 1,
                            self.retry_config.max_retries,
                            error_msg,
                            delay
                        );

                        last_error = Some(error);
                        sleep(delay).await;
                    } else {
                        error!("❌ 操作最终失败: {} - {}", operation_name, error_msg);
                        return Err(error);
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("未知错误").into()))
    }
}

/// 数据一致性检查器
pub struct ConsistencyChecker;

impl ConsistencyChecker {
    /// 检查池子数据一致性
    pub async fn check_pool_consistency(&self, pool: &ClmmPool) -> Vec<ConsistencyIssue> {
        let mut issues = Vec::new();

        // 1. 检查基本字段完整性
        if pool.pool_address.is_empty() {
            issues.push(ConsistencyIssue {
                issue_type: ConsistencyIssueType::MissingField,
                field_name: "pool_address".to_string(),
                description: "池子地址为空".to_string(),
                severity: IssueSeverity::Critical,
            });
        }

        if pool.mint0.mint_address.is_empty() {
            issues.push(ConsistencyIssue {
                issue_type: ConsistencyIssueType::MissingField,
                field_name: "mint0.mint_address".to_string(),
                description: "Mint0地址为空".to_string(),
                severity: IssueSeverity::Critical,
            });
        }

        if pool.mint1.mint_address.is_empty() {
            issues.push(ConsistencyIssue {
                issue_type: ConsistencyIssueType::MissingField,
                field_name: "mint1.mint_address".to_string(),
                description: "Mint1地址为空".to_string(),
                severity: IssueSeverity::Critical,
            });
        }

        // 2. 检查mint顺序
        if pool.mint0.mint_address >= pool.mint1.mint_address {
            issues.push(ConsistencyIssue {
                issue_type: ConsistencyIssueType::InvalidOrder,
                field_name: "mint_order".to_string(),
                description: "Mint地址顺序不正确，mint0应该小于mint1".to_string(),
                severity: IssueSeverity::High,
            });
        }

        // 3. 检查价格合理性
        if pool.price_info.initial_price <= 0.0 {
            issues.push(ConsistencyIssue {
                issue_type: ConsistencyIssueType::InvalidValue,
                field_name: "price_info.initial_price".to_string(),
                description: "初始价格必须大于0".to_string(),
                severity: IssueSeverity::High,
            });
        }

        // 4. 检查时间戳合理性
        let now = chrono::Utc::now().timestamp() as u64;
        if pool.api_created_at > now + 3600 {
            // 允许1小时的时间偏差
            issues.push(ConsistencyIssue {
                issue_type: ConsistencyIssueType::InvalidValue,
                field_name: "created_at".to_string(),
                description: "创建时间不能是未来时间".to_string(),
                severity: IssueSeverity::Medium,
            });
        }

        if pool.updated_at < pool.api_created_at {
            issues.push(ConsistencyIssue {
                issue_type: ConsistencyIssueType::InvalidValue,
                field_name: "updated_at".to_string(),
                description: "更新时间不能早于创建时间".to_string(),
                severity: IssueSeverity::Medium,
            });
        }

        // 5. 检查同步状态一致性
        if pool.sync_status.needs_sync && pool.sync_status.sync_error.is_some() {
            issues.push(ConsistencyIssue {
                issue_type: ConsistencyIssueType::InconsistentState,
                field_name: "sync_status".to_string(),
                description: "同步状态不一致：需要同步但有同步错误".to_string(),
                severity: IssueSeverity::Low,
            });
        }

        // 6. 检查交易信息一致性
        if let Some(tx_info) = &pool.transaction_info {
            if tx_info.signature.is_empty() {
                issues.push(ConsistencyIssue {
                    issue_type: ConsistencyIssueType::MissingField,
                    field_name: "transaction_info.signature".to_string(),
                    description: "交易签名为空".to_string(),
                    severity: IssueSeverity::High,
                });
            }

            // 如果有交易信息，状态应该是Active或Pending
            match pool.status {
                PoolStatus::Created => {
                    issues.push(ConsistencyIssue {
                        issue_type: ConsistencyIssueType::InconsistentState,
                        field_name: "status".to_string(),
                        description: "有交易信息但状态仍为Created".to_string(),
                        severity: IssueSeverity::Medium,
                    });
                }
                _ => {}
            }
        }

        issues
    }

    /// 自动修复可修复的一致性问题
    pub async fn auto_fix_issues(&self, pool: &mut ClmmPool, issues: &[ConsistencyIssue]) -> Vec<String> {
        let mut fixed_issues = Vec::new();

        for issue in issues {
            match &issue.issue_type {
                ConsistencyIssueType::InvalidOrder if issue.field_name == "mint_order" => {
                    // 自动修复mint顺序
                    if pool.mint0.mint_address > pool.mint1.mint_address {
                        std::mem::swap(&mut pool.mint0, &mut pool.mint1);

                        // 调整价格
                        if pool.price_info.initial_price != 0.0 {
                            pool.price_info.initial_price = 1.0 / pool.price_info.initial_price;
                        }
                        if let Some(current_price) = pool.price_info.current_price {
                            pool.price_info.current_price = Some(1.0 / current_price);
                        }

                        fixed_issues.push("修复了mint地址顺序和相应的价格".to_string());
                    }
                }
                ConsistencyIssueType::InvalidValue if issue.field_name == "updated_at" => {
                    // 自动修复更新时间
                    if pool.updated_at < pool.api_created_at {
                        pool.updated_at = pool.api_created_at;
                        fixed_issues.push("修复了更新时间".to_string());
                    }
                }
                _ => {
                    // 其他问题暂时不自动修复
                }
            }
        }

        fixed_issues
    }
}

/// 一致性问题
#[derive(Debug, Clone)]
pub struct ConsistencyIssue {
    pub issue_type: ConsistencyIssueType,
    pub field_name: String,
    pub description: String,
    pub severity: IssueSeverity,
}

/// 一致性问题类型
#[derive(Debug, Clone, PartialEq)]
pub enum ConsistencyIssueType {
    /// 缺失字段
    MissingField,
    /// 无效值
    InvalidValue,
    /// 顺序错误
    InvalidOrder,
    /// 状态不一致
    InconsistentState,
}

/// 问题严重程度
#[derive(Debug, Clone, PartialEq)]
pub enum IssueSeverity {
    /// 严重 - 影响核心功能
    Critical,
    /// 高 - 影响重要功能
    High,
    /// 中 - 影响次要功能
    Medium,
    /// 低 - 不影响功能但需要注意
    Low,
}

/// 事务管理器 - 确保数据操作的原子性
pub struct TransactionManager;

impl TransactionManager {
    /// 执行带事务的池子创建操作
    pub async fn create_pool_with_transaction<F>(&self, operation_name: &str, operation: F) -> AppResult<String>
    where
        F: std::future::Future<Output = AppResult<String>>,
    {
        info!("🔄 开始事务操作: {}", operation_name);

        // 这里可以扩展为真正的数据库事务
        // 目前先实现基本的错误处理和日志记录
        match operation.await {
            Ok(result) => {
                info!("✅ 事务操作成功: {} - 结果: {}", operation_name, result);
                Ok(result)
            }
            Err(error) => {
                error!("❌ 事务操作失败: {} - 错误: {}", operation_name, error);

                // 这里可以添加回滚逻辑
                self.rollback_operation(operation_name).await?;

                Err(error)
            }
        }
    }

    /// 回滚操作 (占位符实现)
    async fn rollback_operation(&self, operation_name: &str) -> AppResult<()> {
        warn!("🔄 执行回滚操作: {}", operation_name);

        // TODO: 实现具体的回滚逻辑
        // 例如：删除部分创建的记录、恢复状态等

        Ok(())
    }
}

/// 健康检查器
pub struct HealthChecker;

impl HealthChecker {
    /// 检查系统健康状态
    pub async fn check_system_health(&self) -> HealthStatus {
        let issues = Vec::new();

        // 检查数据库连接
        // TODO: 实现实际的数据库连接检查

        // 检查RPC连接
        // TODO: 实现实际的RPC连接检查

        // 检查同步状态
        // TODO: 实现同步状态检查

        if issues.is_empty() {
            HealthStatus::Healthy
        } else {
            HealthStatus::Unhealthy { issues }
        }
    }
}

/// 系统健康状态
#[derive(Debug)]
pub enum HealthStatus {
    /// 健康
    Healthy,
    /// 不健康
    Unhealthy { issues: Vec<String> },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_categorization() {
        let handler = ErrorHandler::new(None);

        let network_error = anyhow::anyhow!("Connection timeout");
        assert_eq!(handler.categorize_error(&network_error), ErrorCategory::Network);

        let rpc_error = anyhow::anyhow!("RPC call failed");
        assert_eq!(handler.categorize_error(&rpc_error), ErrorCategory::Rpc);

        let validation_error = anyhow::anyhow!("Invalid address format");
        assert_eq!(handler.categorize_error(&validation_error), ErrorCategory::Validation);
    }

    #[test]
    fn test_retry_delay_calculation() {
        let handler = ErrorHandler::new(None);

        let delay1 = handler.calculate_delay(1);
        let delay2 = handler.calculate_delay(2);
        let delay3 = handler.calculate_delay(3);

        // 验证指数退避
        assert!(delay2 > delay1);
        assert!(delay3 > delay2);
    }
}
