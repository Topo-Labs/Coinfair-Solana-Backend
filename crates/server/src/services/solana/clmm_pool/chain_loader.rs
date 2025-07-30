use crate::services::solana::shared::SharedContext;
use anyhow::Result;
use database::clmm_pool::model::{ClmmPool, ExtensionInfo, PoolStatus, PoolType, PriceInfo, SyncStatus, TokenInfo, VaultInfo};
use solana_sdk::program_pack::Pack;
use solana_sdk::pubkey::Pubkey;
use spl_token::state::Mint;
use std::str::FromStr;
use std::sync::Arc;
use tracing::{debug, error, info, warn};
use utils::solana::account_loader::AccountLoader;

/// 链上池子数据加载器
/// 负责从池子地址获取完整的池子信息并构建 ClmmPool 结构
pub struct ChainPoolLoader {
    shared: Arc<SharedContext>,
}

impl ChainPoolLoader {
    pub fn new(shared: Arc<SharedContext>) -> Self {
        Self { shared }
    }

    /// 从链上加载多个池子的完整信息
    /// 返回成功加载的池子列表，失败的会被过滤掉
    pub async fn load_pools_from_chain(&self, pool_addresses: &[String]) -> Result<Vec<ClmmPool>> {
        info!("🔗 开始从链上加载 {} 个池子信息", pool_addresses.len());

        let mut pools = Vec::new();

        // 批量处理，每批最多处理10个池子避免RPC压力过大
        const BATCH_SIZE: usize = 10;

        for chunk in pool_addresses.chunks(BATCH_SIZE) {
            match self.load_pool_batch(chunk).await {
                Ok(mut batch_pools) => {
                    pools.append(&mut batch_pools);
                }
                Err(e) => {
                    error!("❌ 批量加载池子失败: {:?}", e);
                    // 尝试逐个加载这一批中的池子
                    for pool_address in chunk {
                        match self.load_single_pool(pool_address).await {
                            Ok(pool) => pools.push(pool),
                            Err(e) => {
                                warn!("⚠️ 单个池子加载失败 {}: {}", pool_address, e);
                            }
                        }
                    }
                }
            }
        }

        info!("✅ 成功从链上加载 {} 个池子信息", pools.len());
        Ok(pools)
    }

    /// 批量加载一批池子
    async fn load_pool_batch(&self, pool_addresses: &[String]) -> Result<Vec<ClmmPool>> {
        let mut pools = Vec::new();

        for pool_address in pool_addresses {
            match self.load_single_pool(pool_address).await {
                Ok(pool) => pools.push(pool),
                Err(e) => {
                    warn!("⚠️ 池子加载失败 {}: {}", pool_address, e);
                }
            }
        }

        Ok(pools)
    }

