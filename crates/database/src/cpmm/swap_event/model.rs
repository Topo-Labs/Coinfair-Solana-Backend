use chrono::{DateTime, Utc};
use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};

/// 交换事件数据模型（存储原始事件数据）
///
/// 这个模型对应最新的SwapEvent结构体：
/// ```rust
/// pub struct SwapEvent {
///     pub payer: Pubkey,              // 支付者/交换发起者
///     pub pool_id: Pubkey,            // 池子ID
///     pub input_vault_before: u64,    // 输入金库余额（扣除交易费后）
///     pub output_vault_before: u64,   // 输出金库余额（扣除交易费后）
///     pub input_amount: u64,          // 输入数量（不含转账费）
///     pub output_amount: u64,         // 输出数量（不含转账费）
///     pub input_transfer_fee: u64,    // 输入转账费
///     pub output_transfer_fee: u64,   // 输出转账费
///     pub base_input: bool,           // 是否是基础代币输入
///     pub input_mint: Pubkey,         // 输入代币mint地址
///     pub output_mint: Pubkey,        // 输出代币mint地址
///     pub trade_fee: u64,             // 交易手续费
///     pub creator_fee: u64,           // 创建者费用
///     pub creator_fee_on_input: bool, // 创建者费用是否在输入代币上
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwapEventModel {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,

    // 用户和池子信息
    /// 支付者/交换发起者钱包地址
    pub payer: String,
    /// 池子地址
    pub pool_id: String,

    // 金库状态（交换前）
    /// 输入金库余额（扣除交易费后）
    pub input_vault_before: u64,
    /// 输出金库余额（扣除交易费后）
    pub output_vault_before: u64,

    // 交换数量
    /// 输入数量（不含转账费）
    pub input_amount: u64,
    /// 输出数量（不含转账费）
    pub output_amount: u64,

    // 转账费用
    /// 输入转账费
    pub input_transfer_fee: u64,
    /// 输出转账费
    pub output_transfer_fee: u64,

    // 交换方向和代币信息
    /// 是否是基础代币输入
    pub base_input: bool,
    /// 输入代币mint地址
    pub input_mint: String,
    /// 输出代币mint地址
    pub output_mint: String,

    // 费用信息
    /// 交易手续费（代币数量）
    pub trade_fee: u64,
    /// 创建者费用（代币数量）
    pub creator_fee: u64,
    /// 创建者费用是否在输入代币上
    pub creator_fee_on_input: bool,

    // 交易元信息
    /// 交易签名（唯一标识）
    pub signature: String,
    /// 区块高度
    pub slot: u64,
    /// 区块时间戳
    pub block_time: Option<i64>,

    // 记录时间
    /// 事件创建时间
    pub created_at: DateTime<Utc>,
}

impl SwapEventModel {
    /// 验证事件数据是否有效
    pub fn validate(&self) -> Result<(), String> {
        // 验证支付者地址
        if self.payer.is_empty() {
            return Err("支付者地址不能为空".to_string());
        }

        // 验证池子地址
        if self.pool_id.is_empty() {
            return Err("池子地址不能为空".to_string());
        }

        // 验证代币mint地址
        if self.input_mint.is_empty() {
            return Err("输入代币地址不能为空".to_string());
        }

        if self.output_mint.is_empty() {
            return Err("输出代币地址不能为空".to_string());
        }

        // 验证交换数量
        if self.input_amount == 0 && self.output_amount == 0 {
            return Err("输入和输出数量不能同时为0".to_string());
        }

        // 验证交易签名
        if self.signature.is_empty() {
            return Err("交易签名不能为空".to_string());
        }

        Ok(())
    }

    /// 计算实际输入总量（包含转账费）
    pub fn get_total_input_amount(&self) -> u64 {
        self.input_amount.saturating_add(self.input_transfer_fee)
    }

    /// 计算实际输出总量（包含转账费）
    pub fn get_total_output_amount(&self) -> u64 {
        self.output_amount.saturating_add(self.output_transfer_fee)
    }

