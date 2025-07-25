use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;
use validator::Validate;
use database::clmm_pool::model::ClmmPool;

/// 交换请求DTO
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct SwapRequest {
    /// 输入代币mint地址
    #[validate(custom = "validate_token_type")]
    pub from_token: String,

    /// 输出代币mint地址
    #[validate(custom = "validate_token_type")]
    pub to_token: String,

    /// 池子地址
    pub pool_address: String,

    /// 输入金额（以最小单位计算：SOL为lamports，USDC为micro-USDC）
    #[validate(range(min = 1000))] // 最小0.000001 SOL 或 0.001 USDC
    pub amount: u64,

    /// 最小输出金额（滑点保护）
    #[validate(range(min = 0))]
    pub minimum_amount_out: u64,

    /// 最大滑点百分比（0-100）
    #[validate(range(min = 0.0, max = 50.0))]
    pub max_slippage_percent: f64,
}

/// 交换响应DTO
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SwapResponse {
    /// 交易签名
    pub signature: String,

    /// 输入代币类型
    pub from_token: String,

    /// 输出代币类型
    pub to_token: String,

    /// 实际输入金额
    pub amount_in: u64,

    /// 预期输出金额
    pub amount_out_expected: u64,

    /// 实际输出金额（交易确认后更新）
    pub amount_out_actual: Option<u64>,

    /// 交易状态
    pub status: TransactionStatus,

    /// Solana Explorer链接
    pub explorer_url: String,

    /// 交易时间戳
    pub timestamp: i64,
}

/// 余额查询响应DTO
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct BalanceResponse {
    /// SOL余额（lamports）
    pub sol_balance_lamports: u64,

    /// SOL余额（SOL）
    pub sol_balance: f64,

    /// USDC余额（micro-USDC）
    pub usdc_balance_micro: u64,

    /// USDC余额（USDC）
    pub usdc_balance: f64,

    /// 钱包地址
    pub wallet_address: String,

    /// 查询时间戳
    pub timestamp: i64,
}

/// 价格查询请求DTO
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct PriceQuoteRequest {
    /// 输入代币mint地址
    #[validate(custom = "validate_token_type")]
    pub from_token: String,

    /// 输出代币mint地址
    #[validate(custom = "validate_token_type")]
    pub to_token: String,

    /// 池子地址
    pub pool_address: String,

    /// 输入金额
    #[validate(range(min = 1))]
    pub amount: u64,
}

/// 价格查询响应DTO
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PriceQuoteResponse {
    /// 输入代币类型
    pub from_token: String,

    /// 输出代币类型
    pub to_token: String,

    /// 输入金额
    pub amount_in: u64,

    /// 预期输出金额
    pub amount_out: u64,

    /// 价格（输出代币/输入代币）
    pub price: f64,

    /// 价格影响百分比
    pub price_impact_percent: f64,

    /// 建议最小输出金额（考虑5%滑点）
    pub minimum_amount_out: u64,

    /// 查询时间戳
    pub timestamp: i64,
}

/// 交易状态枚举
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub enum TransactionStatus {
    /// 已发送，等待确认
    Pending,
    /// 已确认
    Confirmed,
    /// 已完成
    Finalized,
    /// 失败
    Failed,
    /// 模拟交易
    Simulated,
}

/// 钱包信息DTO
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct WalletInfo {
    /// 钱包地址
    pub address: String,

    /// 网络类型
    pub network: String,

    /// 是否已连接
    pub connected: bool,
}

/// 交换历史记录DTO
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SwapHistory {
    /// 交易签名
    pub signature: String,

    /// 输入代币
    pub from_token: String,

    /// 输出代币
    pub to_token: String,

    /// 输入金额
    pub amount_in: u64,

    /// 输出金额
    pub amount_out: u64,

    /// 交易状态
    pub status: TransactionStatus,

    /// 交易时间
    pub timestamp: i64,

    /// Gas费用（lamports）
    pub fee: u64,
}

/// 验证代币类型
fn validate_token_type(token: &str) -> Result<(), validator::ValidationError> {
    // 只支持实际的mint地址
    match token {
        // SOL的native mint地址
        "So11111111111111111111111111111111111111112" => Ok(()),
        
        // USDC mint地址（支持多个常见的USDC地址）
        "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v" | // 标准USDC
        "4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU" | // 配置中的USDC
        "A9mUU4qviSctJVPJdBJWkb28deg915LYJKrzQ19ji3FM" => Ok(()), // 其他USDC变体
        
        _ => {
            let mut error = validator::ValidationError::new("invalid_token");
            error.message = Some("必须使用有效的代币mint地址".into());
            Err(error)
        }
    }
}

/// 错误响应DTO
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ErrorResponse {
    /// 错误代码
    pub code: String,

    /// 错误消息
    pub message: String,

    /// 详细信息（可选）
    pub details: Option<String>,

    /// 时间戳
    pub timestamp: i64,
}

impl ErrorResponse {
    pub fn new(code: &str, message: &str) -> Self {
        Self {
            code: code.to_string(),
            message: message.to_string(),
            details: None,
            timestamp: chrono::Utc::now().timestamp(),
        }
    }

