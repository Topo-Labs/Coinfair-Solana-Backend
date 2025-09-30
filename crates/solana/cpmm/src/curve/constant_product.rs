//! Uniswap 不变量恒定乘积曲线::

use crate::curve::calculator::{RoundDirection, TradingTokenResult};

use crate::libraries::big_num::U512;

/// 实现 CurveCalculator 的恒定乘积曲线结构体
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ConstantProductCurve;

impl ConstantProductCurve {
    /// 恒定乘积交换确保 x * y = 常数
    /// 恒定乘积交换计算，从其类中分离出来以便重用。
    ///
    /// 这保证对所有满足以下条件的值都有效：
    ///  - 1 <= source_vault_amount * destination_vault_amount <= u128::MAX
    ///  - 1 <= source_amount <= u64::MAX
    // pub fn swap_base_input_without_fees(
    //     input_amount: u128,
    //     input_vault_amount: u128,
    //     output_vault_amount: u128,
    // ) -> u128 {
    //     // (x + delta_x) * (y - delta_y) = x * y
    //     // delta_y = (delta_x * y) / (x + delta_x)
    //     let numerator = input_amount.checked_mul(output_vault_amount).unwrap();
    //     let denominator = input_vault_amount.checked_add(input_amount).unwrap();
    //     let output_amount = numerator.checked_div(denominator).unwrap();
    //     output_amount
    // }

    // pub fn swap_base_output_without_fees(
    //     output_amount: u128,
    //     input_vault_amount: u128,
    //     output_vault_amount: u128,
    // ) -> u128 {
    //     // (x + delta_x) * (y - delta_y) = x * y
    //     // delta_x = (x * delta_y) / (y - delta_y)
    //     let numerator = input_vault_amount.checked_mul(output_amount).unwrap();
    //     let denominator = output_vault_amount.checked_sub(output_amount).unwrap();
    //     let input_amount = numerator.checked_ceil_div(denominator).unwrap();
    //     input_amount
    // }

    pub fn swap_base_input_without_fees_zero_to_one(
        input_amount: u128,
        input_vault_amount: u128,
        output_vault_amount: u128,
    ) -> u128 {
        let x4 = pow_4th_normalized(input_vault_amount);
        let k = U512::from(x4).checked_mul(U512::from(output_vault_amount)).unwrap();

        let new_x = input_vault_amount.checked_add(input_amount).unwrap();
        let new_x4 = pow_4th_normalized(new_x);

        let new_y = k.checked_div(U512::from(new_x4)).unwrap();
        let new_y_u128 = u128::try_from(new_y).unwrap_or(0);

        output_vault_amount.checked_sub(new_y_u128).unwrap()
    }

    pub fn swap_base_output_without_fees_zero_to_one(
        output_amount: u128,
        input_vault_amount: u128,
        output_vault_amount: u128,
    ) -> u128 {
        assert!(output_amount < output_vault_amount);

        let x4 = pow_4th_normalized(input_vault_amount);
        let k = U512::from(x4).checked_mul(U512::from(output_vault_amount)).unwrap();

        let new_y = output_vault_amount.checked_sub(output_amount).unwrap();
        let required_x4 = k.checked_div(U512::from(new_y)).unwrap();
        let required_x4_u128 = u128::try_from(required_x4).unwrap_or(u128::MAX);

        let new_x = nth_root_4(required_x4_u128);

        new_x.checked_sub(input_vault_amount).unwrap().checked_add(1).unwrap()
    }

    pub fn swap_base_input_without_fees_one_to_zero(
        input_amount: u128,
        input_vault_amount: u128,
        output_vault_amount: u128,
    ) -> u128 {
        let x_vault = output_vault_amount;
        let y_vault = input_vault_amount;
        let delta_y = input_amount;

        let x4 = pow_4th_normalized(x_vault);
        let k = U512::from(x4).checked_mul(U512::from(y_vault)).unwrap();

        let new_y = y_vault.checked_add(delta_y).unwrap();
        let required_x4 = k.checked_div(U512::from(new_y)).unwrap();
        let required_x4_u128 = u128::try_from(required_x4).unwrap_or(0);

        let new_x = nth_root_4(required_x4_u128);

        x_vault.checked_sub(new_x).unwrap()
    }

