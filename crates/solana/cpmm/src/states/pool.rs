use crate::{curve::TradeDirection, error::ErrorCode};
use anchor_lang::prelude::*;
use anchor_spl::token_interface::Mint;
use std::ops::{BitAnd, BitOr, BitXor};
/// 用于派生账户地址和签名的种子
pub const POOL_SEED: &str = "pool";
pub const POOL_LP_MINT_SEED: &str = "pool_lp_mint";
pub const POOL_VAULT_SEED: &str = "pool_vault";

pub const Q32: u128 = (u32::MAX as u128) + 1; // 2^32

pub enum PoolStatusBitIndex {
    Deposit,
    Withdraw,
    Swap,
}

#[derive(PartialEq, Eq)]
pub enum PoolStatusBitFlag {
    Enable,
    Disable,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq)]
pub enum CreatorFeeOn {
    /// token0和token1都可以用作交易费。
    /// 这取决于输入代币是什么。
    BothToken,
    /// 只有token0可以用作交易费。
    OnlyToken0,
    /// 只有token1可以用作交易费。
    OnlyToken1,
}

impl CreatorFeeOn {
    fn from_u8(value: u8) -> Result<Self> {
        match value {
            0 => Ok(CreatorFeeOn::BothToken),
            1 => Ok(CreatorFeeOn::OnlyToken0),
            2 => Ok(CreatorFeeOn::OnlyToken1),
            _ => Err(ErrorCode::InvalidFeeModel.into()),
        }
    }

    pub fn to_u8(&self) -> u8 {
        match self {
            CreatorFeeOn::BothToken => 0u8,
            CreatorFeeOn::OnlyToken0 => 1u8,
            CreatorFeeOn::OnlyToken1 => 2u8,
        }
    }
}

pub struct SwapParams {
    pub trade_direction: TradeDirection,
    pub total_input_token_amount: u64,
    pub total_output_token_amount: u64,
    pub token_0_price_x64: u128,
    pub token_1_price_x64: u128,
    pub is_creator_fee_on_input: bool,
}

#[account(zero_copy(unsafe))]
#[repr(C, packed)]
#[derive(Default, Debug)]
pub struct PoolState {
    /// 池所属的配置
    pub amm_config: Pubkey,
    /// 池创建者
    pub pool_creator: Pubkey,
    /// 代币A
    pub token_0_vault: Pubkey,
    /// 代币B
    pub token_1_vault: Pubkey,

    /// 当存入A或B代币时会发行池代币。
    /// 池代币可以提取回原始的A或B代币。
    pub lp_mint: Pubkey,
    /// 代币A的铸币信息
    pub token_0_mint: Pubkey,
    /// 代币B的铸币信息
    pub token_1_mint: Pubkey,

    /// token_0程序
    pub token_0_program: Pubkey,
    /// token_1程序
    pub token_1_program: Pubkey,

    /// 存储预言机数据的观察账户
    pub observation_key: Pubkey,

    pub auth_bump: u8,
    /// 池状态的位表示
    /// bit0, 1: 禁用存款(值为1), 0: 正常
    /// bit1, 1: 禁用提取(值为2), 0: 正常
    /// bit2, 1: 禁用交换(值为4), 0: 正常
    pub status: u8,

    pub lp_mint_decimals: u8,
    /// mint0和mint1的小数位数
    pub mint_0_decimals: u8,
    pub mint_1_decimals: u8,

    /// 不包括销毁和锁定的真实流通供应量
    pub lp_supply: u64,
    /// 欠流动性提供者的token_0和token_1数量。
    pub protocol_fees_token_0: u64,
    pub protocol_fees_token_1: u64,

    pub fund_fees_token_0: u64,
    pub fund_fees_token_1: u64,

    /// 池中允许交换的时间戳。
    pub open_time: u64,
    /// 最近的纪元
    pub recent_epoch: u64,