    pub fn with_details(mut self, details: &str) -> Self {
        self.details = Some(details.to_string());
        self
    }
}

/// API成功响应包装器
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ApiResponse<T> {
    /// 是否成功
    pub success: bool,

    /// 响应数据
    pub data: Option<T>,

    /// 错误信息
    pub error: Option<ErrorResponse>,

    /// 时间戳
    pub timestamp: i64,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            timestamp: chrono::Utc::now().timestamp(),
        }
    }

    pub fn error(error: ErrorResponse) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(error),
            timestamp: chrono::Utc::now().timestamp(),
        }
    }
}

// ============ Raydium API 兼容格式 ============

/// Raydium计算交换请求参数（GET查询参数）
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct ComputeSwapRequest {
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

    /// 交易版本（V0或V1）
    #[serde(rename = "txVersion")]
    pub tx_version: String,
}

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

/// 转账费信息
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Copy)]
pub struct TransferFeeInfo {
    /// 输入代币转账费
    #[serde(rename = "inputTransferFee")]
    pub input_transfer_fee: u64,

    /// 输出代币转账费
    #[serde(rename = "outputTransferFee")]
    pub output_transfer_fee: u64,

    /// 输入代币精度
    #[serde(rename = "inputMintDecimals")]
    pub input_mint_decimals: u8,

    /// 输出代币精度
    #[serde(rename = "outputMintDecimals")]
    pub output_mint_decimals: u8,
}

