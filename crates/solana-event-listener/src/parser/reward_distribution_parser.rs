use crate::{
    config::EventListenerConfig,
    error::{EventListenerError, Result},
    parser::{event_parser::RewardDistributionEventData, EventParser, ParsedEvent},
};
use async_trait::async_trait;
use borsh::{BorshDeserialize, BorshSerialize};
use solana_sdk::pubkey::Pubkey;
use tracing::{debug, info, warn};

/// 奖励发放事件的原始数据结构（与智能合约保持一致）
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct RewardDistributionEvent {
    /// 奖励分发ID（唯一标识符）
    pub distribution_id: u64,
    /// 奖励池地址
    pub reward_pool: String,
    /// 接收者钱包地址
    pub recipient: String,
    /// 推荐人地址（可选）
    pub referrer: Option<String>,
    /// 奖励代币mint地址
    pub reward_token_mint: String,
    /// 奖励数量（以最小单位计）
    pub reward_amount: u64,
    /// 奖励类型 (0: 交易奖励, 1: 推荐奖励, 2: 流动性奖励, 3: 治理奖励, 4: 空投奖励)
    pub reward_type: u8,
    /// 奖励来源 (0: DEX交易, 1: 流动性挖矿, 2: 推荐计划, 3: 治理投票, 4: 特殊活动)
    pub reward_source: u8,
    /// 相关的交易或池子地址（可选）
    pub related_address: Option<String>,
    /// 奖励倍率（基点，如10000表示1.0倍）
    pub multiplier: u16,
    /// 基础奖励金额（倍率计算前）
    pub base_reward_amount: u64,
    /// 是否已锁定（锁定期内不能提取）
    pub is_locked: bool,
    /// 锁定期结束时间戳（如果is_locked为true）
    pub unlock_timestamp: Option<i64>,
    /// 发放时间戳
    pub distributed_at: i64,
}

/// 奖励发放事件解析器
pub struct RewardDistributionParser {
    /// 事件的discriminator
    discriminator: [u8; 8],
}

impl RewardDistributionParser {
    /// 创建新的奖励发放事件解析器
    pub fn new(_config: &EventListenerConfig) -> Result<Self> {
        // 奖励发放事件的discriminator
        // 注意：实际部署时需要从智能合约IDL获取正确的discriminator
        let discriminator = [178, 95, 213, 88, 42, 167, 129, 77];

        Ok(Self { discriminator })
    }

    /// 从程序数据解析奖励发放事件
    fn parse_program_data(&self, data_str: &str) -> Result<RewardDistributionEvent> {
        use base64::{engine::general_purpose, Engine as _};

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
        let event = RewardDistributionEvent::try_from_slice(event_data).map_err(|e| EventListenerError::EventParsing(format!("Borsh反序列化失败: {}", e)))?;

        debug!(
            "✅ 成功解析奖励发放事件: ID={}, 接收者={}, 数量={}",
            event.distribution_id, event.recipient, event.reward_amount
        );
        Ok(event)
    }

    /// 获取奖励类型名称
    fn get_reward_type_name(&self, reward_type: u8) -> String {
        match reward_type {
            0 => "交易奖励".to_string(),
            1 => "推荐奖励".to_string(),
            2 => "流动性奖励".to_string(),
            3 => "治理奖励".to_string(),
            4 => "空投奖励".to_string(),
            _ => "未知奖励".to_string(),
        }
    }

    /// 获取奖励来源名称
    fn get_reward_source_name(&self, reward_source: u8) -> String {
        match reward_source {
            0 => "DEX交易".to_string(),
            1 => "流动性挖矿".to_string(),
            2 => "推荐计划".to_string(),
            3 => "治理投票".to_string(),
            4 => "特殊活动".to_string(),
            _ => "未知来源".to_string(),
        }
    }

