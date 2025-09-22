use crate::{error::ErrorCode, libraries::big_num::U128};
use anchor_lang::require;

/// The minimum tick
//pub const MIN_TICK: i32 = -443636;
pub const MIN_TICK: i32 = -500000;
/// The minimum tick
pub const MAX_TICK: i32 = -MIN_TICK;

pub const MIN_SQRT_PRICE_X64: u128 = 276707563012096;
pub const MAX_SQRT_PRICE_X64: u128 = 1229763610000000000000000;

// Number 64, encoded as a U128
const NUM_64: U128 = U128([64, 0]);

const BIT_PRECISION: u32 = 16;

pub fn get_sqrt_price_at_tick(tick: i32) -> Result<u128, anchor_lang::error::Error> {
    let abs_tick = tick.abs() as u32;
    require!(abs_tick <= MAX_TICK as u32, ErrorCode::TickUpperOverflow);

    // For X^4*Y=K model, we need 1.0001^(tick/5)
    // Start with 2^64 and multiply by appropriate factors
    let mut ratio = if abs_tick & 0x1 != 0 {
        // 2^64 / 1.0001^(1/5) ≈ 2^64 / 1.000020001
        U128([0xfffeb079c6c30000, 0])
    } else {
        // 2^64
        U128([0, 1])
    };

    // Each bit position represents 1.0001^(2^i/5)
    if abs_tick & 0x2 != 0 {
        ratio = (ratio * U128([0xfffd60f554470000, 0])) >> NUM_64 // 1.0001^(2/5)
    };
    if abs_tick & 0x4 != 0 {
        ratio = (ratio * U128([0xfffac1f188850000, 0])) >> NUM_64 // 1.0001^(4/5)
    };
    if abs_tick & 0x8 != 0 {
        ratio = (ratio * U128([0xfff583fe8d750000, 0])) >> NUM_64 // 1.0001^(8/5)
    };
    if abs_tick & 0x10 != 0 {
        ratio = (ratio * U128([0xffeb086b08cf0000, 0])) >> NUM_64 // 1.0001^(16/5)
    };
    if abs_tick & 0x20 != 0 {
        ratio = (ratio * U128([0xffd6128db0130000, 0])) >> NUM_64 // 1.0001^(32/5)
    };
    if abs_tick & 0x40 != 0 {
        ratio = (ratio * U128([0xffac2bf94c380000, 0])) >> NUM_64 // 1.0001^(64/5)
    };
    if abs_tick & 0x80 != 0 {
        ratio = (ratio * U128([0xff587365c5ad0000, 0])) >> NUM_64 // 1.0001^(128/5)
    };
    if abs_tick & 0x100 != 0 {
        ratio = (ratio * U128([0xfeb154744e430000, 0])) >> NUM_64 // 1.0001^(256/5)
    };
    if abs_tick & 0x200 != 0 {
        ratio = (ratio * U128([0xfd645e6cb0b70000, 0])) >> NUM_64 // 1.0001^(512/5)
    };
    if abs_tick & 0x400 != 0 {
        ratio = (ratio * U128([0xfacf89fcbd8a0000, 0])) >> NUM_64 // 1.0001^(1024/5)
    };
    if abs_tick & 0x800 != 0 {
        ratio = (ratio * U128([0xf5ba01c215810000, 0])) >> NUM_64 // 1.0001^(2048/5)
    };
    if abs_tick & 0x1000 != 0 {
        ratio = (ratio * U128([0xebdd8e840d7e0000, 0])) >> NUM_64 // 1.0001^(4096/5)
    };
    if abs_tick & 0x2000 != 0 {
        ratio = (ratio * U128([0xd9508365cef30000, 0])) >> NUM_64 // 1.0001^(8192/5)
    };
    if abs_tick & 0x4000 != 0 {
        ratio = (ratio * U128([0xb879981500030000, 0])) >> NUM_64 // 1.0001^(16384/5)
    };
    if abs_tick & 0x8000 != 0 {
        ratio = (ratio * U128([0x84ef045f4ea80000, 0])) >> NUM_64 // 1.0001^(32768/5)
    };
    if abs_tick & 0x10000 != 0 {
        ratio = (ratio * U128([0x45075bab7329b000, 0])) >> NUM_64 // 1.0001^(65536/5)
    };
    if abs_tick & 0x20000 != 0 {
        ratio = (ratio * U128([0x129cf7a090456900, 0])) >> NUM_64 // 1.0001^(131072/5)
    };
    if abs_tick & 0x40000 != 0 {
        ratio = (ratio * U128([0x15a73114f85c300, 0])) >> NUM_64 // 1.0001^(262144/5)
    };

    // Handle negative ticks by taking reciprocal
    if tick > 0 {
        ratio = U128::MAX / ratio;
    }

    Ok(ratio.as_u128())
}

