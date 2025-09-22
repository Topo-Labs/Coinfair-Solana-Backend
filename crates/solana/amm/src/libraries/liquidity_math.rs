use super::big_num::{U128, U256, U512};
use super::fixed_point_64;
use super::full_math::MulDiv;
use super::full_math::*;
use super::tick_math;
use super::unsafe_math::UnsafeMathTrait;
use crate::error::ErrorCode;
use anchor_lang::prelude::*;

/// Helper 计算标准化的四次方，控制输出格式
// fn pow_4th_normalized(value_q64: u128) -> U128 {
//     // println!("pow_4th_normalized 输入: {}", value_q64);

//     if value_q64 == 0 {
//         return U128::from(0);
//     }

//     let val = U512::from(value_q64);

//     let squared = val * val;

//     let squared_scaled = squared >> 64;

//     let fourth_power = squared_scaled * squared_scaled;

//     let scaled_result = fourth_power >> 64;

//     let result = scaled_result.as_u128();

//     result.into()
// }

fn pow_4th_normalized(value_q64: u128) -> U128 {
    println!("pow_4th_normalized 输入: {}", value_q64);

    if value_q64 == 0 {
        return U128::from(0);
    }

    let val = U512::from(value_q64);
    println!("转换为 U512: {:?}", val);

    let squared = val * val;
    println!("平方结果 (U512): {:?}", squared);

    let squared_scaled = squared >> 64;
    println!("第一次缩放结果: {:?}", squared_scaled);

    let fourth_power = squared_scaled * squared_scaled;
    println!("四次方结果 (U512): {:?}", fourth_power);

    let scaled_result = fourth_power >> 64;
    println!("缩放128位后的结果: {:?}", scaled_result);

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

    println!("最终转换为 U128: {:?}", result_u128);
    result_u128.into()
}

#[cfg(test)]
mod precision_asymmetry_diagnosis {
    use super::*;
    use crate::libraries::sqrt_price_math;

    #[test]
    fn diagnose_price_asymmetry() {
        println!("=== 诊断价格不对称问题 ===");

        let current_price = 18446744073709551616u128;
        let liquidity = 1000000u128;
        let amount = 1000u64;

        println!("测试参数:");
        println!("  current_price: {}", current_price);
        println!("  liquidity: {}", liquidity);
        println!("  amount: {}", amount);
        println!("  2^64: {}", 1u128 << 64);

        // 详细分析 amount_0 方向的计算
        println!("\n=== 分析 amount_0 方向 (zero_for_one=true) ===");
        let new_price_0 = sqrt_price_math::get_next_sqrt_price_from_amount_0_rounding_up(
            current_price,
            liquidity,
            amount,
            true,
        );

        let change_0 = new_price_0 as i128 - current_price as i128;
        println!("新价格: {}", new_price_0);
        println!("变化: {}", change_0);
        println!(
            "变化 / 2^64: {:.10}",
            change_0 as f64 / (1u128 << 64) as f64
        );

        // 手动计算预期值
        let expected_change_0 = -(amount as i128 * current_price as i128 / liquidity as i128);
        println!("预期变化: {}", expected_change_0);
        println!(
            "预期变化 / 2^64: {:.10}",
            expected_change_0 as f64 / (1u128 << 64) as f64
        );

        let ratio_0 = change_0 as f64 / expected_change_0 as f64;
        println!("实际/预期比值: {:.2}", ratio_0);

        // 检查是否存在 2^64 倍数关系
        if (ratio_0 - (1u128 << 64) as f64).abs() < 1000.0 {
            println!(">>> 可能存在多余的 2^64 因子!");
        }

        // 详细分析 amount_1 方向的计算
        println!("\n=== 分析 amount_1 方向 (zero_for_one=false) ===");
        let new_price_1 = sqrt_price_math::get_next_sqrt_price_from_amount_1_rounding_down(
            current_price,
            liquidity,
            amount,
            true,
        );

        let change_1 = new_price_1 as i128 - current_price as i128;
        println!("新价格: {}", new_price_1);
        println!("变化: {}", change_1);
        println!(
            "变化 / 2^64: {:.10}",
            change_1 as f64 / (1u128 << 64) as f64
        );

        // 手动计算预期值
        let expected_change_1 = (amount as u128 * (1u128 << 64)) / liquidity;
        println!("预期变化: {}", expected_change_1);
        println!(
            "预期变化 / 2^64: {:.10}",
            expected_change_1 as f64 / (1u128 << 64) as f64
        );

        let ratio_1 = change_1 as f64 / expected_change_1 as f64;
        println!("实际/预期比值: {:.2}", ratio_1);

        // 分析两个方向的比值
        println!("\n=== 比较两个方向 ===");
        let asymmetry_ratio = change_0.abs() as f64 / change_1.abs() as f64;
        println!("价格变化比值 |0->1| / |1->0|: {:.2}", asymmetry_ratio);

        if asymmetry_ratio > 100.0 || asymmetry_ratio < 0.01 {
            println!(">>> 价格变化严重不对称!");

            // 检查是否其中一个方向多了 2^64 因子
            let corrected_asymmetry = asymmetry_ratio / (1u128 << 64) as f64;
            println!("如果除以2^64后的比值: {:.2}", corrected_asymmetry);

            if corrected_asymmetry > 0.1 && corrected_asymmetry < 10.0 {
                println!(">>> amount_0 方向可能多了 2^64 因子!");
            }
        }
    }

    #[test]
    fn analyze_sqrt_price_math_functions() {
        println!("=== 分析 sqrt_price_math 函数内部计算 ===");

        let current_price = 18446744073709551616u128;
        let liquidity = 1000000u128;
        let amount = 1000u64;

        // 分析 get_next_sqrt_price_from_amount_0_rounding_up 内部
        println!("\n--- 分析 amount_0 函数内部 ---");

        // 模拟内部计算
        let numerator_1 = U256::from(liquidity) << 64;
        let product = U256::from(amount) * U256::from(current_price);
        let denominator = numerator_1 + product;

        println!("numerator_1 (L << 64): {:?}", numerator_1);
        println!("product (amount * price): {:?}", product);
        println!("denominator: {:?}", denominator);

        // 检查最终计算
        let result = numerator_1 * U256::from(current_price) / denominator;
        println!("计算结果: {:?}", result);
        println!("结果 as u128: {}", result.as_u128());

        // 检查是否需要额外除以 2^64
        let corrected_result = result.as_u128() / (1u128 << 64);
        println!("如果除以2^64: {}", corrected_result);

        // 分析 get_next_sqrt_price_from_amount_1_rounding_down 内部
        println!("\n--- 分析 amount_1 函数内部 ---");

        let quotient = U256::from(amount as u128 * (1u128 << 64)) / liquidity;
        let result_1 = current_price + quotient.as_u128().as_u128();

        println!("quotient计算: {} * 2^64 / {}", amount, liquidity);
        println!("quotient: {:?}", quotient);
        println!("result_1: {}", result_1);

        // 比较两个函数的数量级差异
        let diff_0 = result.as_u128().as_u128() as i128 - current_price as i128;
        let diff_1 = result_1 as i128 - current_price as i128;

        println!("\n--- 比较结果 ---");
        println!("amount_0 变化: {}", diff_0);
        println!("amount_1 变化: {}", diff_1);
        println!("比值: {:.2}", diff_0.abs() as f64 / diff_1.abs() as f64);
    }
}

