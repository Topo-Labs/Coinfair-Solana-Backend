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

/// 在存入，提取，创建池子时发出
#[event]
#[cfg_attr(feature = "client", derive(Debug))]
pub struct LpChangeEvent {
    /// 用户钱包地址
    pub user_wallet: Pubkey,
    /// 池子地址
    pub pool_id: Pubkey,
    /// Lp Mint 地址
    pub lp_mint: Pubkey,
    /// Token_0 Mint地址
    pub token_0_mint: Pubkey,
    /// Token_1 Mint 地址
    pub token_1_mint: Pubkey,
    /// 改变前lp的数量
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
    // 0: 存入，1: 提取， 2: 池子初始化
    pub change_type: u8,
    /// program id of lp mint
    pub lp_mint_program_id: Pubkey,
    /// token_0 program id
    pub token_0_program_id: Pubkey,
    /// token_1 program id
    pub token_1_program_id: Pubkey,
    /// decimals of lp mint
    pub lp_mint_decimals: u8,
    /// token_0 decimals
    pub token_0_decimals: u8,
    /// token_1 decimals
    pub token_1_decimals: u8,
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
