use crate::{
    config::EventListenerConfig,
    error::{EventListenerError, Result},
    parser::{event_parser::NftClaimEventData, EventParser, ParsedEvent},
};
use async_trait::async_trait;
use base64::{engine::general_purpose, Engine as _};
use borsh::{BorshDeserialize, BorshSerialize};
use solana_sdk::pubkey::Pubkey;
use tracing::{debug, info, warn};

/// NFT领取事件的原始数据结构（与智能合约保持一致）
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct NftClaimEvent {
    /// NFT的mint地址
    pub nft_mint: Pubkey,
    /// 领取者钱包地址
    pub claimer: Pubkey,
    /// 推荐人地址（可选）
    pub referrer: Option<Pubkey>,
    /// NFT等级 (1-5级)
    pub tier: u8,
    /// 领取的代币数量（以最小单位计）
    pub claim_amount: u64,
    /// 代币mint地址
    pub token_mint: Pubkey,
    /// 奖励倍率 (基点，如10000表示1.0倍)
    pub reward_multiplier: u16,
    /// 领取类型 (0: 定期领取, 1: 一次性领取, 2: 紧急领取)
    pub claim_type: u8,
    /// 本次领取后的累计领取量
    pub total_claimed: u64,
    /// NFT所属的池子地址（可选）
    pub pool_address: Option<Pubkey>,
    /// 领取时间戳
    pub claimed_at: i64,
}

/// NFT领取事件解析器
pub struct NftClaimParser {
    /// 事件的discriminator
    discriminator: [u8; 8],
}

impl NftClaimParser {
    /// 创建新的NFT领取事件解析器
    pub fn new(_config: &EventListenerConfig) -> Result<Self> {
        // NFT领取事件的discriminator
        // 注意：实际部署时需要从智能合约IDL获取正确的discriminator
        let discriminator = [234, 123, 45, 67, 89, 101, 213, 42];

        Ok(Self { discriminator })
    }

    /// 从程序数据解析NFT领取事件
    fn parse_program_data(&self, data_str: &str) -> Result<NftClaimEvent> {
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
        let event = NftClaimEvent::try_from_slice(event_data).map_err(|e| EventListenerError::EventParsing(format!("Borsh反序列化失败: {}", e)))?;

        debug!("✅ 成功解析NFT领取事件: NFT={}, 领取者={}, 数量={}", event.nft_mint, event.claimer, event.claim_amount);
        Ok(event)
    }

    /// 计算NFT等级奖励
    fn calculate_tier_bonus(&self, tier: u8) -> f64 {
        match tier {
            1 => 1.0, // 基础等级
            2 => 1.2, // 20%奖励
            3 => 1.5, // 50%奖励
            4 => 2.0, // 100%奖励
            5 => 3.0, // 200%奖励
            _ => 1.0, // 默认基础等级
        }
    }

    /// 获取等级名称
    fn get_tier_name(&self, tier: u8) -> String {
        match tier {
            1 => "Bronze".to_string(),
            2 => "Silver".to_string(),
            3 => "Gold".to_string(),
            4 => "Platinum".to_string(),
            5 => "Diamond".to_string(),
            _ => "Unknown".to_string(),
        }
    }

    /// 获取领取类型名称
    fn get_claim_type_name(&self, claim_type: u8) -> String {
        match claim_type {
            0 => "定期领取".to_string(),
            1 => "一次性领取".to_string(),
            2 => "紧急领取".to_string(),
            _ => "未知类型".to_string(),
        }
    }

    /// 计算奖励相关指标
    fn calculate_reward_metrics(&self, event: &NftClaimEvent) -> (f64, u64, f64) {
        // 计算等级奖励倍率
        let tier_bonus = self.calculate_tier_bonus(event.tier);

        // 计算实际奖励金额（包含倍率）
        let actual_reward_multiplier = event.reward_multiplier as f64 / 10000.0;
        let bonus_amount = (event.claim_amount as f64 * tier_bonus * actual_reward_multiplier) as u64;

        // 计算累计奖励进度
        let progress_percentage = if event.total_claimed > 0 {
            (event.claim_amount as f64 / event.total_claimed as f64) * 100.0
        } else {
            100.0
        };

        (tier_bonus, bonus_amount, progress_percentage)
    }

