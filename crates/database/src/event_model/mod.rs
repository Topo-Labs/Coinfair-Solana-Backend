pub mod repository;
pub mod event_model_repository;

use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};

/// CLMM池子信息模型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClmmPoolEvent {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,

    /// 池子地址
    pub pool_address: String,

    /// 代币A的mint地址
    pub token_a_mint: String,

    /// 代币B的mint地址
    pub token_b_mint: String,

    /// 代币A的小数位数
    pub token_a_decimals: u8,

    /// 代币B的小数位数
    pub token_b_decimals: u8,

    /// 手续费率 (万分之一)
    pub fee_rate: u32,

    /// 手续费率百分比
    pub fee_rate_percentage: f64,

    /// 年化手续费率
    pub annual_fee_rate: f64,

    /// 池子类型
    pub pool_type: String,

    /// 初始sqrt价格
    pub sqrt_price_x64: String,

    /// 初始价格比率
    pub initial_price: f64,

    /// 初始tick
    pub initial_tick: i32,

    /// 池子创建者
    pub creator: String,

    /// CLMM配置地址
    pub clmm_config: String,

    /// 是否为稳定币对
    pub is_stable_pair: bool,

    /// 预估流动性价值(USD)
    pub estimated_liquidity_usd: f64,

    /// 创建时间戳
    pub created_at: i64,

    /// 交易签名
    pub signature: String,

    /// 区块高度
    pub slot: u64,

    /// 处理时间
    pub processed_at: i64,

    /// 最后更新时间
    pub updated_at: i64,
}

/// NFT领取事件模型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NftClaimEvent {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,

    /// NFT的mint地址
    pub nft_mint: String,

    /// 领取者钱包地址
    pub claimer: String,

    /// 推荐人地址（可选）
    pub referrer: Option<String>,

    /// NFT等级 (1-5级)
    pub tier: u8,

    /// 等级名称
    pub tier_name: String,

    /// 等级奖励倍率
    pub tier_bonus_rate: f64,

    /// 领取的代币数量
    pub claim_amount: u64,

    /// 代币mint地址
    pub token_mint: String,

    /// 奖励倍率 (基点)
    pub reward_multiplier: u16,

    /// 奖励倍率百分比
    pub reward_multiplier_percentage: f64,

    /// 实际奖励金额（包含倍率）
    pub bonus_amount: u64,

    /// 领取类型
    pub claim_type: u8,

    /// 领取类型名称
    pub claim_type_name: String,

    /// 累计领取量
    pub total_claimed: u64,

    /// 领取进度百分比
    pub claim_progress_percentage: f64,

    /// NFT所属的池子地址（可选）
    pub pool_address: Option<String>,

    /// 是否有推荐人
    pub has_referrer: bool,

    /// 是否为紧急领取
    pub is_emergency_claim: bool,

    /// 预估USD价值
    pub estimated_usd_value: f64,

    /// 领取时间戳
    pub claimed_at: i64,

    /// 交易签名
    pub signature: String,

    /// 区块高度
    pub slot: u64,

    /// 处理时间
    pub processed_at: i64,

    pub updated_at: i64,
}

/// 奖励分发事件模型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RewardDistributionEvent {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,

    /// 奖励分发ID
    pub distribution_id: i64,

    /// 奖励池地址
    pub reward_pool: String,

    /// 接收者钱包地址
    pub recipient: String,

    /// 推荐人地址（可选）
    pub referrer: Option<String>,

    /// 奖励代币mint地址
    pub reward_token_mint: String,

    /// 奖励代币小数位数
    pub reward_token_decimals: Option<u8>,

    /// 奖励代币名称
    pub reward_token_name: Option<String>,

    /// 奖励代币符号
    pub reward_token_symbol: Option<String>,

    /// 奖励代币Logo URI
    pub reward_token_logo_uri: Option<String>,

    /// 奖励数量
    pub reward_amount: u64,

    /// 基础奖励金额
    pub base_reward_amount: u64,

    /// 额外奖励金额
    pub bonus_amount: u64,

    /// 奖励类型
    pub reward_type: u8,

    /// 奖励类型名称
    pub reward_type_name: String,

    /// 奖励来源
    pub reward_source: u8,

    /// 奖励来源名称
    pub reward_source_name: String,

    /// 相关地址
    pub related_address: Option<String>,

    /// 奖励倍率 (基点)
    pub multiplier: u16,

    /// 奖励倍率百分比
    pub multiplier_percentage: f64,

    /// 是否已锁定
    pub is_locked: bool,

    /// 锁定期结束时间戳
    pub unlock_timestamp: Option<i64>,

    /// 锁定天数
    pub lock_days: u64,

    /// 是否有推荐人
    pub has_referrer: bool,

    /// 是否为推荐奖励
    pub is_referral_reward: bool,

    /// 是否为高价值奖励
    pub is_high_value_reward: bool,

    /// 预估USD价值
    pub estimated_usd_value: f64,

    /// 发放时间戳
    pub distributed_at: i64,

    /// 交易签名
    pub signature: String,

    /// 区块高度
    pub slot: u64,

    /// 处理时间
    pub processed_at: i64,

    /// 最后更新时间
    pub updated_at: i64,
}

/// 迁移状态枚举
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MigrationStatus {
    Pending,   // 待迁移
    Success,   // 迁移成功
    Failed,    // 迁移失败
    Retrying,  // 重试中
}

/// 代币对类型枚举
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PairType {
    MemeToSol,     // MEME/SOL
    MemeToUsdc,    // MEME/USDC
    MemeToUsdt,    // MEME/USDT
    MemeToOther,   // MEME/其他代币
}