    /// 计算总费用（trade_fee + creator_fee）
    pub fn get_total_fee(&self) -> u64 {
        self.trade_fee.saturating_add(self.creator_fee)
    }

    /// 获取交换方向描述
    pub fn get_swap_direction(&self) -> &'static str {
        if self.base_input {
            "base_to_quote" // 基础代币 -> 报价代币
        } else {
            "quote_to_base" // 报价代币 -> 基础代币
        }
    }
}

/// 用户交换统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSwapStats {
    /// 用户钱包地址
    pub user_wallet: String,
    /// 总交换次数
    pub total_swaps: u64,
    /// 总输入数量
    pub total_input_amount: u64,
    /// 总输出数量
    pub total_output_amount: u64,
    /// 总手续费
    pub total_fees: u64,
    /// 首次交换时间
    pub first_swap_time: Option<DateTime<Utc>>,
    /// 最新交换时间
    pub latest_swap_time: Option<DateTime<Utc>>,
}

/// 池子交换统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolSwapStats {
    /// 池子地址
    pub pool_id: String,
    /// 总交换次数
    pub total_swaps: u64,
    /// 总交易量（输入）
    pub total_volume_input: u64,
    /// 总交易量（输出）
    pub total_volume_output: u64,
    /// 总手续费收入
    pub total_fees_collected: u64,
    /// 独立交易者数量
    pub unique_traders: u64,
    /// 首次交换时间
    pub first_swap_time: Option<DateTime<Utc>>,
    /// 最新交换时间
    pub latest_swap_time: Option<DateTime<Utc>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_swap_event() -> SwapEventModel {
        SwapEventModel {
            id: None,
            payer: "test_payer_address".to_string(),
            pool_id: "test_pool_id".to_string(),
            input_vault_before: 1000000,
            output_vault_before: 2000000,
            input_amount: 100000,
            output_amount: 200000,
            input_transfer_fee: 100,
            output_transfer_fee: 200,
            base_input: true,
            input_mint: "input_mint_address".to_string(),
            output_mint: "output_mint_address".to_string(),
            trade_fee: 250,
            creator_fee: 50,
            creator_fee_on_input: true,
            signature: "test_signature".to_string(),
            slot: 12345,
            block_time: Some(1234567890),
            created_at: Utc::now(),
        }
    }

    #[test]
    fn test_validate_success() {
        let event = create_test_swap_event();
        assert!(event.validate().is_ok());
    }

    #[test]
    fn test_validate_empty_payer() {
        let mut event = create_test_swap_event();
        event.payer = String::new();
        assert!(event.validate().is_err());
    }

    #[test]
    fn test_validate_empty_pool_id() {
        let mut event = create_test_swap_event();
        event.pool_id = String::new();
        assert!(event.validate().is_err());
    }

    #[test]
    fn test_validate_zero_amounts() {
        let mut event = create_test_swap_event();
        event.input_amount = 0;
        event.output_amount = 0;
        assert!(event.validate().is_err());
    }

    #[test]
    fn test_validate_empty_signature() {
        let mut event = create_test_swap_event();
        event.signature = String::new();
        assert!(event.validate().is_err());
    }

    #[test]
    fn test_get_total_input_amount() {
        let event = create_test_swap_event();
        assert_eq!(event.get_total_input_amount(), 100100); // 100000 + 100
    }

    #[test]
    fn test_get_total_output_amount() {
        let event = create_test_swap_event();
        assert_eq!(event.get_total_output_amount(), 200200); // 200000 + 200
    }

    #[test]
    fn test_get_total_fee() {
        let event = create_test_swap_event();
        assert_eq!(event.get_total_fee(), 300); // 250 + 50
    }

    #[test]
    fn test_get_swap_direction() {
        let mut event = create_test_swap_event();
        event.base_input = true;
        assert_eq!(event.get_swap_direction(), "base_to_quote");

        event.base_input = false;
        assert_eq!(event.get_swap_direction(), "quote_to_base");
    }
}
