//! 交换计算

use crate::curve::{constant_product::ConstantProductCurve, fees::Fees};
use anchor_lang::prelude::*;
use {crate::error::ErrorCode, std::fmt::Debug};

/// 用于映射到ErrorCode::CalculationFailure的辅助函数
pub fn map_zero_to_none(x: u128) -> Option<u128> {
    if x == 0 {
        None
    } else {
        Some(x)
    }
}

/// 交易方向，因为曲线可以专门化处理每个代币
/// （通过添加偏移量或权重）
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TradeDirection {
    /// 输入token 0，输出token 1
    ZeroForOne,
    /// 输入token 1，输出token 0
    OneForZero,
}

/// 四舍五入方向。用于池代币到交易代币的转换，
/// 以避免在任何存款或提取中损失价值。
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RoundDirection {
    /// 向下舍入，即 1.9 => 1, 1.1 => 1, 1.5 => 1
    Floor,
    /// 向上舍入，即 1.9 => 2, 1.1 => 2, 1.5 => 2
    Ceiling,
}

impl TradeDirection {
    /// 给定交易方向，给出交易的相反方向，因此
    /// A到B变成B到A，反之亦然
    pub fn opposite(&self) -> TradeDirection {
        match self {
            TradeDirection::ZeroForOne => TradeDirection::OneForZero,
            TradeDirection::OneForZero => TradeDirection::ZeroForOne,
        }
    }
}

/// 编码同时存入双方的结果
#[derive(Debug, PartialEq)]
pub struct TradingTokenResult {
    /// 代币A的数量
    pub token_0_amount: u128,
    /// 代币B的数量
    pub token_1_amount: u128,
}

/// 编码从源代币到目标代币交换的所有结果
#[derive(Debug, PartialEq)]
pub struct SwapResult {
    /// 输入代币库中的新数量，不包括交易费
    pub new_input_vault_amount: u128,
    /// 输出代币库中的新数量，不包括交易费
    pub new_output_vault_amount: u128,
    /// 用户输入数量，包括交易费，不包括转账费
    pub input_amount: u128,
    /// 要转给用户的数量，包括转账费
    pub output_amount: u128,
    /// 进入池持有者的输入代币数量
    pub trade_fee: u128,
    /// 进入协议的输入代币数量
    pub protocol_fee: u128,
    /// 进入协议团队的输入代币数量
    pub fund_fee: u128,
    /// 进入创建者的费用代币数量
    pub creator_fee: u128,
}

/// 用于包装执行计算的trait对象的具体结构体。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CurveCalculator {}

impl CurveCalculator {
    pub fn validate_supply(token_0_amount: u64, token_1_amount: u64) -> Result<()> {
        if token_0_amount == 0 {
            return Err(ErrorCode::EmptySupply.into());
        }
        if token_1_amount == 0 {
            return Err(ErrorCode::EmptySupply.into());
        }
        Ok(())
    }

