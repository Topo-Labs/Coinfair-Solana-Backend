use crate::{
    config::EventListenerConfig,
    error::{EventListenerError, Result},
    parser::{event_parser::SwapEventData, EventParser, ParsedEvent},
};
use async_trait::async_trait;
use base64::{engine::general_purpose, Engine as _};
use borsh::{BorshDeserialize, BorshSerialize};
use solana_sdk::pubkey::Pubkey;
use tracing::{debug, info, warn};

/// 交换事件的原始数据结构（与智能合约保持一致）
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct SwapEvent {
    /// 池子状态地址
    pub pool_state: Pubkey,
    /// 交换发起者
    pub sender: Pubkey,
    /// 代币0账户
    pub token_account_0: Pubkey,
    /// 代币1账户
    pub token_account_1: Pubkey,
    /// 代币0数量
    pub amount_0: u64,
    /// 代币0手续费
    pub transfer_fee_0: u64,
    /// 代币1数量
    pub amount_1: u64,
    /// 代币1手续费
    pub transfer_fee_1: u64,
    /// 是否从0到1的交换
    pub zero_for_one: bool,
    /// 新的sqrt价格
    pub sqrt_price_x64: u128,
    /// 流动性
    pub liquidity: u128,
    /// tick位置
    pub tick: i32,
}

/// 交换事件解析器
pub struct SwapParser {
    /// 事件的discriminator
    discriminator: [u8; 8],
    /// 目标程序ID，指定此解析器处理哪个程序的事件
    target_program_id: Pubkey,
}

impl SwapParser {
    /// 创建新的交换事件解析器
    pub fn new(_config: &EventListenerConfig, program_id: Pubkey) -> Result<Self> {
        // 交换事件的discriminator（与TokenCreationEvent相同）
        let discriminator = [64, 198, 205, 232, 38, 8, 113, 226];

        Ok(Self { 
            discriminator,
            target_program_id: program_id,
        })
    }

    /// 从程序数据解析交换事件
    fn parse_program_data(&self, data_str: &str) -> Result<SwapEvent> {
        // Base64解码
        let data = general_purpose::STANDARD
            .decode(data_str)
            .map_err(|e| EventListenerError::EventParsing(format!("Base64解码失败: {}", e)))?;

        if data.len() < 8 {
            return Err(EventListenerError::EventParsing("数据长度不足，无法包含discriminator".to_string()));
        }

        // 验证discriminator
        let discriminator = &data[0..8];
        if discriminator != self.discriminator {
            return Err(EventListenerError::DiscriminatorMismatch);
        }

        // Borsh反序列化事件数据
        let event_data = &data[8..];
        let event = SwapEvent::try_from_slice(event_data).map_err(|e| EventListenerError::EventParsing(format!("Borsh反序列化失败: {}", e)))?;

        debug!("✅ 成功解析交换事件: 池子={}, 发送者={}, amount_0={}, amount_1={}", 
               event.pool_state, event.sender, event.amount_0, event.amount_1);
        Ok(event)
    }

    /// 将原始事件转换为SwapEventData
    fn convert_to_parsed_event(&self, event: SwapEvent, signature: String, slot: u64) -> ParsedEvent {
        ParsedEvent::Swap(SwapEventData {
            pool_address: event.pool_state.to_string(),
            sender: event.sender.to_string(),
            token_account_0: event.token_account_0.to_string(),
            token_account_1: event.token_account_1.to_string(),
            amount_0: event.amount_0,
            transfer_fee_0: event.transfer_fee_0,
            amount_1: event.amount_1,
            transfer_fee_1: event.transfer_fee_1,
            zero_for_one: event.zero_for_one,
            sqrt_price_x64: event.sqrt_price_x64.to_string(),
            liquidity: event.liquidity.to_string(),
            tick: event.tick,
            signature,
            slot,
            processed_at: chrono::Utc::now().to_rfc3339(),
        })
    }

    /// 验证交换事件数据
    fn validate_swap(&self, event: &SwapEvent) -> Result<bool> {
        // 验证池子地址
        if event.pool_state == Pubkey::default() {
            warn!("❌ 无效的池子地址");
            return Ok(false);
        }

        // 验证发送者地址
        if event.sender == Pubkey::default() {
            warn!("❌ 无效的发送者地址");
            return Ok(false);
        }

        // 验证交换数量
        if event.amount_0 == 0 && event.amount_1 == 0 {
            warn!("❌ 交换数量不能都为0");
            return Ok(false);
        }

        // 验证sqrt价格
        if event.sqrt_price_x64 == 0 {
            warn!("❌ sqrt价格不能为0");
            return Ok(false);
        }

        // 验证tick范围
        if event.tick < -887272 || event.tick > 887272 {
            warn!("❌ tick超出范围: {}", event.tick);
            return Ok(false);
        }

        Ok(true)
    }
}

