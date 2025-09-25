////////////////////////////////////////////////////////////////////////
//
// 1. 每个Domain(Entity)单独一个文件夹
// 2. 每个Domain由两部分组成:
//    - model: 定义Schema
//    - repository: 实际的数据库底层操作
//
//////////////////////////////////////////////////////////////////////

use mongodb::{Client, Collection}; // 源码中集成了mongodb，因此数据是直接存储在这个程序中的(此处的是driver还是mongodb本身?)
use std::sync::Arc;
use tracing::{error, info};
use auth::permission_config;
use clmm::{clmm_config, clmm_pool, position, refer, reward, token_info};
use utils::{AppConfig, AppResult};

pub mod serde_helpers;
pub mod user;
pub mod clmm;
pub mod events;
pub mod auth;

#[derive(Clone, Debug)]
pub struct Database {
    pub refers: Collection<refer::model::Refer>,
    pub users: Collection<user::model::User>,
    pub rewards: Collection<reward::model::Reward>,
    pub clmm_pools: Collection<clmm_pool::model::ClmmPool>,
    pub clmm_configs: Collection<clmm_config::model::ClmmConfigModel>,
    pub positions: Collection<position::model::Position>,
    pub global_permission_configs: Collection<permission_config::model::GlobalSolanaPermissionConfigModel>,
    pub api_permission_configs: Collection<permission_config::model::SolanaApiPermissionConfigModel>,
    pub permission_config_logs: Collection<permission_config::model::PermissionConfigLogModel>,
    pub token_infos: Collection<token_info::model::TokenInfo>,
    // 事件模型集合
    pub clmm_pool_events: Collection<event_model::ClmmPoolEvent>,
    pub nft_claim_events: Collection<event_model::NftClaimEvent>,
    pub reward_distribution_events: Collection<event_model::RewardDistributionEvent>,
    pub launch_events: Collection<event_model::LaunchEvent>,
    pub deposit_events: Collection<event_model::DepositEvent>,
    pub token_creation_events: Collection<event_model::TokenCreationEvent>,
    // 事件扫描器集合
    pub event_scanner_checkpoints: Collection<event_scanner::model::EventScannerCheckpoints>,
    pub scan_records: Collection<event_scanner::model::ScanRecords>,
    // 仓库层
    pub clmm_pool_repository: clmm_pool::repository::ClmmPoolRepository,
    pub global_permission_repository: permission_config::repository::GlobalPermissionConfigRepository,
    pub api_permission_repository: permission_config::repository::ApiPermissionConfigRepository,
    pub permission_log_repository: permission_config::repository::PermissionConfigLogRepository,
    pub token_info_repository: token_info::repository::TokenInfoRepository,
    // 事件仓库
    pub clmm_pool_event_repository: event_model::repository::ClmmPoolEventRepository,
    pub nft_claim_event_repository: event_model::repository::NftClaimEventRepository,
    pub reward_distribution_event_repository: event_model::repository::RewardDistributionEventRepository,
    pub launch_event_repository: event_model::repository::LaunchEventRepository,
    pub deposit_event_repository: event_model::repository::DepositEventRepository,
    pub token_creation_event_repository: event_model::repository::TokenCreationEventRepository,
    // 事件扫描器仓库
    pub event_scanner_checkpoint_repository: event_scanner::repository::EventScannerCheckpointRepository,
    pub scan_record_repository: event_scanner::repository::ScanRecordRepository,
}

