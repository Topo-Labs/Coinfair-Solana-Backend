use std::collections::HashMap;

/// Discriminator管理器
///
/// 负责管理和识别不同事件类型的discriminator
/// 在Anchor框架中，每个事件都有一个8字节的discriminator用于识别事件类型
pub struct DiscriminatorManager {
    /// discriminator到事件类型名称的映射
    discriminator_map: HashMap<[u8; 8], String>,
}

impl DiscriminatorManager {
    /// 创建新的discriminator管理器
    pub fn new() -> Self {
        let mut manager = Self {
            discriminator_map: HashMap::new(),
        };

        // 注册已知的事件discriminator
        manager.register_known_discriminators();
        manager
    }

    /// 注册已知的事件discriminator
    fn register_known_discriminators(&mut self) {
        // 代币创建事件的discriminator（需要根据实际合约确定）
        // 这里使用示例值，实际应该从合约代码或测试中获取
        // self.register_discriminator([142, 175, 175, 21, 74, 229, 126, 116], "token_creation");
        self.register_discriminator([64, 198, 205, 232, 38, 8, 113, 226], "token_creation");
        //暂时改成swap

        // 未来可以添加更多事件类型
        // self.register_discriminator([89, 202, 187, 172, 108, 193, 190, 8], "pool_creation");
        // self.register_discriminator([123, 45, 67, 89, 10, 11, 12, 13], "nft_claim");
        // self.register_discriminator([98, 76, 54, 32, 10, 98, 76, 54], "reward_distribution");
    }

    /// 注册discriminator
    pub fn register_discriminator(&mut self, discriminator: [u8; 8], event_type: &str) {
        self.discriminator_map.insert(discriminator, event_type.to_string());
        tracing::debug!("📝 注册discriminator: {} -> {:?}", event_type, discriminator);
    }

    /// 根据discriminator获取事件类型
    pub fn get_event_type(&self, discriminator: &[u8; 8]) -> Option<&String> {
        self.discriminator_map.get(discriminator)
    }

    /// 检查discriminator是否已知
    pub fn is_known_discriminator(&self, discriminator: &[u8; 8]) -> bool {
        self.discriminator_map.contains_key(discriminator)
    }

    /// 从数据中提取discriminator
    pub fn extract_discriminator(data: &[u8]) -> Option<[u8; 8]> {
        if data.len() < 8 {
            return None;
        }

        let mut discriminator = [0u8; 8];
        discriminator.copy_from_slice(&data[0..8]);
        Some(discriminator)
    }

    /// 获取所有已注册的discriminator
    pub fn get_all_discriminators(&self) -> Vec<([u8; 8], String)> {
        self.discriminator_map.iter().map(|(k, v)| (*k, v.clone())).collect()
    }

    /// 获取已注册的discriminator数量
    pub fn count(&self) -> usize {
        self.discriminator_map.len()
    }

    /// 生成discriminator（用于测试或开发）
    /// 注意：实际的discriminator应该从智能合约编译后的IDL获取
    pub fn generate_test_discriminator(event_name: &str) -> [u8; 8] {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        event_name.hash(&mut hasher);
        let hash = hasher.finish();

        // 将64位hash转换为8字节数组
        hash.to_le_bytes()
    }

    /// 验证discriminator格式
    pub fn validate_discriminator(discriminator: &[u8; 8]) -> bool {
        // 检查discriminator是否全为0（通常表示无效）
        !discriminator.iter().all(|&b| b == 0)
    }

    /// 将discriminator转换为十六进制字符串（用于调试）
    pub fn discriminator_to_hex(discriminator: &[u8; 8]) -> String {
        discriminator.iter().map(|b| format!("{:02x}", b)).collect::<Vec<_>>().join("")
    }

    /// 从十六进制字符串解析discriminator
    pub fn discriminator_from_hex(hex_str: &str) -> Result<[u8; 8], String> {
        if hex_str.len() != 16 {
            return Err("十六进制字符串长度必须是16个字符".to_string());
        }

        let mut discriminator = [0u8; 8];
        for (i, chunk) in hex_str.as_bytes().chunks(2).enumerate() {
            let hex_byte = std::str::from_utf8(chunk).map_err(|_| "无效的UTF-8字符".to_string())?;
            discriminator[i] = u8::from_str_radix(hex_byte, 16).map_err(|_| format!("无效的十六进制字节: {}", hex_byte))?;
        }

        Ok(discriminator)
    }
}

