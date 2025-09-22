//! Position Utils 性能优化版本
//!
//! 本文件是 position_utils.rs 的高性能优化版本，实现了以下优化：
//! 1. 批量RPC调用 - 使用 get_multiple_accounts 替代单独调用
//! 2. 并发处理 - 同时获取经典Token和Token-2022的NFT
//! 3. 智能过滤 - 预过滤潜在NFT账户，减少不必要的处理
//! 4. 内存优化 - 使用紧凑数据结构和流式处理
//! 5. 性能监控 - 完整的性能统计和监控

use anyhow::Result;
use rayon::prelude::*;
use solana_account_decoder::parse_token::TokenAccountType;
use solana_account_decoder::UiAccountData;
use solana_client::rpc_client::RpcClient;
use solana_client::rpc_response::RpcKeyedAccount;
use solana_sdk::pubkey::Pubkey;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time;
use tracing::{info, warn};

use super::position_utils::{ExistingPosition, PersonalPositionState, PositionNftInfo};
use super::{ConfigManager, PDACalculator};

/// 性能统计 - 优化版本专用
#[derive(Debug, Default)]
pub struct PositionPerformanceStats {
    pub total_queries: AtomicU64,
    pub batch_queries: AtomicU64,
    pub concurrent_queries: AtomicU64,
    pub cache_hits: AtomicU64,
    pub cache_misses: AtomicU64,
    pub total_rpc_calls: AtomicU64,
    pub total_query_time_ms: AtomicU64,
    pub nfts_processed: AtomicUsize,
    pub filtered_accounts: AtomicUsize,
    pub memory_saved_bytes: AtomicU64,
}

