use anyhow::Result;
use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use std::collections::VecDeque;
use std::str::FromStr;
use tracing::{info, warn};

use super::{AccountLoader, ConfigManager, TokenUtils, TransferFeeCalculator};

/// 交换计算器 - 抽取并统一管理复杂的交换计算逻辑
pub struct SwapCalculator<'a> {
    rpc_client: &'a RpcClient,
}

impl<'a> SwapCalculator<'a> {
    pub fn new(rpc_client: &'a RpcClient) -> Self {
        Self { rpc_client }
    }

    /// 计算价格影响 - 抽取自 solana_service.rs 的 calculate_price_impact 方法
    pub async fn calculate_price_impact(
        &self,
        input_mint: &str,
        output_mint: &str,
        input_amount: u64,
        output_amount: u64,
        pool_address: &str,
    ) -> Result<f64> {
        info!("💰 计算价格影响");

        let pool_pubkey = Pubkey::from_str(pool_address)?;
        let input_mint_pubkey = Pubkey::from_str(input_mint)?;
        let output_mint_pubkey = Pubkey::from_str(output_mint)?;

        // 1. 加载池子状态和mint账户
        let load_accounts = vec![pool_pubkey, input_mint_pubkey, output_mint_pubkey];
        let accounts = self.rpc_client.get_multiple_accounts(&load_accounts)?;

        let pool_account = accounts[0].as_ref().ok_or_else(|| anyhow::anyhow!("无法加载池子账户"))?;
        let input_mint_account = accounts[1].as_ref().ok_or_else(|| anyhow::anyhow!("无法加载输入mint账户"))?;
        let output_mint_account = accounts[2].as_ref().ok_or_else(|| anyhow::anyhow!("无法加载输出mint账户"))?;

        let pool_state: raydium_amm_v3::states::PoolState = self.deserialize_anchor_account(pool_account)?;
        let _input_mint_state = spl_token_2022::extension::StateWithExtensions::<spl_token_2022::state::Mint>::unpack(&input_mint_account.data)?;
        let _output_mint_state = spl_token_2022::extension::StateWithExtensions::<spl_token_2022::state::Mint>::unpack(&output_mint_account.data)?;

        // 2. 确定代币是否是mint0或mint1
        let is_input_mint0 = input_mint_pubkey == pool_state.token_mint_0;
        let (input_decimals, output_decimals) = if is_input_mint0 {
            (pool_state.mint_decimals_0, pool_state.mint_decimals_1)
        } else {
            (pool_state.mint_decimals_1, pool_state.mint_decimals_0)
        };

        // 3. 获取池子中的代币余额
        let (input_vault, output_vault) = if is_input_mint0 {
            (pool_state.token_vault_0, pool_state.token_vault_1)
        } else {
            (pool_state.token_vault_1, pool_state.token_vault_0)
        };

        // 4. 加载vault账户以获取实际余额
        let vault_accounts = self.rpc_client.get_multiple_accounts(&[input_vault, output_vault])?;
        let input_vault_account = vault_accounts[0].as_ref().ok_or_else(|| anyhow::anyhow!("无法加载输入vault账户"))?;
        let output_vault_account = vault_accounts[1].as_ref().ok_or_else(|| anyhow::anyhow!("无法加载输出vault账户"))?;

        let input_vault_state = spl_token_2022::extension::StateWithExtensions::<spl_token_2022::state::Account>::unpack(&input_vault_account.data)?;
        let output_vault_state = spl_token_2022::extension::StateWithExtensions::<spl_token_2022::state::Account>::unpack(&output_vault_account.data)?;

        let input_balance = input_vault_state.base.amount;
        let output_balance = output_vault_state.base.amount;

        // 5. 计算价格影响 = (输入金额^2 / (输入余额 * (输入余额 + 输入金额))) * 100
        let input_amount_f64 = input_amount as f64;
        let output_amount_f64 = output_amount as f64;
        let input_balance_f64 = input_balance as f64;
        let output_balance_f64 = output_balance as f64;

        let price_impact = if input_balance_f64 > 0.0 && output_balance_f64 > 0.0 {
            let input_impact = (input_amount_f64 * input_amount_f64) / (input_balance_f64 * (input_balance_f64 + input_amount_f64));
            let output_impact = (output_amount_f64 * output_amount_f64) / (output_balance_f64 * (output_balance_f64 - output_amount_f64));
            let total_impact = (input_impact + output_impact) / 2.0 * 100.0;
            total_impact.min(100.0)
        } else {
            0.0
        };

        info!("💰 价格影响计算结果:");
        info!("  输入金额: {} ({}位小数)", input_amount, input_decimals);
        info!("  输出金额: {} ({}位小数)", output_amount, output_decimals);
        info!("  输入池子余额: {}", input_balance);
        info!("  输出池子余额: {}", output_balance);
        info!("  价格影响: {:.4}%", price_impact);

        Ok(price_impact)
    }

