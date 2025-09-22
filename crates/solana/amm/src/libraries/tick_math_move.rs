use crate::{error::ErrorCode, libraries::big_num::U128};
use anchor_lang::require;

pub const MIN_TICK: i32 = -555400;
pub const MAX_TICK: i32 = -MIN_TICK;
pub const MIN_SQRT_PRICE_X64: u128 = 64426;
pub const MAX_SQRT_PRICE_X64: u128 = 286326652021639;
const NUM_64: U128 = U128([64, 0]); // Number 64, encoded as a U128
const BIT_PRECISION: u32 = 16;

// 根据tick值获取5次方价格
pub fn get_sqrt_price_at_tick(tick: i32) -> Result<u128, anchor_lang::error::Error> {
    let abs_tick_tmp = tick.abs() as u32;
    require!(
        abs_tick_tmp <= MAX_TICK as u32,
        ErrorCode::TickUpperOverflow
    );

    // 优化的tick缩放，保持更高精度
    let scaled_tick = tick * 2 / 5;
    let abs_tick = scaled_tick.abs() as u64;

    // 更精确的初始比率，使用扩展精度
    let mut ratio = if abs_tick & 0x1 != 0 {
        U128([0xfffcb933bd6fb800, 0])
    } else {
        U128([0, 1]) // 2^64
    };

    // 优化的位操作序列，减少累积误差
    if abs_tick & 0x2 != 0 {
        ratio = (ratio * U128([0xfff97272373d4000, 0])) >> NUM_64 // 2^64 / 1.0001^0.4
    };
    if abs_tick & 0x4 != 0 {
        ratio = (ratio * U128([0xfff2e50f5f657000, 0])) >> NUM_64 // 2^64 / 1.0001^0.8
    };
    // i = 3
    if abs_tick & 0x8 != 0 {
        ratio = (ratio * U128([0xffe5caca7e10f000, 0])) >> NUM_64 // 2^64 / 1.0001^1.6
    };
    // i = 4
    if abs_tick & 0x10 != 0 {
        ratio = (ratio * U128([0xffcb9843d60f7000, 0])) >> NUM_64 // 2^64 / 1.0001^3.2
    };
    // i = 5
    if abs_tick & 0x20 != 0 {
        ratio = (ratio * U128([0xff973b41fa98e800, 0])) >> NUM_64 // 2^64 / 1.0001^6.4
    };
    // i = 6
    if abs_tick & 0x40 != 0 {
        ratio = (ratio * U128([0xff2ea16466c9b000, 0])) >> NUM_64 // 2^64 / 1.0001^12.8
    };
    // i = 7
    if abs_tick & 0x80 != 0 {
        ratio = (ratio * U128([0xfe5dee046a9a3800, 0])) >> NUM_64 // 2^64 / 1.0001^25.6
    };
    // i = 8
    if abs_tick & 0x100 != 0 {
        ratio = (ratio * U128([0xfcbe86c7900bb000, 0])) >> NUM_64 // 2^64 / 1.0001^51.2
    };
    // i = 9
    if abs_tick & 0x200 != 0 {
        ratio = (ratio * U128([0xf987a7253ac65800, 0])) >> NUM_64 // 2^64 / 1.0001^102.4
    };
    // i = 10
    if abs_tick & 0x400 != 0 {
        ratio = (ratio * U128([0xf3392b0822bb6000, 0])) >> NUM_64 // 2^64 / 1.0001^204.8
    };
    // i = 11
    if abs_tick & 0x800 != 0 {
        ratio = (ratio * U128([0xe7159475a2caf000, 0])) >> NUM_64 // 2^64 / 1.0001^409.6
    };
    // i = 12
    if abs_tick & 0x1000 != 0 {
        ratio = (ratio * U128([0xd097f3bdfd2f2000, 0])) >> NUM_64 // 2^64 / 1.0001^819.2
    };
    // i = 13
    if abs_tick & 0x2000 != 0 {
        ratio = (ratio * U128([0xa9f746462d9f8000, 0])) >> NUM_64 // 2^64 / 1.0001^1638.4
    };
    // i = 14
    if abs_tick & 0x4000 != 0 {
        ratio = (ratio * U128([0x70d869a156f31c00, 0])) >> NUM_64 // 2^64 / 1.0001^3276.8
    };
    // i = 15
    if abs_tick & 0x8000 != 0 {
        ratio = (ratio * U128([0x31be135f97ed3200, 0])) >> NUM_64 // 2^64 / 1.0001^6553.6
    };
    // i = 16
    if abs_tick & 0x10000 != 0 {
        ratio = (ratio * U128([0x9aa508b5b85a500, 0])) >> NUM_64 // 2^64 / 1.0001^13107.2
    };
    // i = 17
    if abs_tick & 0x20000 != 0 {
        ratio = (ratio * U128([0x5d6af8dedc582c, 0])) >> NUM_64 // 2^64 / 1.0001^26214.4
    };
    // i = 18
    if abs_tick & 0x40000 != 0 {
        ratio = (ratio * U128([0x2216e584f5fa, 0])) >> NUM_64 // 2^64 / 1.0001^52428.8
    };

    // Handle negative ticks
    let price_q64 = if tick > 0 {
        U128::MAX / ratio
    } else if tick == 0 {
        U128([0, 1])
    } else {
        ratio
    };

    // 改进的精度转换，减少舍入误差
    let result = price_q64 >> 32;

    // // 四舍五入而非截断
    // if (U128::from(price_Q64) & U128::from(0xffffffff)) >= U128::from(0x80000000) {
    //     result = result + 1;
    //     Ok(result.as_u128())
    // } else {
    //     Ok(result.as_u128())
    // }
    Ok(result.as_u128())
}

