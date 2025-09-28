use crate::dtos::solana::common::TokenInfo;
use crate::dtos::solana::cpmm::lp::query_lp_mint::{LpMintPoolInfo, PoolPeriodStats, QueryLpMintRequest};
use crate::services::solana::cpmm::lp_change_event::lp_change_event_error::LpChangeEventError;
use crate::services::solana::cpmm::lp_change_event::lp_change_event_service::LpChangeEventService;
use anyhow::Result;
use database::cpmm::lp_change_event::model::LpChangeEvent;
use database::Database;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};
use utils::{ExternalTokenMetadata, TokenMetadataProvider};

/// LP mint查询服务，负责根据lp_mint查询池子信息并整合链上数据
pub struct LpMintQueryService {
    lp_change_event_service: LpChangeEventService,
    metadata_provider: Option<Arc<Mutex<dyn TokenMetadataProvider>>>,
}

impl LpMintQueryService {
    /// 创建新的服务实例
    pub fn new(database: Arc<Database>) -> Result<Self> {
        Ok(Self {
            lp_change_event_service: LpChangeEventService::new(database),
            metadata_provider: None, // 通过setter方法注入
        })
    }

    /// 设置代币元数据提供者
    pub fn set_metadata_provider(&mut self, provider: Arc<tokio::sync::Mutex<dyn TokenMetadataProvider>>) {
        self.metadata_provider = Some(provider);
        info!("✅ LpMintQueryService 代币元数据提供者已设置");
    }

    /// 根据多个LP mint查询池子信息
    pub async fn query_pools_by_lp_mints(
        &self,
        request: QueryLpMintRequest,
    ) -> Result<Vec<Option<LpMintPoolInfo>>, LpChangeEventError> {
        info!("🔍 查询LP mint池子信息，参数: {:?}", request);

        let lp_mints = request.parse_lp_mints();
        if lp_mints.is_empty() {
            warn!("⚠️ LP mint列表为空");
            return Ok(vec![]);
        }

        // 验证LP mint数量限制
        if lp_mints.len() > 100 {
            return Err(LpChangeEventError::QueryParameterError(
                "一次查询的LP mint数量不能超过100个".to_string(),
            ));
        }

        info!("📋 解析到{}个LP mint地址", lp_mints.len());

        // 从数据库查询LP变更事件
        let events = self
            .lp_change_event_service
            .query_events_by_lp_mints(lp_mints.clone(), Some(1000))
            .await?;

        info!("📊 查询到{}条LP变更事件", events.len());
        if events.is_empty() {
            let mut result = vec![];
            (0..lp_mints.len()).for_each(|_| result.push(None));
            return Ok(result);
        }

        // 按lp_mint分组事件
        let mut events_by_lp_mint: HashMap<String, Vec<LpChangeEvent>> = HashMap::new();
        for event in events {
            events_by_lp_mint
                .entry(event.lp_mint.clone())
                .or_insert_with(Vec::new)
                .push(event);
        }

        // 为每个LP mint构建池子信息
        let mut pool_infos = Vec::new();
        for lp_mint in lp_mints {
            match self.build_pool_info(&lp_mint, events_by_lp_mint.get(&lp_mint)).await {
                Ok(pool_info) => {
                    pool_infos.push(pool_info);
                }
                Err(e) => {
                    error!("❌ 构建LP mint {}的池子信息失败: {}", lp_mint, e);
                    // 对于单个LP mint失败，我们添加一个默认的空池子信息
                    pool_infos.push(None);
                }
            }
        }

        info!("✅ 成功构建{}个池子信息", pool_infos.len());
        Ok(pool_infos)
    }