    pub fn swap_base_output_without_fees_one_to_zero(
        output_amount: u128,
        input_vault_amount: u128,
        output_vault_amount: u128,
    ) -> u128 {
        let x_vault = output_vault_amount;
        let y_vault = input_vault_amount;
        let delta_x = output_amount;

        assert!(delta_x < x_vault);

        let x4 = pow_4th_normalized(x_vault);
        let k = U512::from(x4).checked_mul(U512::from(y_vault)).unwrap();

        let new_x = x_vault.checked_sub(delta_x).unwrap();
        let new_x4 = pow_4th_normalized(new_x);

        let new_y = k.checked_div(U512::from(new_x4)).unwrap();
        let new_y_u128 = u128::try_from(new_y).unwrap_or(u128::MAX);

        new_y_u128.checked_sub(y_vault).unwrap().checked_add(1).unwrap()
    }

    /// 根据给定的池代币数量获取交易代币数量，
    /// 需要提供总交易代币数量和池代币供应量。
    ///
    /// 恒定乘积实现是一个简单的比例计算，
    /// 用于确定特定数量的池代币对应多少交易代币
    pub fn lp_tokens_to_trading_tokens(
        lp_token_amount: u128,
        lp_token_supply: u128,
        token_0_vault_amount: u128,
        token_1_vault_amount: u128,
        round_direction: RoundDirection,
    ) -> Option<TradingTokenResult> {
        let mut token_0_amount = lp_token_amount
            .checked_mul(token_0_vault_amount)?
            .checked_div(lp_token_supply)?;
        let mut token_1_amount = lp_token_amount
            .checked_mul(token_1_vault_amount)?
            .checked_div(lp_token_supply)?;
        let (token_0_amount, token_1_amount) = match round_direction {
            RoundDirection::Floor => (token_0_amount, token_1_amount),
            RoundDirection::Ceiling => {
                let token_0_remainder = lp_token_amount
                    .checked_mul(token_0_vault_amount)?
                    .checked_rem(lp_token_supply)?;
                // 同时检查代币 A 和 B 的数量是否为 0，以避免对微量池代币
                // 取过多代币。例如，如果有人要求 1 个池代币，
                // 价值 0.01 个代币 A，我们避免向上取整取 1 个代币 A，
                // 而是返回 0，让它在后续处理中被拒绝。
                if token_0_remainder > 0 && token_0_amount > 0 {
                    token_0_amount += 1;
                }
                let token_1_remainder = lp_token_amount
                    .checked_mul(token_1_vault_amount)?
                    .checked_rem(lp_token_supply)?;
                if token_1_remainder > 0 && token_1_amount > 0 {
                    token_1_amount += 1;
                }
                (token_0_amount, token_1_amount)
            }
        };
        Some(TradingTokenResult {
            token_0_amount,
            token_1_amount,
        })
    }
}

/// 计算4次方（逐步放缩法）
fn pow_4th_normalized(value_q64: u128) -> u128 {
    if value_q64 == 0 {
        return 0u128;
    }

    let val = U512::from(value_q64);
    let squared = val * val;
    let squared_scaled = squared >> 64;
    let fourth_power = squared_scaled * squared_scaled;
    let scaled_result = fourth_power >> 64;

    // 安全检查和转换：检查是否可以安全转换到u128
    let u512_words = scaled_result.0; // 访问内部数组

    // 首先检查高位是否都为0（除了前两个64位字）
    let has_high_bits_overflow = u512_words[2] != 0
        || u512_words[3] != 0
        || u512_words[4] != 0
        || u512_words[5] != 0
        || u512_words[6] != 0
        || u512_words[7] != 0;

    // 检查第二个字的高32位是否为0（避免u128溢出）
    let has_mid_bits_overflow = u512_words[1] > u64::MAX;

    let result_u128 = if has_high_bits_overflow || has_mid_bits_overflow {
        println!("警告: 结果溢出u128，使用饱和转换");
        u128::MAX
    } else {
        // 安全转换：只使用低128位，并确保不会溢出
        let low_64 = u512_words[0] as u128;
        let high_64 = (u512_words[1] as u128) << 64;
        low_64 | high_64
    };

    result_u128.into()
}

