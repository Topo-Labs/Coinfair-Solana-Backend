use crate::error::ReferralError;
use crate::states::{ReferralAccount, ReferralConfig};
use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, MintTo, Token, TokenAccount};

#[account]
pub struct MintCounter {
    pub minter: Pubkey,   // 用户地址
    pub total_mint: u64,  // 总 mint 数量
    pub remain_mint: u64, // 剩余可被 claim 的数量
    pub bump: u8,         // PDA bump
}

#[derive(Accounts)]
pub struct MintReferralNFT<'info> {
    /// 铸造人
    #[account(mut)]
    pub authority: Signer<'info>,

    /// 读取全局配置
    #[account(
        seeds = [b"config"],
        bump = config.bump,
    )]
    pub config: Account<'info, ReferralConfig>,

    /// 记录当前用户的推荐关系
    #[account(
        init_if_needed,
        payer = authority,
        space = 8 + std::mem::size_of::<ReferralAccount>(),
        seeds = [b"referral", authority.key().as_ref()],
        bump
    )]
    pub user_referral: Account<'info, ReferralAccount>,

    /// 官方NFT Mint
    #[account(
        mut,
        address = config.nft_mint, // 确保用的是正确的mint
    )]
    pub official_mint: Account<'info, Mint>,

    /// 用户ATA账户（如果不存在就自动创建）
    #[account(
        init_if_needed,
        payer = authority,
        associated_token::mint = official_mint,
        associated_token::authority = authority,
    )]
    pub user_ata: Account<'info, TokenAccount>,

    /// 记录用户mint数量和剩余claim数
    #[account(
        init_if_needed,
        payer = authority,
        space = 8 + std::mem::size_of::<MintCounter>(),
        seeds = [b"mint_counter", authority.key().as_ref()],
        bump
    )]
    pub mint_counter: Account<'info, MintCounter>,

    /// CHECK: PDA signer only, never mutated
    #[account(
        seeds = [b"mint_authority"],
        bump,
    )]
    pub mint_authority: UncheckedAccount<'info>,

    /// CHECK: PDA-only，NFT 暂存在此账户中
    #[account(
        seeds = [b"nft_pool", authority.key().as_ref()],
        bump,
    )]
    pub nft_pool_authority: UncheckedAccount<'info>,

    /// 存放 NFT 的 ATA，属于 PDA 拥有者（将NFT给到池子，统一保管）
    #[account(
        init_if_needed,
        payer = authority,
        associated_token::mint = official_mint,
        associated_token::authority = nft_pool_authority,
    )]
    pub nft_pool_account: Account<'info, TokenAccount>,

    /// 基础依赖
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub associated_token_program: Program<'info, anchor_spl::associated_token::AssociatedToken>,
    pub rent: Sysvar<'info, Rent>,
}

#[event]
pub struct MintNFTEvent {
    pub minter: Pubkey,           // 铸造者地址
    pub nft_mint: Pubkey,         // NFT mint 地址
    pub amount: u64,              // 铸造数量
    pub total_mint: u64,          // 用户累计铸造总数
    pub remain_mint: u64,         // 剩余可claim数量
    pub nft_pool_account: Pubkey, // NFT存放的池子账户
    pub timestamp: i64,           // 铸造时间戳
}

pub fn mint_nft(ctx: Context<MintReferralNFT>, amount: u64) -> Result<()> {
    require!(amount > 0, ReferralError::InvalidMintAmount); // 防止乱mint 0个

    // 初始化 user_referral 内容（如果是新账户）
    let referral = &mut ctx.accounts.user_referral;
    if referral.upper.is_none() && referral.upper_upper.is_none() {
        referral.upper = None;
        referral.upper_upper = None;
        referral.nft_mint = ctx.accounts.official_mint.key(); // 绑定用的 NFT mint 地址
        referral.bump = ctx.bumps.user_referral; // 从 PDA bump 中获取
    }

    // 更新用户 mint 计数器
    let counter = &mut ctx.accounts.mint_counter;
    counter.minter = ctx.accounts.authority.key(); // 初始化时赋值
    counter.total_mint = counter.total_mint.saturating_add(amount);
    counter.remain_mint = counter.remain_mint.saturating_add(amount);
    counter.bump = ctx.bumps.mint_counter;

    let seeds = &[b"mint_authority" as &[u8], &[ctx.bumps.mint_authority]];

    token::mint_to(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            MintTo {
                mint: ctx.accounts.official_mint.to_account_info(),
                to: ctx.accounts.nft_pool_account.to_account_info(), // mint 到 PDA 中转账户
                authority: ctx.accounts.mint_authority.to_account_info(), // PDA 签名
            },
            &[seeds],
        ),
        amount,
    )?;

    emit!(MintNFTEvent {
        minter: ctx.accounts.authority.key(),
        nft_mint: ctx.accounts.official_mint.key(),
        amount,
        total_mint: counter.total_mint,
        remain_mint: counter.remain_mint,
        nft_pool_account: ctx.accounts.nft_pool_account.key(),
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}