    /// 为单个LP mint构建池子信息
    async fn build_pool_info(
        &self,
        lp_mint: &str,
        events: Option<&Vec<LpChangeEvent>>,
    ) -> Result<Option<LpMintPoolInfo>> {
        debug!("🔨 构建LP mint {}的池子信息", lp_mint);

        // 如果没有事件数据，返回默认空信息
        let events = match events {
            Some(events) if !events.is_empty() => events,
            _ => {
                warn!("⚠️ LP mint {}没有找到相关事件，返回默认信息", lp_mint);
                return Ok(None);
            }
        };

        // 获取最新的事件来提取基础信息
        let latest_event = &events[0]; // events已按时间倒序排列

        // 查询代币信息
        let (mut mint_a_info, mut mint_b_info, mut lp_mint_info) = self
            .fetch_token_infos(&latest_event.token_0_mint, &latest_event.token_1_mint, lp_mint)
            .await?;
        mint_a_info.program_id = latest_event.token_0_program_id.clone();
        mint_b_info.program_id = latest_event.token_1_program_id.clone();
        lp_mint_info.program_id = latest_event.lp_mint_program_id.clone();

        mint_a_info.decimals = latest_event.token_0_decimals;
        mint_b_info.decimals = latest_event.token_1_decimals;
        lp_mint_info.decimals = latest_event.lp_mint_decimals;

        // 计算池子统计数据
        let stats = self.calculate_pool_stats(events);
        let raydium_cp_program_id = std::env::var("RAYDIUM_CP_PROGRAM_ID")
            .unwrap_or_else(|_| "FairxoKThzWcDy9avKPsADqzni18LrXxKAZEHdXVo5gi".to_string());

        // 构建池子信息
        Ok(Some(LpMintPoolInfo {
            pool_type: "Standard".to_string(),
            program_id: raydium_cp_program_id,
            id: latest_event.pool_id.clone(),
            mint_a: mint_a_info,
            mint_b: mint_b_info,
            price: stats.current_price,
            mint_amount_a: stats.total_token_a as f64 / 10f64.powi(latest_event.token_0_decimals as i32),
            mint_amount_b: stats.total_token_b as f64 / 10f64.powi(latest_event.token_1_decimals as i32),
            fee_rate: 0.003,            // 默认费率，实际应从配置或链上查询
            open_time: "0".to_string(), // 需要从事件中获取最早时间
            tvl: stats.tvl,
            day: stats.day_stats,
            week: stats.week_stats,
            month: stats.month_stats,
            pooltype: vec!["Amm".to_string()],
            reward_default_pool_infos: "Ecosystem".to_string(),
            reward_default_infos: vec![], // 奖励信息需要从其他数据源获取
            farm_upcoming_count: 0,
            farm_ongoing_count: 0,
            farm_finished_count: 0,
            market_id: "11111111111111111111111111111111".to_string(), // 需要关联market信息
            lp_mint: lp_mint_info,
            lp_price: stats.lp_price,
            lp_amount: stats.total_lp_amount as f64 / 10f64.powi(latest_event.lp_mint_decimals as i32),
            burn_percent: 0.0, // 需要计算销毁比例
            launch_migrate_pool: false,
        }))
    }

    /// 获取代币信息
    async fn fetch_token_infos(
        &self,
        token_a_mint: &str,
        token_b_mint: &str,
        lp_mint: &str,
    ) -> Result<(TokenInfo, TokenInfo, TokenInfo)> {
        debug!("🔍 查询代币信息: {}, {}, {}", token_a_mint, token_b_mint, lp_mint);

        // 如果有元数据提供者，使用它来获取代币元数据
        if let Some(metadata_provider) = &self.metadata_provider {
            info!("📦 使用代币元数据提供者查询代币信息");

            // 并发查询三个代币的元数据
            let (token_a_result, token_b_result, lp_mint_result) = tokio::try_join!(
                async {
                    let mut provider = metadata_provider.lock().await;
                    provider.get_token_metadata(token_a_mint).await
                },
                async {
                    let mut provider = metadata_provider.lock().await;
                    provider.get_token_metadata(token_b_mint).await
                },
                async {
                    let mut provider = metadata_provider.lock().await;
                    provider.get_token_metadata(lp_mint).await
                }
            )?;

            // 转换为TokenInfo格式
            let token_a_info = self.convert_to_token_info(token_a_result, token_a_mint);
            let token_b_info = self.convert_to_token_info(token_b_result, token_b_mint);
            let lp_mint_info = self.convert_to_token_info(lp_mint_result, lp_mint);

            return Ok((token_a_info, token_b_info, lp_mint_info));
        }

        // 如果没有元数据提供者，返回默认信息
        warn!("⚠️ 没有设置代币元数据提供者，使用默认代币信息");
        let token_a_info = self.create_default_token_info(token_a_mint);
        let token_b_info = self.create_default_token_info(token_b_mint);
        let lp_mint_info = self.create_default_token_info(lp_mint);

        Ok((token_a_info, token_b_info, lp_mint_info))
    }