    /// 减去费用并计算给定源代币数量将提供多少目标代币。
    pub fn swap_base_input(
        trade_direction: TradeDirection,
        input_amount: u128,
        input_vault_amount: u128,
        output_vault_amount: u128,
        trade_fee_rate: u64,
        creator_fee_rate: u64,
        protocol_fee_rate: u64,
        fund_fee_rate: u64,
        is_creator_fee_on_input: bool,
    ) -> Option<SwapResult> {
        let mut creator_fee = 0;

        let trade_fee = Fees::trading_fee(input_amount, trade_fee_rate)?;
        let input_amount_less_fees = if is_creator_fee_on_input {
            creator_fee = Fees::creator_fee(input_amount, creator_fee_rate)?;
            input_amount
                .checked_sub(trade_fee)?
                .checked_sub(creator_fee)?
        } else {
            input_amount.checked_sub(trade_fee)?
        };
        let protocol_fee = Fees::protocol_fee(trade_fee, protocol_fee_rate)?;
        let fund_fee = Fees::fund_fee(trade_fee, fund_fee_rate)?;

        // let output_amount_swapped = ConstantProductCurve::swap_base_input_without_fees(
        //     input_amount_less_fees,
        //     input_vault_amount,
        //     output_vault_amount,
        // );

        let output_amount_swapped = match trade_direction {
            TradeDirection::ZeroForOne => {
                ConstantProductCurve::swap_base_input_without_fees_zero_to_one(
                    input_amount_less_fees,
                    input_vault_amount,
                    output_vault_amount,
                )
            }
            TradeDirection::OneForZero => {
                ConstantProductCurve::swap_base_input_without_fees_one_to_zero(
                    input_amount_less_fees,
                    input_vault_amount,
                    output_vault_amount,
                )
            }
        };

        let output_amount = if is_creator_fee_on_input {
            output_amount_swapped
        } else {
            creator_fee = Fees::creator_fee(output_amount_swapped, creator_fee_rate)?;
            output_amount_swapped.checked_sub(creator_fee)?
        };

        Some(SwapResult {
            new_input_vault_amount: input_vault_amount.checked_add(input_amount_less_fees)?,
            new_output_vault_amount: output_vault_amount.checked_sub(output_amount_swapped)?,
            input_amount,
            output_amount,
            trade_fee,
            protocol_fee,
            fund_fee,
            creator_fee,
        })
    }

    pub fn swap_base_output(
        trade_direction: TradeDirection,
        output_amount: u128,
        input_vault_amount: u128,
        output_vault_amount: u128,
        trade_fee_rate: u64,
        creator_fee_rate: u64,
        protocol_fee_rate: u64,
        fund_fee_rate: u64,
        is_creator_fee_on_input: bool,
    ) -> Option<SwapResult> {
        let trade_fee: u128;
        let mut creator_fee = 0;

        let actual_output_amount = if is_creator_fee_on_input {
            output_amount
        } else {
            let out_amount_with_creator_fee =
                Fees::calculate_pre_fee_amount(output_amount, creator_fee_rate)?;
            creator_fee = out_amount_with_creator_fee - output_amount;
            out_amount_with_creator_fee
        };

        // let input_amount_swapped = ConstantProductCurve::swap_base_output_without_fees(
        //     actual_output_amount,
        //     input_vault_amount,
        //     output_vault_amount,
        // );

        let input_amount_swapped = match trade_direction {
            TradeDirection::ZeroForOne => {
                ConstantProductCurve::swap_base_output_without_fees_zero_to_one(
                    actual_output_amount,
                    input_vault_amount,
                    output_vault_amount,
                )
            }
            TradeDirection::OneForZero => {
                ConstantProductCurve::swap_base_output_without_fees_one_to_zero(
                    actual_output_amount,
                    input_vault_amount,
                    output_vault_amount,
                )
            }
        };

        let input_amount = if is_creator_fee_on_input {
            let input_amount_with_fee = Fees::calculate_pre_fee_amount(
                input_amount_swapped,
                trade_fee_rate + creator_fee_rate,
            )
            .unwrap();
            let total_fee = input_amount_with_fee - input_amount_swapped;
            creator_fee = Fees::split_creator_fee(total_fee, trade_fee_rate, creator_fee_rate)?;
            trade_fee = total_fee - creator_fee;
            input_amount_with_fee
        } else {
            let input_amount_with_fee =
                Fees::calculate_pre_fee_amount(input_amount_swapped, trade_fee_rate).unwrap();
            trade_fee = input_amount_with_fee - input_amount_swapped;
            input_amount_with_fee
        };
        let protocol_fee = Fees::protocol_fee(trade_fee, protocol_fee_rate)?;
        let fund_fee = Fees::fund_fee(trade_fee, fund_fee_rate)?;
        Some(SwapResult {
            new_input_vault_amount: input_vault_amount.checked_add(input_amount_swapped)?,
            new_output_vault_amount: output_vault_amount.checked_sub(actual_output_amount)?,
            input_amount,
            output_amount,
            trade_fee,
            protocol_fee,
            fund_fee,
            creator_fee,
        })
    }