    /// 将原始事件转换为ParsedEvent
    fn convert_to_parsed_event(&self, event: NftClaimEvent, signature: String, slot: u64) -> ParsedEvent {
        let (tier_bonus_rate, bonus_amount, claim_progress) = self.calculate_reward_metrics(&event);

        ParsedEvent::NftClaim(NftClaimEventData {
            nft_mint: event.nft_mint,
            claimer: event.claimer,
            referrer: event.referrer,
            tier: event.tier,
            tier_name: self.get_tier_name(event.tier),
            tier_bonus_rate,
            claim_amount: event.claim_amount,
            token_mint: event.token_mint,
            reward_multiplier: event.reward_multiplier,
            reward_multiplier_percentage: event.reward_multiplier as f64 / 10000.0,
            bonus_amount,
            claim_type: event.claim_type,
            claim_type_name: self.get_claim_type_name(event.claim_type),
            total_claimed: event.total_claimed,
            claim_progress_percentage: claim_progress,
            pool_address: event.pool_address,
            has_referrer: event.referrer.is_some(),
            is_emergency_claim: event.claim_type == 2,
            estimated_usd_value: 0.0, // 需要通过价格预言机获取
            claimed_at: event.claimed_at,
            signature,
            slot,
            processed_at: chrono::Utc::now().to_rfc3339(),
        })
    }

    /// 验证NFT领取事件数据
    fn validate_nft_claim(&self, event: &NftClaimEventData) -> Result<bool> {
        // 验证NFT地址
        if event.nft_mint == Pubkey::default() {
            warn!("❌ 无效的NFT地址");
            return Ok(false);
        }

        // 验证领取者地址
        if event.claimer == Pubkey::default() {
            warn!("❌ 无效的领取者地址");
            return Ok(false);
        }

        // 验证代币地址
        if event.token_mint == Pubkey::default() {
            warn!("❌ 无效的代币地址");
            return Ok(false);
        }

        // 验证NFT等级范围
        if event.tier == 0 || event.tier > 5 {
            warn!("❌ NFT等级超出范围: {}", event.tier);
            return Ok(false);
        }

        // 验证领取数量
        if event.claim_amount == 0 {
            warn!("❌ 领取数量不能为0");
            return Ok(false);
        }

        // 验证奖励倍率合理性 (0.1倍 - 10倍)
        if event.reward_multiplier < 1000 {
            warn!("❌ 奖励倍率过低: {}", event.reward_multiplier);
            return Ok(false);
        }

        // 验证领取类型
        if event.claim_type > 2 {
            warn!("❌ 无效的领取类型: {}", event.claim_type);
            return Ok(false);
        }

        // 验证累计领取量合理性
        if event.total_claimed < event.claim_amount {
            warn!("❌ 累计领取量不能小于本次领取量: total={}, current={}", event.total_claimed, event.claim_amount);
            return Ok(false);
        }

        // 验证时间戳合理性
        let now = chrono::Utc::now().timestamp();
        if event.claimed_at > now || event.claimed_at < (now - 86400) {
            warn!("❌ 领取时间戳异常: {}", event.claimed_at);
            return Ok(false);
        }

        // 验证推荐人不能是自己
        if let Some(referrer) = event.referrer {
            if referrer == event.claimer {
                warn!("❌ 推荐人不能是自己: {}", event.claimer);
                return Ok(false);
            }
        }

        // 验证奖励金额的合理性
        if event.bonus_amount > event.claim_amount * 10 {
            warn!("❌ 奖励金额过大，可能有计算错误: bonus={}, base={}", event.bonus_amount, event.claim_amount);
            return Ok(false);
        }

        Ok(true)
    }
}

#[async_trait]
impl EventParser for NftClaimParser {
    fn get_discriminator(&self) -> [u8; 8] {
        self.discriminator
    }

