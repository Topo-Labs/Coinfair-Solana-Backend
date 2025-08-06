use solana_client::rpc_response::RpcLogsResponse;
use solana_sdk::pubkey::Pubkey;

/// 事件过滤器
/// 
/// 用于过滤和分类接收到的事件，决定哪些事件需要处理
#[derive(Debug, Clone)]
pub struct EventFilter {
    /// 目标程序ID列表
    pub target_programs: Vec<Pubkey>,
    /// 是否过滤错误事件
    pub filter_errors: bool,
    /// 最小日志长度
    pub min_log_length: usize,
    /// 事件类型白名单 (空表示接受所有)
    pub event_type_whitelist: Vec<String>,
    /// 事件类型黑名单
    pub event_type_blacklist: Vec<String>,
}

impl EventFilter {
    /// 创建新的事件过滤器
    pub fn new(target_programs: Vec<Pubkey>) -> Self {
        Self {
            target_programs,
            filter_errors: true,
            min_log_length: 0,
            event_type_whitelist: Vec::new(),
            event_type_blacklist: Vec::new(),
        }
    }

    /// 创建默认过滤器（接受所有事件）
    pub fn accept_all(target_programs: Vec<Pubkey>) -> Self {
        Self {
            target_programs,
            filter_errors: false,
            min_log_length: 0,
            event_type_whitelist: Vec::new(),
            event_type_blacklist: Vec::new(),
        }
    }

    /// 判断是否应该处理该事件
    pub fn should_process(&self, log_response: &RpcLogsResponse) -> bool {
        use tracing::info;
        
        info!("🔍 过滤器检查事件: {}", log_response.signature);
        info!("🔍 日志内容: {:?}", log_response.logs);
        
        // 检查是否有错误且需要过滤错误
        if self.filter_errors && log_response.err.is_some() {
            info!("🚫 事件有错误，被过滤");
            return false;
        }

        // 检查日志长度
        if log_response.logs.len() < self.min_log_length {
            info!("🚫 日志长度不足: {} < {}", log_response.logs.len(), self.min_log_length);
            return false;
        }

        // 检查是否包含目标程序的日志
        let contains_target = self.contains_target_program_logs(&log_response.logs);
        info!("🔍 是否包含目标程序: {}", contains_target);
        if !contains_target {
            return false;
        }

        // 检查事件类型过滤
        if let Some(event_type) = self.extract_event_type(&log_response.logs) {
            info!("🔍 提取的事件类型: {}", event_type);
            
            // 检查黑名单
            if self.event_type_blacklist.contains(&event_type) {
                info!("🚫 事件类型在黑名单中");
                return false;
            }

            // 检查白名单（如果设置了白名单）
            if !self.event_type_whitelist.is_empty() && !self.event_type_whitelist.contains(&event_type) {
                info!("🚫 事件类型不在白名单中");
                return false;
            }
        }

        info!("✅ 事件通过所有过滤器检查");
        true
    }

    /// 检查日志中是否包含目标程序的调用
    fn contains_target_program_logs(&self, logs: &[String]) -> bool {
        for log in logs {
            for program_id in &self.target_programs {
                if log.contains(&program_id.to_string()) {
                    return true;
                }
            }
        }
        false
    }

    /// 从日志中提取事件类型（如果可能）
    fn extract_event_type(&self, logs: &[String]) -> Option<String> {
        for log in logs {
            // 先查找程序数据日志（优先级更高）
            if log.starts_with("Program data: ") {
                // 这里可以根据实际需要实现更复杂的事件类型提取逻辑
                return Some("program_data".to_string());
            }
        }
        
        // 如果没有程序数据日志，再查找指令日志
        for log in logs {
            // 查找指令日志，但排除程序调用日志
            if log.contains("invoke [") && !log.contains("Program ") {
                // 提取指令名称
                if let Some(start) = log.find("invoke [") {
                    if let Some(end) = log[start..].find(']') {
                        let instruction = &log[start + 8..start + end];
                        return Some(instruction.to_string());
                    }
                }
            }
        }
        None
    }

    /// 设置错误过滤
    pub fn with_error_filtering(mut self, filter_errors: bool) -> Self {
        self.filter_errors = filter_errors;
        self
    }

