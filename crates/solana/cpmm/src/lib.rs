pub mod curve;
pub mod error;
pub mod instructions;
pub mod libraries;
pub mod states;
pub mod utils;

// Re-export core library for big_num.rs macro
use crate::curve::fees::FEE_RATE_DENOMINATOR_VALUE;
use anchor_lang::prelude::*;
pub use core as core_;
use instructions::*;
pub use states::CreatorFeeOn;

#[cfg(not(feature = "no-entrypoint"))]
solana_security_txt::security_txt! {
    name: "coinfair",
    project_url: "https://sol.coinfair.xyz",
    contacts: "",
    policy: "",
    source_code: "https://github.com/Topo-Labs/Coinfair-Solana-AMM",
    preferred_languages: "en",
    auditors: ""
}

declare_id!("FairxoKThzWcDy9avKPsADqzni18LrXxKAZEHdXVo5gi");

pub mod admin {
    use super::{pubkey, Pubkey};
    pub const ID: Pubkey = pubkey!("AdmnrQJtt4vRN969ayudxfNDqiNa2AAQ1ErnUPTMYRgJ");
}

/// Coinfair池子创建所需费用（防止恶意创建大量无用池子）
pub mod create_pool_fee_reveiver {
    use super::{pubkey, Pubkey};
    /// 部署钱包地址对WSOL的ATA
    pub const ID: Pubkey = pubkey!("3gXnxLQj6Zs1WNNAdafAbGamfMyZwS62SSesEVF65rBj");
}

pub const AUTH_SEED: &str = "vault_and_lp_mint_auth_seed";

#[program]
pub mod raydium_cp_swap {
    use super::*;

    // AMM协议的配置，包括交易手续费和协议费
    /// # 参数
    ///
    /// * `ctx`- 指令所需的账户。
    /// * `index` - AMM配置的索引，可能有多个配置。
    /// * `trade_fee_rate` - 交易费率，可以更改。
    /// * `protocol_fee_rate` - 交易费中协议费的比率。
    /// * `fund_fee_rate` - 交易费中资金费的比率。
    ///
    pub fn create_amm_config(
        ctx: Context<CreateAmmConfig>,
        index: u16,
        trade_fee_rate: u64,
        protocol_fee_rate: u64,
        fund_fee_rate: u64,
        create_pool_fee: u64,
        creator_fee_rate: u64,
    ) -> Result<()> {
        assert!(trade_fee_rate + creator_fee_rate < FEE_RATE_DENOMINATOR_VALUE);
        assert!(protocol_fee_rate <= FEE_RATE_DENOMINATOR_VALUE);
        assert!(fund_fee_rate <= FEE_RATE_DENOMINATOR_VALUE);
        assert!(fund_fee_rate + protocol_fee_rate <= FEE_RATE_DENOMINATOR_VALUE);
        instructions::create_amm_config(
            ctx,
            index,
            trade_fee_rate,
            protocol_fee_rate,
            fund_fee_rate,
            create_pool_fee,
            creator_fee_rate,
        )
    }

    /// 更新AMM配置的所有者
    /// 必须由当前所有者或管理员调用
    ///
    /// # 参数
    ///
    /// * `ctx`- 账户的上下文
    /// * `trade_fee_rate`- AMM配置的新交易费率，当`param`为0时设置
    /// * `protocol_fee_rate`- AMM配置的新协议费率，当`param`为1时设置
    /// * `fund_fee_rate`- AMM配置的新资金费率，当`param`为2时设置
    /// * `new_owner`- 配置的新所有者，当`param`为3时设置
    /// * `new_fund_owner`- 配置的新资金所有者，当`param`为4时设置
    /// * `param`- 值可以是 0 | 1 | 2 | 3 | 4，否则会报错
    ///
    pub fn update_amm_config(ctx: Context<UpdateAmmConfig>, param: u8, value: u64) -> Result<()> {
        instructions::update_amm_config(ctx, param, value)
    }

    /// 为给定值更新池状态
    ///
    /// # 参数
    ///
    /// * `ctx`- 账户的上下文
    /// * `status` - 状态的值
    ///
    pub fn update_pool_status(ctx: Context<UpdatePoolStatus>, status: u8) -> Result<()> {
        instructions::update_pool_status(ctx, status)
    }

    /// 收取池中累积的协议费
    ///
    /// # 参数
    ///
    /// * `ctx` - 账户的上下文
    /// * `amount_0_requested` - 要发送的token_0的最大数量，可以为0以仅收取token_1的费用
    /// * `amount_1_requested` - 要发送的token_1的最大数量，可以为0以仅收取token_0的费用
    ///
    pub fn collect_protocol_fee(
        ctx: Context<CollectProtocolFee>,
        amount_0_requested: u64,
        amount_1_requested: u64,
    ) -> Result<()> {
        instructions::collect_protocol_fee(ctx, amount_0_requested, amount_1_requested)
    }

