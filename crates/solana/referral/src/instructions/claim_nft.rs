use crate::error::ReferralError;
use crate::instructions::mint_nft::MintCounter;
use crate::states::{ReferralAccount, ReferralConfig};
use anchor_lang::prelude::*;
use anchor_lang::system_program;
use anchor_spl::token::{self, Mint, Token, TokenAccount};

#[derive(Accounts)]
pub struct ClaimReferralNFT<'info> {
    /// 下级用户，执行领取操作
    #[account(mut)]
    pub user: Signer<'info>,

    /// 上级地址，由客户端传入
    /// 必须是一个合法钱包地址
    /// 校验不能是 user 自己
    pub upper: SystemAccount<'info>,

    /// 记录当前用户的推荐关系
    #[account(
        init_if_needed,
        payer = user,
        space = 8 + std::mem::size_of::<ReferralAccount>(),
        seeds = [b"referral", user.key().as_ref()],
        bump
    )]
    pub user_referral: Account<'info, ReferralAccount>,

    /// 上级 NFT mint 次数记录
    #[account(
        mut,
        // init_if_needed,
        // payer = user,
        // space = 8 + std::mem::size_of::<MintCounter>(),
        seeds = [b"mint_counter", upper.key().as_ref()],
        bump
    )]
    pub upper_mint_counter: Account<'info, MintCounter>,
    /// 读取上级的推荐信息
    #[account(
        seeds = [b"referral", upper.key().as_ref()],
        bump
    )]
    pub upper_referral: Account<'info, ReferralAccount>,

    // /// 上级持有的 NFT TokenAccount
    // #[account(
    //     mut,
    //     constraint = upper_nft_account.mint == official_mint.key(),
    //     constraint = upper_nft_account.owner == upper.key(),
    //     constraint = upper_nft_account.amount >= 1,
    // )]
    // pub upper_nft_account: Account<'info, TokenAccount>,
    /// 全局配置，包含官方NFT mint地址、手续费等信息
    #[account(
        seeds = [b"config"],
        bump = config.bump,
    )]
    pub config: Account<'info, ReferralConfig>,

    /// 官方NFT Mint账户
    #[account(
        mut,
        address = config.nft_mint, // 确保官方NFT的mint地址
    )]
    pub official_mint: Account<'info, Mint>,

    /// 用户的ATA账户，接收NFT
    #[account(
        init_if_needed,
        payer = user,
        associated_token::mint = official_mint,
        associated_token::authority = user,
    )]
    pub user_ata: Account<'info, TokenAccount>,

    // /// 用户支付手续费的账户(这里是支付 SPL Token)(废弃)
    // #[account(
    //     mut,
    //     constraint = user_token_account.amount >= config.claim_fee, // 用户账户余额必须足够支付手续费
    // )]
    // pub user_token_account: Account<'info, TokenAccount>,
    /// 支付手续费的目标账户，协议方钱包
    #[account(
        mut,
        address = config.protocol_wallet, // 协议接收钱包
    )]
    pub protocol_wallet: SystemAccount<'info>,

    /// PDA 签名者，用于托管上级 NFT 并进行分发
    /// CHECK: This account is used as upper bound for some calculation
    #[account(
        seeds = [b"nft_pool", upper.key().as_ref()],
        bump,
    )]
    pub nft_pool_authority: UncheckedAccount<'info>,

    /// PDA 所持有的 TokenAccount（上级 mint 的 NFT 暂存在此）
    #[account(
        mut,
        associated_token::mint = official_mint,
        associated_token::authority = nft_pool_authority,
    )]
    pub nft_pool_account: Account<'info, TokenAccount>,

    /// CPI相关的Token Program
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub associated_token_program: Program<'info, anchor_spl::associated_token::AssociatedToken>,
    pub rent: Sysvar<'info, Rent>,
}

#[event]
pub struct ClaimNFTEvent {
    pub claimer: Pubkey,          // 领取者地址
    pub upper: Pubkey,            // 上级地址
    pub nft_mint: Pubkey,         // NFT mint 地址
    pub claim_fee: u64,           // 支付的领取费用
    pub upper_remain_mint: u64,   // 上级剩余可被领取的NFT数量
    pub protocol_wallet: Pubkey,  // 协议费用接收钱包
    pub nft_pool_account: Pubkey, // NFT池子账户
    pub user_ata: Pubkey,         // 用户接收NFT的ATA账户
    pub timestamp: i64,           // 领取时间戳
}

