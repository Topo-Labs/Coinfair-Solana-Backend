use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

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
