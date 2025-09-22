//! A custom implementation of https://github.com/sdroege/rust-muldiv to support phantom overflow resistant
//! multiply-divide operations. This library uses U128 in place of u128 for u64 operations,
//! and supports U128 operations.
//!

use crate::libraries::big_num::{U1024, U128, U256, U512};

/// Trait for calculating `val * num / denom` with different rounding modes and overflow
/// protection.
///
/// Implementations of this trait have to ensure that even if the result of the multiplication does
/// not fit into the type, as long as it would fit after the division the correct result has to be
/// returned instead of `None`. `None` only should be returned if the overall result does not fit
/// into the type.
///
/// This specifically means that e.g. the `u64` implementation must, depending on the arguments, be
/// able to do 128 bit integer multiplication.
pub trait MulDiv<RHS = Self> {
    /// Output type for the methods of this trait.
    type Output;

    /// Calculates `floor(val * num / denom)`, i.e. the largest integer less than or equal to the
    /// result of the division.
    ///
    /// ## Example
    ///
    /// ```rust
    /// use libraries::full_math::MulDiv;
    ///
    /// # fn main() {
    /// let x = 3i8.mul_div_floor(4, 2);
    /// assert_eq!(x, Some(6));
    ///
    /// let x = 5i8.mul_div_floor(2, 3);
    /// assert_eq!(x, Some(3));
    ///
    /// let x = (-5i8).mul_div_floor(2, 3);
    /// assert_eq!(x, Some(-4));
    ///
    /// let x = 3i8.mul_div_floor(3, 2);
    /// assert_eq!(x, Some(4));
    ///
    /// let x = (-3i8).mul_div_floor(3, 2);
    /// assert_eq!(x, Some(-5));
    ///
    /// let x = 127i8.mul_div_floor(4, 3);
    /// assert_eq!(x, None);
    /// # }
    /// ```
    fn mul_div_floor(self, num: RHS, denom: RHS) -> Option<Self::Output>;

    /// Calculates `ceil(val * num / denom)`, i.e. the the smallest integer greater than or equal to
    /// the result of the division.
    ///
    /// ## Example
    ///
    /// ```rust
    /// use libraries::full_math::MulDiv;
    ///
    /// # fn main() {
    /// let x = 3i8.mul_div_ceil(4, 2);
    /// assert_eq!(x, Some(6));
    ///
    /// let x = 5i8.mul_div_ceil(2, 3);
    /// assert_eq!(x, Some(4));
    ///
    /// let x = (-5i8).mul_div_ceil(2, 3);
    /// assert_eq!(x, Some(-3));
    ///
    /// let x = 3i8.mul_div_ceil(3, 2);
    /// assert_eq!(x, Some(5));
    ///
    /// let x = (-3i8).mul_div_ceil(3, 2);
    /// assert_eq!(x, Some(-4));
    ///
    /// let x = (127i8).mul_div_ceil(4, 3);
    /// assert_eq!(x, None);
    /// # }
    /// ```
    fn mul_div_ceil(self, num: RHS, denom: RHS) -> Option<Self::Output>;

    /// Return u64 not out of bounds
    fn to_underflow_u64(self) -> u64;
}

pub trait Upcast256 {
    fn as_u256(self) -> U256;
}
impl Upcast256 for U128 {
    fn as_u256(self) -> U256 {
        U256([self.0[0], self.0[1], 0, 0])
    }
}

pub trait Transfer128 {
    /// Unsafe cast to U128
    /// Bits beyond the 128th position are lost
    fn as_u128(self) -> u128;
}
impl Transfer128 for U128 {
    fn as_u128(self) -> u128 {
        // self.0[0] 是低64位，self.0[1] 是高64位
        (self.0[0] as u128) | ((self.0[1] as u128) << 64)
    }
}

pub trait Downcast256 {
    /// Unsafe cast to U128
    /// Bits beyond the 128th position are lost
    fn as_u128(self) -> U128;
}
impl Downcast256 for U256 {
    fn as_u128(self) -> U128 {
        U128([self.0[0], self.0[1]])
    }
}

pub trait Upcast512 {
    fn as_u512(self) -> U512;
}
impl Upcast512 for U256 {
    fn as_u512(self) -> U512 {
        U512([self.0[0], self.0[1], self.0[2], self.0[3], 0, 0, 0, 0])
    }
}

