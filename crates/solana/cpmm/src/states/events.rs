use anchor_lang::prelude::*;

/// Emitted when deposit and withdraw
#[event]
#[cfg_attr(feature = "client", derive(Debug))]
pub struct LpChangeEvent {
    /// user wallet address
    pub user_wallet: Pubkey,
    /// pool id
    pub pool_id: Pubkey,
    /// lp mint address
    pub lp_mint: Pubkey,
    /// token_0 mint address
    pub token_0_mint: Pubkey,
    /// token_1 mint address
    pub token_1_mint: Pubkey,
    /// lp amount before
    pub lp_amount_before: u64,
    /// pool vault sub trade fees
    pub token_0_vault_before: u64,
    /// pool vault sub trade fees
    pub token_1_vault_before: u64,
    /// calculate result without transfer fee
    pub token_0_amount: u64,
    /// calculate result without transfer fee
    pub token_1_amount: u64,
    pub token_0_transfer_fee: u64,
    pub token_1_transfer_fee: u64,
    // 0: deposit, 1: withdraw, 2: initialize
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

/// Emitted when swap
#[event]
#[cfg_attr(feature = "client", derive(Debug))]
pub struct SwapEvent {
    pub pool_id: Pubkey,
    /// pool vault sub trade fees
    pub input_vault_before: u64,
    /// pool vault sub trade fees
    pub output_vault_before: u64,
    /// calculate result without transfer fee
    pub input_amount: u64,
    /// calculate result without transfer fee
    pub output_amount: u64,
    pub input_transfer_fee: u64,
    pub output_transfer_fee: u64,
    pub base_input: bool,
    pub input_mint: Pubkey,
    pub output_mint: Pubkey,
    pub trade_fee: u64,
    /// Amount of fee tokens going to creator
    pub creator_fee: u64,
    pub creator_fee_on_input: bool,
}