#[event]
pub struct ReferralEstablishedEvent {
    pub user: Pubkey,     // 下级用户
    pub upper: Pubkey,    // 上级用户
    pub nft_mint: Pubkey, // 相关NFT mint地址
    pub timestamp: i64,   // 建立关系时间戳
}

pub fn claim_nft(ctx: Context<ClaimReferralNFT>) -> Result<()> {
    msg!("🔍 Start: claim_nft");

    let user = &ctx.accounts.user;
    let config = &ctx.accounts.config;
    let user_referral = &mut ctx.accounts.user_referral;

    msg!("User: {}", user.key());
    msg!("Upper: {}", ctx.accounts.upper.key());

    // // 1. 确保用户未领取过
    // let user_ata = &ctx.accounts.user_ata;
    // if user_ata.amount > 0 {
    //     return Err(ReferralError::AlreadyClaimed.into());
    // }
    // 2. 校验是否已绑定过上级
    if user_referral.upper.is_some() {
        return Err(ReferralError::AlreadyHasParent.into());
    }
    // 3. 校验不能绑定自己
    if user.key() == ctx.accounts.upper.key() {
        return Err(ReferralError::CannotReferSelf.into());
    }

    msg!("✅ Referral check passed");

    // --------- 设置推荐关系 ----------

    user_referral.upper = Some(ctx.accounts.upper.key());
    // user_referral.upper_upper = ctx.accounts.upper_referral.upper;

    msg!("Set upper done");

    // 4. 更新上级的 mint_counter
    let counter = &mut ctx.accounts.upper_mint_counter;
    msg!("Upper remain_mint: {}", counter.remain_mint);

    if counter.remain_mint == 0 {
        msg!("❌ No remaining mint");
        return Err(ReferralError::NoRemainingMint.into());
    }
    counter.remain_mint -= 1;
    msg!("Decremented remain_mint");

    // 2. 扣除手续费
    // token::transfer(
    //     CpiContext::new(
    //         ctx.accounts.token_program.to_account_info(),
    //         token::Transfer {
    //             from: ctx.accounts.user_token_account.to_account_info(),
    //             to: protocol_wallet.to_account_info(),
    //             authority: user.to_account_info(),
    //         },
    //     ),
    //     config.claim_fee, // 扣除配置数量的 SOL（lamports）
    // )?;

    // 2. 扣除Claim费用
    system_program::transfer(
        CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            system_program::Transfer {
                from: ctx.accounts.user.to_account_info(),
                to: ctx.accounts.protocol_wallet.to_account_info(),
            },
        ),
        config.claim_fee,
    )?;
    msg!("✅ Claim fee transferred");

    // 5. 将NFT从上级账户转移给下级
    // token::transfer(
    //     CpiContext::new(
    //         ctx.accounts.token_program.to_account_info(),
    //         token::Transfer {
    //             from: ctx.accounts.upper_nft_account.to_account_info(),
    //             to: ctx.accounts.user_ata.to_account_info(),
    //             authority: ctx.accounts.upper.to_account_info(),
    //         },
    //     ),
    //     1, // 仅转移1个 NFT
    // )?;

    let binding = ctx.accounts.upper.key();
    let transfer_authority_seeds = &[b"nft_pool", binding.as_ref(), &[ctx.bumps.nft_pool_authority]];
    token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            token::Transfer {
                from: ctx.accounts.nft_pool_account.to_account_info(),
                to: ctx.accounts.user_ata.to_account_info(),
                authority: ctx.accounts.nft_pool_authority.to_account_info(), // PDA 发起转账
            },
            &[transfer_authority_seeds],
        ),
        1,
    )?;
    msg!("✅ NFT transferred to user");

    emit!(ClaimNFTEvent {
        claimer: user.key(),
        upper: ctx.accounts.upper.key(),
        nft_mint: ctx.accounts.official_mint.key(),
        claim_fee: config.claim_fee,
        upper_remain_mint: counter.remain_mint,
        protocol_wallet: ctx.accounts.protocol_wallet.key(),
        nft_pool_account: ctx.accounts.nft_pool_account.key(),
        user_ata: ctx.accounts.user_ata.key(),
        timestamp: Clock::get()?.unix_timestamp,
    });

    emit!(ReferralEstablishedEvent {
        user: user.key(),
        upper: ctx.accounts.upper.key(),
        nft_mint: ctx.accounts.official_mint.key(),
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}
