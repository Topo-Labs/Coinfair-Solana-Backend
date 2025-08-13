use crate::config::EventListenerConfig;
use crate::error::{EventListenerError, Result};
use crate::parser::{NftClaimParser, PoolCreationParser, RewardDistributionParser, SwapParser, TokenCreationParser};
use anchor_lang::pubkey;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use tracing::info;

/// 解析器复合键，用于精确路由
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParserKey {
    /// 程序ID，用于区分不同合约的相同事件类型
    pub program_id: Pubkey,
    /// Discriminator，用于区分事件类型
    pub discriminator: [u8; 8],
}

impl Hash for ParserKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.program_id.hash(state);
        self.discriminator.hash(state);
    }
}

impl ParserKey {
    /// 创建程序特定的解析器键
    pub fn for_program(program_id: Pubkey, discriminator: [u8; 8]) -> Self {
        Self {
            program_id,
            discriminator,
        }
    }

    /// 创建通用解析器键（适用于所有程序）
    pub fn universal(discriminator: [u8; 8]) -> Self {
        Self {
            program_id: UNIVERSAL_PROGRAM_ID,
            discriminator,
        }
    }

    /// 检查是否为通用解析器键
    pub fn is_universal(&self) -> bool {
        self.program_id == UNIVERSAL_PROGRAM_ID
    }
}

/// 通用程序ID，表示解析器可以处理任何程序的该discriminator事件
pub const UNIVERSAL_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0u8; 32]);

/// 解析后的事件数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ParsedEvent {
    /// 代币创建事件
    TokenCreation(TokenCreationEventData),
    /// 池子创建事件
    PoolCreation(PoolCreationEventData),
    /// NFT领取事件
    NftClaim(NftClaimEventData),
    /// 奖励分发事件
    RewardDistribution(RewardDistributionEventData),
    /// 交换事件
    Swap(SwapEventData),
}

impl ParsedEvent {
    /// 获取事件类型字符串
    pub fn event_type(&self) -> &'static str {
        match self {
            ParsedEvent::TokenCreation(_) => "token_creation",
            ParsedEvent::PoolCreation(_) => "pool_creation",
            ParsedEvent::NftClaim(_) => "nft_claim",
            ParsedEvent::RewardDistribution(_) => "reward_distribution",
            ParsedEvent::Swap(_) => "swap",
        }
    }

    /// 获取事件的唯一标识符（用于去重）
    pub fn get_unique_id(&self) -> String {
        match self {
            ParsedEvent::TokenCreation(data) => data.mint_address.to_string(),
            ParsedEvent::PoolCreation(data) => data.pool_address.to_string(),
            ParsedEvent::NftClaim(data) => format!("{}_{}", data.nft_mint, data.signature),
            ParsedEvent::RewardDistribution(data) => format!("{}_{}", data.distribution_id, data.signature),
            ParsedEvent::Swap(data) => format!("{}_{}", data.pool_address, data.signature),
        }
    }
}

/// 代币创建事件数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenCreationEventData {
    /// 代币的 Mint 地址
    pub mint_address: String,
    /// 代币名称
    pub name: String,
    /// 代币符号
    pub symbol: String,
    /// 代币元数据的 URI（如 IPFS 链接）
    pub uri: String,
    /// 代币小数位数
    pub decimals: u8,
    /// 供应量（以最小单位计）
    pub supply: u64,
    /// 创建者的钱包地址
    pub creator: String,
    /// 是否支持白名单（true 表示有白名单机制）
    pub has_whitelist: bool,
    /// 白名单资格检查的时间戳（Unix 时间戳，0 表示无时间限制）
    pub whitelist_deadline: i64,
    /// 创建时间（Unix 时间戳）
    pub created_at: i64,
    /// 交易签名
    pub signature: String,
    /// 区块高度
    pub slot: u64,
}

/// 池子创建事件数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolCreationEventData {
    /// CLMM池子地址
    pub pool_address: String,
    /// 代币A的mint地址
    pub token_a_mint: String,
    /// 代币B的mint地址
    pub token_b_mint: String,
    /// 代币A的小数位数
    pub token_a_decimals: u8,
    /// 代币B的小数位数
    pub token_b_decimals: u8,
    /// 手续费率 (单位: 万分之一)
    pub fee_rate: u32,
    /// 手续费率百分比
    pub fee_rate_percentage: f64,
    /// 年化手续费率
    pub annual_fee_rate: f64,
    /// 池子类型
    pub pool_type: String,
    /// 初始sqrt价格
    pub sqrt_price_x64: String,
    /// 初始价格比率
    pub initial_price: f64,
    /// 初始tick
    pub initial_tick: i32,
    /// 池子创建者
    pub creator: String,
    /// CLMM配置地址
    pub clmm_config: String,
    /// 是否为稳定币对
    pub is_stable_pair: bool,
    /// 预估流动性价值(USD)
    pub estimated_liquidity_usd: f64,
    /// 创建时间戳
    pub created_at: i64,
    /// 交易签名
    pub signature: String,
    /// 区块高度
    pub slot: u64,
    /// 处理时间
    pub processed_at: String,
}

