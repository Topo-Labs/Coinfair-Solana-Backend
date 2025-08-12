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

#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct ClaimNFTEvent {
    pub claimer: Pubkey,          // 领取者地址
    pub upper: Pubkey,            // 上级地址
    pub nft_mint: Pubkey,         // NFT mint 地址
    pub claim_fee: u64,           // 支付的领取费用
    pub upper_remain_mint: u64,   // 上级剩余可被领取的NFT数量
    pub protocol_wallet: Pubkey,  // 协议费用接收钱包
    pub nft_pool_account: Pubkey, // NFT池子账户
    pub user_ata: Pubkey,         // 用户接收NFT的ATA账户
    pub timestamp: i64,           // 领取时间戳
}

/// NFT领取事件解析器
pub struct NftClaimParser {
    /// 事件的discriminator
    discriminator: [u8; 8],
    /// 目标程序ID，指定此解析器处理哪个程序的事件
    target_program_id: Pubkey,
}

impl NftClaimParser {
    /// 创建新的NFT领取事件解析器
    pub fn new(_config: &EventListenerConfig, program_id: Pubkey) -> Result<Self> {
        // NFT领取事件的discriminator
        // let discriminator = [92, 29, 201, 154, 132, 203, 150, 105];
        let discriminator = [0, 164, 135, 76, 199, 190, 102, 78];

        Ok(Self {
            discriminator,
            target_program_id: program_id,
        })
    }