/// 路由计划详情
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RoutePlan {
    /// 流动性池ID
    #[serde(rename = "poolId")]
    pub pool_id: String,

    /// 输入代币mint地址
    #[serde(rename = "inputMint")]
    pub input_mint: String,

    /// 输出代币mint地址
    #[serde(rename = "outputMint")]
    pub output_mint: String,

    /// 手续费代币mint地址
    #[serde(rename = "feeMint")]
    pub fee_mint: String,

    /// 手续费率
    #[serde(rename = "feeRate")]
    pub fee_rate: u32,

    /// 手续费金额
    #[serde(rename = "feeAmount")]
    pub fee_amount: String,

    /// 剩余账户列表
    #[serde(rename = "remainingAccounts")]
    pub remaining_accounts: Vec<String>,

    /// 最后池价格（X64格式）
    #[serde(rename = "lastPoolPriceX64")]
    pub last_pool_price_x64: String,
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

/// 交易数据响应
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TransactionData {
    /// 序列化的交易数据（Base64编码）
    pub transaction: String,
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

// ============ OpenPosition API ============

/// 开仓请求DTO
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct OpenPositionRequest {
    /// 池子地址
    #[validate(length(min = 32, max = 44))]
    pub pool_address: String,

    /// 用户钱包地址
    #[validate(length(min = 32, max = 44))]
    pub user_wallet: String,

    /// 下限价格
    #[validate(range(min = 0.000001, max = 1000000.0))]
    pub tick_lower_price: f64,

    /// 上限价格
    #[validate(range(min = 0.000001, max = 1000000.0))]
    pub tick_upper_price: f64,

    /// 是否基于token0计算流动性
    pub is_base_0: bool,

    /// 输入金额（最小单位）
    #[validate(range(min = 1))]
    pub input_amount: u64,

    /// 是否包含NFT元数据
    #[serde(default)]
    pub with_metadata: bool,

    /// 最大滑点百分比（0-100）
    #[validate(range(min = 0.0, max = 50.0))]
    #[serde(default = "default_slippage")]
    pub max_slippage_percent: f64,
}

fn default_slippage() -> f64 {
    0.5 // 默认0.5%滑点
}

/// 开仓响应DTO
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct OpenPositionResponse {
    /// Base64编码的未签名交易数据
    pub transaction: String,

    /// 交易消息摘要（用于前端显示）
    pub transaction_message: String,

    /// 预期的仓位NFT mint地址
    pub position_nft_mint: String,

    /// 预期的仓位键值
    pub position_key: String,

    /// 下限tick索引
    pub tick_lower_index: i32,

    /// 上限tick索引
    pub tick_upper_index: i32,

    /// 预期的流动性数量
    pub liquidity: String, // 使用字符串避免精度丢失

    /// 预期消耗的token0数量
    pub amount_0: u64,

    /// 预期消耗的token1数量
    pub amount_1: u64,

    /// 池子地址
    pub pool_address: String,

    /// 创建时间戳
    pub timestamp: i64,
}

/// 开仓响应DTO
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct OpenPositionAndSendTransactionResponse {
    /// 交易签名
    pub signature: String,
    
    /// 位置NFT mint地址
    pub position_nft_mint: String,
    
    /// 位置键值
    pub position_key: String,
    
    /// 下限tick索引
    pub tick_lower_index: i32,
    
    /// 上限tick索引
    pub tick_upper_index: i32,
    
    /// 流动性数量
    pub liquidity: String, // 使用字符串避免精度丢失
    
    /// 实际消耗的token0数量
    pub amount_0: u64,
    
    /// 实际消耗的token1数量
    pub amount_1: u64,
    
    /// 池子地址
    pub pool_address: String,
    
    /// 交易状态
    pub status: TransactionStatus,
    
    /// Solana Explorer链接
    pub explorer_url: String,
    
    /// 交易时间戳
    pub timestamp: i64,
}

/// 仓位信息DTO
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PositionInfo {
    /// 仓位键值
    pub position_key: String,

    /// 仓位NFT mint地址
    pub nft_mint: String,

    /// 池子地址
    pub pool_id: String,

    /// 下限tick索引
    pub tick_lower_index: i32,

    /// 上限tick索引
    pub tick_upper_index: i32,

    /// 流动性数量
    pub liquidity: String,

    /// 下限价格
    pub tick_lower_price: f64,

    /// 上限价格
    pub tick_upper_price: f64,

    /// 累计的token0手续费
    pub token_fees_owed_0: u64,

    /// 累计的token1手续费
    pub token_fees_owed_1: u64,

    /// 奖励信息
    pub reward_infos: Vec<PositionRewardInfo>,

    /// 创建时间戳
    pub created_at: i64,
}

/// 仓位奖励信息
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PositionRewardInfo {
    /// 奖励代币mint地址
    pub reward_mint: String,

    /// 累计奖励数量
    pub reward_amount_owed: u64,

    /// 奖励增长内部记录
    pub growth_inside_last_x64: String,
}

/// 获取用户仓位列表请求DTO
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct GetUserPositionsRequest {
    /// 用户钱包地址（可选，默认使用服务配置的钱包）
    #[validate(length(min = 32, max = 44))]
    pub wallet_address: Option<String>,

    /// 池子地址过滤（可选）
    #[validate(length(min = 32, max = 44))]
    pub pool_address: Option<String>,
}

/// 用户仓位列表响应DTO
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UserPositionsResponse {
    /// 仓位列表
    pub positions: Vec<PositionInfo>,

    /// 总仓位数量
    pub total_count: usize,

    /// 查询的钱包地址
    pub wallet_address: String,

    /// 查询时间戳
    pub timestamp: i64,
}

/// 流动性计算请求DTO
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct CalculateLiquidityRequest {
    /// 池子地址
    #[validate(length(min = 32, max = 44))]
    pub pool_address: String,

    /// 池子地址
    #[validate(length(min = 32, max = 44))]
    pub user_wallet: String,

    /// 下限价格
    #[validate(range(min = 0.000001, max = 1000000.0))]
    pub tick_lower_price: f64,

    /// 上限价格
    #[validate(range(min = 0.000001, max = 1000000.0))]
    pub tick_upper_price: f64,

    /// 是否基于token0计算
    pub is_base_0: bool,

    /// 输入金额
    #[validate(range(min = 1))]
    pub input_amount: u64,
}

/// 流动性计算响应DTO
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CalculateLiquidityResponse {
    /// 计算得到的流动性
    pub liquidity: String,

    /// 需要的token0数量
    pub amount_0: u64,

    /// 需要的token1数量
    pub amount_1: u64,

    /// 下限tick索引
    pub tick_lower_index: i32,

    /// 上限tick索引
    pub tick_upper_index: i32,

    /// 当前池子价格
    pub current_price: f64,

    /// 价格在范围内的比例
    pub price_range_utilization: f64,
}

// ============ IncreaseLiquidity API相关DTO ============

/// 增加流动性请求DTO
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct IncreaseLiquidityRequest {
    /// 池子地址
    #[validate(length(min = 32, max = 44))]
    pub pool_address: String,

    /// 用户钱包地址
    #[validate(length(min = 32, max = 44))]
    pub user_wallet: String,

    /// 下限价格
    #[validate(range(min = 0.000001, max = 1000000.0))]
    pub tick_lower_price: f64,

    /// 上限价格
    #[validate(range(min = 0.000001, max = 1000000.0))]
    pub tick_upper_price: f64,

    /// 是否基于token0计算流动性
    pub is_base_0: bool,

    /// 输入金额（最小单位）
    #[validate(range(min = 1))]
    pub input_amount: u64,

    /// 最大滑点百分比（0-100）
    #[validate(range(min = 0.0, max = 50.0))]
    #[serde(default = "default_slippage")]
    pub max_slippage_percent: f64,
}

/// 增加流动性响应DTO
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct IncreaseLiquidityResponse {
    /// Base64编码的未签名交易数据
    pub transaction: String,

    /// 交易消息摘要（用于前端显示）
    pub transaction_message: String,

    /// 找到的现有仓位键值
    pub position_key: String,

    /// 增加的流动性数量
    pub liquidity_added: String, // 使用字符串避免精度丢失

    /// 需要消耗的token0数量
    pub amount_0: u64,

    /// 需要消耗的token1数量
    pub amount_1: u64,

    /// 下限tick索引
    pub tick_lower_index: i32,

    /// 上限tick索引
    pub tick_upper_index: i32,

    /// 池子地址
    pub pool_address: String,

    /// 创建时间戳
    pub timestamp: i64,
}

/// 增加流动性并发送交易响应DTO
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct IncreaseLiquidityAndSendTransactionResponse {
    /// 交易签名
    pub signature: String,

    /// 仓位键值
    pub position_key: String,

    /// 增加的流动性数量
    pub liquidity_added: String, // 使用字符串避免精度丢失

    /// 实际消耗的token0数量
    pub amount_0: u64,

    /// 实际消耗的token1数量
    pub amount_1: u64,

    /// 下限tick索引
    pub tick_lower_index: i32,

    /// 上限tick索引
    pub tick_upper_index: i32,

    /// 池子地址
    pub pool_address: String,

    /// 交易状态
    pub status: TransactionStatus,

    /// 区块链浏览器链接
    pub explorer_url: String,

    /// 时间戳
    pub timestamp: i64,
}

// ============ CreatePool API相关DTO ============

/// 创建池子请求DTO
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct CreatePoolRequest {
    /// AMM配置索引
    #[validate(range(min = 0, max = 255))]
    pub config_index: u16,

    /// 初始价格（token1/token0的比率）
    #[validate(range(min = 0.000001, max = 1000000.0))]
    pub price: f64,

    /// 第一个代币mint地址
    pub mint0: String,

    /// 第二个代币mint地址  
    pub mint1: String,

    /// 池子开放时间（Unix时间戳，0表示立即开放）
    #[validate(range(min = 0))]
    pub open_time: u64,

    /// 用户钱包地址（用于签名交易）
    pub user_wallet: String,
}

/// 创建池子响应DTO
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreatePoolResponse {
    /// Base64编码的未签名交易数据
    pub transaction: String,

    /// 交易描述信息
    pub transaction_message: String,

    /// 池子地址
    pub pool_address: String,

    /// AMM配置地址
    pub amm_config_address: String,

    /// Token0 Vault地址
    pub token_vault_0: String,

    /// Token1 Vault地址
    pub token_vault_1: String,

    /// 观察状态地址
    pub observation_address: String,

    /// Tick Array Bitmap Extension地址
    pub tickarray_bitmap_extension: String,

    /// 初始价格
    pub initial_price: f64,

    /// 初始sqrt_price_x64
    pub sqrt_price_x64: String,

    /// 对应的tick
    pub initial_tick: i32,

    /// 时间戳
    pub timestamp: i64,
}

/// 创建池子并发送交易响应DTO
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreatePoolAndSendTransactionResponse {
    /// 交易签名
    pub signature: String,

    /// 池子地址
    pub pool_address: String,

    /// AMM配置地址
    pub amm_config_address: String,

    /// Token0 Vault地址
    pub token_vault_0: String,

    /// Token1 Vault地址
    pub token_vault_1: String,

    /// 观察状态地址
    pub observation_address: String,

    /// Tick Array Bitmap Extension地址
    pub tickarray_bitmap_extension: String,

    /// 初始价格
    pub initial_price: f64,

    /// 初始sqrt_price_x64
    pub sqrt_price_x64: String,

    /// 对应的tick
    pub initial_tick: i32,

    /// 交易状态
    pub status: TransactionStatus,

    /// 区块链浏览器链接
    pub explorer_url: String,

    /// 时间戳
    pub timestamp: i64,
}

// ============ Classic AMM Pool API相关DTO ============

/// 创建经典AMM池子请求DTO
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct CreateClassicAmmPoolRequest {
    /// 第一个代币mint地址
    #[validate(length(min = 32, max = 44))]
    pub mint0: String,

    /// 第二个代币mint地址
    #[validate(length(min = 32, max = 44))]
    pub mint1: String,

    /// 第一个代币的初始数量（最小单位）
    #[validate(range(min = 1))]
    pub init_amount_0: u64,

    /// 第二个代币的初始数量（最小单位）
    #[validate(range(min = 1))]
    pub init_amount_1: u64,

    /// 池子开放时间（Unix时间戳，0表示立即开放）
    #[validate(range(min = 0))]
    pub open_time: u64,

    /// 用户钱包地址（用于签名交易）
    #[validate(length(min = 32, max = 44))]
    pub user_wallet: String,
}

/// 创建经典AMM池子响应DTO
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateClassicAmmPoolResponse {
    /// Base64编码的未签名交易数据
    pub transaction: String,

    /// 交易描述信息
    pub transaction_message: String,

    /// 池子地址
    pub pool_address: String,

    /// Coin mint地址（按字节序排序后的第一个mint）
    pub coin_mint: String,

    /// PC mint地址（按字节序排序后的第二个mint）
    pub pc_mint: String,

    /// Coin token账户地址
    pub coin_vault: String,

    /// PC token账户地址
    pub pc_vault: String,

    /// LP mint地址
    pub lp_mint: String,

    /// Open orders地址
    pub open_orders: String,

    /// Target orders地址
    pub target_orders: String,

    /// Withdraw queue地址
    pub withdraw_queue: String,

    /// 初始Coin数量
    pub init_coin_amount: u64,

    /// 初始PC数量
    pub init_pc_amount: u64,

    /// 池子开放时间
    pub open_time: u64,

    /// 时间戳
    pub timestamp: i64,
}

/// 创建经典AMM池子并发送交易响应DTO
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateClassicAmmPoolAndSendTransactionResponse {
    /// 交易签名
    pub signature: String,

    /// 池子地址
    pub pool_address: String,

    /// Coin mint地址（按字节序排序后的第一个mint）
    pub coin_mint: String,

    /// PC mint地址（按字节序排序后的第二个mint）
    pub pc_mint: String,

    /// Coin token账户地址
    pub coin_vault: String,

    /// PC token账户地址
    pub pc_vault: String,

    /// LP mint地址
    pub lp_mint: String,

    /// Open orders地址
    pub open_orders: String,

    /// Target orders地址
    pub target_orders: String,

    /// Withdraw queue地址
    pub withdraw_queue: String,

    /// 实际使用的Coin数量
    pub actual_coin_amount: u64,

    /// 实际使用的PC数量
    pub actual_pc_amount: u64,

    /// 池子开放时间
    pub open_time: u64,

    /// 交易状态
    pub status: TransactionStatus,

    /// 区块链浏览器链接
    pub explorer_url: String,

    /// 时间戳
    pub timestamp: i64,
}
// ============ Pool Listing API相关DTO ============

/// 池子列表查询请求参数
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Validate, IntoParams)]
pub struct PoolListRequest {
    /// 按池子类型过滤
    #[serde(rename = "poolType")]
    pub pool_type: Option<String>,
    