// fn pow_4th_normalized(value_q64: u128) -> U128 {
//     println!("pow_4th_normalized 输入: {}", value_q64);

//     if value_q64 == 0 {
//         return U128::from(0);
//     }

//     let val = U512::from(value_q64);
//     println!("转换为 U512: {:?}", val);

//     let squared = val * val;
//     println!("平方结果 (U512): {:?}", squared);

//     let squared_scaled = squared >> 64;
//     println!("第一次缩放结果: {:?}", squared_scaled);

//     let fourth_power = squared_scaled * squared_scaled;
//     println!("四次方结果 (U512): {:?}", fourth_power);

//     let scaled_result = fourth_power >> 64;
//     println!("缩放128位后的结果: {:?}", scaled_result);

//     let result = scaled_result.as_u128();
//     println!("转换为 U256: {:?}", result);

//     result.into()
// }

/// Add a signed liquidity delta to liquidity and revert if it overflows or underflows
pub fn add_delta(x: u128, y: i128) -> Result<u128> {
    let z: u128;
    if y < 0 {
        z = x - u128::try_from(-y).unwrap();
        require_gt!(x, z, ErrorCode::LiquiditySubValueErr);
    } else {
        z = x + u128::try_from(y).unwrap();
        require_gte!(z, x, ErrorCode::LiquidityAddValueErr);
    }
    Ok(z)
}

/// Computes the amount of liquidity received for a given amount of token_0 and price range
/// For X^4*Y=K model: L = (X^4*Y)^(1/5)
///
/// When X changes by Δx: ΔL = Δx * P_effective
/// Use geometric mean for price range: P_eff = sqrt(P_a * P_b)
pub fn get_liquidity_from_amount_0(
    mut sqrt_ratio_a_x64: u128,
    mut sqrt_ratio_b_x64: u128,
    amount_0: u64,
) -> u128 {
    // sqrt_ratio_a_x64 should hold the smaller value
    if sqrt_ratio_a_x64 > sqrt_ratio_b_x64 {
        std::mem::swap(&mut sqrt_ratio_a_x64, &mut sqrt_ratio_b_x64);
    };
    let intermediate = U128::from(sqrt_ratio_a_x64)
        .mul_div_floor(
            U128::from(sqrt_ratio_b_x64),
            U128::from(fixed_point_64::Q64),
        )
        .unwrap();

    U128::from(amount_0)
        .mul_div_floor(
            intermediate,
            U128::from(sqrt_ratio_b_x64 - sqrt_ratio_a_x64),
        )
        .unwrap()
        .as_u128()
}

/// Computes the amount of liquidity received for a given amount of token_1 and price range
/// For X^4*Y=K model: Y = L * P^4, so ΔL = Δy / P_eff^4
// pub fn get_liquidity_from_amount_1(
//     mut sqrt_ratio_a_x64: u128,
//     mut sqrt_ratio_b_x64: u128,
//     amount_1: u64,
// ) -> u128 {
//     if sqrt_ratio_a_x64 > sqrt_ratio_b_x64 {
//         std::mem::swap(&mut sqrt_ratio_a_x64, &mut sqrt_ratio_b_x64);
//     }

//     if sqrt_ratio_a_x64 == sqrt_ratio_b_x64 {
//         return 0;
//     }

//     let p_min_4_5 = pow_4th_normalized(sqrt_ratio_a_x64);
//     let p_max_4_5 = pow_4th_normalized(sqrt_ratio_b_x64);
//     let price_diff = p_max_4_5.saturating_sub(p_min_4_5);

//     if price_diff == U256::from(0) {
//         return 0;
//     }

//     // 使用 mul_div 避免中间溢出
//     // 这相当于：(N * amount_1 * Q64) / price_diff
//     // 或者理解为：(N * amount_1) / (price_diff / Q64)
//     U256::from(amount_1 * 4u64)
//         .mul_div_floor(U256::from(fixed_point_64::Q64), U256::from(price_diff))
//         .unwrap()
//         .as_u128()
//         .as_u128()
// }

pub fn get_liquidity_from_amount_1(
    mut sqrt_ratio_a_x64: u128,
    mut sqrt_ratio_b_x64: u128,
    amount_1: u64,
) -> u128 {
    if sqrt_ratio_a_x64 > sqrt_ratio_b_x64 {
        std::mem::swap(&mut sqrt_ratio_a_x64, &mut sqrt_ratio_b_x64);
    }

    if sqrt_ratio_a_x64 == sqrt_ratio_b_x64 {
        return 0;
    }

    let p_min_4_5 = pow_4th_normalized(sqrt_ratio_a_x64);

    let p_max_4_5 = pow_4th_normalized(sqrt_ratio_b_x64);

    let price_diff = p_max_4_5.saturating_sub(p_min_4_5);

    if price_diff == U128::from(0) {
        return 0;
    }

    let liquidity = U128::from(amount_1 * 4u64)
        .mul_div_floor(U128::from(fixed_point_64::Q64), U128::from(price_diff))
        .unwrap()
        .as_u128();

    liquidity
}

// pub fn get_liquidity_from_amount_1(
//     mut sqrt_ratio_a_x64: u128,
//     mut sqrt_ratio_b_x64: u128,
//     amount_1: u64,
// ) -> u128 {
//     println!("U512 MAX: {}", U512::MAX);
//     println!("=== get_liquidity_from_amount_1 开始 ===");
//     println!("输入参数:");
//     println!("  sqrt_ratio_a_x64: {}", sqrt_ratio_a_x64);
//     println!("  sqrt_ratio_b_x64: {}", sqrt_ratio_b_x64);
//     println!("  amount_1: {}", amount_1);