    /// 创建者费用收取模式
    /// 0: token_0和token_1都可以用作交易费。这取决于交换时的输入代币
    /// 1: 仅token_0作为交易费
    /// 2: 仅token_1作为交易费
    pub creator_fee_on: u8,
    pub enable_creator_fee: bool,
    pub padding1: [u8; 6],
    pub creator_fees_token_0: u64,
    pub creator_fees_token_1: u64,
    /// 为未来更新预留的填充
    pub padding: [u64; 28],
}

impl PoolState {
    pub const LEN: usize = 8 + 10 * 32 + 1 * 5 + 8 * 7 + 1 * 2 + 6 * 1 + 2 * 8 + 8 * 28;

    pub fn initialize(
        &mut self,
        auth_bump: u8,
        lp_supply: u64,
        open_time: u64,
        pool_creator: Pubkey,
        amm_config: Pubkey,
        token_0_vault: Pubkey,
        token_1_vault: Pubkey,
        token_0_mint: &InterfaceAccount<Mint>,
        token_1_mint: &InterfaceAccount<Mint>,
        lp_mint: Pubkey,
        lp_mint_decimals: u8,
        observation_key: Pubkey,
        creator_fee_on: CreatorFeeOn,
        enable_creator_fee: bool,
    ) {
        self.amm_config = amm_config.key();
        self.pool_creator = pool_creator.key();
        self.token_0_vault = token_0_vault;
        self.token_1_vault = token_1_vault;
        self.lp_mint = lp_mint.key();
        self.token_0_mint = token_0_mint.key();
        self.token_1_mint = token_1_mint.key();
        self.token_0_program = *token_0_mint.to_account_info().owner;
        self.token_1_program = *token_1_mint.to_account_info().owner;
        self.observation_key = observation_key;
        self.auth_bump = auth_bump;
        self.lp_mint_decimals = lp_mint_decimals;
        self.mint_0_decimals = token_0_mint.decimals;
        self.mint_1_decimals = token_1_mint.decimals;
        self.lp_supply = lp_supply;
        self.protocol_fees_token_0 = 0;
        self.protocol_fees_token_1 = 0;
        self.fund_fees_token_0 = 0;
        self.fund_fees_token_1 = 0;
        self.open_time = open_time;
        self.recent_epoch = Clock::get().unwrap().epoch;
        self.creator_fee_on = creator_fee_on.to_u8();
        self.enable_creator_fee = enable_creator_fee;
        self.padding1 = [0u8; 6];
        self.creator_fees_token_0 = 0;
        self.creator_fees_token_1 = 0;
        self.padding = [0u64; 28];
    }

    pub fn set_status(&mut self, status: u8) {
        self.status = status
    }

    pub fn set_status_by_bit(&mut self, bit: PoolStatusBitIndex, flag: PoolStatusBitFlag) {
        let s = u8::from(1) << (bit as u8);
        if flag == PoolStatusBitFlag::Disable {
            self.status = self.status.bitor(s);
        } else {
            let m = u8::from(255).bitxor(s);
            self.status = self.status.bitand(m);
        }
    }

    /// 按位获取状态，如果是`正常`状态，返回true
    pub fn get_status_by_bit(&self, bit: PoolStatusBitIndex) -> bool {
        let status = u8::from(1) << (bit as u8);
        self.status.bitand(status) == 0
    }

    pub fn vault_amount_without_fee(&self, vault_0: u64, vault_1: u64) -> Result<(u64, u64)> {
        let fees_token_0 = self
            .protocol_fees_token_0
            .checked_add(self.fund_fees_token_0)
            .ok_or(ErrorCode::MathOverflow)?
            .checked_add(self.creator_fees_token_0)
            .ok_or(ErrorCode::MathOverflow)?;
        let fees_token_1 = self
            .protocol_fees_token_1
            .checked_add(self.fund_fees_token_1)
            .ok_or(ErrorCode::MathOverflow)?
            .checked_add(self.creator_fees_token_1)
            .ok_or(ErrorCode::MathOverflow)?;
        Ok((
            vault_0.checked_sub(fees_token_0).ok_or(ErrorCode::InsufficientVault)?,
            vault_1.checked_sub(fees_token_1).ok_or(ErrorCode::InsufficientVault)?,
        ))
    }

