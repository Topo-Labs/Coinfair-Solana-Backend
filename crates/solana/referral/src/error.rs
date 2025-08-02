use anchor_lang::prelude::*;

#[error_code]
pub enum ReferralError {
    #[msg("Already claimed upper")]
    AlreadyClaimed,

    #[msg("Cannot set self as upper")]
    CannotSelfSetUpper,

    #[msg("Invalid NFT for claiming")]
    InvalidNFT,

    #[msg("Cannot refer to self")]
    CannotReferSelf,

    #[msg("Already has a parent")]
    AlreadyHasParent,

    #[msg("Invalid mint amount")]
    InvalidMintAmount,

    #[msg("Invalid claim fee.")]
    InvalidClaimFee,

    #[msg("Invalid referral code: NFTNotFromUpper")]
    NFTNotFromUpper,
    //
    #[msg("Not approved")]
    NotApproved,

    #[msg("NFT is not enough")]
    NoRemainingMint,
}
