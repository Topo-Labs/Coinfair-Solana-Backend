use crate::instructions::mint_nft::MintCounter;
use anchor_lang::prelude::*;
use anchor_lang::AccountDeserialize;

#[derive(Accounts)]
pub struct GetMintCounter<'info> {
    /// CHECK: 用户地址，客户端传过来的
    pub user: UncheckedAccount<'info>,

    /// CHECK: 可能尚未初始化的 mint_counter
    #[account(
        seeds = [b"mint_counter", user.key().as_ref()],
        bump,
    )]
    pub mint_counter: AccountInfo<'info>,
}

pub fn get_mint_counter(ctx: Context<GetMintCounter>) -> Result<(u64, u64)> {
    let info = &ctx.accounts.mint_counter;

    if info.data_is_empty() {
        // 说明 mint_counter 还未初始化
        Ok((0, 0))
    } else {
        // 手动解包，不使用 Account<T> 包装器，避免生命周期冲突
        let mut data: &[u8] = &info.data.borrow();
        let counter = MintCounter::try_deserialize(&mut data)?;
        // // 手动反序列化成 MintCounter 类型
        // let counter: Account<MintCounter> = Account::try_from(info)?;
        Ok((counter.total_mint, counter.remain_mint))
    }
}

// #[derive(Accounts)]
// pub struct GetMintCounter<'info> {
//     /// CHECK: 客户端传来的用户地址
//     pub user: UncheckedAccount<'info>,

//     /// 用户 mint 次数信息，可能尚未初始化
//     #[account(
//         seeds = [b"mint_counter", user.key().as_ref()],
//         bump,
//         optional
//     )]
//     pub mint_counter: Option<Account<'info, MintCounter>>,
// }

// pub fn get_mint_counter(ctx: Context<GetMintCounter>) -> Result<(u64, u64)> {
//     let maybe_counter = &ctx.accounts.mint_counter;

//     let (total, remain) = if let Some(counter) = maybe_counter {
//         (counter.total_mint, counter.remain_mint)
//     } else {
//         (0, 0)
//     };

//     Ok((total, remain))
// }
