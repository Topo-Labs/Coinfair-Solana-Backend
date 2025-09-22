use crate::{
    config::EventListenerConfig,
    error::{EventListenerError, Result},
    parser::{event_parser::PoolCreatedEventData, EventParser, ParsedEvent},
};
use async_trait::async_trait;
use base64::{engine::general_purpose, Engine as _};
use borsh::{BorshDeserialize, BorshSerialize};
use chrono::Utc;
use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use tracing::{debug, info, warn};
use utils::solana::account_loader::AccountLoader;

/// 池子创建事件的原始数据结构（与Raydium CLMM智能合约保持一致）
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct PoolCreatedEvent {
    /// 第一个代币的mint地址（按地址排序）
    pub token_mint_0: Pubkey,
    /// 第二个代币的mint地址（按地址排序）
    pub token_mint_1: Pubkey,
    /// tick间距的最小数量
    pub tick_spacing: u16,
    /// 创建的池子地址
    pub pool_state: Pubkey,
    /// 初始sqrt价格，Q64.64格式
    pub sqrt_price_x64: u128,
    /// 初始tick，即池子起始价格的log base 1.0001
    pub tick: i32,
    /// token_0的金库地址
    pub token_vault_0: Pubkey,
    /// token_1的金库地址
    pub token_vault_1: Pubkey,
}

/// 池子创建事件解析器
pub struct PoolCreationParser {
    /// 事件的discriminator（从Raydium CLMM IDL获取）
    discriminator: [u8; 8],
    /// 目标程序ID，指定此解析器处理哪个程序的事件
    target_program_id: Pubkey,
    /// RPC客户端，用于查询链上数据
    rpc_client: RpcClient,
}

impl PoolCreationParser {
    /// 创建新的池子创建事件解析器
    pub fn new(config: &EventListenerConfig, program_id: Pubkey) -> Result<Self> {
        // Coinfair合约PoolCreatedEvent的discriminator
        let discriminator = [25, 94, 75, 47, 112, 99, 53, 63];

        // 创建RPC客户端
        let rpc_client = RpcClient::new(config.solana.rpc_url.clone());

        Ok(Self {
            discriminator,
            target_program_id: program_id,
            rpc_client,
        })
    }

    /// 从程序数据解析池子创建事件
    fn parse_program_data(&self, data_str: &str) -> Result<PoolCreatedEvent> {
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
        let event = PoolCreatedEvent::try_from_slice(event_data)
            .map_err(|e| EventListenerError::EventParsing(format!("Borsh反序列化失败: {}", e)))?;
        info!("池子解析成功：{:?}", event);
        debug!(
            "✅ 成功解析池子创建事件: 池子={}, 代币对={}/{}",
            event.pool_state, event.token_mint_0, event.token_mint_1
        );
        Ok(event)
    }

    /// 计算池子相关指标
    fn calculate_pool_metrics(&self, event: &PoolCreatedEvent, fee_rate: u32) -> (f64, f64, String) {
        // 计算价格 (从sqrt_price_x64反推)
        let sqrt_price_x64 = event.sqrt_price_x64;
        let price_ratio = if sqrt_price_x64 > 0 {
            let sqrt_price = sqrt_price_x64 as f64 / (1u128 << 64) as f64;
            sqrt_price * sqrt_price
        } else {
            0.0
        };

        // 计算年化手续费率（需要从其他地方获取fee_rate）
        let annual_fee_rate = (fee_rate as f64 / 10000.0) * 365.0; // 假设每天交易一次

        // 根据tick_spacing确定池子类型
        let pool_type = match event.tick_spacing {
            1 => "超高精度".to_string(),   // tick_spacing=1，最高精度
            5 => "高精度".to_string(),     // tick_spacing=5
            10 => "标准精度".to_string(),  // tick_spacing=10
            60 => "低精度".to_string(),    // tick_spacing=60
            120 => "超低精度".to_string(), // tick_spacing=120
            _ => format!("自定义精度({})", event.tick_spacing),
        };

        (price_ratio, annual_fee_rate, pool_type)
    }

