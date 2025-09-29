use anchor_lang::prelude::*;

pub const PERMISSION_SEED: &str = "permission";

/// 保存工厂的当前所有者
#[account]
#[derive(Default, Debug)]
pub struct Permission {
    /// 权限
    pub authority: Pubkey,
    /// 填充
    pub padding: [u64; 30],
}

impl Permission {
    pub const LEN: usize = 8 + 32 + 8 * 30;
}
