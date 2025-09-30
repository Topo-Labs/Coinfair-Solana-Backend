use anchor_lang::prelude::*;

pub const AMM_CONFIG_SEED: &str = "amm_config";

/// 保存工厂的当前所有者
#[account]
#[derive(Default, Debug)]
pub struct AmmConfig {
    /// 用于识别PDA的Bump
    pub bump: u8,
    /// 控制是否可以创建新池的状态
    pub disable_create_pool: bool,
    /// 配置索引
    pub index: u16,
    /// 交易费，以百分之bip(10^-6)为单位（用户Swap时收取的交易的输入代币数量，给流动性提供者作为奖励）
    pub trade_fee_rate: u64,
    /// 协议费（交易费中的一定比例，给协议方）
    pub protocol_fee_rate: u64,
    /// 资金费，以百分之bip(10^-6)为单位
    pub fund_fee_rate: u64,
    /// 创建新池的费用（创建池子时一次性收取创建者的）
    pub create_pool_fee: u64,
    /// 协议费所有者的地址
    pub protocol_owner: Pubkey,
    /// 资金费所有者的地址
    pub fund_owner: Pubkey,
    /// 池创建者费用，以百分之bip(10^-6)为单位（该池的每笔交易中收取交易金额的一定比例，给池子的创建者）（目前不补偿，即为0）
    pub creator_fee_rate: u64,
    /// 填充
    pub padding: [u64; 15],
}

impl AmmConfig {
    pub const LEN: usize = 8 + 1 + 1 + 2 + 4 * 8 + 32 * 2 + 8 + 8 * 15;
}