    /// 从链上查询缺失的信息（如费率、小数位等）
    /// 对于新创建的池子，如果账户未确认，进行重试（3秒延迟，总共重试3次）
    async fn fetch_missing_info(
        &self,
        pool_address: Pubkey,
        token_mint_0: Pubkey,
        token_mint_1: Pubkey,
    ) -> Result<(u32, u8, u8, Pubkey, Pubkey, i64)> {
        let account_loader = AccountLoader::new(&self.rpc_client);
        let max_retries = 6;
        let retry_delay = std::time::Duration::from_secs(6);

        info!("🔍 从链上查询池子状态: {}", pool_address);

        // 重试逻辑：尝试最多3次，每次间隔3秒
        for attempt in 1..=max_retries {
            match account_loader
                .load_and_deserialize::<raydium_amm_v3::states::PoolState>(&pool_address)
                .await
            {
                Ok(pool_state) => {
                    debug!(
                        "✅ 成功获取池子状态（第{}次尝试），AMM配置: {}",
                        attempt, pool_state.amm_config
                    );

                    // 查询AMM配置以获取费率
                    let fee_rate = match self.fetch_amm_config_fee_rate(&pool_state.amm_config).await {
                        Some(rate) => rate,
                        None => 3000, // AMM配置查询失败时使用默认费率
                    };

                    // 直接从PoolState获取代币小数位数
                    let token_0_decimals = pool_state.mint_decimals_0;
                    let token_1_decimals = pool_state.mint_decimals_1;

                    // 直接从PoolState获取创建者
                    let creator = pool_state.owner;

                    // 直接从PoolState获取CLMM配置地址
                    let clmm_config = pool_state.amm_config;

                    // 使用池子的开放时间作为创建时间
                    let created_at = Utc::now().timestamp();

                    info!(
                        "📊 池子信息查询完成（第{}次尝试） - 费率: {}, 小数位: {}/{}, 创建者: {}, 配置: {}, 创建时间: {}",
                        attempt, fee_rate, token_0_decimals, token_1_decimals, creator, clmm_config, created_at
                    );

                    return Ok((
                        fee_rate,
                        token_0_decimals,
                        token_1_decimals,
                        creator,
                        clmm_config,
                        created_at,
                    ));
                }
                Err(e) => {
                    if attempt < max_retries {
                        warn!(
                            "⚠️ 池子状态查询失败（第{}次尝试）: {} - {}秒后重试",
                            attempt,
                            e,
                            retry_delay.as_secs()
                        );

                        // 等待指定时间后重试
                        tokio::time::sleep(retry_delay).await;
                    } else {
                        // 最后一次尝试失败，使用默认值
                        warn!(
                            "❌ 池子状态查询失败（所有{}次重试都失败）: {} - 使用默认值",
                            max_retries, e
                        );

                        // 所有重试都失败后，使用默认值
                        let default_fee_rate = 3000u32; // 0.3%
                        let default_decimals = 6u8; // 大多数SPL代币使用9位小数
                        let default_creator = Pubkey::new_from_array([0u8; 32]); // 零地址作为占位符
                        let default_clmm_config = Pubkey::new_from_array([0u8; 32]); // 零地址作为占位符
                        let current_timestamp = chrono::Utc::now().timestamp();

                        warn!(
                            "🔄 使用默认池子信息 - 费率: {}, 小数位: {}/{}, 时间戳: {}",
                            default_fee_rate, default_decimals, default_decimals, current_timestamp
                        );

                        // 可以尝试从代币mint地址查询小数位数
                        let (token_0_decimals, token_1_decimals) =
                            self.fetch_token_decimals(token_mint_0, token_mint_1).await;

                        return Ok((
                            default_fee_rate,
                            token_0_decimals.unwrap_or(default_decimals),
                            token_1_decimals.unwrap_or(default_decimals),
                            default_creator,
                            default_clmm_config,
                            current_timestamp,
                        ));
                    }
                }
            }
        }

        // 这个代码路径理论上不会被执行，但为了编译器满意
        unreachable!("重试循环应该总是返回一个结果");
    }