    /// 使用CLI逻辑计算交换输出
    pub async fn calculate_output_using_cli_logic(
        &self,
        input_mint: &str,
        output_mint: &str,
        amount: u64,
        pool_address: &str,
        base_in: bool,
        slippage_bps: u16,
    ) -> Result<(u64, u64)> {
        info!("执行与CLI完全相同的交换计算逻辑");

        let pool_pubkey = Pubkey::from_str(pool_address)?;
        let input_mint_pubkey = Pubkey::from_str(input_mint)?;
        let output_mint_pubkey = Pubkey::from_str(output_mint)?;

        // 1. 使用ConfigManager获取配置
        let raydium_program_id = ConfigManager::get_raydium_program_id()?;

        // 2. 使用TokenUtils标准化mint顺序
        let (mint0, mint1, _zero_for_one) = TokenUtils::normalize_mint_order(&input_mint_pubkey, &output_mint_pubkey);

        // 3. 使用AccountLoader加载核心交换账户
        let account_loader = AccountLoader::new(self.rpc_client);
        let swap_accounts = account_loader.load_swap_core_accounts(&pool_pubkey, &input_mint_pubkey, &output_mint_pubkey).await?;

        // 4. 为了保持与CLI完全一致，获取原始mint账户数据用于transfer fee计算
        let load_accounts = vec![mint0, mint1];
        let mint_accounts = self.rpc_client.get_multiple_accounts(&load_accounts)?;
        let mint0_account = mint_accounts[0].as_ref().ok_or_else(|| anyhow::anyhow!("无法加载mint0账户"))?;
        let mint1_account = mint_accounts[1].as_ref().ok_or_else(|| anyhow::anyhow!("无法加载mint1账户"))?;

        // 5. 使用TransferFeeCalculator计算transfer fee
        let epoch = self.rpc_client.get_epoch_info()?.epoch;
        let transfer_fee = if base_in {
            if swap_accounts.zero_for_one {
                TransferFeeCalculator::get_transfer_fee_from_mint_state_simple(&mint0_account.data, epoch, amount)?
            } else {
                TransferFeeCalculator::get_transfer_fee_from_mint_state_simple(&mint1_account.data, epoch, amount)?
            }
        } else {
            0
        };
        let amount_specified = amount.checked_sub(transfer_fee).unwrap();

        info!("💰 Transfer fee计算:");
        info!("  原始金额: {}", amount);
        info!("  Transfer fee: {}", transfer_fee);
        info!("  扣除费用后金额: {}", amount_specified);

        // 6. 加载当前和接下来的5个tick arrays
        let mut tick_arrays = self.load_cur_and_next_five_tick_array_like_cli(
            &swap_accounts.pool_state,
            &swap_accounts.tickarray_bitmap_extension,
            swap_accounts.zero_for_one,
            &raydium_program_id,
            &pool_pubkey,
        ).await?;

        // 7. 使用CLI完全相同的get_out_put_amount_and_remaining_accounts逻辑
        let (output_amount, _tick_array_indexs) = self.get_output_amount_and_remaining_accounts_cli_exact(
            amount_specified,
            None,
            swap_accounts.zero_for_one,
            base_in,
            &swap_accounts.amm_config_state,
            &swap_accounts.pool_state,
            &swap_accounts.tickarray_bitmap_extension,
            &mut tick_arrays,
        )?;

        // 8. 使用与CLI完全相同的slippage计算逻辑
        let other_amount_threshold = if base_in {
            let slippage = slippage_bps as f64 / 10000.0;
            let amount_with_slippage = (output_amount as f64 * (1.0 - slippage)) as u64;
            amount_with_slippage
        } else {
            let slippage = slippage_bps as f64 / 10000.0;
            let amount_with_slippage = (output_amount as f64 * (1.0 + slippage)) as u64;
            amount_with_slippage
        };

        info!("✅ CLI完全相同逻辑计算完成");
        info!("  输入金额: {} (原始: {})", amount_specified, amount);
        info!("  输出金额: {}", output_amount);
        info!("  滑点保护后阈值: {}", other_amount_threshold);
        info!("  Transfer fee: {}", transfer_fee);
        info!("  Zero for one: {}", swap_accounts.zero_for_one);

        Ok((output_amount, other_amount_threshold))
    }