    fn get_event_type(&self) -> &'static str {
        "nft_claim"
    }

    async fn parse_from_logs(&self, logs: &[String], signature: &str, slot: u64) -> Result<Option<ParsedEvent>> {
        for (index, log) in logs.iter().enumerate() {
            if log.starts_with("Program data: ") {
                if let Some(data_part) = log.strip_prefix("Program data: ") {
                    match self.parse_program_data(data_part) {
                        Ok(event) => {
                            info!(
                                "🎁 第{}行发现NFT领取事件: {} 领取 {} (等级: {} {})",
                                index + 1,
                                event.claimer,
                                event.claim_amount,
                                event.tier,
                                self.get_tier_name(event.tier)
                            );
                            let parsed_event = self.convert_to_parsed_event(event, signature.to_string(), slot);
                            return Ok(Some(parsed_event));
                        }
                        Err(EventListenerError::DiscriminatorMismatch) => {
                            // Discriminator不匹配是正常情况，继续尝试下一条日志
                            continue;
                        }
                        Err(e) => {
                            debug!("⚠️ 第{}行NFT领取事件解析失败: {}", index + 1, e);
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
            ParsedEvent::NftClaim(nft_event) => self.validate_nft_claim(nft_event),
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
                program_id: Pubkey::from_str("FA1RJDDXysgwg5Gm3fJXWxt26JQzPkAzhTA114miqNUX").unwrap(),
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

    fn create_test_nft_claim_event() -> NftClaimEvent {
        NftClaimEvent {
            nft_mint: Pubkey::new_unique(),
            claimer: Pubkey::new_unique(),
            referrer: Some(Pubkey::new_unique()),
            tier: 3,
            claim_amount: 1000000, // 1 token with 6 decimals
            token_mint: Pubkey::new_unique(),
            reward_multiplier: 15000, // 1.5倍
            claim_type: 0,            // 定期领取
            total_claimed: 5000000,   // 总共领取了5个代币
            pool_address: Some(Pubkey::new_unique()),
            claimed_at: chrono::Utc::now().timestamp(),
        }
    }

    #[test]
    fn test_nft_claim_parser_creation() {
        let config = create_test_config();
        let parser = NftClaimParser::new(&config).unwrap();

        assert_eq!(parser.get_event_type(), "nft_claim");
        assert_eq!(parser.get_discriminator(), [234, 123, 45, 67, 89, 101, 213, 42]);
    }

    #[test]
    fn test_tier_bonus_calculation() {
        let config = create_test_config();
        let parser = NftClaimParser::new(&config).unwrap();

        assert_eq!(parser.calculate_tier_bonus(1), 1.0);
        assert_eq!(parser.calculate_tier_bonus(2), 1.2);
        assert_eq!(parser.calculate_tier_bonus(3), 1.5);
        assert_eq!(parser.calculate_tier_bonus(4), 2.0);
        assert_eq!(parser.calculate_tier_bonus(5), 3.0);
        assert_eq!(parser.calculate_tier_bonus(99), 1.0); // 未知等级
    }

    #[test]
    fn test_tier_name_mapping() {
        let config = create_test_config();
        let parser = NftClaimParser::new(&config).unwrap();

        assert_eq!(parser.get_tier_name(1), "Bronze");
        assert_eq!(parser.get_tier_name(2), "Silver");
        assert_eq!(parser.get_tier_name(3), "Gold");
        assert_eq!(parser.get_tier_name(4), "Platinum");
        assert_eq!(parser.get_tier_name(5), "Diamond");
        assert_eq!(parser.get_tier_name(99), "Unknown");
    }

    #[test]
    fn test_convert_to_parsed_event() {
        let config = create_test_config();
        let parser = NftClaimParser::new(&config).unwrap();
        let test_event = create_test_nft_claim_event();

        let parsed = parser.convert_to_parsed_event(test_event.clone(), "test_signature".to_string(), 12345);

        match parsed {
            ParsedEvent::NftClaim(data) => {
                assert_eq!(data.nft_mint, test_event.nft_mint);
                assert_eq!(data.claimer, test_event.claimer);
                assert_eq!(data.tier, test_event.tier);
                assert_eq!(data.tier_name, "Gold");
                assert_eq!(data.tier_bonus_rate, 1.5);
                assert_eq!(data.claim_amount, test_event.claim_amount);
                assert_eq!(data.reward_multiplier_percentage, 1.5);
                assert_eq!(data.has_referrer, true);
                assert_eq!(data.is_emergency_claim, false);
                assert_eq!(data.signature, "test_signature");
                assert_eq!(data.slot, 12345);
            }
            _ => panic!("期望NftClaim事件"),
        }
    }

    #[test]
    fn test_calculate_reward_metrics() {
        let config = create_test_config();
        let parser = NftClaimParser::new(&config).unwrap();

        let event = NftClaimEvent {
            tier: 3, // Gold tier (1.5x bonus)
            claim_amount: 1000000,
            reward_multiplier: 12000, // 1.2x
            total_claimed: 5000000,
            ..create_test_nft_claim_event()
        };

        let (tier_bonus, bonus_amount, progress) = parser.calculate_reward_metrics(&event);

        assert_eq!(tier_bonus, 1.5);
        assert_eq!(bonus_amount, 1800000); // 1000000 * 1.5 * 1.2
        assert_eq!(progress, 20.0); // 1000000 / 5000000 * 100
    }

    #[tokio::test]
    async fn test_validate_nft_claim() {
        let config = create_test_config();
        let parser = NftClaimParser::new(&config).unwrap();

        let valid_event = NftClaimEventData {
            nft_mint: Pubkey::new_unique(),
            claimer: Pubkey::new_unique(),
            referrer: Some(Pubkey::new_unique()),
            tier: 3,
            tier_name: "Gold".to_string(),
            tier_bonus_rate: 1.5,
            claim_amount: 1000000,
            token_mint: Pubkey::new_unique(),
            reward_multiplier: 15000,
            reward_multiplier_percentage: 1.5,
            bonus_amount: 1500000,
            claim_type: 0,
            claim_type_name: "定期领取".to_string(),
            total_claimed: 5000000,
            claim_progress_percentage: 20.0,
            pool_address: Some(Pubkey::new_unique()),
            has_referrer: true,
            is_emergency_claim: false,
            estimated_usd_value: 0.0,
            claimed_at: chrono::Utc::now().timestamp(),
            signature: "test_sig".to_string(),
            slot: 12345,
            processed_at: chrono::Utc::now().to_rfc3339(),
        };

        assert!(parser.validate_nft_claim(&valid_event).unwrap());

        // 测试无效事件（等级为0）
        let invalid_event = NftClaimEventData {
            tier: 0, // 无效等级
            ..valid_event.clone()
        };

        assert!(!parser.validate_nft_claim(&invalid_event).unwrap());

        // 测试推荐人是自己的情况
        let self_referrer_event = NftClaimEventData {
            referrer: Some(valid_event.claimer), // 推荐人是自己
            ..valid_event.clone()
        };

        assert!(!parser.validate_nft_claim(&self_referrer_event).unwrap());
    }

    #[test]
    fn test_borsh_serialization() {
        let event = create_test_nft_claim_event();

        // 测试序列化
        let serialized = borsh::to_vec(&event).unwrap();
        assert!(!serialized.is_empty());

        // 测试反序列化
        let deserialized = NftClaimEvent::try_from_slice(&serialized).unwrap();
        assert_eq!(deserialized.nft_mint, event.nft_mint);
        assert_eq!(deserialized.claimer, event.claimer);
        assert_eq!(deserialized.claim_amount, event.claim_amount);
        assert_eq!(deserialized.tier, event.tier);
    }

    #[tokio::test]
    async fn test_parse_from_logs_no_program_data() {
        let config = create_test_config();
        let parser = NftClaimParser::new(&config).unwrap();

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
        let parser = NftClaimParser::new(&config).unwrap();

        let event = ParsedEvent::NftClaim(NftClaimEventData {
            nft_mint: Pubkey::new_unique(),
            claimer: Pubkey::new_unique(),
            referrer: Some(Pubkey::new_unique()),
            tier: 3,
            tier_name: "Gold".to_string(),
            tier_bonus_rate: 1.5,
            claim_amount: 1000000,
            token_mint: Pubkey::new_unique(),
            reward_multiplier: 15000,
            reward_multiplier_percentage: 1.5,
            bonus_amount: 1500000,
            claim_type: 0,
            claim_type_name: "定期领取".to_string(),
            total_claimed: 5000000,
            claim_progress_percentage: 20.0,
            pool_address: Some(Pubkey::new_unique()),
            has_referrer: true,
            is_emergency_claim: false,
            estimated_usd_value: 0.0,
            claimed_at: chrono::Utc::now().timestamp(),
            signature: "test_sig".to_string(),
            slot: 12345,
            processed_at: chrono::Utc::now().to_rfc3339(),
        });

        assert!(parser.validate_event(&event).await.unwrap());
    }
}
