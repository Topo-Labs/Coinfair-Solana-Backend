use super::big_num::{U128, U256, U512};
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

/// Computes the amount of liquidity received for a given amount of token_0 and price range
/// 根据代币A数量和价格范围计算流动性
/// 公式: L = (amount_0 * price_a * price_b) / (price_b - price_a)
pub fn get_liquidity_from_amount_0(
    mut sqrt_ratio_a_x64: u128,
    mut sqrt_ratio_b_x64: u128,
    amount_0: u64,
) -> u128 {
    if sqrt_ratio_a_x64 > sqrt_ratio_b_x64 {
        std::mem::swap(&mut sqrt_ratio_a_x64, &mut sqrt_ratio_b_x64);
    }

    let diff = U128::from(sqrt_ratio_b_x64)
        .checked_sub(U128::from(sqrt_ratio_a_x64))
        .unwrap_or(U128::zero());
    if diff.is_zero() {
        return 0;
    }

    // 计算 price_a * price_b >> 32
    let product = U256::from(sqrt_ratio_a_x64) * U256::from(sqrt_ratio_b_x64);
    let shifted_product = product >> 32; // 右移避免溢出
    let numerator = shifted_product * U256::from(amount_0);
    let result = numerator / U256::from(diff.as_u128());

    if result > U256::from(u128::MAX) {
        panic!("Overflow in liquidity calculation");
    }
    result.as_u128()
}

/// Computes the amount of liquidity received for a given amount of token_1 and price range
/// 根据代币B数量和价格范围计算流动性
/// 公式: L = (N * amount_1 * 2^128) / (price_b^4 - price_a^4)
pub fn get_liquidity_from_amount_1(
    mut sqrt_ratio_a_x64: u128,
    mut sqrt_ratio_b_x64: u128,
    amount_1: u64,
) -> u128 {
    println!(
        "[get_liquidity_from_amount_1] sqrt_ratio_b_x64: {}, sqrt_ratio_b_x64: {}, amount_1: {}",
        sqrt_ratio_a_x64, sqrt_ratio_b_x64, amount_1
    );

    if sqrt_ratio_a_x64 > sqrt_ratio_b_x64 {
        std::mem::swap(&mut sqrt_ratio_a_x64, &mut sqrt_ratio_b_x64);
    }

    let pow4_a_u256 = pow4_q128(sqrt_ratio_a_x64);
    let pow4_b_u256 = pow4_q128(sqrt_ratio_b_x64);
    let pow4_diff_u256 = pow4_b_u256.checked_sub(pow4_a_u256).unwrap_or(U256::zero());

    if pow4_diff_u256.is_zero() {
        return 0;
    }

    let n_times_amount_u256 = U256::from(N * amount_1 as u128);
    let numerator_u256 = n_times_amount_u256 << 128;
    let result_u256 = numerator_u256 / pow4_diff_u256;

    if result_u256 > U256::from(u128::MAX) {
        panic!("Overflow in liquidity");
    }
    result_u256.as_u128()
}

/// Computes price^4 in Q128 format using U256 to match Move's precision
/// 计算 price^4，结果为 Q128 格式，使用 U256 匹配 Move 的精度
fn pow4_q128(price_x64: u128) -> U256 {
    let price_u256 = U256::from(price_x64);
    let pow2 = price_u256 * price_u256; // Q64
    pow2 * pow2 // Q128
}

/// Computes price^4 in Q256 format using U512 to avoid overflow
/// 计算 price^4，结果为 Q256 格式，使用 U512 避免溢出
fn pow4_q512(price_x64: u128) -> U512 {
    let reduced_price = price_x64 >> 32; // 除以 2^32，降低精度
    let price_u512 = U512::from(reduced_price);
    let pow2 = price_u512 * price_u512; // Q64
    let pow4 = pow2 * pow2; // Q128
    pow4 << 128 // Q256
}

/// Computes the maximum amount of liquidity received for a given amount of token_0, token_1, the current
/// pool prices and the prices at the tick boundaries
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

/// Computes the maximum amount of liquidity received for a given amount of token_0
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