    /// 从程序数据解析NFT领取事件
    fn parse_program_data(&self, data_str: &str) -> Result<ClaimNFTEvent> {
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
        let event = ClaimNFTEvent::try_from_slice(event_data).map_err(|e| EventListenerError::EventParsing(format!("Borsh反序列化失败: {}", e)))?;

        debug!(
            "✅ 成功解析NFT领取事件: NFT={}, 领取者={}, 领取费用={}",
            event.nft_mint, event.claimer, event.claim_fee
        );
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

    /// 根据领取费用估算NFT等级
    fn estimate_tier_from_fee(&self, claim_fee: u64) -> u8 {
        // 根据实际业务逻辑调整费用阈值
        match claim_fee {
            0..=50000 => 1,       // Bronze: 0-0.05 SOL
            50001..=100000 => 2,  // Silver: 0.05-0.1 SOL
            100001..=200000 => 3, // Gold: 0.1-0.2 SOL
            200001..=500000 => 4, // Platinum: 0.2-0.5 SOL
            _ => 5,               // Diamond: >0.5 SOL
        }
    }

    /// 根据NFT等级计算奖励倍率
    fn calculate_multiplier_from_tier(&self, tier: u8) -> u16 {
        // 返回基点（10000 = 1.0倍）
        match tier {
            1 => 10000, // 1.0倍
            2 => 12000, // 1.2倍
            3 => 15000, // 1.5倍
            4 => 20000, // 2.0倍
            5 => 30000, // 3.0倍
            _ => 10000, // 默认1.0倍
        }
    }

    /// 计算领取进度百分比
    fn calculate_claim_progress(&self, event: &ClaimNFTEvent) -> f64 {
        // 基于上级剩余mint数量估算进度
        // 假设初始总量为1000（根据实际业务调整）
        let assumed_initial_total = 1000.0;
        let remaining = event.upper_remain_mint as f64;
        let claimed = assumed_initial_total - remaining;

        if assumed_initial_total > 0.0 {
            (claimed / assumed_initial_total * 100.0).min(100.0).max(0.0)
        } else {
            0.0
        }
    }

    /// 估算USD价值
    fn estimate_usd_value(&self, claim_amount: u64) -> f64 {
        // 假设SOL价格为$100（实际应该从价格API获取）
        let sol_price_usd = 100.0;
        let sol_amount = claim_amount as f64 / 1_000_000_000.0; // lamports转SOL
        sol_amount * sol_price_usd
    }

    /// 将原始事件转换为ParsedEvent
    fn convert_to_parsed_event(&self, event: ClaimNFTEvent, signature: String, slot: u64) -> ParsedEvent {
        // 根据claim_fee推算NFT等级 (简化逻辑，可根据实际业务调整)
        let tier = self.estimate_tier_from_fee(event.claim_fee);
        let tier_bonus_rate = self.calculate_tier_bonus(tier);

        // 基于实际费用计算相关指标
        let claim_amount = event.claim_fee; // 使用实际支付的费用作为领取数量
        let reward_multiplier = self.calculate_multiplier_from_tier(tier);
        let bonus_amount = (claim_amount as f64 * tier_bonus_rate) as u64;
        let claim_progress = self.calculate_claim_progress(&event);

        ParsedEvent::NftClaim(NftClaimEventData {
            nft_mint: event.nft_mint.to_string(),
            claimer: event.claimer.to_string(),
            referrer: Some(event.upper.to_string()),
            tier,
            tier_name: self.get_tier_name(tier),
            tier_bonus_rate,
            claim_amount,
            token_mint: "So11111111111111111111111111111111111111112".to_string(), // SOL mint地址
            reward_multiplier,
            reward_multiplier_percentage: reward_multiplier as f64 / 10000.0,
            bonus_amount,
            claim_type: 1, // 固定为一次性领取
            claim_type_name: self.get_claim_type_name(1),
            total_claimed: claim_amount, // 当前就是总的领取量
            claim_progress_percentage: claim_progress,
            pool_address: Some(event.nft_pool_account.to_string()),
            has_referrer: true,        // 新结构总是有upper字段
            is_emergency_claim: false, // 根据业务逻辑，一般NFT领取不是紧急领取
            estimated_usd_value: self.estimate_usd_value(claim_amount),
            claimed_at: event.timestamp,
            signature,
            slot,
            processed_at: chrono::Utc::now().to_rfc3339(),
        })
    }

    /// 验证NFT领取事件数据
    fn validate_nft_claim(&self, event: &NftClaimEventData) -> Result<bool> {
        // 验证NFT地址
        if event.nft_mint == String::default() {
            warn!("❌ 无效的NFT地址");
            return Ok(false);
        }

        // 验证领取者地址
        if event.claimer == String::default() {
            warn!("❌ 无效的领取者地址");
            return Ok(false);
        }

        // 验证代币地址
        if event.token_mint == String::default() {
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
            warn!(
                "❌ 累计领取量不能小于本次领取量: total={}, current={}",
                event.total_claimed, event.claim_amount
            );
            return Ok(false);
        }

        // 验证时间戳合理性
        let now = chrono::Utc::now().timestamp();
        if event.claimed_at > now || event.claimed_at < (now - 86400) {
            warn!("❌ 领取时间戳异常: {}", event.claimed_at);
            return Ok(false);
        }

        // 验证推荐人不能是自己
        if let Some(referrer) = &event.referrer {
            if referrer == &event.claimer {
                warn!("❌ 推荐人不能是自己: {}", event.claimer);
                return Ok(false);
            }
        }

        // 验证奖励金额的合理性
        if event.bonus_amount > event.claim_amount * 10 {
            warn!(
                "❌ 奖励金额过大，可能有计算错误: bonus={}, base={}",
                event.bonus_amount, event.claim_amount
            );
            return Ok(false);
        }

        Ok(true)
    }
}

#[async_trait]
impl EventParser for NftClaimParser {
    fn get_program_id(&self) -> Pubkey {
        self.target_program_id
    }

    fn get_discriminator(&self) -> [u8; 8] {
        self.discriminator
    }

