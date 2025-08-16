//! 验证配置修复的测试

use utils::AppConfig;
use crate::services::solana::shared::config::ConfigurationManager;

#[cfg(test)]
mod config_fix_tests {
    use super::*;

    #[test]
    fn test_app_config_clone() {
        // 创建一个AppConfig实例
        let config1 = AppConfig::new_for_test();
        
        // 测试克隆功能
        let config2 = config1.clone();
        
        // 验证克隆的配置保持了原始值
        assert_eq!(config1.app_host, config2.app_host);
        assert_eq!(config1.app_port, config2.app_port);
        assert_eq!(config1.mongo_uri, config2.mongo_uri);
        assert_eq!(config1.mongo_db, config2.mongo_db);
        assert_eq!(config1.rpc_url, config2.rpc_url);
        assert_eq!(config1.raydium_program_id, config2.raydium_program_id);
        
        println!("✅ AppConfig克隆功能正常");
        println!("  RPC URL: {}", config2.rpc_url);
        println!("  Mongo DB: {}", config2.mongo_db);
        println!("  Raydium Program ID: {}", config2.raydium_program_id);
    }

    #[test]
    fn test_configuration_manager_clone() {
        // 创建ConfigurationManager实例
        let app_config = AppConfig::new_for_test();
        let config_manager1 = ConfigurationManager::new(app_config);
        
        // 测试克隆功能
        let config_manager2 = config_manager1.clone();
        
        // 验证两个配置管理器产生相同的配置
        let config1 = config_manager1.get_config().unwrap();
        let config2 = config_manager2.get_config().unwrap();
        
        assert_eq!(config1.rpc_url, config2.rpc_url);
        assert_eq!(config1.amm_program_id, config2.amm_program_id);
        assert_eq!(config1.usdc_mint, config2.usdc_mint);
        
        println!("✅ ConfigurationManager克隆功能正常");
        println!("  RPC URL: {}", config1.rpc_url);
        println!("  AMM Program ID: {}", config1.amm_program_id);
    }

    #[test]
    fn test_config_preservation() {
        // 模拟原始配置有特定值
        let mut original_config = AppConfig::new_for_test();
        original_config.rpc_url = "https://special-rpc-url.com".to_string();
        original_config.mongo_db = "special_database".to_string();
        
        // 克隆配置
        let cloned_config = original_config.clone();
        
        // 验证特殊值被保留
        assert_eq!(cloned_config.rpc_url, "https://special-rpc-url.com");
        assert_eq!(cloned_config.mongo_db, "special_database");
        
        println!("✅ 配置特殊值正确保留");
        println!("  保留的RPC URL: {}", cloned_config.rpc_url);
        println!("  保留的数据库名: {}", cloned_config.mongo_db);
    }
}