/// Computes the maximum amount of liquidity received for a given amount of token_1
pub fn get_liquidity_from_single_amount_1(
    sqrt_ratio_x64: u128,
    mut sqrt_ratio_a_x64: u128,
    mut sqrt_ratio_b_x64: u128,
    amount_1: u64,
) -> u128 {
    println!("Debug: sqrt_ratio_x64: {}", sqrt_ratio_x64);
    println!("Debug: sqrt_ratio_a_x64: {}", sqrt_ratio_a_x64);
    println!("Debug: sqrt_ratio_b_x64: {}", sqrt_ratio_b_x64);
    println!("Debug: amount_1: {}", amount_1);

    if sqrt_ratio_a_x64 > sqrt_ratio_b_x64 {
        std::mem::swap(&mut sqrt_ratio_a_x64, &mut sqrt_ratio_b_x64);
    }

    if sqrt_ratio_x64 <= sqrt_ratio_a_x64 {
        println!("Debug: 进入分支1 - 只有token_0流动性活跃");
        return 0;
    } else if sqrt_ratio_x64 < sqrt_ratio_b_x64 {
        println!("Debug: 进入分支2 - 价格在范围内");
        return get_liquidity_from_amount_1(sqrt_ratio_a_x64, sqrt_ratio_x64, amount_1);
    } else {
        println!("Debug: 进入分支3 - 只有token_1流动性活跃");
        return get_liquidity_from_amount_1(sqrt_ratio_a_x64, sqrt_ratio_b_x64, amount_1);
    }
}

/// Gets the delta amount_0 for given liquidity and price range
pub fn get_delta_amount_0_unsigned(
    mut sqrt_ratio_a_x64: u128,
    mut sqrt_ratio_b_x64: u128,
    liquidity: u128,
    round_up: bool,
) -> Result<u64> {
    if sqrt_ratio_a_x64 > sqrt_ratio_b_x64 {
        std::mem::swap(&mut sqrt_ratio_a_x64, &mut sqrt_ratio_b_x64);
    }
    let diff = sqrt_ratio_b_x64.checked_sub(sqrt_ratio_a_x64).unwrap_or(0);
    if diff == 0 || liquidity == 0 {
        return Ok(0);
    }

    let product = U256::from(liquidity) * U256::from(diff);
    let numerator = product << 32; // 使用 <<32 匹配 Move
    let denominator = U256::from(sqrt_ratio_a_x64) * U256::from(sqrt_ratio_b_x64);

    let result = if round_up {
        (numerator + denominator - 1) / denominator
    } else {
        numerator / denominator
    };

    if result > U256::from(u64::MAX) {
        return Err(ErrorCode::MaxTokenOverflow.into());
    }
    Ok(result.as_u64())
}

/// Gets the delta amount_1 for given liquidity and price range
pub fn get_delta_amount_1_unsigned(
    mut sqrt_ratio_a_x64: u128,
    mut sqrt_ratio_b_x64: u128,
    liquidity: u128,
    round_up: bool,
) -> Result<u64> {
    if sqrt_ratio_a_x64 > sqrt_ratio_b_x64 {
        std::mem::swap(&mut sqrt_ratio_a_x64, &mut sqrt_ratio_b_x64);
    }

    let pow4_a_u512 = pow4_q512(sqrt_ratio_a_x64);
    let pow4_b_u512 = pow4_q512(sqrt_ratio_b_x64);
    let pow4_diff_u512 = pow4_b_u512.checked_sub(pow4_a_u512).unwrap_or(U512::zero());

    if pow4_diff_u512.is_zero() || liquidity == 0 {
        return Ok(0);
    }

    let product = U512::from(liquidity) * pow4_diff_u512;
    let result = if round_up {
        (product + (U512::from(N) << 128) - U512::from(1)) / (U512::from(N) << 128)
    } else {
        product / (U512::from(N) << 128)
    };

    if result > U512::from(u64::MAX) {
        return Err(ErrorCode::MaxTokenOverflow.into());
    }
    Ok(result.as_u64())
}

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

/// Computes signed delta amounts based on tick ranges
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
        )?;
    } else if tick_current < tick_upper {
        amount_0 = get_delta_amount_0_signed(
            sqrt_price_x64_current,
            tick_math::get_sqrt_price_at_tick(tick_upper)?,
            liquidity_delta,
        )?;
        amount_1 = get_delta_amount_1_signed(
            tick_math::get_sqrt_price_at_tick(tick_lower)?,
            sqrt_price_x64_current,
            liquidity_delta,
        )?;
    } else {
        amount_1 = get_delta_amount_1_signed(
            tick_math::get_sqrt_price_at_tick(tick_lower)?,
            tick_math::get_sqrt_price_at_tick(tick_upper)?,
            liquidity_delta,
        )?;
    }
    Ok((amount_0, amount_1))
}
