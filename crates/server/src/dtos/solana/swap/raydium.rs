use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

use crate::dtos::solana::common::{RoutePlan, TransferFeeInfo};

// Raydium计算交换请求参数（GET查询参数）
// #[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
// pub struct ComputeSwapRequest {
//     /// 输入代币的mint地址
//     #[serde(rename = "inputMint")]
//     pub input_mint: String,

//     /// 输出代币的mint地址
//     #[serde(rename = "outputMint")]
//     pub output_mint: String,

//     /// 输入或输出金额（以最小单位计算）
//     #[validate(length(min = 1))]
//     pub amount: String,

//     /// 滑点容忍度（基点，如50表示0.5%）
//     #[serde(rename = "slippageBps")]
//     #[validate(range(min = 1, max = 10000))]
//     pub slippage_bps: u16,

//     /// 交易版本（V0或V1）
//     #[serde(rename = "txVersion")]
//     pub tx_version: String,
// }

/// SwapV2计算交换请求参数（支持转账费）
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct ComputeSwapV2Request {
    /// 输入代币的mint地址
    #[serde(rename = "inputMint")]
    pub input_mint: String,

    /// 输出代币的mint地址
    #[serde(rename = "outputMint")]
    pub output_mint: String,

    /// 输入或输出金额（以最小单位计算）
    #[validate(length(min = 1))]
    pub amount: String,

    /// 滑点容忍度（基点，如50表示0.5%）
    #[serde(rename = "slippageBps")]
    #[validate(range(min = 1, max = 10000))]
    pub slippage_bps: u16,

    /// 限价（可选）
    #[serde(rename = "limitPrice")]
    pub limit_price: Option<f64>,

    /// 是否启用转账费计算（默认为true）
    #[serde(rename = "enableTransferFee")]
    pub enable_transfer_fee: Option<bool>,

    /// 交易版本（V0或V1）
    #[serde(rename = "txVersion")]
    pub tx_version: String,
}

/// Raydium标准响应格式包装器
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RaydiumResponse<T> {
    /// 请求唯一标识符
    pub id: String,

    /// 请求是否成功
    pub success: bool,

    /// API版本
    pub version: String,

    /// 响应数据
    pub data: T,
}

impl<T> RaydiumResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            success: true,
            version: "V1".to_string(),
            data,
        }
    }

    pub fn with_id(data: T, id: String) -> Self {
        Self {
            id,
            success: true,
            version: "V1".to_string(),
            data,
        }
    }
}

/// 交换计算结果数据
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SwapComputeData {
    /// 交换类型（BaseIn/BaseOut）
    #[serde(rename = "swapType")]
    pub swap_type: String,

    /// 输入代币mint地址
    #[serde(rename = "inputMint")]
    pub input_mint: String,

    /// 输入金额
    #[serde(rename = "inputAmount")]
    pub input_amount: String,

    /// 输出代币mint地址
    #[serde(rename = "outputMint")]
    pub output_mint: String,

    /// 输出金额
    #[serde(rename = "outputAmount")]
    pub output_amount: String,

    /// 最小输出阈值（考虑滑点）
    #[serde(rename = "otherAmountThreshold")]
    pub other_amount_threshold: String,

    /// 滑点设置（基点）
    #[serde(rename = "slippageBps")]
    pub slippage_bps: u16,

    /// 价格影响百分比
    #[serde(rename = "priceImpactPct")]
    pub price_impact_pct: f64,

    /// 推荐人费用
    #[serde(rename = "referrerAmount")]
    pub referrer_amount: String,

    /// 路由计划
    #[serde(rename = "routePlan")]
    pub route_plan: Vec<RoutePlan>,
}