    /// 给定池代币数量获取交易代币数量，
    /// 提供总交易代币和池代币供应量。
    pub fn lp_tokens_to_trading_tokens(
        lp_token_amount: u128,
        lp_token_supply: u128,
        token_0_vault_amount: u128,
        token_1_vault_amount: u128,
        round_direction: RoundDirection,
    ) -> Option<TradingTokenResult> {
        ConstantProductCurve::lp_tokens_to_trading_tokens(
            lp_token_amount,
            lp_token_supply,
            token_0_vault_amount,
            token_1_vault_amount,
            round_direction,
        )
    }
}

/// 曲线的测试辅助函数
#[cfg(test)]
pub mod test {
    use {
        super::*, proptest::prelude::*, spl_math::precise_number::PreciseNumber,
        spl_math::uint::U256,
    };

    /// 大多数曲线执行转换测试时的ε值，
    /// 比较单侧存款与交换+存款。
    pub const CONVERSION_BASIS_POINTS_GUARANTEE: u128 = 50;

    /// 给定流动性参数计算曲线的总归一化值。
    ///
    /// 此函数的常数产品实现给出Uniswap不变量的平方根。
    pub fn normalized_value(
        swap_token_a_amount: u128,
        swap_token_b_amount: u128,
    ) -> Option<PreciseNumber> {
        let swap_token_a_amount = PreciseNumber::new(swap_token_a_amount)?;
        let swap_token_b_amount = PreciseNumber::new(swap_token_b_amount)?;
        swap_token_a_amount
            .checked_mul(&swap_token_b_amount)?
            .sqrt()
    }

    /// 测试函数检查交换从不会减少池的整体价值。
    ///
    /// 由于曲线计算使用无符号整数，在某些点可能发生截断，
    /// 意味着如果给交换者太多，可能在任一方向损失价值。
    ///
    /// 此测试保证价值的相对变化最多为1个归一化代币，
    /// 并且价值从不会因交易而减少。
    pub fn check_curve_value_from_swap(
        source_token_amount: u128,
        swap_source_amount: u128,
        swap_destination_amount: u128,
        trade_direction: TradeDirection,
    ) {
        let destination_amount_swapped =
            ConstantProductCurve::swap_base_input_without_fees_one_to_zero(
                source_token_amount,
                swap_source_amount,
                swap_destination_amount,
            );

        let (swap_token_0_amount, swap_token_1_amount) = match trade_direction {
            TradeDirection::ZeroForOne => (swap_source_amount, swap_destination_amount),
            TradeDirection::OneForZero => (swap_destination_amount, swap_source_amount),
        };
        let previous_value = swap_token_0_amount
            .checked_mul(swap_token_1_amount)
            .unwrap();

        let new_swap_source_amount = swap_source_amount.checked_add(source_token_amount).unwrap();
        let new_swap_destination_amount = swap_destination_amount
            .checked_sub(destination_amount_swapped)
            .unwrap();
        let (swap_token_0_amount, swap_token_1_amount) = match trade_direction {
            TradeDirection::ZeroForOne => (new_swap_source_amount, new_swap_destination_amount),
            TradeDirection::OneForZero => (new_swap_destination_amount, new_swap_source_amount),
        };

        let new_value = swap_token_0_amount
            .checked_mul(swap_token_1_amount)
            .unwrap();
        assert!(new_value >= previous_value);
    }