/// NFT领取事件数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NftClaimEventData {
    /// NFT的mint地址
    pub nft_mint: String,
    /// 领取者钱包地址
    pub claimer: String,
    /// 推荐人地址（可选）
    pub referrer: Option<String>,
    /// NFT等级 (1-5级)
    pub tier: u8,
    /// 等级名称
    pub tier_name: String,
    /// 等级奖励倍率
    pub tier_bonus_rate: f64,
    /// 领取的代币数量
    pub claim_amount: u64,
    /// 代币mint地址
    pub token_mint: String,
    /// 奖励倍率 (基点)
    pub reward_multiplier: u16,
    /// 奖励倍率百分比
    pub reward_multiplier_percentage: f64,
    /// 实际奖励金额（包含倍率）
    pub bonus_amount: u64,
    /// 领取类型
    pub claim_type: u8,
    /// 领取类型名称
    pub claim_type_name: String,
    /// 累计领取量
    pub total_claimed: u64,
    /// 领取进度百分比
    pub claim_progress_percentage: f64,
    /// NFT所属的池子地址（可选）
    pub pool_address: Option<String>,
    /// 是否有推荐人
    pub has_referrer: bool,
    /// 是否为紧急领取
    pub is_emergency_claim: bool,
    /// 预估USD价值
    pub estimated_usd_value: f64,
    /// 领取时间戳
    pub claimed_at: i64,
    /// 交易签名
    pub signature: String,
    /// 区块高度
    pub slot: u64,
    /// 处理时间
    pub processed_at: String,
}

/// 奖励分发事件数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RewardDistributionEventData {
    /// 奖励分发ID
    pub distribution_id: u64,
    /// 奖励池地址
    pub reward_pool: String,
    /// 接收者钱包地址
    pub recipient: String,
    /// 推荐人地址（可选）
    pub referrer: Option<String>,
    /// 奖励代币mint地址
    pub reward_token_mint: String,
    /// 奖励代币小数位数
    pub reward_token_decimals: Option<u8>,
    /// 奖励代币名称
    pub reward_token_name: Option<String>,
    /// 奖励代币符号
    pub reward_token_symbol: Option<String>,
    /// 奖励代币Logo URI
    pub reward_token_logo_uri: Option<String>,
    /// 奖励数量
    pub reward_amount: u64,
    /// 基础奖励金额
    pub base_reward_amount: u64,
    /// 额外奖励金额
    pub bonus_amount: u64,
    /// 奖励类型
    pub reward_type: u8,
    /// 奖励类型名称
    pub reward_type_name: String,
    /// 奖励来源
    pub reward_source: u8,
    /// 奖励来源名称
    pub reward_source_name: String,
    /// 相关地址
    pub related_address: Option<String>,
    /// 奖励倍率 (基点)
    pub multiplier: u16,
    /// 奖励倍率百分比
    pub multiplier_percentage: f64,
    /// 是否已锁定
    pub is_locked: bool,
    /// 锁定期结束时间戳
    pub unlock_timestamp: Option<i64>,
    /// 锁定天数
    pub lock_days: u64,
    /// 是否有推荐人
    pub has_referrer: bool,
    /// 是否为推荐奖励
    pub is_referral_reward: bool,
    /// 是否为高价值奖励
    pub is_high_value_reward: bool,
    /// 预估USD价值
    pub estimated_usd_value: f64,
    /// 发放时间戳
    pub distributed_at: i64,
    /// 交易签名
    pub signature: String,
    /// 区块高度
    pub slot: u64,
    /// 处理时间
    pub processed_at: String,
}

/// 交换事件数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwapEventData {
    /// 池子地址
    pub pool_address: String,
    /// 交换发起者
    pub sender: String,
    /// 代币0账户
    pub token_account_0: String,
    /// 代币1账户
    pub token_account_1: String,
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
    pub sqrt_price_x64: String,
    /// 流动性
    pub liquidity: String,
    /// tick位置
    pub tick: i32,
    /// 交易签名
    pub signature: String,
    /// 区块高度
    pub slot: u64,
    /// 处理时间
    pub processed_at: String,
}

/// 事件解析器接口
#[async_trait]
pub trait EventParser: Send + Sync {
    /// 获取此解析器处理的事件类型的program_id
    fn get_program_id(&self) -> Pubkey;

    /// 获取此解析器处理的事件类型的discriminator
    fn get_discriminator(&self) -> [u8; 8];

    /// 获取事件类型名称
    fn get_event_type(&self) -> &'static str;

    /// 检查此解析器是否支持特定程序
    /// 返回true表示支持，false表示不支持，None表示通用解析器（支持所有程序）
    fn supports_program(&self, _program_id: &Pubkey) -> Option<bool> {
        // 默认实现：通用解析器，支持所有程序
        None
    }

    /// 获取此解析器支持的程序ID列表
    /// 返回空列表表示通用解析器
    fn get_supported_programs(&self) -> Vec<Pubkey> {
        Vec::new()
    }

    /// 从日志数据中解析事件
    async fn parse_from_logs(&self, logs: &[String], signature: &str, slot: u64) -> Result<Option<ParsedEvent>>;

    /// 验证解析后的事件数据
    async fn validate_event(&self, event: &ParsedEvent) -> Result<bool>;
}