    pub fn token_price_x32(&self, vault_0: u64, vault_1: u64) -> Result<(u128, u128)> {
        let (token_0_amount, token_1_amount) = self.vault_amount_without_fee(vault_0, vault_1)?;
        Ok((
            token_1_amount as u128 * Q32 as u128 / token_0_amount as u128,
            token_0_amount as u128 * Q32 as u128 / token_1_amount as u128,
        ))
    }

    pub fn update_lp_supply(&mut self, liquidity_delta: u64, add: bool, recent_epoch: u64) -> Result<()> {
        if add {
            self.lp_supply = self
                .lp_supply
                .checked_add(liquidity_delta)
                .ok_or(ErrorCode::MathOverflow)?;
        } else {
            self.lp_supply = self
                .lp_supply
                .checked_sub(liquidity_delta)
                .ok_or(ErrorCode::MathOverflow)?;
        }
        self.recent_epoch = recent_epoch;
        Ok(())
    }

    // 确定创建者用于计算交易费的方法
    pub fn is_creator_fee_on_input(&self, direction: TradeDirection) -> Result<bool> {
        let fee_on = CreatorFeeOn::from_u8(self.creator_fee_on)?;
        Ok(match (fee_on, direction) {
            (CreatorFeeOn::BothToken, _) => true,
            (CreatorFeeOn::OnlyToken0, TradeDirection::ZeroForOne) => true,
            (CreatorFeeOn::OnlyToken1, TradeDirection::OneForZero) => true,
            _ => false,
        })
    }

    pub fn get_swap_params(
        &self,
        input_vault_key: Pubkey,
        output_vault_key: Pubkey,
        input_vault_amount: u64,
        output_vault_amount: u64,
    ) -> Result<SwapParams> {
        let (
            trade_direction,
            total_input_token_amount,
            total_output_token_amount,
            token_0_price_x64,
            token_1_price_x64,
            is_creator_fee_on_input,
        ) = if input_vault_key == self.token_0_vault && output_vault_key == self.token_1_vault {
            let (total_input_token_amount, total_output_token_amount) =
                self.vault_amount_without_fee(input_vault_amount, output_vault_amount)?;
            let (token_0_price_x64, token_1_price_x64) =
                self.token_price_x32(input_vault_amount, output_vault_amount)?;

            (
                TradeDirection::ZeroForOne,
                total_input_token_amount,
                total_output_token_amount,
                token_0_price_x64,
                token_1_price_x64,
                self.is_creator_fee_on_input(TradeDirection::ZeroForOne)?,
            )
        } else if input_vault_key == self.token_1_vault && output_vault_key == self.token_0_vault {
            let (total_output_token_amount, total_input_token_amount) =
                self.vault_amount_without_fee(output_vault_amount, input_vault_amount)?;
            let (token_0_price_x64, token_1_price_x64) =
                self.token_price_x32(output_vault_amount, input_vault_amount)?;

            (
                TradeDirection::OneForZero,
                total_input_token_amount,
                total_output_token_amount,
                token_0_price_x64,
                token_1_price_x64,
                self.is_creator_fee_on_input(TradeDirection::OneForZero)?,
            )
        } else {
            return err!(ErrorCode::InvalidVault);
        };
        Ok(SwapParams {
            trade_direction,
            total_input_token_amount,
            total_output_token_amount,
            token_0_price_x64,
            token_1_price_x64,
            is_creator_fee_on_input,
        })
    }

    pub fn adjust_creator_fee_rate(&self, creator_fee_rate: u64) -> u64 {
        if self.enable_creator_fee {
            creator_fee_rate
        } else {
            0
        }
    }