/// LaunchEvent数据库模型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchEvent {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,

    // 核心业务字段
    /// meme币合约地址
    pub meme_token_mint: String,
    /// 配对代币地址(通常是SOL或USDC)
    pub base_token_mint: String,
    /// 用户钱包地址
    pub user_wallet: String,
    
    // 价格和流动性参数
    /// CLMM配置索引
    pub config_index: u32,
    /// 初始价格
    pub initial_price: f64,
    /// 价格下限
    pub tick_lower_price: f64,
    /// 价格上限
    pub tick_upper_price: f64,
    
    // 代币数量
    /// meme币数量
    pub meme_token_amount: u64,
    /// 配对代币数量
    pub base_token_amount: u64,
    
    // 交易参数
    /// 最大滑点百分比
    pub max_slippage_percent: f64,
    /// 是否包含NFT元数据
    pub with_metadata: bool,
    
    // 时间字段
    /// 池子开放时间戳，0表示立即开放
    pub open_time: u64,
    /// 发射时间戳
    pub launched_at: i64,
    
    // 迁移状态跟踪
    /// 迁移状态（pending/success/failed/retrying）
    pub migration_status: String,
    /// 迁移后的池子地址（成功后填入）
    pub migrated_pool_address: Option<String>,
    /// 迁移完成时间
    pub migration_completed_at: Option<i64>,
    /// 迁移错误信息（失败时填入）
    pub migration_error: Option<String>,
    /// 迁移重试次数
    pub migration_retry_count: u32,
    
    // 统计分析字段
    /// 流动性总价值（USD估算）
    pub total_liquidity_usd: f64,
    /// 代币对类型（meme/stable、meme/sol等）
    pub pair_type: String,
    /// 价格区间宽度百分比
    pub price_range_width_percent: f64,
    /// 是否为高价值发射（基于流动性阈值）
    pub is_high_value_launch: bool,
    
    // 区块链标准字段 - 事件来源的交易签名
    /// 事件交易签名
    pub signature: String,
    /// 区块高度
    pub slot: u64,
    /// 处理时间
    pub processed_at: i64,
    /// 最后更新时间
    pub updated_at: i64,
}

/// 代币创建事件模型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenCreationEvent {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,

    // ====== 核心业务字段 ======
    /// 项目配置地址
    pub project_config: String,
    /// 代币的 Mint 地址
    pub mint_address: String,
    /// 代币名称
    pub name: String,
    /// 代币符号
    pub symbol: String,
    /// 代币元数据的 URI（如 IPFS 链接）
    pub metadata_uri: String,
    /// 代币logo的URI
    pub logo_uri: String,
    /// 代币小数位数
    pub decimals: u8,
    /// 供应量（以最小单位计）
    pub supply: u64,
    /// 创建者的钱包地址
    pub creator: String,

    // ====== 白名单相关字段 ======
    /// 是否支持白名单（true 表示有白名单机制）
    pub has_whitelist: bool,
    /// 白名单资格检查的时间戳（Unix 时间戳，0 表示无时间限制）
    pub whitelist_deadline: i64,

    // ====== 扩展信息字段 ======
    /// 扩展信息 (JSON格式，包含项目详细信息、社交链接等)
    pub extensions: Option<mongodb::bson::Document>,
    /// 数据来源类型
    pub source: Option<String>,

    // ====== 区块链标准字段 ======
    /// 交易签名
    pub signature: String,
    /// 区块高度
    pub slot: u64,
    /// 创建时间（Unix 时间戳）
    pub created_at: i64,
    /// 事件处理时间
    pub processed_at: i64,
    /// 最后更新时间
    pub updated_at: i64,
}

/// 存款事件模型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepositEvent {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,

    // ====== 核心业务字段 ======
    /// 存款用户钱包地址
    pub user: String,
    /// 项目配置地址
    pub project_config: String,
    /// 项目代币mint的地址，用于区分是哪个项目，并非存款代币的mint，存款代币都是存sol
    pub token_mint: String,
    /// 存款数量（原始数量，需要根据decimals换算）
    pub amount: u64,
    /// 累计筹资总额
    pub total_raised: u64,

    // ====== 代币元数据字段 ======
    /// 代币小数位数
    pub token_decimals: Option<u8>,
    /// 代币名称
    pub token_name: Option<String>,
    /// 代币符号
    pub token_symbol: Option<String>,
    /// 代币Logo URI
    pub token_logo_uri: Option<String>,

    // ====== 业务扩展字段 ======
    /// 存款类型 (0: 初始存款, 1: 追加存款, 2: 应急存款)
    pub deposit_type: u8,
    /// 存款类型名称
    pub deposit_type_name: String,
    /// 关联的流动性池地址（可选）
    pub related_pool: Option<String>,
    /// 是否为高价值存款（基于预设阈值）
    pub is_high_value_deposit: bool,
    /// 预估USD价值
    pub estimated_usd_value: f64,
    /// 实际存款金额（考虑decimals后的可读数量）
    pub actual_amount: f64,
    /// 实际累计筹资额（考虑decimals后的可读数量）
    pub actual_total_raised: f64,

    // ====== 区块链标准字段 ======
    /// 交易签名（唯一标识）
    pub signature: String,
    /// 区块高度
    pub slot: u64,
    /// 存款时间戳
    pub deposited_at: i64,
    /// 事件处理时间
    pub processed_at: i64,
    /// 最后更新时间
    pub updated_at: i64,
}
