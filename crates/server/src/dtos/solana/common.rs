use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;
use validator::Validate;

/// 交易状态枚举
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
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
    pub id: String,
    /// 是否成功
    pub success: bool,

    /// 响应数据
    pub data: Option<T>,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            success: true,
            data: Some(data),
        }
    }

    pub fn error(_error: ErrorResponse) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            success: false,
            data: None,
        }
    }
}

/// 交易数据响应
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TransactionData {
    /// 序列化的交易数据（Base64编码）
    pub transaction: String,
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
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
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

/// 分页查询参数
#[derive(Debug, Clone, Serialize, Deserialize, Validate, IntoParams, ToSchema)]
pub struct PaginationParams {
    /// 页码（从1开始）
    #[validate(range(min = 1))]
    #[serde(default = "default_page")]
    pub page: u64,

    /// 每页条数（最大100）
    #[validate(range(min = 1, max = 100))]
    #[serde(default = "default_page_size")]
    pub page_size: u64,

    /// 排序字段
    pub sort_by: Option<String>,

    /// 排序方向（asc/desc）
    #[validate(custom = "validate_sort_order")]
    pub sort_order: Option<String>,
}

pub fn default_page() -> u64 {
    1
}

pub fn default_page_size() -> u64 {
    20
}

/// 验证代币类型
pub fn validate_token_type(token: &str) -> Result<(), validator::ValidationError> {
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

pub fn validate_sort_order(value: &str) -> Result<(), validator::ValidationError> {
    if value == "asc" || value == "desc" {
        Ok(())
    } else {
        Err(validator::ValidationError::new("invalid_sort_order"))
    }
}

pub fn default_slippage() -> f64 {
    0.5 // 默认0.5%滑点
}