    /// 计算奖励相关指标
    fn calculate_reward_metrics(&self, event: &RewardDistributionEvent) -> (f64, u64, u64, bool) {
        // 奖励倍率
        let multiplier_rate = event.multiplier as f64 / 10000.0;

        // 额外奖励金额（倍率产生的额外部分）
        let bonus_amount = if event.reward_amount > event.base_reward_amount {
            event.reward_amount - event.base_reward_amount
        } else {
            0
        };

        // 计算锁定期（天数）
        let lock_days = if event.is_locked && event.unlock_timestamp.is_some() {
            let unlock_time = event.unlock_timestamp.unwrap();
            let lock_duration = unlock_time - event.distributed_at;
            (lock_duration / 86400) as u64 // 转换为天数
        } else {
            0
        };

        // 是否为高价值奖励（大于等价1000 USDC）
        let is_high_value = event.reward_amount >= 1_000_000_000; // 假设6位小数的代币

        (multiplier_rate, bonus_amount, lock_days, is_high_value)
    }

    /// 将原始事件转换为ParsedEvent
    fn convert_to_parsed_event(&self, event: RewardDistributionEvent, signature: String, slot: u64) -> ParsedEvent {
        let (multiplier_percentage, bonus_amount, lock_days, is_high_value) = self.calculate_reward_metrics(&event);

        ParsedEvent::RewardDistribution(RewardDistributionEventData {
            distribution_id: event.distribution_id,
            reward_pool: event.reward_pool,
            recipient: event.recipient,
            referrer: event.referrer.clone(),
            reward_token_mint: event.reward_token_mint,
            reward_amount: event.reward_amount,
            base_reward_amount: event.base_reward_amount,
            bonus_amount,
            reward_type: event.reward_type,
            reward_type_name: self.get_reward_type_name(event.reward_type),
            reward_source: event.reward_source,
            reward_source_name: self.get_reward_source_name(event.reward_source),
            related_address: event.related_address,
            multiplier: event.multiplier,
            multiplier_percentage,
            is_locked: event.is_locked,
            unlock_timestamp: event.unlock_timestamp,
            lock_days,
            has_referrer: event.referrer.is_some(),
            is_referral_reward: event.reward_type == 1,
            is_high_value_reward: is_high_value,
            estimated_usd_value: 0.0, // 需要通过价格预言机获取
            distributed_at: event.distributed_at,
            signature,
            slot,
            processed_at: chrono::Utc::now().to_rfc3339(),
        })
    }

    /// 验证奖励发放事件数据
    fn validate_reward_distribution(&self, event: &RewardDistributionEventData) -> Result<bool> {
        // 验证分发ID
        if event.distribution_id == 0 {
            warn!("❌ 分发ID不能为0");
            return Ok(false);
        }

        // 验证奖励池地址
        if event.reward_pool == Pubkey::default().to_string() {
            warn!("❌ 无效的奖励池地址");
            return Ok(false);
        }

        // 验证接收者地址
        if event.recipient == Pubkey::default().to_string() {
            warn!("❌ 无效的接收者地址");
            return Ok(false);
        }

        // 验证奖励代币地址
        if event.reward_token_mint == Pubkey::default().to_string() {
            warn!("❌ 无效的奖励代币地址");
            return Ok(false);
        }

        // 验证奖励数量
        if event.reward_amount == 0 {
            warn!("❌ 奖励数量不能为0");
            return Ok(false);
        }

        // 验证基础奖励数量
        if event.base_reward_amount == 0 {
            warn!("❌ 基础奖励数量不能为0");
            return Ok(false);
        }

        // 验证奖励数量与基础数量的关系
        if event.reward_amount < event.base_reward_amount {
            warn!("❌ 奖励数量不能小于基础奖励数量: reward={}, base={}", event.reward_amount, event.base_reward_amount);
            return Ok(false);
        }

        // 验证奖励类型
        if event.reward_type > 4 {
            warn!("❌ 无效的奖励类型: {}", event.reward_type);
            return Ok(false);
        }

        // 验证奖励来源
        if event.reward_source > 4 {
            warn!("❌ 无效的奖励来源: {}", event.reward_source);
            return Ok(false);
        }

        // 验证倍率合理性 (0.1倍 - 6.5倍，因为u16最大值限制)
        if event.multiplier < 1000 {
            warn!("❌ 奖励倍率过低: {}", event.multiplier);
            return Ok(false);
        }

        // 验证锁定逻辑
        if event.is_locked && event.unlock_timestamp.is_none() {
            warn!("❌ 已锁定的奖励必须有解锁时间");
            return Ok(false);
        }

        // 验证解锁时间合理性
        if let Some(unlock_time) = event.unlock_timestamp {
            if unlock_time <= event.distributed_at {
                warn!("❌ 解锁时间不能早于或等于发放时间: unlock={}, distribute={}", unlock_time, event.distributed_at);
                return Ok(false);
            }

            // 验证锁定期不能超过2年
            let max_lock_duration = 2 * 365 * 24 * 3600; // 2年的秒数
            if unlock_time - event.distributed_at > max_lock_duration {
                warn!("❌ 锁定期不能超过2年: {} 秒", unlock_time - event.distributed_at);
                return Ok(false);
            }
        }

        // 验证时间戳合理性
        let now = chrono::Utc::now().timestamp();
        if event.distributed_at > now || event.distributed_at < (now - 86400) {
            warn!("❌ 发放时间戳异常: {}", event.distributed_at);
            return Ok(false);
        }

        // 验证推荐人不能是自己
        if let Some(referrer) = &event.referrer {
            if referrer == &event.recipient {
                warn!("❌ 推荐人不能是自己: {}", event.recipient);
                return Ok(false);
            }
        }

        // 验证推荐奖励的逻辑一致性
        if event.is_referral_reward && event.referrer.is_none() {
            warn!("❌ 推荐奖励必须有推荐人");
            return Ok(false);
        }

        // 验证奖励金额的合理性（防止天文数字）
        let max_reasonable_amount = 1_000_000_000_000_000_000u64; // 10^18
        if event.reward_amount > max_reasonable_amount {
            warn!("❌ 奖励数量过大，可能有错误: {}", event.reward_amount);
            return Ok(false);
        }

        Ok(true)
    }
}