impl Database {
    pub async fn new(config: Arc<AppConfig>) -> AppResult<Self> {
        let client = Client::with_uri_str(&config.mongo_uri).await?;
        let db: mongodb::Database = client.database(&config.mongo_db);

        let refers = db.collection("Refer");
        let users = db.collection("User");
        let rewards = db.collection("Reward");
        let clmm_pools = db.collection("ClmmPool");
        let clmm_configs = db.collection("ClmmConfig");
        let positions = db.collection("Position");
        let global_permission_configs = db.collection("GlobalSolanaPermissionConfig");
        let api_permission_configs = db.collection("SolanaApiPermissionConfig");
        let permission_config_logs = db.collection("PermissionConfigLog");
        let token_infos = db.collection("TokenInfo");
        // 事件集合
        let clmm_pool_events = db.collection("ClmmPoolEvent");
        let nft_claim_events = db.collection("NftClaimEvent");
        let reward_distribution_events = db.collection("RewardDistributionEvent");
        let launch_events = db.collection("LaunchEvent");
        let deposit_events = db.collection("DepositEvent");
        let token_creation_events = db.collection("TokenCreationEvent");
        // 事件扫描器集合
        let event_scanner_checkpoints = db.collection("EventScannerCheckpoints");
        let scan_records = db.collection("ScanRecords");

        // 初始化仓库层
        let clmm_pool_repository = clmm_pool::repository::ClmmPoolRepository::new(clmm_pools.clone());
        let global_permission_repository =
            permission_config::repository::GlobalPermissionConfigRepository::new(global_permission_configs.clone());
        let api_permission_repository =
            permission_config::repository::ApiPermissionConfigRepository::new(api_permission_configs.clone());
        let permission_log_repository =
            permission_config::repository::PermissionConfigLogRepository::new(permission_config_logs.clone());
        let token_info_repository = token_info::repository::TokenInfoRepository::new(token_infos.clone());
        // 事件仓库
        let clmm_pool_event_repository =
            event_model::repository::ClmmPoolEventRepository::new(clmm_pool_events.clone());
        let nft_claim_event_repository =
            event_model::repository::NftClaimEventRepository::new(nft_claim_events.clone());
        let reward_distribution_event_repository =
            event_model::repository::RewardDistributionEventRepository::new(reward_distribution_events.clone());
        let launch_event_repository = event_model::repository::LaunchEventRepository::new(launch_events.clone());
        let deposit_event_repository = event_model::repository::DepositEventRepository::new(deposit_events.clone());
        let token_creation_event_repository =
            event_model::repository::TokenCreationEventRepository::new(token_creation_events.clone());
        // 事件扫描器仓库
        let event_scanner_checkpoint_repository =
            event_scanner::repository::EventScannerCheckpointRepository::new(event_scanner_checkpoints.clone());
        let scan_record_repository = event_scanner::repository::ScanRecordRepository::new(scan_records.clone());

        info!("🧱 database({:#}) connected.", &config.mongo_db);

        Ok(Database {
            refers,
            users,
            rewards,
            clmm_pools,
            clmm_configs,
            positions,
            global_permission_configs,
            api_permission_configs,
            permission_config_logs,
            token_infos,
            clmm_pool_events,
            nft_claim_events,
            reward_distribution_events,
            launch_events,
            deposit_events,
            token_creation_events,
            event_scanner_checkpoints,
            scan_records,
            clmm_pool_repository,
            global_permission_repository,
            api_permission_repository,
            permission_log_repository,
            token_info_repository,
            clmm_pool_event_repository,
            nft_claim_event_repository,
            reward_distribution_event_repository,
            launch_event_repository,
            deposit_event_repository,
            token_creation_event_repository,
            event_scanner_checkpoint_repository,
            scan_record_repository,
        })
    }

    /// 初始化权限配置索引
    pub async fn init_repository_indexes(&self) -> AppResult<()> {
        // 初始化权限配置索引
        let _result = self.api_permission_repository.init_indexes().await;

        // 初始化权限配置日志索引
        let _result = self.permission_log_repository.init_indexes().await;

        // 初始化代币信息索引
        let _result = self.token_info_repository.init_indexes().await;

        // 初始化事件索引
        let _result = self.clmm_pool_event_repository.init_indexes().await;
        let _result = self.nft_claim_event_repository.init_indexes().await;
        let _result = self.reward_distribution_event_repository.init_indexes().await;
        let _result = self.launch_event_repository.init_indexes().await;
        let _result = self.deposit_event_repository.init_indexes().await;
        let _result = self.token_creation_event_repository.init_indexes().await;

        // 初始化事件扫描器索引
        let _result = self.event_scanner_checkpoint_repository.init_indexes().await;
        let _result = self.scan_record_repository.init_indexes().await;

        info!("✅ 权限配置和事件索引初始化完成");
        Ok(())
    }

