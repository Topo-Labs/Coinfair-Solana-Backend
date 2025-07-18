use anyhow::Result;
use base64;
use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use tracing::info;

use super::{ConfigManager, PDACalculator};

/// Position工具类 - 统一管理Position相关的计算和操作
pub struct PositionUtils<'a> {
    rpc_client: &'a RpcClient,
}

impl<'a> PositionUtils<'a> {
    pub fn new(rpc_client: &'a RpcClient) -> Self {
        Self { rpc_client }
    }

    /// 价格转换为sqrt_price_x64
    pub fn price_to_sqrt_price_x64(&self, price: f64, decimals_0: u8, decimals_1: u8) -> u128 {
        // 调整小数位数差异
        let decimal_adjustment = 10_f64.powi(decimals_0 as i32 - decimals_1 as i32);
        let adjusted_price = price / decimal_adjustment;

        // 计算sqrt_price
        let sqrt_price = adjusted_price.sqrt();

        // 转换为Q64.64格式
        (sqrt_price * (1u128 << 64) as f64) as u128
    }

    /// sqrt_price_x64转换为价格
    pub fn sqrt_price_x64_to_price(&self, sqrt_price_x64: u128, decimals_0: u8, decimals_1: u8) -> f64 {
        let sqrt_price = sqrt_price_x64 as f64 / (1u128 << 64) as f64;
        let price = sqrt_price * sqrt_price;

        // 调整小数位数
        let decimal_adjustment = 10_f64.powi(decimals_0 as i32 - decimals_1 as i32);
        price * decimal_adjustment
    }

    /// 根据价格计算tick索引
    pub fn price_to_tick(&self, price: f64, decimals_0: u8, decimals_1: u8) -> Result<i32> {
        let sqrt_price_x64 = self.price_to_sqrt_price_x64(price, decimals_0, decimals_1);
        raydium_amm_v3::libraries::tick_math::get_tick_at_sqrt_price(sqrt_price_x64).map_err(|e| anyhow::anyhow!("价格转tick失败: {:?}", e))
    }