/// SwapV2交换计算结果数据（支持转账费）
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SwapComputeV2Data {
    /// 交换类型（BaseInV2/BaseOutV2）
    #[serde(rename = "swapType")]
    pub swap_type: String,

    /// 输入代币mint地址
    #[serde(rename = "inputMint")]
    pub input_mint: String,

    /// 输入金额
    #[serde(rename = "inputAmount")]
    pub input_amount: String,

    /// 输出代币mint地址
    #[serde(rename = "outputMint")]
    pub output_mint: String,

    /// 输出金额
    #[serde(rename = "outputAmount")]
    pub output_amount: String,

    /// 最小输出阈值（考虑滑点）
    #[serde(rename = "otherAmountThreshold")]
    pub other_amount_threshold: String,

    /// 滑点设置（基点）
    #[serde(rename = "slippageBps")]
    pub slippage_bps: u16,

    /// 价格影响百分比
    #[serde(rename = "priceImpactPct")]
    pub price_impact_pct: f64,

    /// 推荐人费用
    #[serde(rename = "referrerAmount")]
    pub referrer_amount: String,

    /// 路由计划
    #[serde(rename = "routePlan")]
    pub route_plan: Vec<RoutePlan>,

    /// 转账费信息
    #[serde(rename = "transferFeeInfo")]
    pub transfer_fee_info: Option<TransferFeeInfo>,

    /// 扣除转账费后的实际金额
    #[serde(rename = "amountSpecified")]
    pub amount_specified: Option<String>,

    /// 当前epoch
    pub epoch: Option<u64>,
}

/// 交易构建请求
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct TransactionSwapRequest {
    /// 用户钱包地址
    #[validate(length(min = 32, max = 64))]
    pub wallet: String,

    /// 计算单元价格（微lamports）
    #[serde(rename = "computeUnitPriceMicroLamports")]
    pub compute_unit_price_micro_lamports: String,

    /// 交换响应数据（来自compute接口）
    #[serde(rename = "swapResponse")]
    pub swap_response: RaydiumResponse<SwapComputeData>,

    /// 交易版本
    #[serde(rename = "txVersion")]
    pub tx_version: String,

    /// 是否包装SOL
    #[serde(rename = "wrapSol")]
    pub wrap_sol: bool,

    /// 是否解包装SOL
    #[serde(rename = "unwrapSol")]
    pub unwrap_sol: bool,

    /// 输入代币账户地址（可选）
    #[serde(rename = "inputAccount")]
    pub input_account: Option<String>,

    /// 输出代币账户地址（可选）
    #[serde(rename = "outputAccount")]
    pub output_account: Option<String>,
}

/// SwapV2交易构建请求
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct TransactionSwapV2Request {
    /// 用户钱包地址
    #[validate(length(min = 32, max = 64))]
    pub wallet: String,

    /// 计算单元价格（微lamports）
    #[serde(rename = "computeUnitPriceMicroLamports")]
    pub compute_unit_price_micro_lamports: String,

    /// SwapV2交换响应数据（来自compute-v2接口）
    #[serde(rename = "swapResponse")]
    pub swap_response: RaydiumResponse<SwapComputeV2Data>,

    /// 交易版本
    #[serde(rename = "txVersion")]
    pub tx_version: String,

    /// 是否包装SOL
    #[serde(rename = "wrapSol")]
    pub wrap_sol: bool,

    /// 是否解包装SOL
    #[serde(rename = "unwrapSol")]
    pub unwrap_sol: bool,

    /// 输入代币账户地址（可选）
    #[serde(rename = "inputAccount")]
    pub input_account: Option<String>,

    /// 输出代币账户地址（可选）
    #[serde(rename = "outputAccount")]
    pub output_account: Option<String>,
}

/// Raydium错误响应
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RaydiumErrorResponse {
    /// 请求唯一标识符
    pub id: String,

    /// 请求是否成功（固定为false）
    pub success: bool,

    /// API版本
    pub version: String,

    /// 错误信息
    pub error: String,
}

impl RaydiumErrorResponse {
    pub fn new(error_message: &str) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            success: false,
            version: "V1".to_string(),
            error: error_message.to_string(),
        }
    }

    pub fn with_id(error_message: &str, id: String) -> Self {
        Self {
            id,
            success: false,
            version: "V1".to_string(),
            error: error_message.to_string(),
        }
    }
}