pub trait Downcast512 {
    /// Unsafe cast to U256
    /// Bits beyond the 256th position are lost
    fn as_u256(self) -> U256;
}
impl Downcast512 for U512 {
    fn as_u256(self) -> U256 {
        U256([self.0[0], self.0[1], self.0[2], self.0[3]])
    }
}

impl MulDiv for u64 {
    type Output = u64;

    fn mul_div_floor(self, num: Self, denom: Self) -> Option<Self::Output> {
        assert_ne!(denom, 0);
        let r = (U128::from(self) * U128::from(num)) / U128::from(denom);
        if r > U128::from(u64::MAX) {
            None
        } else {
            Some(r.as_u64())
        }
    }

    fn mul_div_ceil(self, num: Self, denom: Self) -> Option<Self::Output> {
        assert_ne!(denom, 0);
        let r = (U128::from(self) * U128::from(num) + U128::from(denom - 1)) / U128::from(denom);
        if r > U128::from(u64::MAX) {
            None
        } else {
            Some(r.as_u64())
        }
    }

    fn to_underflow_u64(self) -> u64 {
        self
    }
}

impl MulDiv for U128 {
    type Output = U128;

    fn mul_div_floor(self, num: Self, denom: Self) -> Option<Self::Output> {
        assert_ne!(denom, U128::default());
        let r = ((self.as_u256()) * (num.as_u256())) / (denom.as_u256());
        if r > U128::MAX.as_u256() {
            None
        } else {
            Some(r.as_u128())
        }
    }

    fn mul_div_ceil(self, num: Self, denom: Self) -> Option<Self::Output> {
        assert_ne!(denom, U128::default());
        let r = (self.as_u256() * num.as_u256() + (denom - 1).as_u256()) / denom.as_u256();
        if r > U128::MAX.as_u256() {
            None
        } else {
            Some(r.as_u128())
        }
    }

    fn to_underflow_u64(self) -> u64 {
        if self < U128::from(u64::MAX) {
            self.as_u64()
        } else {
            0
        }
    }
}

impl MulDiv for U256 {
    type Output = U256;

    fn mul_div_floor(self, num: Self, denom: Self) -> Option<Self::Output> {
        assert_ne!(denom, U256::default());
        let r = (self.as_u512() * num.as_u512()) / denom.as_u512();
        if r > U256::MAX.as_u512() {
            None
        } else {
            Some(r.as_u256())
        }
    }

    fn mul_div_ceil(self, num: Self, denom: Self) -> Option<Self::Output> {
        assert_ne!(denom, U256::default());
        let r = (self.as_u512() * num.as_u512() + (denom - 1).as_u512()) / denom.as_u512();
        if r > U256::MAX.as_u512() {
            None
        } else {
            Some(r.as_u256())
        }
    }

    fn to_underflow_u64(self) -> u64 {
        if self < U256::from(u64::MAX) {
            self.as_u64()
        } else {
            0
        }
    }
}

/// Upcast trait for U512 to U1024  
pub trait Upcast1024 {
    fn as_u1024(self) -> U1024;
}

impl Upcast1024 for U512 {
    fn as_u1024(self) -> U1024 {
        U1024([
            self.0[0], self.0[1], self.0[2], self.0[3], self.0[4], self.0[5], self.0[6], self.0[7], 0, 0, 0, 0, 0, 0,
            0, 0,
        ])
    }
}

/// Downcast trait for U1024 to U512
pub trait Downcast1024 {
    /// Unsafe cast to U512
    /// Bits beyond the 512th position are lost
    fn as_u512(self) -> U512;
}

impl Downcast1024 for U1024 {
    fn as_u512(self) -> U512 {
        U512([
            self.0[0], self.0[1], self.0[2], self.0[3], self.0[4], self.0[5], self.0[6], self.0[7],
        ])
    }
}

/// Helper function to check if U1024 value fits in U512
fn fits_in_u512(value: U1024) -> bool {
    // Check if any of the high 8 words are non-zero
    for i in 8..16 {
        if value.0[i] != 0 {
            return false;
        }
    }
    true
}

/// Compare two U1024 values: returns true if a > b
fn _greater_than_u1024(a: U1024, b: U1024) -> bool {
    // Compare from most significant to least significant word
    for i in (0..16).rev() {
        if a.0[i] > b.0[i] {
            return true;
        } else if a.0[i] < b.0[i] {
            return false;
        }
    }
    false // They are equal
}