    /// 设置最小日志长度
    pub fn with_min_log_length(mut self, min_length: usize) -> Self {
        self.min_log_length = min_length;
        self
    }

    /// 添加事件类型到白名单
    pub fn with_event_whitelist(mut self, event_types: Vec<String>) -> Self {
        self.event_type_whitelist = event_types;
        self
    }

    /// 添加事件类型到黑名单
    pub fn with_event_blacklist(mut self, event_types: Vec<String>) -> Self {
        self.event_type_blacklist = event_types;
        self
    }

    /// 获取过滤器统计信息
    pub fn get_filter_stats(&self) -> FilterStats {
        FilterStats {
            target_program_count: self.target_programs.len(),
            filter_errors: self.filter_errors,
            min_log_length: self.min_log_length,
            whitelist_count: self.event_type_whitelist.len(),
            blacklist_count: self.event_type_blacklist.len(),
        }
    }
}

/// 过滤器统计信息
#[derive(Debug, Clone, serde::Serialize)]
pub struct FilterStats {
    pub target_program_count: usize,
    pub filter_errors: bool,
    pub min_log_length: usize,
    pub whitelist_count: usize,
    pub blacklist_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn create_test_log_response(
        logs: Vec<String>,
        signature: &str,
        err: Option<String>,
    ) -> RpcLogsResponse {
        RpcLogsResponse {
            signature: signature.to_string(),
            err: err.map(|_| solana_sdk::transaction::TransactionError::AccountNotFound),
            logs,
        }
    }

    #[test]
    fn test_event_filter_creation() {
        let program_id = Pubkey::from_str("FA1RJDDXysgwg5Gm3fJXWxt26JQzPkAzhTA114miqNUX").unwrap();
        let filter = EventFilter::new(vec![program_id]);

        assert_eq!(filter.target_programs.len(), 1);
        assert!(filter.filter_errors);
        assert_eq!(filter.min_log_length, 0);
    }

    #[test]
    fn test_accept_all_filter() {
        let program_id = Pubkey::from_str("FA1RJDDXysgwg5Gm3fJXWxt26JQzPkAzhTA114miqNUX").unwrap();
        let filter = EventFilter::accept_all(vec![program_id]);

        assert!(!filter.filter_errors);
        assert_eq!(filter.min_log_length, 0);
    }

    #[test]
    fn test_should_process_with_target_program() {
        let program_id = Pubkey::from_str("FA1RJDDXysgwg5Gm3fJXWxt26JQzPkAzhTA114miqNUX").unwrap();
        let filter = EventFilter::accept_all(vec![program_id]);

        let logs = vec![
            format!("Program {} invoke [1]", program_id),
            "Program data: test".to_string(),
        ];

        let log_response = create_test_log_response(logs, "test_signature", None);
        assert!(filter.should_process(&log_response));
    }

    #[test]
    fn test_should_not_process_without_target_program() {
        let program_id = Pubkey::from_str("FA1RJDDXysgwg5Gm3fJXWxt26JQzPkAzhTA114miqNUX").unwrap();
        let filter = EventFilter::accept_all(vec![program_id]);

        let logs = vec![
            "Program 11111111111111111111111111111111 invoke [1]".to_string(),
            "Program data: test".to_string(),
        ];

        let log_response = create_test_log_response(logs, "test_signature", None);
        assert!(!filter.should_process(&log_response));
    }

    #[test]
    fn test_error_filtering() {
        let program_id = Pubkey::from_str("FA1RJDDXysgwg5Gm3fJXWxt26JQzPkAzhTA114miqNUX").unwrap();
        let filter = EventFilter::new(vec![program_id]).with_error_filtering(true);

        let logs = vec![
            format!("Program {} invoke [1]", program_id),
        ];

        // Test with error
        let log_response_with_error = create_test_log_response(
            logs.clone(),
            "test_signature",
            Some("InstructionError".to_string()),
        );
        assert!(!filter.should_process(&log_response_with_error));

        // Test without error
        let log_response_without_error = create_test_log_response(logs, "test_signature", None);
        assert!(filter.should_process(&log_response_without_error));
    }