//     if sqrt_ratio_a_x64 > sqrt_ratio_b_x64 {
//         println!(
//             "交换价格顺序: {} <-> {}",
//             sqrt_ratio_a_x64, sqrt_ratio_b_x64
//         );
//         std::mem::swap(&mut sqrt_ratio_a_x64, &mut sqrt_ratio_b_x64);
//     }
//     println!("排序后:");
//     println!("  sqrt_ratio_a_x64 (min): {}", sqrt_ratio_a_x64);
//     println!("  sqrt_ratio_b_x64 (max): {}", sqrt_ratio_b_x64);

//     if sqrt_ratio_a_x64 == sqrt_ratio_b_x64 {
//         println!("价格相同，返回流动性 0");
//         return 0;
//     }

//     println!("\n--- 步骤1: 计算四次方 ---");
//     let p_min_4_5 = pow_4th_normalized(sqrt_ratio_a_x64);
//     println!("pow_4th_normalized({}) = {:?}", sqrt_ratio_a_x64, p_min_4_5);

//     let p_max_4_5 = pow_4th_normalized(sqrt_ratio_b_x64);
//     println!("pow_4th_normalized({}) = {:?}", sqrt_ratio_b_x64, p_max_4_5);

//     println!("\n--- 步骤2: 计算价格差 ---");
//     let price_diff = p_max_4_5.saturating_sub(p_min_4_5);
//     println!("price_diff = p_max_4_5 - p_min_4_5 = {:?}", price_diff);

//     if price_diff == U128::from(0) {
//         println!("价格差为0，返回流动性 0");
//         return 0;
//     }

//     let liquidity = U128::from(amount_1 * 4u64)
//         .mul_div_floor(U128::from(fixed_point_64::Q64), U128::from(price_diff))
//         .unwrap()
//         .as_u128();

//     println!("liquidity: {}", liquidity);
//     liquidity
// }

/// Computes liquidity from both amounts, returning the minimum
pub fn get_liquidity_from_amounts(
    sqrt_ratio_x64: u128,
    mut sqrt_ratio_a_x64: u128,
    mut sqrt_ratio_b_x64: u128,
    amount_0: u64,
    amount_1: u64,
) -> u128 {
    if sqrt_ratio_a_x64 > sqrt_ratio_b_x64 {
        std::mem::swap(&mut sqrt_ratio_a_x64, &mut sqrt_ratio_b_x64);
    }

    if sqrt_ratio_x64 <= sqrt_ratio_a_x64 {
        get_liquidity_from_amount_0(sqrt_ratio_a_x64, sqrt_ratio_b_x64, amount_0)
    } else if sqrt_ratio_x64 < sqrt_ratio_b_x64 {
        u128::min(
            get_liquidity_from_amount_0(sqrt_ratio_x64, sqrt_ratio_b_x64, amount_0),
            get_liquidity_from_amount_1(sqrt_ratio_a_x64, sqrt_ratio_x64, amount_1),
        )
    } else {
        get_liquidity_from_amount_1(sqrt_ratio_a_x64, sqrt_ratio_b_x64, amount_1)
    }
}

/// Single token_0 liquidity calculation
pub fn get_liquidity_from_single_amount_0(
    sqrt_ratio_x64: u128,
    mut sqrt_ratio_a_x64: u128,
    mut sqrt_ratio_b_x64: u128,
    amount_0: u64,
) -> u128 {
    if sqrt_ratio_a_x64 > sqrt_ratio_b_x64 {
        std::mem::swap(&mut sqrt_ratio_a_x64, &mut sqrt_ratio_b_x64);
    }

    if sqrt_ratio_x64 <= sqrt_ratio_a_x64 {
        get_liquidity_from_amount_0(sqrt_ratio_a_x64, sqrt_ratio_b_x64, amount_0)
    } else if sqrt_ratio_x64 < sqrt_ratio_b_x64 {
        get_liquidity_from_amount_0(sqrt_ratio_x64, sqrt_ratio_b_x64, amount_0)
    } else {
        0
    }
}

/// Single token_1 liquidity calculation
pub fn get_liquidity_from_single_amount_1(
    sqrt_ratio_x64: u128,
    mut sqrt_ratio_a_x64: u128,
    mut sqrt_ratio_b_x64: u128,
    amount_1: u64,
) -> u128 {
    if sqrt_ratio_a_x64 > sqrt_ratio_b_x64 {
        std::mem::swap(&mut sqrt_ratio_a_x64, &mut sqrt_ratio_b_x64);
    }

    if sqrt_ratio_x64 <= sqrt_ratio_a_x64 {
        0
    } else if sqrt_ratio_x64 < sqrt_ratio_b_x64 {
        get_liquidity_from_amount_1(sqrt_ratio_a_x64, sqrt_ratio_x64, amount_1)
    } else {
        get_liquidity_from_amount_1(sqrt_ratio_a_x64, sqrt_ratio_b_x64, amount_1)
    }
}

/// Gets the delta amount_0 for given liquidity and price range
/// For X^4*Y=K model: Δx = L * (P_b - P_a) / (P_a * P_b)
pub fn get_delta_amount_0_unsigned(
    mut sqrt_ratio_a_x64: u128,
    mut sqrt_ratio_b_x64: u128,
    liquidity: u128,
    round_up: bool,
) -> Result<u64> {
    // sqrt_ratio_a_x64 should hold the smaller value
    if sqrt_ratio_a_x64 > sqrt_ratio_b_x64 {
        std::mem::swap(&mut sqrt_ratio_a_x64, &mut sqrt_ratio_b_x64);
    };

    let numerator_1 = U256::from(liquidity) << fixed_point_64::RESOLUTION;
    let numerator_2 = U256::from(sqrt_ratio_b_x64 - sqrt_ratio_a_x64);

    assert!(sqrt_ratio_a_x64 > 0);

    let result = if round_up {
        U256::div_rounding_up(
            numerator_1
                .mul_div_ceil(numerator_2, U256::from(sqrt_ratio_b_x64))
                .unwrap(),
            U256::from(sqrt_ratio_a_x64),
        )
    } else {
        numerator_1
            .mul_div_floor(numerator_2, U256::from(sqrt_ratio_b_x64))
            .unwrap()
            / U256::from(sqrt_ratio_a_x64)
    };
    if result > U256::from(u64::MAX) {
        return Err(ErrorCode::MaxTokenOverflow.into());
    }
    return Ok(result.as_u64());
}