    /// 测试函数检查存款从不会减少池代币的价值。
    ///
    /// 由于曲线计算使用无符号整数，在某些点可能发生截断，
    /// 意味着如果给存款者太多，可能损失价值。
    pub fn check_pool_value_from_deposit(
        lp_token_amount: u128,
        lp_token_supply: u128,
        swap_token_0_amount: u128,
        swap_token_1_amount: u128,
    ) {
        let deposit_result = CurveCalculator::lp_tokens_to_trading_tokens(
            lp_token_amount,
            lp_token_supply,
            swap_token_0_amount,
            swap_token_1_amount,
            RoundDirection::Ceiling,
        )
        .unwrap();
        let new_swap_token_0_amount = swap_token_0_amount + deposit_result.token_0_amount;
        let new_swap_token_1_amount = swap_token_1_amount + deposit_result.token_1_amount;
        let new_lp_token_supply = lp_token_supply + lp_token_amount;

        // the following inequality must hold:
        // new_token_a / new_pool_token_supply >= token_a / pool_token_supply
        // which reduces to:
        // new_token_a * pool_token_supply >= token_a * new_pool_token_supply

        // These numbers can be just slightly above u64 after the deposit, which
        // means that their multiplication can be just above the range of u128.
        // For ease of testing, we bump these up to U256.
        let lp_token_supply = U256::from(lp_token_supply);
        let new_lp_token_supply = U256::from(new_lp_token_supply);
        let swap_token_0_amount = U256::from(swap_token_0_amount);
        let new_swap_token_0_amount = U256::from(new_swap_token_0_amount);
        let swap_token_b_amount = U256::from(swap_token_1_amount);
        let new_swap_token_b_amount = U256::from(new_swap_token_1_amount);

        assert!(
            new_swap_token_0_amount * lp_token_supply >= swap_token_0_amount * new_lp_token_supply
        );
        assert!(
            new_swap_token_b_amount * lp_token_supply >= swap_token_b_amount * new_lp_token_supply
        );
    }

    /// Test function checking that a withdraw never reduces the value of pool
    /// tokens.
    ///
    /// Since curve calculations use unsigned integers, there is potential for
    /// truncation at some point, meaning a potential for value to be lost if
    /// too much is given to the depositor.
    pub fn check_pool_value_from_withdraw(
        lp_token_amount: u128,
        lp_token_supply: u128,
        swap_token_0_amount: u128,
        swap_token_1_amount: u128,
    ) {
        let withdraw_result = CurveCalculator::lp_tokens_to_trading_tokens(
            lp_token_amount,
            lp_token_supply,
            swap_token_0_amount,
            swap_token_1_amount,
            RoundDirection::Floor,
        )
        .unwrap();
        let new_swap_token_0_amount = swap_token_0_amount - withdraw_result.token_0_amount;
        let new_swap_token_1_amount = swap_token_1_amount - withdraw_result.token_1_amount;
        let new_pool_token_supply = lp_token_supply - lp_token_amount;

        let value = normalized_value(swap_token_0_amount, swap_token_1_amount).unwrap();
        // since we can get rounding issues on the pool value which make it seem that
        // the value per token has gone down, we bump it up by an epsilon of 1
        // to cover all cases
        let new_value = normalized_value(new_swap_token_0_amount, new_swap_token_1_amount).unwrap();

        // the following inequality must hold:
        // new_pool_value / new_pool_token_supply >= pool_value / pool_token_supply
        // which can also be written:
        // new_pool_value * pool_token_supply >= pool_value * new_pool_token_supply

        let lp_token_supply = PreciseNumber::new(lp_token_supply).unwrap();
        let new_lp_token_supply = PreciseNumber::new(new_pool_token_supply).unwrap();
        assert!(new_value
            .checked_mul(&lp_token_supply)
            .unwrap()
            .greater_than_or_equal(&value.checked_mul(&new_lp_token_supply).unwrap()));
    }

    prop_compose! {
        pub fn total_and_intermediate(max_value: u64)(total in 1..max_value)
                        (intermediate in 1..total, total in Just(total))
                        -> (u64, u64) {
           (total, intermediate)
       }
    }
}