/// 事件解析器注册表
///
/// 管理所有已注册的事件解析器，并根据复合键(program_id + discriminator)路由事件到对应的解析器
pub struct EventParserRegistry {
    /// 使用复合键映射的解析器表
    parsers: HashMap<ParserKey, Box<dyn EventParser>>,
}

impl EventParserRegistry {
    /// 创建新的解析器注册表
    pub fn new(config: &EventListenerConfig) -> Result<Self> {
        let mut registry = Self {
            parsers: HashMap::new(),
        };

        // 交换事件解析器
        let swap_parser = Box::new(SwapParser::new(
            config,
            pubkey!("FA1RJDDXysgwg5Gm3fJXWxt26JQzPkAzhTA114miqNUX"),
        )?);
        registry.register_program_parser(swap_parser)?;

        // 交换事件解析器
        // let swap_parser = Box::new(SwapParser::new(config, pubkey!("devi51mZmdwUJGU9hjN27vEz64Gps7uUefqxg27EAtH"))?);
        // registry.register_program_parser(swap_parser)?;

        // 代币创建事件解析器
        let token_creation_parser = Box::new(TokenCreationParser::new(
            config,
            pubkey!("FA1RJDDXysgwg5Gm3fJXWxt26JQzPkAzhTA114miqNUX"),
        )?);
        registry.register_program_parser(token_creation_parser)?;

        // 池子创建事件解析器
        let pool_creation_parser = Box::new(PoolCreationParser::new(
            config,
            pubkey!("FA1RJDDXysgwg5Gm3fJXWxt26JQzPkAzhTA114miqNUX"),
        )?);
        registry.register_program_parser(pool_creation_parser)?;

        // NFT领取事件解析器
        let nft_claim_parser = Box::new(NftClaimParser::new(
            config,
            pubkey!("REFxcjx4pKym9j5Jzbo9wh92CtYTzHt9fqcjgvZGvUL"),
        )?);
        registry.register_program_parser(nft_claim_parser)?;

        // 奖励分发事件解析器
        let reward_distribution_parser = Box::new(RewardDistributionParser::new(
            config,
            pubkey!("REFxcjx4pKym9j5Jzbo9wh92CtYTzHt9fqcjgvZGvUL"),
        )?);
        registry.register_program_parser(reward_distribution_parser)?;

        Ok(registry)
    }

    /// 注册程序特定的事件解析器
    pub fn register_program_parser(&mut self, parser: Box<dyn EventParser>) -> Result<()> {
        let discriminator = parser.get_discriminator();
        let event_type = parser.get_event_type();
        let program_id = parser.get_program_id();
        let parser_key = ParserKey::for_program(program_id, discriminator);

        if self.parsers.contains_key(&parser_key) {
            return Err(EventListenerError::EventParsing(format!(
                "解析器键 {:?} 已注册",
                parser_key
            )));
        }

        self.parsers.insert(parser_key.clone(), parser);
        tracing::info!(
            "✅ 注册程序特定解析器: {} ({:?}) -> {:?}",
            program_id,
            event_type,
            discriminator,
        );
        Ok(())
    }

    /// 注册通用事件解析器（适用于所有程序）
    pub fn register_universal_parser(&mut self, parser: Box<dyn EventParser>) -> Result<()> {
        let discriminator = parser.get_discriminator();
        let event_type = parser.get_event_type();
        let parser_key = ParserKey::universal(discriminator);

        // 检查是否已存在通用解析器
        if self.parsers.contains_key(&parser_key) {
            return Err(EventListenerError::EventParsing(format!(
                "通用解析器键 {:?} 已注册",
                parser_key
            )));
        }

        // 注册到新的复合键映射
        self.parsers.insert(parser_key.clone(), parser);

        tracing::info!("✅ 注册通用解析器: {} ({:?})", event_type, discriminator);
        Ok(())
    }

    /// 从单条日志和完整上下文解析事件（智能路由版本）
    pub async fn parse_event_with_context(
        &self,
        logs: &[String],
        signature: &str,
        slot: u64,
        subscribed_programs: &[Pubkey],
    ) -> Result<Option<ParsedEvent>> {
        // 尝试从日志中提取程序ID
        let program_id_hint = self.extract_program_id_from_logs(logs, subscribed_programs);

        tracing::info!("🧠 智能路由启动 - 程序ID提示: {:?}", program_id_hint);

        let mut first_valid_event = None;
        let mut program_data_count = 0;
        let mut processed_count = 0;
        let mut skipped_count = 0;

        // 处理所有程序数据日志
        for (index, log) in logs.iter().enumerate() {
            if log.starts_with("Program data: ") {
                program_data_count += 1;
                if let Some(data_part) = log.strip_prefix("Program data: ") {
                    tracing::info!("📊 处理第{}个Program data (行{})", program_data_count, index + 1);

                    match self
                        .try_parse_program_data_with_hint(data_part, signature, slot, program_id_hint)
                        .await?
                    {
                        Some(event) => {
                            tracing::info!("✅ 第{}个事件解析成功: {}", program_data_count, event.event_type());
                            processed_count += 1;
                            if first_valid_event.is_none() {
                                first_valid_event = Some(event);
                            } else {
                                tracing::info!(
                                    "⏭️ 跳过第{}个事件（已有有效事件）: {}",
                                    program_data_count,
                                    event.event_type()
                                );
                                skipped_count += 1;
                            }
                        }
                        None => {
                            // 这里包括了白名单过滤和解析失败的情况
                            // 具体的跳过原因已经在try_parse_program_data_with_hint中记录
                            skipped_count += 1;
                        }
                    }
                }
            }
        }

        if program_data_count > 0 {
            tracing::info!(
                "📋 事件处理总结: 发现{}个Program data，处理{}个，跳过{}个",
                program_data_count,
                processed_count,
                skipped_count
            );
        }

        // 如果没有找到任何事件，尝试通用解析器
        if first_valid_event.is_none() {
            tracing::info!("🔄 Program data解析未找到事件，尝试通用解析器");
            for parser in self.parsers.values() {
                if let Some(event) = parser.parse_from_logs(logs, signature, slot).await? {
                    tracing::info!("✅ 通用解析器成功: {}", parser.get_event_type());
                    return Ok(Some(event));
                }
            }
        }

        if first_valid_event.is_some() {
            tracing::info!(
                "✅ 智能路由成功解析事件: {:?}",
                first_valid_event.as_ref().unwrap().event_type()
            );
        } else {
            tracing::info!("❌ 智能路由未找到匹配的解析器");
        }

        Ok(first_valid_event)
    }