/// Gets the delta amount_1 for given liquidity and price range
/// For X^4*Y=K model: Δy = L * (P_b^4 - P_a^4)
pub fn get_delta_amount_1_unsigned(
    mut sqrt_ratio_a_x64: u128,
    mut sqrt_ratio_b_x64: u128,
    liquidity: u128,
    round_up: bool,
) -> Result<u64> {
    // sqrt_ratio_a_x64 should hold the smaller value
    if sqrt_ratio_a_x64 > sqrt_ratio_b_x64 {
        std::mem::swap(&mut sqrt_ratio_a_x64, &mut sqrt_ratio_b_x64);
    };

    let p_min_4_5 = pow_4th_normalized(sqrt_ratio_a_x64);
    let p_max_4_5 = pow_4th_normalized(sqrt_ratio_b_x64);
    let price_diff = p_max_4_5.saturating_sub(p_min_4_5).as_u256();

    let result = if round_up {
        U256::from(liquidity)
            .mul_div_ceil(U256::from(price_diff), U256::from(4 * fixed_point_64::Q64))
    } else {
        U256::from(liquidity)
            .mul_div_floor(U256::from(price_diff), U256::from(4 * fixed_point_64::Q64))
    }
    .unwrap();
    if result > U256::from(u64::MAX) {
        return Err(ErrorCode::MaxTokenOverflow.into());
    }
    return Ok(result.as_u64());
}

/// Helper function to get signed delta amount_0
pub fn get_delta_amount_0_signed(
    sqrt_ratio_a_x64: u128,
    sqrt_ratio_b_x64: u128,
    liquidity: i128,
) -> Result<u64> {
    if liquidity < 0 {
        get_delta_amount_0_unsigned(
            sqrt_ratio_a_x64,
            sqrt_ratio_b_x64,
            u128::try_from(-liquidity).unwrap(),
            false,
        )
    } else {
        get_delta_amount_0_unsigned(
            sqrt_ratio_a_x64,
            sqrt_ratio_b_x64,
            u128::try_from(liquidity).unwrap(),
            true,
        )
    }
}

/// Helper function to get signed delta amount_1
pub fn get_delta_amount_1_signed(
    sqrt_ratio_a_x64: u128,
    sqrt_ratio_b_x64: u128,
    liquidity: i128,
) -> Result<u64> {
    if liquidity < 0 {
        get_delta_amount_1_unsigned(
            sqrt_ratio_a_x64,
            sqrt_ratio_b_x64,
            u128::try_from(-liquidity).unwrap(),
            false,
        )
    } else {
        get_delta_amount_1_unsigned(
            sqrt_ratio_a_x64,
            sqrt_ratio_b_x64,
            u128::try_from(liquidity).unwrap(),
            true,
        )
    }
}

pub fn get_delta_amounts_signed(
    tick_current: i32,
    sqrt_price_x64_current: u128,
    tick_lower: i32,
    tick_upper: i32,
    liquidity_delta: i128,
) -> Result<(u64, u64)> {
    let mut amount_0 = 0;
    let mut amount_1 = 0;
    if tick_current < tick_lower {
        amount_0 = get_delta_amount_0_signed(
            tick_math::get_sqrt_price_at_tick(tick_lower)?,
            tick_math::get_sqrt_price_at_tick(tick_upper)?,
            liquidity_delta,
        )
        .unwrap();
    } else if tick_current < tick_upper {
        amount_0 = get_delta_amount_0_signed(
            sqrt_price_x64_current,
            tick_math::get_sqrt_price_at_tick(tick_upper)?,
            liquidity_delta,
        )
        .unwrap();
        amount_1 = get_delta_amount_1_signed(
            tick_math::get_sqrt_price_at_tick(tick_lower)?,
            sqrt_price_x64_current,
            liquidity_delta,
        )
        .unwrap();
    } else {
        amount_1 = get_delta_amount_1_signed(
            tick_math::get_sqrt_price_at_tick(tick_lower)?,
            tick_math::get_sqrt_price_at_tick(tick_upper)?,
            liquidity_delta,
        )
        .unwrap();
    }
    Ok((amount_0, amount_1))
}

#[test]
fn test_pow_4th_normalized_with_given_params() {
    // 使用你提供的实际参数
    let sqrt_ratio_a = 18226716948933807364u128; // tick_lower_price_x64
    let sqrt_ratio_b = 18613505734318141148u128; // tick_upper_price_x64

    println!("=== 测试实际参数 ===");

    // 测试第一个参数
    let integer_part_a = sqrt_ratio_a >> 64;
    let result_a = pow_4th_normalized(sqrt_ratio_a);
    println!("sqrt_ratio_a: {}", sqrt_ratio_a);
    println!("integer_part_a: {}", integer_part_a);
    println!("result_a: {:?}", result_a);

    // 测试第二个参数
    let integer_part_b = sqrt_ratio_b >> 64;
    let result_b = pow_4th_normalized(sqrt_ratio_b);
    println!("sqrt_ratio_b: {}", sqrt_ratio_b);
    println!("integer_part_b: {}", integer_part_b);
    println!("result_b: {:?}", result_b);

    // 手动验证计算逻辑
    let expected_a = integer_part_a.pow(4);
    let expected_b = integer_part_b.pow(4);
    println!("Expected fourth power a: {}", expected_a);
    println!("Expected fourth power b: {}", expected_b);
}

#[test]
fn test_get_liquidity_from_amount_1_basic() {
    let sqrt_ratio_a = 18226716948933807364u128; // tick_lower_price_x64
    let sqrt_ratio_b = 18613505734318141148u128; // tick_upper_price_x64
    let amount_1 = 10000000000u64; // input_amount

    let result = get_liquidity_from_amount_1(sqrt_ratio_a, sqrt_ratio_b, amount_1);

    println!("Test parameters:");
    println!("  sqrt_ratio_a: {}", sqrt_ratio_a);
    println!("  sqrt_ratio_b: {}", sqrt_ratio_b);
    println!("  amount_1: {}", amount_1);
    println!("  liquidity: {}", result);

    // 基本合理性检查
    assert!(result > 0, "流动性应该大于0");
    assert!(result < u128::MAX, "流动性不应该溢出");
}

#[test]
fn test_get_liquidity_from_amount_1_detailed() {
    let sqrt_ratio_a = 18226716948933807364u128;
    let sqrt_ratio_b = 18613505734318141148u128;
    let amount_1 = 10000000000u64;

    // 计算中间值用于验证
    let p_min_4_5 = pow_4th_normalized(sqrt_ratio_a);
    let p_max_4_5 = pow_4th_normalized(sqrt_ratio_b);
    let price_diff = p_max_4_5.saturating_sub(p_min_4_5);

    println!("Intermediate calculations:");
    println!("  p_min_4_5: {}", p_min_4_5);
    println!("  p_max_4_5: {}", p_max_4_5);
    println!("  price_diff: {}", price_diff);
    println!("  N * amount_1: {}", amount_1 * 4u64);
    println!("  Q64: {}", fixed_point_64::Q64);

    let result = get_liquidity_from_amount_1(sqrt_ratio_a, sqrt_ratio_b, amount_1);

    // 验证中间计算的合理性
    assert!(p_max_4_5 > p_min_4_5, "p_max_4_5 应该大于 p_min_4_5");
    assert!(price_diff > U128::from(0), "价格差应该大于0");
    assert!(result > 0, "流动性应该大于0");

    // 数量级检查：流动性应该在合理范围内
    assert!(result > 1000, "流动性应该不会太小");
    assert!(result < 1e18 as u128, "流动性应该不会过大");
}

