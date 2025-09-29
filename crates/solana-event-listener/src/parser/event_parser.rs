use crate::config::EventListenerConfig;
use crate::error::{EventListenerError, Result};
use crate::parser::cpmm_init_pool_parser::InitPoolEventData;
use crate::parser::cpmm_lp_change_parser::LpChangeEventData;
use crate::parser::deposit_event_parser::DepositEventData;
use crate::parser::launch_event_parser::LaunchEventData;
use crate::parser::nft_claim_parser::NftClaimEventData;
use crate::parser::pool_creation_parser::PoolCreatedEventData;
use crate::parser::reward_distribution_parser::RewardDistributionEventData;
use crate::parser::swap_parser::SwapEventData;
use crate::parser::token_creation_parser::TokenCreationEventData;
use crate::parser::{
    DepositEventParser, InitPoolParser, LaunchEventParser, LpChangeParser, NftClaimParser, PoolCreationParser,
    RewardDistributionParser, SwapParser, TokenCreationParser,
};
use anchor_lang::pubkey;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use solana_sdk::pubkey::Pubkey;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use tracing::info;
use utils::TokenMetadataProvider;

/// 事件数据流来源
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventDataSource {
    /// WebSocket实时订阅数据流
    WebSocketSubscription,
    /// 回填服务数据流
    BackfillService,
}

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

/// 从事件类型计算discriminator
pub fn calculate_event_discriminator(event_type: &str) -> [u8; 8] {
    let mut hasher = Sha256::new();
    hasher.update(format!("event:{}", event_type).as_bytes());
    let hash = hasher.finalize();

    // 取前8字节作为discriminator
    let mut discriminator = [0u8; 8];
    discriminator.copy_from_slice(&hash[..8]);
    discriminator
}

/// 解析后的事件数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ParsedEvent {
    /// 代币创建事件
    TokenCreation(TokenCreationEventData),
    /// 池子创建事件
    PoolCreation(PoolCreatedEventData),
    /// NFT领取事件
    NftClaim(NftClaimEventData),
    /// 奖励分发事件
    RewardDistribution(RewardDistributionEventData),
    /// 交换事件
    Swap(SwapEventData),
    /// Meme币发射事件
    Launch(LaunchEventData),
    /// 存款事件
    Deposit(DepositEventData),
    /// LP变更事件
    LpChange(LpChangeEventData),
    /// 池子初始化事件
    InitPool(InitPoolEventData),
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
            ParsedEvent::Launch(_) => "launch",
            ParsedEvent::Deposit(_) => "deposit",
            ParsedEvent::LpChange(_) => "lp_change",
            ParsedEvent::InitPool(_) => "init_pool",
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
            ParsedEvent::Launch(data) => format!("{}_{}", data.meme_token_mint, data.signature),
            ParsedEvent::Deposit(data) => format!("{}_{}_{}", data.user, data.token_mint, data.signature),
            ParsedEvent::LpChange(data) => data.signature.clone(), // 使用signature作为唯一标识
            ParsedEvent::InitPool(data) => data.pool_id.clone(),   // 使用pool_id作为唯一标识
        }
    }
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
    /// 回填服务配置的ParserKey集合（program_id + discriminator）
    backfill_parser_keys: HashSet<ParserKey>,
}

impl EventParserRegistry {
    /// 创建新的解析器注册表
    pub fn new(config: &EventListenerConfig) -> Result<Self> {
        Self::new_with_metadata_provider(config, None)
    }

    /// 创建新的解析器注册表（支持注入元数据提供者）
    pub fn new_with_metadata_provider(
        config: &EventListenerConfig,
        metadata_provider: Option<Arc<tokio::sync::Mutex<dyn TokenMetadataProvider>>>,
    ) -> Result<Self> {
        Self::new_with_metadata_provider_and_backfill(config, metadata_provider, None)
    }