    /// 排序字段 (default, created_at, price, open_time)
    #[serde(rename = "poolSortField")]
    pub pool_sort_field: Option<String>,
    
    /// 排序方向 (asc, desc)
    #[serde(rename = "sortType")]
    pub sort_type: Option<String>,
    
    /// 页大小 (1-100, 默认20)
    #[serde(rename = "pageSize")]
    #[validate(range(min = 1, max = 100))]
    pub page_size: Option<u64>,
    
    /// 页码 (1-based, 默认1)
    #[validate(range(min = 1))]
    pub page: Option<u64>,
    
    /// 按创建者钱包地址过滤
    #[serde(rename = "creatorWallet")]
    pub creator_wallet: Option<String>,
    
    /// 按代币mint地址过滤
    #[serde(rename = "mintAddress")]
    pub mint_address: Option<String>,
    
    /// 按池子状态过滤
    pub status: Option<String>,
}

impl Default for PoolListRequest {
    fn default() -> Self {
        Self {
            pool_type: None,
            pool_sort_field: Some("default".to_string()),
            sort_type: Some("desc".to_string()),
            page_size: Some(20),
            page: Some(1),
            creator_wallet: None,
            mint_address: None,
            status: None,
        }
    }
}

/// 新的池子列表响应格式（匹配期望格式）
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct NewPoolListResponse {
    /// 请求ID
    pub id: String,
    
    /// 请求是否成功
    pub success: bool,
    
    /// 响应数据
    pub data: PoolListData,
}