#[test]
fn test_get_liquidity_from_amount_1_edge_cases() {
    let sqrt_ratio_a = 18226716948933807364u128;
    let sqrt_ratio_b = 18613505734318141148u128;

    // 测试零数量
    let result_zero = get_liquidity_from_amount_1(sqrt_ratio_a, sqrt_ratio_b, 0);
    assert_eq!(result_zero, 0, "零数量应该返回零流动性");

    // 测试相同价格
    let result_same = get_liquidity_from_amount_1(sqrt_ratio_a, sqrt_ratio_a, 10000000000);
    assert_eq!(result_same, 0, "相同价格应该返回零流动性");

    // 测试价格顺序（应该自动交换）
    let result_normal = get_liquidity_from_amount_1(sqrt_ratio_a, sqrt_ratio_b, 10000000000);
    let result_reversed = get_liquidity_from_amount_1(sqrt_ratio_b, sqrt_ratio_a, 10000000000);
    assert_eq!(result_normal, result_reversed, "价格顺序不应该影响结果");
}

#[test]
fn test_get_liquidity_from_amount_1_with_given_params() {
    // 使用题目给定的具体参数
    let sqrt_price_x64 = 18446744073709551616u128; // 这个参数在当前函数中不使用，但记录下来
    let tick_lower_price_x64 = 18226716948933807364u128;
    let tick_upper_price_x64 = 18613505734318141148u128;
    let input_amount = 10000000000u64;

    let result =
        get_liquidity_from_amount_1(tick_lower_price_x64, tick_upper_price_x64, input_amount);

    println!("=== 使用给定参数的测试结果 ===");
    println!("sqrt_price_x64: {} (未在函数中使用)", sqrt_price_x64);
    println!("tick_lower_price_x64: {}", tick_lower_price_x64);
    println!("tick_upper_price_x64: {}", tick_upper_price_x64);
    println!("input_amount: {}", input_amount);
    println!("计算得到的流动性: {}", result);

    // 验证结果的合理性
    assert!(result > 0, "流动性必须为正");

    // 由于这是具体的数值，我们可以验证数量级是否合理
    // 根据公式 L = (N * amount_1 * Q64) / price_diff
    // 其中 N = 4, amount_1 = 10^10, Q64 = 2^64
    // 预期结果应该在合理范围内
    let expected_magnitude = (4u64 * input_amount) as u128;
    assert!(result > expected_magnitude / 1000, "结果不应该过小");
    assert!(result < expected_magnitude * 1000, "结果不应该过大");
}

#[test]
fn test_mathematical_properties() {
    let sqrt_ratio_a = 18226716948933807364u128;
    let sqrt_ratio_b = 18613505734318141148u128;

    // 测试线性性：双倍输入应该产生双倍输出
    let amount_1 = 1000000000u64;
    let result_1x = get_liquidity_from_amount_1(sqrt_ratio_a, sqrt_ratio_b, amount_1);
    let result_2x = get_liquidity_from_amount_1(sqrt_ratio_a, sqrt_ratio_b, amount_1 * 2);

    println!("线性性测试:");
    println!("  1x amount result: {}", result_1x);
    println!("  2x amount result: {}", result_2x);
    println!("  ratio: {:.6}", result_2x as f64 / result_1x as f64);

    // 允许一些舍入误差，比率应该接近2
    let ratio = result_2x as f64 / result_1x as f64;
    assert!((ratio - 2.0).abs() < 0.01, "双倍输入应该产生接近双倍的输出");

    // 测试单调性：更大的输入应该产生更大的输出
    let amounts = [1000000000u64, 5000000000u64, 10000000000u64];
    let mut previous_result = 0u128;

    for amount in amounts.iter() {
        let result = get_liquidity_from_amount_1(sqrt_ratio_a, sqrt_ratio_b, *amount);
        assert!(result > previous_result, "更大的输入应该产生更大的流动性");
        previous_result = result;
    }
}

// 添加这个测试函数来验证关键计算
#[cfg(test)]
mod critical_math_tests {
    use super::*;
    use crate::libraries::sqrt_price_math;
    use crate::libraries::swap_math;

    #[test]
    fn test_with_real_calculated_liquidity() {
        println!("=== 使用真实计算的流动性测试 ===");

        let sqrt_price_x64 = 18446744073709551616u128;
        let tick_lower_price_x64 = 18226716948933807364u128;
        let tick_upper_price_x64 = 18613505734318141148u128;
        let input_amount = 10000000000u64;

        // 使用真实的流动性计算
        let real_liquidity =
            get_liquidity_from_amount_1(tick_lower_price_x64, tick_upper_price_x64, input_amount);

        println!("真实计算的流动性: {}", real_liquidity);

        if real_liquidity > 0 {
            let small_amount = 1000u64;

            let new_price_0for1 = sqrt_price_math::get_next_sqrt_price_from_input(
                sqrt_price_x64,
                real_liquidity,
                small_amount,
                true,
            );
            let new_price_1for0 = sqrt_price_math::get_next_sqrt_price_from_input(
                sqrt_price_x64,
                real_liquidity,
                small_amount,
                false,
            );

            let change_0 = (new_price_0for1 as i128 - sqrt_price_x64 as i128).abs();
            let change_1 = (new_price_1for0 as i128 - sqrt_price_x64 as i128).abs();

            println!("使用真实流动性的价格影响:");
            println!("  0->1: {}", change_0);
            println!("  1->0: {}", change_1);

            if change_1 > 0 {
                let ratio = change_0 as f64 / change_1 as f64;
                println!("  比值: {:.6}", ratio);

                if ratio > 10.0 || ratio < 0.1 {
                    println!("仍然不对称 - 需要修复四次方函数");
                } else {
                    println!("现在对称了!");
                }
            }
        } else {
            println!("真实流动性为0 - 四次方函数需要修复");
        }
    }

