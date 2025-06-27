use serde::{Deserialize, Serialize};
use validator::Validate;
use utoipa::ToSchema;

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