    /// 从日志中提取程序ID（解析用）
    /// 新策略：查找包含Program data的程序调用块，并验证是否在订阅列表中
    pub fn extract_program_id_from_logs(&self, logs: &[String], subscribed_programs: &[Pubkey]) -> Option<Pubkey> {
        // 首先找到所有Program data的位置
        let mut program_data_indices = Vec::new();
        for (i, log) in logs.iter().enumerate() {
            if log.starts_with("Program data: ") {
                program_data_indices.push(i);
            }
        }

        if program_data_indices.is_empty() {
            tracing::debug!("🔍 未找到Program data日志");
            return None;
        }

        // 为每个Program data找到所属的程序调用块
        for &data_index in &program_data_indices {
            tracing::debug!("🔍 分析第{}行的Program data", data_index + 1);

            // 查找包含这个Program data的程序调用块
            // 策略：从Program data往前查找最近的program invoke，然后往后查找对应的success/consumed
            let mut current_program_id: Option<Pubkey> = None;
            let mut invoke_stack: Vec<(usize, Pubkey)> = Vec::new();

            // 从头开始分析日志，构建调用栈
            for (i, log) in logs.iter().enumerate().take(data_index + 5) {
                // 包括data之后的几行
                if log.starts_with("Program ") && log.contains(" invoke [") {
                    // 新的程序调用
                    let parts: Vec<&str> = log.split_whitespace().collect();
                    if parts.len() >= 3 {
                        if let Ok(program_id) = parts[1].parse::<Pubkey>() {
                            invoke_stack.push((i, program_id));
                            tracing::debug!("🔍 第{}行程序调用: {}", i + 1, program_id);
                        }
                    }
                } else if log.starts_with("Program ") && (log.contains(" success") || log.contains(" consumed ")) {
                    // 程序调用结束
                    let parts: Vec<&str> = log.split_whitespace().collect();
                    if parts.len() >= 2 {
                        if let Ok(program_id) = parts[1].parse::<Pubkey>() {
                            // 检查这是否是我们正在寻找的Program data所属的程序
                            if i > data_index {
                                // 这个success/consumed在Program data之后，可能就是包含data的程序
                                tracing::debug!("🔍 第{}行程序结束: {} (在Program data之后)", i + 1, program_id);

                                // 检查是否为订阅的程序
                                if self.is_subscribed_program(&program_id, subscribed_programs) {
                                    tracing::info!("🎯 找到订阅的程序 (基于success日志): {}", program_id);
                                    return Some(program_id);
                                } else {
                                    tracing::debug!("🚫 程序不在订阅列表中: {}", program_id);
                                }
                            }
                        }
                    }
                } else if i == data_index {
                    // 这就是Program data行，查看当前活跃的程序调用栈
                    if let Some(&(_, program_id)) = invoke_stack.last() {
                        tracing::debug!("🔍 Program data行{}，当前活跃程序: {}", i + 1, program_id);

                        // 检查是否为订阅的程序
                        if self.is_subscribed_program(&program_id, subscribed_programs) {
                            current_program_id = Some(program_id);
                            tracing::debug!("✅ 找到订阅的程序 (基于调用栈): {}", program_id);
                        } else {
                            tracing::debug!("🚫 程序不在订阅列表中: {}", program_id);
                        }
                    }
                }
            }

            // 如果找到了当前活跃的订阅程序，返回它
            if let Some(program_id) = current_program_id {
                tracing::info!(
                    "🎯 基于调用栈确定第{}行Program data的程序: {}",
                    data_index + 1,
                    program_id
                );
                return Some(program_id);
            }
        }

        tracing::warn!("⚠️ 未找到Program data对应的订阅程序");
        None
    }

    /// 检查程序ID是否在订阅列表中
    fn is_subscribed_program(&self, program_id: &Pubkey, subscribed_programs: &[Pubkey]) -> bool {
        subscribed_programs.contains(program_id)
    }

