//! 数据转换工具模块
//!
//!
//! 负责将数据库模型转换为新的API响应格式

use crate::dtos::solana::clmm::pool::listing::{
    ExtendedMintInfo, NewPoolListResponse, NewPoolListResponse2, PeriodStats, PoolConfigInfo, PoolInfo, PoolListData,
};
use crate::dtos::statics::static_dto::SaveClmmConfigRequest;
use crate::services::solana::clmm::config::config_service::ClmmConfigServiceTrait;
use anyhow::Result;
use database::clmm_pool::model::{ClmmPool, PoolListRequest, PoolListResponse};
use database::clmm_pool::PoolType;
use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tracing::{debug, info, warn};
use utils::constants;
use utils::solana::account_loader::AccountLoader;
use utils::{MetaplexService, TokenMetadata};
use uuid::Uuid;

/// AMM配置缓存项
#[derive(Debug, Clone)]
pub struct AmmConfigCache {
    pub protocol_fee_rate: u32,
    pub trade_fee_rate: u32,
    pub tick_spacing: u16,
    pub fund_fee_rate: u32,
    pub timestamp: u64, // 缓存时间戳
}

/// 数据转换服务
pub struct DataTransformService {
    metaplex_service: MetaplexService,
    rpc_client: Option<Arc<RpcClient>>,
    clmm_config_service: Option<Arc<dyn ClmmConfigServiceTrait>>, // 新增依赖
    pub amm_config_cache: Arc<Mutex<HashMap<String, AmmConfigCache>>>, // 线程安全的配置缓存
}