    /// 从官方API获取remaining accounts（备用方案）
    pub async fn get_remaining_accounts_from_official_api(
        &self,
        _pool_id: &str,
        input_mint: &str,
        output_mint: &str,
        amount_specified: u64,
    ) -> Result<(Vec<String>, String)> {
        warn!("🌐 使用官方API获取remaining accounts（备用方案）");

        let url = format!(
            "https://transaction-v1.raydium.io/compute/swap-base-in?inputMint={}&outputMint={}&amount={}&slippageBps=50&txVersion=V0",
            input_mint, output_mint, amount_specified
        );

        let response = reqwest::get(&url).await?;
        if !response.status().is_success() {
            return Err(anyhow::anyhow!("官方API请求失败: {}", response.status()));
        }

        let data: serde_json::Value = response.json().await?;

        if let Some(route_plan) = data.get("data")
            .and_then(|d| d.get("routePlan"))
            .and_then(|r| r.as_array())
            .and_then(|arr| arr.first())
        {
            let remaining_accounts = route_plan
                .get("remainingAccounts")
                .and_then(|r| r.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect::<Vec<String>>())
                .unwrap_or_default();

            let last_pool_price_x64 = route_plan.get("lastPoolPriceX64")
                .and_then(|p| p.as_str())
                .unwrap_or("0")
                .to_string();

            info!("✅ 从官方API获取成功");
            info!("  Remaining accounts: {:?}", remaining_accounts);
            info!("  Pool price X64: {}", last_pool_price_x64);

            Ok((remaining_accounts, last_pool_price_x64))
        } else {
            Err(anyhow::anyhow!("无法从官方API响应中提取数据"))
        }
    }

    /// 反序列化anchor账户
    fn deserialize_anchor_account<T: anchor_lang::AccountDeserialize>(
        &self,
        account: &solana_sdk::account::Account,
    ) -> Result<T> {
        let mut data: &[u8] = &account.data;
        T::try_deserialize(&mut data).map_err(Into::into)
    }