    /// 收取池中累积的资金费
    ///
    /// # 参数
    ///
    /// * `ctx` - 账户的上下文
    /// * `amount_0_requested` - 要发送的token_0的最大数量，可以为0以仅收取token_1的费用
    /// * `amount_1_requested` - 要发送的token_1的最大数量，可以为0以仅收取token_0的费用
    ///
    pub fn collect_fund_fee(
        ctx: Context<CollectFundFee>,
        amount_0_requested: u64,
        amount_1_requested: u64,
    ) -> Result<()> {
        instructions::collect_fund_fee(ctx, amount_0_requested, amount_1_requested)
    }

    /// 收取创建者费用
    ///
    /// # 参数
    ///
    /// * `ctx` - 账户的上下文
    ///
    pub fn collect_creator_fee(ctx: Context<CollectCreatorFee>) -> Result<()> {
        instructions::collect_creator_fee(ctx)
    }

    /// 创建权限账户
    ///
    /// # 参数
    ///
    /// * `ctx`- 账户的上下文
    ///
    pub fn create_permission_pda(ctx: Context<CreatePermissionPda>) -> Result<()> {
        instructions::create_permission_pda(ctx)
    }

    /// 关闭权限账户
    ///
    /// # 参数
    ///
    /// * `ctx`- 账户的上下文
    ///
    pub fn close_permission_pda(ctx: Context<ClosePermissionPda>) -> Result<()> {
        instructions::close_permission_pda(ctx)
    }

    /// 为给定的代币对和初始价格创建池
    ///
    /// # 参数
    ///
    /// * `ctx`- 账户的上下文
    /// * `init_amount_0` - 要存入的初始amount_0
    /// * `init_amount_1` - 要存入的初始amount_1
    /// * `open_time` - 允许交换的时间戳
    ///
    pub fn initialize(ctx: Context<Initialize>, init_amount_0: u64, init_amount_1: u64, open_time: u64) -> Result<()> {
        instructions::initialize(ctx, init_amount_0, init_amount_1, open_time)
    }

    /// 创建具有权限的池
    ///
    /// # 参数
    ///
    /// * `ctx`- 账户的上下文
    /// * `init_amount_0` - 要存入的初始amount_0
    /// * `init_amount_1` - 要存入的初始amount_1
    /// * `open_time` - 允许交换的时间戳
    /// * `creator_fee_on` - 创建者费用模式，0：token0和token1都可以（取决于输入），1：仅token0，2：仅token1
    ///
    pub fn initialize_with_permission(
        ctx: Context<InitializeWithPermission>,
        init_amount_0: u64,
        init_amount_1: u64,
        open_time: u64,
        creator_fee_on: CreatorFeeOn,
    ) -> Result<()> {
        instructions::initialize_with_permission(ctx, init_amount_0, init_amount_1, open_time, creator_fee_on)
    }

    /// 向池中存入LP代币
    ///
    /// # 参数
    ///
    /// * `ctx`- 账户的上下文
    /// * `lp_token_amount` - 增加的LP数量
    /// * `maximum_token_0_amount` - 要存入的最大token 0数量，防止过度滑点
    /// * `maximum_token_1_amount` - 要存入的最大token 1数量，防止过度滑点
    ///
    pub fn deposit(
        ctx: Context<Deposit>,
        lp_token_amount: u64,
        maximum_token_0_amount: u64,
        maximum_token_1_amount: u64,
    ) -> Result<()> {
        instructions::deposit(ctx, lp_token_amount, maximum_token_0_amount, maximum_token_1_amount)
    }

    /// 提取LP代币换取token0和token1
    ///
    /// # 参数
    ///
    /// * `ctx`- 账户的上下文
    /// * `lp_token_amount` - 要销毁的池代币数量。用户根据返回的池代币百分比接收token a和b的输出。
    /// * `minimum_token_0_amount` - 要接收的最小token 0数量，防止过度滑点
    /// * `minimum_token_1_amount` - 要接收的最小token 1数量，防止过度滑点
    ///
    pub fn withdraw(
        ctx: Context<Withdraw>,
        lp_token_amount: u64,
        minimum_token_0_amount: u64,
        minimum_token_1_amount: u64,
    ) -> Result<()> {
        instructions::withdraw(ctx, lp_token_amount, minimum_token_0_amount, minimum_token_1_amount)
    }

    /// 基于输入数量在池中交换代币
    ///
    /// # 参数
    ///
    /// * `ctx`- 账户的上下文
    /// * `amount_in` - 要转移的输入数量，输出到目标地址基于汇率
    /// * `minimum_amount_out` - 输出代币的最小数量，防止过度滑点
    ///
    pub fn swap_base_input(ctx: Context<Swap>, amount_in: u64, minimum_amount_out: u64) -> Result<()> {
        instructions::swap_base_input(ctx, amount_in, minimum_amount_out)
    }

    /// 基于输出数量在池中交换代币
    ///
    /// # 参数
    ///
    /// * `ctx`- 账户的上下文
    /// * `max_amount_in` - 输入数量防止过度滑点
    /// * `amount_out` - 输出代币的数量
    ///
    pub fn swap_base_output(ctx: Context<Swap>, max_amount_in: u64, amount_out: u64) -> Result<()> {
        instructions::swap_base_output(ctx, max_amount_in, amount_out)
    }
}