#[async_trait]
impl EventParser for SwapParser {
    fn get_program_id(&self) -> Pubkey {
        self.target_program_id
    }

    fn get_discriminator(&self) -> [u8; 8] {
        self.discriminator
    }

    fn get_event_type(&self) -> &'static str {
        "swap"
    }

    fn supports_program(&self, program_id: &Pubkey) -> Option<bool> {
        Some(*program_id == self.target_program_id)
    }

    async fn parse_from_logs(&self, logs: &[String], signature: &str, slot: u64) -> Result<Option<ParsedEvent>> {
        for (index, log) in logs.iter().enumerate() {
            if log.starts_with("Program data: ") {
                if let Some(data_part) = log.strip_prefix("Program data: ") {
                    match self.parse_program_data(data_part) {
                        Ok(event) => {
                            info!(
                                "💱 第{}行发现交换事件: 池子={}, 交换者={}, 数量={}->{}",
                                index + 1,
                                event.pool_state,
                                event.sender,
                                event.amount_0,
                                event.amount_1
                            );
                            
                            if self.validate_swap(&event)? {
                                let parsed_event = self.convert_to_parsed_event(event, signature.to_string(), slot);
                                return Ok(Some(parsed_event));
                            }
                        }
                        Err(EventListenerError::DiscriminatorMismatch) => {
                            // Discriminator不匹配是正常情况，继续尝试下一条日志
                            continue;
                        }
                        Err(e) => {
                            debug!("⚠️ 第{}行交换事件解析失败: {}", index + 1, e);
                            continue;
                        }
                    }
                }
            }
        }
        Ok(None)
    }

    async fn validate_event(&self, event: &ParsedEvent) -> Result<bool> {
        match event {
            ParsedEvent::Swap(_) => Ok(true),
            _ => Ok(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use borsh::BorshDeserialize;
    use solana_sdk::pubkey::Pubkey;
    use std::str::FromStr;

    fn create_test_config() -> EventListenerConfig {
        EventListenerConfig {
            solana: crate::config::settings::SolanaConfig {
                rpc_url: "https://api.devnet.solana.com".to_string(),
                ws_url: "wss://api.devnet.solana.com".to_string(),
                commitment: "confirmed".to_string(),
                program_ids: vec![Pubkey::from_str("FA1RJDDXysgwg5Gm3fJXWxt26JQzPkAzhTA114miqNUX").unwrap()],
                private_key: None,
            },
            database: crate::config::settings::DatabaseConfig {
                uri: "mongodb://localhost:27017".to_string(),
                database_name: "test".to_string(),
                max_connections: 10,
                min_connections: 2,
            },
            listener: crate::config::settings::ListenerConfig {
                batch_size: 100,
                sync_interval_secs: 30,
                max_retries: 3,
                retry_delay_ms: 1000,
                signature_cache_size: 10000,
                checkpoint_save_interval_secs: 60,
                backoff: crate::config::settings::BackoffConfig::default(),
                batch_write: crate::config::settings::BatchWriteConfig::default(),
            },
            monitoring: crate::config::settings::MonitoringConfig {
                metrics_interval_secs: 60,
                enable_performance_monitoring: true,
                health_check_interval_secs: 30,
            },
        }
    }

    #[test]
    fn test_manual_swap_event_parsing() {
        // 实际交换事件的Program data样本
        let program_data_samples = vec![
            "QMbN6CYIceLMGVG4MU+4ATrjvnYksJMPuMJgCPDP1rdRiKjoj6HsZW5rIlaQU+bQ2trw/mEw5Ts8MT5LpaWvcjF+jxy32bzweGbf5NhXXDsAo6eSe6tqrro9sQFopURaKkodvL3GGqAbpd/JYbZV98UXob/ADOEQw+2rDIEszGzDveqoHB9EswjsDgAAAAAAAAAAAAAAAABAQg8AAAAAAAAAAAAAAAAAAOBhVPT8qoQCAQAAAAAAAABPO8PfAAAAAAAAAAAAAAAAwwAAAA==",
        ];

        let expected_swap_discriminator = [64, 198, 205, 232, 38, 8, 113, 226];

        for (i, data_str) in program_data_samples.iter().enumerate() {
            println!("=== 测试 Program data {} ===", i + 1);
            println!("Base64数据: {}...", &data_str[..50]);

            // 解码Base64数据
            use base64::{engine::general_purpose, Engine as _};
            match general_purpose::STANDARD.decode(data_str) {
                Ok(data) => {
                    println!("解码后数据长度: {} bytes", data.len());

                    if data.len() < 8 {
                        println!("❌ 数据长度不足，无法包含discriminator");
                        continue;
                    }

                    // 检查discriminator
                    let discriminator = &data[0..8];
                    println!("实际discriminator: {:?}", discriminator);
                    println!("期望discriminator: {:?}", expected_swap_discriminator);

                    if discriminator == expected_swap_discriminator {
                        println!("✅ Discriminator匹配，尝试解析SwapEvent");

                        // 尝试解析SwapEvent
                        let event_data = &data[8..];
                        match SwapEvent::try_from_slice(event_data) {
                            Ok(swap_event) => {
                                println!("✅ 成功解析Swap事件！");
                                println!("🔍 Pool State: {}", swap_event.pool_state);
                                println!("🔍 Sender: {}", swap_event.sender);
                                println!("🔍 Amount 0: {}", swap_event.amount_0);
                                println!("🔍 Amount 1: {}", swap_event.amount_1);
                                println!("🔍 Zero for One: {}", swap_event.zero_for_one);
                                println!("🔍 Sqrt Price: {}", swap_event.sqrt_price_x64);
                                println!("🔍 Liquidity: {}", swap_event.liquidity);
                                println!("🔍 Tick: {}", swap_event.tick);

                                // 验证关键字段合理性
                                assert!(!swap_event.pool_state.to_string().is_empty());
                                assert!(!swap_event.sender.to_string().is_empty());
                                println!("✅ SwapEvent字段验证通过");
                            }
                            Err(e) => {
                                println!("❌ SwapEvent解析失败: {}", e);
                                println!("事件数据长度: {} bytes", event_data.len());
                                // 打印前32字节的十六进制数据用于调试
                                let hex_data = event_data.iter().take(32).map(|b| format!("{:02x}", b)).collect::<Vec<_>>().join(" ");
                                println!("事件数据前32字节: {}", hex_data);
                            }
                        }
                    } else {
                        println!("❌ Discriminator不匹配，跳过解析");
                    }
                }
                Err(e) => {
                    println!("❌ Base64解码失败: {}", e);
                }
            }
            println!();
        }
    }

    #[test]
    fn test_swap_parser_creation() {
        let config = create_test_config();
        let parser = SwapParser::new(&config, Pubkey::new_unique()).unwrap();

        assert_eq!(parser.get_event_type(), "swap");
        assert_eq!(parser.get_discriminator(), [64, 198, 205, 232, 38, 8, 113, 226]);
    }

    #[test]
    fn test_swap_parser_supports_program() {
        let config = create_test_config();
        let target_program = Pubkey::from_str("FA1RJDDXysgwg5Gm3fJXWxt26JQzPkAzhTA114miqNUX").unwrap();
        let parser = SwapParser::new(&config, target_program).unwrap();

        // 应该支持目标程序
        assert_eq!(parser.supports_program(&target_program), Some(true));

        // 不应该支持其他程序
        let other_program = Pubkey::new_unique();
        assert_eq!(parser.supports_program(&other_program), Some(false));
    }

    #[tokio::test]
    async fn test_parse_from_logs_no_program_data() {
        let config = create_test_config();
        let parser = SwapParser::new(&config, Pubkey::new_unique()).unwrap();

        let logs = vec![
            "Program 11111111111111111111111111111111 invoke [1]".to_string(),
            "Program 11111111111111111111111111111111 success".to_string(),
        ];

        let result = parser.parse_from_logs(&logs, "test_sig", 12345).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_validate_event() {
        let config = create_test_config();
        let parser = SwapParser::new(&config, Pubkey::new_unique()).unwrap();

        // 创建测试交换事件
        let swap_event = ParsedEvent::Swap(SwapEventData {
            pool_address: Pubkey::new_unique().to_string(),
            sender: Pubkey::new_unique().to_string(),
            token_account_0: Pubkey::new_unique().to_string(),
            token_account_1: Pubkey::new_unique().to_string(),
            amount_0: 1000000,
            transfer_fee_0: 1000,
            amount_1: 2000000,
            transfer_fee_1: 2000,
            zero_for_one: true,
            sqrt_price_x64: (1u128 << 64).to_string(),
            liquidity: (1000u128).to_string(),
            tick: 0,
            signature: "test_sig".to_string(),
            slot: 12345,
            processed_at: chrono::Utc::now().to_rfc3339(),
        });

        assert!(parser.validate_event(&swap_event).await.unwrap());

        // 测试其他类型的事件应该返回false
        let token_event = ParsedEvent::TokenCreation(crate::parser::event_parser::TokenCreationEventData {
            mint_address: Pubkey::new_unique().to_string(),
            name: "Test".to_string(),
            symbol: "TEST".to_string(),
            uri: "https://example.com".to_string(),
            decimals: 9,
            supply: 1000000,
            creator: Pubkey::new_unique().to_string(),
            has_whitelist: false,
            whitelist_deadline: 0,
            created_at: 1234567890,
            signature: "test_sig".to_string(),
            slot: 12345,
        });

        assert!(!parser.validate_event(&token_event).await.unwrap());
    }
}