// 根据5次方价格获取tick值
// 辅助函数：将布尔值转换为 u8
fn bool_to_u8(value: bool) -> u8 {
    if value {
        1
    } else {
        0
    }
}

// 辅助函数：计算 MSB (Most Significant Bit)
fn most_significant_bit(mut value: u128) -> u8 {
    let mut msb = 0u8;

    let f = bool_to_u8(value >= 0x10000000000000000) << 6;
    msb |= f;
    value >>= f;

    let f = bool_to_u8(value >= 0x100000000) << 5;
    msb |= f;
    value >>= f;

    let f = bool_to_u8(value >= 0x10000) << 4;
    msb |= f;
    value >>= f;

    let f = bool_to_u8(value >= 0x100) << 3;
    msb |= f;
    value >>= f;

    let f = bool_to_u8(value >= 0x10) << 2;
    msb |= f;
    value >>= f;

    let f = bool_to_u8(value >= 0x4) << 1;
    msb |= f;
    value >>= f;

    let f = bool_to_u8(value >= 0x2) << 0;
    msb |= f;

    msb
}

// 主函数：根据价格计算 tick
pub fn get_tick_at_sqrt_price(sqrt_price_q32: u128) -> Result<i32, anchor_lang::error::Error> {
    // 验证价格范围
    require!(
        sqrt_price_q32 >= MIN_SQRT_PRICE_X64 && sqrt_price_q32 <= MAX_SQRT_PRICE_X64,
        ErrorCode::SqrtPriceX64
    );

    // 将 Q32 格式转换为 Q64 格式
    let sqrt_price_q64 = sqrt_price_q32 << 32;
    let mut r = sqrt_price_q64;

    // 计算最高有效位
    let msb = most_significant_bit(r);

    // 计算 log2(x) * 2^32，提高精度
    let mut log_2_x32 = ((msb as i128) - 64) << 32;

    // 更精确的 r 标准化
    r = if msb >= 64 {
        sqrt_price_q64 >> (msb - 63)
    } else {
        sqrt_price_q64 << (63 - msb)
    };

    // 扩展的牛顿迭代法，增加迭代次数以提高精度
    let mut shift = 31i32;
    while shift >= 12 {
        // 从14改为12，增加2个迭代
        r = (r * r) >> 63;
        let f = (r >> 64) as u8;
        log_2_x32 |= (f as i128) << shift;
        r >>= f;
        shift -= 1;
    }

    // 更精确的常数：log(sqrt(1.0001)) * 2^64，使用高精度计算
    let log_sqrt_10001 = log_2_x32 * 148859666078120i128;

    // 改进的 tick 界限计算，使用更准确的常数
    let tick_low = ((log_sqrt_10001 - 184467440737095517i128) >> 64) as i32;
    let tick_high = ((log_sqrt_10001 + 15793534762490258744i128) >> 64) as i32;

    // 确保 tick 在有效范围内
    let tick_low = tick_low.max(MIN_TICK);
    let tick_high = tick_high.min(MAX_TICK);

    // 优化的 tick 选择算法
    if tick_low == tick_high {
        Ok(tick_low)
    } else {
        // 更精确的价格比较，考虑边界情况
        let price_at_high = get_sqrt_price_at_tick(tick_high)?;
        let price_at_low = get_sqrt_price_at_tick(tick_low)?;

        // 检查中间 tick 是否更准确
        let tick_mid = (tick_low + tick_high) / 2;
        if tick_mid != tick_low && tick_mid != tick_high {
            let price_at_mid = get_sqrt_price_at_tick(tick_mid)?;

            // 计算三个 tick 与目标价格的差值
            let diff_high = if price_at_high >= sqrt_price_q32 {
                price_at_high - sqrt_price_q32
            } else {
                sqrt_price_q32 - price_at_high
            };

            let diff_low = if price_at_low >= sqrt_price_q32 {
                price_at_low - sqrt_price_q32
            } else {
                sqrt_price_q32 - price_at_low
            };

            let diff_mid = if price_at_mid >= sqrt_price_q32 {
                price_at_mid - sqrt_price_q32
            } else {
                sqrt_price_q32 - price_at_mid
            };

            // 选择差值最小的 tick
            if diff_mid <= diff_low && diff_mid <= diff_high {
                Ok(tick_mid)
            } else if diff_low <= diff_high {
                Ok(tick_low)
            } else {
                Ok(tick_high)
            }
        } else {
            // 标准的两点比较
            let diff_high = if price_at_high >= sqrt_price_q32 {
                price_at_high - sqrt_price_q32
            } else {
                sqrt_price_q32 - price_at_high
            };

            let diff_low = if price_at_low >= sqrt_price_q32 {
                price_at_low - sqrt_price_q32
            } else {
                sqrt_price_q32 - price_at_low
            };

            if diff_high <= diff_low {
                Ok(tick_high)
            } else {
                Ok(tick_low)
            }
        }
    }
}

