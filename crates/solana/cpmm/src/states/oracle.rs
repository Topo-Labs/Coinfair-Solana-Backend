/// 预言机提供价格数据，对各种系统设计都很有用
///
use anchor_lang::prelude::*;
#[cfg(test)]
use std::time::{SystemTime, UNIX_EPOCH};
/// 用于派生账户地址和签名的种子
pub const OBSERVATION_SEED: &str = "observation";
// ObservationState 元素的数量
pub const OBSERVATION_NUM: usize = 100;
pub const OBSERVATION_UPDATE_DURATION_DEFAULT: u64 = 15;

/// ObservationState 中观察值的元素
#[zero_copy(unsafe)]
#[repr(C, packed)]
#[derive(Default, Debug)]
pub struct Observation {
    /// 观察值的区块时间戳
    pub block_timestamp: u64,
    /// 持续时间内 token0 价格的累积值，Q32.32 格式，剩余 64 位用于溢出
    pub cumulative_token_0_price_x32: u128,
    /// 持续时间内 token1 价格的累积值，Q32.32 格式，剩余 64 位用于溢出
    pub cumulative_token_1_price_x32: u128,
}
impl Observation {
    pub const LEN: usize = 8 + 16 + 16;
}

#[account(zero_copy(unsafe))]
#[repr(C, packed)]
#[cfg_attr(feature = "client", derive(Debug))]
pub struct ObservationState {
    /// ObservationState 是否已初始化
    pub initialized: bool,
    /// 观察值数组最近更新的索引
    pub observation_index: u16,
    pub pool_id: Pubkey,
    /// 观察值数组
    pub observations: [Observation; OBSERVATION_NUM],
    /// 用于功能更新的填充
    pub padding: [u64; 4],
}

impl Default for ObservationState {
    #[inline]
    fn default() -> ObservationState {
        ObservationState {
            initialized: false,
            observation_index: 0,
            pool_id: Pubkey::default(),
            observations: [Observation::default(); OBSERVATION_NUM],
            padding: [0u64; 4],
        }
    }
}

impl ObservationState {
    pub const LEN: usize = 8 + 1 + 2 + 32 + (Observation::LEN * OBSERVATION_NUM) + 8 * 4;

    // 向账户写入预言机观察值，返回下一个 observation_index。
    /// 每秒最多写入一次。索引表示最近写入的元素。
    /// 如果索引在允许数组长度的末尾 (100 - 1)，下一个索引将变为 0。
    ///
    /// # 参数
    ///
    /// * `self` - 要写入的 ObservationState 账户
    /// * `block_timestamp` - 要更新的当前时间戳
    /// * `token_0_price_x32` - 新观察值时的 token_0_price_x32
    /// * `token_1_price_x32` - 新观察值时的 token_1_price_x32
    /// * `observation_index` - 预言机数组中元素的最后更新索引
    ///
    /// # 返回值
    /// * `next_observation_index` - 预言机数组中要更新元素的新索引
    ///
    pub fn update(
        &mut self,
        block_timestamp: u64,
        token_0_price_x32: u128,
        token_1_price_x32: u128,
    ) {
        let observation_index = self.observation_index;
        if !self.initialized {
            // 跳过池初始价格
            self.initialized = true;
            self.observations[observation_index as usize].block_timestamp = block_timestamp;
            self.observations[observation_index as usize].cumulative_token_0_price_x32 = 0;
            self.observations[observation_index as usize].cumulative_token_1_price_x32 = 0;
        } else {
            let last_observation = self.observations[observation_index as usize];
            let delta_time = block_timestamp.saturating_sub(last_observation.block_timestamp);
            if delta_time < OBSERVATION_UPDATE_DURATION_DEFAULT {
                return;
            }
            let delta_token_0_price_x32 = token_0_price_x32.checked_mul(delta_time.into()).unwrap();
            let delta_token_1_price_x32 = token_1_price_x32.checked_mul(delta_time.into()).unwrap();
            let next_observation_index = if observation_index as usize == OBSERVATION_NUM - 1 {
                0
            } else {
                observation_index + 1
            };
            self.observations[next_observation_index as usize].block_timestamp = block_timestamp;
            // cumulative_token_price_x32 只占用前 64 位，剩余 64 位用于存储溢出数据
            self.observations[next_observation_index as usize].cumulative_token_0_price_x32 =
                last_observation
                    .cumulative_token_0_price_x32
                    .wrapping_add(delta_token_0_price_x32);
            self.observations[next_observation_index as usize].cumulative_token_1_price_x32 =
                last_observation
                    .cumulative_token_1_price_x32
                    .wrapping_add(delta_token_1_price_x32);
            self.observation_index = next_observation_index;
        }
    }
}

/// 返回截断到 32 位的区块时间戳，即 mod 2**32
///
pub fn block_timestamp() -> u64 {
    Clock::get().unwrap().unix_timestamp as u64 // truncation is desired
}

#[cfg(test)]
pub fn block_timestamp_mock() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

#[cfg(test)]
pub mod observation_test {
    use super::*;

    #[test]
    fn observation_state_size_test() {
        assert_eq!(
            std::mem::size_of::<ObservationState>(),
            ObservationState::LEN - 8
        )
    }
}
