use super::big_num::U128;
use super::big_num::U256;
use super::fixed_point_64;
use super::full_math::MulDiv;
use super::tick_math;
use super::unsafe_math::UnsafeMathTrait;
use crate::error::ErrorCode;
use anchor_lang::prelude::*;

// 常数 N = 4 (对应 X^4Y = K 模型)
const N: u128 = 4;

/// Add a signed liquidity delta to liquidity and revert if it overflows or underflows
///
/// # Arguments
///
/// * `x` - The liquidity (L) before change
/// * `y` - The delta (ΔL) by which liquidity should be changed
///
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

/// 辅助函数：计算4次方 (Q256格式) - 添加溢出检查
/// 输入: price_x64 (Q64格式)
/// 输出: price^4 (Q256格式，为了避免溢出)
// fn pow4_q512(price_x64: u128) -> U512 {
//     // 大幅降低精度避免溢出
//     // 将 Q64 格式右移 32 位，变成 Q32 格式
//     let reduced_price = price_x64 >> 32; // 除以 2^32

//     let price_u512 = U512::from(reduced_price);
//     let pow2 = price_u512 * price_u512; // Q64
//     let pow4 = pow2 * pow2; // Q128

//     // 调整精度：原本 4次方后应该是 Q256，现在是 Q128
//     // 需要左移 128 位来补偿
//     pow4 << 128 // 现在是 Q256 等效
// }
fn pow4_safe(price_x64: u128) -> U256 {
    let reduced_price = price_x64 >> 16; // 除以 2^16
    let price_u256 = U256::from(reduced_price);
    let pow2 = price_u256 * price_u256; // Q40
    pow2 * pow2 // Q80
}

/// Computes the amount of liquidity received for a given amount of token_0 and price range
/// 根据代币A数量和价格范围计算流动性
/// 公式: L = (amount_0 * price_a * price_b) / (price_b - price_a)
pub fn get_liquidity_from_amount_0(
    mut sqrt_ratio_a_x64: u128,
    mut sqrt_ratio_b_x64: u128,
    amount_0: u64,
) -> u128 {
    // sqrt_ratio_a_x64 should hold the smaller value
    if sqrt_ratio_a_x64 > sqrt_ratio_b_x64 {
        std::mem::swap(&mut sqrt_ratio_a_x64, &mut sqrt_ratio_b_x64);
    };

    // 计算 price_a * price_b / Q64
    let intermediate = U128::from(sqrt_ratio_a_x64)
        .mul_div_floor(
            U128::from(sqrt_ratio_b_x64),
            U128::from(fixed_point_64::Q64),
        )
        .unwrap();

    // 计算 amount_0 * intermediate / (price_b - price_a)
    U128::from(amount_0)
        .mul_div_floor(
            intermediate,
            U128::from(sqrt_ratio_b_x64 - sqrt_ratio_a_x64),
        )
        .unwrap()
        .as_u128()
}

/// Computes the amount of liquidity received for a given amount of token_1 and price range
/// 根据代币B数量和价格范围计算流动性  
/// 公式: L = (N * amount_1 * 2^128) / (price_b^4 - price_a^4)
/// Computes the amount of liquidity received for a given amount of token_1 and price range
/// 根据代币B数量和价格范围计算流动性  
/// 公式: L = (N * amount_1 * 2^128) / (price_b^4 - price_a^4)
/// 使用改进算法避免溢出
pub fn get_liquidity_from_amount_1(
    mut sqrt_ratio_a_x64: u128,
    mut sqrt_ratio_b_x64: u128,
    amount_1: u64,
) -> u128 {
    println!("get_liquidity_from_amount_1 调用 (改进算法):");
    println!("  sqrt_ratio_a_x64: {}", sqrt_ratio_a_x64);
    println!("  sqrt_ratio_b_x64: {}", sqrt_ratio_b_x64);
    println!("  amount_1: {}", amount_1);

    // sqrt_ratio_a_x64 should hold the smaller value
    if sqrt_ratio_a_x64 > sqrt_ratio_b_x64 {
        std::mem::swap(&mut sqrt_ratio_a_x64, &mut sqrt_ratio_b_x64);
    };

    // 使用安全的4次方计算（降低精度版本）
    let pow4_a = pow4_safe(sqrt_ratio_a_x64);
    let pow4_b = pow4_safe(sqrt_ratio_b_x64);

    if pow4_b <= pow4_a {
        println!("  错误: pow4_b <= pow4_a");
        return 0;
    }

    let pow4_diff = pow4_b - pow4_a;

    println!("  pow4_diff 是否为零: {}", pow4_diff.is_zero());

    if pow4_diff.is_zero() {
        println!("  错误: 分母为零!");
        return 0;
    }

    // 在 U256 空间中计算分子
    // 由于精度降低了，我们需要相应调整分子
    let numerator = U256::from(N * amount_1 as u128) * (U256::from(1u128) << 128); // 调整精度

    println!("  numerator 是否为零: {}", numerator.is_zero());

    // 在 U256 中进行除法
    let result = numerator / pow4_diff;

    println!("  result: {:?}", result);

    println!("  numerator 大小级别: 约2^{}", numerator.leading_zeros());
    println!("  pow4_diff 大小级别: 约2^{}", pow4_diff.leading_zeros());

    // 转换为 u128
    match u128::try_from(result) {
        Ok(liquidity) => {
            println!("  成功转换为 u128: {}", liquidity);
            liquidity
        }
        Err(_) => {
            println!("  结果太大，使用截断处理");
            // 如果结果太大，取u128的最大值或者进一步右移
            (result >> 64).as_u128().min(u128::MAX / 1000) // 保守处理
        }
    }
}