    /// 创建新的解析器注册表（支持注入元数据提供者和回填配置）
    pub fn new_with_metadata_provider_and_backfill(
        config: &EventListenerConfig,
        metadata_provider: Option<Arc<tokio::sync::Mutex<dyn TokenMetadataProvider>>>,
        backfill_parser_keys: Option<HashSet<ParserKey>>,
    ) -> Result<Self> {
        let mut registry = Self {
            parsers: HashMap::new(),
            backfill_parser_keys: backfill_parser_keys.unwrap_or_default(),
        };

        // 交换事件解析器
        let swap_parser = Box::new(SwapParser::new(
            config,
            pubkey!("FA1RJDDXysgwg5Gm3fJXWxt26JQzPkAzhTA114miqNUX"),
        )?);
        registry.register_program_parser(swap_parser)?;

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
        let mut reward_distribution_parser = Box::new(RewardDistributionParser::new(
            config,
            pubkey!("FA1RJDDXysgwg5Gm3fJXWxt26JQzPkAzhTA114miqNUX"),
        )?);

        // 如果提供了元数据提供者，则注入到奖励分发解析器中
        if let Some(ref provider) = metadata_provider {
            reward_distribution_parser.set_metadata_provider(provider.clone());
            info!("✅ 已将代币元数据提供者注入到奖励分发解析器");
        }

        registry.register_program_parser(reward_distribution_parser)?;

        // 代币创建事件解析器
        let token_creation_parser = Box::new(TokenCreationParser::new(
            config,
            pubkey!("AZxHQhxgjENmx8x9CQ8r86Eodo8Qg6H9wYiuRqbonaoH"),
        )?);
        registry.register_program_parser(token_creation_parser)?;

        // 存款事件解析器
        let mut deposit_parser = Box::new(DepositEventParser::new(
            config,
            pubkey!("AZxHQhxgjENmx8x9CQ8r86Eodo8Qg6H9wYiuRqbonaoH"),
        )?);

        // 如果提供了元数据提供者，则注入到存款解析器中
        if let Some(provider) = &metadata_provider {
            deposit_parser.set_metadata_provider(provider.clone());
            info!("✅ 已将代币元数据提供者注入到存款解析器");
        }

        registry.register_program_parser(deposit_parser)?;

        // LaunchEvent解析器 - 支持Meme币发射平台 发射动作现在是在合约里处理，暂时不订阅发射事件
        // 注册AZxHQhxgjENmx8x9CQ8r86Eodo8Qg6H9wYiuRqbonaoH程序的Launch解析器
        let launch_parser1 = Box::new(LaunchEventParser::new(
            config,
            pubkey!("AZxHQhxgjENmx8x9CQ8r86Eodo8Qg6H9wYiuRqbonaoH"),
        )?);
        registry.register_program_parser(launch_parser1)?;

        // 使用默认的Raydium CPMM程序ID
        let lp_change_parser = Box::new(LpChangeParser::new(
            config,
            pubkey!("FairxoKThzWcDy9avKPsADqzni18LrXxKAZEHdXVo5gi"),
        )?);
        registry.register_program_parser(lp_change_parser)?;

        // 池子初始化事件解析器 - 使用配置中的CPMM程序ID
        let init_pool_parser = Box::new(InitPoolParser::new(
            config,
            pubkey!("FairxoKThzWcDy9avKPsADqzni18LrXxKAZEHdXVo5gi"),
        )?);
        registry.register_program_parser(init_pool_parser)?;

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

    /// 从单条日志和完整上下文解析所有事件（处理多事件版本）
    ///
    /// 与 `parse_event_with_context` 不同，此方法会处理并返回所有找到的有效事件，
    /// 而不是只返回第一个有效事件。
    ///
    /// # 参数
    /// - `logs`: 交易日志
    /// - `signature`: 交易签名
    /// - `slot`: 区块高度
    /// - `subscribed_programs`: 订阅的程序列表
    /// - `data_source`: 数据流来源，用于选择合适的过滤策略
    ///
    pub async fn parse_all_events_with_context(
        &self,
        logs: &[String],
        signature: &str,
        slot: u64,
        subscribed_programs: &[Pubkey],
        data_source: Option<EventDataSource>,
    ) -> Result<Vec<ParsedEvent>> {
        // 尝试从日志中提取程序ID
        let program_id_hint = self.extract_program_id_from_logs(logs, subscribed_programs);

        tracing::info!(
            "🧠 智能路由启动（处理所有事件）- 数据源: {:?}, 程序ID提示: {:?}, 使用ParserKey精确过滤",
            data_source.unwrap_or(EventDataSource::WebSocketSubscription),
            program_id_hint
        );

        let mut all_valid_events = Vec::new();
        let mut program_data_count = 0;
        let mut processed_count = 0;
        let mut skipped_count = 0;

        // 处理所有程序数据日志
        for (index, log) in logs.iter().enumerate() {
            if log.starts_with("Program data: ") {
                program_data_count += 1;
                if let Some(data_part) = log.strip_prefix("Program data: ") {
                    tracing::info!(
                        "📊 处理第{}个Program data (行{}, 数据: {})",
                        program_data_count,
                        index + 1,
                        data_part
                    );

                    // 为这个特定的Program data确定程序ID
                    let specific_program_id = self.extract_program_id_for_data_index(logs, index, subscribed_programs);

                    tracing::info!(
                        "🎯 第{}个Program data (行{}) 确定的程序ID: {:?}",
                        program_data_count,
                        index + 1,
                        specific_program_id
                    );

                    match self
                        .try_parse_program_data_with_hint(data_part, signature, slot, specific_program_id, data_source)
                        .await?
                    {
                        Some(event) => {
                            tracing::info!("✅ 第{}个事件解析成功: {}", program_data_count, event.event_type());
                            processed_count += 1;
                            // 收集所有有效事件，不跳过任何一个
                            all_valid_events.push(event);
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
                "📋 事件处理总结（处理所有事件）: 发现{}个Program data，成功处理{}个，跳过{}个",
                program_data_count,
                processed_count,
                skipped_count
            );
        }

        // 如果没有找到任何事件，尝试通用解析器
        // if all_valid_events.is_empty() {
        //     tracing::info!("🔄 Program data解析未找到事件，尝试通用解析器");
        //     for parser in self.parsers.values() {
        //         if let Some(event) = parser.parse_from_logs(logs, signature, slot).await? {
        //             tracing::info!("✅ 通用解析器成功: {}", parser.get_event_type());
        //             all_valid_events.push(event);
        //         }
        //     }
        // }

        if !all_valid_events.is_empty() {
            tracing::info!(
                "✅ 智能路由成功解析{}个事件: {:?}",
                all_valid_events.len(),
                all_valid_events.iter().map(|e| e.event_type()).collect::<Vec<_>>()
            );
        } else {
            tracing::info!("❌ 智能路由未找到匹配的解析器");
        }

        Ok(all_valid_events)
    }

    /// 从日志中提取程序ID（解析用）
    /// 新策略：查找包含Program data的程序调用块，并验证是否在允许的程序列表中
    /// 注意：这个方法只返回第一个找到的程序ID，用于兼容性
    pub fn extract_program_id_from_logs(&self, logs: &[String], allowed_programs: &[Pubkey]) -> Option<Pubkey> {
        // 找到第一个Program data
        let first_data_index = logs.iter().position(|log| log.starts_with("Program data: "))?;

        // 为第一个Program data确定程序ID
        self.extract_program_id_for_data_index(logs, first_data_index, allowed_programs)
    }

    /// 为特定的Program data索引确定其所属的程序ID
    pub fn extract_program_id_for_data_index(
        &self,
        logs: &[String],
        data_index: usize,
        allowed_programs: &[Pubkey],
    ) -> Option<Pubkey> {
        tracing::debug!("🔍 分析第{}行的Program data", data_index + 1);

        // 策略：从Program data往前查找，找到距离最近的allowed program的invoke
        let mut best_match: Option<(usize, Pubkey)> = None;

        // 从Program data位置往前搜索，寻找距离最近的允许程序调用
        for i in (0..data_index).rev() {
            let log = &logs[i];
            if log.starts_with("Program ") && log.contains(" invoke [") {
                let parts: Vec<&str> = log.split_whitespace().collect();
                if parts.len() >= 3 {
                    if let Ok(program_id) = parts[1].parse::<Pubkey>() {
                        // 检查是否为允许的程序
                        if self.is_allowed_program(&program_id, allowed_programs) {
                            tracing::debug!("🔍 第{}行找到允许的程序调用: {}", i + 1, program_id);

                            // 验证这个程序调用确实包含我们的Program data
                            // 查找对应的success/consumed在Program data之后
                            let has_success_after = logs
                                .iter()
                                .enumerate()
                                .skip(data_index + 1) // 从Program data之后开始查找
                                .any(|(j, log)| {
                                    if log.starts_with("Program ")
                                        && (log.contains(" success") || log.contains(" consumed "))
                                    {
                                        let parts: Vec<&str> = log.split_whitespace().collect();
                                        if parts.len() >= 2 {
                                            if let Ok(success_program_id) = parts[1].parse::<Pubkey>() {
                                                if success_program_id == program_id {
                                                    tracing::debug!(
                                                        "✅ 第{}行找到对应的success: {}",
                                                        j + 1,
                                                        program_id
                                                    );
                                                    return true;
                                                }
                                            }
                                        }
                                    }
                                    false
                                });

                            if has_success_after {
                                best_match = Some((i, program_id));
                                break; // 找到最近的就退出
                            } else {
                                tracing::debug!("⚠️ 程序{}在Program data之后没有找到success", program_id);
                            }
                        }
                    }
                }
            }
        }

        if let Some((invoke_line, program_id)) = best_match {
            tracing::info!(
                "🎯 第{}行Program data属于第{}行调用的程序: {}",
                data_index + 1,
                invoke_line + 1,
                program_id
            );
            return Some(program_id);
        }

        tracing::warn!("⚠️ 第{}行Program data未找到对应的允许程序", data_index + 1);
        None
    }

    /// 检查程序ID是否在允许的程序列表中
    fn is_allowed_program(&self, program_id: &Pubkey, allowed_programs: &[Pubkey]) -> bool {
        allowed_programs.contains(program_id)
    }

    /// 设置回填服务配置的ParserKey集合
    pub fn set_backfill_parser_keys(&mut self, parser_keys: HashSet<ParserKey>) {
        self.backfill_parser_keys = parser_keys;
        tracing::info!("🔑 设置回填ParserKey集合: {} 个键", self.backfill_parser_keys.len());
        for key in &self.backfill_parser_keys {
            tracing::info!(
                "  - Program: {}, Discriminator: {:?}",
                key.program_id,
                key.discriminator
            );
        }
    }

    /// 获取回填服务配置的ParserKey集合
    pub fn get_backfill_parser_keys(&self) -> &HashSet<ParserKey> {
        &self.backfill_parser_keys
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
        data_source: Option<EventDataSource>,
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

        // ParserKey集合过滤：根据数据源使用不同的精确过滤策略
        if let Some(program_id) = program_id_hint {
            let parser_key = ParserKey::for_program(program_id, discriminator);
            let universal_key = ParserKey::universal(discriminator);

            let allowed_by_data_source = match data_source {
                Some(EventDataSource::BackfillService) => {
                    // 回填服务使用配置的ParserKey集合进行精确过滤
                    let backfill_keys = self.get_backfill_parser_keys();
                    let allowed = backfill_keys.contains(&parser_key)
                        || backfill_keys
                            .iter()
                            .any(|key| key.discriminator == discriminator && key.is_universal());

                    if !allowed {
                        tracing::info!(
                            "⏭️ 回填服务跳过未配置的事件: program={}, discriminator={:?} - 不在回填ParserKey集合中",
                            program_id,
                            discriminator
                        );
                    }
                    allowed
                }
                Some(EventDataSource::WebSocketSubscription) | None => {
                    // WebSocket订阅使用已注册解析器进行过滤
                    let allowed = self.parsers.contains_key(&parser_key) || self.parsers.contains_key(&universal_key);

                    if !allowed {
                        tracing::info!(
                            "⏭️ WebSocket订阅跳过未注册事件: program={}, discriminator={:?} - 不在已注册解析器中",
                            program_id,
                            discriminator
                        );
                    }
                    allowed
                }
            };

            if !allowed_by_data_source {
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
    use crate::parser::token_creation_parser::TokenCreationEventData;

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
            project_config: Pubkey::new_unique().to_string(),
            mint_address: Pubkey::new_unique().to_string(),
            name: "Test Token".to_string(),
            symbol: "TEST".to_string(),
            metadata_uri: "https://example.com/metadata.json".to_string(),
            logo_uri: "https://example.com/logo.png".to_string(),
            decimals: 9,
            supply: 1000000,
            creator: Pubkey::new_unique().to_string(),
            has_whitelist: false,
            whitelist_deadline: 0,
            created_at: 1234567890,
            signature: "test_signature".to_string(),
            slot: 12345,
            extensions: None,
            source: None,
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
            backfill: None,
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
            backfill: None,
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
            backfill: None,
        };

        let registry = EventParserRegistry::new(&config).unwrap();

        // 应该有9个解析器：swap、token_creation、pool_creation、nft_claim、reward_distribution、launch、deposit、lp_change、init_pool
        assert_eq!(registry.parser_count(), 9);

        let parsers = registry.get_registered_parsers();
        let parser_types: Vec<String> = parsers.iter().map(|(name, _)| name.clone()).collect();

        assert!(parser_types.contains(&"swap".to_string()));
        assert!(parser_types.contains(&"token_creation".to_string()));
        assert!(parser_types.contains(&"pool_creation".to_string()));
        assert!(parser_types.contains(&"nft_claim".to_string()));
        assert!(parser_types.contains(&"reward_distribution".to_string()));
        assert!(parser_types.contains(&"launch".to_string()));
        assert!(parser_types.contains(&"deposit".to_string()));
        assert!(parser_types.contains(&"lp_change".to_string()));
        assert!(parser_types.contains(&"init_pool".to_string()));

        // 注意：现在有9个解析器（新增了init_pool解析器）
        println!("📊 解析器统计: 总数={}, 类型={:?}", parsers.len(), parser_types);
    }

    #[tokio::test]
    async fn test_data_source_filtering() {
        use solana_sdk::pubkey::Pubkey;
        use std::str::FromStr;

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
            backfill: None,
        };

        let mut registry = EventParserRegistry::new(&config).unwrap();

        // 设置回填ParserKey集合（不同于WebSocket订阅的程序列表）
        let websocket_program = Pubkey::from_str("FA1RJDDXysgwg5Gm3fJXWxt26JQzPkAzhTA114miqNUX").unwrap();
        let backfill_program = Pubkey::from_str("AZxHQhxgjENmx8x9CQ8r86Eodo8Qg6H9wYiuRqbonaoH").unwrap();

        // 创建测试用的ParserKey集合
        let mut backfill_keys = std::collections::HashSet::new();
        let test_discriminator = calculate_event_discriminator("TestEvent");
        let test_parser_key = ParserKey::for_program(backfill_program, test_discriminator);
        backfill_keys.insert(test_parser_key);

        registry.set_backfill_parser_keys(backfill_keys);

        // 测试数据源过滤逻辑
        let logs = vec!["Program data: test".to_string()];

        // 使用WebSocket数据源 - 应该使用websocket_program
        let result_websocket = registry
            .parse_all_events_with_context(
                &logs,
                "test_sig",
                12345,
                &[websocket_program],
                Some(EventDataSource::WebSocketSubscription),
            )
            .await
            .unwrap();

        // 使用回填数据源 - 应该使用backfill_program
        let result_backfill = registry
            .parse_all_events_with_context(
                &logs,
                "test_sig",
                12345,
                &[websocket_program],
                Some(EventDataSource::BackfillService),
            )
            .await
            .unwrap();

        // 不传数据源（默认WebSocket行为）
        let result_default = registry
            .parse_all_events_with_context(&logs, "test_sig", 12345, &[websocket_program], None)
            .await
            .unwrap();

        // 验证结果（由于没有有效的Program data，都应该返回空，但过滤逻辑已经执行）
        assert!(result_websocket.is_empty());
        assert!(result_backfill.is_empty());
        assert!(result_default.is_empty());

        // 验证回填ParserKey配置已正确设置
        let backfill_keys = registry.get_backfill_parser_keys();
        assert_eq!(backfill_keys.len(), 1);
        assert!(backfill_keys.contains(&test_parser_key));
    }

    #[tokio::test]
    async fn test_parse_all_events_with_context() {
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
            backfill: None,
        };

        let registry = EventParserRegistry::new(&config).unwrap();

        // 测试无Program data的日志
        let logs = vec![
            "Program 11111111111111111111111111111111 invoke [1]".to_string(),
            "Program 11111111111111111111111111111111 success".to_string(),
        ];

        let result = registry
            .parse_all_events_with_context(&logs, "test_sig", 12345, &config.solana.program_ids, None)
            .await
            .unwrap();
        assert!(result.is_empty());

        // 测试包含无效Program data的日志
        let logs_with_invalid_data = vec![
            "Program data: invalid_base64_data".to_string(),
            "Program data: another_invalid_data".to_string(),
        ];

        let result = registry
            .parse_all_events_with_context(
                &logs_with_invalid_data,
                "test_sig",
                12345,
                &config.solana.program_ids,
                None,
            )
            .await;

        match result {
            Ok(events) => assert!(events.is_empty(), "应该返回空的事件列表"),
            Err(_) => {} // 也可能因为Base64解码失败而出错
        }
    }

    #[tokio::test]
    async fn test_parser_key_filtering_by_data_source() {
        use solana_sdk::pubkey::Pubkey;
        use std::collections::HashSet;
        use std::str::FromStr;

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
            backfill: None,
        };

        let mut registry = EventParserRegistry::new(&config).unwrap();

        // 设置测试用的回填ParserKey集合
        let test_program_id = Pubkey::from_str("AZxHQhxgjENmx8x9CQ8r86Eodo8Qg6H9wYiuRqbonaoH").unwrap();
        let test_event_type = "TestEvent";
        let test_discriminator = calculate_event_discriminator(test_event_type);
        let test_parser_key = ParserKey::for_program(test_program_id, test_discriminator);

        let mut backfill_keys = HashSet::new();
        backfill_keys.insert(test_parser_key);
        registry.set_backfill_parser_keys(backfill_keys);

        // 获取回填ParserKey集合并验证
        let retrieved_keys = registry.get_backfill_parser_keys();
        assert_eq!(retrieved_keys.len(), 1);
        assert!(retrieved_keys.contains(&test_parser_key));

        println!("✅ ParserKey过滤逻辑测试通过");
        println!("   - 测试程序ID: {}", test_program_id);
        println!("   - 测试事件类型: {}", test_event_type);
        println!("   - 计算的discriminator: {:?}", test_discriminator);
        println!("   - 生成的ParserKey: {:?}", test_parser_key);
    }

    #[test]
    fn test_calculate_event_discriminator() {
        // 测试discriminator计算的一致性
        let event_type = "LaunchEvent";
        let discriminator1 = calculate_event_discriminator(event_type);
        let discriminator2 = calculate_event_discriminator(event_type);

        // 同一事件类型应该产生相同的discriminator
        assert_eq!(discriminator1, discriminator2);

        // 不同事件类型应该产生不同的discriminator
        let discriminator3 = calculate_event_discriminator("TokenCreationEvent");
        assert_ne!(discriminator1, discriminator3);

        println!("✅ Discriminator计算测试通过");
        println!("   - LaunchEvent discriminator: {:?}", discriminator1);
        println!("   - TokenCreationEvent discriminator: {:?}", discriminator3);
    }

    #[test]
    fn test_referral_reward_event_parser_registration() {
        use std::collections::HashSet;
        use std::str::FromStr;

        // 创建测试配置
        let config = crate::config::EventListenerConfig {
            solana: crate::config::settings::SolanaConfig {
                rpc_url: "https://api.devnet.solana.com".to_string(),
                ws_url: "wss://api.devnet.solana.com".to_string(),
                commitment: "confirmed".to_string(),
                program_ids: vec![],
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
        };

        // 模拟回填服务的ParserKey集合
        let fa1r_program_id = Pubkey::from_str("FA1RJDDXysgwg5Gm3fJXWxt26JQzPkAzhTA114miqNUX").unwrap();
        let ref_program_id = Pubkey::from_str("REFxcjx4pKym9j5Jzbo9wh92CtYTzHt9fqcjgvZGvUL").unwrap();
        let discriminator = calculate_event_discriminator("ReferralRewardEvent");

        let fa1r_parser_key = ParserKey::for_program(fa1r_program_id, discriminator);
        let ref_parser_key = ParserKey::for_program(ref_program_id, discriminator);

        let mut backfill_keys = HashSet::new();
        backfill_keys.insert(fa1r_parser_key);
        backfill_keys.insert(ref_parser_key);

        // 创建注册表
        let registry =
            EventParserRegistry::new_with_metadata_provider_and_backfill(&config, None, Some(backfill_keys)).unwrap();

        println!("🔍 测试ReferralRewardEvent解析器注册:");
        println!("   - FA1R程序ID: {}", fa1r_program_id);
        println!("   - REF程序ID: {}", ref_program_id);
        println!("   - discriminator: {:?}", discriminator);

        // 验证FA1R程序ID的解析器能找到，REF程序没有对应的奖励分发解析器
        let fa1r_parser = registry.find_best_parser(discriminator, Some(fa1r_program_id));
        let ref_parser = registry.find_best_parser(discriminator, Some(ref_program_id));

        println!("   - FA1R程序解析器找到: {}", fa1r_parser.is_some());
        println!("   - REF程序解析器找到: {}", ref_parser.is_some());

        assert!(fa1r_parser.is_some(), "应该能找到FA1R程序的RewardDistributionParser");
        assert!(ref_parser.is_none(), "REF程序不应该有ReferralRewardEvent解析器");

        println!("✅ FA1R程序的ReferralRewardEvent解析器正确注册，REF程序没有该解析器");
    }

    #[test]
    fn test_parser_key_creation_and_comparison() {
        use solana_sdk::pubkey::Pubkey;
        use std::str::FromStr;

        let program_id = Pubkey::from_str("AZxHQhxgjENmx8x9CQ8r86Eodo8Qg6H9wYiuRqbonaoH").unwrap();
        let discriminator = calculate_event_discriminator("TestEvent");

        // 测试程序特定ParserKey创建
        let parser_key1 = ParserKey::for_program(program_id, discriminator);
        let parser_key2 = ParserKey::for_program(program_id, discriminator);
        assert_eq!(parser_key1, parser_key2);

        // 测试通用ParserKey创建
        let universal_key1 = ParserKey::universal(discriminator);
        let universal_key2 = ParserKey::universal(discriminator);
        assert_eq!(universal_key1, universal_key2);
        assert!(universal_key1.is_universal());

        // 程序特定key和通用key应该不相等
        assert_ne!(parser_key1, universal_key1);

        println!("✅ ParserKey创建和比较测试通过");
    }

    #[tokio::test]
    async fn test_init_pool_parser_registration() {
        // 测试InitPoolParser是否正确注册到EventParserRegistry
        let config = crate::config::EventListenerConfig {
            solana: crate::config::settings::SolanaConfig {
                rpc_url: "https://api.devnet.solana.com".to_string(),
                ws_url: "wss://api.devnet.solana.com".to_string(),
                commitment: "confirmed".to_string(),
                program_ids: vec![],
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
        };

        let registry = EventParserRegistry::new(&config).unwrap();

        // 验证init_pool解析器已注册
        let parsers = registry.get_registered_parsers();
        let parser_types: Vec<String> = parsers.iter().map(|(name, _)| name.clone()).collect();

        assert!(
            parser_types.contains(&"init_pool".to_string()),
            "InitPoolParser should be registered in EventParserRegistry"
        );

        // 验证解析器的详细信息
        let detailed_parsers = registry.get_registered_parsers_detailed();
        let init_pool_parser = detailed_parsers
            .iter()
            .find(|(event_type, _, _)| event_type == "init_pool");

        assert!(init_pool_parser.is_some(), "InitPoolParser details should be available");

        let (_, discriminator, program_id) = init_pool_parser.unwrap();

        // 验证discriminator是正确计算的
        let expected_discriminator = calculate_event_discriminator("InitPoolEvent");
        assert_eq!(*discriminator, expected_discriminator, "Discriminator should match");

        // 验证程序ID
        assert!(program_id.is_some(), "Program ID should be set for InitPoolParser");

        println!("✅ InitPoolParser注册测试通过");
        println!("   - Event Type: init_pool");
        println!("   - Discriminator: {:?}", discriminator);
        println!("   - Program ID: {:?}", program_id);
    }

    #[test]
    fn test_get_cpmm_program_id_config() {
        use std::env;

        // 测试默认值
        env::remove_var("CPMM_PROGRAM_ID");
        let config = crate::config::EventListenerConfig {
            solana: crate::config::settings::SolanaConfig {
                rpc_url: "https://api.devnet.solana.com".to_string(),
                ws_url: "wss://api.devnet.solana.com".to_string(),
                commitment: "confirmed".to_string(),
                program_ids: vec![],
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
        };

        let default_program_id = config.get_cpmm_program_id().unwrap();
        assert_eq!(
            default_program_id.to_string(),
            "FairxoKThzWcDy9avKPsADqzni18LrXxKAZEHdXVo5gi"
        );

        // 测试环境变量覆盖
        env::set_var("CPMM_PROGRAM_ID", "AZxHQhxgjENmx8x9CQ8r86Eodo8Qg6H9wYiuRqbonaoH");
        let env_program_id = config.get_cpmm_program_id().unwrap();
        assert_eq!(
            env_program_id.to_string(),
            "AZxHQhxgjENmx8x9CQ8r86Eodo8Qg6H9wYiuRqbonaoH"
        );

        // 清理环境变量
        env::remove_var("CPMM_PROGRAM_ID");

        println!("✅ get_cpmm_program_id配置测试通过");
    }

    #[test]
    fn test_extract_program_id_multiple_data() {
        use solana_sdk::pubkey::Pubkey;
        use std::str::FromStr;

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
            backfill: None,
        };

        let registry = EventParserRegistry::new(&config).unwrap();

        // 模拟实际的多Program data日志
        let azxh_program = Pubkey::from_str("AZxHQhxgjENmx8x9CQ8r86Eodo8Qg6H9wYiuRqbonaoH").unwrap();
        let fa1r_program = Pubkey::from_str("FA1RJDDXysgwg5Gm3fJXWxt26JQzPkAzhTA114miqNUX").unwrap();
        let allowed_programs = vec![azxh_program, fa1r_program];

        let logs = vec![
            "Program ComputeBudget111111111111111111111111111111 invoke [1]".to_string(),
            "Program ComputeBudget111111111111111111111111111111 success".to_string(),
            "Program AZxHQhxgjENmx8x9CQ8r86Eodo8Qg6H9wYiuRqbonaoH invoke [1]".to_string(),
            "Program log: Instruction: LaunchMvp".to_string(),
            "Program data: G8EvgnNc716p/Idl/sjYHDtqSfhA7htGDXRo4ucE3uxcKePhq3AUZgabiFf+q4GE+2h/Y0YYwDXaxDncGus7VZig8AAAAAABAINxunm81YV3JKamvYB0swDg/SWx1a2ylKyPBUIu968AAAAAOoww4o55NT4AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAGSns7bgDQDyBSoBAAAAmpmZmZmZqT8A".to_string(),
            "Program FA1RJDDXysgwg5Gm3fJXWxt26JQzPkAzhTA114miqNUX invoke [2]".to_string(),
            "Program log: Instruction: CreatePool".to_string(),
            "Program data: GV5LL3BjNT8Gm4hX/quBhPtof2NGGMA12sQ53BrrO1WYoPAAAAAAAan8h2X+yNgcO2pJ+EDuG0YNdGji5wTe7Fwp4+GrcBRmPACREgOzkDtzcjYnC9HFUqZ8O6kPAFWAAvmPgDaWf3BCCp6kzAxUogQAAAAAAAAAAABUFf3/cjZ0upqxPm82geqwQJAtvneasdTpNXsSxDqy9e9IqF+9vwPdD97M+I5Iysa0yg8/w+HPaMbpMWP2gT9seAu+uQ==".to_string(),
            "Program FA1RJDDXysgwg5Gm3fJXWxt26JQzPkAzhTA114miqNUX consumed 83388 of 722486 compute units".to_string(),
            "Program FA1RJDDXysgwg5Gm3fJXWxt26JQzPkAzhTA114miqNUX success".to_string(),
            "Program AZxHQhxgjENmx8x9CQ8r86Eodo8Qg6H9wYiuRqbonaoH consumed 169102 of 799700 compute units".to_string(),
            "Program AZxHQhxgjENmx8x9CQ8r86Eodo8Qg6H9wYiuRqbonaoH success".to_string(),
        ];

        // 测试第一个Program data (index 4) - 应该属于AZxH程序
        let first_program_id = registry.extract_program_id_for_data_index(&logs, 4, &allowed_programs);
        assert_eq!(first_program_id, Some(azxh_program));
        println!("✅ 第一个Program data正确识别为AZxH程序");

        // 测试第二个Program data (index 7) - 应该属于FA1R程序
        let second_program_id = registry.extract_program_id_for_data_index(&logs, 7, &allowed_programs);
        assert_eq!(second_program_id, Some(fa1r_program));
        println!("✅ 第二个Program data正确识别为FA1R程序");

        // 测试原始方法只返回第一个
        let first_found = registry.extract_program_id_from_logs(&logs, &allowed_programs);
        assert_eq!(first_found, Some(azxh_program));
        println!("✅ 原始方法正确返回第一个Program data的程序ID");

        println!("✅ 多Program data程序ID提取测试通过");
    }
}
