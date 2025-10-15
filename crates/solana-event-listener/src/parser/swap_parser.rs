use crate::{
    config::EventListenerConfig,
    error::{EventListenerError, Result},
    parser::{EventParser, ParsedEvent},
};
use async_trait::async_trait;
use base64::{engine::general_purpose, Engine as _};
use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use tracing::{debug, info, warn};

/// 交换事件的原始数据结构（与最新智能合约保持一致）
/// 最新的SwapEvent结构体（需求中提供的新版本）
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct SwapEvent {
    /// 支付者/交换发起者
    pub payer: Pubkey,
    /// 池子ID
    pub pool_id: Pubkey,
    /// 输入金库余额（扣除交易费后）
    pub input_vault_before: u64,
    /// 输出金库余额（扣除交易费后）
    pub output_vault_before: u64,
    /// 输入数量（不含转账费）
    pub input_amount: u64,
    /// 输出数量（不含转账费）
    pub output_amount: u64,
    /// 输入转账费
    pub input_transfer_fee: u64,
    /// 输出转账费
    pub output_transfer_fee: u64,
    /// 是否是基础代币输入
    pub base_input: bool,
    /// 输入代币mint地址
    pub input_mint: Pubkey,
    /// 输出代币mint地址
    pub output_mint: Pubkey,
    /// 交易手续费
    pub trade_fee: u64,
    /// 创建者费用
    pub creator_fee: u64,
    /// 创建者费用是否在输入代币上
    pub creator_fee_on_input: bool,
}