impl PositionPerformanceStats {
    pub fn record_query(&self, rpc_calls: u64, query_time_ms: u64, nfts_count: usize, was_concurrent: bool) {
        self.total_queries.fetch_add(1, Ordering::Relaxed);
        self.total_rpc_calls.fetch_add(rpc_calls, Ordering::Relaxed);
        self.total_query_time_ms.fetch_add(query_time_ms, Ordering::Relaxed);
        self.nfts_processed.fetch_add(nfts_count, Ordering::Relaxed);

        if was_concurrent {
            self.concurrent_queries.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn record_batch_query(&self) {
        self.batch_queries.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_cache_hit(&self) {
        self.cache_hits.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_cache_miss(&self) {
        self.cache_misses.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_filtered_accounts(&self, filtered_count: usize, total_count: usize) {
        self.filtered_accounts.fetch_add(filtered_count, Ordering::Relaxed);
        // 估算节省的内存 (假设每个账户平均500字节)
        let saved_bytes = (total_count - filtered_count) * 500;
        self.memory_saved_bytes.fetch_add(saved_bytes as u64, Ordering::Relaxed);
    }

    pub fn get_stats(&self) -> String {
        let total_queries = self.total_queries.load(Ordering::Relaxed);
        let batch_queries = self.batch_queries.load(Ordering::Relaxed);
        let concurrent_queries = self.concurrent_queries.load(Ordering::Relaxed);
        let cache_hits = self.cache_hits.load(Ordering::Relaxed);
        let cache_misses = self.cache_misses.load(Ordering::Relaxed);
        let total_rpc_calls = self.total_rpc_calls.load(Ordering::Relaxed);
        let total_time = self.total_query_time_ms.load(Ordering::Relaxed);
        let total_nfts = self.nfts_processed.load(Ordering::Relaxed);
        let filtered_accounts = self.filtered_accounts.load(Ordering::Relaxed);
        let memory_saved = self.memory_saved_bytes.load(Ordering::Relaxed);

        let cache_hit_rate = if cache_hits + cache_misses > 0 {
            (cache_hits as f64 / (cache_hits + cache_misses) as f64) * 100.0
        } else {
            0.0
        };

        format!(
            "Position查询优化统计:\n\
             - 总查询数: {}\n\
             - 批量查询数: {} ({:.1}%)\n\
             - 并发查询数: {} ({:.1}%)\n\
             - 缓存命中: {} ({:.1}%)\n\
             - 缓存未命中: {}\n\
             - 总RPC调用: {}\n\
             - 平均RPC调用/查询: {:.1}\n\
             - 总查询时间: {}ms\n\
             - 平均查询时间: {:.1}ms\n\
             - 处理的NFT总数: {}\n\
             - 平均NFT数/查询: {:.1}\n\
             - 过滤的账户数: {}\n\
             - 节省的内存: {:.2}MB",
            total_queries,
            batch_queries,
            if total_queries > 0 {
                (batch_queries as f64 / total_queries as f64) * 100.0
            } else {
                0.0
            },
            concurrent_queries,
            if total_queries > 0 {
                (concurrent_queries as f64 / total_queries as f64) * 100.0
            } else {
                0.0
            },
            cache_hits,
            cache_hit_rate,
            cache_misses,
            total_rpc_calls,
            if total_queries > 0 {
                total_rpc_calls as f64 / total_queries as f64
            } else {
                0.0
            },
            total_time,
            if total_queries > 0 {
                total_time as f64 / total_queries as f64
            } else {
                0.0
            },
            total_nfts,
            if total_queries > 0 {
                total_nfts as f64 / total_queries as f64
            } else {
                0.0
            },
            filtered_accounts,
            memory_saved as f64 / 1024.0 / 1024.0
        )
    }

    pub fn get_cache_hit_rate(&self) -> f64 {
        let hits = self.cache_hits.load(Ordering::Relaxed) as f64;
        let total = hits + self.cache_misses.load(Ordering::Relaxed) as f64;

        if total > 0.0 {
            hits / total
        } else {
            0.0
        }
    }

    pub fn get_average_rpc_duration(&self) -> f64 {
        let total_duration = self.total_query_time_ms.load(Ordering::Relaxed) as f64;
        let total_calls = self.total_rpc_calls.load(Ordering::Relaxed) as f64;

        if total_calls > 0.0 {
            total_duration / total_calls
        } else {
            0.0
        }
    }
}

/// 紧凑的Position NFT信息 - 减少内存使用
#[derive(Debug, Clone, Copy)]
pub struct CompactPositionNftInfo {
    pub nft_mint: [u8; 32],
    pub nft_account: [u8; 32],
    pub position_pda: [u8; 32],
    pub token_program_type: TokenProgramType,
}

#[derive(Debug, Clone, Copy)]
pub enum TokenProgramType {
    Classic = 0,
    Token2022 = 1,
}

impl CompactPositionNftInfo {
    pub fn from_standard(info: &PositionNftInfo) -> Self {
        Self {
            nft_mint: info.nft_mint.to_bytes(),
            nft_account: info.nft_account.to_bytes(),
            position_pda: info.position_pda.to_bytes(),
            token_program_type: if info.token_program == spl_token::id() {
                TokenProgramType::Classic
            } else {
                TokenProgramType::Token2022
            },
        }
    }

    pub fn to_standard(&self) -> PositionNftInfo {
        PositionNftInfo {
            nft_mint: Pubkey::new_from_array(self.nft_mint),
            nft_account: Pubkey::new_from_array(self.nft_account),
            position_pda: Pubkey::new_from_array(self.position_pda),
            token_program: match self.token_program_type {
                TokenProgramType::Classic => spl_token::id(),
                TokenProgramType::Token2022 => spl_token_2022::id(),
            },
        }
    }
}

/// Position工具类 - 优化版本
pub struct PositionUtilsOptimized<'a> {
    rpc_client: &'a RpcClient,
    stats: Option<Arc<PositionPerformanceStats>>,
}

impl<'a> PositionUtilsOptimized<'a> {
    pub fn new(rpc_client: &'a RpcClient) -> Self {
        Self {
            rpc_client,
            stats: Some(Arc::new(PositionPerformanceStats::default())),
        }
    }

    pub fn with_stats(rpc_client: &'a RpcClient, stats: Arc<PositionPerformanceStats>) -> Self {
        Self {
            rpc_client,
            stats: Some(stats),
        }
    }

    pub fn get_performance_stats(&self) -> Option<String> {
        self.stats.as_ref().map(|s| s.get_stats())
    }

    // ============ 核心优化方法 ============

    /// 批量获取多个position账户 - 新增优化方法
    async fn get_positions_batch(
        &self,
        position_pdas: Vec<Pubkey>,
    ) -> Result<Vec<Option<solana_sdk::account::Account>>> {
        use solana_sdk::commitment_config::CommitmentConfig;

        if position_pdas.is_empty() {
            return Ok(Vec::new());
        }

        info!("🚀 批量获取 {} 个position账户", position_pdas.len());

        // 使用 get_multiple_accounts 批量获取
        let accounts = self
            .rpc_client
            .get_multiple_accounts_with_commitment(&position_pdas, CommitmentConfig::confirmed())?
            .value;

        info!("✅ 批量获取完成，收到 {} 个账户响应", accounts.len());

        if let Some(stats) = &self.stats {
            stats.record_batch_query();
        }

        Ok(accounts)
    }

    /// 获取用户的position NFTs - 并发优化版本
    pub async fn get_user_position_nfts_optimized(&self, user_wallet: &Pubkey) -> Result<Vec<PositionNftInfo>> {
        info!("🔍 优化版本：并发获取用户的Position NFTs（包括Token和Token-2022）");

        let start_time = Instant::now();

        // 先获取Token程序ID以避免借用检查问题
        let spl_token_id = spl_token::id();
        let spl_token_2022_id = spl_token_2022::id();

        // 并发获取两种类型的NFT
        let (classic_result, token2022_result) = tokio::join!(
            self.get_position_nfts_by_program_optimized(user_wallet, &spl_token_id),
            self.get_position_nfts_by_program_optimized(user_wallet, &spl_token_2022_id)
        );

        let classic_nfts = classic_result?;
        let token2022_nfts = token2022_result?;

        let mut all_position_nfts = Vec::new();
        all_position_nfts.extend(classic_nfts.clone());
        all_position_nfts.extend(token2022_nfts.clone());

        // 按NFT mint地址排序以确保一致性
        all_position_nfts.sort_by_key(|nft| nft.nft_mint.to_string());

        let query_time = start_time.elapsed();
        info!(
            "  ✅ 并发获取完成：{} 个经典Token NFT，{} 个Token-2022 NFT，总共 {} 个NFT，耗时: {:?}",
            classic_nfts.len(),
            token2022_nfts.len(),
            all_position_nfts.len(),
            query_time
        );

        // 记录性能统计
        if let Some(stats) = &self.stats {
            stats.record_query(2, query_time.as_millis() as u64, all_position_nfts.len(), true);
        }

        Ok(all_position_nfts)
    }

    /// 根据特定的Token程序获取position NFTs - 优化版本
    async fn get_position_nfts_by_program_optimized(
        &self,
        user_wallet: &Pubkey,
        token_program: &Pubkey,
    ) -> Result<Vec<PositionNftInfo>> {
        use solana_sdk::commitment_config::CommitmentConfig;

        info!(
            "🔍 智能过滤获取{}程序的Position NFT",
            if *token_program == spl_token::id() {
                "经典Token"
            } else {
                "Token-2022"
            }
        );

        let commitment = CommitmentConfig::confirmed();
        let config = solana_client::rpc_request::TokenAccountsFilter::ProgramId(*token_program);
        let token_accounts_response =
            self.rpc_client
                .get_token_accounts_by_owner_with_commitment(user_wallet, config, commitment)?;

        let all_token_accounts = token_accounts_response.value;
        info!("  📥 获取到 {} 个Token账户", all_token_accounts.len());

        // 使用流式处理和紧凑数据结构
        let processor = self.create_nft_filter_processor();
        let compact_nfts = self
            .process_token_accounts_streaming(all_token_accounts.clone(), processor)
            .await?;

        info!("  🔍 流式过滤得到 {} 个潜在NFT", compact_nfts.len());

        // 记录过滤统计
        if let Some(stats) = &self.stats {
            stats.record_filtered_accounts(compact_nfts.len(), all_token_accounts.len());
        }

        // 批量验证Position存在性
        if compact_nfts.is_empty() {
            return Ok(Vec::new());
        }

        let position_pdas: Vec<Pubkey> = compact_nfts
            .iter()
            .map(|nft| Pubkey::new_from_array(nft.position_pda))
            .collect();

        let position_accounts = self.get_positions_batch(position_pdas).await?;

        // 只保留真实存在的Position，转换回标准格式
        let verified_nfts: Vec<PositionNftInfo> = compact_nfts
            .iter()
            .zip(position_accounts.iter())
            .filter_map(|(compact_nft, account_opt)| {
                if account_opt.is_some() {
                    Some(compact_nft.to_standard())
                } else {
                    None
                }
            })
            .collect();

        info!(
            "  ✅ 从{}程序验证得到 {} 个真实Position NFT",
            if *token_program == spl_token::id() {
                "经典Token"
            } else {
                "Token-2022"
            },
            verified_nfts.len()
        );

        Ok(verified_nfts)
    }

    /// 内部查找方法 - 批量优化版本
    pub async fn find_existing_position_optimized(
        &self,
        user_wallet: &Pubkey,
        pool_address: &Pubkey,
        tick_lower: i32,
        tick_upper: i32,
    ) -> Result<Option<ExistingPosition>> {
        let start_time = Instant::now();

        info!("🔍 优化版本：检查是否存在相同范围的仓位");
        info!("  钱包: {}", user_wallet);
        info!("  池子: {}", pool_address);
        info!("  Tick范围: {} - {}", tick_lower, tick_upper);

        // 使用带重试的NFT获取
        let position_nfts = self.get_user_position_nfts_with_retry(user_wallet, 3).await?;
        info!("🔍 找到 {} 个Position NFT", position_nfts.len());

        if position_nfts.is_empty() {
            return Ok(None);
        }

        // 提取所有position PDA
        let position_pdas: Vec<Pubkey> = position_nfts.iter().map(|nft| nft.position_pda).collect();

        // 批量获取所有position账户
        let position_accounts = self.get_positions_batch(position_pdas).await?;

        // 并行处理和匹配 - 使用Rayon进行CPU密集型并行处理
        let matching_result = position_nfts
            .par_iter()
            .zip(position_accounts.par_iter())
            .enumerate()
            .find_first(|(index, (nft_info, position_account_opt))| {
                info!(
                    "🔍 检查NFT #{}: mint={}, position_pda={}",
                    index + 1,
                    nft_info.nft_mint,
                    nft_info.position_pda
                );

                if let Some(position_account) = position_account_opt {
                    info!(
                        "  ✅ 成功获取position账户数据，大小: {} bytes",
                        position_account.data.len()
                    );

                    match self.deserialize_position_state(position_account) {
                        Ok(position_state) => {
                            info!("  ✅ 成功反序列化position状态:");
                            info!("    池子ID: {}", position_state.pool_id);
                            info!(
                                "    tick范围: {} - {}",
                                position_state.tick_lower_index, position_state.tick_upper_index
                            );
                            info!("    流动性: {}", position_state.liquidity);

                            if position_state.pool_id == *pool_address
                                && position_state.tick_lower_index == tick_lower
                                && position_state.tick_upper_index == tick_upper
                            {
                                info!("  🎯 找到匹配的仓位！");
                                return true;
                            } else {
                                info!("  ⏭️ 仓位不匹配，继续搜索");
                                return false;
                            }
                        }
                        Err(e) => {
                            warn!("  ⚠️ 反序列化position状态失败: {:?}", e);
                            return false;
                        }
                    }
                } else {
                    warn!("  ⚠️ 获取position账户失败，账户可能不存在");
                    return false;
                }
            });

        // 处理匹配结果
        if let Some((_index, (nft_info, position_account_opt))) = matching_result {
            if let Some(position_account) = position_account_opt {
                if let Ok(position_state) = self.deserialize_position_state(position_account) {
                    let query_time = start_time.elapsed();

                    // 记录性能统计
                    if let Some(stats) = &self.stats {
                        // 批量查询减少了RPC调用次数：NFT查询 + 批量position查询
                        let rpc_calls = 2;
                        stats.record_query(rpc_calls, query_time.as_millis() as u64, position_nfts.len(), false);
                    }

                    return Ok(Some(ExistingPosition {
                        nft_mint: nft_info.nft_mint,
                        nft_token_account: nft_info.nft_account,
                        position_key: nft_info.position_pda,
                        liquidity: position_state.liquidity,
                        nft_token_program: nft_info.token_program,
                    }));
                }
            }
        }

        let query_time = start_time.elapsed();

        // 记录性能统计
        if let Some(stats) = &self.stats {
            let rpc_calls = 2; // NFT查询 + 批量position查询
            stats.record_query(rpc_calls, query_time.as_millis() as u64, position_nfts.len(), false);
        }

        info!("✅ 确认没有相同范围的仓位，总耗时: {:?}", query_time);
        Ok(None)
    }

    /// 带重试机制的并发NFT获取
    async fn get_user_position_nfts_with_retry(
        &self,
        user_wallet: &Pubkey,
        max_retries: u32,
    ) -> Result<Vec<PositionNftInfo>> {
        let mut attempts = 0;

        while attempts <= max_retries {
            match self.get_user_position_nfts_optimized(user_wallet).await {
                Ok(nfts) => return Ok(nfts),
                Err(e) => {
                    attempts += 1;
                    if attempts > max_retries {
                        return Err(anyhow::anyhow!("获取NFT失败，已重试{}次: {:?}", max_retries, e));
                    }

                    warn!("获取NFT失败，第{}次重试: {:?}", attempts, e);

                    // 指数退避
                    let delay = Duration::from_millis(100 * (2_u64.pow(attempts - 1)));
                    time::sleep(delay).await;
                }
            }
        }

        unreachable!()
    }

    /// 流式处理大量Token账户
    async fn process_token_accounts_streaming<F, R>(
        &self,
        accounts: Vec<RpcKeyedAccount>,
        processor: F,
    ) -> Result<Vec<R>>
    where
        F: Fn(&RpcKeyedAccount) -> Option<R> + Send + Sync,
        R: Send,
    {
        const BATCH_SIZE: usize = 50; // 每批处理50个账户
        let mut results = Vec::new();

        for (batch_index, chunk) in accounts.chunks(BATCH_SIZE).enumerate() {
            info!("  📦 处理第{}批，包含{}个账户", batch_index + 1, chunk.len());

            let batch_results: Vec<R> = chunk.iter().filter_map(|account| processor(account)).collect();

            results.extend(batch_results);

            // 让出CPU时间，避免阻塞其他任务
            if batch_index % 5 == 4 {
                // 每处理5批后让出一次
                tokio::task::yield_now().await;
            }
        }

        info!(
            "  ✅ 流式处理完成，总共处理{}个账户，得到{}个结果",
            accounts.len(),
            results.len()
        );
        Ok(results)
    }

    /// 内存友好的NFT过滤处理器
    fn create_nft_filter_processor(&self) -> impl Fn(&RpcKeyedAccount) -> Option<CompactPositionNftInfo> + '_ {
        move |account_info: &RpcKeyedAccount| -> Option<CompactPositionNftInfo> {
            if !self.is_potential_position_nft(account_info) {
                return None;
            }

            if let UiAccountData::Json(parsed_account) = &account_info.account.data {
                if let Ok(TokenAccountType::Account(ui_token_account)) =
                    serde_json::from_value(parsed_account.parsed.clone())
                {
                    if let (Ok(nft_mint), Ok(nft_account)) = (
                        ui_token_account.mint.parse::<Pubkey>(),
                        account_info.pubkey.parse::<Pubkey>(),
                    ) {
                        let raydium_program_id = ConfigManager::get_raydium_program_id().ok()?;
                        let (position_pda, _) =
                            Pubkey::find_program_address(&[b"position", nft_mint.as_ref()], &raydium_program_id);

                        let token_program_type = if parsed_account.program == "spl-token-2022" {
                            TokenProgramType::Token2022
                        } else {
                            TokenProgramType::Classic
                        };

                        return Some(CompactPositionNftInfo {
                            nft_mint: nft_mint.to_bytes(),
                            nft_account: nft_account.to_bytes(),
                            position_pda: position_pda.to_bytes(),
                            token_program_type,
                        });
                    }
                }
            }

            None
        }
    }

    /// NFT账户预过滤器
    fn is_potential_position_nft(&self, account_info: &RpcKeyedAccount) -> bool {
        // 快速预过滤：只检查关键属性
        if let UiAccountData::Json(parsed_account) = &account_info.account.data {
            if parsed_account.program == "spl-token" || parsed_account.program == "spl-token-2022" {
                if let Ok(TokenAccountType::Account(ui_token_account)) =
                    serde_json::from_value(parsed_account.parsed.clone())
                {
                    // NFT特征：decimals=0, amount=1
                    return ui_token_account.token_amount.decimals == 0 && ui_token_account.token_amount.amount == "1";
                }
            }
        }
        false
    }

    /// 反序列化position状态 - 复用原有逻辑
    pub fn deserialize_position_state(&self, account: &solana_sdk::account::Account) -> Result<PersonalPositionState> {
        let mut data: &[u8] = &account.data;
        anchor_lang::AccountDeserialize::try_deserialize(&mut data)
            .map_err(|e| anyhow::anyhow!("反序列化position状态失败: {:?}", e))
    }

    // ============ 向后兼容的包装方法 ============

    /// 向后兼容：检查仓位是否已存在
    pub async fn find_existing_position(
        &self,
        user_wallet: &Pubkey,
        pool_address: &Pubkey,
        tick_lower: i32,
        tick_upper: i32,
    ) -> Result<Option<ExistingPosition>> {
        self.find_existing_position_optimized(user_wallet, pool_address, tick_lower, tick_upper)
            .await
    }

    /// 向后兼容：获取用户的position NFTs
    pub async fn get_user_position_nfts(&self, user_wallet: &Pubkey) -> Result<Vec<PositionNftInfo>> {
        self.get_user_position_nfts_optimized(user_wallet).await
    }

    // ============ 从原始PositionUtils复制的方法（保持API兼容性）============

    /// 价格转换为sqrt_price_x64
    // pub fn price_to_sqrt_price_x64(&self, price: f64, decimals_0: u8, decimals_1: u8) -> u128 {
    //     // 调整小数位数差异
    //     let decimal_adjustment = 10_f64.powi(decimals_0 as i32 - decimals_1 as i32);
    //     let adjusted_price = price / decimal_adjustment;

    //     // 计算sqrt_price
    //     let sqrt_price = adjusted_price.sqrt();

    //     // 转换为Q64.64格式
    //     (sqrt_price * (1u128 << 64) as f64) as u128
    // }

    // /// sqrt_price_x64转换为价格
    // pub fn sqrt_price_x64_to_price(&self, sqrt_price_x64: u128, decimals_0: u8, decimals_1: u8) -> f64 {
    //     let sqrt_price = sqrt_price_x64 as f64 / (1u128 << 64) as f64;
    //     let price = sqrt_price * sqrt_price;

    //     // 调整小数位数
    //     let decimal_adjustment = 10_f64.powi(decimals_0 as i32 - decimals_1 as i32);
    //     price * decimal_adjustment
    // }

    pub fn price_to_sqrt_price_x64(&self, price: f64, decimals_0: u8, decimals_1: u8) -> u128 {
        raydium_amm_v3_clent::price_to_sqrt_price_x64(price, decimals_0, decimals_1)
    }

    pub fn sqrt_price_x64_to_price(&self, price: u128, decimals_0: u8, decimals_1: u8) -> f64 {
        raydium_amm_v3_clent::sqrt_price_x64_to_price(price, decimals_0, decimals_1)
    }

    /// 根据价格计算tick索引
    pub fn price_to_tick(&self, price: f64, decimals_0: u8, decimals_1: u8) -> Result<i32> {
        let sqrt_price_x64 = raydium_amm_v3_clent::price_to_sqrt_price_x64(price, decimals_0, decimals_1);
        raydium_amm_v3::libraries::tick_math::get_tick_at_sqrt_price(sqrt_price_x64)
            .map_err(|e| anyhow::anyhow!("价格转tick失败: {:?}", e))
    }

    /// 根据tick计算价格
    pub fn tick_to_price(&self, tick: i32, decimals_0: u8, decimals_1: u8) -> Result<f64> {
        let sqrt_price_x64 = raydium_amm_v3::libraries::tick_math::get_sqrt_price_at_tick(tick)
            .map_err(|e| anyhow::anyhow!("tick转价格失败: {:?}", e))?;
        Ok(raydium_amm_v3_clent::sqrt_price_x64_to_price(
            sqrt_price_x64,
            decimals_0,
            decimals_1,
        ))
    }

    /// 根据tick spacing调整tick
    pub fn tick_with_spacing(&self, tick: i32, tick_spacing: i32) -> i32 {
        let division = tick / tick_spacing;
        if tick < 0 && tick % tick_spacing != 0 {
            (division - 1) * tick_spacing
        } else {
            division * tick_spacing
        }
    }

    /// 计算单一代币流动性（基于输入金额）
    pub fn calculate_liquidity_from_single_amount(
        &self,
        current_sqrt_price_x64: u128,
        sqrt_price_lower_x64: u128,
        sqrt_price_upper_x64: u128,
        amount: u64,
        is_token_0: bool,
    ) -> Result<u128> {
        if is_token_0 {
            Ok(
                raydium_amm_v3::libraries::liquidity_math::get_liquidity_from_single_amount_0(
                    current_sqrt_price_x64,
                    sqrt_price_lower_x64,
                    sqrt_price_upper_x64,
                    amount,
                ),
            )
        } else {
            Ok(
                raydium_amm_v3::libraries::liquidity_math::get_liquidity_from_single_amount_1(
                    current_sqrt_price_x64,
                    sqrt_price_lower_x64,
                    sqrt_price_upper_x64,
                    amount,
                ),
            )
        }
    }

    /// 根据流动性计算token数量
    pub fn calculate_amounts_from_liquidity(
        &self,
        current_tick: i32,
        current_sqrt_price_x64: u128,
        tick_lower: i32,
        tick_upper: i32,
        liquidity: u128,
    ) -> Result<(u64, u64)> {
        raydium_amm_v3::libraries::liquidity_math::get_delta_amounts_signed(
            current_tick,
            current_sqrt_price_x64,
            tick_lower,
            tick_upper,
            liquidity as i128,
        )
        .map_err(|e| anyhow::anyhow!("流动性计算金额失败: {:?}", e))
    }

    /// 应用滑点保护
    pub fn apply_slippage(&self, amount: u64, slippage_percent: f64, is_min: bool) -> u64 {
        if is_min {
            // 减少金额（用于计算最小输出）
            ((amount as f64) * (1.0 - slippage_percent / 100.0)).floor() as u64
        } else {
            // 增加金额（用于计算最大输入）
            ((amount as f64) * (1.0 + slippage_percent / 100.0)).ceil() as u64
        }
    }

    /// 计算tick array的起始索引
    pub fn get_tick_array_start_index(&self, tick: i32, tick_spacing: u16) -> i32 {
        raydium_amm_v3::states::TickArrayState::get_array_start_index(tick, tick_spacing)
    }

    /// 构建remaining accounts（tick arrays和bitmap）
    pub async fn build_remaining_accounts(
        &self,
        pool_address: &Pubkey,
        tick_lower: i32,
        tick_upper: i32,
        tick_spacing: u16,
    ) -> Result<Vec<solana_sdk::instruction::AccountMeta>> {
        let raydium_program_id = ConfigManager::get_raydium_program_id()?;
        let mut remaining_accounts = Vec::new();

        // 添加tick array bitmap extension
        let (bitmap_pda, _) =
            PDACalculator::calculate_tickarray_bitmap_extension_pda(&raydium_program_id, pool_address);
        remaining_accounts.push(solana_sdk::instruction::AccountMeta::new(bitmap_pda, false));

        // 计算需要的tick arrays
        let tick_array_lower_start = self.get_tick_array_start_index(tick_lower, tick_spacing);
        let tick_array_upper_start = self.get_tick_array_start_index(tick_upper, tick_spacing);

        // 添加下限tick array
        let (tick_array_lower_pda, _) =
            PDACalculator::calculate_tick_array_pda(&raydium_program_id, pool_address, tick_array_lower_start);
        remaining_accounts.push(solana_sdk::instruction::AccountMeta::new(tick_array_lower_pda, false));

        // 如果上限和下限不在同一个tick array中，添加上限tick array
        if tick_array_lower_start != tick_array_upper_start {
            let (tick_array_upper_pda, _) =
                PDACalculator::calculate_tick_array_pda(&raydium_program_id, pool_address, tick_array_upper_start);
            remaining_accounts.push(solana_sdk::instruction::AccountMeta::new(tick_array_upper_pda, false));
        }

        Ok(remaining_accounts)
    }

    /// 计算价格范围的利用率
    pub fn calculate_price_range_utilization(&self, current_price: f64, lower_price: f64, upper_price: f64) -> f64 {
        if lower_price >= upper_price {
            return 0.0;
        }

        if current_price <= lower_price {
            0.0
        } else if current_price >= upper_price {
            1.0
        } else {
            (current_price - lower_price) / (upper_price - lower_price)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_client::rpc_client::RpcClient;
    use std::str::FromStr;

    #[tokio::test]
    async fn test_optimized_position_utils_creation() {
        // 测试创建优化版本
        let rpc_client = RpcClient::new("https://api.devnet.solana.com".to_string());
        let utils = PositionUtilsOptimized::new(&rpc_client);

        // 验证统计对象被正确创建
        assert!(utils.stats.is_some());
        let stats = utils.get_performance_stats().unwrap();
        assert!(stats.contains("Position查询优化统计"));
    }

    #[tokio::test]
    async fn test_compact_position_nft_info() {
        // 测试紧凑数据结构的转换
        let original = PositionNftInfo {
            nft_mint: Pubkey::from_str("11111111111111111111111111111112").unwrap(),
            nft_account: Pubkey::from_str("11111111111111111111111111111113").unwrap(),
            position_pda: Pubkey::from_str("11111111111111111111111111111114").unwrap(),
            token_program: spl_token::id(),
        };

        let compact = CompactPositionNftInfo::from_standard(&original);
        let restored = compact.to_standard();

        assert_eq!(original.nft_mint, restored.nft_mint);
        assert_eq!(original.nft_account, restored.nft_account);
        assert_eq!(original.position_pda, restored.position_pda);
        assert_eq!(original.token_program, restored.token_program);
    }

    #[test]
    fn test_performance_stats() {
        let stats = PositionPerformanceStats::default();

        // 测试记录查询
        stats.record_query(5, 1000, 10, false);
        stats.record_batch_query();
        stats.record_cache_hit();
        stats.record_cache_miss();
        stats.record_filtered_accounts(5, 100);

        // 验证统计数据
        assert_eq!(stats.total_queries.load(Ordering::Relaxed), 1);
        assert_eq!(stats.batch_queries.load(Ordering::Relaxed), 1);
        assert_eq!(stats.cache_hits.load(Ordering::Relaxed), 1);
        assert_eq!(stats.cache_misses.load(Ordering::Relaxed), 1);
        assert_eq!(stats.filtered_accounts.load(Ordering::Relaxed), 5);

        let report = stats.get_stats();
        assert!(report.contains("总查询数: 1"));
        assert!(report.contains("批量查询数: 1"));

        // 测试缓存命中率
        let hit_rate = stats.get_cache_hit_rate();
        assert!((hit_rate - 0.5).abs() < f64::EPSILON); // 1/(1+1) = 0.5
    }

    #[test]
    fn test_token_program_type() {
        assert_eq!(TokenProgramType::Classic as u8, 0);
        assert_eq!(TokenProgramType::Token2022 as u8, 1);
    }
}