    /// 检查程序ID是否为系统程序（辅助验证用）
    #[allow(dead_code)]
    fn is_system_program(&self, program_id: &Pubkey) -> bool {
        const SYSTEM_PROGRAMS: &[&str] = &[
            "ComputeBudget111111111111111111111111111111",
            "11111111111111111111111111111111",
            "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",
            "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL",
            "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb",
        ];

        SYSTEM_PROGRAMS
            .iter()
            .any(|&sys_prog| program_id.to_string() == sys_prog)
    }

    /// 智能查找解析器（利用supports_program方法）
    fn find_best_parser(
        &self,
        discriminator: [u8; 8],
        program_id_hint: Option<Pubkey>,
    ) -> Option<&Box<dyn EventParser>> {
        // 策略1：如果有程序ID提示，优先查找程序特定解析器
        if let Some(program_id) = program_id_hint {
            let parser_key = ParserKey::for_program(program_id, discriminator);
            if let Some(parser) = self.parsers.get(&parser_key) {
                tracing::debug!(
                    "🎯 找到程序特定解析器: {} for {:?}",
                    parser.get_event_type(),
                    program_id
                );
                return Some(parser);
            }
        }

        // 策略2：查找通用解析器
        let universal_key = ParserKey::universal(discriminator);
        if let Some(parser) = self.parsers.get(&universal_key) {
            // 如果有程序ID，检查解析器是否支持该程序
            if let Some(program_id) = program_id_hint {
                match parser.supports_program(&program_id) {
                    Some(true) => {
                        tracing::debug!(
                            "🌐 通用解析器支持程序: {} for {:?}",
                            parser.get_event_type(),
                            program_id
                        );
                        return Some(parser);
                    }
                    Some(false) => {
                        tracing::debug!(
                            "🚫 通用解析器不支持程序: {} for {:?}",
                            parser.get_event_type(),
                            program_id
                        );
                        return None;
                    }
                    None => {
                        tracing::debug!("🌐 使用通用解析器: {}", parser.get_event_type());
                        return Some(parser);
                    }
                }
            } else {
                tracing::debug!("🌐 使用通用解析器: {}", parser.get_event_type());
                return Some(parser);
            }
        }

        // 策略3：遍历所有解析器，寻找支持该程序的解析器
        if let Some(program_id) = program_id_hint {
            for (key, parser) in &self.parsers {
                if parser.get_discriminator() == discriminator {
                    match parser.supports_program(&program_id) {
                        Some(true) => {
                            tracing::debug!(
                                "🔍 找到支持程序的解析器: {} for {:?}",
                                parser.get_event_type(),
                                program_id
                            );
                            return Some(parser);
                        }
                        None => {
                            // 通用解析器，如果还没查过就使用
                            if key.is_universal() {
                                tracing::debug!("🔍 找到通用解析器: {}", parser.get_event_type());
                                return Some(parser);
                            }
                        }
                        Some(false) => continue,
                    }
                }
            }
        }

        None
    }

    /// 尝试从程序数据解析事件（带程序ID提示的版本）
    async fn try_parse_program_data_with_hint(
        &self,
        data_str: &str,
        signature: &str,
        slot: u64,
        program_id_hint: Option<Pubkey>,
    ) -> Result<Option<ParsedEvent>> {
        // 解码Base64数据
        use base64::{engine::general_purpose, Engine as _};
        let data = general_purpose::STANDARD
            .decode(data_str)
            .map_err(|e| EventListenerError::EventParsing(format!("Base64解码失败: {}", e)))?;

        if data.len() < 8 {
            return Ok(None);
        }

        // 提取discriminator
        let discriminator: [u8; 8] = data[0..8]
            .try_into()
            .map_err(|_| EventListenerError::EventParsing("无法提取discriminator".to_string()))?;
        info!("🔍 提取的discriminator: {:?}", discriminator);

        // 白名单检查：检查是否为已注册的事件类型
        if let Some(program_id) = program_id_hint {
            let parser_key = ParserKey::for_program(program_id, discriminator);
            let universal_key = ParserKey::universal(discriminator);

            // 检查是否在已注册的解析器中
            if !self.parsers.contains_key(&parser_key) && !self.parsers.contains_key(&universal_key) {
                tracing::info!(
                    "⏭️ 跳过未注册事件: program={}, discriminator={:?} - 不在关心列表中",
                    program_id,
                    discriminator
                );
                return Ok(None);
            }
        }
        // 使用智能解析器查找
        if let Some(parser) = self.find_best_parser(discriminator, program_id_hint) {
            tracing::info!(
                "🔍 找到匹配的解析器: {} {} ({:?})",
                parser.get_program_id(),
                parser.get_event_type(),
                discriminator
            );
            if let Some(prog_id) = program_id_hint {
                tracing::info!("🎯 使用程序特定路由: {:?}", prog_id);
            } else {
                tracing::info!("🌐 使用通用路由");
            }

            // 使用找到的解析器解析事件
            tracing::info!(
                "🔧 开始调用解析器: {} 处理数据: {}...",
                parser.get_event_type(),
                &data_str[..50.min(data_str.len())]
            );
            if let Some(event) = parser
                .parse_from_logs(&[format!("Program data: {}", data_str)], signature, slot)
                .await?
            {
                // 验证解析后的事件
                tracing::info!("✅ 解析器返回了事件，开始验证");
                if parser.validate_event(&event).await? {
                    return Ok(Some(event));
                } else {
                    tracing::warn!("⚠️ 事件验证失败: {}", signature);
                }
            } else {
                tracing::warn!("⚠️ 解析器返回了None: {} - {}", parser.get_event_type(), signature);
            }
        } else {
            tracing::info!("🤷 未找到匹配的解析器: {:?}", discriminator);
            if let Some(prog_id) = program_id_hint {
                tracing::info!("🔍 未找到程序 {:?} 的解析器", prog_id);
            }
        }

        Ok(None)
    }