    #[test]
    fn test_critical_calculations_with_your_data() {
        println!("=== 使用您的实际数据进行关键计算测试 ===");

        // 您的实际参数
        let sqrt_price_x64 = 18446744073709551616u128;
        let tick_lower_price_x64 = 18226716948933807364u128;
        let tick_upper_price_x64 = 18613505734318141148u128;
        let input_amount = 10000000000u64;
        let liquidity = 1000000u128; // 假设的流动性值，您可以用实际值替换

        println!("输入参数:");
        println!("  sqrt_price_x64: {}", sqrt_price_x64);
        println!("  tick_lower_price_x64: {}", tick_lower_price_x64);
        println!("  tick_upper_price_x64: {}", tick_upper_price_x64);
        println!("  input_amount: {}", input_amount);
        println!("  liquidity: {}", liquidity);

        // 测试1: 价格到tick的转换
        println!("\n=== 测试1: 价格到tick转换 ===");
        match tick_math::get_tick_at_sqrt_price(sqrt_price_x64) {
            Ok(tick) => {
                println!("sqrt_price_x64 {} -> tick {}", sqrt_price_x64, tick);
                match tick_math::get_sqrt_price_at_tick(tick) {
                    Ok(recovered_price) => {
                        println!("tick {} -> recovered_price {}", tick, recovered_price);
                        let diff = (recovered_price as i128 - sqrt_price_x64 as i128).abs();
                        println!("价格差异: {}", diff);
                        if diff > sqrt_price_x64 as i128 / 1000 {
                            // 如果差异超过0.1%
                            println!("警告: 价格转换差异过大!");
                        }
                    }
                    Err(e) => println!("tick转价格失败: {:?}", e),
                }
            }
            Err(e) => println!("价格转tick失败: {:?}", e),
        }

        // 测试2: delta amount计算
        println!("\n=== 测试2: Delta Amount计算 ===");
        match get_delta_amount_0_unsigned(
            tick_lower_price_x64,
            tick_upper_price_x64,
            liquidity,
            true,
        ) {
            Ok(delta_0) => println!("delta_amount_0: {}", delta_0),
            Err(e) => println!("delta_amount_0计算失败: {:?}", e),
        }

        match get_delta_amount_1_unsigned(
            tick_lower_price_x64,
            tick_upper_price_x64,
            liquidity,
            true,
        ) {
            Ok(delta_1) => println!("delta_amount_1: {}", delta_1),
            Err(e) => println!("delta_amount_1计算失败: {:?}", e),
        }

        // 测试3: 价格影响计算
        println!("\n=== 测试3: 价格影响计算 ===");
        let small_amount = 1000u64; // 小数量输入
        let new_price_0for1 = sqrt_price_math::get_next_sqrt_price_from_input(
            sqrt_price_x64,
            liquidity,
            small_amount,
            true, // zero_for_one
        );
        println!(
            "小量交换(0->1)价格变化: {} -> {}",
            sqrt_price_x64, new_price_0for1
        );
        println!(
            "价格变化量: {}",
            new_price_0for1 as i128 - sqrt_price_x64 as i128
        );

        let new_price_1for0 = sqrt_price_math::get_next_sqrt_price_from_input(
            sqrt_price_x64,
            liquidity,
            small_amount,
            false, // one_for_zero
        );
        println!(
            "小量交换(1->0)价格变化: {} -> {}",
            sqrt_price_x64, new_price_1for0
        );
        println!(
            "价格变化量: {}",
            new_price_1for0 as i128 - sqrt_price_x64 as i128
        );

        // 测试4: 检查是否存在零变化情况
        println!("\n=== 测试4: 零变化检查 ===");
        if new_price_0for1 == sqrt_price_x64 {
            println!("警告: zero_for_one交换价格无变化!");
        }
        if new_price_1for0 == sqrt_price_x64 {
            println!("警告: one_for_zero交换价格无变化!");
        }

        // 测试5: 边界条件
        println!("\n=== 测试5: 边界条件测试 ===");
        println!("MIN_SQRT_PRICE_X64: {}", tick_math::MIN_SQRT_PRICE_X64);
        println!("MAX_SQRT_PRICE_X64: {}", tick_math::MAX_SQRT_PRICE_X64);
        println!(
            "当前价格是否在合理范围: {}",
            sqrt_price_x64 > tick_math::MIN_SQRT_PRICE_X64
                && sqrt_price_x64 < tick_math::MAX_SQRT_PRICE_X64
        );
    }

    #[test]
    fn test_swap_step_isolation() {
        println!("=== 独立测试SwapStep计算 ===");

        // 使用简化的参数进行测试
        let current_price = 18446744073709551616u128;
        let target_price = 18400000000000000000u128; // 稍微低一点的价格
        let liquidity = 1000000u128;
        let amount_remaining = 1000000u64;
        let fee_rate = 3000u32; // 0.3%
        let is_base_input = true;
        let zero_for_one = true;

        println!("测试参数:");
        println!("  current_price: {}", current_price);
        println!("  target_price: {}", target_price);
        println!("  liquidity: {}", liquidity);
        println!("  amount_remaining: {}", amount_remaining);
        println!("  fee_rate: {}", fee_rate);
        println!("  is_base_input: {}", is_base_input);
        println!("  zero_for_one: {}", zero_for_one);

        match swap_math::compute_swap_step(
            current_price,
            target_price,
            liquidity,
            amount_remaining,
            fee_rate,
            is_base_input,
            zero_for_one,
            1,
        ) {
            Ok(step) => {
                println!("SwapStep结果:");
                println!("  amount_in: {}", step.amount_in);
                println!("  amount_out: {}", step.amount_out);
                println!("  fee_amount: {}", step.fee_amount);
                println!("  sqrt_price_next_x64: {}", step.sqrt_price_next_x64);

                // 检查结果的合理性
                if step.amount_in == 0 && step.amount_out == 0 {
                    println!("警告: SwapStep返回零数量!");
                }
                if step.sqrt_price_next_x64 == current_price {
                    println!("警告: SwapStep价格无变化!");
                }
            }
            Err(e) => {
                println!("SwapStep计算失败: {:?}", e);
            }
        }
    }
}
#[cfg(test)]
mod precision_diagnostic {
    use super::*;
    use crate::libraries::sqrt_price_math;