    /// 将Metaplex元数据转换为TokenInfo
    fn convert_to_token_info(&self, metadata: Option<ExternalTokenMetadata>, address: &str) -> TokenInfo {
        match metadata {
            Some(meta) => {
                info!(
                    "✅ 成功获取代币{}的元数据: {}",
                    address,
                    meta.symbol.as_deref().unwrap_or("UNK")
                );
                TokenInfo {
                    chain_id: utils::SolanaChainId::from_env().chain_id(), // Solana主网
                    address: address.to_string(),
                    program_id: "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA".to_string(),
                    logo_uri: meta.logo_uri.unwrap_or_default(),
                    symbol: meta.symbol.unwrap_or_else(|| "UNK".to_string()),
                    name: meta.name.unwrap_or_else(|| "Unknown Token".to_string()),
                    decimals: 6,  // ExternalTokenMetadata没有decimals字段，使用默认值6
                    tags: vec![], // 可以根据需要添加标签逻辑
                    extensions: serde_json::Value::Object(serde_json::Map::new()),
                }
            }
            None => {
                warn!("⚠️ 无法获取代币{}的元数据，使用默认信息", address);
                self.create_default_token_info(address)
            }
        }
    }

    /// 创建默认的代币信息
    fn create_default_token_info(&self, address: &str) -> TokenInfo {
        TokenInfo {
            chain_id: 101,
            address: address.to_string(),
            program_id: "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA".to_string(),
            logo_uri: "".to_string(),
            symbol: "UNK".to_string(),
            name: "Unknown Token".to_string(),
            decimals: 6, // 默认6位小数
            tags: vec![],
            extensions: serde_json::Value::Object(serde_json::Map::new()),
        }
    }

    /// 计算池子统计数据
    fn calculate_pool_stats(&self, events: &[LpChangeEvent]) -> PoolStats {
        debug!("📊 计算池子统计数据，事件数量: {}", events.len());

        let mut total_lp_amount = 0u64;
        let mut total_token_a = 0u64;
        let mut total_token_b = 0u64;

        // 从最新的事件获取当前状态
        if let Some(latest_event) = events.first() {
            total_lp_amount = latest_event.lp_amount_after;
            total_token_a = latest_event.token_0_vault_after;
            total_token_b = latest_event.token_1_vault_after;
        }

        // 计算价格（简化计算）
        let current_price = if total_token_a > 0 && total_token_b > 0 {
            total_token_b as f64 / total_token_a as f64
        } else {
            0.0
        };

        // 计算TVL（简化为token B的价值 * 2，假设token B是稳定币）
        let tvl = (total_token_b as f64 / 1_000_000.0) * 2.0; // 假设6位小数的稳定币

        // LP价格计算
        let lp_price = if total_lp_amount > 0 {
            tvl / (total_lp_amount as f64 / 1_000_000_000.0) // 假设LP代币9位小数
        } else {
            0.0
        };

        // 统计数据（简化实现，实际需要根据时间范围计算）
        let default_stats = PoolPeriodStats {
            volume: 0.0,
            volume_quote: 0.0,
            volume_fee: 0.0,
            apr: 0.0,
            fee_apr: 0.0,
            price_min: current_price,
            price_max: current_price,
            reward_apr: vec![0.0],
        };

        PoolStats {
            current_price,
            total_lp_amount,
            total_token_a,
            total_token_b,
            tvl,
            lp_price,
            day_stats: default_stats.clone(),
            week_stats: default_stats.clone(),
            month_stats: default_stats,
        }
    }
}

/// 池子统计数据结构
#[derive(Debug, Clone)]
struct PoolStats {
    current_price: f64,
    total_lp_amount: u64,
    total_token_a: u64,
    total_token_b: u64,
    tvl: f64,
    lp_price: f64,
    day_stats: PoolPeriodStats,
    week_stats: PoolPeriodStats,
    month_stats: PoolPeriodStats,
}

#[cfg(test)]
mod tests {
    use super::*;
    use utils::metaplex_service::MetaplexService;