/// 池子列表数据
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PoolListData {
    /// 池子总数
    pub count: u64,
    
    /// 池子详细信息列表
    pub data: Vec<PoolInfo>,
    
    /// 是否有下一页
    #[serde(rename = "hasNextPage")]
    pub has_next_page: bool,
}

/// 池子信息（新格式）
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PoolInfo {
    /// 池子类型
    #[serde(rename = "type")]
    pub pool_type: String,
    
    /// 程序ID
    #[serde(rename = "programId")]
    pub program_id: String,
    
    /// 池子ID（地址）
    pub id: String,
    
    /// 代币A信息
    #[serde(rename = "mintA")]
    pub mint_a: ExtendedMintInfo,
    
    /// 代币B信息
    #[serde(rename = "mintB")]
    pub mint_b: ExtendedMintInfo,
    
    /// 默认奖励池信息
    #[serde(rename = "rewardDefaultPoolInfos")]
    pub reward_default_pool_infos: String,
    
    /// 默认奖励信息
    #[serde(rename = "rewardDefaultInfos")]
    pub reward_default_infos: Vec<RewardInfo>,
    
    /// 当前价格
    pub price: f64,
    
    /// 代币A数量
    #[serde(rename = "mintAmountA")]
    pub mint_amount_a: f64,
    
    /// 代币B数量
    #[serde(rename = "mintAmountB")]
    pub mint_amount_b: f64,
    
    /// 手续费率
    #[serde(rename = "feeRate")]
    pub fee_rate: f64,
    
    /// 开放时间
    #[serde(rename = "openTime")]
    pub open_time: String,
    
    /// 总价值锁定
    pub tvl: f64,
    
    /// 日统计
    pub day: Option<PeriodStats>,
    
    /// 周统计
    pub week: Option<PeriodStats>,
    
    /// 月统计
    pub month: Option<PeriodStats>,
    
    /// 池子类型标签
    pub pooltype: Vec<String>,
    
    /// 即将开始的农场数量
    #[serde(rename = "farmUpcomingCount")]
    pub farm_upcoming_count: u32,
    
    /// 进行中的农场数量
    #[serde(rename = "farmOngoingCount")]
    pub farm_ongoing_count: u32,
    
    /// 已结束的农场数量
    #[serde(rename = "farmFinishedCount")]
    pub farm_finished_count: u32,
    
    /// 配置信息
    pub config: Option<PoolConfigInfo>,
    
    /// 燃烧百分比
    #[serde(rename = "burnPercent")]
    pub burn_percent: f64,
    
    /// 启动迁移池
    #[serde(rename = "launchMigratePool")]
    pub launch_migrate_pool: bool,
}