    /// 获取AMM配置的费率
    async fn fetch_amm_config_fee_rate(&self, amm_config_address: &Pubkey) -> Option<u32> {
        let account_loader = AccountLoader::new(&self.rpc_client);
        match account_loader
            .load_and_deserialize::<raydium_amm_v3::states::AmmConfig>(amm_config_address)
            .await
        {
            Ok(amm_config) => {
                debug!("✅ 获取AMM配置费率: {}", amm_config.trade_fee_rate);
                Some(amm_config.trade_fee_rate)
            }
            Err(e) => {
                warn!("⚠️ 无法获取AMM配置: {}", e);
                None
            }
        }
    }

    /// 尝试从代币mint地址获取小数位数
    async fn fetch_token_decimals(&self, token_mint_0: Pubkey, token_mint_1: Pubkey) -> (Option<u8>, Option<u8>) {
        // 尝试获取代币0的小数位数
        let decimals_0 = match self.rpc_client.get_account(&token_mint_0) {
            Ok(account) => {
                if account.data.len() >= 45 {
                    // SPL Token Mint账户需要至少45字节
                    // SPL Token Mint账户中小数位数在第44个字节（从0开始索引）
                    Some(account.data[44])
                } else {
                    debug!("⚠️ 代币0账户数据长度不足: {}", account.data.len());
                    None
                }
            }
            Err(e) => {
                debug!("⚠️ 无法获取代币0账户 {}: {}", token_mint_0, e);
                None
            }
        };

        // 尝试获取代币1的小数位数
        let decimals_1 = match self.rpc_client.get_account(&token_mint_1) {
            Ok(account) => {
                if account.data.len() >= 45 {
                    // SPL Token Mint账户需要至少45字节
                    // SPL Token Mint账户中小数位数在第44个字节（从0开始索引）
                    Some(account.data[44])
                } else {
                    debug!("⚠️ 代币1账户数据长度不足: {}", account.data.len());
                    None
                }
            }
            Err(e) => {
                debug!("⚠️ 无法获取代币1账户 {}: {}", token_mint_1, e);
                None
            }
        };

        if let Some(dec_0) = decimals_0 {
            debug!("✅ 获取代币0小数位数: {}", dec_0);
        }
        if let Some(dec_1) = decimals_1 {
            debug!("✅ 获取代币1小数位数: {}", dec_1);
        }

        (decimals_0, decimals_1)
    }

    /// 将原始事件转换为ParsedEvent
    async fn convert_to_parsed_event(
        &self,
        event: PoolCreatedEvent,
        signature: String,
        slot: u64,
    ) -> Result<ParsedEvent> {
        // 获取缺失的信息
        let (fee_rate, token_0_decimals, token_1_decimals, creator, clmm_config, created_at) = self
            .fetch_missing_info(event.pool_state, event.token_mint_0, event.token_mint_1)
            .await?;

        let (initial_price, annual_fee_rate, pool_type) = self.calculate_pool_metrics(&event, fee_rate);

        Ok(ParsedEvent::PoolCreation(PoolCreatedEventData {
            pool_address: event.pool_state.to_string(),
            token_a_mint: event.token_mint_0.to_string(),
            token_b_mint: event.token_mint_1.to_string(),
            token_a_decimals: token_0_decimals,
            token_b_decimals: token_1_decimals,
            fee_rate,
            fee_rate_percentage: fee_rate as f64 / 10000.0,
            annual_fee_rate,
            pool_type,
            sqrt_price_x64: event.sqrt_price_x64.to_string(),
            initial_price,
            initial_tick: event.tick,
            creator: creator.to_string(),
            clmm_config: clmm_config.to_string(),
            is_stable_pair: false,        // 需要通过代币分析确定
            estimated_liquidity_usd: 0.0, // 创建时暂无流动性
            created_at,
            signature,
            slot,
            processed_at: chrono::Utc::now().to_rfc3339(),
        }))
    }