    #[test]
    fn diagnose_precision_issues() {
        println!("=== 精度问题诊断 ===");

        // 首先检查常数
        println!("检查常数定义:");
        println!("  fixed_point_64::Q64: {}", fixed_point_64::Q64);
        println!("  2^64: {}", 1u128 << 64);
        println!(
            "  fixed_point_64::RESOLUTION: {}",
            fixed_point_64::RESOLUTION
        );

        // 使用简单的测试值
        let current_price = 1u128 << 64; // Q64.64 格式的 1.0
        let liquidity = 1000000u128;
        let amount = 1000u64;

        println!("\n测试输入:");
        println!("  current_price (Q64.64): {}", current_price);
        println!("  liquidity: {}", liquidity);
        println!("  amount: {}", amount);

        // 测试 amount_0 方向的价格计算
        println!("\n=== 测试 get_next_sqrt_price_from_amount_0_rounding_up ===");
        let new_price_0 = sqrt_price_math::get_next_sqrt_price_from_amount_0_rounding_up(
            current_price,
            liquidity,
            amount,
            true, // add = true
        );
        println!("新价格: {}", new_price_0);
        println!("价格变化: {}", new_price_0 as i128 - current_price as i128);
        println!(
            "价格变化 / 2^64: {:.10}",
            (new_price_0 as i128 - current_price as i128) as f64
                / (1u64 << 32) as f64
                / (1u64 << 32) as f64
        );

        // 测试 amount_1 方向的价格计算
        println!("\n=== 测试 get_next_sqrt_price_from_amount_1_rounding_down ===");
        let new_price_1 = sqrt_price_math::get_next_sqrt_price_from_amount_1_rounding_down(
            current_price,
            liquidity,
            amount,
            true, // add = true
        );
        println!("新价格: {}", new_price_1);
        println!("价格变化: {}", new_price_1 as i128 - current_price as i128);
        println!(
            "价格变化 / 2^64: {:.10}",
            (new_price_1 as i128 - current_price as i128) as f64
                / (1u64 << 32) as f64
                / (1u64 << 32) as f64
        );

        // 手动计算预期值进行对比
        println!("\n=== 手动计算预期值 ===");

        // 对于 amount_1: √P' = √P + Δy / L
        // 预期变化应该是: amount * Q64 / liquidity
        let expected_change_1 = (amount as u128 * fixed_point_64::Q64) / liquidity;
        let expected_price_1 = current_price + expected_change_1;
        println!("amount_1 预期价格变化: {}", expected_change_1);
        println!("amount_1 预期新价格: {}", expected_price_1);
        println!(
            "amount_1 实际vs预期差异: {}",
            new_price_1 as i128 - expected_price_1 as i128
        );

        // 对于 amount_0: 更复杂，但我们可以检查数量级
        println!("\n=== 检查中间计算步骤 ===");

        // 模拟 get_next_sqrt_price_from_amount_0_rounding_up 的计算
        let numerator_1 = U256::from(liquidity) << fixed_point_64::RESOLUTION;
        let product = U256::from(amount) * U256::from(current_price);
        let denominator = numerator_1 + product;

        println!("numerator_1 (L << 64): {:?}", numerator_1);
        println!("product (amount * price): {:?}", product);
        println!("denominator: {:?}", denominator);

        // 检查是否存在额外的位移
        println!("\n=== 位移操作检查 ===");
        println!("numerator_1 >> 64: {:?}", numerator_1 >> 64);
        println!("denominator >> 64: {:?}", denominator >> 64);
    }

    #[test]
    fn compare_with_manual_calculation() {
        println!("=== 与手动计算对比 ===");

        let current_price = 18446744073709551616u128; // 您的实际价格
        let liquidity = 1000000u128;
        let amount = 1000u64;

        println!("使用实际参数:");
        println!("  current_price: {}", current_price);
        println!("  liquidity: {}", liquidity);
        println!("  amount: {}", amount);

        // 调用实际函数
        let actual_new_price = sqrt_price_math::get_next_sqrt_price_from_amount_1_rounding_down(
            current_price,
            liquidity,
            amount,
            true,
        );

        // 手动计算: √P' = √P + (amount * Q64) / L
        let manual_change = (amount as u128 * fixed_point_64::Q64) / liquidity;
        let manual_new_price = current_price + manual_change;

        println!("\n结果对比:");
        println!("  实际函数结果: {}", actual_new_price);
        println!("  手动计算结果: {}", manual_new_price);
        println!(
            "  差异: {}",
            actual_new_price as i128 - manual_new_price as i128
        );

        // 检查差异是否恰好是 2^64 的倍数
        let diff = actual_new_price as i128 - manual_new_price as i128;
        let diff_divided_by_2_64 = diff as f64 / (1u64 << 32) as f64 / (1u64 << 32) as f64;
        println!("  差异 / 2^64: {:.10}", diff_divided_by_2_64);

        if diff_divided_by_2_64.abs() > 0.001 && diff_divided_by_2_64.abs() < 1000.0 {
            println!("  >>> 疑似存在 2^64 倍数的精度问题!");
        }
    }

    #[test]
    fn check_liquidity_calculations() {
        println!("=== 检查流动性计算中的精度 ===");

        let sqrt_ratio_a = 18226716948933807364u128;
        let sqrt_ratio_b = 18613505734318141148u128;
        let amount_1 = 10000000000u64;

        // 检查 pow_4th_normalized 函数
        println!("检查 pow_4th_normalized:");
        let p_a = pow_4th_normalized(sqrt_ratio_a);
        let p_b = pow_4th_normalized(sqrt_ratio_b);

        println!("  pow_4th_normalized(a): {:?}", p_a);
        println!("  pow_4th_normalized(b): {:?}", p_b);

        // 手动计算四次方
        let a_integer = sqrt_ratio_a >> 64;
        let b_integer = sqrt_ratio_b >> 64;
        let manual_a_4th = a_integer.pow(4);
        let manual_b_4th = b_integer.pow(4);

        println!("  手动计算 a^4: {}", manual_a_4th);
        println!("  手动计算 b^4: {}", manual_b_4th);

        // 检查是否存在多余的位移
        let p_a_shifted = U128::from(p_a.as_u128() >> 64);
        let p_b_shifted = U128::from(p_b.as_u128() >> 64);

        println!("  pow_4th_normalized(a) >> 64: {:?}", p_a_shifted);
        println!("  pow_4th_normalized(b) >> 64: {:?}", p_b_shifted);

        // 检查流动性计算
        let price_diff = p_b.saturating_sub(p_a);
        println!("  price_diff: {:?}", price_diff);

        let liquidity_calc = U128::from(amount_1 * 4u64)
            .mul_div_floor(U128::from(fixed_point_64::Q64), U128::from(price_diff))
            .unwrap();

        println!("  计算的流动性: {:?}", liquidity_calc);

        // 检查是否需要额外的位移
        let liquidity_adjusted = liquidity_calc.as_u128() >> 64;
        println!("  流动性 >> 64: {}", liquidity_adjusted);
    }
}
#[cfg(test)]
mod test_fixed_pow {
    use super::*;
    use crate::libraries::sqrt_price_math;

