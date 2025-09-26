use crate::dtos::solana::common::{TransactionStatus, validate_pubkey, default_slippage_option};
use crate::dtos::solana::cpmm::swap::swap_base_in::PoolStateInfo;
#[cfg(test)]
use crate::dtos::solana::cpmm::swap::swap_base_in::AmmConfigInfo;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::Validate;

/// CPMM SwapBaseOut请求参数
///
/// 执行基于固定输出金额的代币交换
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct CpmmSwapBaseOutRequest {
    /// 池子地址
    #[validate(custom = "validate_pubkey")]
    pub pool_id: String,

    /// 用户输入代币账户地址
    #[validate(custom = "validate_pubkey")]
    pub user_input_token: String,

    /// 期望输出代币数量（扣除转账费前，以最小单位计算，如lamports）
    #[validate(range(min = 1, message = "输出金额必须大于0"))]
    pub amount_out_less_fee: u64,

    /// 滑点容忍度（百分比，0.0-100.0，默认0.5%）
    #[validate(range(min = 0.0, max = 100.0, message = "滑点必须在0-100%之间"))]
    #[serde(default = "default_slippage_option")]
    pub slippage: Option<f64>,
}

/// CPMM SwapBaseOut响应结果
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CpmmSwapBaseOutResponse {
    /// 交易签名
    pub signature: String,

    /// 池子地址
    pub pool_id: String,

    /// 输入代币Mint地址
    pub input_token_mint: String,

    /// 输出代币Mint地址
    pub output_token_mint: String,

    /// 期望的输出金额（扣除转账费前）
    pub amount_out_less_fee: u64,

    /// 实际输出金额（包含转账费）
    pub actual_amount_out: u64,

    /// 计算得出的输入需求（扣除转账费前）
    pub source_amount_swapped: u64,

    /// 实际输入转账总额（包含转账费）
    pub input_transfer_amount: u64,

    /// 最大输入金额（考虑滑点）
    pub max_amount_in: u64,

    /// 输入代币转账费
    pub input_transfer_fee: u64,

    /// 输出代币转账费
    pub output_transfer_fee: u64,

    /// 交易状态
    pub status: TransactionStatus,

    /// 区块链浏览器链接
    pub explorer_url: String,

    /// 时间戳
    pub timestamp: i64,
}

/// CPMM SwapBaseOut计算结果（用于报价和预计算）
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CpmmSwapBaseOutCompute {
    /// 池子地址
    pub pool_id: String,

    /// 输入代币Mint地址
    pub input_token_mint: String,

    /// 输出代币Mint地址
    pub output_token_mint: String,

    /// 期望的输出金额（扣除转账费前）
    pub amount_out_less_fee: u64,

    /// 实际输出金额（包含转账费）
    pub actual_amount_out: u64,

    /// 计算得出的输入需求（扣除转账费前）
    pub source_amount_swapped: u64,

    /// 实际输入转账总额（包含转账费）
    pub input_transfer_amount: u64,

    /// 最大输入金额（考虑滑点）
    pub max_amount_in: u64,

    /// 输入代币转账费
    pub input_transfer_fee: u64,

    /// 输出代币转账费
    pub output_transfer_fee: u64,

    /// 价格比率（output/input）
    pub price_ratio: f64,

    /// 价格影响（百分比）
    pub price_impact_percent: f64,

    /// 交换手续费
    pub trade_fee: u64,

    /// 滑点容忍度
    pub slippage: f64,

    /// 池子当前状态快照
    pub pool_info: PoolStateInfo,
}