    fn get_event_type(&self) -> &'static str {
        "nft_claim"
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
                                "🎁 第{}行发现NFT领取事件: {} 推荐人 {} (nft mint: {} 领取费用: {})",
                                index + 1,
                                event.claimer,
                                event.upper,
                                event.nft_mint,
                                event.claim_fee
                            );
                            let parsed_event = self.convert_to_parsed_event(event, signature.to_string(), slot);
                            return Ok(Some(parsed_event));
                        }
                        Err(EventListenerError::DiscriminatorMismatch) => {
                            // Discriminator不匹配是正常情况，继续尝试下一条日志
                            continue;
                        }
                        Err(e) => {
                            warn!("⚠️ 第{}行NFT领取事件解析失败: {}", index + 1, e);
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
    use anchor_lang::pubkey;
    use solana_sdk::pubkey::Pubkey;

    fn create_test_config() -> EventListenerConfig {
        EventListenerConfig {
            solana: crate::config::settings::SolanaConfig {
                rpc_url: "https://api.devnet.solana.com".to_string(),
                ws_url: "wss://api.devnet.solana.com".to_string(),
                commitment: "confirmed".to_string(),
                program_ids: vec![pubkey!("FA1RJDDXysgwg5Gm3fJXWxt26JQzPkAzhTA114miqNUX")],
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

    fn create_test_nft_claim_event() -> ClaimNFTEvent {
        ClaimNFTEvent {
            nft_mint: Pubkey::new_unique(),
            claimer: Pubkey::new_unique(),
            upper: Pubkey::new_unique(),
            claim_fee: 100,
            upper_remain_mint: 100,
            protocol_wallet: Pubkey::new_unique(),
            nft_pool_account: Pubkey::new_unique(),
            user_ata: Pubkey::new_unique(),
            timestamp: chrono::Utc::now().timestamp(),
        }
    }

    #[test]
    fn test_nft_claim_parser_creation() {
        let config = create_test_config();
        let parser = NftClaimParser::new(&config, Pubkey::new_unique()).unwrap();

        assert_eq!(parser.get_event_type(), "nft_claim");
        assert_eq!(parser.get_discriminator(), [0, 164, 135, 76, 199, 190, 102, 78]);
    }

    #[test]
    fn test_tier_bonus_calculation() {
        let config = create_test_config();
        let parser = NftClaimParser::new(&config, Pubkey::new_unique()).unwrap();

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
        let parser = NftClaimParser::new(&config, Pubkey::new_unique()).unwrap();

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
        let parser = NftClaimParser::new(&config, Pubkey::new_unique()).unwrap();
        let test_event = create_test_nft_claim_event();

        let parsed = parser.convert_to_parsed_event(test_event.clone(), "test_signature".to_string(), 12345);

        match parsed {
            ParsedEvent::NftClaim(data) => {
                assert_eq!(data.nft_mint, test_event.nft_mint.to_string());
                assert_eq!(data.claimer, test_event.claimer.to_string());
                assert_eq!(data.referrer, Some(test_event.upper.to_string()));
                assert_eq!(data.tier, 1); // claim_fee=100对应Bronze等级
                assert_eq!(data.tier_name, "Bronze");
                assert_eq!(data.tier_bonus_rate, 1.0);
                assert_eq!(data.reward_multiplier, 10000); // 1.0倍 = 10000基点
                assert_eq!(data.reward_multiplier_percentage, 1.0);
                assert_eq!(data.claim_amount, test_event.claim_fee);
                assert_eq!(data.total_claimed, test_event.claim_fee);
                assert_eq!(data.has_referrer, true);
                assert_eq!(data.is_emergency_claim, false);
                assert_eq!(data.pool_address, Some(test_event.nft_pool_account.to_string()));
                assert_eq!(data.signature, "test_signature");
                assert_eq!(data.slot, 12345);
                assert_eq!(data.claimed_at, test_event.timestamp);
            }
            _ => panic!("期望NftClaim事件"),
        }
    }

    #[test]
    fn test_estimate_tier_from_fee() {
        let config = create_test_config();
        let parser = NftClaimParser::new(&config, Pubkey::new_unique()).unwrap();

        // 测试不同费用对应的等级
        assert_eq!(parser.estimate_tier_from_fee(1000), 1); // Bronze
        assert_eq!(parser.estimate_tier_from_fee(75000), 2); // Silver
        assert_eq!(parser.estimate_tier_from_fee(150000), 3); // Gold
        assert_eq!(parser.estimate_tier_from_fee(300000), 4); // Platinum
        assert_eq!(parser.estimate_tier_from_fee(600000), 5); // Diamond
    }

    #[test]
    fn test_calculate_multiplier_from_tier() {
        let config = create_test_config();
        let parser = NftClaimParser::new(&config, Pubkey::new_unique()).unwrap();

        // 测试等级对应的倍率
        assert_eq!(parser.calculate_multiplier_from_tier(1), 10000); // 1.0倍
        assert_eq!(parser.calculate_multiplier_from_tier(2), 12000); // 1.2倍
        assert_eq!(parser.calculate_multiplier_from_tier(3), 15000); // 1.5倍
        assert_eq!(parser.calculate_multiplier_from_tier(4), 20000); // 2.0倍
        assert_eq!(parser.calculate_multiplier_from_tier(5), 30000); // 3.0倍
    }

    #[test]
    fn test_calculate_claim_progress() {
        let config = create_test_config();
        let parser = NftClaimParser::new(&config, Pubkey::new_unique()).unwrap();

        let mut event = create_test_nft_claim_event();
        event.upper_remain_mint = 200; // 剩余200个

        let progress = parser.calculate_claim_progress(&event);
        assert_eq!(progress, 80.0); // (1000-200)/1000 * 100 = 80%
    }

    #[test]
    fn test_estimate_usd_value() {
        let config = create_test_config();
        let parser = NftClaimParser::new(&config, Pubkey::new_unique()).unwrap();

        // 1 SOL = 1,000,000,000 lamports
        let one_sol_lamports = 1_000_000_000;
        let usd_value = parser.estimate_usd_value(one_sol_lamports);
        assert_eq!(usd_value, 100.0); // 假设SOL价格$100

        let half_sol_lamports = 500_000_000;
        let half_sol_usd = parser.estimate_usd_value(half_sol_lamports);
        assert_eq!(half_sol_usd, 50.0);
    }

    #[tokio::test]
    async fn test_validate_nft_claim() {
        let config = create_test_config();
        let parser = NftClaimParser::new(&config, Pubkey::new_unique()).unwrap();

        let valid_event = NftClaimEventData {
            nft_mint: Pubkey::new_unique().to_string(),
            claimer: Pubkey::new_unique().to_string(),
            referrer: Some(Pubkey::new_unique().to_string()),
            tier: 3,
            tier_name: "Gold".to_string(),
            tier_bonus_rate: 1.5,
            claim_amount: 1000000,
            token_mint: Pubkey::new_unique().to_string(),
            reward_multiplier: 15000,
            reward_multiplier_percentage: 1.5,
            bonus_amount: 1500000,
            claim_type: 0,
            claim_type_name: "定期领取".to_string(),
            total_claimed: 5000000,
            claim_progress_percentage: 20.0,
            pool_address: Some(Pubkey::new_unique().to_string()),
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
            referrer: Some(valid_event.claimer.clone()), // 推荐人是自己
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
        let deserialized = ClaimNFTEvent::try_from_slice(&serialized).unwrap();
        assert_eq!(deserialized.nft_mint, event.nft_mint);
        assert_eq!(deserialized.claimer, event.claimer);
    }

    #[tokio::test]
    async fn test_parse_from_logs_no_program_data() {
        let config = create_test_config();
        let parser = NftClaimParser::new(&config, Pubkey::new_unique()).unwrap();

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
        let parser = NftClaimParser::new(&config, Pubkey::new_unique()).unwrap();

        let event = ParsedEvent::NftClaim(NftClaimEventData {
            nft_mint: Pubkey::new_unique().to_string(),
            claimer: Pubkey::new_unique().to_string(),
            referrer: Some(Pubkey::new_unique().to_string()),
            tier: 3,
            tier_name: "Gold".to_string(),
            tier_bonus_rate: 1.5,
            claim_amount: 1000000,
            token_mint: Pubkey::new_unique().to_string(),
            reward_multiplier: 15000,
            reward_multiplier_percentage: 1.5,
            bonus_amount: 1500000,
            claim_type: 0,
            claim_type_name: "定期领取".to_string(),
            total_claimed: 5000000,
            claim_progress_percentage: 20.0,
            pool_address: Some(Pubkey::new_unique().to_string()),
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