    /// 加载当前和接下来的5个tick arrays
    pub async fn load_cur_and_next_five_tick_array_like_cli(
        &self,
        pool_state: &raydium_amm_v3::states::PoolState,
        tickarray_bitmap_extension: &raydium_amm_v3::states::TickArrayBitmapExtension,
        zero_for_one: bool,
        raydium_program_id: &Pubkey,
        pool_pubkey: &Pubkey,
    ) -> Result<VecDeque<raydium_amm_v3::states::TickArrayState>> {
        let (_, mut current_valid_tick_array_start_index) = pool_state
            .get_first_initialized_tick_array(&Some(*tickarray_bitmap_extension), zero_for_one)
            .map_err(|e| anyhow::anyhow!("获取第一个初始化的tick array失败: {:?}", e))?;

        let mut tick_array_keys = Vec::new();
        tick_array_keys.push(
            Pubkey::find_program_address(
                &[
                    "tick_array".as_bytes(),
                    pool_pubkey.as_ref(),
                    current_valid_tick_array_start_index.to_be_bytes().as_ref(),
                ],
                raydium_program_id,
            ).0,
        );

        let mut max_array_size = 5;
        while max_array_size != 0 {
            let next_tick_array_index = pool_state
                .next_initialized_tick_array_start_index(
                    &Some(*tickarray_bitmap_extension),
                    current_valid_tick_array_start_index,
                    zero_for_one,
                )
                .map_err(|e| anyhow::anyhow!("获取下一个tick array索引失败: {:?}", e))?;

            if next_tick_array_index.is_none() {
                break;
            }
            current_valid_tick_array_start_index = next_tick_array_index.unwrap();
            tick_array_keys.push(
                Pubkey::find_program_address(
                    &[
                        "tick_array".as_bytes(),
                        pool_pubkey.as_ref(),
                        current_valid_tick_array_start_index.to_be_bytes().as_ref(),
                    ],
                    raydium_program_id,
                ).0,
            );
            max_array_size -= 1;
        }

        let tick_array_rsps = self.rpc_client.get_multiple_accounts(&tick_array_keys)?;
        let mut tick_arrays = VecDeque::new();

        for tick_array in tick_array_rsps {
            match tick_array {
                Some(account) => {
                    let tick_array_state: raydium_amm_v3::states::TickArrayState = self.deserialize_anchor_account(&account)?;
                    tick_arrays.push_back(tick_array_state);
                }
                None => {
                    warn!("某个tick array账户不存在，跳过");
                }
            }
        }

        Ok(tick_arrays)
    }

    /// 精确移植CLI的get_out_put_amount_and_remaining_accounts函数逻辑
    pub fn get_output_amount_and_remaining_accounts_cli_exact(
        &self,
        input_amount: u64,
        sqrt_price_limit_x64: Option<u128>,
        zero_for_one: bool,
        is_base_input: bool,
        pool_config: &raydium_amm_v3::states::AmmConfig,
        pool_state: &raydium_amm_v3::states::PoolState,
        tickarray_bitmap_extension: &raydium_amm_v3::states::TickArrayBitmapExtension,
        tick_arrays: &mut VecDeque<raydium_amm_v3::states::TickArrayState>,
    ) -> Result<(u64, VecDeque<i32>)> {
        info!("执行CLI精确相同的get_out_put_amount_and_remaining_accounts逻辑");

        let (is_pool_current_tick_array, current_vaild_tick_array_start_index) = pool_state
            .get_first_initialized_tick_array(&Some(*tickarray_bitmap_extension), zero_for_one)
            .map_err(|e| anyhow::anyhow!("获取第一个初始化tick array失败: {:?}", e))?;

        let (amount_calculated, tick_array_start_index_vec) = self.swap_compute_cli_exact(
            zero_for_one,
            is_base_input,
            is_pool_current_tick_array,
            pool_config.trade_fee_rate,
            input_amount,
            current_vaild_tick_array_start_index,
            sqrt_price_limit_x64.unwrap_or(0),
            pool_state,
            tickarray_bitmap_extension,
            tick_arrays,
        )?;

        info!("  计算出的tick_array索引: {:?}", tick_array_start_index_vec);
        info!("  计算出的金额: {}", amount_calculated);

        Ok((amount_calculated, tick_array_start_index_vec))
    }