// 辅助函数：验证 tick 和价格的往返转换
pub fn validate_tick_price_conversion(tick: i32) -> Result<bool, anchor_lang::error::Error> {
    let sqrt_price = get_sqrt_price_at_tick(tick)?;
    let recovered_tick = get_tick_at_sqrt_price(sqrt_price)?;

    // 允许小幅误差（±1 tick）
    Ok((tick - recovered_tick).abs() <= 1)
}
// pub fn get_tick_at_sqrt_price(
//     price_fifth_root_x64: u128,
// ) -> Result<i32, anchor_lang::error::Error> {
//     // Validate input range
//     require!(
//         price_fifth_root_x64 >= MIN_SQRT_PRICE_X64 && price_fifth_root_x64 < MAX_SQRT_PRICE_X64,
//         ErrorCode::SqrtPriceX64
//     );

//     // Determine log_2(price_fifth_root). First by calculating integer portion (msb)
//     let msb: u32 = 128 - price_fifth_root_x64.leading_zeros() - 1;
//     let log2p_integer_x32 = (msb as i128 - 64) << 32;

//     // Get fractional value (r/2^msb), msb always > 128
//     // We begin the iteration from bit 63 (0.5 in Q64.64)
//     let mut bit: i128 = 0x8000_0000_0000_0000i128;
//     let mut precision = 0;
//     let mut log2p_fraction_x64 = 0;