// pub fn get_liquidity_from_amount_1(
//     mut sqrt_ratio_a_x64: u128,
//     mut sqrt_ratio_b_x64: u128,
//     amount_1: u64,
// ) -> u128 {
//     println!("get_liquidity_from_amount_1 调用:");
//     println!("  sqrt_ratio_a_x64: {}", sqrt_ratio_a_x64);
//     println!("  sqrt_ratio_b_x64: {}", sqrt_ratio_b_x64);
//     println!("  amount_1: {}", amount_1);

//     // sqrt_ratio_a_x64 should hold the smaller value
//     if sqrt_ratio_a_x64 > sqrt_ratio_b_x64 {
//         std::mem::swap(&mut sqrt_ratio_a_x64, &mut sqrt_ratio_b_x64);
//     };

//     // 计算4次方
//     let pow4_a_u512 = pow4_q512(sqrt_ratio_a_x64);
//     let pow4_b_u512 = pow4_q512(sqrt_ratio_b_x64);
//     let pow4_diff_u512 = pow4_b_u512 - pow4_a_u512;

//     println!("  pow4_diff_u512: {:?}", pow4_diff_u512.as_u128()); // 如果太大会panic，先试试

//     let n_times_amount_u512 = U512::from(N * amount_1 as u128);
//     let numerator_u512 = n_times_amount_u512 << 128;

//     println!("  numerator_u512: {:?}", numerator_u512.as_u128()); // 如果太大会panic

//     let result_u512 = numerator_u512 / pow4_diff_u512;
//     let result = result_u512.as_u128();

//     println!("  最终结果: {}", result);
//     result
// }
// pub fn get_liquidity_from_amount_1(
//     mut sqrt_ratio_a_x64: u128,
//     mut sqrt_ratio_b_x64: u128,
//     amount_1: u64,
// ) -> u128 {
//     // sqrt_ratio_a_x64 should hold the smaller value
//     if sqrt_ratio_a_x64 > sqrt_ratio_b_x64 {
//         std::mem::swap(&mut sqrt_ratio_a_x64, &mut sqrt_ratio_b_x64);
//     };

//     // 使用 U512 进行 4次方计算
//     let pow4_a_u512 = pow4_q512(sqrt_ratio_a_x64);
//     let pow4_b_u512 = pow4_q512(sqrt_ratio_b_x64);
//     let pow4_diff_u512 = pow4_b_u512 - pow4_a_u512;

//     // 在 U512 空间中计算分子：N * amount_1 * 2^128
//     let n_times_amount_u512 = U512::from(N * amount_1 as u128);
//     let numerator_u512 = n_times_amount_u512 << 128;

//     // 在 U512 中进行除法
//     let result_u512 = numerator_u512 / pow4_diff_u512;

//     // 转换为 u128（应该安全，因为流动性通常不会超过 u128 范围）
//     result_u512.as_u128()
// }

/// Computes the maximum amount of liquidity received for a given amount of token_0, token_1, the current
/// pool prices and the prices at the tick boundaries
pub fn get_liquidity_from_amounts(
    sqrt_ratio_x64: u128,
    mut sqrt_ratio_a_x64: u128,
    mut sqrt_ratio_b_x64: u128,
    amount_0: u64,
    amount_1: u64,
) -> u128 {
    // sqrt_ratio_a_x64 should hold the smaller value
    if sqrt_ratio_a_x64 > sqrt_ratio_b_x64 {
        std::mem::swap(&mut sqrt_ratio_a_x64, &mut sqrt_ratio_b_x64);
    };

    if sqrt_ratio_x64 <= sqrt_ratio_a_x64 {
        // If P ≤ P_lower, only token_0 liquidity is active
        get_liquidity_from_amount_0(sqrt_ratio_a_x64, sqrt_ratio_b_x64, amount_0)
    } else if sqrt_ratio_x64 < sqrt_ratio_b_x64 {
        // If P_lower < P < P_upper, active liquidity is the minimum of the liquidity provided
        // by token_0 and token_1
        u128::min(
            get_liquidity_from_amount_0(sqrt_ratio_x64, sqrt_ratio_b_x64, amount_0),
            get_liquidity_from_amount_1(sqrt_ratio_a_x64, sqrt_ratio_x64, amount_1),
        )
    } else {
        // If P ≥ P_upper, only token_1 liquidity is active
        get_liquidity_from_amount_1(sqrt_ratio_a_x64, sqrt_ratio_b_x64, amount_1)
    }
}