/// Compare two U1024 values: returns true if a >= b
fn greater_equal_u1024(a: U1024, b: U1024) -> bool {
    // Compare from most significant to least significant word
    for i in (0..16).rev() {
        if a.0[i] > b.0[i] {
            return true;
        } else if a.0[i] < b.0[i] {
            return false;
        }
    }
    true // They are equal
}

/// MulDiv implementation for U512 using manual arithmetic
impl MulDiv for U512 {
    type Output = U512;

    fn mul_div_floor(self, num: Self, denom: Self) -> Option<Self::Output> {
        assert_ne!(denom, U512::default());

        // Convert to U1024 and perform manual multiplication and division
        let self_u1024 = self.as_u1024();
        let num_u1024 = num.as_u1024();
        let denom_u1024 = denom.as_u1024();

        // Manual multiplication: self_u1024 * num_u1024
        let product = mul_u1024(self_u1024, num_u1024);

        // Manual division: product / denom_u1024
        let (quotient, _remainder) = div_u1024(product, denom_u1024);

        // Check if result fits in U512
        if !fits_in_u512(quotient) {
            None
        } else {
            Some(quotient.as_u512())
        }
    }

    fn mul_div_ceil(self, num: Self, denom: Self) -> Option<Self::Output> {
        assert_ne!(denom, U512::default());

        let self_u1024 = self.as_u1024();
        let num_u1024 = num.as_u1024();
        let denom_u1024 = denom.as_u1024();

        // Manual multiplication
        let product = mul_u1024(self_u1024, num_u1024);

        // Add (denom - 1) for ceiling division
        let denom_minus_one = sub_u1024(denom_u1024, U1024::one());
        let product_plus = add_u1024(product, denom_minus_one);

        // Manual division
        let (quotient, _remainder) = div_u1024(product_plus, denom_u1024);

        // Check if result fits in U512
        if !fits_in_u512(quotient) {
            None
        } else {
            Some(quotient.as_u512())
        }
    }

    fn to_underflow_u64(self) -> u64 {
        if self < U512::from(u64::MAX) {
            self.as_u64()
        } else {
            0
        }
    }
}

// ============================================================================
// Manual U1024 Arithmetic Operations
// ============================================================================

/// Manual multiplication for U1024
fn mul_u1024(a: U1024, b: U1024) -> U1024 {
    let mut result = U1024::zero();

    for i in 0..16 {
        if b.0[i] != 0 {
            let mut carry = 0u64;
            for j in 0..(16 - i) {
                let prod = a.0[j] as u128 * b.0[i] as u128 + result.0[i + j] as u128 + carry as u128;
                result.0[i + j] = prod as u64;
                carry = (prod >> 64) as u64;
            }
        }
    }

    result
}

/// Manual division for U1024 - returns (quotient, remainder)
fn div_u1024(dividend: U1024, divisor: U1024) -> (U1024, U1024) {
    if divisor.is_zero() {
        panic!("Division by zero");
    }

    if less_than_u1024(dividend, divisor) {
        return (U1024::zero(), dividend);
    }

    let mut quotient = U1024::zero();
    let mut remainder = dividend;

    // Simple long division algorithm
    for bit_pos in (0..1024).rev() {
        let shifted_divisor = shift_left_u1024(divisor, bit_pos);
        if greater_equal_u1024(remainder, shifted_divisor) {
            remainder = sub_u1024(remainder, shifted_divisor);
            quotient = set_bit_u1024(quotient, bit_pos);
        }

        if remainder.is_zero() {
            break;
        }
    }

    (quotient, remainder)
}

/// Manual addition for U1024
fn add_u1024(a: U1024, b: U1024) -> U1024 {
    let mut result = U1024::zero();
    let mut carry = 0u64;

    for i in 0..16 {
        let sum = a.0[i] as u128 + b.0[i] as u128 + carry as u128;
        result.0[i] = sum as u64;
        carry = (sum >> 64) as u64;
    }

    result
}

