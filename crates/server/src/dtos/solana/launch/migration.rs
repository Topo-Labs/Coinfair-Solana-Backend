use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use validator::Validate;

use crate::dtos::solana::common::TransactionStatus;

/// 发射迁移请求
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Validate)]
pub struct LaunchMigrationRequest {
    /// meme币合约地址
    #[validate(length(min = 32, max = 44))]
    pub meme_token_mint: String,

    /// 配对代币地址(通常是SOL或USDC)
    #[validate(length(min = 32, max = 44))]
    pub base_token_mint: String,

    /// 用户钱包地址
    #[validate(length(min = 32, max = 44))]
    pub user_wallet: String,

    /// CLMM配置索引
    #[validate(range(max = 100))]
    pub config_index: u32,

    /// 初始价格
    #[validate(range(min = 0.0, max = 1e18))]
    pub initial_price: f64,

    /// 池子开放时间戳，0表示立即开放
    pub open_time: u64,

    /// 价格下限
    #[validate(range(min = 0.0, max = 1e18))]
    pub tick_lower_price: f64,

    /// 价格上限
    #[validate(range(min = 0.0, max = 1e18))]
    pub tick_upper_price: f64,

    /// meme币数量
    #[validate(range(min = 1))]
    pub meme_token_amount: u64,

    /// 配对代币数量
    #[validate(range(min = 1))]
    pub base_token_amount: u64,

    /// 最大滑点百分比
    #[validate(range(min = 0.0, max = 100.0))]
    pub max_slippage_percent: f64,

    /// 是否包含NFT元数据
    pub with_metadata: Option<bool>,
}

/// 发射迁移响应
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct LaunchMigrationResponse {
    /// 序列化的交易数据(Base64编码)
    pub transaction: String,

    /// 人类可读的交易描述
    pub transaction_message: String,

    /// 创建的池子地址
    pub pool_address: String,

    /// AMM配置地址
    pub amm_config_address: String,

    /// 代币0金库地址
    pub token_vault_0: String,

    /// 代币1金库地址
    pub token_vault_1: String,

    /// 观察者地址
    pub observation_address: String,

    /// TickArray位图扩展地址
    pub tickarray_bitmap_extension: String,

    /// 仓位NFT mint地址
    pub position_nft_mint: String,

    /// 仓位PDA地址
    pub position_key: String,

    /// 提供的流动性数量（字符串形式避免精度丢失）
    pub liquidity: String,

    /// 经过mint顺序调整后的实际初始价格
    pub initial_price: f64,

    /// 平方根价格的x64表示
    pub sqrt_price_x64: String,

    /// 对应的tick值
    pub initial_tick: i32,

    /// 下限tick索引
    pub tick_lower_index: i32,

    /// 上限tick索引
    pub tick_upper_index: i32,

    /// token0的最大消耗量
    pub amount_0: u64,

    /// token1的最大消耗量
    pub amount_1: u64,

    /// 生成时间戳
    pub timestamp: i64,
}

/// 发射迁移并发送交易响应
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct LaunchMigrationAndSendTransactionResponse {
    /// 交易签名哈希
    pub signature: String,

    /// 交易状态
    pub status: TransactionStatus,

    /// 区块链浏览器链接
    pub explorer_url: String,

    /// 已创建的池子地址
    pub pool_address: String,

    /// AMM配置地址
    pub amm_config_address: String,

    /// 代币0金库地址
    pub token_vault_0: String,

    /// 代币1金库地址
    pub token_vault_1: String,

    /// 观察者地址
    pub observation_address: String,

    /// TickArray位图扩展地址
    pub tickarray_bitmap_extension: String,

    /// 仓位NFT mint地址
    pub position_nft_mint: String,

    /// 仓位PDA地址
    pub position_key: String,

    /// 实际提供的流动性数量
    pub liquidity: String,

    /// 实际初始价格
    pub initial_price: f64,

    /// 平方根价格的x64表示
    pub sqrt_price_x64: String,

    /// 对应的tick值
    pub initial_tick: i32,

    /// 下限tick索引
    pub tick_lower_index: i32,

    /// 上限tick索引
    pub tick_upper_index: i32,

    /// token0实际消耗量
    pub amount_0: u64,

    /// token1实际消耗量
    pub amount_1: u64,

    /// 执行完成时间戳
    pub timestamp: i64,
}

/// 迁移地址信息结构 (内部使用)
#[derive(Debug)]
pub struct MigrationAddresses {
    pub pool_address: String,
    pub amm_config_address: String,
    pub token_vault_0: String,
    pub token_vault_1: String,
    pub observation_address: String,
    pub tickarray_bitmap_extension: String,
    pub position_nft_mint: String,
    pub position_key: String,
    pub liquidity: u128,
    pub actual_initial_price: f64,
    pub sqrt_price_x64: u128,
    pub initial_tick: i32,
    pub tick_lower_index: i32,
    pub tick_upper_index: i32,
    pub amount_0: u64,
    pub amount_1: u64,
}

/// Launch Migration历史查询参数
#[derive(Debug, Clone, Serialize, Deserialize, Validate, IntoParams)]
pub struct UserLaunchHistoryParams {
    /// 创建者钱包地址
    #[validate(length(min = 32, max = 44))]
    pub creator_wallet: String,
    
    /// 状态过滤
    pub status: Option<String>,
    
    /// 页码
    #[validate(range(min = 1))]
    pub page: Option<u64>,
    
    /// 每页数量
    #[validate(range(min = 1, max = 100))]  
    pub limit: Option<u64>,
}

/// Launch Migration历史响应
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UserLaunchHistoryResponse {
    /// Launch记录列表（复用ClmmPool）
    pub launches: Vec<database::clmm_pool::model::ClmmPool>,
    
    /// 总记录数
    pub total_count: u64,
    
    /// 分页信息
    pub pagination: PaginationInfo,
}

/// 分页信息
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PaginationInfo {
    /// 当前页码
    pub current_page: u64,
    
    /// 每页数量
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

/// Launch Migration统计信息
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct LaunchMigrationStats {
    /// 总Launch次数
    pub total_launches: u64,
    
    /// 成功的Launch次数
    pub successful_launches: u64,
    
    /// 待确认的Launch次数  
    pub pending_launches: u64,
    
    /// 今日Launch次数
    pub today_launches: u64,
    
    /// 成功率百分比
    pub success_rate: f64,
    
    /// 按天统计的Launch数量（最近7天）
    pub daily_launch_counts: Vec<DailyLaunchCount>,
}

/// 每日Launch统计
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DailyLaunchCount {
    /// 日期 (YYYY-MM-DD)
    pub date: String,
    
    /// Launch次数
    pub count: u64,
    
    /// 成功次数
    pub success_count: u64,
}

/// Launch Migration统计响应
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct LaunchMigrationStatsResponse {
    pub stats: LaunchMigrationStats,
}
