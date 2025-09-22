use super::big_num::{U256, U512};
use super::fixed_point_64;
use super::full_math::MulDiv;
use super::unsafe_math::UnsafeMathTrait;

/// Gets the next sqrt price (P^(1/5)) given a delta of token_0 for X^4*Y=K model
///
/// For X^4*Y=K model:
/// - L = (X^4*Y)^(1/5) = X * P where P = (4Y/X)^(1/5)
/// - When X changes: P' = P*L / (L + Δx*P)
pub fn get_next_sqrt_price_from_amount_0_rounding_up(
    sqrt_price_x64: u128,
    liquidity: u128,
    amount: u64,
    add: bool,
) -> u128 {
    if amount == 0 {
        return sqrt_price_x64;
    }

    // Use U256 for basic calculations, U512 for overflow-prone operations
    let price = U256::from(sqrt_price_x64);
    let l = U256::from(liquidity);
    let amount_256 = U256::from(amount);
    let q64 = U256::from(fixed_point_64::Q64);

    if add {
        // Adding token_0 decreases price: P' = P*L / (L + Δx*P)
        let product = amount_256.mul_div_floor(price, q64).unwrap_or(U256::zero());
        let denominator = l + product;

        if denominator.is_zero() {
            return sqrt_price_x64;
        }

        // Check if we need U512 for precision
        if price > U256::from(u64::MAX) || l > U256::from(u64::MAX) {
            return get_next_sqrt_price_from_amount_0_u512(sqrt_price_x64, liquidity, amount, add);
        }

        let numerator = l.mul_div_floor(price, U256::one()).unwrap_or(U256::zero());
        let numerator_scaled = numerator
            .mul_div_floor(q64, U256::one())
            .unwrap_or(U256::zero());

        let result = div_rounding_up_u256(numerator_scaled, denominator);
        safe_u256_to_u128(result)
    } else {
        // Removing token_0 increases price: P' = P*L / (L - Δx*P)
        let product = amount_256.mul_div_floor(price, q64).unwrap_or(U256::zero());

        if product >= l {
            return u128::MAX;
        }

        let denominator = l - product;
        let numerator = l.mul_div_floor(price, U256::one()).unwrap_or(U256::zero());
        let numerator_scaled = numerator
            .mul_div_floor(q64, U256::one())
            .unwrap_or(U256::zero());

        let result = div_rounding_up_u256(numerator_scaled, denominator);
        safe_u256_to_u128(result)
    }
}

/// High precision version using U512 for large values
fn get_next_sqrt_price_from_amount_0_u512(
    sqrt_price_x64: u128,
    liquidity: u128,
    amount: u64,
    add: bool,
) -> u128 {
    let price = U512::from(sqrt_price_x64);
    let l = U512::from(liquidity);
    let amount_512 = U512::from(amount);
    let q64 = U512::from(fixed_point_64::Q64);

    if add {
        let product = (amount_512 * price) / q64;
        let denominator = l + product;

        if denominator.is_zero() {
            return sqrt_price_x64;
        }

        let numerator = l * price * q64;
        let result = div_rounding_up_u512(numerator, denominator);
        safe_u512_to_u128(result)
    } else {
        let product = (amount_512 * price) / q64;

        if product >= l {
            return u128::MAX;
        }

        let denominator = l - product;
        let numerator = l * price * q64;
        let result = div_rounding_up_u512(numerator, denominator);
        safe_u512_to_u128(result)
    }
}

/// Gets the next sqrt price (P^(1/5)) given a delta of token_1 for X^4*Y=K model
///
/// For X^4*Y=K model:
/// - P'^5 = P^5 + 4Δy*P/L
/// - For small changes: P' ≈ P + 4Δy/(5*L*P^3)
/// - For large changes: P' = (P^5 + 4Δy*P/L)^(1/5)
pub fn get_next_sqrt_price_from_amount_1_rounding_down(
    sqrt_price_x64: u128,
    liquidity: u128,
    amount: u64,
    add: bool,
) -> u128 {
    if amount == 0 {
        return sqrt_price_x64;
    }

    // Always use U512 for token_1 calculations due to 5th power operations
    let price = U512::from(sqrt_price_x64);
    let l = U512::from(liquidity);
    let amount_512 = U512::from(amount);
    let q64 = U512::from(fixed_point_64::Q64);

    // Calculate P^3, P^4, P^5
    let price_pow2 = (price * price) / q64;
    let price_pow3 = (price_pow2 * price) / q64;
    let price_pow4 = (price_pow3 * price) / q64;
    let price_pow5 = (price_pow4 * price) / q64;

    // Calculate threshold for linear vs precise calculation
    let threshold = price_pow5 / U512::from(100); // 1% of P^5
    let delta_term = (amount_512 * U512::from(4) * price * q64) / l;

    if delta_term < threshold {
        // Use linear approximation: P' ≈ P + 4Δy/(5*L*P^3)
        let denominator = (U512::from(5) * l * price_pow3) / q64;

        if denominator.is_zero() {
            return sqrt_price_x64;
        }

        let numerator = amount_512 * U512::from(4) * q64 * q64;
        let delta_price = numerator / denominator;

        if add {
            sqrt_price_x64.saturating_add(safe_u512_to_u128(delta_price))
        } else {
            sqrt_price_x64.saturating_sub(safe_u512_to_u128(delta_price))
        }
    } else {
        // Use precise formula: P' = (P^5 + 4*Δy*P/L)^(1/5)
        let delta_p5_term = (amount_512 * U512::from(4) * price * q64 * q64 * q64 * q64) / l;

        let new_price_pow5 = if add {
            price_pow5 + delta_p5_term
        } else {
            if delta_p5_term > price_pow5 {
                U512::one() // Prevent underflow
            } else {
                price_pow5 - delta_p5_term
            }
        };

        // Calculate 5th root
        safe_u512_to_u128(nth_root_u512(new_price_pow5, 5))
    }
}

