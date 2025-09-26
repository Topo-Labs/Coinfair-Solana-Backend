use anyhow::Result;
use solana_account_decoder::parse_token::TokenAccountType;
use solana_account_decoder::parse_token::UiAccountState;
use solana_account_decoder::UiAccountData;
use solana_client::rpc_client::RpcClient;
// use solana_sdk::program_pack::Pack;
use solana_sdk::pubkey::Pubkey;
// use spl_token::state::Account as TokenAccount;
use std::str::FromStr;
use tracing::info;
use tracing::warn;

use super::{ConfigManager, PDACalculator};

/// Position工具类 - 统一管理Position相关的计算和操作
pub struct PositionUtils<'a> {
    rpc_client: &'a RpcClient,
}

impl<'a> PositionUtils<'a> {
    pub fn new(rpc_client: &'a RpcClient) -> Self {
        Self { rpc_client }
    }

    pub fn price_to_sqrt_price_x64(&self, price: f64, decimals_0: u8, decimals_1: u8) -> u128 {
        raydium_amm_v3_client::price_to_sqrt_price_x64(price, decimals_0, decimals_1)
    }

    pub fn sqrt_price_x64_to_price(&self, price: u128, decimals_0: u8, decimals_1: u8) -> f64 {
        raydium_amm_v3_client::sqrt_price_x64_to_price(price, decimals_0, decimals_1)
    }

    // /// 根据价格计算tick索引
    // pub fn price_to_tick(&self, price: f64, decimals_0: u8, decimals_1: u8) -> Result<i32> {
    //     let sqrt_price_x64 = raydium_amm_v3_client::price_to_sqrt_price_x64(price, decimals_0, decimals_1);
    //     raydium_amm_v3::libraries::tick_math::get_tick_at_sqrt_price(sqrt_price_x64)
    //         .map_err(|e| anyhow::anyhow!("价格转tick失败: {:?}", e))
    // }

    // /// 根据tick计算价格
    // pub fn tick_to_price(&self, tick: i32, decimals_0: u8, decimals_1: u8) -> Result<f64> {
    //     let sqrt_price_x64 = raydium_amm_v3::libraries::tick_math::get_sqrt_price_at_tick(tick)
    //         .map_err(|e| anyhow::anyhow!("tick转价格失败: {:?}", e))?;
    //     Ok(raydium_amm_v3_client::sqrt_price_x64_to_price(
    //         sqrt_price_x64,
    //         decimals_0,
    //         decimals_1,
    //     ))
    // }

    pub fn price_to_tick(&self, price: f64, _decimals_0: u8, _decimals_1: u8) -> Result<i32> {
        // 直接使用新导出的函数，该函数已经适配了x^4*y=k曲线
        Ok(raydium_amm_v3_client::price_to_tick(price))
    }

    /// 根据tick计算价格
    pub fn tick_to_price(&self, tick: i32, decimals_0: u8, decimals_1: u8) -> Result<f64> {
        // 直接使用新导出的函数，该函数已经适配了x^4*y=k曲线
        let price = raydium_amm_v3_client::tick_to_price(tick);
        // 应用小数位调整
        Ok(price * 10_f64.powi(decimals_1 as i32 - decimals_0 as i32))
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
        // 注意：对于OpenPosition，我们需要计算最大输入金额，所以is_min应该为false
        // 这将增加金额以提供滑点保护
        if is_min {
            // 减少金额（用于计算最小输出）
            ((amount as f64) * (1.0 - slippage_percent / 100.0)).floor() as u64
        } else {
            // 增加金额（用于计算最大输入） - 与CLI版本的round_up=true一致
            ((amount as f64) * (1.0 + slippage_percent / 100.0)).ceil() as u64
        }
    }

    /// 检查仓位是否已存在 - 带重试逻辑
    pub async fn find_existing_position(
        &self,
        user_wallet: &Pubkey,
        pool_address: &Pubkey,
        tick_lower: i32,
        tick_upper: i32,
    ) -> Result<Option<ExistingPosition>> {
        info!("🔍 检查是否存在相同范围的仓位");
        info!("  钱包: {}", user_wallet);
        info!("  池子: {}", pool_address);
        info!("  Tick范围: {} - {}", tick_lower, tick_upper);

        match self
            .find_existing_position_internal(user_wallet, pool_address, tick_lower, tick_upper)
            .await
        {
            Ok(Some(position)) => {
                info!("✅ 找到相同范围的仓位: {}", position.position_key);
                return Ok(Some(position));
            }
            Ok(None) => {
                info!("✅ 确认没有相同范围的仓位");
                return Ok(None);
            }
            Err(e) => {
                warn!("⚠️ 查找仓位失败: {:?}", e);
                return Err(e);
            }
        }
    }