/// Manual subtraction for U1024
fn sub_u1024(a: U1024, b: U1024) -> U1024 {
    let mut result = U1024::zero();
    let mut borrow = 0u64;

    for i in 0..16 {
        let a_word = a.0[i] as u128;
        let b_word = b.0[i] as u128 + borrow as u128;

        if a_word >= b_word {
            result.0[i] = (a_word - b_word) as u64;
            borrow = 0;
        } else {
            // Borrow from the next higher word
            result.0[i] = ((a_word + (1u128 << 64)) - b_word) as u64;
            borrow = 1;
        }
    }

    result
}
/// Check if a < b for U1024
fn less_than_u1024(a: U1024, b: U1024) -> bool {
    for i in (0..16).rev() {
        if a.0[i] < b.0[i] {
            return true;
        } else if a.0[i] > b.0[i] {
            return false;
        }
    }
    false // They are equal
}

/// Left shift U1024 by n bits
fn shift_left_u1024(value: U1024, shift: usize) -> U1024 {
    if shift >= 1024 {
        return U1024::zero();
    }

    let word_shift = shift / 64;
    let bit_shift = shift % 64;
    let mut result = U1024::zero();

    for i in 0..(16 - word_shift) {
        result.0[i + word_shift] = value.0[i] << bit_shift;
        if bit_shift > 0 && i > 0 {
            result.0[i + word_shift] |= value.0[i - 1] >> (64 - bit_shift);
        }
    }

    result
}

/// Set bit at position for U1024
fn set_bit_u1024(mut value: U1024, bit_pos: usize) -> U1024 {
    if bit_pos < 1024 {
        let word_idx = bit_pos / 64;
        let bit_idx = bit_pos % 64;
        value.0[word_idx] |= 1u64 << bit_idx;
    }
    value
}

#[cfg(test)]
mod muldiv_u64_tests {
    use super::*;

    use quickcheck::{quickcheck, Arbitrary, Gen};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct NonZero(u64);

    impl Arbitrary for NonZero {
        fn arbitrary<G: Gen>(g: &mut G) -> Self {
            loop {
                let v = u64::arbitrary(g);
                if v != 0 {
                    return NonZero(v);
                }
            }
        }
    }

    quickcheck! {
        fn scale_floor(val: u64, num: u64, den: NonZero) -> bool {
            let res = val.mul_div_floor(num, den.0);

            let expected = (U128::from(val) * U128::from(num)) / U128::from(den.0);

            if expected > U128::from(u64::MAX) {
                res.is_none()
            } else {
                res == Some(expected.as_u64())
            }
        }
    }

    quickcheck! {
        fn scale_ceil(val: u64, num: u64, den: NonZero) -> bool {
            let res = val.mul_div_ceil(num, den.0);

            let mut expected = (U128::from(val) * U128::from(num)) / U128::from(den.0);
            let expected_rem = (U128::from(val) * U128::from(num)) % U128::from(den.0);

            if expected_rem != U128::default() {
                expected += U128::from(1)
            }

            if expected > U128::from(u64::MAX) {
                res.is_none()
            } else {
                res == Some(expected.as_u64())
            }
        }
    }
}

#[cfg(test)]
mod muldiv_u128_tests {
    use super::*;

    use quickcheck::{quickcheck, Arbitrary, Gen};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct NonZero(U128);

    impl Arbitrary for NonZero {
        fn arbitrary<G: Gen>(g: &mut G) -> Self {
            loop {
                let v = U128::from(u128::arbitrary(g));
                if v != U128::default() {
                    return NonZero(v);
                }
            }
        }
    }

    impl Arbitrary for U128 {
        fn arbitrary<G: Gen>(g: &mut G) -> Self {
            loop {
                let v = U128::from(u128::arbitrary(g));
                if v != U128::default() {
                    return v;
                }
            }
        }
    }

    quickcheck! {
        fn scale_floor(val: U128, num: U128, den: NonZero) -> bool {
            let res = val.mul_div_floor(num, den.0);

            let expected = ((val.as_u256()) * (num.as_u256())) / (den.0.as_u256());

            if expected > U128::MAX.as_u256() {
                res.is_none()
            } else {
                res == Some(expected.as_u128())
            }
        }
    }

    quickcheck! {
        fn scale_ceil(val: U128, num: U128, den: NonZero) -> bool {
            let res = val.mul_div_ceil(num, den.0);

            let mut expected = ((val.as_u256()) * (num.as_u256())) / (den.0.as_u256());
            let expected_rem = ((val.as_u256()) * (num.as_u256())) % (den.0.as_u256());

            if expected_rem != U256::default() {
                expected += U256::from(1)
            }

            if expected > U128::MAX.as_u256() {
                res.is_none()
            } else {
                res == Some(expected.as_u128())
            }
        }
    }
}