/// 扩展的mint信息（新格式）
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ExtendedMintInfo {
    /// 链ID
    #[serde(rename = "chainId")]
    pub chain_id: u32,
    
    /// 代币地址
    pub address: String,
    
    /// 程序ID
    #[serde(rename = "programId")]
    pub program_id: String,
    
    /// Logo URI
    #[serde(rename = "logoURI")]
    pub logo_uri: Option<String>,
    
    /// 代币符号
    pub symbol: Option<String>,
    
    /// 代币名称
    pub name: Option<String>,
    
    /// 精度
    pub decimals: u8,
    
    /// 标签
    pub tags: Vec<String>,
    
    /// 扩展信息
    pub extensions: serde_json::Value,
}

/// 奖励信息
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RewardInfo {
    /// 奖励代币信息
    pub mint: ExtendedMintInfo,
    
    /// 每秒奖励
    #[serde(rename = "perSecond")]
    pub per_second: String,
    
    /// 开始时间
    #[serde(rename = "startTime")]
    pub start_time: String,
    
    /// 结束时间
    #[serde(rename = "endTime")]
    pub end_time: String,
}

/// 周期统计信息
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PeriodStats {
    /// 交易量
    pub volume: f64,
    
    /// 报价交易量
    #[serde(rename = "volumeQuote")]
    pub volume_quote: f64,
    
    /// 手续费交易量
    #[serde(rename = "volumeFee")]
    pub volume_fee: f64,
    
    /// 年化收益率
    pub apr: f64,
    
    /// 手续费年化收益率
    #[serde(rename = "feeApr")]
    pub fee_apr: f64,
    
    /// 最低价格
    #[serde(rename = "priceMin")]
    pub price_min: f64,
    
    /// 最高价格
    #[serde(rename = "priceMax")]
    pub price_max: f64,
    
    /// 奖励年化收益率
    #[serde(rename = "rewardApr")]
    pub reward_apr: Vec<f64>,
}

/// 池子配置信息
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PoolConfigInfo {
    /// 配置ID
    pub id: String,
    
    /// 配置索引
    pub index: u32,
    
    /// 协议费率
    #[serde(rename = "protocolFeeRate")]
    pub protocol_fee_rate: u32,
    
    /// 交易费率
    #[serde(rename = "tradeFeeRate")]
    pub trade_fee_rate: u32,
    
    /// Tick间距
    #[serde(rename = "tickSpacing")]
    pub tick_spacing: u32,
    
    /// 基金费率
    #[serde(rename = "fundFeeRate")]
    pub fund_fee_rate: u32,
    
    /// 默认范围
    #[serde(rename = "defaultRange")]
    pub default_range: f64,
    
    /// 默认范围点
    #[serde(rename = "defaultRangePoint")]
    pub default_range_point: Vec<f64>,
}

/// 旧版池子列表响应（保持向后兼容）
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PoolListResponse {
    /// 池子列表
    pub pools: Vec<ClmmPool>,
    
    /// 分页元数据
    pub pagination: PaginationMeta,
    
    /// 过滤器摘要
    pub filters: FilterSummary,
}

/// 分页元数据
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PaginationMeta {
    /// 当前页码
    pub current_page: u64,
    
    /// 页大小
    pub page_size: u64,
    
    /// 符合条件的总记录数
    pub total_count: u64,
    
    /// 总页数
    pub total_pages: u64,
    
    /// 是否有下一页
    pub has_next: bool,
    
    /// 是否有上一页
    pub has_prev: bool,
}

/// 过滤器摘要
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct FilterSummary {
    /// 应用的池子类型过滤器
    pub pool_type: Option<String>,
    
    /// 应用的排序字段
    pub sort_field: String,
    
    /// 应用的排序方向
    pub sort_direction: String,
    
    /// 按池子类型统计数量
    pub type_counts: Vec<TypeCount>,
}

/// 池子类型统计
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TypeCount {
    /// 池子类型
    pub pool_type: String,
    
    /// 数量
    pub count: u64,
}

// ============ DecreaseLiquidity API相关DTO ============

/// 减少流动性请求DTO
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct DecreaseLiquidityRequest {
    /// 池子地址
    #[validate(length(min = 32, max = 44))]
    pub pool_address: String,
    
    /// 用户钱包地址
    #[validate(length(min = 32, max = 44))]
    pub user_wallet: String,
    
    /// 下限tick索引
    pub tick_lower_index: i32,
    
    /// 上限tick索引  
    pub tick_upper_index: i32,
    
    /// 要减少的流动性数量（可选，如果为空则减少全部流动性）
    pub liquidity: Option<String>, // 使用字符串避免精度丢失
    
    /// 最大滑点百分比（0-100）
    #[validate(range(min = 0.0, max = 50.0))]
    pub max_slippage_percent: Option<f64>,
    
    /// 是否只模拟交易（不实际发送）
    #[serde(default)]
    pub simulate: bool,
}