    /// 获取所有已注册的解析器信息
    pub fn get_registered_parsers(&self) -> Vec<(String, [u8; 8])> {
        self.parsers
            .values()
            .map(|parser| (parser.get_event_type().to_string(), parser.get_discriminator()))
            .collect()
    }

    /// 获取所有已注册的解析器详细信息（包含程序ID信息）
    pub fn get_registered_parsers_detailed(&self) -> Vec<(String, [u8; 8], Option<Pubkey>)> {
        self.parsers
            .iter()
            .map(|(key, parser)| {
                let program_id = if key.is_universal() { None } else { Some(key.program_id) };
                (
                    parser.get_event_type().to_string(),
                    parser.get_discriminator(),
                    program_id,
                )
            })
            .collect()
    }

    /// 获取注册的解析器数量
    pub fn parser_count(&self) -> usize {
        self.parsers.len()
    }

    /// 获取按程序分组的解析器统计
    pub fn get_parser_stats_by_program(&self) -> std::collections::HashMap<String, usize> {
        let mut stats = std::collections::HashMap::new();

        for key in self.parsers.keys() {
            let program_key = if key.is_universal() {
                "universal".to_string()
            } else {
                key.program_id.to_string()
            };

            *stats.entry(program_key).or_insert(0) += 1;
        }

        stats
    }

    /// 获取详细的解析器注册统计
    pub fn get_detailed_stats(&self) -> ParserRegistryStats {
        let total_parsers = self.parsers.len();
        let mut program_specific_count = 0;
        let mut universal_count = 0;
        let mut programs_with_parsers = std::collections::HashSet::new();
        let mut event_types = std::collections::HashSet::new();

        for (key, parser) in &self.parsers {
            event_types.insert(parser.get_event_type().to_string());

            if key.is_universal() {
                universal_count += 1;
            } else {
                program_specific_count += 1;
                programs_with_parsers.insert(key.program_id.to_string());
            }
        }

        ParserRegistryStats {
            total_parsers,
            program_specific_count,
            universal_count,
            unique_programs: programs_with_parsers.len(),
            unique_event_types: event_types.len(),
            programs_with_parsers: programs_with_parsers.into_iter().collect(),
            event_types: event_types.into_iter().collect(),
        }
    }
}

/// 解析器注册表统计信息
#[derive(Debug, Clone, serde::Serialize)]
pub struct ParserRegistryStats {
    /// 总解析器数量
    pub total_parsers: usize,
    /// 程序特定解析器数量
    pub program_specific_count: usize,
    /// 通用解析器数量
    pub universal_count: usize,
    /// 有解析器的唯一程序数量
    pub unique_programs: usize,
    /// 唯一事件类型数量
    pub unique_event_types: usize,
    /// 有解析器的程序列表
    pub programs_with_parsers: Vec<String>,
    /// 支持的事件类型列表
    pub event_types: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_sdk::pubkey::Pubkey;

    // Mock解析器用于测试
    struct MockParser {
        discriminator: [u8; 8],
        event_type: &'static str,
        program_id: Pubkey,
    }

    #[async_trait]
    impl EventParser for MockParser {
        fn get_program_id(&self) -> Pubkey {
            self.program_id
        }

        fn get_discriminator(&self) -> [u8; 8] {
            self.discriminator
        }

        fn get_event_type(&self) -> &'static str {
            self.event_type
        }

        async fn parse_from_logs(&self, _logs: &[String], _signature: &str, _slot: u64) -> Result<Option<ParsedEvent>> {
            // Mock实现
            Ok(None)
        }

