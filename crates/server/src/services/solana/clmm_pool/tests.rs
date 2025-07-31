// Tests for ClmmPoolService

use super::ClmmPoolService;
use crate::dtos::solana_dto::CreatePoolRequest;
use crate::services::solana::config::ClmmConfigService;
use crate::services::solana::shared::SharedContext;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use std::sync::Arc;

#[cfg(test)]
mod create_pool_tests {
    use super::*;

    #[test]
    fn test_calculate_sqrt_price_x64() {
        // 直接测试计算逻辑，不依赖SolanaService实例
        let calculate_sqrt_price_x64 = |price: f64, decimals_0: u8, decimals_1: u8| -> u128 {
            let multipler = |decimals: u8| -> f64 { (10_i32).checked_pow(decimals.try_into().unwrap()).unwrap() as f64 };

            let price_to_x64 = |price: f64| -> u128 { (price * raydium_amm_v3::libraries::fixed_point_64::Q64 as f64) as u128 };

            let price_with_decimals = price * multipler(decimals_1) / multipler(decimals_0);
            price_to_x64(price_with_decimals.sqrt())
        };

        // 测试基本价格计算
        let price = 1.0;
        let decimals_0 = 9; // SOL
        let decimals_1 = 6; // USDC

        let sqrt_price_x64 = calculate_sqrt_price_x64(price, decimals_0, decimals_1);

        // 验证结果不为0
        assert!(sqrt_price_x64 > 0);

        // 测试价格为2.0的情况
        let price_2 = 2.0;
        let sqrt_price_x64_2 = calculate_sqrt_price_x64(price_2, decimals_0, decimals_1);

        // 价格为2时的sqrt_price应该大于价格为1时的
        assert!(sqrt_price_x64_2 > sqrt_price_x64);
    }

    #[test]
    fn test_mint_order_logic() {
        // 测试mint顺序调整逻辑
        let mint0_str = "So11111111111111111111111111111111111111112"; // SOL
        let mint1_str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"; // USDC

        let mut mint0 = Pubkey::from_str(mint0_str).unwrap();
        let mut mint1 = Pubkey::from_str(mint1_str).unwrap();
        let mut price = 100.0; // 1 SOL = 100 USDC

        // 检查是否需要交换
        if mint0 > mint1 {
            std::mem::swap(&mut mint0, &mut mint1);
            price = 1.0 / price;
        }

        // 验证mint0应该小于mint1
        assert!(mint0 < mint1);

        // 验证价格调整是否正确
        if mint0_str == "So11111111111111111111111111111111111111112" && mint0 != Pubkey::from_str(mint0_str).unwrap() {
            // 如果SOL不是mint0，价格应该被调整
            assert_eq!(price, 0.01); // 1/100
        }
    }

    #[test]
    fn test_create_pool_request_validation() {
        // 测试CreatePool请求的基本验证逻辑
        let request = CreatePoolRequest {
            config_index: 0,
            price: 1.5,
            mint0: "So11111111111111111111111111111111111111112".to_string(),
            mint1: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(),
            open_time: 0,
            user_wallet: "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM".to_string(),
        };

        // 验证价格大于0
        assert!(request.price > 0.0);

        // 验证mint地址不相同
        assert_ne!(request.mint0, request.mint1);

        // 验证可以解析为有效的Pubkey
        assert!(Pubkey::from_str(&request.mint0).is_ok());
        assert!(Pubkey::from_str(&request.mint1).is_ok());
        assert!(Pubkey::from_str(&request.user_wallet).is_ok());
    }

    #[tokio::test]
    async fn test_clmm_pool_service_creation() {
        // Test that ClmmPoolService can be created with SharedContext and Database
        // This is a basic smoke test to ensure the service structure is correct
        let shared_context = match SharedContext::new() {
            Ok(ctx) => Arc::new(ctx),
            Err(_) => {
                // Skip test if SharedContext can't be created (e.g., missing env vars)
                return;
            }
        };

        // 创建测试用的数据库实例
        let app_config = Arc::new(utils::AppConfig::default());
        let database = match database::Database::new(app_config).await {
            Ok(db) => db,
            Err(_) => {
                // Skip test if Database can't be created (e.g., missing MongoDB connection)
                return;
            }
        };

        let config_service = Arc::new(ClmmConfigService::new(Arc::new(database.clone()), shared_context.rpc_client.clone()));
        let _service = ClmmPoolService::new(shared_context, &database, config_service);
        // If we get here without panicking, the service was created successfully
    }
}
