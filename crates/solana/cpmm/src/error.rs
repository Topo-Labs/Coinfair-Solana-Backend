/// TokenSwap程序可能返回的错误。
use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("Not approved")]
    NotApproved,
    /// 输入的所有者未设置为程序生成的程序地址。
    #[msg("Input account owner is not the program address")]
    InvalidOwner,
    /// 输入代币账户为空。
    #[msg("Input token account empty")]
    EmptySupply,
    /// 输入代币对于交换无效。
    #[msg("InvalidInput")]
    InvalidInput,
    /// 提供的池代币铸币地址不正确
    #[msg("Address of the provided lp token mint is incorrect")]
    IncorrectLpMint,
    /// 超过期望的滑点限制
    #[msg("Exceeds desired slippage limit")]
    ExceededSlippage,
    /// 给定的池代币数量导致零交易代币
    #[msg("Given pool token amount results in zero trading tokens")]
    ZeroTradingTokens,
    #[msg("Not support token_2022 mint extension")]
    NotSupportMint,
    #[msg("invaild vault")]
    InvalidVault,
    #[msg("Init lp amount is too less(Because 100 amount lp will be locked)")]
    InitLpAmountTooLess,
    #[msg("TransferFee calculate not match")]
    TransferFeeCalculateNotMatch,
    #[msg("Math overflow")]
    MathOverflow,
    #[msg("Insufficient vault")]
    InsufficientVault,
    #[msg("Invalid fee model")]
    InvalidFeeModel,
    #[msg("Fee is zero")]
    NoFeeCollect,

    #[msg("swap_with_referral: upper account mismatch")]
    UpperAccountMismatch,
    #[msg("swap_with_referral: upper token account mismatch")]
    UpperTokenAccountMismatch,
    #[msg("swap_with_referral: upper upper mismatch")]
    UpperUpperMismatch,
    #[msg("swap_with_referral: upper upper token account mismatch")]
    UpperUpperTokenAccountMismatch,
    #[msg("swap_with_referral: project token account mismatch")]
    ProjectTokenAccountMismatch,

    #[msg("Create Pool: Base mint - must be either token_mint_0 or token_mint_1")]
    InvalidBaseMint,
    #[msg("with_hook: incomplete transfer hook accounts")]
    IncompleteTransferHookAccounts,
    #[msg("with_hook: invalid hook mint")]
    InvalidHookMint,
}