/// CPMM SwapBaseOut交易构建请求
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct CpmmSwapBaseOutTransactionRequest {
    /// 用户钱包地址
    #[validate(custom = "validate_pubkey")]
    pub wallet: String,

    /// 交易版本
    pub tx_version: String,

    /// 交换计算结果
    pub swap_compute: CpmmSwapBaseOutCompute,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cpmm_swap_base_out_request_validation() {
        // 测试有效请求
        let valid_request = CpmmSwapBaseOutRequest {
            pool_id: "8k7F9Xb2wVxeJY4QcLrPz1cDEf3GhJ5mNvRtU6sB2W9A".to_string(),
            user_input_token: "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM".to_string(),
            amount_out_less_fee: 1000000,
            slippage: Some(0.5),
        };

        let validation_result = valid_request.validate();
        assert!(validation_result.is_ok(), "有效的SwapBaseOut请求应该通过验证");
    }

    #[test]
    fn test_cpmm_swap_base_out_request_invalid_pool_id() {
        // 测试无效的池子地址
        let invalid_request = CpmmSwapBaseOutRequest {
            pool_id: "invalid_pool_id".to_string(),
            user_input_token: "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM".to_string(),
            amount_out_less_fee: 1000000,
            slippage: Some(0.5),
        };

        let validation_result = invalid_request.validate();
        assert!(validation_result.is_err(), "无效的池子地址应该验证失败");
    }

    #[test]
    fn test_cpmm_swap_base_out_request_zero_amount() {
        // 测试零输出金额
        let zero_amount_request = CpmmSwapBaseOutRequest {
            pool_id: "8k7F9Xb2wVxeJY4QcLrPz1cDEf3GhJ5mNvRtU6sB2W9A".to_string(),
            user_input_token: "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM".to_string(),
            amount_out_less_fee: 0,
            slippage: Some(0.5),
        };

        let validation_result = zero_amount_request.validate();
        assert!(validation_result.is_err(), "零输出金额应该验证失败");
    }

    #[test]
    fn test_cpmm_swap_base_out_request_invalid_slippage() {
        // 测试无效的滑点值（超过100%）
        let invalid_slippage_request = CpmmSwapBaseOutRequest {
            pool_id: "8k7F9Xb2wVxeJY4QcLrPz1cDEf3GhJ5mNvRtU6sB2W9A".to_string(),
            user_input_token: "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM".to_string(),
            amount_out_less_fee: 1000000,
            slippage: Some(150.0), // 150%，超过限制
        };

        let validation_result = invalid_slippage_request.validate();
        assert!(validation_result.is_err(), "超过100%的滑点应该验证失败");
    }

    #[test]
    fn test_cpmm_swap_base_out_request_negative_slippage() {
        // 测试负数滑点
        let negative_slippage_request = CpmmSwapBaseOutRequest {
            pool_id: "8k7F9Xb2wVxeJY4QcLrPz1cDEf3GhJ5mNvRtU6sB2W9A".to_string(),
            user_input_token: "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM".to_string(),
            amount_out_less_fee: 1000000,
            slippage: Some(-1.0), // 负数滑点
        };

        let validation_result = negative_slippage_request.validate();
        assert!(validation_result.is_err(), "负数滑点应该验证失败");
    }

    #[test]
    fn test_cpmm_swap_base_out_request_default_slippage() {
        // 测试默认滑点的序列化和反序列化
        let json_str = r#"{
            "pool_id": "8k7F9Xb2wVxeJY4QcLrPz1cDEf3GhJ5mNvRtU6sB2W9A",
            "user_input_token": "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM",
            "amount_out_less_fee": 1000000
        }"#;

        let request: CpmmSwapBaseOutRequest = serde_json::from_str(json_str).unwrap();

        // 验证默认值是否正确应用
        assert_eq!(request.slippage, Some(0.5), "默认滑点应该是0.5%");

        let validation_result = request.validate();
        assert!(validation_result.is_ok(), "没有指定滑点应该使用默认值并通过验证");

        // 测试直接构造时的行为
        let request_with_none = CpmmSwapBaseOutRequest {
            pool_id: "8k7F9Xb2wVxeJY4QcLrPz1cDEf3GhJ5mNvRtU6sB2W9A".to_string(),
            user_input_token: "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM".to_string(),
            amount_out_less_fee: 1000000,
            slippage: None, // 直接设置为None
        };

        // 这种情况下，None值不会被默认函数替换，因为默认函数只在序列化时应用
        assert_eq!(request_with_none.slippage, None, "直接设置None不会触发默认值");

        let validation_result = request_with_none.validate();
        assert!(validation_result.is_ok(), "None滑点应该在服务层转换为默认值");
    }

    #[test]
    fn test_cpmm_swap_base_out_response_serialization() {
        // 测试响应序列化
        let response = CpmmSwapBaseOutResponse {
            signature: "5VfYe...transaction_signature".to_string(),
            pool_id: "8k7F9Xb2wVxeJY4QcLrPz1cDEf3GhJ5mNvRtU6sB2W9A".to_string(),
            input_token_mint: "So11111111111111111111111111111111111111112".to_string(),
            output_token_mint: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(),
            amount_out_less_fee: 1000000,
            actual_amount_out: 1005000,
            source_amount_swapped: 95000000,
            input_transfer_amount: 95250000,
            max_amount_in: 95725000,
            input_transfer_fee: 250000,
            output_transfer_fee: 5000,
            status: TransactionStatus::Confirmed,
            explorer_url: "https://solscan.io/tx/5VfYe...".to_string(),
            timestamp: 1678901234,
        };

        let serialized = serde_json::to_string(&response);
        assert!(serialized.is_ok(), "响应应该能够成功序列化");

        let deserialized: Result<CpmmSwapBaseOutResponse, _> =
            serde_json::from_str(&serialized.unwrap());
        assert!(deserialized.is_ok(), "序列化的响应应该能够反序列化");
    }

    #[test]
    fn test_cpmm_swap_base_out_compute_serialization() {
        // 测试计算结果序列化
        let compute_result = CpmmSwapBaseOutCompute {
            pool_id: "8k7F9Xb2wVxeJY4QcLrPz1cDEf3GhJ5mNvRtU6sB2W9A".to_string(),
            input_token_mint: "So11111111111111111111111111111111111111112".to_string(),
            output_token_mint: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(),
            amount_out_less_fee: 1000000,
            actual_amount_out: 1005000,
            source_amount_swapped: 95000000,
            input_transfer_amount: 95250000,
            max_amount_in: 95725000,
            input_transfer_fee: 250000,
            output_transfer_fee: 5000,
            price_ratio: 0.0105,
            price_impact_percent: 0.95,
            trade_fee: 2500,
            slippage: 0.5,
            pool_info: PoolStateInfo {
                total_token_0_amount: 100000000000,
                total_token_1_amount: 1000000000000,
                token_0_mint: "So11111111111111111111111111111111111111112".to_string(),
                token_1_mint: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(),
                trade_direction: "ZeroForOne".to_string(),
                amm_config: AmmConfigInfo {
                    trade_fee_rate: 2500,
                    creator_fee_rate: 0,
                    protocol_fee_rate: 1200,
                    fund_fee_rate: 0,
                },
            },
        };

        let serialized = serde_json::to_string(&compute_result);
        assert!(serialized.is_ok(), "计算结果应该能够成功序列化");

        let deserialized: Result<CpmmSwapBaseOutCompute, _> =
            serde_json::from_str(&serialized.unwrap());
        assert!(deserialized.is_ok(), "序列化的计算结果应该能够反序列化");
    }

    #[test]
    fn test_cpmm_swap_base_out_transaction_request_validation() {
        // 创建一个有效的计算结果用于测试
        let swap_compute = CpmmSwapBaseOutCompute {
            pool_id: "8k7F9Xb2wVxeJY4QcLrPz1cDEf3GhJ5mNvRtU6sB2W9A".to_string(),
            input_token_mint: "So11111111111111111111111111111111111111112".to_string(),
            output_token_mint: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(),
            amount_out_less_fee: 1000000,
            actual_amount_out: 1005000,
            source_amount_swapped: 95000000,
            input_transfer_amount: 95250000,
            max_amount_in: 95725000,
            input_transfer_fee: 250000,
            output_transfer_fee: 5000,
            price_ratio: 0.0105,
            price_impact_percent: 0.95,
            trade_fee: 2500,
            slippage: 0.5,
            pool_info: PoolStateInfo {
                total_token_0_amount: 100000000000,
                total_token_1_amount: 1000000000000,
                token_0_mint: "So11111111111111111111111111111111111111112".to_string(),
                token_1_mint: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(),
                trade_direction: "ZeroForOne".to_string(),
                amm_config: AmmConfigInfo {
                    trade_fee_rate: 2500,
                    creator_fee_rate: 0,
                    protocol_fee_rate: 1200,
                    fund_fee_rate: 0,
                },
            },
        };

        // 测试有效的交易构建请求
        let valid_transaction_request = CpmmSwapBaseOutTransactionRequest {
            wallet: "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM".to_string(),
            tx_version: "0".to_string(),
            swap_compute: swap_compute.clone(),
        };

        let validation_result = valid_transaction_request.validate();
        assert!(validation_result.is_ok(), "有效的交易构建请求应该通过验证");

        // 测试无效的钱包地址
        let invalid_wallet_request = CpmmSwapBaseOutTransactionRequest {
            wallet: "invalid_wallet_address".to_string(),
            tx_version: "0".to_string(),
            swap_compute,
        };

        let validation_result = invalid_wallet_request.validate();
        assert!(validation_result.is_err(), "无效的钱包地址应该验证失败");
    }
}