    /// 初始化默认权限配置
    pub async fn init_default_permission_config(&self) -> AppResult<()> {
        // 1. 获取或创建全局配置（会自动创建默认配置如果不存在）
        let _global_config = self
            .global_permission_repository
            .find_global_config()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to init global permission config: {}", e))?;

        // 2. 检查API配置是否已存在，如果不存在则创建默认配置
        let existing_configs = self
            .api_permission_repository
            .count_total_configs()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to count API configs: {}", e))?;

        if existing_configs == 0 {
            info!("🔧 数据库中没有API权限配置，正在创建默认配置...");
            self.create_default_api_configs().await?;
        } else {
            info!("📊 数据库中已有{}个API权限配置，跳过默认配置创建", existing_configs);
        }

        info!("✅ 默认权限配置初始化完成");
        Ok(())
    }

    /// 创建默认的API权限配置到数据库
    async fn create_default_api_configs(&self) -> AppResult<()> {
        use auth::permission_config::model::SolanaApiPermissionConfigModel;

        let now = chrono::Utc::now().timestamp() as u64;

        // 定义默认API配置（与solana_permissions.rs中的配置保持一致）
        let default_apis = vec![
            // 交换相关 API
            (
                "/api/v1/solana/swap",
                "代币交换",
                "交换",
                r#"{"RequirePermission":"ReadPool"}"#,
                r#"{"RequirePermissionAndTier":["CreatePosition","Basic"]}"#,
            ),
            ("/api/v1/solana/quote", "价格报价", "交换", r#""Allow""#, r#""Deny""#),
            ("/api/v1/solana/balance", "余额查询", "查询", r#""Allow""#, r#""Deny""#),
            // 仓位相关 API
            (
                "/api/v1/solana/position/open",
                "开仓",
                "仓位",
                r#"{"RequirePermission":"ReadPosition"}"#,
                r#"{"RequirePermissionAndTier":["CreatePosition","Premium"]}"#,
            ),
            (
                "/api/v1/solana/position/open-and-send-transaction",
                "开仓并发送交易",
                "仓位",
                r#"{"RequirePermission":"ReadPosition"}"#,
                r#"{"RequirePermissionAndTier":["CreatePosition","Premium"]}"#,
            ),
            (
                "/api/v1/solana/position/increase-liquidity",
                "增加流动性",
                "仓位",
                r#"{"RequirePermission":"ReadPosition"}"#,
                r#"{"RequirePermissionAndTier":["CreatePosition","Basic"]}"#,
            ),
            (
                "/api/v1/solana/position/decrease-liquidity",
                "减少流动性",
                "仓位",
                r#"{"RequirePermission":"ReadPosition"}"#,
                r#"{"RequirePermissionAndTier":["CreatePosition","Basic"]}"#,
            ),
            (
                "/api/v1/solana/position/list",
                "仓位列表",
                "查询",
                r#"{"RequirePermission":"ReadPosition"}"#,
                r#""Deny""#,
            ),
            (
                "/api/v1/solana/position/info",
                "仓位信息",
                "查询",
                r#"{"RequirePermission":"ReadPosition"}"#,
                r#""Deny""#,
            ),
            // 池子相关 API
            (
                "/api/v1/solana/pool/create/clmm",
                "创建CLMM池",
                "池子",
                r#"{"RequirePermission":"ReadPool"}"#,
                r#"{"RequirePermissionAndTier":["CreatePool","VIP"]}"#,
            ),
            (
                "/api/v1/solana/pool/create/cpmm",
                "创建CPMM池",
                "池子",
                r#"{"RequirePermission":"ReadPool"}"#,
                r#"{"RequirePermissionAndTier":["CreatePool","VIP"]}"#,
            ),
            (
                "/api/v1/solana/pools/info/list",
                "池子列表",
                "查询",
                r#""Allow""#,
                r#""Deny""#,
            ),
            (
                "/api/v1/solana/pools/info/mint",
                "按代币对查询池子",
                "查询",
                r#""Allow""#,
                r#""Deny""#,
            ),
            (
                "/api/v1/solana/pools/info/ids",
                "按ID查询池子",
                "查询",
                r#""Allow""#,
                r#""Deny""#,
            ),
            (
                "/api/v1/solana/pools/key/ids",
                "池子密钥信息",
                "查询",
                r#"{"RequirePermission":"ReadPool"}"#,
                r#""Deny""#,
            ),
            // 流动性相关 API
            (
                "/api/v1/solana/pools/line/*",
                "流动性分布图",
                "查询",
                r#""Allow""#,
                r#""Deny""#,
            ),
            // 配置相关 API
            (
                "/api/v1/solana/main/clmm-config/*",
                "CLMM配置",
                "配置",
                r#""Allow""#,
                r#"{"RequirePermissionAndTier":["AdminConfig","Admin"]}"#,
            ),
            // 静态配置 API
            (
                "/api/v1/solana/main/version",
                "版本信息",
                "配置",
                r#""Allow""#,
                r#""Deny""#,
            ),
            (
                "/api/v1/solana/main/auto-fee",
                "自动手续费",
                "配置",
                r#""Allow""#,
                r#""Deny""#,
            ),
            ("/api/v1/solana/main/rpcs", "RPC信息", "配置", r#""Allow""#, r#""Deny""#),
            (
                "/api/v1/solana/main/chain-time",
                "链时间",
                "配置",
                r#""Allow""#,
                r#""Deny""#,
            ),
            (
                "/api/v1/solana/mint/list",
                "代币列表",
                "配置",
                r#""Allow""#,
                r#""Deny""#,
            ),
        ];

        let mut created_count = 0;

        for (endpoint, name, category, read_policy, write_policy) in default_apis {
            let config_model = SolanaApiPermissionConfigModel {
                id: None,
                endpoint: endpoint.to_string(),
                name: name.to_string(),
                category: category.to_string(),
                read_policy: read_policy.to_string(),
                write_policy: write_policy.to_string(),
                enabled: true,
                created_at: now,
                updated_at: now,
            };

            match self.api_permission_repository.create_api_config(config_model).await {
                Ok(_) => {
                    created_count += 1;
                    info!("✅ 创建默认API配置: {}", endpoint);
                }
                Err(e) => {
                    error!("❌ 创建API配置失败 [{}]: {}", endpoint, e);
                    // 继续创建其他配置，不中断整个过程
                }
            }
        }

        info!("📊 成功创建{}个默认API权限配置到数据库", created_count);
        Ok(())
    }
}

// Re-export specific items to avoid naming conflicts
// Export specific items from clmm_config
pub use clmm::clmm_config::{model as clmm_config_model, repository as clmm_config_repository};

// Export specific items from clmm_pool, excluding TokenInfo to avoid conflict
pub use clmm::clmm_pool::{
    migration,
    model::{
        ClmmPool, ExtensionInfo, PoolQueryParams, PoolStats, PoolStatus, PoolType, PriceInfo, SyncStatus,
        TransactionInfo, TransactionStatus, VaultInfo,
    },
    repository as clmm_pool_repository,
};

// Re-export clmm_pool::TokenInfo with alias if needed
pub use clmm::clmm_pool::model::TokenInfo as ClmmTokenInfo;

// Export all from permission_config with aliases to avoid conflicts
pub use auth::permission_config::{model as permission_config_model, repository as permission_config_repository};

// Export all from position (no conflicts)
pub use clmm::position::*;

// Export all from token_info with aliases to avoid conflicts
pub use clmm::token_info::{model as token_info_model, repository as token_info_repository};

// Export all from event_scanner with aliases to avoid conflicts
pub use events::event_scanner::{model as event_scanner_model, repository as event_scanner_repository};
use events::{event_model, event_scanner};
