//! 所有费用信息，目前用于验证

pub const FEE_RATE_DENOMINATOR_VALUE: u64 = 1_000_000;

pub struct Fees {}

fn ceil_div(token_amount: u128, fee_numerator: u128, fee_denominator: u128) -> Option<u128> {
    token_amount
        .checked_mul(u128::from(fee_numerator))
        .unwrap()
        .checked_add(fee_denominator)?
        .checked_sub(1)?
        .checked_div(fee_denominator)
}

/// 计算交换费用的辅助函数
pub fn floor_div(token_amount: u128, fee_numerator: u128, fee_denominator: u128) -> Option<u128> {
    Some(token_amount.checked_mul(fee_numerator)?.checked_div(fee_denominator)?)
}

impl Fees {
    /// 以交易代币计算交易费用
    pub fn trading_fee(amount: u128, trade_fee_rate: u64) -> Option<u128> {
        ceil_div(
            amount,
            u128::from(trade_fee_rate),
            u128::from(FEE_RATE_DENOMINATOR_VALUE),
        )
    }

    /// 以交易代币计算所有者协议费用
    pub fn protocol_fee(amount: u128, protocol_fee_rate: u64) -> Option<u128> {
        floor_div(
            amount,
            u128::from(protocol_fee_rate),
            u128::from(FEE_RATE_DENOMINATOR_VALUE),
        )
    }

    /// 以交易代币计算所有者基金费用
    pub fn fund_fee(amount: u128, fund_fee_rate: u64) -> Option<u128> {
        floor_div(
            amount,
            u128::from(fund_fee_rate),
            u128::from(FEE_RATE_DENOMINATOR_VALUE),
        )
    }

    /// 计算创建者费用
    pub fn creator_fee(amount: u128, creator_fee_rate: u64) -> Option<u128> {
        ceil_div(
            amount,
            u128::from(creator_fee_rate),
            u128::from(FEE_RATE_DENOMINATOR_VALUE),
        )
    }

    pub fn split_creator_fee(total_fee: u128, trade_fee_rate: u64, creator_fee_rate: u64) -> Option<u128> {
        floor_div(
            total_fee,
            u128::from(creator_fee_rate),
            u128::from(trade_fee_rate + creator_fee_rate),
        )
    }

    pub fn calculate_pre_fee_amount(post_fee_amount: u128, trade_fee_rate: u64) -> Option<u128> {
        if trade_fee_rate == 0 {
            Some(post_fee_amount)
        } else {
            let numerator = post_fee_amount.checked_mul(u128::from(FEE_RATE_DENOMINATOR_VALUE))?;
            let denominator = u128::from(FEE_RATE_DENOMINATOR_VALUE).checked_sub(u128::from(trade_fee_rate))?;

            numerator
                .checked_add(denominator)?
                .checked_sub(1)?
                .checked_div(denominator)
        }
    }
}