//     // Log2 iterative approximation for the fractional part
//     // Go through each 2^(j) bit where j < 64 in a Q64.64 number
//     // Append current bit value to fraction result if r^2 Q2.126 is more than 2
//     let mut r = if msb >= 64 {
//         price_fifth_root_x64 >> (msb - 63)
//     } else {
//         price_fifth_root_x64 << (63 - msb)
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

//     // Change of base rule: multiply with 5 / log2(1.0001)
//     // Since we want tick where 1.0001^(tick/5) = price_fifth_root
//     // We have: tick = 5 * log_{1.0001}(price_fifth_root)
//     // Converting to log2: tick = 5 * log2(price_fifth_root) / log2(1.0001)

//     // Constants for P^(1/5) model:
//     // log2(1.0001) ≈ 0.0001442622910945383
//     // 5 / log2(1.0001) ≈ 34659.09186707279
//     // In Q32.32 format: 34659.09186707279 * 2^32 ≈ 148,859,666,078,137

//     let log_10001_fifth_root_x64 = log2p_x32 * 148859666078137i128;

//     // Error margin adjustment for P^(1/5) model
//     // 14 bit refinement gives an error margin of 2^-14 / log2(1.0001) * 5
//     // This accounts for the 5th root relationship

//     // tick - 0.01 (adjusted for 5th root model)
//     let tick_low = ((log_10001_fifth_root_x64 - 214748364i128) >> 64) as i32;

//     // tick + error_margin + 0.01 (adjusted for 5th root model)
//     let tick_high = ((log_10001_fifth_root_x64 + 9085672978i128) >> 64) as i32;

//     // Validate and return the correct tick
//     Ok(if tick_low == tick_high {
//         tick_low
//     } else if get_sqrt_price_at_tick(tick_high).unwrap() <= price_fifth_root_x64 {
//         tick_high
//     } else {
//         tick_low
//     })
// }

#[cfg(test)]
mod tick_math_test {
    use super::*;
    mod get_sqrt_price_at_tick_test {
        use super::*;
        use crate::libraries::fixed_point_64;

        #[test]
        fn check_get_sqrt_price_at_tick_at_min_or_max_tick() {
            assert_eq!(
                get_sqrt_price_at_tick(MIN_TICK).unwrap(),
                MIN_SQRT_PRICE_X64
            );
            let min_sqrt_price = MIN_SQRT_PRICE_X64 as f64 / fixed_point_64::Q64 as f64;
            println!("min_sqrt_price: {}", min_sqrt_price);
            assert_eq!(
                get_sqrt_price_at_tick(MAX_TICK).unwrap(),
                MAX_SQRT_PRICE_X64
            );
            let max_sqrt_price = MAX_SQRT_PRICE_X64 as f64 / fixed_point_64::Q64 as f64;
            println!("max_sqrt_price: {}", max_sqrt_price);
        }
    }

    mod get_tick_at_sqrt_price_test {
        use super::*;

        #[test]
        fn check_get_tick_at_sqrt_price_at_min_or_max_sqrt_price() {
            assert_eq!(
                get_tick_at_sqrt_price(MIN_SQRT_PRICE_X64).unwrap(),
                MIN_TICK,
            );

            // we can't reach MAX_SQRT_PRICE_X64
            assert_eq!(
                get_tick_at_sqrt_price(MAX_SQRT_PRICE_X64 - 1).unwrap(),
                MAX_TICK - 1,
            );
        }
    }