    /// 根据tick计算价格
    pub fn tick_to_price(&self, tick: i32, decimals_0: u8, decimals_1: u8) -> Result<f64> {
        let sqrt_price_x64 = raydium_amm_v3::libraries::tick_math::get_sqrt_price_at_tick(tick).map_err(|e| anyhow::anyhow!("tick转价格失败: {:?}", e))?;
        Ok(self.sqrt_price_x64_to_price(sqrt_price_x64, decimals_0, decimals_1))
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
    pub fn calculate_liquidity_from_single_amount(&self, current_sqrt_price_x64: u128, sqrt_price_lower_x64: u128, sqrt_price_upper_x64: u128, amount: u64, is_token_0: bool) -> Result<u128> {
        if is_token_0 {
            Ok(raydium_amm_v3::libraries::liquidity_math::get_liquidity_from_single_amount_0(
                current_sqrt_price_x64,
                sqrt_price_lower_x64,
                sqrt_price_upper_x64,
                amount,
            ))
        } else {
            Ok(raydium_amm_v3::libraries::liquidity_math::get_liquidity_from_single_amount_1(
                current_sqrt_price_x64,
                sqrt_price_lower_x64,
                sqrt_price_upper_x64,
                amount,
            ))
        }
    }

    /// 根据流动性计算token数量
    pub fn calculate_amounts_from_liquidity(&self, current_tick: i32, current_sqrt_price_x64: u128, tick_lower: i32, tick_upper: i32, liquidity: u128) -> Result<(u64, u64)> {
        raydium_amm_v3::libraries::liquidity_math::get_delta_amounts_signed(current_tick, current_sqrt_price_x64, tick_lower, tick_upper, liquidity as i128).map_err(|e| anyhow::anyhow!("流动性计算金额失败: {:?}", e))
    }

    /// 应用滑点保护
    pub fn apply_slippage(&self, amount: u64, slippage_percent: f64, is_min: bool) -> u64 {
        let slippage_factor = slippage_percent / 100.0;
        if is_min {
            // 最小输出，减去滑点
            (amount as f64 * (1.0 - slippage_factor)) as u64
        } else {
            // 最大输入，加上滑点
            (amount as f64 * (1.0 + slippage_factor)) as u64
        }
    }

    /// 检查位置是否已存在
    pub async fn find_existing_position(&self, user_wallet: &Pubkey, pool_address: &Pubkey, tick_lower: i32, tick_upper: i32) -> Result<Option<ExistingPosition>> {
        info!("🔍 检查是否存在相同范围的位置");

        // 获取用户所有NFT和position
        let position_nfts = self.get_user_position_nfts(user_wallet).await?;

        for nft_info in position_nfts {
            // 加载position状态
            if let Ok(position_account) = self.rpc_client.get_account(&nft_info.position_pda) {
                if let Ok(position_state) = self.deserialize_position_state(&position_account) {
                    if position_state.pool_id == *pool_address && position_state.tick_lower_index == tick_lower && position_state.tick_upper_index == tick_upper {
                        return Ok(Some(ExistingPosition {
                            nft_mint: nft_info.nft_mint,
                            position_key: nft_info.position_pda,
                            liquidity: position_state.liquidity,
                        }));
                    }
                }
            }
        }

        Ok(None)
    }

    /// 获取用户的position NFTs
    pub async fn get_user_position_nfts(&self, user_wallet: &Pubkey) -> Result<Vec<PositionNftInfo>> {
        use base64::Engine;
        use solana_sdk::program_pack::Pack;
        use spl_token::state::Account as TokenAccount;

        // 获取用户所有代币账户
        let token_accounts = self
            .rpc_client
            .get_token_accounts_by_owner(user_wallet, solana_client::rpc_request::TokenAccountsFilter::ProgramId(spl_token::id()))?;

        let mut position_nfts = Vec::new();
        let raydium_program_id = ConfigManager::get_raydium_program_id()?;

        for token_account_info in token_accounts {
            // 直接尝试解析账户数据
            if let Ok(raw_account) = self.rpc_client.get_account(&Pubkey::from_str(&token_account_info.pubkey)?) {
                if let Ok(token_account) = TokenAccount::unpack(&raw_account.data) {
                    // 检查是否为NFT（amount = 1, decimals = 0）
                    if token_account.amount == 1 {
                        // 检查mint的decimals
                        if let Ok(mint_account) = self.rpc_client.get_account(&token_account.mint) {
                            if let Ok(mint_state) = spl_token::state::Mint::unpack(&mint_account.data) {
                                if mint_state.decimals == 0 {
                                    // 计算position PDA
                                    let (position_pda, _) = Pubkey::find_program_address(&[b"position", token_account.mint.as_ref()], &raydium_program_id);

                                    // 解析账户地址
                                    let nft_account_pubkey = Pubkey::from_str(&token_account_info.pubkey)?;

                                    position_nfts.push(PositionNftInfo {
                                        nft_mint: token_account.mint,
                                        nft_account: nft_account_pubkey,
                                        position_pda,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(position_nfts)
    }

    /// 反序列化position状态
    pub fn deserialize_position_state(&self, account: &solana_sdk::account::Account) -> Result<PersonalPositionState> {
        let mut data: &[u8] = &account.data;
        anchor_lang::AccountDeserialize::try_deserialize(&mut data).map_err(|e| anyhow::anyhow!("反序列化position状态失败: {:?}", e))
    }

    /// 计算tick array的起始索引
    pub fn get_tick_array_start_index(&self, tick: i32, tick_spacing: u16) -> i32 {
        raydium_amm_v3::states::TickArrayState::get_array_start_index(tick, tick_spacing)
    }

    /// 构建remaining accounts（tick arrays和bitmap）
    pub async fn build_remaining_accounts(&self, pool_address: &Pubkey, tick_lower: i32, tick_upper: i32, tick_spacing: u16) -> Result<Vec<solana_sdk::instruction::AccountMeta>> {
        let raydium_program_id = ConfigManager::get_raydium_program_id()?;
        let mut remaining_accounts = Vec::new();

        // 添加tick array bitmap extension
        let (bitmap_pda, _) = PDACalculator::calculate_tickarray_bitmap_extension_pda(&raydium_program_id, pool_address);
        remaining_accounts.push(solana_sdk::instruction::AccountMeta::new(bitmap_pda, false));

        // 计算需要的tick arrays
        let tick_array_lower_start = self.get_tick_array_start_index(tick_lower, tick_spacing);
        let tick_array_upper_start = self.get_tick_array_start_index(tick_upper, tick_spacing);

        // 添加下限tick array
        let (tick_array_lower_pda, _) = PDACalculator::calculate_tick_array_pda(&raydium_program_id, pool_address, tick_array_lower_start);
        remaining_accounts.push(solana_sdk::instruction::AccountMeta::new(tick_array_lower_pda, false));

        // 如果上限和下限不在同一个tick array中，添加上限tick array
        if tick_array_lower_start != tick_array_upper_start {
            let (tick_array_upper_pda, _) = PDACalculator::calculate_tick_array_pda(&raydium_program_id, pool_address, tick_array_upper_start);
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

/// 用户NFT位置信息
#[derive(Debug, Clone)]
pub struct PositionNftInfo {
    pub nft_mint: Pubkey,
    pub nft_account: Pubkey,
    pub position_pda: Pubkey,
}

/// 已存在的位置信息
#[derive(Debug, Clone)]
pub struct ExistingPosition {
    pub nft_mint: Pubkey,
    pub position_key: Pubkey,
    pub liquidity: u128,
}

/// 简化的PersonalPositionState结构体（用于反序列化）
#[derive(Debug, Clone)]
pub struct PersonalPositionState {
    pub nft_mint: Pubkey,
    pub pool_id: Pubkey,
    pub tick_lower_index: i32,
    pub tick_upper_index: i32,
    pub liquidity: u128,
    pub token_fees_owed_0: u64,
    pub token_fees_owed_1: u64,
}

impl anchor_lang::AccountDeserialize for PersonalPositionState {
    fn try_deserialize(buf: &mut &[u8]) -> anchor_lang::Result<Self> {
        // 跳过discriminator
        if buf.len() < 8 {
            return Err(anchor_lang::error::ErrorCode::AccountDidNotDeserialize.into());
        }
        *buf = &buf[8..];

        // 简化的反序列化实现
        if buf.len() < 256 {
            return Err(anchor_lang::error::ErrorCode::AccountDidNotDeserialize.into());
        }

        // 读取关键字段（这里需要根据实际的内存布局调整）
        let nft_mint = Pubkey::new_from_array({
            let mut array = [0u8; 32];
            array.copy_from_slice(&buf[0..32]);
            array
        });

        let pool_id = Pubkey::new_from_array({
            let mut array = [0u8; 32];
            array.copy_from_slice(&buf[32..64]);
            array
        });

        // 跳过其他字段，直接读取tick indexes
        let tick_lower_index = i32::from_le_bytes([buf[64], buf[65], buf[66], buf[67]]);
        let tick_upper_index = i32::from_le_bytes([buf[68], buf[69], buf[70], buf[71]]);

        // 读取流动性（u128）
        let mut liquidity_bytes = [0u8; 16];
        liquidity_bytes.copy_from_slice(&buf[72..88]);
        let liquidity = u128::from_le_bytes(liquidity_bytes);

        Ok(PersonalPositionState {
            nft_mint,
            pool_id,
            tick_lower_index,
            tick_upper_index,
            liquidity,
            token_fees_owed_0: 0, // 简化处理
            token_fees_owed_1: 0, // 简化处理
        })
    }

    fn try_deserialize_unchecked(_buf: &mut &[u8]) -> anchor_lang::Result<Self> {
        unimplemented!()
    }
}