/// 减少流动性响应DTO
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DecreaseLiquidityResponse {
    /// Base64编码的未签名交易数据
    pub transaction: String,
    
    /// 交易消息摘要（用于前端显示）
    pub transaction_message: String,
    
    /// 仓位键值
    pub position_key: String,
    
    /// 减少的流动性数量
    pub liquidity_removed: String, // 使用字符串避免精度丢失
    
    /// 预期获得的token0数量（减去滑点和转账费）
    pub amount_0_min: u64,
    
    /// 预期获得的token1数量（减去滑点和转账费）
    pub amount_1_min: u64,
    
    /// 预期实际获得的token0数量（未减去滑点和转账费）
    pub amount_0_expected: u64,
    
    /// 预期实际获得的token1数量（未减去滑点和转账费）
    pub amount_1_expected: u64,
    
    /// 下限tick索引
    pub tick_lower_index: i32,
    
    /// 上限tick索引
    pub tick_upper_index: i32,
    
    /// 池子地址
    pub pool_address: String,
    
    /// 是否会完全关闭仓位
    pub will_close_position: bool,
    
    /// 时间戳
    pub timestamp: i64,
}

/// 减少流动性并发送交易响应DTO
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DecreaseLiquidityAndSendTransactionResponse {
    /// 交易签名
    pub signature: String,
    
    /// 仓位键值
    pub position_key: String,
    
    /// 减少的流动性数量
    pub liquidity_removed: String, // 使用字符串避免精度丢失
    
    /// 实际获得的token0数量
    pub amount_0_actual: u64,
    
    /// 实际获得的token1数量
    pub amount_1_actual: u64,
    
    /// 下限tick索引
    pub tick_lower_index: i32,
    
    /// 上限tick索引  
    pub tick_upper_index: i32,
    
    /// 池子地址
    pub pool_address: String,
    
    /// 是否已完全关闭仓位
    pub position_closed: bool,
    
    /// 交易状态
    pub status: TransactionStatus,
    
    /// Solana Explorer链接
    pub explorer_url: String,
    
    /// 时间戳
    pub timestamp: i64,
}

#[cfg(test)]
mod pool_listing_tests {
    use super::*;
    use validator::Validate;

    #[test]
    fn test_pool_list_request_default() {
        let request = PoolListRequest::default();
        
        assert_eq!(request.pool_type, None);
        assert_eq!(request.pool_sort_field, Some("default".to_string()));
        assert_eq!(request.sort_type, Some("desc".to_string()));
        assert_eq!(request.page_size, Some(20));
        assert_eq!(request.page, Some(1));
        assert_eq!(request.creator_wallet, None);
        assert_eq!(request.mint_address, None);
        assert_eq!(request.status, None);
    }