    /// 精确移植CLI的swap_compute函数逻辑
    fn swap_compute_cli_exact(
        &self,
        zero_for_one: bool,
        is_base_input: bool,
        is_pool_current_tick_array: bool,
        fee: u32,
        amount_specified: u64,
        current_vaild_tick_array_start_index: i32,
        sqrt_price_limit_x64: u128,
        pool_state: &raydium_amm_v3::states::PoolState,
        tickarray_bitmap_extension: &raydium_amm_v3::states::TickArrayBitmapExtension,
        tick_arrays: &mut VecDeque<raydium_amm_v3::states::TickArrayState>,
    ) -> Result<(u64, VecDeque<i32>)> {
        use raydium_amm_v3::libraries::{liquidity_math, swap_math, tick_math};
        use std::ops::Neg;

        if amount_specified == 0 {
            return Err(anyhow::anyhow!("amountSpecified must not be 0"));
        }

        // 价格限制处理
        let sqrt_price_limit_x64 = if sqrt_price_limit_x64 == 0 {
            if zero_for_one {
                tick_math::MIN_SQRT_PRICE_X64 + 1
            } else {
                tick_math::MAX_SQRT_PRICE_X64 - 1
            }
        } else {
            sqrt_price_limit_x64
        };

        // 价格限制验证
        if zero_for_one {
            if sqrt_price_limit_x64 < tick_math::MIN_SQRT_PRICE_X64 {
                return Err(anyhow::anyhow!("sqrt_price_limit_x64 must greater than MIN_SQRT_PRICE_X64"));
            }
            if sqrt_price_limit_x64 >= pool_state.sqrt_price_x64 {
                return Err(anyhow::anyhow!("sqrt_price_limit_x64 must smaller than current"));
            }
        } else {
            if sqrt_price_limit_x64 > tick_math::MAX_SQRT_PRICE_X64 {
                return Err(anyhow::anyhow!("sqrt_price_limit_x64 must smaller than MAX_SQRT_PRICE_X64"));
            }
            if sqrt_price_limit_x64 <= pool_state.sqrt_price_x64 {
                return Err(anyhow::anyhow!("sqrt_price_limit_x64 must greater than current"));
            }
        }

        // 交换状态结构体
        #[derive(Debug)]
        struct SwapState {
            amount_specified_remaining: u64,
            amount_calculated: u64,
            sqrt_price_x64: u128,
            tick: i32,
            liquidity: u128,
        }

        // 步骤计算结构体
        #[derive(Default)]
        struct StepComputations {
            sqrt_price_start_x64: u128,
            tick_next: i32,
            initialized: bool,
            sqrt_price_next_x64: u128,
            amount_in: u64,
            amount_out: u64,
            fee_amount: u64,
        }

        // 初始化交换状态
        let mut tick_match_current_tick_array = is_pool_current_tick_array;
        let mut state = SwapState {
            amount_specified_remaining: amount_specified,
            amount_calculated: 0,
            sqrt_price_x64: pool_state.sqrt_price_x64,
            tick: pool_state.tick_current,
            liquidity: pool_state.liquidity,
        };

        // 获取当前tick array
        let mut tick_array_current = tick_arrays.pop_front().ok_or_else(|| anyhow::anyhow!("没有可用的tick array"))?;
        if tick_array_current.start_tick_index != current_vaild_tick_array_start_index {
            return Err(anyhow::anyhow!("tick array start tick index does not match"));
        }
        let mut tick_array_start_index_vec = VecDeque::new();
        tick_array_start_index_vec.push_back(tick_array_current.start_tick_index);

        let mut loop_count = 0;

        // 主交换循环
        while state.amount_specified_remaining != 0 
            && state.sqrt_price_x64 != sqrt_price_limit_x64 
            && state.tick < tick_math::MAX_TICK 
            && state.tick > tick_math::MIN_TICK 
        {
            if loop_count > 10 {
                return Err(anyhow::anyhow!("loop_count limit"));
            }

            let mut step = StepComputations::default();
            step.sqrt_price_start_x64 = state.sqrt_price_x64;

            // 查找下一个初始化tick
            let mut next_initialized_tick = if let Some(tick_state) = tick_array_current
                .next_initialized_tick(state.tick, pool_state.tick_spacing, zero_for_one)
                .map_err(|e| anyhow::anyhow!("next_initialized_tick failed: {:?}", e))?
            {
                Box::new(*tick_state)
            } else {
                if !tick_match_current_tick_array {
                    tick_match_current_tick_array = true;
                    Box::new(*tick_array_current.first_initialized_tick(zero_for_one)
                        .map_err(|e| anyhow::anyhow!("first_initialized_tick failed: {:?}", e))?)
                } else {
                    Box::new(raydium_amm_v3::states::TickState::default())
                }
            };

            // 如果当前tick array没有更多初始化tick，切换到下一个
            if !next_initialized_tick.is_initialized() {
                let current_vaild_tick_array_start_index = pool_state
                    .next_initialized_tick_array_start_index(
                        &Some(*tickarray_bitmap_extension),
                        current_vaild_tick_array_start_index,
                        zero_for_one,
                    )
                    .map_err(|e| anyhow::anyhow!("next_initialized_tick_array_start_index failed: {:?}", e))?;

                if current_vaild_tick_array_start_index.is_none() {
                    return Err(anyhow::anyhow!("tick array start tick index out of range limit"));
                }

                tick_array_current = tick_arrays.pop_front().ok_or_else(|| anyhow::anyhow!("没有更多tick arrays"))?;
                let expected_index = current_vaild_tick_array_start_index.unwrap();
                if tick_array_current.start_tick_index != expected_index {
                    return Err(anyhow::anyhow!("tick array start tick index does not match"));
                }
                tick_array_start_index_vec.push_back(tick_array_current.start_tick_index);

                let first_initialized_tick = tick_array_current.first_initialized_tick(zero_for_one)
                    .map_err(|e| anyhow::anyhow!("first_initialized_tick failed: {:?}", e))?;

                next_initialized_tick = Box::new(*first_initialized_tick);
            }

            // 设置下一个tick和价格
            step.tick_next = next_initialized_tick.tick;
            step.initialized = next_initialized_tick.is_initialized();
            if step.tick_next < tick_math::MIN_TICK {
                step.tick_next = tick_math::MIN_TICK;
            } else if step.tick_next > tick_math::MAX_TICK {
                step.tick_next = tick_math::MAX_TICK;
            }

            step.sqrt_price_next_x64 = tick_math::get_sqrt_price_at_tick(step.tick_next)
                .map_err(|e| anyhow::anyhow!("get_sqrt_price_at_tick failed: {:?}", e))?;

            let target_price = if (zero_for_one && step.sqrt_price_next_x64 < sqrt_price_limit_x64) 
                || (!zero_for_one && step.sqrt_price_next_x64 > sqrt_price_limit_x64) {
                sqrt_price_limit_x64
            } else {
                step.sqrt_price_next_x64
            };

            // 计算交换步骤
            let swap_step = swap_math::compute_swap_step(
                state.sqrt_price_x64,
                target_price,
                state.liquidity,
                state.amount_specified_remaining,
                fee,
                is_base_input,
                zero_for_one,
                1,
            ).map_err(|e| anyhow::anyhow!("compute_swap_step failed: {:?}", e))?;

            state.sqrt_price_x64 = swap_step.sqrt_price_next_x64;
            step.amount_in = swap_step.amount_in;
            step.amount_out = swap_step.amount_out;
            step.fee_amount = swap_step.fee_amount;

            // 更新状态
            if is_base_input {
                state.amount_specified_remaining = state.amount_specified_remaining
                    .checked_sub(step.amount_in + step.fee_amount)
                    .unwrap();
                state.amount_calculated = state.amount_calculated
                    .checked_add(step.amount_out)
                    .unwrap();
            } else {
                state.amount_specified_remaining = state.amount_specified_remaining
                    .checked_sub(step.amount_out)
                    .unwrap();
                state.amount_calculated = state.amount_calculated
                    .checked_add(step.amount_in + step.fee_amount)
                    .unwrap();
            }

            // 处理tick转换
            if state.sqrt_price_x64 == step.sqrt_price_next_x64 {
                if step.initialized {
                    let mut liquidity_net = next_initialized_tick.liquidity_net;
                    if zero_for_one {
                        liquidity_net = liquidity_net.neg();
                    }
                    state.liquidity = liquidity_math::add_delta(state.liquidity, liquidity_net)
                        .map_err(|e| anyhow::anyhow!("add_delta failed: {:?}", e))?;
                }

                state.tick = if zero_for_one { 
                    step.tick_next - 1 
                } else { 
                    step.tick_next 
                };
            } else if state.sqrt_price_x64 != step.sqrt_price_start_x64 {
                state.tick = tick_math::get_tick_at_sqrt_price(state.sqrt_price_x64)
                    .map_err(|e| anyhow::anyhow!("get_tick_at_sqrt_price failed: {:?}", e))?;
            }

            loop_count += 1;
        }

        Ok((state.amount_calculated, tick_array_start_index_vec))
    }
}