    /// 从链上加载单个池子的完整信息
    pub async fn load_single_pool(&self, pool_address: &str) -> Result<ClmmPool> {
        debug!("🔍 开始加载池子: {}", pool_address);

        // 1. 解析池子地址
        let pool_pubkey = Pubkey::from_str(pool_address).map_err(|e| anyhow::anyhow!("无效的池子地址 {}: {}", pool_address, e))?;

        // 2. 获取池子账户信息
        let account_loader = AccountLoader::new(&self.shared.rpc_client);
        let pool_account = self.shared.rpc_client.get_account(&pool_pubkey).map_err(|e| anyhow::anyhow!("获取池子账户失败: {}", e))?;

        // 3. 解析池子状态
        let pool_state: raydium_amm_v3::states::PoolState = account_loader.deserialize_anchor_account(&pool_account)?;

        // 4. 计算相关PDA地址
        let raydium_program_id = utils::solana::ConfigManager::get_raydium_program_id()?;

        // AMM配置地址 - 从池子状态获取
        // 注意：pool_state.amm_config 是配置的 Pubkey，而不是索引
        // 我们需要从其他地方获取配置索引，这里先设为0作为默认值
        let config_index = 0u16; // TODO: 需要从其他来源获取正确的配置索引
        let (amm_config_key, _) = Pubkey::find_program_address(&["amm_config".as_bytes(), &config_index.to_be_bytes()], &raydium_program_id);

        // TickArray Bitmap Extension地址
        let (tickarray_bitmap_extension, _) = Pubkey::find_program_address(&["pool_tick_array_bitmap_extension".as_bytes(), pool_pubkey.as_ref()], &raydium_program_id);

        // Observation地址
        let (observation_key, _) = Pubkey::find_program_address(&["observation".as_bytes(), pool_pubkey.as_ref()], &raydium_program_id);

        // 5. 批量获取mint和vault信息
        let load_pubkeys = vec![pool_state.token_mint_0, pool_state.token_mint_1, pool_state.token_vault_0, pool_state.token_vault_1];

        let accounts = account_loader.load_multiple_accounts(&load_pubkeys).await?;

        // 6. 解析mint信息
        let mint0_account = accounts[0].as_ref().ok_or_else(|| anyhow::anyhow!("无法获取mint0账户"))?;
        let mint1_account = accounts[1].as_ref().ok_or_else(|| anyhow::anyhow!("无法获取mint1账户"))?;

        let mint0_state = Mint::unpack(&mint0_account.data)?;
        let mint1_state = Mint::unpack(&mint1_account.data)?;

        // 7. 计算当前价格和tick
        let current_sqrt_price = pool_state.sqrt_price_x64;
        let current_tick = pool_state.tick_current;
        let current_price = self.calculate_price_from_sqrt_price_x64(current_sqrt_price, mint0_state.decimals, mint1_state.decimals);

        // 8. 构建ClmmPool结构
        let now = chrono::Utc::now().timestamp() as u64;

        let pool = ClmmPool {
            id: None,
            pool_address: pool_address.to_string(),
            amm_config_address: amm_config_key.to_string(),
            config_index,

            mint0: TokenInfo {
                mint_address: pool_state.token_mint_0.to_string(),
                decimals: mint0_state.decimals,
                owner: mint0_account.owner.to_string(),
                symbol: None, // 需要额外查询获取
                name: None,   // 需要额外查询获取
            },

            mint1: TokenInfo {
                mint_address: pool_state.token_mint_1.to_string(),
                decimals: mint1_state.decimals,
                owner: mint1_account.owner.to_string(),
                symbol: None, // 需要额外查询获取
                name: None,   // 需要额外查询获取
            },

            price_info: PriceInfo {
                initial_price: current_price, // 使用当前价格作为初始价格
                sqrt_price_x64: current_sqrt_price.to_string(),
                initial_tick: current_tick,
                current_price: Some(current_price),
                current_tick: Some(current_tick),
            },

            vault_info: VaultInfo {
                token_vault_0: pool_state.token_vault_0.to_string(),
                token_vault_1: pool_state.token_vault_1.to_string(),
            },

            extension_info: ExtensionInfo {
                observation_address: observation_key.to_string(),
                tickarray_bitmap_extension: tickarray_bitmap_extension.to_string(),
            },

            creator_wallet: pool_state.owner.to_string(), // 使用池子owner作为创建者
            open_time: pool_state.open_time,
            created_at: now,
            updated_at: now,
            transaction_info: None,
            status: PoolStatus::Active, // 已存在的池子认为是活跃状态

            sync_status: SyncStatus {
                last_sync_at: now,
                sync_version: 1,
                needs_sync: false, // 刚从链上获取，不需要同步
                sync_error: None,
            },

            pool_type: PoolType::Concentrated, // 当前只支持CLMM池
        };

        debug!("✅ 池子信息加载完成: {}", pool_address);
        Ok(pool)
    }

    /// 从sqrt_price_x64计算实际价格
    fn calculate_price_from_sqrt_price_x64(&self, sqrt_price_x64: u128, decimals0: u8, decimals1: u8) -> f64 {
        // sqrt_price_x64 = sqrt(price) * 2^64
        // price = (sqrt_price_x64 / 2^64)^2
        let sqrt_price = sqrt_price_x64 as f64 / (1u128 << 64) as f64;
        let price = sqrt_price * sqrt_price;

        // 根据decimals调整价格
        let decimal_adjustment = 10_f64.powi(decimals0 as i32 - decimals1 as i32);
        price * decimal_adjustment
    }
}