/// Calculates the greatest tick value such that get_sqrt_price_at_tick(tick) <= ratio
/// For X^4*Y=K model where stored value is P^(1/5) = (4Y/X)^(1/5)
///
/// Formula: tick = 5 * log_{1.0001}(P^(1/5))
/// Which converts to: tick = 5 * log_2(P^(1/5)) / log_2(1.0001)
pub fn get_tick_at_sqrt_price(
    price_fifth_root_x64: u128,
) -> Result<i32, anchor_lang::error::Error> {
    // Validate input range
    require!(
        price_fifth_root_x64 >= MIN_SQRT_PRICE_X64 && price_fifth_root_x64 < MAX_SQRT_PRICE_X64,
        ErrorCode::SqrtPriceX64
    );

    // Determine log_2(price_fifth_root). First by calculating integer portion (msb)
    let msb: u32 = 128 - price_fifth_root_x64.leading_zeros() - 1;
    let log2p_integer_x32 = (msb as i128 - 64) << 32;

    // Get fractional value (r/2^msb), msb always > 128
    // We begin the iteration from bit 63 (0.5 in Q64.64)
    let mut bit: i128 = 0x8000_0000_0000_0000i128;
    let mut precision = 0;
    let mut log2p_fraction_x64 = 0;

    // Log2 iterative approximation for the fractional part
    let mut r = if msb >= 64 {
        price_fifth_root_x64 >> (msb - 63)
    } else {
        price_fifth_root_x64 << (63 - msb)
    };

    while bit > 0 && precision < BIT_PRECISION {
        r *= r;
        let is_r_more_than_two = r >> 127 as u32;
        r >>= 63 + is_r_more_than_two;
        log2p_fraction_x64 += bit * is_r_more_than_two as i128;
        bit >>= 1;
        precision += 1;
    }
    let log2p_fraction_x32 = log2p_fraction_x64 >> 32;
    let log2p_x32 = log2p_integer_x32 + log2p_fraction_x32;

    // Change of base rule for X^4*Y=K model:
    // tick = 5 * log_{1.0001}(P^(1/5)) = 5 * log_2(P^(1/5)) / log_2(1.0001)
    //
    // Constants:
    // log_2(1.0001) ≈ 0.0001442622910945383
    // 5 / log_2(1.0001) ≈ 34659.09186707279
    // In Q32.32 format: 34659.09186707279 * 2^32 ≈ 148,859,666,078,137
    let log_10001_fifth_root_x64 = log2p_x32 * 148859666078137i128;

    // Error margin adjustment for X^4*Y=K model
    // Accounting for the 5x factor in tick calculation
    let tick_low = ((log_10001_fifth_root_x64 - 214748364i128) >> 64) as i32;
    let tick_high = ((log_10001_fifth_root_x64 + 9085672978i128) >> 64) as i32;

    // Validate and return the correct tick
    Ok(if tick_low == tick_high {
        tick_low
    } else if get_sqrt_price_at_tick(tick_high).unwrap() <= price_fifth_root_x64 {
        tick_high
    } else {
        tick_low
    })
}