/// Computes the maximum amount of liquidity received for a given amount of token_0, token_1, the current
/// pool prices and the prices at the tick boundaries
pub fn get_liquidity_from_single_amount_0(
    sqrt_ratio_x64: u128,
    mut sqrt_ratio_a_x64: u128,
    mut sqrt_ratio_b_x64: u128,
    amount_0: u64,
) -> u128 {
    // sqrt_ratio_a_x64 should hold the smaller value
    if sqrt_ratio_a_x64 > sqrt_ratio_b_x64 {
        std::mem::swap(&mut sqrt_ratio_a_x64, &mut sqrt_ratio_b_x64);
    };

    if sqrt_ratio_x64 <= sqrt_ratio_a_x64 {
        // If P ≤ P_lower, only token_0 liquidity is active
        get_liquidity_from_amount_0(sqrt_ratio_a_x64, sqrt_ratio_b_x64, amount_0)
    } else if sqrt_ratio_x64 < sqrt_ratio_b_x64 {
        // If P_lower < P < P_upper, active liquidity is the minimum of the liquidity provided
        // by token_0 and token_1
        get_liquidity_from_amount_0(sqrt_ratio_x64, sqrt_ratio_b_x64, amount_0)
    } else {
        // If P ≥ P_upper, only token_1 liquidity is active
        0
    }
}

/// Computes the maximum amount of liquidity received for a given amount of token_0, token_1, the current
/// pool prices and the prices at the tick boundaries
pub fn get_liquidity_from_single_amount_1(
    sqrt_ratio_x64: u128,
    mut sqrt_ratio_a_x64: u128,
    mut sqrt_ratio_b_x64: u128,
    amount_1: u64,
) -> u128 {
    // 添加调试信息
    println!("Debug: sqrt_ratio_x64: {}", sqrt_ratio_x64);
    println!("Debug: sqrt_ratio_a_x64: {}", sqrt_ratio_a_x64);
    println!("Debug: sqrt_ratio_b_x64: {}", sqrt_ratio_b_x64);
    println!("Debug: amount_1: {}", amount_1);

    // sqrt_ratio_a_x64 should hold the smaller value
    if sqrt_ratio_a_x64 > sqrt_ratio_b_x64 {
        std::mem::swap(&mut sqrt_ratio_a_x64, &mut sqrt_ratio_b_x64);
    };

    if sqrt_ratio_x64 <= sqrt_ratio_a_x64 {
        // If P ≤ P_lower, only token_0 liquidity is active
        println!("Debug: 进入分支1 - 只有token_0流动性活跃");
        return 0;
    } else if sqrt_ratio_x64 < sqrt_ratio_b_x64 {
        // If P_lower < P < P_upper, active liquidity is the minimum of the liquidity provided
        // by token_0 and token_1
        println!("Debug: 进入分支2 - 价格在范围内");
        return get_liquidity_from_amount_1(sqrt_ratio_a_x64, sqrt_ratio_x64, amount_1);
    } else {
        // If P ≥ P_upper, only token_1 liquidity is active
        println!("Debug: 进入分支3 - 只有token_1流动性活跃");
        return get_liquidity_from_amount_1(sqrt_ratio_a_x64, sqrt_ratio_b_x64, amount_1);
    }
}
// pub fn get_liquidity_from_single_amount_1(
//     sqrt_ratio_x64: u128,
//     mut sqrt_ratio_a_x64: u128,
//     mut sqrt_ratio_b_x64: u128,
//     amount_1: u64,
// ) -> u128 {
//     // sqrt_ratio_a_x64 should hold the smaller value
//     if sqrt_ratio_a_x64 > sqrt_ratio_b_x64 {
//         std::mem::swap(&mut sqrt_ratio_a_x64, &mut sqrt_ratio_b_x64);
//     };