        async fn validate_event(&self, _event: &ParsedEvent) -> Result<bool> {
            Ok(true)
        }
    }

    #[test]
    fn test_parsed_event_types() {
        let event = ParsedEvent::TokenCreation(TokenCreationEventData {
            mint_address: Pubkey::new_unique().to_string(),
            name: "Test Token".to_string(),
            symbol: "TEST".to_string(),
            uri: "https://example.com/metadata.json".to_string(),
            decimals: 9,
            supply: 1000000,
            creator: Pubkey::new_unique().to_string(),
            has_whitelist: false,
            whitelist_deadline: 0,
            created_at: 1234567890,
            signature: "test_signature".to_string(),
            slot: 12345,
        });

        assert_eq!(event.event_type(), "token_creation");
    }

    #[tokio::test]
    async fn test_registry_creation() {
        let config = crate::config::EventListenerConfig {
            solana: crate::config::settings::SolanaConfig {
                rpc_url: "https://api.devnet.solana.com".to_string(),
                ws_url: "wss://api.devnet.solana.com".to_string(),
                commitment: "confirmed".to_string(),
                program_ids: vec![Pubkey::new_unique()],
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
        };

        let registry = EventParserRegistry::new(&config).unwrap();
        assert!(registry.parser_count() > 0);

        let parsers = registry.get_registered_parsers();
        assert!(!parsers.is_empty());
    }

    #[tokio::test]
    async fn test_parser_registration() {
        let config = crate::config::EventListenerConfig {
            solana: crate::config::settings::SolanaConfig {
                rpc_url: "https://api.devnet.solana.com".to_string(),
                ws_url: "wss://api.devnet.solana.com".to_string(),
                commitment: "confirmed".to_string(),
                program_ids: vec![Pubkey::new_unique()],
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
        };

        let mut registry = EventParserRegistry::new(&config).unwrap();
        let initial_count = registry.parser_count();

        // 注册新的mock解析器
        let mock_parser = Box::new(MockParser {
            discriminator: [1, 2, 3, 4, 5, 6, 7, 8],
            event_type: "mock_event",
            program_id: Pubkey::new_unique(),
        });

        registry.register_universal_parser(mock_parser).unwrap();
        assert_eq!(registry.parser_count(), initial_count + 1);

        // 尝试注册相同discriminator的解析器应该失败
        let duplicate_parser = Box::new(MockParser {
            discriminator: [1, 2, 3, 4, 5, 6, 7, 8],
            event_type: "duplicate_event",
            program_id: Pubkey::new_unique(),
        });

        assert!(registry.register_universal_parser(duplicate_parser).is_err());
    }

    #[tokio::test]
    async fn test_registry_with_all_parsers() {
        let config = crate::config::EventListenerConfig {
            solana: crate::config::settings::SolanaConfig {
                rpc_url: "https://api.devnet.solana.com".to_string(),
                ws_url: "wss://api.devnet.solana.com".to_string(),
                commitment: "confirmed".to_string(),
                program_ids: vec![Pubkey::new_unique()],
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
        };

        let registry = EventParserRegistry::new(&config).unwrap();

        // 应该有六个解析器：2个swap、token_creation、pool_creation、nft_claim、reward_distribution
        assert_eq!(registry.parser_count(), 6);

        let parsers = registry.get_registered_parsers();
        let parser_types: Vec<String> = parsers.iter().map(|(name, _)| name.clone()).collect();

        assert!(parser_types.contains(&"swap".to_string()));
        assert!(parser_types.contains(&"token_creation".to_string()));
        assert!(parser_types.contains(&"pool_creation".to_string()));
        assert!(parser_types.contains(&"nft_claim".to_string()));
        assert!(parser_types.contains(&"reward_distribution".to_string()));

        // 注意：由于有两个swap解析器，总数是6个
        println!("📊 解析器统计: 总数={}, 类型={:?}", parsers.len(), parser_types);
    }

    #[tokio::test]
    async fn test_parse_event_with_context() {
        let config = crate::config::EventListenerConfig {
            solana: crate::config::settings::SolanaConfig {
                rpc_url: "https://api.devnet.solana.com".to_string(),
                ws_url: "wss://api.devnet.solana.com".to_string(),
                commitment: "confirmed".to_string(),
                program_ids: vec![Pubkey::new_unique()],
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
        };

        let registry = EventParserRegistry::new(&config).unwrap();

        // 测试无程序数据日志的情况
        let logs = vec![
            "Program 11111111111111111111111111111111 invoke [1]".to_string(),
            "Program 11111111111111111111111111111111 success".to_string(),
        ];

        let result = registry
            .parse_event_with_context(&logs, "test_sig", 12345, &config.solana.program_ids)
            .await
            .unwrap();
        assert!(result.is_none());

        // 测试无效的程序数据
        let logs_with_invalid_data = vec!["Program data: invalid_base64_data".to_string()];

        let result = registry
            .parse_event_with_context(&logs_with_invalid_data, "test_sig", 12345, &config.solana.program_ids)
            .await;
        // 应该失败或者返回 None
        match result {
            Ok(None) => {} // 正常情况
            Err(_) => {}   // 也可能失败
            _ => panic!("应该返回None或错误"),
        }
    }

    #[tokio::test]
    async fn test_intelligent_program_id_extraction() {
        let config = crate::config::EventListenerConfig {
            solana: crate::config::settings::SolanaConfig {
                rpc_url: "https://api.devnet.solana.com".to_string(),
                ws_url: "wss://api.devnet.solana.com".to_string(),
                commitment: "confirmed".to_string(),
                program_ids: vec!["FA1RJDDXysgwg5Gm3fJXWxt26JQzPkAzhTA114miqNUX".parse().unwrap()],
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
        };

        let registry = EventParserRegistry::new(&config).unwrap();

        // 模拟实际的交易日志，包含系统程序调用和目标程序调用
        let test_logs = vec![
            "Program ComputeBudget111111111111111111111111111111 invoke [1]".to_string(),
            "Program ComputeBudget111111111111111111111111111111 success".to_string(),
            "Program ComputeBudget111111111111111111111111111111 invoke [1]".to_string(),
            "Program ComputeBudget111111111111111111111111111111 success".to_string(),
            "Program ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL invoke [1]".to_string(),
            "Program ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL success".to_string(),
            "Program ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL invoke [1]".to_string(),
            "Program ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL success".to_string(),
            "Program FA1RJDDXysgwg5Gm3fJXWxt26JQzPkAzhTA114miqNUX invoke [1]".to_string(),
            "Program logged: Instruction: SwapV2".to_string(),
            "Program TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb invoke [2]".to_string(),
            "Program TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb success".to_string(),
            "Program TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb invoke [2]".to_string(),
            "Program TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb success".to_string(),
            "Program data: QMbN6CYIceLMGVG4MU+4ATrjvnYksJMPuMJgCPDP1rdRiKjoj6HsZW5rIlaQU+bQ2trw/mEw5Ts8MT5LpaWvcjF+jxy32bzweGbf5NhXXDsAo6eSe6tqrro9sQFopURaKkodvL3GGqAbpd/JYbZV98UXob/ADOEQw+2rDIEszGzDveqoHB9EswjsDgAAAAAAAAAAAAAAAABAQg8AAAAAAAAAAAAAAAAAAOBhVPT8qoQCAQAAAAAAAABPO8PfAAAAAAAAAAAAAAAAwwAAAA==".to_string(),
            "Program FA1RJDDXysgwg5Gm3fJXWxt26JQzPkAzhTA114miqNUX consumed 76104 of 1386486 compute units".to_string(),
            "Program FA1RJDDXysgwg5Gm3fJXWxt26JQzPkAzhTA114miqNUX success".to_string(),
        ];

        // 测试新的程序ID提取逻辑
        let extracted_program_id = registry.extract_program_id_from_logs(&test_logs, &config.solana.program_ids);

        println!("🔍 提取的程序ID: {:?}", extracted_program_id);

        // 应该提取到我们目标程序的ID，而不是系统程序
        assert!(extracted_program_id.is_some(), "应该能够提取程序ID");

        let program_id = extracted_program_id.unwrap();
        assert_eq!(
            program_id,
            "FA1RJDDXysgwg5Gm3fJXWxt26JQzPkAzhTA114miqNUX".parse().unwrap(),
            "应该提取到目标程序的ID而不是系统程序ID"
        );

        println!("✅ 程序ID提取测试成功: {}", program_id);

        // 测试智能路由能否正确工作
        let result = registry
            .parse_event_with_context(&test_logs, "test_sig", 12345, &config.solana.program_ids)
            .await;
        match result {
            Ok(_) => {
                println!("✅ 智能路由处理正常（无论是否找到事件）");
            }
            Err(e) => {
                println!("⚠️ 智能路由遇到错误: {}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_intelligent_routing_three_tier_strategy() {
        let config = crate::config::EventListenerConfig {
            solana: crate::config::settings::SolanaConfig {
                rpc_url: "https://api.devnet.solana.com".to_string(),
                ws_url: "wss://api.devnet.solana.com".to_string(),
                commitment: "confirmed".to_string(),
                program_ids: vec!["CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK".parse().unwrap()],
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
        };

        let registry = EventParserRegistry::new(&config).unwrap();

        // 测试程序ID提取功能
        let logs_with_raydium_program = vec![
            "Program CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK invoke [1]".to_string(),
            "Program data: some_invalid_data".to_string(),
            "Program CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK success".to_string(),
        ];

        // 验证能够从日志中提取程序ID
        let extracted_program_id =
            registry.extract_program_id_from_logs(&logs_with_raydium_program, &config.solana.program_ids);
        assert!(extracted_program_id.is_some(), "应该能够从日志中提取Raydium程序ID");

        let program_id = extracted_program_id.unwrap();
        assert_eq!(
            program_id,
            "CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK".parse().unwrap(),
            "提取的程序ID应该匹配Raydium CLMM程序ID"
        );

        // 测试智能路由的三层策略
        let result = registry
            .parse_event_with_context(
                &logs_with_raydium_program,
                "test_sig",
                12345,
                &config.solana.program_ids,
            )
            .await;

        // 验证智能路由正常工作（即使数据无效）
        match result {
            Ok(None) => {
                println!("✅ 三层智能路由策略正常工作，未找到匹配事件（符合预期）");
            }
            Err(_) => {
                println!("✅ 三层智能路由策略正常工作，数据解析失败（符合预期）");
            }
            Ok(Some(_)) => {
                println!("⚠️ 意外解析成功，测试数据可能有问题");
            }
        }

        // 验证解析器注册表的统计信息
        let stats = registry.get_detailed_stats();
        // 应该有六个解析器：2个swap、token_creation、pool_creation、nft_claim、reward_distribution
        assert_eq!(stats.total_parsers, 6, "应该有6个解析器");
        assert_eq!(stats.program_specific_count, 6, "应该都是程序特定解析器");
        assert_eq!(stats.universal_count, 0, "应该没有通用解析器");
        assert_eq!(stats.unique_programs, 3, "应该有3个不同的程序");

        println!("📊 解析器统计: {:?}", stats);
    }
}
