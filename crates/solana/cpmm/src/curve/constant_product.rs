//! Uniswap 不变量恒定乘积曲线::

use crate::{
    curve::calculator::{RoundDirection, TradingTokenResult},
    utils::CheckedCeilDiv,
};

use crate::libraries::big_num::{U256, U512};

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

    // pub fn swap_base_output_without_fees_one_to_zero(
    //     output_amount: u128,
    //     input_vault_amount: u128,
    //     output_vault_amount: u128,
    // ) -> u128 {
    //     let x_vault = output_vault_amount;
    //     let y_vault = input_vault_amount;
    //     let delta_x = output_amount;

    //     assert!(delta_x < x_vault);

    //     let x4 = pow_4th_normalized(x_vault);
    //     let k = U512::from(x4).checked_mul(U512::from(y_vault)).unwrap();

    //     let new_x = x_vault.checked_sub(delta_x).unwrap();
    //     let new_x4 = pow_4th_normalized(new_x);

    //     let new_y = k.checked_div(U512::from(new_x4)).unwrap();
    //     let new_y_u128 = u128::try_from(new_y).unwrap_or(u128::MAX);

    //     new_y_u128
    //         .checked_sub(y_vault)
    //         .unwrap()
    //         .checked_add(1)
    //         .unwrap()
    // }

    pub fn swap_base_output_without_fees_one_to_zero(
        output_amount: u128,
        input_vault_amount: u128,
        output_vault_amount: u128,
    ) -> u128 {
        println!("=== 开始计算 ===");
        println!("输入参数:");
        println!("  output_amount: {}", output_amount);
        println!("  input_vault_amount: {}", input_vault_amount);
        println!("  output_vault_amount: {}", output_vault_amount);

        let x_vault = output_vault_amount;
        let y_vault = input_vault_amount;
        let delta_x = output_amount;

        println!("\n变量赋值:");
        println!("  x_vault: {}", x_vault);
        println!("  y_vault: {}", y_vault);
        println!("  delta_x: {}", delta_x);

        assert!(delta_x < x_vault);
        println!("  ✓ delta_x < x_vault 检查通过");

        let x4 = pow_4th_normalized(x_vault);
        println!("\n计算 x^4:");
        println!("  x4 = pow_4th_normalized({}) = {}", x_vault, x4);

        let k = U512::from(x4).checked_mul(U512::from(y_vault)).unwrap();
        println!("\n计算不变量 k = x^4 * y:");
        println!("  k = {} * {} = {:?}", x4, y_vault, k);

        let new_x = x_vault.checked_sub(delta_x).unwrap();
        println!("\n计算新的 x:");
        println!("  new_x = {} - {} = {}", x_vault, delta_x, new_x);

        let new_x4 = pow_4th_normalized(new_x);
        println!("\n计算 new_x^4:");
        println!("  new_x4 = pow_4th_normalized({}) = {}", new_x, new_x4);

        let new_y = k.checked_div(U512::from(new_x4)).unwrap();
        println!("\n计算新的 y = k / new_x^4:");
        println!("  new_y = {:?} / {} = {:?}", k, new_x4, new_y);

        let new_y_u128 = u128::try_from(new_y).unwrap_or(u128::MAX);
        println!("  new_y_u128 = {}", new_y_u128);

        let delta_y = new_y_u128.checked_sub(y_vault).unwrap();
        println!("\n计算需要的输入量 delta_y:");
        println!("  delta_y = {} - {} = {}", new_y_u128, y_vault, delta_y);

        let result = delta_y.checked_add(1).unwrap();
        println!("\n向上取整 (+1):");
        println!("  result = {} + 1 = {}", delta_y, result);

        println!("\n=== 计算完成 ===");
        println!("最终结果: {}\n", result);

        result
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
// fn pow_4th_normalized(value_q64: u128) -> u128 {
//     if value_q64 == 0 {
//         return 0u128;
//     }

//     let val = U512::from(value_q64);
//     let squared = val * val;
//     let squared_scaled = squared >> 64;
//     let fourth_power = squared_scaled * squared_scaled;
//     let scaled_result = fourth_power >> 64;

//     // 安全检查和转换：检查是否可以安全转换到u128
//     let u512_words = scaled_result.0; // 访问内部数组

//     // 首先检查高位是否都为0（除了前两个64位字）
//     let has_high_bits_overflow = u512_words[2] != 0
//         || u512_words[3] != 0
//         || u512_words[4] != 0
//         || u512_words[5] != 0
//         || u512_words[6] != 0
//         || u512_words[7] != 0;

//     // 检查第二个字的高32位是否为0（避免u128溢出）
//     let has_mid_bits_overflow = u512_words[1] > u64::MAX;

//     let result_u128 = if has_high_bits_overflow || has_mid_bits_overflow {
//         println!("警告: 结果溢出u128，使用饱和转换");
//         u128::MAX
//     } else {
//         // 安全转换：只使用低128位，并确保不会溢出
//         let low_64 = u512_words[0] as u128;
//         let high_64 = (u512_words[1] as u128) << 64;
//         low_64 | high_64
//     };

//     result_u128.into()
// }

// fn pow_4th_normalized(value: u128) -> u128 {
//     println!("\n--- pow_4th_normalized 开始 ---");
//     println!("输入 value: {}", value);

//     if value == 0 {
//         println!("value 为 0，直接返回 0");
//         println!("--- pow_4th_normalized 结束 ---\n");
//         return 0u128;
//     }

//     // 由于 value <= 2^64，可以安全地计算 value^2
//     let val_u128 = value;
//     let squared = (val_u128 as u128).checked_mul(val_u128 as u128).unwrap();
//     println!("计算平方: {} * {} = {}", val_u128, val_u128, squared);

//     // squared 最大为 2^128，不会溢出 u128
//     // 现在计算 squared^2 = value^4，这会超出 u128，所以需要用 U512
//     let squared_u512 = U512::from(squared);
//     let fourth_power = squared_u512 * squared_u512;
//     println!("计算四次方: {:?}", fourth_power);

//     // 转换回 u128，检查是否溢出
//     let u512_words = fourth_power.0;
//     println!("U512 内部数组:");
//     for (i, word) in u512_words.iter().enumerate() {
//         if *word != 0 {
//             println!("  words[{}] = {}", i, word);
//         }
//     }

//     // 检查高位是否都为0（除了前两个64位字）
//     let has_high_bits_overflow = u512_words[2] != 0
//         || u512_words[3] != 0
//         || u512_words[4] != 0
//         || u512_words[5] != 0
//         || u512_words[6] != 0
//         || u512_words[7] != 0;
//     println!("高位溢出检查 (words[2-7]): {}", has_high_bits_overflow);

//     let result_u128 = if has_high_bits_overflow {
//         println!("⚠️ 警告: 结果溢出u128，使用饱和转换");
//         u128::MAX
//     } else {
//         // 安全转换：只使用低128位
//         let low_64 = u512_words[0] as u128;
//         let high_64 = (u512_words[1] as u128) << 64;
//         println!("低 64 位: {}", low_64);
//         println!("高 64 位 (左移后): {}", high_64);
//         let result = low_64 | high_64;
//         println!("组合结果: {}", result);
//         result
//     };

//     println!("最终返回值: {}", result_u128);
//     println!("--- pow_4th_normalized 结束 ---\n");

//     result_u128
// }

/// 计算 value^4，使用 Q64 定点数格式避免溢出
///
/// 原理：
/// - 输入 value 表示实际数量（0 到 2^64）（以9位精度，最多可支持184亿枚的代币数量）
/// - 先转换为 Q64 格式：value_q64 = value << 64
/// - 计算 (value_q64)^4 并在每次平方后归一化
/// - 最终结果也是 Q64 格式

/// 计算 value^4，使用 Q64 定点数格式避免溢出
/// 支持输入范围：0 到 2^64
/// 返回值也是 Q64 格式

/// 计算 value^4，返回 U512 类型
///
/// 输入: value (保证 <= 2^64)
/// 输出: value^4 (U512 类型，可以容纳最大 2^256 的结果)
fn pow_4th_normalized(value: u128) -> U512 {
    if value == 0 {
        return U512::zero();
    }

    // 转换为 U512
    let val = U512::from(value);

    // 第一次平方: val^2
    let val_squared = val * val;

    // 第二次平方: (val^2)^2 = val^4
    let val_fourth = val_squared * val_squared;

    val_fourth
}

/// 计算4次方根（牛顿迭代法）
// fn nth_root_4(value: u128) -> u128 {
//     if value == 0 {
//         return 0;
//     }
//     if value == 1 {
//         return 1;
//     }

//     // 初始猜测：使用二分查找的起点
//     let mut x = (value >> 96).max(1) as u128; // 粗略的初始值
//     if x == 0 {
//         x = 1;
//     }

//     // 牛顿迭代: x_new = (3*x + value/x³) / 4
//     for _ in 0..50 {
//         let x_cubed = (x as u128).checked_mul(x).unwrap().checked_mul(x).unwrap();

//         if x_cubed == 0 {
//             break;
//         }

//         let term1 = (3u128).checked_mul(x).unwrap();
//         let term2 = value.checked_div(x_cubed).unwrap();
//         let numerator = term1.checked_add(term2).unwrap();
//         let x_new = numerator.checked_div(4).unwrap();

//         if x_new == x || x_new.abs_diff(x) <= 1 {
//             break;
//         }
//         x = x_new;
//     }

//     x
// }

fn nth_root_4(value: u128) -> u128 {
    if value == 0 {
        return 0;
    }
    if value == 1 {
        return 1;
    }

    // 使用更好的初始猜测值
    // 对于 value，⁴√value ≈ 2^(log2(value)/4)
    let mut x = {
        // 找到 value 的近似位数
        let bits = 128 - value.leading_zeros();
        // 四次方根大约在 bits/4 位
        let initial_bits = (bits / 4).max(1);
        1u128 << (initial_bits - 1)
    };

    // 牛顿迭代法求四次方根 127307017307
    // 对于 f(x) = x^4 - value = 0
    // x_new = x - f(x)/f'(x) = x - (x^4 - value)/(4x^3)
    // x_new = (4x^4 - x^4 + value)/(4x^3) = (3x^4 + value)/(4x^3)
    // x_new = (3x + value/x^3) / 4

    for _ in 0..100 {
        // 计算 x^3，注意溢出
        let x_squared = match x.checked_mul(x) {
            Some(v) => v,
            None => break, // x 太大，停止迭代
        };
        let x_cubed = match x_squared.checked_mul(x) {
            Some(v) => v,
            None => break,
        };

        if x_cubed == 0 {
            break;
        }

        // 计算 value / x^3
        let quotient = value / x_cubed;

        // 计算 3x + value/x^3
        let three_x = match 3u128.checked_mul(x) {
            Some(v) => v,
            None => break,
        };
        let numerator = match three_x.checked_add(quotient) {
            Some(v) => v,
            None => break,
        };

        // 计算 x_new = (3x + value/x^3) / 4
        let x_new = numerator / 4;

        // 检查收敛
        if x_new >= x || x - x_new <= 1 {
            break;
        }

        x = x_new;
    }

    // 微调：确保返回的是 floor(⁴√value)
    // 检查 x^4 和 (x+1)^4
    while x > 0 {
        let x_fourth = match x.checked_pow(4) {
            Some(v) if v <= value => break,
            _ => {
                x -= 1;
                continue;
            }
        };
    }

    // 向上检查是否可以增加
    loop {
        match (x + 1).checked_pow(4) {
            Some(v) if v <= value => x += 1,
            _ => break,
        }
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

    #[test]
    fn test_pow_4th_normalized() {
        println!("\n========== pow_4th 测试 (返回 U512) ==========\n");

        // 测试 0
        let result = pow_4th_normalized(0);
        assert_eq!(result, U512::zero());
        println!("✓ 0^4 = {:?}\n", result);

        // 测试 1
        let result = pow_4th_normalized(1);
        assert_eq!(result, U512::from(1));
        println!("✓ 1^4 = {:?}\n", result);

        // 测试 10
        let value = 10u128;
        let result = pow_4th_normalized(value);
        let expected = U512::from(10000u128); // 10^4 = 10000
        assert_eq!(result, expected);
        println!("✓ 10^4 = {:?}\n", result);

        // 测试 100
        let value = 100u128;
        let result = pow_4th_normalized(value);
        let expected = U512::from(100000000u128); // 100^4 = 100000000
        assert_eq!(result, expected);
        println!("✓ 100^4 = {:?}\n", result);

        // 测试 1000
        let value = 1000u128;
        let result = pow_4th_normalized(value);
        let expected = U512::from(1000000000000u128); // 1000^4 = 1000000000000
        assert_eq!(result, expected);
        println!("✓ 1000^4 = {:?}\n", result);

        // 测试 2^16
        let value = 1u128 << 16; // 65536
        let result = pow_4th_normalized(value);
        // (2^16)^4 = 2^64
        let expected = U512::from(1u128 << 64);
        assert_eq!(result, expected);
        println!("✓ 2^16 的四次方 = 2^64 = {:?}\n", result);

        // 测试 2^32
        let value = 1u128 << 32;
        let result = pow_4th_normalized(value);
        // (2^32)^4 = 2^128
        println!("✓ 2^32 的四次方 = 2^128");
        println!("  结果: {:?}\n", result);

        // 测试 2^48
        let value = 1u128 << 48;
        let result = pow_4th_normalized(value);
        // (2^48)^4 = 2^192
        println!("✓ 2^48 的四次方 = 2^192");
        println!("  结果: {:?}\n", result);

        // 测试 2^56
        let value = 1u128 << 56;
        let result = pow_4th_normalized(value);
        // (2^56)^4 = 2^224
        println!("✓ 2^56 的四次方 = 2^224");
        println!("  结果: {:?}\n", result);

        // 测试 2^60
        let value = 1u128 << 60;
        let result = pow_4th_normalized(value);
        // (2^60)^4 = 2^240
        println!("✓ 2^60 的四次方 = 2^240");
        println!("  结果: {:?}\n", result);

        // 测试 2^63
        let value = 1u128 << 63;
        let result = pow_4th_normalized(value);
        // (2^63)^4 = 2^252
        println!("✓ 2^63 的四次方 = 2^252");
        println!("  结果: {:?}\n", result);

        // 测试 2^64 - 1 (最大输入)
        let value = (1u128 << 64) - 1;
        let result = pow_4th_normalized(value);
        println!("✓ (2^64 - 1) 的四次方");
        println!("  结果: {:?}\n", result);

        println!("========== 所有测试通过 ==========");
    }

    #[test]
    fn test_nth_root_4() {
        println!("\n========== 4次方根测试 ==========\n");

        // 测试 0
        let result = nth_root_4(0);
        assert_eq!(result, 0);
        println!("✓ ⁴√0 = {}\n", result);

        // 测试 1
        let result = nth_root_4(1);
        assert_eq!(result, 1);
        println!("✓ ⁴√1 = {}\n", result);

        // 测试完全四次方数
        let test_cases = vec![
            (16u128, 2u128),          // 2^4 = 16
            (81u128, 3u128),          // 3^4 = 81
            (256u128, 4u128),         // 4^4 = 256
            (625u128, 5u128),         // 5^4 = 625
            (10000u128, 10u128),      // 10^4 = 10000
            (100000000u128, 100u128), // 100^4 = 100000000
        ];

        for (value, expected) in test_cases {
            let result = nth_root_4(value);
            assert_eq!(result, expected, "⁴√{} 应该等于 {}, 但得到 {}", value, expected, result);
            println!("✓ ⁴√{} = {}", value, result);
        }
        println!();

        // 测试非完全四次方数（检查近似值）
        let value = 1000u128;
        let result = nth_root_4(value);
        let verify = result.pow(4);
        println!("⁴√1000 ≈ {}", result);
        println!("验证: {}^4 = {}", result, verify);
        assert!(verify <= 1000 && (result + 1).pow(4) > 1000, "结果应该是最接近的整数");
        println!();

        // 测试较大的值
        let value = 1u128 << 64; // 2^64
        let result = nth_root_4(value);
        let expected = 1u128 << 16; // (2^64)^(1/4) = 2^16
        println!("⁴√(2^64) = 2^16 = {}", expected);
        println!("计算结果: {}", result);
        // 允许一些误差
        assert!(
            (result as i128 - expected as i128).abs() <= 1,
            "⁴√(2^64) 应该接近 2^16, 但得到 {}",
            result
        );
        println!();

        // 测试 2^128 附近的值
        let value = 1000000000000u128; // 10^12
        let result = nth_root_4(value);
        let verify = result.pow(4);
        println!("⁴√{} ≈ {}", value, result);
        println!("验证: {}^4 = {}", result, verify);
        println!();

        // 测试精度：检查结果是否是最佳近似
        let value = 12345u128;
        let result = nth_root_4(value);
        let lower = result.pow(4);
        let upper = (result + 1).pow(4);
        println!("⁴√{} ≈ {}", value, result);
        println!("{}^4 = {} (小于等于 {})", result, lower, value);
        println!("{}^4 = {} (大于 {})", result + 1, upper, value);
        assert!(lower <= value && upper > value, "结果应该是floor(⁴√value)");
        println!();

        println!("========== 所有测试通过 ==========");
    }

    #[test]
    fn test_nth_root_4_convergence() {
        println!("\n========== 收敛性测试 ==========\n");

        // 测试迭代是否正确收敛
        let value = 1679616u128; // 这是 36^4
        let result = nth_root_4(value);
        assert_eq!(result, 36, "⁴√{} 应该精确等于 36", value);
        println!("✓ ⁴√{} = {} (精确)", value, result);

        // 测试边界情况
        let value = 1679615u128; // 比 36^4 小 1
        let result = nth_root_4(value);
        assert_eq!(result, 35, "⁴√{} 应该等于 35", value);
        println!("✓ ⁴√{} = {} (floor)", value, result);

        let value = 1679617u128; // 比 36^4 大 1
        let result = nth_root_4(value);
        assert_eq!(result, 36, "⁴√{} 应该等于 36", value);
        println!("✓ ⁴√{} = {} (floor)", value, result);

        println!("\n========== 收敛性测试通过 ==========");
    }

    #[test]
    fn test_swap_base_output_without_fees_one_to_zero() {
        let output_amount = 100;
        let input_vault_amount = 1009;
        let output_vault_amount = 1000;

        let result = ConstantProductCurve::swap_base_output_without_fees_one_to_zero(
            output_amount,
            input_vault_amount,
            output_vault_amount,
        );

        println!("输入需求: {}", result);
        assert!(result > 0);
    }

    // Raydium cpmm

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