#[async_trait]
impl EventParser for RewardDistributionParser {
    fn get_discriminator(&self) -> [u8; 8] {
        self.discriminator
    }

    fn get_event_type(&self) -> &'static str {
        "reward_distribution"
    }

    async fn parse_from_logs(&self, logs: &[String], signature: &str, slot: u64) -> Result<Option<ParsedEvent>> {
        for (index, log) in logs.iter().enumerate() {
            if log.starts_with("Program data: ") {
                if let Some(data_part) = log.strip_prefix("Program data: ") {
                    match self.parse_program_data(data_part) {
                        Ok(event) => {
                            info!(
                                "💰 第{}行发现奖励发放事件: ID={} 向 {} 发放 {} {} ({})",
                                index + 1,
                                event.distribution_id,
                                event.recipient,
                                event.reward_amount,
                                self.get_reward_type_name(event.reward_type),
                                if event.is_locked { "已锁定" } else { "可提取" }
                            );
                            let parsed_event = self.convert_to_parsed_event(event, signature.to_string(), slot);
                            return Ok(Some(parsed_event));
                        }
                        Err(EventListenerError::DiscriminatorMismatch) => {
                            // Discriminator不匹配是正常情况，继续尝试下一条日志
                            continue;
                        }
                        Err(e) => {
                            debug!("⚠️ 第{}行奖励发放事件解析失败: {}", index + 1, e);
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
            ParsedEvent::RewardDistribution(reward_event) => self.validate_reward_distribution(reward_event),
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

    fn create_test_reward_distribution_event() -> RewardDistributionEvent {
        let now = chrono::Utc::now().timestamp();
        RewardDistributionEvent {
            distribution_id: 12345,
            reward_pool: Pubkey::new_unique().to_string(),
            recipient: Pubkey::new_unique().to_string(),
            referrer: Some(Pubkey::new_unique().to_string()),
            reward_token_mint: Pubkey::new_unique().to_string(),
            reward_amount: 1500000, // 1.5 tokens with 6 decimals
            reward_type: 2,         // 流动性奖励
            reward_source: 1,       // 流动性挖矿
            related_address: Some(Pubkey::new_unique().to_string()),
            multiplier: 15000,           // 1.5倍
            base_reward_amount: 1000000, // 1 token基础奖励
            is_locked: true,
            unlock_timestamp: Some(now + 7 * 24 * 3600), // 7天后解锁
            distributed_at: now,
        }
    }

    #[test]
    fn test_reward_distribution_parser_creation() {
        let config = create_test_config();
        let parser = RewardDistributionParser::new(&config).unwrap();

        assert_eq!(parser.get_event_type(), "reward_distribution");
        assert_eq!(parser.get_discriminator(), [178, 95, 213, 88, 42, 167, 129, 77]);
    }

    #[test]
    fn test_reward_type_mapping() {
        let config = create_test_config();
        let parser = RewardDistributionParser::new(&config).unwrap();

        assert_eq!(parser.get_reward_type_name(0), "交易奖励");
        assert_eq!(parser.get_reward_type_name(1), "推荐奖励");
        assert_eq!(parser.get_reward_type_name(2), "流动性奖励");
        assert_eq!(parser.get_reward_type_name(3), "治理奖励");
        assert_eq!(parser.get_reward_type_name(4), "空投奖励");
        assert_eq!(parser.get_reward_type_name(99), "未知奖励");
    }

    #[test]
    fn test_reward_source_mapping() {
        let config = create_test_config();
        let parser = RewardDistributionParser::new(&config).unwrap();

        assert_eq!(parser.get_reward_source_name(0), "DEX交易");
        assert_eq!(parser.get_reward_source_name(1), "流动性挖矿");
        assert_eq!(parser.get_reward_source_name(2), "推荐计划");
        assert_eq!(parser.get_reward_source_name(3), "治理投票");
        assert_eq!(parser.get_reward_source_name(4), "特殊活动");
        assert_eq!(parser.get_reward_source_name(99), "未知来源");
    }

    #[test]
    fn test_convert_to_parsed_event() {
        let config = create_test_config();
        let parser = RewardDistributionParser::new(&config).unwrap();
        let test_event = create_test_reward_distribution_event();

        let parsed = parser.convert_to_parsed_event(test_event.clone(), "test_signature".to_string(), 12345);

        match parsed {
            ParsedEvent::RewardDistribution(data) => {
                assert_eq!(data.distribution_id, test_event.distribution_id);
                assert_eq!(data.recipient, test_event.recipient);
                assert_eq!(data.reward_amount, test_event.reward_amount);
                assert_eq!(data.base_reward_amount, test_event.base_reward_amount);
                assert_eq!(data.bonus_amount, 500000); // 1500000 - 1000000
                assert_eq!(data.reward_type_name, "流动性奖励");
                assert_eq!(data.reward_source_name, "流动性挖矿");
                assert_eq!(data.multiplier_percentage, 1.5);
                assert_eq!(data.is_locked, true);
                assert_eq!(data.lock_days, 7);
                assert_eq!(data.has_referrer, true);
                assert_eq!(data.is_referral_reward, false);
                assert_eq!(data.signature, "test_signature");
                assert_eq!(data.slot, 12345);
            }
            _ => panic!("期望RewardDistribution事件"),
        }
    }

    #[test]
    fn test_calculate_reward_metrics() {
        let config = create_test_config();
        let parser = RewardDistributionParser::new(&config).unwrap();

        let event = RewardDistributionEvent {
            reward_amount: 1500000,
            base_reward_amount: 1000000,
            multiplier: 15000, // 1.5x
            is_locked: true,
            unlock_timestamp: Some(chrono::Utc::now().timestamp() + 7 * 24 * 3600),
            distributed_at: chrono::Utc::now().timestamp(),
            ..create_test_reward_distribution_event()
        };

        let (multiplier_rate, bonus_amount, lock_days, is_high_value) = parser.calculate_reward_metrics(&event);

        assert_eq!(multiplier_rate, 1.5);
        assert_eq!(bonus_amount, 500000); // 1500000 - 1000000
        assert_eq!(lock_days, 7);
        assert_eq!(is_high_value, false); // 小于1000 USDC等值
    }

    #[tokio::test]
    async fn test_validate_reward_distribution() {
        let config = create_test_config();
        let parser = RewardDistributionParser::new(&config).unwrap();

        let valid_event = RewardDistributionEventData {
            distribution_id: 12345,
            reward_pool: Pubkey::new_unique().to_string(),
            recipient: Pubkey::new_unique().to_string(),
            referrer: Some(Pubkey::new_unique().to_string()),
            reward_token_mint: Pubkey::new_unique().to_string(),
            reward_amount: 1500000,
            base_reward_amount: 1000000,
            bonus_amount: 500000,
            reward_type: 2,
            reward_type_name: "流动性奖励".to_string(),
            reward_source: 1,
            reward_source_name: "流动性挖矿".to_string(),
            related_address: Some(Pubkey::new_unique().to_string()),
            multiplier: 15000,
            multiplier_percentage: 1.5,
            is_locked: true,
            unlock_timestamp: Some(chrono::Utc::now().timestamp() + 7 * 24 * 3600),
            lock_days: 7,
            has_referrer: true,
            is_referral_reward: false,
            is_high_value_reward: false,
            estimated_usd_value: 0.0,
            distributed_at: chrono::Utc::now().timestamp(),
            signature: "test_sig".to_string(),
            slot: 12345,
            processed_at: chrono::Utc::now().to_rfc3339(),
        };

        assert!(parser.validate_reward_distribution(&valid_event).unwrap());

        // 测试无效事件（分发ID为0）
        let invalid_event = RewardDistributionEventData {
            distribution_id: 0, // 无效ID
            ..valid_event.clone()
        };

        assert!(!parser.validate_reward_distribution(&invalid_event).unwrap());

        // 测试推荐人是自己的情况
        let self_referrer_event = RewardDistributionEventData {
            referrer: Some(valid_event.recipient.clone()), // 推荐人是自己
            ..valid_event.clone()
        };

        assert!(!parser.validate_reward_distribution(&self_referrer_event).unwrap());

        // 测试锁定但没有解锁时间的情况
        let locked_no_unlock_event = RewardDistributionEventData {
            is_locked: true,
            unlock_timestamp: None, // 没有解锁时间
            ..valid_event.clone()
        };

        assert!(!parser.validate_reward_distribution(&locked_no_unlock_event).unwrap());
    }

    #[test]
    fn test_borsh_serialization() {
        let event = create_test_reward_distribution_event();

        // 测试序列化
        let serialized = borsh::to_vec(&event).unwrap();
        assert!(!serialized.is_empty());

        // 测试反序列化
        let deserialized = RewardDistributionEvent::try_from_slice(&serialized).unwrap();
        assert_eq!(deserialized.distribution_id, event.distribution_id);
        assert_eq!(deserialized.recipient, event.recipient);
        assert_eq!(deserialized.reward_amount, event.reward_amount);
        assert_eq!(deserialized.reward_type, event.reward_type);
    }

    #[tokio::test]
    async fn test_parse_from_logs_no_program_data() {
        let config = create_test_config();
        let parser = RewardDistributionParser::new(&config).unwrap();

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
        let parser = RewardDistributionParser::new(&config).unwrap();

        let event = ParsedEvent::RewardDistribution(RewardDistributionEventData {
            distribution_id: 12345,
            reward_pool: Pubkey::new_unique().to_string(),
            recipient: Pubkey::new_unique().to_string(),
            referrer: Some(Pubkey::new_unique().to_string()),
            reward_token_mint: Pubkey::new_unique().to_string(),
            reward_amount: 1500000,
            base_reward_amount: 1000000,
            bonus_amount: 500000,
            reward_type: 2,
            reward_type_name: "流动性奖励".to_string(),
            reward_source: 1,
            reward_source_name: "流动性挖矿".to_string(),
            related_address: Some(Pubkey::new_unique().to_string()),
            multiplier: 15000,
            multiplier_percentage: 1.5,
            is_locked: true,
            unlock_timestamp: Some(chrono::Utc::now().timestamp() + 7 * 24 * 3600),
            lock_days: 7,
            has_referrer: true,
            is_referral_reward: false,
            is_high_value_reward: false,
            estimated_usd_value: 0.0,
            distributed_at: chrono::Utc::now().timestamp(),
            signature: "test_sig".to_string(),
            slot: 12345,
            processed_at: chrono::Utc::now().to_rfc3339(),
        });

        assert!(parser.validate_event(&event).await.unwrap());
    }
}