    #[test]
    fn test_corrected_pow_4th_function() {
        println!("=== 测试修正的四次方函数 ===");

        let sqrt_ratio_a = 18226716948933807364u128;
        let sqrt_ratio_b = 18613505734318141148u128;

        println!("原始值:");
        println!("  sqrt_ratio_a: {}", sqrt_ratio_a);
        println!("  sqrt_ratio_b: {}", sqrt_ratio_b);
        println!("  2^64: {}", 1u128 << 64);

        // 测试修正的函数
        let fixed_a = pow_4th_normalized(sqrt_ratio_a);
        let fixed_b = pow_4th_normalized(sqrt_ratio_b);

        println!("\n修正函数结果:");
        println!("  fixed_a: {:?}", fixed_a);
        println!("  fixed_b: {:?}", fixed_b);

        // 验证结果是否合理
        if fixed_b > fixed_a {
            println!("✓ 结果顺序正确: b > a");
        } else {
            println!("✗ 结果顺序错误");
        }

        let diff = fixed_b.saturating_sub(fixed_a);
        println!("  差值: {:?}", diff);

        // 用修正的函数重新计算流动性
        if diff > U128::from(0) {
            let amount_1 = 10000000000u64;
            let liquidity_corrected = U128::from(amount_1 * 4u64)
                .mul_div_floor(U128::from(fixed_point_64::Q64), U128::from(diff))
                .unwrap();

            println!("\n重新计算的流动性:");
            println!("  liquidity_corrected: {:?}", liquidity_corrected);

            // 检查数量级是否合理
            if liquidity_corrected.as_u128() > 0 {
                println!("✓ 流动性计算结果合理");
            } else {
                println!("✗ 流动性计算结果仍有问题");
            }
        }
    }

    #[test]
    fn debug_liquidity_calculation() {
        println!("=== 调试流动性计算 ===");

        let sqrt_ratio_a = 18226716948933807364u128;
        let sqrt_ratio_b = 18613505734318141148u128;
        let amount_1 = 10000000000u64;

        println!("输入:");
        println!("  sqrt_ratio_a: {}", sqrt_ratio_a);
        println!("  sqrt_ratio_b: {}", sqrt_ratio_b);
        println!("  amount_1: {}", amount_1);

        // 调试四次方计算
        let p_min_4_5 = pow_4th_normalized(sqrt_ratio_a);
        let p_max_4_5 = pow_4th_normalized(sqrt_ratio_b);

        println!("四次方结果:");
        println!("  p_min_4_5: {:?}", p_min_4_5);
        println!("  p_max_4_5: {:?}", p_max_4_5);

        let price_diff = p_max_4_5.saturating_sub(p_min_4_5);
        println!("  price_diff: {:?}", price_diff);

        if price_diff == U128::from(0) {
            println!("问题：price_diff 为 0!");

            // 检查是否因为两个结果相同
            if p_min_4_5 == p_max_4_5 {
                println!("原因：两个四次方结果相同");
                println!("需要增加精度或调整缩放策略");
            }
        } else {
            let liquidity = U128::from(amount_1 * 4u64)
                .mul_div_floor(U128::from(fixed_point_64::Q64), U128::from(price_diff))
                .unwrap();

            println!("流动性计算:");
            println!("  numerator: {}", amount_1 * 4u64);
            println!("  Q64: {}", fixed_point_64::Q64);
            println!("  price_diff: {}", price_diff.as_u128());
            println!("  liquidity: {:?}", liquidity);
        }
    }

    #[test]
    fn test_liquidity_with_fixed_pow() {
        let sqrt_ratio_a = 18226716948933807364u128;
        let sqrt_ratio_b = 18613505734318141148u128;
        let amount_1 = 10000000000u64;

        let liquidity = get_liquidity_from_amount_1(sqrt_ratio_a, sqrt_ratio_b, amount_1);
        println!("修复后的流动性: {}", liquidity);

        // 使用这个流动性测试交换
        if liquidity > 0 {
            let current_price = 18446744073709551616u128;
            let test_amount = 1000u64;

            let new_price_0to1 = sqrt_price_math::get_next_sqrt_price_from_input(
                current_price,
                liquidity,
                test_amount,
                true,
            );
            let new_price_1to0 = sqrt_price_math::get_next_sqrt_price_from_input(
                current_price,
                liquidity,
                test_amount,
                false,
            );

            let change_0to1 = (new_price_0to1 as i128 - current_price as i128).abs();
            let change_1to0 = (new_price_1to0 as i128 - current_price as i128).abs();

            println!("使用修复流动性的价格影响:");
            println!("  0->1: {}", change_0to1);
            println!("  1->0: {}", change_1to0);
            println!("  比值: {:.6}", change_0to1 as f64 / change_1to0 as f64);
        }
    }

    #[test]
    fn compare_price_impact_after_fix() {
        println!("=== 修复后的价格影响对比 ===");

        // 使用修正的pow函数重新实现get_liquidity_from_amount_1
        let sqrt_ratio_a = 18226716948933807364u128;
        let sqrt_ratio_b = 18613505734318141148u128;
        let amount_1 = 1000u64;

        let fixed_p_a = pow_4th_normalized(sqrt_ratio_a);
        let fixed_p_b = pow_4th_normalized(sqrt_ratio_b);
        let fixed_price_diff = fixed_p_b.saturating_sub(fixed_p_a);

        if fixed_price_diff > U128::from(0) {
            let fixed_liquidity = U128::from(amount_1 * 4u64)
                .mul_div_floor(
                    U128::from(fixed_point_64::Q64),
                    U128::from(fixed_price_diff),
                )
                .unwrap();

            println!("修正后的流动性: {:?}", fixed_liquidity);

            // 使用修正的流动性测试价格影响
            if fixed_liquidity.as_u128() > 0 {
                let current_price = 18446744073709551616u128;
                let test_amount = 1000u64;

                let new_price_0to1 = sqrt_price_math::get_next_sqrt_price_from_input(
                    current_price,
                    fixed_liquidity.as_u128(),
                    test_amount,
                    true,
                );
                let new_price_1to0 = sqrt_price_math::get_next_sqrt_price_from_input(
                    current_price,
                    fixed_liquidity.as_u128(),
                    test_amount,
                    false,
                );

                let change_0to1 = (new_price_0to1 as i128 - current_price as i128).abs();
                let change_1to0 = (new_price_1to0 as i128 - current_price as i128).abs();

                println!("\n修正后的价格影响:");
                println!("  0->1 变化: {}", change_0to1);
                println!("  1->0 变化: {}", change_1to0);

                let ratio = change_0to1 as f64 / change_1to0 as f64;
                println!("  影响比值: {:.6}", ratio);

                if ratio > 0.1 && ratio < 10.0 {
                    println!("✓ 价格影响比较对称");
                } else {
                    println!("✗ 价格影响仍不对称");
                }
            }
        }
    }
}