/// 计算4次方根（牛顿迭代法）
fn nth_root_4(value: u128) -> u128 {
    if value == 0 {
        return 0;
    }
    if value == 1 {
        return 1;
    }

    // 初始猜测：使用二分查找的起点
    let mut x = (value >> 96).max(1) as u128; // 粗略的初始值
    if x == 0 {
        x = 1;
    }

    // 牛顿迭代: x_new = (3*x + value/x³) / 4
    for _ in 0..50 {
        let x_cubed = (x as u128).checked_mul(x).unwrap().checked_mul(x).unwrap();

        if x_cubed == 0 {
            break;
        }

        let term1 = (3u128).checked_mul(x).unwrap();
        let term2 = value.checked_div(x_cubed).unwrap();
        let numerator = term1.checked_add(term2).unwrap();
        let x_new = numerator.checked_div(4).unwrap();

        if x_new == x || x_new.abs_diff(x) <= 1 {
            break;
        }
        x = x_new;
    }

    x
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::curve::calculator::{
            test::{
                check_curve_value_from_swap, check_pool_value_from_deposit, check_pool_value_from_withdraw,
                total_and_intermediate,
            },
            RoundDirection, TradeDirection,
        },
        proptest::prelude::*,
    };

    fn check_pool_token_rate(
        token_a: u128,
        token_b: u128,
        deposit: u128,
        supply: u128,
        expected_a: u128,
        expected_b: u128,
    ) {
        let results = ConstantProductCurve::lp_tokens_to_trading_tokens(
            deposit,
            supply,
            token_a,
            token_b,
            RoundDirection::Ceiling,
        )
        .unwrap();
        assert_eq!(results.token_0_amount, expected_a);
        assert_eq!(results.token_1_amount, expected_b);
    }

    #[test]
    fn trading_token_conversion() {
        check_pool_token_rate(2, 49, 5, 10, 1, 25);
        check_pool_token_rate(100, 202, 5, 101, 5, 10);
        check_pool_token_rate(5, 501, 2, 10, 1, 101);
    }

    #[test]
    fn fail_trading_token_conversion() {
        let results = ConstantProductCurve::lp_tokens_to_trading_tokens(5, 10, u128::MAX, 0, RoundDirection::Floor);
        assert!(results.is_none());
        let results = ConstantProductCurve::lp_tokens_to_trading_tokens(5, 10, 0, u128::MAX, RoundDirection::Floor);
        assert!(results.is_none());
    }

    fn test_truncation(
        source_amount: u128,
        swap_source_amount: u128,
        swap_destination_amount: u128,
        expected_source_amount_swapped: u128,
        expected_destination_amount_swapped: u128,
    ) {
        let invariant = swap_source_amount * swap_destination_amount;
        let destination_amount_swapped = ConstantProductCurve::swap_base_input_without_fees_one_to_zero(
            source_amount,
            swap_source_amount,
            swap_destination_amount,
        );
        assert_eq!(source_amount, expected_source_amount_swapped);
        assert_eq!(destination_amount_swapped, expected_destination_amount_swapped);
        let new_invariant =
            (swap_source_amount + source_amount) * (swap_destination_amount - destination_amount_swapped);
        assert!(new_invariant >= invariant);
    }

    #[test]
    fn constant_product_swap_rounding() {
        let tests: &[(u128, u128, u128, u128, u128)] = &[
            // spot: 10 * 70b / ~4m = 174,999.99
            (10, 4_000_000, 70_000_000_000, 10, 174_999),
            // spot: 20 * 1 / 3.000 = 6.6667 (source can be 18 to get 6 dest.)
            (20, 30_000 - 20, 10_000, 20, 6),
            // spot: 19 * 1 / 2.999 = 6.3334 (source can be 18 to get 6 dest.)
            (19, 30_000 - 20, 10_000, 19, 6),
            // spot: 18 * 1 / 2.999 = 6.0001
            (18, 30_000 - 20, 10_000, 18, 6),
            // spot: 10 * 3 / 2.0010 = 14.99
            (10, 20_000, 30_000, 10, 14),
            // spot: 10 * 3 / 2.0001 = 14.999
            (10, 20_000 - 9, 30_000, 10, 14),
            // spot: 10 * 3 / 2.0000 = 15
            (10, 20_000 - 10, 30_000, 10, 15),
            // spot: 100 * 3 / 6.001 = 49.99 (source can be 99 to get 49 dest.)
            (100, 60_000, 30_000, 100, 49),
            // spot: 99 * 3 / 6.001 = 49.49
            (99, 60_000, 30_000, 99, 49),
            // spot: 98 * 3 / 6.001 = 48.99 (source can be 97 to get 48 dest.)
            (98, 60_000, 30_000, 98, 48),
        ];
        for (
            source_amount,
            swap_source_amount,
            swap_destination_amount,
            expected_source_amount,
            expected_destination_amount,
        ) in tests.iter()
        {
            test_truncation(
                *source_amount,
                *swap_source_amount,
                *swap_destination_amount,
                *expected_source_amount,
                *expected_destination_amount,
            );
        }
    }

    proptest! {
        #[test]
        fn curve_value_does_not_decrease_from_swap(
            source_token_amount in 1..u64::MAX,
            swap_source_amount in 1..u64::MAX,
            swap_destination_amount in 1..u64::MAX,
        ) {
            check_curve_value_from_swap(
                source_token_amount as u128,
                swap_source_amount as u128,
                swap_destination_amount as u128,
                TradeDirection::ZeroForOne
            );
        }
    }

    proptest! {
        #[test]
        fn curve_value_does_not_decrease_from_deposit(
            pool_token_amount in 1..u64::MAX,
            pool_token_supply in 1..u64::MAX,
            swap_token_a_amount in 1..u64::MAX,
            swap_token_b_amount in 1..u64::MAX,
        ) {
            let pool_token_amount = pool_token_amount as u128;
            let pool_token_supply = pool_token_supply as u128;
            let swap_token_a_amount = swap_token_a_amount as u128;
            let swap_token_b_amount = swap_token_b_amount as u128;
            // Make sure we will get at least one trading token out for each
            // side, otherwise the calculation fails
            prop_assume!(pool_token_amount * swap_token_a_amount / pool_token_supply >= 1);
            prop_assume!(pool_token_amount * swap_token_b_amount / pool_token_supply >= 1);
            check_pool_value_from_deposit(
                pool_token_amount,
                pool_token_supply,
                swap_token_a_amount,
                swap_token_b_amount,
            );
        }
    }

    proptest! {
        #[test]
        fn curve_value_does_not_decrease_from_withdraw(
            (pool_token_supply, pool_token_amount) in total_and_intermediate(u64::MAX),
            swap_token_a_amount in 1..u64::MAX,
            swap_token_b_amount in 1..u64::MAX,
        ) {
            let pool_token_amount = pool_token_amount as u128;
            let pool_token_supply = pool_token_supply as u128;
            let swap_token_a_amount = swap_token_a_amount as u128;
            let swap_token_b_amount = swap_token_b_amount as u128;
            // Make sure we will get at least one trading token out for each
            // side, otherwise the calculation fails
            prop_assume!(pool_token_amount * swap_token_a_amount / pool_token_supply >= 1);
            prop_assume!(pool_token_amount * swap_token_b_amount / pool_token_supply >= 1);
            check_pool_value_from_withdraw(
                pool_token_amount,
                pool_token_supply,
                swap_token_a_amount,
                swap_token_b_amount,
            );
        }
    }
}