/// 交换事件数据（用于事件系统传递）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwapEventData {
    /// 支付者/交换发起者
    pub payer: String,
    /// 池子地址
    pub pool_id: String,
    /// 输入金库余额（扣除交易费后）
    pub input_vault_before: u64,
    /// 输出金库余额（扣除交易费后）
    pub output_vault_before: u64,
    /// 输入数量（不含转账费）
    pub input_amount: u64,
    /// 输出数量（不含转账费）
    pub output_amount: u64,
    /// 输入转账费
    pub input_transfer_fee: u64,
    /// 输出转账费
    pub output_transfer_fee: u64,
    /// 是否是基础代币输入
    pub base_input: bool,
    /// 输入代币mint地址
    pub input_mint: String,
    /// 输出代币mint地址
    pub output_mint: String,
    /// 交易手续费
    pub trade_fee: u64,
    /// 创建者费用
    pub creator_fee: u64,
    /// 创建者费用是否在输入代币上
    pub creator_fee_on_input: bool,
    /// 交易签名
    pub signature: String,
    /// 区块高度
    pub slot: u64,
    /// 处理时间
    pub processed_at: String,
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
        // 根据设计文档，使用事件类型名称计算discriminator
        let discriminator = crate::parser::event_parser::calculate_event_discriminator("SwapEvent");

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
            return Err(EventListenerError::EventParsing(
                "数据长度不足，无法包含discriminator".to_string(),
            ));
        }

        // 验证discriminator
        let discriminator = &data[0..8];
        if discriminator != self.discriminator {
            return Err(EventListenerError::DiscriminatorMismatch);
        }

        // Borsh反序列化事件数据
        let event_data = &data[8..];
        let event = SwapEvent::try_from_slice(event_data)
            .map_err(|e| EventListenerError::EventParsing(format!("Borsh反序列化失败: {}", e)))?;

        debug!(
            "✅ 成功解析交换事件: 池子={}, 发送者={}, input={}, output={}",
            event.pool_id, event.payer, event.input_amount, event.output_amount
        );
        Ok(event)
    }

    /// 将原始事件转换为SwapEventData
    fn convert_to_parsed_event(&self, event: SwapEvent, signature: String, slot: u64) -> ParsedEvent {
        ParsedEvent::Swap(SwapEventData {
            payer: event.payer.to_string(),
            pool_id: event.pool_id.to_string(),
            input_vault_before: event.input_vault_before,
            output_vault_before: event.output_vault_before,
            input_amount: event.input_amount,
            output_amount: event.output_amount,
            input_transfer_fee: event.input_transfer_fee,
            output_transfer_fee: event.output_transfer_fee,
            base_input: event.base_input,
            input_mint: event.input_mint.to_string(),
            output_mint: event.output_mint.to_string(),
            trade_fee: event.trade_fee,
            creator_fee: event.creator_fee,
            creator_fee_on_input: event.creator_fee_on_input,
            signature,
            slot,
            processed_at: chrono::Utc::now().to_rfc3339(),
        })
    }

    /// 验证交换事件数据
    fn validate_swap(&self, event: &SwapEvent) -> Result<bool> {
        // 验证池子地址
        if event.pool_id == Pubkey::default() {
            warn!("❌ 无效的池子地址");
            return Ok(false);
        }

        // 验证支付者地址
        if event.payer == Pubkey::default() {
            warn!("❌ 无效的支付者地址");
            return Ok(false);
        }

        // 验证输入输出代币地址
        if event.input_mint == Pubkey::default() || event.output_mint == Pubkey::default() {
            warn!("❌ 无效的代币mint地址");
            return Ok(false);
        }

        // 验证交换数量
        if event.input_amount == 0 && event.output_amount == 0 {
            warn!("❌ 输入和输出数量不能同时为0");
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
                                "💱 第{}行发现交换事件: 池子={}, 交换者={}, 输入={}, 输出={}",
                                index + 1,
                                event.pool_id,
                                event.payer,
                                event.input_amount,
                                event.output_amount
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
    use crate::parser::token_creation_parser::TokenCreationEventData;

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
            backfill: None,
        }
    }

    #[test]
    fn test_manual_swap_event_parsing() {
        // 实际交换事件的Program data样本
        let program_data_samples = vec![
            "QMbN6CYIceLMGVG4MU+4ATrjvnYksJMPuMJgCPDP1rdRiKjoj6HsZW5rIlaQU+bQ2trw/mEw5Ts8MT5LpaWvcjF+jxy32bzweGbf5NhXXDsAo6eSe6tqrro9sQFopURaKkodvL3GGqAbpd/JYbZV98UXob/ADOEQw+2rDIEszGzDveqoHB9EswjsDgAAAAAAAAAAAAAAAABAQg8AAAAAAAAAAAAAAAAAAOBhVPT8qoQCAQAAAAAAAABPO8PfAAAAAAAAAAAAAAAAwwAAAA==",
        ];

        let expected_swap_discriminator = crate::parser::event_parser::calculate_event_discriminator("SwapEvent");

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
                                println!("🔍 Payer: {}", swap_event.payer);
                                println!("🔍 Pool ID: {}", swap_event.pool_id);
                                println!("🔍 Input Amount: {}", swap_event.input_amount);
                                println!("🔍 Output Amount: {}", swap_event.output_amount);
                                println!("🔍 Input Mint: {}", swap_event.input_mint);
                                println!("🔍 Output Mint: {}", swap_event.output_mint);
                                println!("🔍 Base Input: {}", swap_event.base_input);
                                println!("🔍 Trade Fee: {}", swap_event.trade_fee);

                                // 验证关键字段合理性
                                assert!(!swap_event.pool_id.to_string().is_empty());
                                assert!(!swap_event.payer.to_string().is_empty());
                                println!("✅ SwapEvent字段验证通过");
                            }
                            Err(e) => {
                                println!("❌ SwapEvent解析失败: {}", e);
                                println!("事件数据长度: {} bytes", event_data.len());
                                // 打印前32字节的十六进制数据用于调试
                                let hex_data = event_data
                                    .iter()
                                    .take(32)
                                    .map(|b| format!("{:02x}", b))
                                    .collect::<Vec<_>>()
                                    .join(" ");
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
        assert_eq!(
            parser.get_discriminator(),
            crate::parser::event_parser::calculate_event_discriminator("SwapEvent")
        );
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
            payer: Pubkey::new_unique().to_string(),
            pool_id: Pubkey::new_unique().to_string(),
            input_vault_before: 1000000,
            output_vault_before: 2000000,
            input_amount: 1000000,
            output_amount: 2000000,
            input_transfer_fee: 1000,
            output_transfer_fee: 2000,
            base_input: true,
            input_mint: Pubkey::new_unique().to_string(),
            output_mint: Pubkey::new_unique().to_string(),
            trade_fee: 3000,
            creator_fee: 500,
            creator_fee_on_input: true,
            signature: "test_sig".to_string(),
            slot: 12345,
            processed_at: chrono::Utc::now().to_rfc3339(),
        });

        assert!(parser.validate_event(&swap_event).await.unwrap());

        // 测试其他类型的事件应该返回false
        let token_event = ParsedEvent::TokenCreation(TokenCreationEventData {
            project_config: Pubkey::new_unique().to_string(),
            mint_address: Pubkey::new_unique().to_string(),
            name: "Test".to_string(),
            symbol: "TEST".to_string(),
            metadata_uri: "https://example.com".to_string(),
            logo_uri: "https://example.com/logo.png".to_string(),
            decimals: 9,
            supply: 1000000,
            creator: Pubkey::new_unique().to_string(),
            has_whitelist: false,
            whitelist_deadline: 0,
            created_at: 1234567890,
            signature: "test_sig".to_string(),
            slot: 12345,
            extensions: None,
            source: None,
        });

        assert!(!parser.validate_event(&token_event).await.unwrap());
    }
}