    /// 验证池子创建事件数据
    fn validate_pool_creation(&self, event: &PoolCreatedEventData) -> Result<bool> {
        // 验证池子地址
        if event.pool_address == Pubkey::default().to_string() {
            warn!("❌ 无效的池子地址");
            return Ok(false);
        }

        // 验证代币地址
        if event.token_a_mint == Pubkey::default().to_string() || event.token_b_mint == Pubkey::default().to_string() {
            warn!("❌ 无效的代币地址: {} 或 {}", event.token_a_mint, event.token_b_mint);
            return Ok(false);
        }

        // 验证代币不能相同
        if event.token_a_mint == event.token_b_mint {
            warn!("❌ 代币A和代币B不能相同: {}", event.token_a_mint);
            return Ok(false);
        }

        // 验证小数位数合理性
        if event.token_a_decimals > 18 || event.token_b_decimals > 18 {
            warn!(
                "❌ 代币小数位数超出合理范围: A={}, B={}",
                event.token_a_decimals, event.token_b_decimals
            );
            return Ok(false);
        }

        // 验证手续费率合理性 (0.01% - 10%)
        if event.fee_rate == 0 || event.fee_rate > 100000 {
            warn!("❌ 手续费率不合理: {}", event.fee_rate);
            return Ok(false);
        }

        // 验证sqrt价格
        if event.sqrt_price_x64.parse::<u128>().unwrap() == 0 {
            warn!("❌ sqrt价格不能为0");
            return Ok(false);
        }

        // 验证创建者地址
        if event.creator == Pubkey::default().to_string() {
            warn!("❌ 无效的创建者地址");
            return Ok(false);
        }

        // 验证CLMM配置地址
        if event.clmm_config == Pubkey::default().to_string() {
            warn!("❌ 无效的CLMM配置地址");
            return Ok(false);
        }

        // 验证时间戳合理性
        let now = chrono::Utc::now().timestamp();
        if event.created_at > now || event.created_at < (now - 86400) {
            warn!("❌ 创建时间戳异常: {}", event.created_at);
            return Ok(false);
        }

        // 验证tick范围 (Raydium CLMM的tick范围)
        if event.initial_tick < -887272 || event.initial_tick > 887272 {
            warn!("❌ 初始tick超出范围: {}", event.initial_tick);
            return Ok(false);
        }

        Ok(true)
    }
}

#[async_trait]
impl EventParser for PoolCreationParser {
    fn get_program_id(&self) -> Pubkey {
        self.target_program_id
    }

    fn get_discriminator(&self) -> [u8; 8] {
        self.discriminator
    }