    #[test]
    fn test_min_log_length_filtering() {
        let program_id = Pubkey::from_str("FA1RJDDXysgwg5Gm3fJXWxt26JQzPkAzhTA114miqNUX").unwrap();
        let filter = EventFilter::accept_all(vec![program_id]).with_min_log_length(2);

        // Test with insufficient logs
        let short_logs = vec![
            format!("Program {} invoke [1]", program_id),
        ];
        let log_response_short = create_test_log_response(short_logs, "test_signature", None);
        assert!(!filter.should_process(&log_response_short));

        // Test with sufficient logs
        let long_logs = vec![
            format!("Program {} invoke [1]", program_id),
            "Program data: test".to_string(),
        ];
        let log_response_long = create_test_log_response(long_logs, "test_signature", None);
        assert!(filter.should_process(&log_response_long));
    }

    #[test]
    fn test_extract_event_type() {
        let program_id = Pubkey::from_str("FA1RJDDXysgwg5Gm3fJXWxt26JQzPkAzhTA114miqNUX").unwrap();
        let filter = EventFilter::new(vec![program_id]);

        // Test program data extraction
        let logs_with_data = vec!["Program data: test_data".to_string()];
        assert_eq!(filter.extract_event_type(&logs_with_data), Some("program_data".to_string()));

        // Test instruction extraction
        let logs_with_invoke = vec!["invoke [create_token]".to_string()];
        assert_eq!(filter.extract_event_type(&logs_with_invoke), Some("create_token".to_string()));

        // Test no event type
        let logs_without_events = vec!["Regular log message".to_string()];
        assert_eq!(filter.extract_event_type(&logs_without_events), None);
    }

    #[test]
    fn test_whitelist_filtering() {
        let program_id = Pubkey::from_str("FA1RJDDXysgwg5Gm3fJXWxt26JQzPkAzhTA114miqNUX").unwrap();
        let filter = EventFilter::accept_all(vec![program_id])
            .with_event_whitelist(vec!["program_data".to_string()]);

        let logs_allowed = vec![
            format!("Program {} invoke [1]", program_id),
            "Program data: test".to_string(),
        ];
        let log_response_allowed = create_test_log_response(logs_allowed, "test_signature", None);
        assert!(filter.should_process(&log_response_allowed));

        let logs_not_allowed = vec![
            format!("Program {} invoke [1]", program_id),
            "invoke [other_instruction]".to_string(),
        ];
        let log_response_not_allowed = create_test_log_response(logs_not_allowed, "test_signature", None);
        assert!(!filter.should_process(&log_response_not_allowed));
    }

    #[test]
    fn test_blacklist_filtering() {
        let program_id = Pubkey::from_str("FA1RJDDXysgwg5Gm3fJXWxt26JQzPkAzhTA114miqNUX").unwrap();
        let filter = EventFilter::accept_all(vec![program_id])
            .with_event_blacklist(vec!["forbidden_event".to_string()]);

        let logs_allowed = vec![
            format!("Program {} invoke [1]", program_id),
            "Program data: test".to_string(),
        ];
        let log_response_allowed = create_test_log_response(logs_allowed, "test_signature", None);
        assert!(filter.should_process(&log_response_allowed));

        let logs_blacklisted = vec![
            format!("Program {} invoke [1]", program_id),
            "invoke [forbidden_event]".to_string(),
        ];
        let log_response_blacklisted = create_test_log_response(logs_blacklisted, "test_signature", None);
        assert!(!filter.should_process(&log_response_blacklisted));
    }

    #[test]
    fn test_filter_stats() {
        let program_id = Pubkey::from_str("FA1RJDDXysgwg5Gm3fJXWxt26JQzPkAzhTA114miqNUX").unwrap();
        let filter = EventFilter::new(vec![program_id])
            .with_min_log_length(5)
            .with_event_whitelist(vec!["event1".to_string(), "event2".to_string()])
            .with_event_blacklist(vec!["bad_event".to_string()]);

        let stats = filter.get_filter_stats();
        assert_eq!(stats.target_program_count, 1);
        assert!(stats.filter_errors);
        assert_eq!(stats.min_log_length, 5);
        assert_eq!(stats.whitelist_count, 2);
        assert_eq!(stats.blacklist_count, 1);
    }
}