    #[test]
    fn test_pool_list_request_validation_valid() {
        let request = PoolListRequest {
            pool_type: Some("concentrated".to_string()),
            pool_sort_field: Some("created_at".to_string()),
            sort_type: Some("asc".to_string()),
            page_size: Some(50),
            page: Some(2),
            creator_wallet: Some("9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM".to_string()),
            mint_address: Some("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string()),
            status: Some("Active".to_string()),
        };

        assert!(request.validate().is_ok());
    }

    #[test]
    fn test_pool_list_request_validation_invalid_page_size() {
        let request = PoolListRequest {
            page_size: Some(101), // 超过最大值100
            ..Default::default()
        };

        let validation_result = request.validate();
        assert!(validation_result.is_err());
        
        let errors = validation_result.unwrap_err();
        assert!(errors.field_errors().contains_key("pageSize"));
    }

    #[test]
    fn test_pool_list_request_validation_invalid_page_size_zero() {
        let request = PoolListRequest {
            page_size: Some(0), // 小于最小值1
            ..Default::default()
        };

        let validation_result = request.validate();
        assert!(validation_result.is_err());
        
        let errors = validation_result.unwrap_err();
        assert!(errors.field_errors().contains_key("pageSize"));
    }

    #[test]
    fn test_pool_list_request_validation_invalid_page_zero() {
        let request = PoolListRequest {
            page: Some(0), // 小于最小值1
            ..Default::default()
        };

        let validation_result = request.validate();
        assert!(validation_result.is_err());
        
        let errors = validation_result.unwrap_err();
        assert!(errors.field_errors().contains_key("page"));
    }

    #[test]
    fn test_pool_list_request_serialization() {
        let request = PoolListRequest {
            pool_type: Some("concentrated".to_string()),
            pool_sort_field: Some("created_at".to_string()),
            sort_type: Some("asc".to_string()),
            page_size: Some(50),
            page: Some(2),
            creator_wallet: Some("9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM".to_string()),
            mint_address: Some("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string()),
            status: Some("Active".to_string()),
        };

        let json = serde_json::to_string(&request).unwrap();
        let deserialized: PoolListRequest = serde_json::from_str(&json).unwrap();

        assert_eq!(request.pool_type, deserialized.pool_type);
        assert_eq!(request.pool_sort_field, deserialized.pool_sort_field);
        assert_eq!(request.sort_type, deserialized.sort_type);
        assert_eq!(request.page_size, deserialized.page_size);
        assert_eq!(request.page, deserialized.page);
        assert_eq!(request.creator_wallet, deserialized.creator_wallet);
        assert_eq!(request.mint_address, deserialized.mint_address);
        assert_eq!(request.status, deserialized.status);
    }

    #[test]
    fn test_pool_list_request_serde_rename() {
        let json = r#"{
            "poolType": "concentrated",
            "poolSortField": "created_at",
            "sortType": "asc",
            "pageSize": 50,
            "page": 2,
            "creatorWallet": "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM",
            "mintAddress": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
            "status": "Active"
        }"#;

        let request: PoolListRequest = serde_json::from_str(json).unwrap();

        assert_eq!(request.pool_type, Some("concentrated".to_string()));
        assert_eq!(request.pool_sort_field, Some("created_at".to_string()));
        assert_eq!(request.sort_type, Some("asc".to_string()));
        assert_eq!(request.page_size, Some(50));
        assert_eq!(request.page, Some(2));
        assert_eq!(request.creator_wallet, Some("9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM".to_string()));
        assert_eq!(request.mint_address, Some("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string()));
        assert_eq!(request.status, Some("Active".to_string()));
    }

    #[test]
    fn test_pagination_meta_creation() {
        let pagination = PaginationMeta {
            current_page: 2,
            page_size: 20,
            total_count: 150,
            total_pages: 8,
            has_next: true,
            has_prev: true,
        };

        assert_eq!(pagination.current_page, 2);
        assert_eq!(pagination.page_size, 20);
        assert_eq!(pagination.total_count, 150);
        assert_eq!(pagination.total_pages, 8);
        assert!(pagination.has_next);
        assert!(pagination.has_prev);
    }

    #[test]
    fn test_filter_summary_creation() {
        let type_counts = vec![
            TypeCount {
                pool_type: "concentrated".to_string(),
                count: 100,
            },
            TypeCount {
                pool_type: "standard".to_string(),
                count: 50,
            },
        ];

        let filter_summary = FilterSummary {
            pool_type: Some("concentrated".to_string()),
            sort_field: "created_at".to_string(),
            sort_direction: "desc".to_string(),
            type_counts,
        };

        assert_eq!(filter_summary.pool_type, Some("concentrated".to_string()));
        assert_eq!(filter_summary.sort_field, "created_at");
        assert_eq!(filter_summary.sort_direction, "desc");
        assert_eq!(filter_summary.type_counts.len(), 2);
        assert_eq!(filter_summary.type_counts[0].pool_type, "concentrated");
        assert_eq!(filter_summary.type_counts[0].count, 100);
        assert_eq!(filter_summary.type_counts[1].pool_type, "standard");
        assert_eq!(filter_summary.type_counts[1].count, 50);
    }

    #[test]
    fn test_pool_list_response_creation() {
        let pools = vec![]; // Empty for test
        let pagination = PaginationMeta {
            current_page: 1,
            page_size: 20,
            total_count: 0,
            total_pages: 0,
            has_next: false,
            has_prev: false,
        };
        let filters = FilterSummary {
            pool_type: None,
            sort_field: "default".to_string(),
            sort_direction: "desc".to_string(),
            type_counts: vec![],
        };

        let response = PoolListResponse {
            pools,
            pagination,
            filters,
        };

        assert_eq!(response.pools.len(), 0);
        assert_eq!(response.pagination.current_page, 1);
        assert_eq!(response.filters.sort_field, "default");
    }

    #[test]
    fn test_type_count_serialization() {
        let type_count = TypeCount {
            pool_type: "concentrated".to_string(),
            count: 42,
        };

        let json = serde_json::to_string(&type_count).unwrap();
        let deserialized: TypeCount = serde_json::from_str(&json).unwrap();

        assert_eq!(type_count.pool_type, deserialized.pool_type);
        assert_eq!(type_count.count, deserialized.count);
    }

    #[test]
    fn test_pool_list_request_edge_cases() {
        // Test with minimum valid values
        let request = PoolListRequest {
            page_size: Some(1),
            page: Some(1),
            ..Default::default()
        };
        assert!(request.validate().is_ok());

        // Test with maximum valid values
        let request = PoolListRequest {
            page_size: Some(100),
            page: Some(u64::MAX),
            ..Default::default()
        };
        assert!(request.validate().is_ok());
    }

    #[test]
    fn test_pool_list_request_optional_fields() {
        // Test with all optional fields as None
        let request = PoolListRequest {
            pool_type: None,
            pool_sort_field: None,
            sort_type: None,
            page_size: None,
            page: None,
            creator_wallet: None,
            mint_address: None,
            status: None,
        };
        assert!(request.validate().is_ok());
    }
}