    /// 内部查找方法 - 单次尝试
    async fn find_existing_position_internal(
        &self,
        user_wallet: &Pubkey,
        pool_address: &Pubkey,
        tick_lower: i32,
        tick_upper: i32,
    ) -> Result<Option<ExistingPosition>> {
        // 获取用户所有NFT和position
        let position_nfts = self.get_user_position_nfts(user_wallet).await?;
        info!("🔍 找到 {} 个Position NFT", position_nfts.len());

        for (index, nft_info) in position_nfts.iter().enumerate() {
            info!(
                "🔍 检查NFT #{}: mint={}, position_pda={}",
                index + 1,
                nft_info.nft_mint,
                nft_info.position_pda
            );

            let position_account = self.rpc_client.get_account(&nft_info.position_pda);
            match position_account {
                Ok(position_account) => {
                    info!(
                        "  ✅ 成功获取position账户数据，大小: {} bytes",
                        position_account.data.len()
                    );

                    match self.deserialize_position_state(&position_account) {
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
                                return Ok(Some(ExistingPosition {
                                    nft_mint: nft_info.nft_mint,
                                    nft_token_account: nft_info.nft_account,
                                    position_key: nft_info.position_pda,
                                    liquidity: position_state.liquidity,
                                    nft_token_program: nft_info.token_program, // 添加Token Program信息
                                }));
                            } else {
                                info!("  ⏭️ 仓位不匹配，继续搜索");
                            }
                        }
                        Err(e) => {
                            warn!("  ⚠️ 反序列化position状态失败: {:?}", e);
                        }
                    }
                }
                Err(e) => {
                    warn!("  ⚠️ 获取position账户失败: {:?}", e);
                }
            }
        }

        Ok(None)
    }

    /// 获取用户的position NFTs（同时支持经典Token和Token-2022）- 增强版本
    pub async fn get_user_position_nfts(&self, user_wallet: &Pubkey) -> Result<Vec<PositionNftInfo>> {
        info!("🔍 获取用户的Position NFTs（包括Token和Token-2022）");

        let mut all_position_nfts = Vec::new();

        // 1. 获取经典Token的NFT - 使用 Confirmed commitment 确保数据新鲜度
        let classic_nfts = self
            .get_position_nfts_by_program_enhanced(user_wallet, &spl_token::id())
            .await?;
        all_position_nfts.extend(classic_nfts.clone());

        // 2. 获取Token-2022的NFT - 使用 Confirmed commitment 确保数据新鲜度
        let token2022_nfts = self
            .get_position_nfts_by_program_enhanced(user_wallet, &spl_token_2022::id())
            .await?;
        all_position_nfts.extend(token2022_nfts.clone());

        info!(
            "  找到 {} 个经典Token NFT，{} 个Token-2022 NFT，总共 {} 个NFT",
            classic_nfts.len(),
            token2022_nfts.len(),
            all_position_nfts.len()
        );

        // 3. 按NFT mint地址排序以确保一致性
        all_position_nfts.sort_by_key(|nft| nft.nft_mint.to_string());

        Ok(all_position_nfts)
    }

    /// 根据特定的Token程序获取position NFTs - 增强版本，使用 Confirmed commitment
    async fn get_position_nfts_by_program_enhanced(
        &self,
        user_wallet: &Pubkey,
        token_program: &Pubkey,
    ) -> Result<Vec<PositionNftInfo>> {
        use solana_sdk::commitment_config::CommitmentConfig;

        info!(
            "🔍 获取{}程序的Position NFT",
            if *token_program == spl_token::id() {
                "经典Token"
            } else {
                "Token-2022"
            }
        );

        // 使用 Confirmed commitment 确保获取到最新数据
        let commitment = CommitmentConfig::confirmed();

        // 获取指定Token程序的所有代币账户 - 使用 Confirmed commitment
        let config = solana_client::rpc_request::TokenAccountsFilter::ProgramId(*token_program);
        let token_accounts_response =
            self.rpc_client
                .get_token_accounts_by_owner_with_commitment(user_wallet, config, commitment)?;

        let token_accounts = token_accounts_response.value;
        info!("  找到 {} 个Token账户", token_accounts.len());

        let mut position_nfts = Vec::new();
        let raydium_program_id = ConfigManager::get_raydium_program_id()?;

        for token_account_info in token_accounts {
            // info!("  检查Token账户 {}", token_account_info.pubkey);
            if let UiAccountData::Json(parsed_account) = token_account_info.account.data {
                if parsed_account.program == "spl-token" || parsed_account.program == "spl-token-2022" {
                    if let Ok(TokenAccountType::Account(ui_token_account)) =
                        serde_json::from_value(parsed_account.parsed)
                    {
                        let _frozen = ui_token_account.state == UiAccountState::Frozen;

                        let token = ui_token_account
                            .mint
                            .parse::<Pubkey>()
                            .unwrap_or_else(|err| panic!("Invalid mint: {}", err));
                        // let token_account = token_account_info
                        //     .pubkey
                        //     .parse::<Pubkey>()
                        //     .unwrap_or_else(|err| panic!("Invalid token account: {}", err));
                        let token_amount = ui_token_account
                            .token_amount
                            .amount
                            .parse::<u64>()
                            .unwrap_or_else(|err| panic!("Invalid token amount: {}", err));

                        let _close_authority = ui_token_account.close_authority.map_or(*user_wallet, |s| {
                            s.parse::<Pubkey>()
                                .unwrap_or_else(|err| panic!("Invalid close authority: {}", err))
                        });

                        if ui_token_account.token_amount.decimals == 0 && token_amount == 1 {
                            // 计算position PDA
                            let (position_pda, _) =
                                Pubkey::find_program_address(&[b"position", token.as_ref()], &raydium_program_id);
                            // 解析账户地址
                            let nft_account_pubkey = Pubkey::from_str(&token_account_info.pubkey)?;
                            info!("      ✅ 找到Position NFT: mint={}, pda={}", token, position_pda);

                            position_nfts.push(PositionNftInfo {
                                nft_mint: token,
                                nft_account: nft_account_pubkey,
                                position_pda,
                                token_program: *token_program, // 记录Token Program信息
                            });
                        }
                    }
                }
            }
        }

        info!(
            "  ✅ 从{}程序找到 {} 个Position NFT",
            if *token_program == spl_token::id() {
                "经典Token"
            } else {
                "Token-2022"
            },
            position_nfts.len()
        );

        Ok(position_nfts)
    }

    /// 反序列化position状态
    pub fn deserialize_position_state(&self, account: &solana_sdk::account::Account) -> Result<PersonalPositionState> {
        let mut data: &[u8] = &account.data;
        anchor_lang::AccountDeserialize::try_deserialize(&mut data)
            .map_err(|e| anyhow::anyhow!("反序列化position状态失败: {:?}", e))
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

/// 用户NFT仓位信息
#[derive(Debug, Clone, Copy)]
pub struct PositionNftInfo {
    pub nft_mint: Pubkey,
    pub nft_account: Pubkey,
    pub position_pda: Pubkey,
    pub token_program: Pubkey, // 添加Token Program信息
}

/// 已存在的仓位信息
#[derive(Debug, Clone)]
pub struct ExistingPosition {
    pub nft_mint: Pubkey,
    pub nft_token_account: Pubkey,
    pub position_key: Pubkey,
    pub liquidity: u128,
    pub nft_token_program: Pubkey, // 添加Token Program信息
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
        // 使用正确的Raydium CLMM PersonalPositionState反序列化
        // 直接使用raydium_amm_v3的反序列化方法
        let position_state = raydium_amm_v3::states::PersonalPositionState::try_deserialize(buf)?;

        Ok(PersonalPositionState {
            nft_mint: position_state.nft_mint,
            pool_id: position_state.pool_id,
            tick_lower_index: position_state.tick_lower_index,
            tick_upper_index: position_state.tick_upper_index,
            liquidity: position_state.liquidity,
            token_fees_owed_0: position_state.token_fees_owed_0,
            token_fees_owed_1: position_state.token_fees_owed_1,
        })
    }

    fn try_deserialize_unchecked(buf: &mut &[u8]) -> anchor_lang::Result<Self> {
        let position_state = raydium_amm_v3::states::PersonalPositionState::try_deserialize_unchecked(buf)?;

        Ok(PersonalPositionState {
            nft_mint: position_state.nft_mint,
            pool_id: position_state.pool_id,
            tick_lower_index: position_state.tick_lower_index,
            tick_upper_index: position_state.tick_upper_index,
            liquidity: position_state.liquidity,
            token_fees_owed_0: position_state.token_fees_owed_0,
            token_fees_owed_1: position_state.token_fees_owed_1,
        })
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_position_seed_consistency() {
        // 验证我们使用的 "position" 字符串是否与raydium_amm_v3::states::POSITION_SEED一致
        let our_seed = b"position";
        let raydium_seed = raydium_amm_v3::states::POSITION_SEED.as_bytes();

        assert_eq!(
            our_seed,
            raydium_seed,
            "我们使用的POSITION_SEED与raydium库不一致! 我们使用: {:?}, raydium使用: {:?}",
            std::str::from_utf8(our_seed),
            std::str::from_utf8(raydium_seed)
        );
    }

    #[test]
    fn test_pda_calculation_consistency() {
        use solana_sdk::pubkey::Pubkey;
        use std::str::FromStr;

        // 测试用的NFT mint地址
        let test_mint = Pubkey::from_str("11111111111111111111111111111112").unwrap();
        let test_program_id = Pubkey::from_str("CAMMCzo5YL8w4VFF8KVHrK22GGUQpMTdQa5KH8DF9EgV").unwrap();

        // 我们的计算方式
        let (our_pda, our_bump) = Pubkey::find_program_address(&[b"position", test_mint.as_ref()], &test_program_id);

        // 外部项目的计算方式
        let (external_pda, external_bump) = Pubkey::find_program_address(
            &[raydium_amm_v3::states::POSITION_SEED.as_bytes(), test_mint.as_ref()],
            &test_program_id,
        );

        assert_eq!(
            our_pda, external_pda,
            "PDA计算不一致! 我们计算: {}, 外部项目计算: {}",
            our_pda, external_pda
        );
        assert_eq!(
            our_bump, external_bump,
            "PDA bump不一致! 我们计算: {}, 外部项目计算: {}",
            our_bump, external_bump
        );
    }
}
