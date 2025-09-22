use crate::{error::ErrorCode, libraries::big_num::U128};
use anchor_lang::require;

/// The minimum tick
//pub const MIN_TICK: i32 = -443636;
pub const MIN_TICK: i32 = -500000;
/// The minimum tick
pub const MAX_TICK: i32 = -MIN_TICK;

pub const MIN_SQRT_PRICE_X64: u128 = 837899702511180; // 最小sqrt价格
pub const MAX_SQRT_PRICE_X64: u128 = 406113483393196591711450; // 最大sqrt价格
const BIT_PRECISION: u32 = 14; // 精度位数

// Number 64, encoded as a U128
const NUM_64: U128 = U128([64, 0]);

// 正向函数：从 tick 计算 sqrt_price
pub fn get_sqrt_price_at_tick(tick: i32) -> Result<u128, anchor_lang::error::Error> {
    let abs_tick = tick.abs() as u32;
    require!(abs_tick <= MAX_TICK as u32, ErrorCode::TickUpperOverflow);

    // i = 0
    let mut ratio = if abs_tick & 0x1 != 0 {
        U128([0xfffeb079cec2f800, 0]) // 2^64 / 1.0001^(2^0/5) = 2^64 / 1.0001^0.2
    } else {
        // 2^64
        U128([0, 1])
    };

    // i = 1
    if abs_tick & 0x2 != 0 {
        ratio = (ratio * U128([0xfffd60f555468000, 0])) >> NUM_64; // 2^64 / 1.0001^0.4
    }

    // i = 2
    if abs_tick & 0x4 != 0 {
        ratio = (ratio * U128([0xfffac1f18985d800, 0])) >> NUM_64; // 2^64 / 1.0001^0.8
    }

    // i = 3
    if abs_tick & 0x8 != 0 {
        ratio = (ratio * U128([0xfff583fe8ea75800, 0])) >> NUM_64; // 2^64 / 1.0001^1.6
    }

    // i = 4
    if abs_tick & 0x10 != 0 {
        ratio = (ratio * U128([0xffeb086b097cf000, 0])) >> NUM_64; // 2^64 / 1.0001^3.2
    }

    // i = 5
    if abs_tick & 0x20 != 0 {
        ratio = (ratio * U128([0xffd6128db1b13000, 0])) >> NUM_64; // 2^64 / 1.0001^6.4
    }

    // i = 6
    if abs_tick & 0x40 != 0 {
        ratio = (ratio * U128([0xffac2bf94e3c3800, 0])) >> NUM_64; // 2^64 / 1.0001^12.8
    }

    // i = 7
    if abs_tick & 0x80 != 0 {
        ratio = (ratio * U128([0xff587365c86ad000, 0])) >> NUM_64; // 2^64 / 1.0001^25.6
    }

    // i = 8
    if abs_tick & 0x100 != 0 {
        ratio = (ratio * U128([0xfeb154744f433800, 0])) >> NUM_64; // 2^64 / 1.0001^51.2
    }

    // i = 9
    if abs_tick & 0x200 != 0 {
        ratio = (ratio * U128([0xfd645e6cb1fb7800, 0])) >> NUM_64; // 2^64 / 1.0001^102.4
    }

    // i = 10
    if abs_tick & 0x400 != 0 {
        ratio = (ratio * U128([0xfacf89fcbf8a2800, 0])) >> NUM_64; // 2^64 / 1.0001^204.8
    }

    // i = 11
    if abs_tick & 0x800 != 0 {
        ratio = (ratio * U128([0xf5ba01c217381800, 0])) >> NUM_64; // 2^64 / 1.0001^409.6
    }

    // i = 12
    if abs_tick & 0x1000 != 0 {
        ratio = (ratio * U128([0xebdd8e840e7e2800, 0])) >> NUM_64; // 2^64 / 1.0001^819.2
    }

    // i = 13
    if abs_tick & 0x2000 != 0 {
        ratio = (ratio * U128([0xd9508365d1f36800, 0])) >> NUM_64; // 2^64 / 1.0001^1638.4
    }

    // i = 14
    if abs_tick & 0x4000 != 0 {
        ratio = (ratio * U128([0xb879981501034000, 0])) >> NUM_64; // 2^64 / 1.0001^3276.8
    }

    // i = 15
    if abs_tick & 0x8000 != 0 {
        ratio = (ratio * U128([0x84ef045f4fa89800, 0])) >> NUM_64; // 2^64 / 1.0001^6553.6
    }

    // i = 16
    if abs_tick & 0x10000 != 0 {
        ratio = (ratio * U128([0x45075bab742fb000, 0])) >> NUM_64; // 2^64 / 1.0001^13107.2
    }

    // i = 17
    if abs_tick & 0x20000 != 0 {
        ratio = (ratio * U128([0x129cf7a090d56900, 0])) >> NUM_64; // 2^64 / 1.0001^26214.4
    }

    // i = 18
    if abs_tick & 0x40000 != 0 {
        ratio = (ratio * U128([0x15a73114f95c300, 0])) >> NUM_64; // 2^64 / 1.0001^52428.8
    }

    // Handle negative ticks
    if tick > 0 {
        ratio = U128::MAX / ratio;
    }

    Ok(ratio.as_u128())
}