impl Default for DiscriminatorManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discriminator_manager_creation() {
        let manager = DiscriminatorManager::new();
        assert!(manager.count() > 0);
    }

    #[test]
    fn test_discriminator_registration() {
        let mut manager = DiscriminatorManager::new();
        let test_discriminator = [1, 2, 3, 4, 5, 6, 7, 8];
        let event_type = "test_event";

        assert!(!manager.is_known_discriminator(&test_discriminator));

        manager.register_discriminator(test_discriminator, event_type);

        assert!(manager.is_known_discriminator(&test_discriminator));
        assert_eq!(manager.get_event_type(&test_discriminator), Some(&event_type.to_string()));
    }

    #[test]
    fn test_extract_discriminator() {
        let data = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        let discriminator = DiscriminatorManager::extract_discriminator(&data);
        assert_eq!(discriminator, Some([1, 2, 3, 4, 5, 6, 7, 8]));

        // 测试数据太短的情况
        let short_data = [1, 2, 3];
        let short_discriminator = DiscriminatorManager::extract_discriminator(&short_data);
        assert_eq!(short_discriminator, None);
    }

    #[test]
    fn test_generate_test_discriminator() {
        let discriminator1 = DiscriminatorManager::generate_test_discriminator("event1");
        let discriminator2 = DiscriminatorManager::generate_test_discriminator("event2");
        let discriminator1_again = DiscriminatorManager::generate_test_discriminator("event1");

        // 不同的事件名应该产生不同的discriminator
        assert_ne!(discriminator1, discriminator2);

        // 相同的事件名应该产生相同的discriminator
        assert_eq!(discriminator1, discriminator1_again);
    }

    #[test]
    fn test_validate_discriminator() {
        let valid_discriminator = [1, 2, 3, 4, 5, 6, 7, 8];
        let invalid_discriminator = [0, 0, 0, 0, 0, 0, 0, 0];

        assert!(DiscriminatorManager::validate_discriminator(&valid_discriminator));
        assert!(!DiscriminatorManager::validate_discriminator(&invalid_discriminator));
    }

    #[test]
    fn test_discriminator_hex_conversion() {
        let discriminator = [0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef];
        let hex_str = DiscriminatorManager::discriminator_to_hex(&discriminator);
        assert_eq!(hex_str, "0123456789abcdef");

        let parsed_discriminator = DiscriminatorManager::discriminator_from_hex(&hex_str).unwrap();
        assert_eq!(parsed_discriminator, discriminator);
    }

    #[test]
    fn test_discriminator_from_hex_invalid() {
        // 测试长度错误
        assert!(DiscriminatorManager::discriminator_from_hex("123").is_err());

        // 测试无效字符
        assert!(DiscriminatorManager::discriminator_from_hex("0123456789abcdeg").is_err());
    }

    #[test]
    fn test_get_all_discriminators() {
        let manager = DiscriminatorManager::new();
        let all_discriminators = manager.get_all_discriminators();
        assert!(!all_discriminators.is_empty());

        // 验证返回的数据格式
        for (discriminator, event_type) in all_discriminators {
            assert!(DiscriminatorManager::validate_discriminator(&discriminator));
            assert!(!event_type.is_empty());
        }
    }

    #[test]
    fn test_known_discriminator_registration() {
        let manager = DiscriminatorManager::new();

        // 验证预定义的discriminator已注册
        // let token_creation_discriminator = [142, 175, 175, 21, 74, 229, 126, 116];
        let token_creation_discriminator = [64, 198, 205, 232, 38, 8, 113, 226];
        assert!(manager.is_known_discriminator(&token_creation_discriminator));
        assert_eq!(manager.get_event_type(&token_creation_discriminator), Some(&"token_creation".to_string()));
    }
}