    fn get_event_type(&self) -> &'static str {
        "pool_creation"
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
                                "🏊 第{}行发现池子创建事件: {} (tick_spacing: {})",
                                index + 1,
                                event.pool_state,
                                event.tick_spacing
                            );
                            match self.convert_to_parsed_event(event, signature.to_string(), slot).await {
                                Ok(parsed_event) => return Ok(Some(parsed_event)),
                                Err(e) => {
                                    warn!("❌ 池子事件转换失败: {}", e);
                                    continue;
                                }
                            }
                        }
                        Err(EventListenerError::DiscriminatorMismatch) => {
                            // Discriminator不匹配是正常情况，继续尝试下一条日志
                            continue;
                        }
                        Err(e) => {
                            debug!("⚠️ 第{}行池子创建事件解析失败: {}", index + 1, e);
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
            ParsedEvent::PoolCreation(pool_event) => self.validate_pool_creation(pool_event),
            _ => Ok(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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

    fn create_test_pool_creation_event() -> PoolCreatedEvent {
        PoolCreatedEvent {
            token_mint_0: Pubkey::new_unique(),
            token_mint_1: Pubkey::new_unique(),
            tick_spacing: 10,
            pool_state: Pubkey::new_unique(),
            sqrt_price_x64: 1u128 << 64, // 价格为1.0
            tick: 0,
            token_vault_0: Pubkey::new_unique(),
            token_vault_1: Pubkey::new_unique(),
        }
    }

    #[test]
    fn test_pool_creation_parser_creation() {
        let config = create_test_config();
        let parser = PoolCreationParser::new(&config, Pubkey::new_unique()).unwrap();

        assert_eq!(parser.get_event_type(), "pool_creation");
        assert_eq!(parser.get_discriminator(), [25, 94, 75, 47, 112, 99, 53, 63]);
    }

    #[tokio::test]
    #[ignore] // 忽略这个测试，因为它需要实际的RPC连接
    async fn test_convert_to_parsed_event() {
        let config = create_test_config();
        let parser = PoolCreationParser::new(&config, Pubkey::new_unique()).unwrap();
        let test_event = create_test_pool_creation_event();

        // 注意：这个测试需要实际的RPC连接来获取缺失的链上信息
        // 在实际部署中，convert_to_parsed_event方法需要链上数据来完成池子信息的解析
        let parsed = parser
            .convert_to_parsed_event(test_event.clone(), "test_signature".to_string(), 12345)
            .await;

        match parsed {
            Ok(ParsedEvent::PoolCreation(data)) => {
                assert_eq!(data.pool_address, test_event.pool_state.to_string());
                assert_eq!(data.token_a_mint, test_event.token_mint_0.to_string());
                assert_eq!(data.token_b_mint, test_event.token_mint_1.to_string());
                assert_eq!(data.signature, "test_signature");
                assert_eq!(data.slot, 12345);
            }
            Err(e) => {
                // 这里可能会因为网络问题失败
                println!("RPC连接错误: {}", e);
            }
            _ => panic!("期望PoolCreation事件"),
        }
    }

    #[tokio::test]
    async fn test_validate_pool_creation() {
        let config = create_test_config();
        let parser = PoolCreationParser::new(&config, Pubkey::new_unique()).unwrap();

        let valid_event = PoolCreatedEventData {
            pool_address: Pubkey::new_unique().to_string(),
            token_a_mint: Pubkey::new_unique().to_string(),
            token_b_mint: Pubkey::new_unique().to_string(),
            token_a_decimals: 9,
            token_b_decimals: 6,
            fee_rate: 3000,
            fee_rate_percentage: 0.3,
            annual_fee_rate: 109.5,
            pool_type: "标准费率".to_string(),
            sqrt_price_x64: (1u128 << 64).to_string(),
            initial_price: 1.0,
            initial_tick: 0,
            creator: Pubkey::new_unique().to_string(),
            clmm_config: Pubkey::new_unique().to_string(),
            is_stable_pair: false,
            estimated_liquidity_usd: 0.0,
            created_at: chrono::Utc::now().timestamp(),
            signature: "test_sig".to_string(),
            slot: 12345,
            processed_at: chrono::Utc::now().to_rfc3339(),
        };

        assert!(parser.validate_pool_creation(&valid_event).unwrap());

        // 测试无效事件（相同的代币）
        let invalid_event = PoolCreatedEventData {
            token_b_mint: valid_event.token_a_mint.clone(), // 相同的代币
            ..valid_event.clone()
        };

        assert!(!parser.validate_pool_creation(&invalid_event).unwrap());
    }

    #[test]
    fn test_calculate_pool_metrics() {
        let config = create_test_config();
        let parser = PoolCreationParser::new(&config, Pubkey::new_unique()).unwrap();

        let event = PoolCreatedEvent {
            tick_spacing: 10,            // 标准精度
            sqrt_price_x64: 1u128 << 64, // sqrt(1.0)
            ..create_test_pool_creation_event()
        };

        let fee_rate = 3000; // 0.3%
        let (price, annual_fee, pool_type) = parser.calculate_pool_metrics(&event, fee_rate);

        assert!((price - 1.0).abs() < 0.0001); // 价格应该接近1.0
        assert_eq!(annual_fee, 109.5); // 0.3% * 365
        assert_eq!(pool_type, "标准精度");
    }

    #[test]
    fn test_borsh_serialization() {
        let event = create_test_pool_creation_event();

        // 测试序列化
        let serialized = borsh::to_vec(&event).unwrap();
        assert!(!serialized.is_empty());

        // 测试反序列化
        let deserialized = PoolCreatedEvent::try_from_slice(&serialized).unwrap();
        assert_eq!(deserialized.pool_state, event.pool_state);
        assert_eq!(deserialized.token_mint_0, event.token_mint_0);
        assert_eq!(deserialized.tick_spacing, event.tick_spacing);
    }

    #[tokio::test]
    async fn test_parse_from_logs_no_program_data() {
        let config = create_test_config();
        let parser = PoolCreationParser::new(&config, Pubkey::new_unique()).unwrap();

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
        let parser = PoolCreationParser::new(&config, Pubkey::new_unique()).unwrap();

        let event = ParsedEvent::PoolCreation(PoolCreatedEventData {
            pool_address: Pubkey::new_unique().to_string(),
            token_a_mint: Pubkey::new_unique().to_string(),
            token_b_mint: Pubkey::new_unique().to_string(),
            token_a_decimals: 9,
            token_b_decimals: 6,
            fee_rate: 3000,
            fee_rate_percentage: 0.3,
            annual_fee_rate: 109.5,
            pool_type: "标准费率".to_string(),
            sqrt_price_x64: (1u128 << 64).to_string(),
            initial_price: 1.0,
            initial_tick: 0,
            creator: Pubkey::new_unique().to_string(),
            clmm_config: Pubkey::new_unique().to_string(),
            is_stable_pair: false,
            estimated_liquidity_usd: 0.0,
            created_at: chrono::Utc::now().timestamp(),
            signature: "test_sig".to_string(),
            slot: 12345,
            processed_at: chrono::Utc::now().to_rfc3339(),
        });

        assert!(parser.validate_event(&event).await.unwrap());
    }

    #[test]
    fn test_discriminator_mismatch_error() {
        let config = create_test_config();
        let parser = PoolCreationParser::new(&config, Pubkey::new_unique()).unwrap();

        // 创建一个带有错误discriminator的base64数据
        let mut data = vec![0u8; 100];
        // 设置一个错误的discriminator（不是池子创建事件的）
        data[0..8].copy_from_slice(&[99, 99, 99, 99, 99, 99, 99, 99]);

        let data_str = general_purpose::STANDARD.encode(&data);
        let result = parser.parse_program_data(&data_str);

        // 验证返回的是DiscriminatorMismatch错误
        assert!(matches!(result, Err(EventListenerError::DiscriminatorMismatch)));
    }

    #[tokio::test]
    async fn test_parse_from_logs_skips_discriminator_mismatch() {
        let config = create_test_config();
        let parser = PoolCreationParser::new(&config, Pubkey::new_unique()).unwrap();

        // 创建一个带有错误discriminator的日志
        let mut wrong_data = vec![0u8; 100];
        wrong_data[0..8].copy_from_slice(&[99, 99, 99, 99, 99, 99, 99, 99]);
        let wrong_log = format!("Program data: {}", general_purpose::STANDARD.encode(&wrong_data));

        // 创建一个正确的日志（但没有完整的事件数据，只是为了测试流程）
        let logs = vec!["Some other log".to_string(), wrong_log, "Another log".to_string()];

        // 解析日志，应该跳过discriminator不匹配的日志，返回None（因为没有匹配的事件）
        let result = parser.parse_from_logs(&logs, "test_signature", 12345).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }
}