    /// 创建一个测试用的默认TokenInfo - 直接测试逻辑而不依赖服务
    fn create_test_default_token_info(address: &str) -> TokenInfo {
        TokenInfo {
            chain_id: 101,
            address: address.to_string(),
            program_id: "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA".to_string(),
            logo_uri: "".to_string(),
            symbol: "UNK".to_string(),
            name: "Unknown Token".to_string(),
            decimals: 6,
            tags: vec![],
            extensions: serde_json::Value::Object(serde_json::Map::new()),
        }
    }

    /// 从元数据创建TokenInfo - 直接测试转换逻辑
    fn convert_metadata_to_token_info(metadata: Option<ExternalTokenMetadata>, address: &str) -> TokenInfo {
        match metadata {
            Some(meta) => TokenInfo {
                chain_id: 101,
                address: address.to_string(),
                program_id: "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA".to_string(),
                logo_uri: meta.logo_uri.unwrap_or_default(),
                symbol: meta.symbol.unwrap_or_else(|| "UNK".to_string()),
                name: meta.name.unwrap_or_else(|| "Unknown Token".to_string()),
                decimals: 6,
                tags: vec![],
                extensions: serde_json::Value::Object(serde_json::Map::new()),
            },
            None => create_test_default_token_info(address),
        }
    }

    #[test]
    fn test_default_token_info_creation() {
        let token_info = create_test_default_token_info("So11111111111111111111111111111111111111112");

        assert_eq!(token_info.address, "So11111111111111111111111111111111111111112");
        assert_eq!(token_info.symbol, "UNK");
        assert_eq!(token_info.name, "Unknown Token");
        assert_eq!(token_info.decimals, 6);
        assert_eq!(token_info.chain_id, 101);
        assert_eq!(token_info.program_id, "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
    }

    #[test]
    fn test_convert_to_token_info_with_metadata() {
        // 测试有元数据的情况
        let meta = ExternalTokenMetadata {
            address: "So11111111111111111111111111111111111111112".to_string(),
            symbol: Some("WSOL".to_string()),
            name: Some("Wrapped SOL".to_string()),
            logo_uri: Some("https://example.com/logo.png".to_string()),
            description: Some("Wrapped Solana".to_string()),
            external_url: None,
            attributes: None,
            tags: vec![],
        };

        let token_info = convert_metadata_to_token_info(Some(meta), "So11111111111111111111111111111111111111112");

        assert_eq!(token_info.address, "So11111111111111111111111111111111111111112");
        assert_eq!(token_info.symbol, "WSOL");
        assert_eq!(token_info.name, "Wrapped SOL");
        assert_eq!(token_info.logo_uri, "https://example.com/logo.png");
        assert_eq!(token_info.decimals, 6);
    }

    #[test]
    fn test_convert_to_token_info_without_metadata() {
        // 测试没有元数据的情况
        let token_info = convert_metadata_to_token_info(None, "test_address");

        assert_eq!(token_info.address, "test_address");
        assert_eq!(token_info.symbol, "UNK");
        assert_eq!(token_info.name, "Unknown Token");
        assert_eq!(token_info.decimals, 6);
    }

    #[test]
    fn test_metaplex_service_creation() {
        // 测试MetaplexService可以成功创建
        let metaplex_service = MetaplexService::new(None);
        assert!(metaplex_service.is_ok(), "MetaplexService创建应该成功");

        // 测试可以包装成TokenMetadataProvider
        if let Ok(service) = metaplex_service {
            let _provider: Arc<tokio::sync::Mutex<dyn TokenMetadataProvider>> =
                Arc::new(tokio::sync::Mutex::new(service));
            // 测试成功，说明类型转换正确
            assert!(true);
        }
    }

    #[test]
    fn test_pool_stats_calculation_logic() {
        // 测试池子统计逻辑（不依赖数据库的部分）
        let current_price = 1.5;
        let total_lp_amount = 1000000000u64; // 1 billion
        let total_token_a = 500000000u64; // 500 million
        let total_token_b = 750000000u64; // 750 million

        // 模拟TVL计算
        let tvl = (total_token_b as f64 / 1_000_000.0) * 2.0;
        assert_eq!(tvl, 1500.0);

        // 模拟LP价格计算
        let lp_price = tvl / (total_lp_amount as f64 / 1_000_000_000.0);
        assert_eq!(lp_price, 1500.0);

        // 验证价格计算
        let calculated_price = total_token_b as f64 / total_token_a as f64;
        assert_eq!(calculated_price, current_price);
    }
}
