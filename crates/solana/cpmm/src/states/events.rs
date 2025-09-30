use anchor_lang::prelude::*;

/// 初始化池子时发出
#[event]
#[cfg_attr(feature = "client", derive(Debug))]
pub struct InitPoolEvent {
    pub pool_id: Pubkey,
    pub pool_creator: Pubkey,
    pub token_0_mint: Pubkey,
    pub token_1_mint: Pubkey,
    pub token_0_vault: Pubkey,
    pub token_1_vault: Pubkey,
    pub lp_program_id: Pubkey,
    pub lp_mint: Pubkey,
    pub decimals: u8,
}

/// 在存入和提取时发出
#[event]
#[cfg_attr(feature = "client", derive(Debug))]
pub struct LpChangeEvent {
    pub pool_id: Pubkey,
    pub lp_amount_before: u64,
    /// 池金库减去交易费用
    pub token_0_vault_before: u64,
    /// 池金库减去交易费用
    pub token_1_vault_before: u64,
    /// 不包含转账费用的计算结果
    pub token_0_amount: u64,
    /// 不包含转账费用的计算结果
    pub token_1_amount: u64,
    pub token_0_transfer_fee: u64,
    pub token_1_transfer_fee: u64,
    // 0: 存入，1: 提取
    pub change_type: u8,
}

/// 在交换时发出
#[event]
#[cfg_attr(feature = "client", derive(Debug))]
pub struct SwapEvent {
    pub pool_id: Pubkey,
    /// 池金库减去交易费用
    pub input_vault_before: u64,
    /// 池金库减去交易费用
    pub output_vault_before: u64,
    /// 不包含转账费用的计算结果
    pub input_amount: u64,
    /// 不包含转账费用的计算结果
    pub output_amount: u64,
    pub input_transfer_fee: u64,
    pub output_transfer_fee: u64,
    pub base_input: bool,
    pub input_mint: Pubkey,
    pub output_mint: Pubkey,
    pub trade_fee: u64,
    /// 给创建者的费用代币数量
    pub creator_fee: u64,
    pub creator_fee_on_input: bool,
}
