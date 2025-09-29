///! 128 和 256 位数字
///! U128 比 u128 更高效
///! https://github.com/solana-labs/solana/issues/19549
use uint::construct_uint;
construct_uint! {
    pub struct U128(2);
}

construct_uint! {
    pub struct U256(4);
}

pub trait CheckedCeilDiv: Sized {
    /// 执行向上取整除法
    fn checked_ceil_div(&self, rhs: Self) -> Option<Self>;
}

impl CheckedCeilDiv for u128 {
    fn checked_ceil_div(&self, rhs: Self) -> Option<Self> {
        let mut quotient = self.checked_div(rhs)?;
        let remainder = self.checked_rem(rhs)?;
        if remainder != 0 {
            quotient = quotient.checked_add(1)?;
        }
        Some(quotient)
    }
}

pub trait DownCast {
    fn to_u64(&self) -> Option<u64>;
}

impl DownCast for u128 {
    fn to_u64(&self) -> Option<u64> {
        if *self > u64::MAX as u128 {
            None
        } else {
            Some(*self as u64)
        }
    }
}