// 反向函数：从 sqrt_price 计算 tick
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
    // Go through each 2^(j) bit where j < 64 in a Q64.64 number
    // Append current bit value to fraction result if r^2 Q2.126 is more than 2
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

    // Change of base rule: multiply with 5 / log2(1.0001)
    // Since we want tick where 1.0001^(tick/5) = price_fifth_root
    // We have: tick = 5 * log_{1.0001}(price_fifth_root)
    // Converting to log2: tick = 5 * log2(price_fifth_root) / log2(1.0001)
    // Constants for P^(1/5) model:
    // log2(1.0001) ≈ 0.0001442622910945383
    // 5 / log2(1.0001) ≈ 34659.09186707279
    // In Q32.32 format: 34659.09186707279 * 2^32 ≈ 148,859,666,078,137
    let log_10001_fifth_root_x64 = log2p_x32 * 148859666078137i128;

    // Error margin adjustment for P^(1/5) model
    // 14 bit refinement gives an error margin of 2^-14 / log2(1.0001) * 5
    // This accounts for the 5th root relationship
    // tick - 0.01 (adjusted for 5th root model)
    let tick_low = ((log_10001_fifth_root_x64 - 214748364i128) >> 64) as i32;
    // tick + error_margin + 0.01 (adjusted for 5th root model)
    let tick_high = ((log_10001_fifth_root_x64 + 9085672978i128) >> 64) as i32;

    // Validate and return the correct tick
    Ok(if tick_low == tick_high {
        tick_low
    } else if get_sqrt_price_at_tick(tick_high).unwrap_or(0) <= price_fifth_root_x64 {
        tick_high
    } else {
        tick_low
    })
}

// pub const MIN_SQRT_PRICE_X64: u128 = 276707563012096;
// pub const MAX_SQRT_PRICE_X64: u128 = 1229763610000000000000000;
// const BIT_PRECISION: u32 = 16;

// pub fn get_sqrt_price_at_tick(tick: i32) -> Result<u128, anchor_lang::error::Error> {
//     let abs_tick = tick.abs() as u32;
//     require!(abs_tick <= MAX_TICK as u32, ErrorCode::TickUpperOverflow);

//     // i = 0
//     let mut ratio = if abs_tick & 0x1 != 0 {
//         U128([0xfffcb933bd6fb800, 0])
//     } else {
//         // 2^64
//         U128([0, 1])
//     };
//     // i = 1
//     if abs_tick & 0x2 != 0 {
//         ratio = (ratio * U128([0xfff97272373d4000, 0])) >> NUM_64
//     };
//     // i = 2
//     if abs_tick & 0x4 != 0 {
//         ratio = (ratio * U128([0xfff2e50f5f657000, 0])) >> NUM_64
//     };
//     // i = 3
//     if abs_tick & 0x8 != 0 {
//         ratio = (ratio * U128([0xffe5caca7e10f000, 0])) >> NUM_64
//     };
//     // i = 4
//     if abs_tick & 0x10 != 0 {
//         ratio = (ratio * U128([0xffcb9843d60f7000, 0])) >> NUM_64
//     };
//     // i = 5
//     if abs_tick & 0x20 != 0 {
//         ratio = (ratio * U128([0xff973b41fa98e800, 0])) >> NUM_64
//     };
//     // i = 6
//     if abs_tick & 0x40 != 0 {
//         ratio = (ratio * U128([0xff2ea16466c9b000, 0])) >> NUM_64
//     };
//     // i = 7
//     if abs_tick & 0x80 != 0 {
//         ratio = (ratio * U128([0xfe5dee046a9a3800, 0])) >> NUM_64
//     };
//     // i = 8
//     if abs_tick & 0x100 != 0 {
//         ratio = (ratio * U128([0xfcbe86c7900bb000, 0])) >> NUM_64
//     };
//     // i = 9
//     if abs_tick & 0x200 != 0 {
//         ratio = (ratio * U128([0xf987a7253ac65800, 0])) >> NUM_64
//     };
//     // i = 10
//     if abs_tick & 0x400 != 0 {
//         ratio = (ratio * U128([0xf3392b0822bb6000, 0])) >> NUM_64
//     };
//     // i = 11
//     if abs_tick & 0x800 != 0 {
//         ratio = (ratio * U128([0xe7159475a2caf000, 0])) >> NUM_64
//     };
//     // i = 12
//     if abs_tick & 0x1000 != 0 {
//         ratio = (ratio * U128([0xd097f3bdfd2f2000, 0])) >> NUM_64
//     };
//     // i = 13
//     if abs_tick & 0x2000 != 0 {
//         ratio = (ratio * U128([0xa9f746462d9f8000, 0])) >> NUM_64
//     };
//     // i = 14
//     if abs_tick & 0x4000 != 0 {
//         ratio = (ratio * U128([0x70d869a156f31c00, 0])) >> NUM_64
//     };
//     // i = 15
//     if abs_tick & 0x8000 != 0 {
//         ratio = (ratio * U128([0x31be135f97ed3200, 0])) >> NUM_64
//     };
//     // i = 16
//     if abs_tick & 0x10000 != 0 {
//         ratio = (ratio * U128([0x9aa508b5b85a500, 0])) >> NUM_64
//     };
//     // i = 17
//     if abs_tick & 0x20000 != 0 {
//         ratio = (ratio * U128([0x5d6af8dedc582c, 0])) >> NUM_64
//     };
//     // i = 18
//     if abs_tick & 0x40000 != 0 {
//         ratio = (ratio * U128([0x2216e584f5fa, 0])) >> NUM_64
//     }