    pub fn update_fees(
        &mut self,
        protocol_fee: u64,
        fund_fee: u64,
        creator_fee: u64,
        direction: TradeDirection,
    ) -> Result<()> {
        if !self.enable_creator_fee {
            require_eq!(creator_fee, 0)
        }
        let is_creator_fee_on_input = self.is_creator_fee_on_input(direction)?;
        match direction {
            TradeDirection::ZeroForOne => {
                self.protocol_fees_token_0 = self.protocol_fees_token_0.checked_add(protocol_fee).unwrap();
                self.fund_fees_token_0 = self.fund_fees_token_0.checked_add(fund_fee).unwrap();

                if is_creator_fee_on_input {
                    self.creator_fees_token_0 = self.creator_fees_token_0.checked_add(creator_fee).unwrap();
                } else {
                    self.creator_fees_token_1 = self.creator_fees_token_1.checked_add(creator_fee).unwrap();
                }
            }
            TradeDirection::OneForZero => {
                self.protocol_fees_token_1 = self.protocol_fees_token_1.checked_add(protocol_fee).unwrap();
                self.fund_fees_token_1 = self.fund_fees_token_1.checked_add(fund_fee).unwrap();
                if is_creator_fee_on_input {
                    self.creator_fees_token_1 = self.creator_fees_token_1.checked_add(creator_fee).unwrap();
                } else {
                    self.creator_fees_token_0 = self.creator_fees_token_0.checked_add(creator_fee).unwrap();
                }
            }
        };
        Ok(())
    }
}

#[cfg(test)]
pub mod pool_test {
    use super::*;

    #[test]
    fn pool_state_size_test() {
        assert_eq!(std::mem::size_of::<PoolState>(), PoolState::LEN - 8)
    }

    mod pool_status_test {
        use super::*;

        #[test]
        fn get_set_status_by_bit() {
            let mut pool_state = PoolState::default();
            pool_state.set_status(4); // 0000100
            assert_eq!(pool_state.get_status_by_bit(PoolStatusBitIndex::Swap), false);
            assert_eq!(pool_state.get_status_by_bit(PoolStatusBitIndex::Deposit), true);
            assert_eq!(pool_state.get_status_by_bit(PoolStatusBitIndex::Withdraw), true);

            // disable -> disable, nothing to change
            pool_state.set_status_by_bit(PoolStatusBitIndex::Swap, PoolStatusBitFlag::Disable);
            assert_eq!(pool_state.get_status_by_bit(PoolStatusBitIndex::Swap), false);

            // disable -> enable
            pool_state.set_status_by_bit(PoolStatusBitIndex::Swap, PoolStatusBitFlag::Enable);
            assert_eq!(pool_state.get_status_by_bit(PoolStatusBitIndex::Swap), true);

            // enable -> enable, nothing to change
            pool_state.set_status_by_bit(PoolStatusBitIndex::Swap, PoolStatusBitFlag::Enable);
            assert_eq!(pool_state.get_status_by_bit(PoolStatusBitIndex::Swap), true);
            // enable -> disable
            pool_state.set_status_by_bit(PoolStatusBitIndex::Swap, PoolStatusBitFlag::Disable);
            assert_eq!(pool_state.get_status_by_bit(PoolStatusBitIndex::Swap), false);

            pool_state.set_status(5); // 0000101
            assert_eq!(pool_state.get_status_by_bit(PoolStatusBitIndex::Swap), false);
            assert_eq!(pool_state.get_status_by_bit(PoolStatusBitIndex::Deposit), false);
            assert_eq!(pool_state.get_status_by_bit(PoolStatusBitIndex::Withdraw), true);

            pool_state.set_status(7); // 0000111
            assert_eq!(pool_state.get_status_by_bit(PoolStatusBitIndex::Swap), false);
            assert_eq!(pool_state.get_status_by_bit(PoolStatusBitIndex::Deposit), false);
            assert_eq!(pool_state.get_status_by_bit(PoolStatusBitIndex::Withdraw), false);

            pool_state.set_status(3); // 0000011
            assert_eq!(pool_state.get_status_by_bit(PoolStatusBitIndex::Swap), true);
            assert_eq!(pool_state.get_status_by_bit(PoolStatusBitIndex::Deposit), false);
            assert_eq!(pool_state.get_status_by_bit(PoolStatusBitIndex::Withdraw), false);
        }
    }
}