/// Gets the next sqrt price given an input amount of token_0 or token_1
pub fn get_next_sqrt_price_from_input(
    sqrt_price_x64: u128,
    liquidity: u128,
    amount_in: u64,
    zero_for_one: bool,
) -> u128 {
    assert!(sqrt_price_x64 > 0);
    assert!(liquidity > 0);

    if zero_for_one {
        get_next_sqrt_price_from_amount_0_rounding_up(sqrt_price_x64, liquidity, amount_in, true)
    } else {
        get_next_sqrt_price_from_amount_1_rounding_down(sqrt_price_x64, liquidity, amount_in, true)
    }
}

// ============================================================================
// Utility Functions
// ============================================================================

/// Calculate nth root using Newton's method with U512
fn nth_root_u512(value: U512, n: u32) -> U512 {
    if value.is_zero() {
        return U512::zero();
    }
    if n == 1 {
        return value;
    }
    if n == 0 {
        return U512::one();
    }

    // Special case for square root
    if n == 2 {
        return integer_sqrt_u512(value);
    }

    // Initial guess based on bit length
    let bit_length = 512 - value.leading_zeros();
    let mut x = if bit_length > n {
        U512::one() << (bit_length / n)
    } else {
        U512::one()
    };

    // Newton's method: x_{k+1} = ((n-1)*x_k + value/x_k^(n-1)) / n
    for iteration in 0..30 {
        // Calculate x^(n-1)
        let x_pow_n_minus_1 = pow_u512(x, n - 1);

        if x_pow_n_minus_1.is_zero() {
            break;
        }

        let term1 = x * U512::from(n - 1);
        let term2 = value / x_pow_n_minus_1;
        let numerator = term1 + term2;
        let new_x = numerator / U512::from(n);

        // Check for convergence
        if new_x == x {
            break;
        }

        let diff = if x > new_x { x - new_x } else { new_x - x };
        if diff <= U512::one() {
            break;
        }

        // Prevent infinite loops
        if iteration > 15 && diff > (x >> 10) {
            break;
        }

        x = new_x;
    }

    x
}

/// Calculate U512 integer square root
fn integer_sqrt_u512(value: U512) -> U512 {
    if value.is_zero() {
        return U512::zero();
    }

    let bit_length = 512 - value.leading_zeros();
    let mut x = U512::one() << (bit_length / 2);

    for _ in 0..25 {
        if x.is_zero() {
            break;
        }

        let new_x = (x + value / x) / U512::from(2);
        if new_x >= x {
            break;
        }
        x = new_x;
    }

    x
}

/// Calculate U512 power
fn pow_u512(base: U512, exp: u32) -> U512 {
    if exp == 0 {
        return U512::one();
    }
    if exp == 1 {
        return base;
    }

    let mut result = U512::one();
    let mut base_power = base;
    let mut remaining_exp = exp;

    while remaining_exp > 0 {
        if remaining_exp & 1 == 1 {
            result = result * base_power;
        }

        if remaining_exp > 1 {
            base_power = base_power * base_power;
        }

        remaining_exp >>= 1;
    }

    result
}

/// Division with rounding up for U256
fn div_rounding_up_u256(numerator: U256, denominator: U256) -> U256 {
    if denominator.is_zero() {
        return U256::zero();
    }
    (numerator + denominator - U256::one()) / denominator
}

/// Division with rounding up for U512
fn div_rounding_up_u512(numerator: U512, denominator: U512) -> U512 {
    if denominator.is_zero() {
        return U512::zero();
    }
    (numerator + denominator - U512::one()) / denominator
}

/// Safely convert U256 to u128
fn safe_u256_to_u128(value: U256) -> u128 {
    if value > U256::from(u128::MAX) {
        u128::MAX
    } else {
        value.as_u128()
    }
}

/// Safely convert U512 to u128
fn safe_u512_to_u128(value: U512) -> u128 {
    if value > U512::from(u128::MAX) {
        u128::MAX
    } else {
        value.as_u128()
    }
}

/// Gets the next sqrt price given an output amount of token0 or token1
pub fn get_next_sqrt_price_from_output(
    sqrt_price_x64: u128,
    liquidity: u128,
    amount_out: u64,
    zero_for_one: bool,
) -> u128 {
    assert!(sqrt_price_x64 > 0);
    assert!(liquidity > 0);

    if zero_for_one {
        get_next_sqrt_price_from_amount_1_rounding_down(
            sqrt_price_x64,
            liquidity,
            amount_out,
            false,
        )
    } else {
        get_next_sqrt_price_from_amount_0_rounding_up(sqrt_price_x64, liquidity, amount_out, false)
    }
}