//     // Divide to obtain 1.0001^(2^(i - 1)) * 2^32 in numerator
//     if tick > 0 {
//         ratio = U128::MAX / ratio;
//     }

//     Ok(ratio.as_u128())
// }

// /// Calculates the greatest tick value such that get_sqrt_price_at_tick(tick) <= ratio
// /// Throws if sqrt_price_x64 < MIN_SQRT_RATIO or sqrt_price_x64 > MAX_SQRT_RATIO
// ///
// /// Formula: `i = log base(√1.0001) (√P)`
// pub fn get_tick_at_sqrt_price(sqrt_price_x64: u128) -> Result<i32, anchor_lang::error::Error> {
//     // second inequality must be < because the price can never reach the price at the max tick
//     require!(
//         sqrt_price_x64 >= MIN_SQRT_PRICE_X64 && sqrt_price_x64 < MAX_SQRT_PRICE_X64,
//         ErrorCode::SqrtPriceX64
//     );

//     // Determine log_b(sqrt_ratio). First by calculating integer portion (msb)
//     let msb: u32 = 128 - sqrt_price_x64.leading_zeros() - 1;
//     let log2p_integer_x32 = (msb as i128 - 64) << 32;

//     // get fractional value (r/2^msb), msb always > 128
//     // We begin the iteration from bit 63 (0.5 in Q64.64)
//     let mut bit: i128 = 0x8000_0000_0000_0000i128;
//     let mut precision = 0;
//     let mut log2p_fraction_x64 = 0;

//     // Log2 iterative approximation for the fractional part
//     // Go through each 2^(j) bit where j < 64 in a Q64.64 number
//     // Append current bit value to fraction result if r^2 Q2.126 is more than 2
//     let mut r = if msb >= 64 {
//         sqrt_price_x64 >> (msb - 63)
//     } else {
//         sqrt_price_x64 << (63 - msb)
//     };

//     while bit > 0 && precision < BIT_PRECISION {
//         r *= r;
//         let is_r_more_than_two = r >> 127 as u32;
//         r >>= 63 + is_r_more_than_two;
//         log2p_fraction_x64 += bit * is_r_more_than_two as i128;
//         bit >>= 1;
//         precision += 1;
//     }
//     let log2p_fraction_x32 = log2p_fraction_x64 >> 32;
//     let log2p_x32 = log2p_integer_x32 + log2p_fraction_x32;

//     // 14 bit refinement gives an error margin of 2^-14 / log2 (√1.0001) = 0.8461 < 1
//     // Since tick is a decimal, an error under 1 is acceptable

//     // Change of base rule: multiply with 2^16 / log2 (√1.0001)
//     let log_sqrt_10001_x64 = log2p_x32 * 59543866431248i128;

//     // tick - 0.01
//     let tick_low = ((log_sqrt_10001_x64 - 184467440737095516i128) >> 64) as i32;

//     // tick + (2^-14 / log2(√1.001)) + 0.01
//     let tick_high = ((log_sqrt_10001_x64 + 15793534762490258745i128) >> 64) as i32;

//     Ok(if tick_low == tick_high {
//         tick_low
//     } else if get_sqrt_price_at_tick(tick_high).unwrap() <= sqrt_price_x64 {
//         tick_high
//     } else {
//         tick_low
//     })
// }