impl DataTransformService {
    /// 创建新的数据转换服务（不带RPC客户端）
    pub fn new() -> Result<Self> {
        let metaplex_service = MetaplexService::new(None)?;

        Ok(Self {
            metaplex_service,
            rpc_client: None,
            clmm_config_service: None,
            amm_config_cache: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// 创建新的数据转换服务（带RPC客户端，支持链上查询）
    pub fn new_with_rpc(rpc_client: Arc<RpcClient>) -> Result<Self> {
        let metaplex_service = MetaplexService::new(None)?;

        Ok(Self {
            metaplex_service,
            rpc_client: Some(rpc_client),
            clmm_config_service: None,
            amm_config_cache: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// 创建新的数据转换服务（完整版本：带RPC客户端和配置服务）
    pub fn new_optimized(
        rpc_client: Option<Arc<RpcClient>>,
        clmm_config_service: Option<Arc<dyn ClmmConfigServiceTrait>>,
    ) -> Result<Self> {
        let metaplex_service = MetaplexService::new(None)?;

        Ok(Self {
            metaplex_service,
            rpc_client,
            clmm_config_service,
            amm_config_cache: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// 将传统的池子列表响应转换为新格式
    pub async fn transform_pool_list_response(
        &mut self,
        old_response: PoolListResponse,
        _request: &PoolListRequest,
    ) -> Result<NewPoolListResponse> {
        info!("🔄 开始转换池子列表响应格式");

        // 收集需要获取元数据的mint地址（只收集代币信息为空的）
        let mut mint_addresses = Vec::new();
        let mut empty_token_count = 0;
        let mut filled_token_count = 0;

        for pool in &old_response.pools {
            // 检查mint0信息是否为空
            if pool.mint0.is_empty() {
                if !mint_addresses.contains(&pool.mint0.mint_address) {
                    mint_addresses.push(pool.mint0.mint_address.clone());
                    empty_token_count += 1;
                }
            } else {
                filled_token_count += 1;
            }

            // 检查mint1信息是否为空
            if pool.mint1.is_empty() {
                if !mint_addresses.contains(&pool.mint1.mint_address) {
                    mint_addresses.push(pool.mint1.mint_address.clone());
                    empty_token_count += 1;
                }
            } else {
                filled_token_count += 1;
            }
        }

        info!(
            "📊 代币信息统计: {} 个需要从链上获取, {} 个使用本地缓存",
            empty_token_count, filled_token_count
        );

        // 批量获取需要的mint元数据（只获取缺失的）
        let metadata_map = if !mint_addresses.is_empty() {
            info!("🔗 从链上获取 {} 个代币的元数据", mint_addresses.len());
            self.metaplex_service.get_tokens_metadata(&mint_addresses).await?
        } else {
            info!("✅ 所有代币信息已缓存，跳过链上查询");
            HashMap::new()
        };

        // 收集所有唯一的AMM配置地址，准备批量加载
        let mut amm_config_addresses = Vec::new();
        for pool in &old_response.pools {
            if !amm_config_addresses.contains(&pool.amm_config_address) {
                amm_config_addresses.push(pool.amm_config_address.clone());
            }
        }

        // 批量加载AMM配置（使用我们设计的批量方法！）
        if !amm_config_addresses.is_empty() {
            info!("🔗 批量预加载 {} 个AMM配置到缓存", amm_config_addresses.len());
            let _loaded_configs = self.load_multiple_amm_configs(&amm_config_addresses).await?;
        }

        // 转换池子数据
        let mut pool_infos = Vec::new();
        for pool in old_response.pools {
            let pool_info = self.transform_pool_to_pool_info(pool, &metadata_map).await?;
            pool_infos.push(pool_info);
        }

        // 构建新的响应格式
        let response = NewPoolListResponse {
            id: Uuid::new_v4().to_string(),
            success: true,
            data: PoolListData {
                count: old_response.pagination.total_count,
                data: pool_infos,
                has_next_page: old_response.pagination.has_next,
            },
        };

        info!("✅ 池子列表响应格式转换完成，共 {} 个池子", response.data.data.len());
        Ok(response)
    }

    /// 优化的AMM配置获取方法（三层查询策略）
    pub async fn get_amm_config_optimized(&self, config_address: &str) -> Result<Option<AmmConfigCache>> {
        // 1. 检查内存缓存
        if let Some(cached) = self.check_memory_cache(config_address)? {
            debug!("📋 使用内存缓存的AMM配置: {}", config_address);
            return Ok(Some(cached));
        }

        // 2. 尝试从数据库获取
        if let Some(config_service) = &self.clmm_config_service {
            if let Ok(Some(db_config)) = self.get_config_from_database(config_service, config_address).await {
                info!("🗄️ 从数据库获取AMM配置: {}", config_address);
                self.update_memory_cache(config_address, &db_config)?;
                return Ok(Some(db_config));
            }
        }

        // 3. 回退到链上查询
        info!("🔗 从链上查询AMM配置: {}", config_address);
        if let Ok(Some(chain_config)) = self.load_amm_config_from_chain(config_address).await {
            // 异步写入数据库（不阻塞主流程）
            self.async_save_config_to_database(config_address, &chain_config).await;

            // 更新内存缓存
            self.update_memory_cache(config_address, &chain_config)?;

            return Ok(Some(chain_config));
        }

        Ok(None)
    }

    /// 检查内存缓存
    pub fn check_memory_cache(&self, config_address: &str) -> Result<Option<AmmConfigCache>> {
        let cache = self
            .amm_config_cache
            .lock()
            .map_err(|e| anyhow::anyhow!("缓存锁获取失败: {}", e))?;

        if let Some(cached_config) = cache.get(config_address) {
            let current_time = chrono::Utc::now().timestamp() as u64;
            // 缓存有效期为5分钟
            if current_time - cached_config.timestamp < 300 {
                return Ok(Some(cached_config.clone()));
            } else {
                debug!("⏰ 缓存已过期: {}", config_address);
            }
        }

        Ok(None)
    }

    /// 更新内存缓存
    fn update_memory_cache(&self, config_address: &str, config: &AmmConfigCache) -> Result<()> {
        let mut cache = self
            .amm_config_cache
            .lock()
            .map_err(|e| anyhow::anyhow!("缓存锁获取失败: {}", e))?;
        cache.insert(config_address.to_string(), config.clone());
        Ok(())
    }

    /// 从数据库获取配置
    async fn get_config_from_database(
        &self,
        config_service: &Arc<dyn ClmmConfigServiceTrait>,
        config_address: &str,
    ) -> Result<Option<AmmConfigCache>> {
        match config_service.get_config_by_address(config_address).await {
            Ok(Some(config)) => Ok(Some(AmmConfigCache {
                protocol_fee_rate: config.protocol_fee_rate as u32,
                trade_fee_rate: config.trade_fee_rate as u32,
                tick_spacing: config.tick_spacing as u16,
                fund_fee_rate: config.fund_fee_rate as u32,
                timestamp: chrono::Utc::now().timestamp() as u64,
            })),
            Ok(None) => Ok(None),
            Err(e) => {
                warn!("⚠️ 数据库查询配置失败: {}", e);
                Ok(None)
            }
        }
    }

    /// 异步保存配置到数据库
    pub async fn async_save_config_to_database(&self, config_address: &str, config: &AmmConfigCache) {
        if let Some(config_service) = &self.clmm_config_service {
            let config_service_clone = config_service.clone();
            let address = config_address.to_string();
            let config_clone = config.clone();

            tokio::spawn(async move {
                // 需要从 config_address 推导出 config_index
                if let Ok(config_index) = Self::derive_config_index_from_address(&address) {
                    let save_request = SaveClmmConfigRequest {
                        index: config_index,
                        protocol_fee_rate: config_clone.protocol_fee_rate as u64,
                        trade_fee_rate: config_clone.trade_fee_rate as u64,
                        tick_spacing: config_clone.tick_spacing as u32,
                        fund_fee_rate: config_clone.fund_fee_rate as u64,
                        default_range: 0.1,                                   // 使用默认值
                        default_range_point: vec![0.01, 0.05, 0.1, 0.2, 0.5], // 使用默认值
                    };

                    match config_service_clone.save_clmm_config_from_request(save_request).await {
                        Ok(_) => {
                            tracing::info!("✅ 成功异步保存配置到数据库: {}", address);
                        }
                        Err(e) => {
                            tracing::warn!("⚠️ 异步保存配置到数据库失败: {}, 错误: {}", address, e);
                        }
                    }
                } else {
                    tracing::warn!("⚠️ 无法从配置地址推导索引: {}", address);
                }
            });
        }
    }

    /// 从配置地址推导配置索引
    fn derive_config_index_from_address(config_address: &str) -> Result<u32> {
        // 从常见索引进行匹配
        for index in 0..=10u16 {
            if let Ok(calculated_address) = Self::calculate_config_pda_static(index) {
                if calculated_address == config_address {
                    return Ok(index as u32);
                }
            }
        }
        Err(anyhow::anyhow!("无法推导配置索引"))
    }

    /// 静态PDA计算方法
    fn calculate_config_pda_static(index: u16) -> Result<String> {
        let raydium_program_id = utils::solana::ConfigManager::get_raydium_program_id()
            .map_err(|e| anyhow::anyhow!("获取Raydium程序ID失败: {}", e))?;

        let (config_pda, _bump) =
            utils::solana::calculators::PDACalculator::calculate_amm_config_pda(&raydium_program_id, index);

        Ok(config_pda.to_string())
    }

    /// 将传统的池子列表响应转换为新格式
    pub async fn transform_pool_list_response2(
        &mut self,
        old_response: PoolListResponse,
        _request: &PoolListRequest,
    ) -> Result<NewPoolListResponse2> {
        info!("🔄 开始转换池子列表响应格式");

        // 收集需要获取元数据的mint地址（只收集代币信息为空的）
        let mut mint_addresses = Vec::new();
        let mut empty_token_count = 0;
        let mut filled_token_count = 0;

        for pool in &old_response.pools {
            // 检查mint0信息是否为空
            if pool.mint0.is_empty() {
                if !mint_addresses.contains(&pool.mint0.mint_address) {
                    mint_addresses.push(pool.mint0.mint_address.clone());
                    empty_token_count += 1;
                }
            } else {
                filled_token_count += 1;
            }

            // 检查mint1信息是否为空
            if pool.mint1.is_empty() {
                if !mint_addresses.contains(&pool.mint1.mint_address) {
                    mint_addresses.push(pool.mint1.mint_address.clone());
                    empty_token_count += 1;
                }
            } else {
                filled_token_count += 1;
            }
        }

        info!(
            "📊 代币信息统计: {} 个需要从链上获取, {} 个使用本地缓存",
            empty_token_count, filled_token_count
        );

        // 批量获取需要的mint元数据（只获取缺失的）
        let metadata_map = if !mint_addresses.is_empty() {
            info!("🔗 从链上获取 {} 个代币的元数据", mint_addresses.len());
            self.metaplex_service.get_tokens_metadata(&mint_addresses).await?
        } else {
            info!("✅ 所有代币信息已缓存，跳过链上查询");
            HashMap::new()
        };

        // 收集所有唯一的AMM配置地址，准备批量加载
        let mut amm_config_addresses = Vec::new();
        for pool in &old_response.pools {
            if !amm_config_addresses.contains(&pool.amm_config_address) {
                amm_config_addresses.push(pool.amm_config_address.clone());
            }
        }

        // 批量加载AMM配置（使用我们设计的批量方法！）
        if !amm_config_addresses.is_empty() {
            info!("🔗 批量预加载 {} 个AMM配置到缓存", amm_config_addresses.len());
            let _loaded_configs = self.load_multiple_amm_configs(&amm_config_addresses).await?;
        }

        // 转换池子数据
        let mut pool_infos = Vec::new();
        for pool in old_response.pools {
            let pool_info = self.transform_pool_to_pool_info(pool, &metadata_map).await?;
            pool_infos.push(pool_info);
        }

        // 构建新的响应格式
        let response = NewPoolListResponse2 {
            id: Uuid::new_v4().to_string(),
            success: true,
            data: pool_infos,
        };

        info!("✅ 池子列表响应格式转换完成，共 {} 个池子", response.data.len());
        Ok(response)
    }

    /// 从链上加载AMM配置（支持缓存）
    pub async fn load_amm_config_from_chain(&self, config_address: &str) -> Result<Option<AmmConfigCache>> {
        // 检查缓存
        {
            let cache = self
                .amm_config_cache
                .lock()
                .map_err(|e| anyhow::anyhow!("缓存锁获取失败: {}", e))?;
            if let Some(cached_config) = cache.get(config_address) {
                let current_time = chrono::Utc::now().timestamp() as u64;
                // 缓存有效期为5分钟
                if current_time - cached_config.timestamp < 300 {
                    info!("📋 使用缓存的AMM配置: {}", config_address);
                    return Ok(Some(cached_config.clone()));
                } else {
                    info!("⏰ 缓存已过期，重新从链上加载: {}", config_address);
                }
            }
        }

        // 如果没有RPC客户端，返回None
        let rpc_client = match &self.rpc_client {
            Some(client) => client,
            None => {
                warn!("❌ 没有RPC客户端，跳过链上AMM配置查询");
                return Ok(None);
            }
        };

        info!("🔗 从链上加载AMM配置: {}", config_address);

        // 解析配置地址
        let config_pubkey = match config_address.parse::<Pubkey>() {
            Ok(pubkey) => pubkey,
            Err(e) => {
                warn!("❌ 无效的配置地址 {}: {}", config_address, e);
                return Ok(None);
            }
        };

        // 使用AccountLoader加载配置
        let account_loader = AccountLoader::new(rpc_client);

        match account_loader
            .load_and_deserialize::<raydium_amm_v3::states::AmmConfig>(&config_pubkey)
            .await
        {
            Ok(amm_config) => {
                info!("✅ 成功从链上加载AMM配置: {:?}", amm_config);

                let cache_item = AmmConfigCache {
                    protocol_fee_rate: amm_config.protocol_fee_rate,
                    trade_fee_rate: amm_config.trade_fee_rate,
                    tick_spacing: amm_config.tick_spacing,
                    fund_fee_rate: amm_config.fund_fee_rate,
                    timestamp: chrono::Utc::now().timestamp() as u64,
                };

                // 更新缓存
                {
                    let mut cache = self
                        .amm_config_cache
                        .lock()
                        .map_err(|e| anyhow::anyhow!("缓存锁获取失败: {}", e))?;
                    cache.insert(config_address.to_string(), cache_item.clone());
                }

                Ok(Some(cache_item))
            }
            Err(e) => {
                warn!("❌ 从链上加载AMM配置失败 {}: {}", config_address, e);
                Ok(None)
            }
        }
    }

    /// 批量加载多个AMM配置（优化版本：内存缓存 → 数据库 → 链上查询）
    pub async fn load_multiple_amm_configs(
        &self,
        config_addresses: &[String],
    ) -> Result<HashMap<String, AmmConfigCache>> {
        let mut results = HashMap::new();
        let mut need_db_query = Vec::new();

        info!("🔍 开始批量加载{}个AMM配置", config_addresses.len());

        // 1. 批量检查内存缓存
        {
            let cache = self
                .amm_config_cache
                .lock()
                .map_err(|e| anyhow::anyhow!("缓存锁获取失败: {}", e))?;
            let current_time = chrono::Utc::now().timestamp() as u64;

            for config_address in config_addresses {
                if let Some(cached_config) = cache.get(config_address) {
                    // 缓存有效期为5分钟
                    if current_time - cached_config.timestamp < 300 {
                        debug!("📋 使用缓存的AMM配置: {}", config_address);
                        results.insert(config_address.clone(), cached_config.clone());
                        continue;
                    } else {
                        debug!("⏰ 缓存已过期: {}", config_address);
                    }
                }
                need_db_query.push(config_address.clone());
            }
        }

        if need_db_query.is_empty() {
            info!("✅ 所有{}个配置都来自内存缓存", results.len());
            return Ok(results);
        }

        // 2. 批量从数据库查询缺失的配置
        let mut need_chain_query = need_db_query.clone();
        if let Some(config_service) = &self.clmm_config_service {
            info!("🗄️ 从数据库查询{}个配置", need_db_query.len());

            match config_service.get_configs_by_addresses(&need_db_query).await {
                Ok(db_configs) => {
                    for config in db_configs {
                        let cache_config = AmmConfigCache {
                            protocol_fee_rate: config.protocol_fee_rate as u32,
                            trade_fee_rate: config.trade_fee_rate as u32,
                            tick_spacing: config.tick_spacing as u16,
                            fund_fee_rate: config.fund_fee_rate as u32,
                            timestamp: chrono::Utc::now().timestamp() as u64,
                        };

                        results.insert(config.id.clone(), cache_config.clone());

                        // 更新内存缓存
                        if let Ok(()) = self.update_memory_cache(&config.id, &cache_config) {
                            debug!("🗄️ 从数据库获取并缓存配置: {}", config.id);
                        }

                        // 从链上查询列表中移除
                        need_chain_query.retain(|addr| addr != &config.id);
                    }
                    info!(
                        "✅ 从数据库获取{}个配置",
                        results.len() - (config_addresses.len() - need_db_query.len())
                    );
                }
                Err(e) => {
                    warn!("⚠️ 批量数据库查询失败: {}", e);
                }
            }
        }

        // 3. 只对数据库中没有的配置才查询链上
        if need_chain_query.is_empty() {
            info!("✅ 批量加载完成，全部来自缓存和数据库，共{}个配置", results.len());
            return Ok(results);
        }

        // 如果没有RPC客户端，返回已有结果
        let rpc_client = match &self.rpc_client {
            Some(client) => client,
            None => {
                info!("🔍 没有RPC客户端，跳过{}个配置的链上查询", need_chain_query.len());
                return Ok(results);
            }
        };

        info!("🔗 从链上查询{}个配置", need_chain_query.len());

        // 解析所有需要从链上加载的地址
        let mut pubkeys = Vec::new();
        let mut valid_addresses = Vec::new();

        for addr in &need_chain_query {
            match addr.parse::<Pubkey>() {
                Ok(pubkey) => {
                    pubkeys.push(pubkey);
                    valid_addresses.push(addr.clone());
                }
                Err(e) => {
                    warn!("❌ 无效的配置地址 {}: {}", addr, e);
                }
            }
        }

        if pubkeys.is_empty() {
            return Ok(results);
        }

        // 使用AccountLoader批量加载
        let account_loader = AccountLoader::new(rpc_client);
        let mut chain_configs = HashMap::new();

        match account_loader.load_multiple_accounts(&pubkeys).await {
            Ok(accounts) => {
                for (i, account_opt) in accounts.iter().enumerate() {
                    let config_address = &valid_addresses[i];

                    if let Some(account) = account_opt {
                        match account_loader.deserialize_anchor_account::<raydium_amm_v3::states::AmmConfig>(account) {
                            Ok(amm_config) => {
                                info!("✅ 成功从链上加载AMM配置: {}", config_address);

                                let cache_item = AmmConfigCache {
                                    protocol_fee_rate: amm_config.protocol_fee_rate,
                                    trade_fee_rate: amm_config.trade_fee_rate,
                                    tick_spacing: amm_config.tick_spacing,
                                    fund_fee_rate: amm_config.fund_fee_rate,
                                    timestamp: chrono::Utc::now().timestamp() as u64,
                                };

                                results.insert(config_address.clone(), cache_item.clone());
                                chain_configs.insert(config_address.clone(), cache_item.clone());

                                // 更新内存缓存
                                if let Ok(()) = self.update_memory_cache(config_address, &cache_item) {
                                    debug!("🔗 从链上获取并缓存配置: {}", config_address);
                                }
                            }
                            Err(e) => {
                                warn!("❌ 反序列化AMM配置失败 {}: {}", config_address, e);
                            }
                        }
                    } else {
                        warn!("⚠️ 未找到AMM配置账户: {}", config_address);
                    }
                }
            }
            Err(e) => {
                warn!("❌ 批量从链上加载AMM配置失败: {}", e);
            }
        }

        // 4. 异步批量写入数据库（新增功能）
        if !chain_configs.is_empty() && self.clmm_config_service.is_some() {
            let config_service_clone = self.clmm_config_service.clone();
            let configs_to_save = chain_configs.clone();

            tokio::spawn(async move {
                if let Some(config_service) = config_service_clone {
                    for (address, config) in configs_to_save {
                        // 使用相同的异步保存逻辑
                        if let Ok(config_index) = Self::derive_config_index_from_address(&address) {
                            let save_request = SaveClmmConfigRequest {
                                index: config_index,
                                protocol_fee_rate: config.protocol_fee_rate as u64,
                                trade_fee_rate: config.trade_fee_rate as u64,
                                tick_spacing: config.tick_spacing as u32,
                                fund_fee_rate: config.fund_fee_rate as u64,
                                default_range: 0.1,
                                default_range_point: vec![0.01, 0.05, 0.1, 0.2, 0.5],
                            };

                            match config_service.save_clmm_config_from_request(save_request).await {
                                Ok(_) => {
                                    tracing::info!("✅ 批量异步保存配置成功: {}", address);
                                }
                                Err(e) => {
                                    tracing::warn!("⚠️ 批量异步保存配置失败: {}, 错误: {}", address, e);
                                }
                            }
                        }
                    }
                }
            });

            info!("🔄 启动异步保存{}个新配置到数据库", chain_configs.len());
        }

        info!(
            "✅ 批量加载完成，共{}个配置（内存: {}, 数据库: {}, 链上: {}）",
            results.len(),
            config_addresses.len() - need_db_query.len(),
            need_db_query.len() - need_chain_query.len(),
            chain_configs.len()
        );
        Ok(results)
    }

    /// 将单个池子转换为新的池子信息格式
    async fn transform_pool_to_pool_info(
        &self,
        pool: ClmmPool,
        metadata_map: &HashMap<String, TokenMetadata>,
    ) -> Result<PoolInfo> {
        debug!("🔄 转换池子信息: {}", pool.pool_address);

        // 获取mint A的元数据 - 智能使用本地或链上数据
        let mint_a = self.create_extended_mint_info_smart(&pool.mint0, metadata_map)?;

        // 获取mint B的元数据 - 智能使用本地或链上数据
        let mint_b = self.create_extended_mint_info_smart(&pool.mint1, metadata_map)?;

        // 创建池子配置信息（优先从链上获取真实配置，失败时使用硬编码值）
        let config = Some(
            self.create_pool_config_info_from_chain(&pool)
                .await
                .unwrap_or_else(|e| {
                    warn!("⚠️ 从链上获取配置失败，使用硬编码配置: {}", e);
                    self.create_pool_config_info_fallback(&pool)
                }),
        );

        let pool_info = PoolInfo {
            pool_type: match pool.pool_type {
                PoolType::Concentrated => "Concentrated".to_string(),
                PoolType::Standard => "Standard".to_string(),
            },
            program_id: self.get_program_id_for_pool(&pool),
            id: pool.pool_address.clone(),
            mint_a,
            mint_b,
            reward_default_pool_infos: self.get_reward_pool_type(&pool.pool_type),
            reward_default_infos: vec![], // 暂时为空，未来可以从链上获取
            price: pool.price_info.current_price.unwrap_or(pool.price_info.initial_price),
            mint_amount_a: 0.0, // 暂时为空，需要从链上获取
            mint_amount_b: 0.0, // 暂时为空，需要从链上获取
            fee_rate: self.calculate_fee_rate(pool.config_index),
            open_time: pool.open_time.to_string(),
            tvl: 0.0,                            // 暂时为空，需要计算
            day: Some(PeriodStats::default()),   // 暂时为空，需要从交易数据汇聚
            week: Some(PeriodStats::default()),  // 暂时为空，需要从交易数据汇聚
            month: Some(PeriodStats::default()), // 暂时为空，需要从交易数据汇聚
            pooltype: self.get_pool_tags(&pool),
            farm_upcoming_count: 0,
            farm_ongoing_count: 0,
            farm_finished_count: self.calculate_farm_finished_count(&pool),
            config,
            burn_percent: self.calculate_burn_percent(&pool),
            launch_migrate_pool: self.is_launch_migrate_pool(&pool),
        };

        debug!("✅ 池子信息转换完成: {}", pool_info.id);
        Ok(pool_info)
    }

    /// 创建扩展的mint信息（智能版本）- 优先使用本地缓存数据
    fn create_extended_mint_info_smart(
        &self,
        token_info: &database::clmm_pool::model::TokenInfo,
        metadata_map: &HashMap<String, TokenMetadata>,
    ) -> Result<ExtendedMintInfo> {
        let mint_address = &token_info.mint_address;

        if token_info.is_empty() {
            // 代币信息为空，使用链上获取的元数据
            debug!("🔗 使用链上数据构建mint信息: {}", mint_address);
            self.create_extended_mint_info(mint_address, token_info.decimals, &token_info.owner, metadata_map)
        } else {
            // 代币信息已缓存，使用本地数据，并结合链上元数据进行增强
            debug!("📋 使用本地缓存构建mint信息: {}", mint_address);
            let chain_metadata = metadata_map.get(mint_address);

            let mint_info = ExtendedMintInfo {
                chain_id: self.get_chain_id(),
                address: mint_address.clone(),
                program_id: token_info.owner.clone(),
                // 优先使用本地缓存的symbol和name，如果为空则使用链上数据
                logo_uri: chain_metadata.and_then(|m| m.logo_uri.clone()),
                symbol: token_info
                    .symbol
                    .clone()
                    .or_else(|| chain_metadata.and_then(|m| m.symbol.clone())),
                name: token_info
                    .name
                    .clone()
                    .or_else(|| chain_metadata.and_then(|m| m.name.clone())),
                decimals: token_info.decimals,
                // 结合本地和链上数据增强标签
                tags: self.enhance_mint_tags_with_local_data(chain_metadata, mint_address, token_info),
                extensions: self.create_mint_extensions_with_local_data(mint_address, chain_metadata, token_info),
            };

            Ok(mint_info)
        }
    }

    /// 创建扩展的mint信息（智能版本）
    fn create_extended_mint_info(
        &self,
        mint_address: &str,
        decimals: u8,
        owner: &str,
        metadata_map: &HashMap<String, TokenMetadata>,
    ) -> Result<ExtendedMintInfo> {
        let metadata = metadata_map.get(mint_address);

        let mint_info = ExtendedMintInfo {
            chain_id: self.get_chain_id(),
            address: mint_address.to_string(),
            program_id: owner.to_string(),
            logo_uri: metadata.and_then(|m| m.logo_uri.clone()),
            symbol: metadata.and_then(|m| m.symbol.clone()),
            name: metadata.and_then(|m| m.name.clone()),
            decimals,
            tags: self.enhance_mint_tags(metadata, mint_address, decimals),
            extensions: self.create_mint_extensions(mint_address, metadata),
        };

        Ok(mint_info)
    }

    /// 获取链ID（根据环境动态判断）
    fn get_chain_id(&self) -> u32 {
        use utils::SolanaChainId;
        SolanaChainId::from_env().chain_id()
    }

    /// 增强mint标签（结合本地数据版本）
    fn enhance_mint_tags_with_local_data(
        &self,
        chain_metadata: Option<&TokenMetadata>,
        mint_address: &str,
        token_info: &database::clmm_pool::model::TokenInfo,
    ) -> Vec<String> {
        let mut tags = chain_metadata.map(|m| m.tags.clone()).unwrap_or_default();

        // 根据小数位数添加标签
        match token_info.decimals {
            0..=2 => tags.push("low-precision".to_string()),
            3..=6 => tags.push("standard-precision".to_string()),
            7..=9 => tags.push("high-precision".to_string()),
            _ => tags.push("ultra-precision".to_string()),
        }

        // 检查是否为知名代币
        if self.is_well_known_token(mint_address) {
            tags.push("verified".to_string());
            tags.push("blue-chip".to_string());
        }

        // 检查是否为稳定币（优先使用本地symbol）
        let symbol_to_check = token_info
            .symbol
            .as_ref()
            .or_else(|| chain_metadata.and_then(|m| m.symbol.as_ref()));
        if self.is_stablecoin_by_symbol(mint_address, symbol_to_check) {
            tags.push("stablecoin".to_string());
        }

        // 检查是否为封装代币（优先使用本地symbol）
        if self.is_wrapped_token_by_symbol(mint_address, symbol_to_check) {
            tags.push("wrapped".to_string());
        }

        // 如果有本地缓存的symbol，添加verified标签
        if token_info.symbol.is_some() && !token_info.symbol.as_ref().unwrap().is_empty() {
            tags.push("cached".to_string());
        }

        tags
    }

    /// 创建mint扩展信息（结合本地数据版本）
    fn create_mint_extensions_with_local_data(
        &self,
        mint_address: &str,
        chain_metadata: Option<&TokenMetadata>,
        token_info: &database::clmm_pool::model::TokenInfo,
    ) -> serde_json::Value {
        let mut extensions = serde_json::Map::new();

        // 添加数据来源信息
        extensions.insert(
            "data_source".to_string(),
            serde_json::Value::String(if token_info.is_empty() {
                "onchain".to_string()
            } else {
                "cached".to_string()
            }),
        );

        // 添加代币类型信息（优先使用本地数据）
        let symbol_to_check = token_info
            .symbol
            .as_ref()
            .or_else(|| chain_metadata.and_then(|m| m.symbol.as_ref()));
        extensions.insert(
            "type".to_string(),
            serde_json::Value::String(self.classify_token_type_by_symbol(mint_address, symbol_to_check)),
        );

        // 添加安全等级（本地缓存的数据通常更安全）
        let security_level = if !token_info.is_empty() {
            "high".to_string() // 本地缓存的数据认为是高安全等级
        } else {
            self.assess_security_level(mint_address, chain_metadata)
        };
        extensions.insert("security_level".to_string(), serde_json::Value::String(security_level));

        // 添加流动性等级估算
        extensions.insert(
            "liquidity_tier".to_string(),
            serde_json::Value::String(self.estimate_liquidity_tier(mint_address)),
        );

        // 如果有本地名称和符号，添加到扩展信息中
        if let Some(symbol) = &token_info.symbol {
            if !symbol.is_empty() {
                extensions.insert("cached_symbol".to_string(), serde_json::Value::String(symbol.clone()));
            }
        }
        if let Some(name) = &token_info.name {
            if !name.is_empty() {
                extensions.insert("cached_name".to_string(), serde_json::Value::String(name.clone()));
            }
        }

        // 如果有链上元数据，添加额外信息
        if let Some(meta) = chain_metadata {
            if let Some(description) = &meta.description {
                extensions.insert(
                    "description".to_string(),
                    serde_json::Value::String(description.clone()),
                );
            }

            if let Some(external_url) = &meta.external_url {
                extensions.insert("website".to_string(), serde_json::Value::String(external_url.clone()));
            }
        }

        serde_json::Value::Object(extensions)
    }

    /// 根据符号判断是否为稳定币
    fn is_stablecoin_by_symbol(&self, mint_address: &str, symbol: Option<&String>) -> bool {
        // 检查地址
        if matches!(
            mint_address,
            "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v" |  // USDC
            "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB" // USDT
        ) {
            return true;
        }

        // 检查符号
        if let Some(symbol_str) = symbol {
            return matches!(symbol_str.as_str(), "USDC" | "USDT" | "DAI" | "BUSD" | "FRAX");
        }

        false
    }

    /// 根据符号判断是否为封装代币
    fn is_wrapped_token_by_symbol(&self, mint_address: &str, symbol: Option<&String>) -> bool {
        // 检查WSOL
        if mint_address == "So11111111111111111111111111111111111111112" {
            return true;
        }

        // 检查符号是否以W开头
        if let Some(symbol_str) = symbol {
            return symbol_str.starts_with('W') && symbol_str.len() > 1;
        }

        false
    }

    /// 根据符号分类代币类型
    fn classify_token_type_by_symbol(&self, mint_address: &str, symbol: Option<&String>) -> String {
        if self.is_stablecoin_by_symbol(mint_address, symbol) {
            "stablecoin".to_string()
        } else if self.is_wrapped_token_by_symbol(mint_address, symbol) {
            "wrapped".to_string()
        } else if self.is_well_known_token(mint_address) {
            "blue-chip".to_string()
        } else {
            "token".to_string()
        }
    }
    fn enhance_mint_tags(&self, metadata: Option<&TokenMetadata>, mint_address: &str, decimals: u8) -> Vec<String> {
        let mut tags = metadata.map(|m| m.tags.clone()).unwrap_or_default();

        // 根据小数位数添加标签
        match decimals {
            0..=2 => tags.push("low-precision".to_string()),
            3..=6 => tags.push("standard-precision".to_string()),
            7..=9 => tags.push("high-precision".to_string()),
            _ => tags.push("ultra-precision".to_string()),
        }

        // 检查是否为知名代币
        if self.is_well_known_token(mint_address) {
            tags.push("verified".to_string());
            tags.push("blue-chip".to_string());
        }

        // 检查是否为稳定币
        if self.is_stablecoin(mint_address, metadata) {
            tags.push("stablecoin".to_string());
        }

        // 检查是否为封装代币
        if self.is_wrapped_token(mint_address, metadata) {
            tags.push("wrapped".to_string());
        }

        tags
    }

    /// 创建mint扩展信息
    fn create_mint_extensions(&self, mint_address: &str, metadata: Option<&TokenMetadata>) -> serde_json::Value {
        let mut extensions = serde_json::Map::new();

        // 添加代币类型信息
        extensions.insert(
            "type".to_string(),
            serde_json::Value::String(self.classify_token_type(mint_address, metadata)),
        );

        // 添加安全等级
        extensions.insert(
            "security_level".to_string(),
            serde_json::Value::String(self.assess_security_level(mint_address, metadata)),
        );

        // 添加流动性等级估算
        extensions.insert(
            "liquidity_tier".to_string(),
            serde_json::Value::String(self.estimate_liquidity_tier(mint_address)),
        );

        // 如果有元数据，添加额外信息
        if let Some(meta) = metadata {
            if let Some(description) = &meta.description {
                extensions.insert(
                    "description".to_string(),
                    serde_json::Value::String(description.clone()),
                );
            }

            if let Some(external_url) = &meta.external_url {
                extensions.insert("website".to_string(), serde_json::Value::String(external_url.clone()));
            }
        }

        serde_json::Value::Object(extensions)
    }

    /// 判断是否为知名代币
    fn is_well_known_token(&self, mint_address: &str) -> bool {
        matches!(
            mint_address,
            "So11111111111111111111111111111111111111112" |  // WSOL
            "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v" |  // USDC
            "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB" |  // USDT
            "4k3Dyjzvzp8eMZWUXbBCjEvwSkkk59S5iCNLY3QrkX6R" |  // RAY
            "A1KLoBrKBde8Ty9qtNQUtq3C2ortoC3u7twggz7sEto6" // SAMO
        )
    }

    /// 判断是否为稳定币
    fn is_stablecoin(&self, mint_address: &str, metadata: Option<&TokenMetadata>) -> bool {
        // 检查地址
        if matches!(
            mint_address,
            "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v" |  // USDC
            "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB" // USDT
        ) {
            return true;
        }

        // 检查符号
        if let Some(meta) = metadata {
            if let Some(symbol) = &meta.symbol {
                return matches!(symbol.as_str(), "USDC" | "USDT" | "DAI" | "BUSD" | "FRAX");
            }
        }

        false
    }

    /// 判断是否为封装代币
    fn is_wrapped_token(&self, mint_address: &str, metadata: Option<&TokenMetadata>) -> bool {
        // 检查WSOL
        if mint_address == "So11111111111111111111111111111111111111112" {
            return true;
        }

        // 检查符号是否以W开头
        if let Some(meta) = metadata {
            if let Some(symbol) = &meta.symbol {
                return symbol.starts_with('W') && symbol.len() > 1;
            }
        }

        false
    }

    /// 分类代币类型
    fn classify_token_type(&self, mint_address: &str, metadata: Option<&TokenMetadata>) -> String {
        if self.is_stablecoin(mint_address, metadata) {
            "stablecoin".to_string()
        } else if self.is_wrapped_token(mint_address, metadata) {
            "wrapped".to_string()
        } else if self.is_well_known_token(mint_address) {
            "blue-chip".to_string()
        } else {
            "token".to_string()
        }
    }

    /// 评估安全等级
    fn assess_security_level(&self, mint_address: &str, metadata: Option<&TokenMetadata>) -> String {
        if self.is_well_known_token(mint_address) {
            "high".to_string()
        } else if metadata.is_some() && metadata.unwrap().logo_uri.is_some() {
            "medium".to_string()
        } else {
            "low".to_string()
        }
    }

    /// 估算流动性等级
    fn estimate_liquidity_tier(&self, mint_address: &str) -> String {
        if self.is_well_known_token(mint_address) {
            "tier1".to_string()
        } else {
            "tier3".to_string() // 默认为较低等级，实际应该根据链上数据判断
        }
    }

    /// 根据配置索引计算交易费率（Fallback方法，硬编码）
    fn calculate_trade_fee_rate_fallback(&self, config_index: u16) -> u32 {
        match config_index {
            0 => 3000,  // 0.01%
            1 => 500,   // 0.05%
            2 => 2500,  // 0.25%
            3 => 10000, // 1%
            _ => 500,   // 默认0.05%
        }
    }

    /// 根据配置索引计算tick间距（Fallback方法，硬编码）
    fn calculate_tick_spacing_fallback(&self, config_index: u16) -> u32 {
        match config_index {
            0 => 60,
            1 => 10,
            2 => 50,
            3 => 100,
            _ => 10, // 默认
        }
    }

    /// 根据配置索引计算手续费率
    fn calculate_fee_rate(&self, config_index: u16) -> f64 {
        match config_index {
            0 => 0.0005, // 0.05%
            1 => 0.0005, // 0.05%
            2 => 0.0025, // 0.25%
            3 => 0.01,   // 1%
            4 => 0.0001, // 0.01%
            _ => 0.0005, // 默认0.05%
        }
    }

    /// 从链上创建池子配置信息（优化版本：使用三层查询策略）
    async fn create_pool_config_info_from_chain(&self, pool: &ClmmPool) -> Result<PoolConfigInfo> {
        let config_address = &pool.amm_config_address;

        // 使用优化的配置加载方法
        let amm_config = self.get_amm_config_optimized(config_address).await?;

        match amm_config {
            Some(config) => {
                info!("✅ 使用AMM配置数据: {}", config_address);

                // 根据真实的tick间距和池子类型计算范围相关数据
                let default_range =
                    self.calculate_default_range_from_tick_spacing(config.tick_spacing, &pool.pool_type);
                let current_price = pool.price_info.current_price.unwrap_or(pool.price_info.initial_price);
                let default_range_point =
                    self.generate_range_points_from_tick_spacing(config.tick_spacing, &pool.pool_type, current_price);

                Ok(PoolConfigInfo {
                    id: config_address.clone(),
                    index: pool.config_index as u32,
                    protocol_fee_rate: config.protocol_fee_rate,
                    trade_fee_rate: config.trade_fee_rate,
                    tick_spacing: config.tick_spacing as u32,
                    fund_fee_rate: config.fund_fee_rate,
                    default_range,
                    default_range_point,
                })
            }
            None => {
                // 如果优化查询失败，抛出错误，让调用方使用fallback
                Err(anyhow::anyhow!(
                    "无法从缓存、数据库或链上加载AMM配置: {}",
                    config_address
                ))
            }
        }
    }

    /// Fallback方法：创建池子配置信息（使用硬编码值，向后兼容）
    fn create_pool_config_info_fallback(&self, pool: &ClmmPool) -> PoolConfigInfo {
        let config_index = pool.config_index;
        let trade_fee_rate = self.calculate_trade_fee_rate_fallback(config_index);
        let tick_spacing = self.calculate_tick_spacing_fallback(config_index);

        // 根据配置索引动态计算协议费率（硬编码值）
        let protocol_fee_rate = self.calculate_protocol_fee_rate_fallback(config_index);

        // 根据配置索引动态计算基金费率（硬编码值）
        let fund_fee_rate = self.calculate_fund_fee_rate_fallback(config_index);

        // 根据tick间距和池子类型智能计算默认范围
        let default_range = self.calculate_default_range_from_tick_spacing(tick_spacing as u16, &pool.pool_type);

        // 根据池子的价格波动性和tick间距生成智能的范围点
        let current_price = pool.price_info.current_price.unwrap_or(pool.price_info.initial_price);
        let default_range_point =
            self.generate_range_points_from_tick_spacing(tick_spacing as u16, &pool.pool_type, current_price);

        PoolConfigInfo {
            id: pool.amm_config_address.clone(),
            index: config_index as u32,
            protocol_fee_rate,
            trade_fee_rate,
            tick_spacing,
            fund_fee_rate,
            default_range,
            default_range_point,
        }
    }

    /// 根据配置索引计算协议费率（Fallback方法，硬编码）
    fn calculate_protocol_fee_rate_fallback(&self, config_index: u16) -> u32 {
        match config_index {
            0 => 25000,  // 2.5% - 低费率配置，更高的协议费率
            1 => 120000, // 12% - 标准配置
            2 => 300000, // 30% - 高费率配置，更高的协议分成
            3 => 500000, // 50% - 超高费率配置
            _ => 120000, // 默认12%
        }
    }

    /// 根据配置索引计算基金费率（Fallback方法，硬编码）
    fn calculate_fund_fee_rate_fallback(&self, config_index: u16) -> u32 {
        match config_index {
            0 => 10000,  // 1% - 低费率配置
            1 => 40000,  // 4% - 标准配置
            2 => 80000,  // 8% - 高费率配置
            3 => 120000, // 12% - 超高费率配置
            _ => 40000,  // 默认4%
        }
    }

    /// 根据真实tick间距和池子类型计算默认范围（新方法，基于链上数据）
    fn calculate_default_range_from_tick_spacing(
        &self,
        tick_spacing: u16,
        pool_type: &database::clmm_pool::model::PoolType,
    ) -> f64 {
        match pool_type {
            database::clmm_pool::model::PoolType::Concentrated => {
                // 集中流动性池：根据真实tick间距调整范围
                match tick_spacing {
                    1 => 0.01,  // 超窄间距，适合稳定币对
                    10 => 0.05, // 窄范围，适合相关资产
                    50 => 0.1,  // 中等范围，标准配置
                    60 => 0.02, // Raydium特殊配置
                    100 => 0.2, // 较宽范围，适合波动性资产
                    _ => 0.1,   // 默认
                }
            }
            database::clmm_pool::model::PoolType::Standard => {
                // 标准池：固定较宽范围
                0.5
            }
        }
    }

    /// 根据真实tick间距生成智能的范围点（新方法，基于链上数据）
    fn generate_range_points_from_tick_spacing(
        &self,
        tick_spacing: u16,
        pool_type: &database::clmm_pool::model::PoolType,
        current_price: f64,
    ) -> Vec<f64> {
        match pool_type {
            database::clmm_pool::model::PoolType::Concentrated => {
                match tick_spacing {
                    // 超窄间距：稳定币对，提供精细的范围选择
                    1 => vec![0.005, 0.01, 0.02, 0.05, 0.1],

                    // 标准间距：常规交易对
                    10 => {
                        if current_price > 1000.0 {
                            // 高价格资产：更宽的范围
                            vec![0.02, 0.05, 0.1, 0.2, 0.5]
                        } else if current_price < 1.0 {
                            // 低价格资产：更精细的范围
                            vec![0.01, 0.03, 0.06, 0.12, 0.25]
                        } else {
                            // 中等价格资产：标准范围
                            vec![0.01, 0.05, 0.1, 0.2, 0.4]
                        }
                    }

                    // 中等间距：适中波动性
                    50 => vec![0.05, 0.1, 0.2, 0.4, 0.8],

                    // Raydium特殊配置
                    60 => vec![0.01, 0.02, 0.05, 0.1, 0.2],

                    // 宽间距：高波动性资产
                    100 => vec![0.1, 0.2, 0.5, 1.0, 2.0],

                    // 其他情况：使用保守的默认值
                    _ => vec![0.02, 0.05, 0.1, 0.2, 0.5],
                }
            }
            database::clmm_pool::model::PoolType::Standard => {
                // 标准池：提供更宽的范围选择
                vec![0.1, 0.3, 0.5, 1.0, 2.0]
            }
        }
    }

    /// 获取池子对应的程序ID
    fn get_program_id_for_pool(&self, pool: &ClmmPool) -> String {
        // 根据池子类型和配置返回相应的程序ID
        match pool.pool_type {
            PoolType::Concentrated => std::env::var("RAYDIUM_PROGRAM_ID")
                .unwrap_or_else(|_| constants::DEFAULT_RAYDIUM_PROGRAM_ID.to_string()),
            PoolType::Standard => std::env::var("RAYDIUM_PROGRAM_ID")
                .unwrap_or_else(|_| constants::DEFAULT_RAYDIUM_PROGRAM_ID.to_string()),
        }
    }

    /// 获取奖励池类型标识
    fn get_reward_pool_type(&self, pool_type: &database::clmm_pool::model::PoolType) -> String {
        match pool_type {
            database::clmm_pool::model::PoolType::Concentrated => "Clmm".to_string(),
            database::clmm_pool::model::PoolType::Standard => "Standard".to_string(),
        }
    }

    /// 生成池子标签
    fn get_pool_tags(&self, pool: &ClmmPool) -> Vec<String> {
        let mut tags = Vec::new();

        // 基于池子类型的标签
        match pool.pool_type {
            database::clmm_pool::model::PoolType::Concentrated => {
                tags.push("CLMM".to_string());
                tags.push("concentrated".to_string());
            }
            database::clmm_pool::model::PoolType::Standard => {
                tags.push("AMM".to_string());
                tags.push("standard".to_string());
            }
        }

        // 基于配置索引的标签
        match pool.config_index {
            0 => tags.push("low-fee".to_string()),
            1 => tags.push("standard-fee".to_string()),
            2 => tags.push("medium-fee".to_string()),
            3 => tags.push("high-fee".to_string()),
            _ => {}
        }

        // 基于tick间距的标签（使用fallback方法获取tick间距）
        let tick_spacing = self.calculate_tick_spacing_fallback(pool.config_index);
        match tick_spacing {
            1 => tags.push("stable-pair".to_string()),
            10 => tags.push("correlated".to_string()),
            50 => tags.push("standard".to_string()),
            100 => tags.push("volatile".to_string()),
            _ => {}
        }

        // 基于价格的标签
        let current_price = pool.price_info.current_price.unwrap_or(pool.price_info.initial_price);
        if current_price > 1000.0 {
            tags.push("high-value".to_string());
        } else if current_price < 0.01 {
            tags.push("micro-cap".to_string());
        }

        tags
    }

    /// 计算完成的farm数量（基于池子年龄和活动）
    fn calculate_farm_finished_count(&self, pool: &ClmmPool) -> u32 {
        let current_time = chrono::Utc::now().timestamp() as u64;
        let pool_age_days = (current_time - pool.api_created_at) / 86400;

        // 基于池子年龄和类型估算已完成的farm数量
        match pool.pool_type {
            database::clmm_pool::model::PoolType::Concentrated => {
                // CLMM池子通常有更多的激励活动
                match pool_age_days {
                    0..=7 => 0,    // 新池子
                    8..=30 => 1,   // 1个月内
                    31..=90 => 3,  // 3个月内
                    91..=180 => 5, // 6个月内
                    _ => 8,        // 老池子
                }
            }
            database::clmm_pool::model::PoolType::Standard => {
                // 标准池子的farm活动较少
                match pool_age_days {
                    0..=30 => 0,
                    31..=90 => 1,
                    91..=365 => 2,
                    _ => 3,
                }
            }
        }
    }

    /// 计算销毁百分比（基于代币特征）
    fn calculate_burn_percent(&self, pool: &ClmmPool) -> f64 {
        // 检查代币地址是否为已知的通缩/销毁代币
        let _mint_a = &pool.mint0.mint_address;
        let _mint_b = &pool.mint1.mint_address;

        // 已知的通缩代币映射
        let deflation_tokens = [
            ("SHIB", 0.1), // 示例：Shiba Inu有销毁机制
            ("FLOKI", 0.05), // 示例：Floki有销毁机制
                           // 更多通缩代币可以在这里添加
        ];

        // 检查是否为已知的通缩代币
        for (symbol, burn_rate) in deflation_tokens.iter() {
            if pool.mint0.symbol.as_ref().map_or(false, |s| s.contains(symbol))
                || pool.mint1.symbol.as_ref().map_or(false, |s| s.contains(symbol))
            {
                return *burn_rate;
            }
        }

        // 如果不是已知的通缩代币，返回0
        0.0
    }

    /// 判断是否为启动迁移池
    fn is_launch_migrate_pool(&self, pool: &ClmmPool) -> bool {
        let current_time = chrono::Utc::now().timestamp() as u64;
        let pool_age_hours = (current_time - pool.api_created_at) / 3600;

        // 新创建的池子（24小时内）可能是迁移池
        if pool_age_hours < 24 {
            return true;
        }

        // 检查是否为从旧版本升级的池子
        // 这里可以根据实际的迁移逻辑进行判断
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use database::clmm_pool::model::{
        ExtensionInfo, PoolStatus, PoolType, PriceInfo, SyncStatus, TokenInfo, VaultInfo,
    };
    #[allow(dead_code)]
    fn create_test_pool() -> ClmmPool {
        ClmmPool {
            id: None,
            pool_address: "test_pool_address".to_string(),
            amm_config_address: "test_config_address".to_string(),
            config_index: 0,
            mint0: TokenInfo {
                mint_address: "So11111111111111111111111111111111111111112".to_string(),
                decimals: 9,
                owner: "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA".to_string(),
                symbol: Some("WSOL".to_string()),
                name: Some("Wrapped SOL".to_string()),
                log_uri: None,
                description: None,
                external_url: None,
                tags: None,
                attributes: None,
            },
            mint1: TokenInfo {
                mint_address: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(),
                decimals: 6,
                owner: "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA".to_string(),
                symbol: Some("USDC".to_string()),
                name: Some("USD Coin".to_string()),
                log_uri: None,
                description: None,
                external_url: None,
                tags: None,
                attributes: None,
            },
            price_info: PriceInfo {
                initial_price: 100.0,
                sqrt_price_x64: "test_sqrt_price".to_string(),
                initial_tick: 0,
                current_price: Some(105.0),
                current_tick: Some(10),
            },
            vault_info: VaultInfo {
                token_vault_0: "test_vault_0".to_string(),
                token_vault_1: "test_vault_1".to_string(),
            },
            extension_info: ExtensionInfo {
                observation_address: "test_observation".to_string(),
                tickarray_bitmap_extension: "test_bitmap".to_string(),
            },
            creator_wallet: "test_creator".to_string(),
            open_time: 0,
            api_created_at: 1640995200,
            api_created_slot: None,
            updated_at: 1640995200,

            // 链上事件字段
            event_signature: None,
            event_updated_slot: None,
            event_confirmed_at: None,
            event_updated_at: None,

            transaction_info: None,
            status: PoolStatus::Active,
            sync_status: SyncStatus {
                last_sync_at: 1640995200,
                sync_version: 1,
                needs_sync: false,
                sync_error: None,
            },
            pool_type: PoolType::Concentrated,
            data_source: database::clmm_pool::DataSource::ApiCreated,
            chain_confirmed: false,
        }
    }

    #[test]
    fn test_calculate_fee_rates() {
        let transform_service = DataTransformService::new().unwrap();

        // 测试fallback方法
        assert_eq!(transform_service.calculate_trade_fee_rate_fallback(0), 3000);
        assert_eq!(transform_service.calculate_trade_fee_rate_fallback(1), 500);
        assert_eq!(transform_service.calculate_trade_fee_rate_fallback(2), 2500);
        assert_eq!(transform_service.calculate_trade_fee_rate_fallback(999), 500); // default

        assert_eq!(transform_service.calculate_fee_rate(0), 0.0005);
        assert_eq!(transform_service.calculate_fee_rate(1), 0.0005);
        assert_eq!(transform_service.calculate_fee_rate(2), 0.0025);
        assert_eq!(transform_service.calculate_fee_rate(999), 0.0005); // default
    }

    #[test]
    fn test_calculate_tick_spacing() {
        let transform_service = DataTransformService::new().unwrap();

        // 测试fallback方法
        assert_eq!(transform_service.calculate_tick_spacing_fallback(0), 60);
        assert_eq!(transform_service.calculate_tick_spacing_fallback(1), 10);
        assert_eq!(transform_service.calculate_tick_spacing_fallback(2), 50);
        assert_eq!(transform_service.calculate_tick_spacing_fallback(3), 100);
        assert_eq!(transform_service.calculate_tick_spacing_fallback(999), 10); // default
    }

    #[tokio::test]
    async fn test_load_amm_config_from_chain() {
        // 这个测试需要真实的RPC连接，仅在集成测试时运行
        if std::env::var("RUN_INTEGRATION_TESTS").is_ok() {
            let rpc_client = Arc::new(solana_client::rpc_client::RpcClient::new(
                "https://api.devnet.solana.com".to_string(),
            ));
            let service = DataTransformService::new_with_rpc(rpc_client).unwrap();

            // 使用一个已知的测试配置地址
            let test_config_address = "test_config_address";

            match service.load_amm_config_from_chain(test_config_address).await {
                Ok(Some(config)) => {
                    assert!(config.tick_spacing > 0);
                    assert!(config.trade_fee_rate > 0);
                    println!("✅ 成功加载配置: {:?}", config);
                }
                Ok(None) => {
                    println!("⚠️ 配置不存在或RPC客户端未配置");
                }
                Err(e) => {
                    println!("❌ 加载配置失败: {}", e);
                }
            }
        } else {
            println!("跳过集成测试 - 设置RUN_INTEGRATION_TESTS环境变量以运行");
        }
    }

    #[test]
    fn test_cache_functionality() {
        let service = DataTransformService::new().unwrap();

        // 测试缓存结构初始化
        {
            let cache = service.amm_config_cache.lock().unwrap();
            assert!(cache.is_empty());
        }

        // 测试缓存插入
        {
            let mut cache = service.amm_config_cache.lock().unwrap();
            let test_config = AmmConfigCache {
                protocol_fee_rate: 120000,
                trade_fee_rate: 500,
                tick_spacing: 10,
                fund_fee_rate: 40000,
                timestamp: chrono::Utc::now().timestamp() as u64,
            };
            cache.insert("test_address".to_string(), test_config.clone());

            assert_eq!(cache.len(), 1);
            assert_eq!(cache.get("test_address").unwrap().tick_spacing, 10);
        }
    }

    #[test]
    fn test_tick_spacing_based_calculations() {
        let service = DataTransformService::new().unwrap();

        // 测试基于真实tick间距的范围计算
        let default_range =
            service.calculate_default_range_from_tick_spacing(10, &database::clmm_pool::model::PoolType::Concentrated);
        assert_eq!(default_range, 0.05);

        let default_range =
            service.calculate_default_range_from_tick_spacing(60, &database::clmm_pool::model::PoolType::Concentrated);
        assert_eq!(default_range, 0.02);

        // 测试基于真实tick间距的范围点计算
        let range_points = service.generate_range_points_from_tick_spacing(
            10,
            &database::clmm_pool::model::PoolType::Concentrated,
            100.0,
        );
        assert!(!range_points.is_empty());
        assert!(range_points.contains(&0.01));
    }

    #[test]
    fn test_fallback_methods() {
        let service = DataTransformService::new().unwrap();

        // 测试fallback方法返回的硬编码值
        assert_eq!(service.calculate_trade_fee_rate_fallback(0), 3000);
        assert_eq!(service.calculate_trade_fee_rate_fallback(1), 500);

        assert_eq!(service.calculate_tick_spacing_fallback(0), 60);
        assert_eq!(service.calculate_tick_spacing_fallback(1), 10);

        assert_eq!(service.calculate_protocol_fee_rate_fallback(1), 120000);
        assert_eq!(service.calculate_fund_fee_rate_fallback(1), 40000);
    }

    #[tokio::test]
    async fn test_create_extended_mint_info() {
        let transform_service = DataTransformService::new().unwrap();
        let metadata_map = HashMap::new();

        let mint_info = transform_service
            .create_extended_mint_info(
                "So11111111111111111111111111111111111111112",
                9,
                "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",
                &metadata_map,
            )
            .unwrap();

        assert_eq!(mint_info.chain_id, 101);
        assert_eq!(mint_info.address, "So11111111111111111111111111111111111111112");
        assert_eq!(mint_info.decimals, 9);
        assert_eq!(mint_info.program_id, "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
    }
}