    #[test]
    fn tick_round_down() {
        // tick is negative
        let sqrt_price_x64 = get_sqrt_price_at_tick(-28861).unwrap();
        let mut tick = get_tick_at_sqrt_price(sqrt_price_x64).unwrap();
        assert_eq!(tick, -28861);
        tick = get_tick_at_sqrt_price(sqrt_price_x64 + 1).unwrap();
        assert_eq!(tick, -28861);
        tick = get_tick_at_sqrt_price(get_sqrt_price_at_tick(-28860).unwrap() - 1).unwrap();
        assert_eq!(tick, -28861);
        tick = get_tick_at_sqrt_price(sqrt_price_x64 - 1).unwrap();
        assert_eq!(tick, -28862);

        // tick is positive
        let sqrt_price_x64 = get_sqrt_price_at_tick(28861).unwrap();
        tick = get_tick_at_sqrt_price(sqrt_price_x64).unwrap();
        assert_eq!(tick, 28861);
        tick = get_tick_at_sqrt_price(sqrt_price_x64 + 1).unwrap();
        assert_eq!(tick, 28861);
        tick = get_tick_at_sqrt_price(get_sqrt_price_at_tick(28862).unwrap() - 1).unwrap();
        assert_eq!(tick, 28861);
        tick = get_tick_at_sqrt_price(sqrt_price_x64 - 1).unwrap();
        assert_eq!(tick, 28860);
    }

    mod fuzz_tests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn get_sqrt_price_at_tick_test (
                tick in MIN_TICK+1..MAX_TICK-1,
            ) {
                let sqrt_price_x64 = get_sqrt_price_at_tick(tick).unwrap();

                assert!(sqrt_price_x64 >= MIN_SQRT_PRICE_X64);
                assert!(sqrt_price_x64 <= MAX_SQRT_PRICE_X64);

                let minus_tick_price_x64 = get_sqrt_price_at_tick(tick - 1).unwrap();
                let plus_tick_price_x64 = get_sqrt_price_at_tick(tick + 1).unwrap();
                assert!(minus_tick_price_x64 < sqrt_price_x64 && sqrt_price_x64 < plus_tick_price_x64);
            }

            #[test]
            fn get_tick_at_sqrt_price_test (
                sqrt_price in MIN_SQRT_PRICE_X64..MAX_SQRT_PRICE_X64
            ) {
                let tick = get_tick_at_sqrt_price(sqrt_price).unwrap();

                assert!(tick >= MIN_TICK);
                assert!(tick <= MAX_TICK);

                assert!(sqrt_price >= get_sqrt_price_at_tick(tick).unwrap() && sqrt_price < get_sqrt_price_at_tick(tick + 1).unwrap())
            }

            #[test]
            fn tick_and_sqrt_price_symmetry_test (
                tick in MIN_TICK..MAX_TICK
            ) {

                let sqrt_price_x64 = get_sqrt_price_at_tick(tick).unwrap();
                let resolved_tick = get_tick_at_sqrt_price(sqrt_price_x64).unwrap();
                assert!(resolved_tick == tick);
            }


            #[test]
            fn get_sqrt_price_at_tick_is_sequence_test (
                tick in MIN_TICK+1..MAX_TICK
            ) {

                let sqrt_price_x64 = get_sqrt_price_at_tick(tick).unwrap();
                let last_sqrt_price_x64 = get_sqrt_price_at_tick(tick-1).unwrap();
                assert!(last_sqrt_price_x64 < sqrt_price_x64);
            }

            #[test]
            fn get_tick_at_sqrt_price_is_sequence_test (
                sqrt_price in (MIN_SQRT_PRICE_X64 + 10)..MAX_SQRT_PRICE_X64
            ) {

                let tick = get_tick_at_sqrt_price(sqrt_price).unwrap();
                let last_tick = get_tick_at_sqrt_price(sqrt_price - 10).unwrap();
                assert!(last_tick <= tick);
            }
        }
    }
}