//     if sqrt_ratio_x64 <= sqrt_ratio_a_x64 {
//         // If P ≤ P_lower, only token_0 liquidity is active
//         0
//     } else if sqrt_ratio_x64 < sqrt_ratio_b_x64 {
//         // If P_lower < P < P_upper, active liquidity is the minimum of the liquidity provided
//         // by token_0 and token_1
//         get_liquidity_from_amount_1(sqrt_ratio_a_x64, sqrt_ratio_x64, amount_1)
//     } else {
//         // If P ≥ P_upper, only token_1 liquidity is active
//         get_liquidity_from_amount_1(sqrt_ratio_a_x64, sqrt_ratio_b_x64, amount_1)
//     }
// }

/// Gets the delta amount_0 for given liquidity and price range
///
/// # Formula
///
/// * `Δx = L * (1 / √P_lower - 1 / √P_upper)`
/// * i.e. `L * (√P_upper - √P_lower) / (√P_upper * √P_lower)`
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

///// Coinfair: Gets the delta amount_0 for given liquidity and price range
//pub fn get_delta_amount_0_unsigned(
//    mut sqrt_ratio_a_x64: u128,
//    mut sqrt_ratio_b_x64: u128,
//    liquidity: u128,
//    round_up: bool,
//) -> Result<u64> {
//    if sqrt_ratio_a_x64 > sqrt_ratio_b_x64 {
//        std::mem::swap(&mut sqrt_ratio_a_x64, &mut sqrt_ratio_b_x64);
//    }
//
//    // L = x² ⇒ Δx = √L * (sqrt_b - sqrt_a)
//    let sqrt_l = U256::from(liquidity).integer_sqrt();
//    let delta_x = U256::from(sqrt_ratio_b_x64 - sqrt_ratio_a_x64);
//    let amount = sqrt_l
//        .mul_div_floor(delta_x, U256::from(fixed_point_64::Q64))
//        .unwrap();
//
//    if amount > U256::from(u64::MAX) {
//        return Err(ErrorCode::MaxTokenOverflow.into());
//    }
//
//    Ok(amount.as_u64())
//}

/// Gets the delta amount_1 for given liquidity and price range
/// 基于 X^4Y=K 模型的修正版本
/// 公式: Δy = L * (price_upper^4 - price_lower^4) / (N * 2^128)
pub fn get_delta_amount_1_unsigned(
    mut sqrt_ratio_a_x64: u128,
    mut sqrt_ratio_b_x64: u128,
    liquidity: u128,
    round_up: bool,
) -> Result<u64> {
    if sqrt_ratio_a_x64 > sqrt_ratio_b_x64 {
        std::mem::swap(&mut sqrt_ratio_a_x64, &mut sqrt_ratio_b_x64);
    };

    // 使用安全的4次方计算
    let pow4_a = pow4_safe(sqrt_ratio_a_x64);
    let pow4_b = pow4_safe(sqrt_ratio_b_x64);
    let pow4_diff = pow4_b - pow4_a;

    // 在 U256 空间中计算
    let product = U256::from(liquidity) * pow4_diff;
    let divisor = U256::from(N) << 128; // 调整精度匹配

    let result = if round_up {
        (product + divisor - U256::from(1u128)) / divisor
    } else {
        product / divisor
    };

    if result > U256::from(u64::MAX) {
        return Err(ErrorCode::MaxTokenOverflow.into());
    }

    Ok(result.as_u64())
}

///// Coinfair: Gets the delta amount_1 for given liquidity and price range
//pub fn get_delta_amount_1_unsigned(
//    mut sqrt_ratio_a_x64: u128,
//    mut sqrt_ratio_b_x64: u128,
//    liquidity: u128,
//    round_up: bool,
//) -> Result<u64> {
//    if sqrt_ratio_a_x64 > sqrt_ratio_b_x64 {
//        std::mem::swap(&mut sqrt_ratio_a_x64, &mut sqrt_ratio_b_x64);
//    }
//
//    // P = y / x⁴ ⇒ y = P * L²
//    // Δy = L² * (P_b - P_a)
//    let l_squared = U256::from(liquidity).pow(2);
//    let p_a = U256::from(sqrt_ratio_a_x64).pow(4);
//    let p_b = U256::from(sqrt_ratio_b_x64).pow(4);
//
//    let delta_p = p_b.checked_sub(p_a).ok_or(ErrorCode::MathOverflow)?;
//    let amount = l_squared
//        .mul_div_floor(delta_p, U256::from(fixed_point_64::Q64).pow(4))
//        .unwrap();
//
//    if amount > U256::from(u64::MAX) {
//        return Err(ErrorCode::MaxTokenOverflow.into());
//    }
//
//    Ok(amount.as_u64())
//}

/// Helper function to get signed delta amount_0 for given liquidity and price range
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

/// Helper function to get signed delta amount_1 for given liquidity and price range
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
