use anyhow::Result;
use async_trait::async_trait;
use database::{clmm_config::ClmmConfigRepository, Database};
use solana_sdk::signature::Signer;
use std::sync::Arc;
use tracing::{error, info, warn};

use crate::dtos::statics::static_dto::{
    ClmmConfig, ClmmConfigResponse, CreateAmmConfigAndSendTransactionResponse, CreateAmmConfigRequest,
    CreateAmmConfigResponse, SaveClmmConfigRequest, SaveClmmConfigResponse,
};
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;

/// CLMM配置服务trait
#[async_trait]
pub trait ClmmConfigServiceTrait: Send + Sync {
    /// 获取CLMM配置列表
    async fn get_clmm_configs(&self) -> Result<ClmmConfigResponse>;

    /// 从链上同步CLMM配置到数据库
    async fn sync_clmm_configs_from_chain(&self) -> Result<u64>;

    /// 保存CLMM配置到数据库
    async fn save_clmm_config(&self, config: ClmmConfig) -> Result<String>;

    /// 保存新的CLMM配置（基于请求数据）
    async fn save_clmm_config_from_request(&self, request: SaveClmmConfigRequest) -> Result<SaveClmmConfigResponse>;

    /// 创建新的AMM配置（构建交易）
    async fn create_amm_config(&self, request: CreateAmmConfigRequest) -> Result<CreateAmmConfigResponse>;

    /// 创建新的AMM配置并发送交易（用于测试）
    async fn create_amm_config_and_send_transaction(
        &self,
        request: CreateAmmConfigRequest,
    ) -> Result<CreateAmmConfigAndSendTransactionResponse>;

    /// 根据配置地址获取单个配置
    async fn get_config_by_address(&self, config_address: &str) -> Result<Option<ClmmConfig>>;

    /// 根据配置地址列表批量获取配置
    async fn get_configs_by_addresses(&self, config_addresses: &[String]) -> Result<Vec<ClmmConfig>>;
}

/// CLMM配置服务实现
#[derive(Clone)]
pub struct ClmmConfigService {
    database: Arc<Database>,
    rpc_client: Arc<solana_client::rpc_client::RpcClient>,
}

impl ClmmConfigService {
    /// 创建新的CLMM配置服务
    pub fn new(database: Arc<Database>, rpc_client: Arc<solana_client::rpc_client::RpcClient>) -> Self {
        Self { database, rpc_client }
    }

    /// 获取配置仓库
    fn get_repository(&self) -> ClmmConfigRepository {
        ClmmConfigRepository::new(self.database.clmm_configs.clone())
    }

    /// 计算CLMM配置的真实PDA地址
    /// 这个方法确保所有配置ID计算保持一致
    fn calculate_config_pda(&self, index: u16) -> Result<String> {
        info!("🔍 计算CLMM配置PDA，索引: {}", index);

        let raydium_program_id = utils::solana::ConfigManager::get_raydium_program_id()
            .map_err(|e| anyhow::anyhow!("获取Raydium程序ID失败: {}", e))?;

        let (config_pda, bump) =
            utils::solana::calculators::PDACalculator::calculate_amm_config_pda(&raydium_program_id, index);

        let config_id = config_pda.to_string();
        info!("✅ 索引{}的配置PDA: {} (bump: {})", index, config_id, bump);

        Ok(config_id)
    }
}

#[async_trait]
impl ClmmConfigServiceTrait for ClmmConfigService {
    async fn get_clmm_configs(&self) -> Result<ClmmConfigResponse> {
        info!("🔧 获取CLMM配置列表");

        let repository = self.get_repository();

        match repository.get_all_enabled_configs().await {
            Ok(configs) if !configs.is_empty() => {
                info!("✅ 从数据库获取到{}个CLMM配置", configs.len());

                // 转换为API响应格式
                let api_configs: Vec<ClmmConfig> = configs
                    .iter()
                    .map(|config| ClmmConfig {
                        id: config.config_id.clone(),
                        index: config.index,
                        protocol_fee_rate: config.protocol_fee_rate,
                        trade_fee_rate: config.trade_fee_rate,
                        tick_spacing: config.tick_spacing,
                        fund_fee_rate: config.fund_fee_rate,
                        default_range: config.default_range,
                        default_range_point: config.default_range_point.clone(),
                    })
                    .collect();

                return Ok(api_configs);
            }
            Ok(_) => {
                info!("⚠️ 数据库中没有CLMM配置，尝试从链上同步");

                // 尝试从链上同步
                match self.sync_clmm_configs_from_chain().await {
                    Ok(count) => {
                        info!("✅ 从链上同步了{}个CLMM配置", count);

                        // 重新从数据库获取
                        let configs = repository.get_all_enabled_configs().await?;
                        let api_configs: Vec<ClmmConfig> = configs
                            .iter()
                            .map(|config| ClmmConfig {
                                id: config.config_id.clone(),
                                index: config.index,
                                protocol_fee_rate: config.protocol_fee_rate,
                                trade_fee_rate: config.trade_fee_rate,
                                tick_spacing: config.tick_spacing,
                                fund_fee_rate: config.fund_fee_rate,
                                default_range: config.default_range,
                                default_range_point: config.default_range_point.clone(),
                            })
                            .collect();

                        return Ok(api_configs);
                    }
                    Err(e) => {
                        warn!("⚠️ 从链上同步失败: {}", e);
                    }
                }
            }
            Err(e) => {
                error!("❌ 从数据库获取CLMM配置失败: {}", e);
            }
        }

        // 如果数据库不可用或同步失败，返回默认配置
        info!("📋 返回默认CLMM配置");
        Ok(ClmmConfig::default_configs())
    }

    async fn sync_clmm_configs_from_chain(&self) -> Result<u64> {
        info!("🔗 开始从链上同步CLMM配置");

        let repository = self.get_repository();

        // 获取配置的索引列表
        let amm_config_indexes = std::env::var("AMM_CONFIG_INDEXES")
            .unwrap_or_else(|_| "0,1,2".to_string())
            .split(',')
            .filter_map(|s| s.trim().parse::<u16>().ok())
            .collect::<Vec<u16>>();

        if amm_config_indexes.is_empty() {
            return Err(anyhow::anyhow!("未配置有效的AMM_CONFIG_INDEXES"));
        }

        info!("📋 将同步索引: {:?}", amm_config_indexes);

        // 计算所有AMM配置PDA
        let mut pda_addresses = Vec::new();
        for &index in &amm_config_indexes {
            let config_id = self.calculate_config_pda(index)?;
            let config_pda = config_id
                .parse::<solana_sdk::pubkey::Pubkey>()
                .map_err(|e| anyhow::anyhow!("解析配置PDA失败: {}", e))?;
            pda_addresses.push(config_pda);
        }
        info!("📋 计算所有AMM配置PDA: {:?}", pda_addresses);
        // 使用account_loader批量获取账户
        let account_loader = utils::solana::account_loader::AccountLoader::new(&self.rpc_client);
        let accounts = account_loader.load_multiple_accounts(&pda_addresses).await?;

        let mut saved_configs = Vec::new();

        for (i, account_opt) in accounts.iter().enumerate() {
            if let Some(account) = account_opt {
                let index = amm_config_indexes[i];

                match account_loader.deserialize_anchor_account::<raydium_amm_v3::states::AmmConfig>(account) {
                    Ok(amm_config) => {
                        info!("✅ 成功解析AMM配置索引{}: {:?}", index, amm_config);

                        // 创建配置模型 - 使用统一计算的配置ID
                        let config_id = self.calculate_config_pda(index)?;
                        let config_model = database::clmm_config::ClmmConfigModel::new(
                            config_id,
                            index as u32,
                            amm_config.protocol_fee_rate as u64,
                            amm_config.trade_fee_rate as u64,
                            amm_config.tick_spacing as u32,
                            amm_config.fund_fee_rate as u64,
                            0.1,                             // 默认范围
                            vec![0.01, 0.05, 0.1, 0.2, 0.5], // 默认范围点
                        );

                        // 保存到数据库
                        match repository.save_config(&config_model).await {
                            Ok(id) => {
                                info!("✅ 保存CLMM配置成功: {} (索引{})", id, index);
                                saved_configs.push(config_model);
                            }
                            Err(e) => {
                                error!("❌ 保存CLMM配置失败 (索引{}): {}", index, e);
                            }
                        }
                    }
                    Err(e) => {
                        error!("❌ 反序列化AMM配置失败 (索引{}): {}", index, e);
                    }
                }
            } else {
                warn!("⚠️ 未找到AMM配置账户 (索引{})", amm_config_indexes[i]);
            }
        }

        info!("✅ 从链上同步CLMM配置完成，共{}个配置", saved_configs.len());
        Ok(saved_configs.len() as u64)
    }

    async fn save_clmm_config(&self, config: ClmmConfig) -> Result<String> {
        info!("💾 保存CLMM配置: {}", config.id);

        let repository = self.get_repository();

        // 转换为数据库模型
        let config_model = database::clmm_config::ClmmConfigModel::new(
            config.id.clone(),
            config.index,
            config.protocol_fee_rate,
            config.trade_fee_rate,
            config.tick_spacing,
            config.fund_fee_rate,
            config.default_range,
            config.default_range_point,
        );

        // 保存到数据库
        match repository.save_config(&config_model).await {
            Ok(id) => {
                info!("✅ CLMM配置保存成功: {}", id);
                Ok(id)
            }
            Err(e) => {
                error!("❌ CLMM配置保存失败: {}", e);
                Err(e)
            }
        }
    }

    async fn save_clmm_config_from_request(&self, request: SaveClmmConfigRequest) -> Result<SaveClmmConfigResponse> {
        info!("📝 保存新的CLMM配置，索引: {}", request.index);

        let repository = self.get_repository();

        // 检查该索引是否已存在配置
        let existing_config = repository.get_config_by_index(request.index).await?;
        let is_new_config = existing_config.is_none();

        // 生成真实的配置ID (从链上计算PDA)
        let config_id = if let Some(existing) = &existing_config {
            existing.config_id.clone()
        } else {
            // 使用统一的PDA计算方法
            self.calculate_config_pda(request.index as u16)?
        };

        // 创建数据库模型
        let config_model = database::clmm_config::ClmmConfigModel::new(
            config_id.clone(),
            request.index,
            request.protocol_fee_rate,
            request.trade_fee_rate,
            request.tick_spacing,
            request.fund_fee_rate,
            request.default_range,
            request.default_range_point,
        );

        // 保存到数据库
        match repository.save_config(&config_model).await {
            Ok(_saved_id) => {
                let message = if is_new_config {
                    format!("成功创建新的CLMM配置，索引: {}", request.index)
                } else {
                    format!("成功更新CLMM配置，索引: {}", request.index)
                };

                info!("✅ {}", message);

                Ok(SaveClmmConfigResponse {
                    id: config_id,
                    created: is_new_config,
                    message,
                })
            }
            Err(e) => {
                error!("❌ 保存CLMM配置失败: {}", e);
                Err(e)
            }
        }
    }

    /// 创建新的AMM配置（构建交易）
    async fn create_amm_config(&self, request: CreateAmmConfigRequest) -> Result<CreateAmmConfigResponse> {
        info!("🔧 开始构建创建AMM配置交易");
        info!("  配置索引: {}", request.config_index);
        info!("  tick间距: {}", request.tick_spacing);
        info!("  交易费率: {}", request.trade_fee_rate);
        info!("  协议费率: {}", request.protocol_fee_rate);
        info!("  基金费率: {}", request.fund_fee_rate);

        // 1. 获取必要的配置信息
        let raydium_program_id = utils::solana::ConfigManager::get_raydium_program_id()
            .map_err(|e| anyhow::anyhow!("获取Raydium程序ID失败: {}", e))?;

        let admin_keypair = utils::solana::ConfigManager::get_admin_keypair()
            .map_err(|e| anyhow::anyhow!("获取管理员密钥失败: {}", e))?;

        // 2. 计算AMM配置地址
        let (config_address, _bump) =
            utils::solana::PDACalculator::calculate_amm_config_pda(&raydium_program_id, request.config_index);

        info!("📍 计算得到的配置地址: {}", config_address);

        // 3. 检查配置是否已存在
        match self.rpc_client.get_account(&config_address) {
            Ok(_) => {
                return Err(anyhow::anyhow!("配置索引 {} 已存在", request.config_index));
            }
            Err(_) => {
                info!("✅ 配置索引 {} 可用", request.config_index);
            }
        }

        // 4. 构建创建AMM配置指令
        let create_instruction = utils::solana::AmmConfigInstructionBuilder::build_create_amm_config_instruction(
            &raydium_program_id,
            &admin_keypair.pubkey(),
            request.config_index,
            request.tick_spacing,
            request.trade_fee_rate,
            request.protocol_fee_rate,
            request.fund_fee_rate,
        )?;

        // 5. 构建未签名交易
        let mut message = solana_sdk::message::Message::new(&[create_instruction], Some(&admin_keypair.pubkey()));
        message.recent_blockhash = self
            .rpc_client
            .get_latest_blockhash()
            .map_err(|e| anyhow::anyhow!("获取最新区块哈希失败: {}", e))?;

        // 序列化交易消息为Base64
        let transaction_data = bincode::serialize(&message).map_err(|e| anyhow::anyhow!("序列化交易失败: {}", e))?;
        let transaction_base64 = BASE64_STANDARD.encode(&transaction_data);

        info!("✅ 创建AMM配置交易构建成功");

        // 构建交易消息摘要
        let transaction_message = format!(
            "创建AMM配置 - 索引: {}, tick间距: {}, 交易费率: {}",
            request.config_index, request.tick_spacing, request.trade_fee_rate
        );

        let now = chrono::Utc::now().timestamp();

        let response = CreateAmmConfigResponse {
            transaction: transaction_base64,
            transaction_message,
            config_address: config_address.to_string(),
            config_index: request.config_index,
            tick_spacing: request.tick_spacing,
            trade_fee_rate: request.trade_fee_rate,
            protocol_fee_rate: request.protocol_fee_rate,
            fund_fee_rate: request.fund_fee_rate,
            timestamp: now,
        };

        // 异步保存配置到数据库（不阻塞主流程）
        let config_to_save = ClmmConfig {
            id: config_address.to_string(),
            index: request.config_index as u32,
            protocol_fee_rate: request.protocol_fee_rate as u64,
            trade_fee_rate: request.trade_fee_rate as u64,
            tick_spacing: request.tick_spacing as u32,
            fund_fee_rate: request.fund_fee_rate as u64,
            default_range: 0.1,
            default_range_point: vec![0.01, 0.05, 0.1, 0.2, 0.5],
        };

        let service_clone = self.clone();
        tokio::spawn(async move {
            if let Err(e) = service_clone.save_clmm_config(config_to_save).await {
                tracing::warn!("保存AMM配置到数据库失败: {}", e);
            } else {
                tracing::info!("✅ AMM配置已异步保存到数据库");
            }
        });

        Ok(response)
    }

    /// 创建新的AMM配置并发送交易（用于测试）
    async fn create_amm_config_and_send_transaction(
        &self,
        request: CreateAmmConfigRequest,
    ) -> Result<CreateAmmConfigAndSendTransactionResponse> {
        info!("🚀 开始创建AMM配置并发送交易");
        info!("  配置索引: {}", request.config_index);
        info!("  tick间距: {}", request.tick_spacing);
        info!("  交易费率: {}", request.trade_fee_rate);
        info!("  协议费率: {}", request.protocol_fee_rate);
        info!("  基金费率: {}", request.fund_fee_rate);

        // 1. 获取必要的配置信息
        let raydium_program_id = utils::solana::ConfigManager::get_raydium_program_id()
            .map_err(|e| anyhow::anyhow!("获取Raydium程序ID失败: {}", e))?;

        let admin_keypair = utils::solana::ConfigManager::get_admin_keypair()
            .map_err(|e| anyhow::anyhow!("获取管理员密钥失败: {}", e))?;

        // 2. 计算AMM配置地址
        let (config_address, _bump) =
            utils::solana::PDACalculator::calculate_amm_config_pda(&raydium_program_id, request.config_index);

        info!("📍 计算得到的配置地址: {}", config_address);

        // 3. 检查配置是否已存在
        match self.rpc_client.get_account(&config_address) {
            Ok(_) => {
                return Err(anyhow::anyhow!("配置索引 {} 已存在", request.config_index));
            }
            Err(_) => {
                info!("✅ 配置索引 {} 可用", request.config_index);
            }
        }

        // 4. 构建创建AMM配置指令
        let create_instruction = utils::solana::AmmConfigInstructionBuilder::build_create_amm_config_instruction(
            &raydium_program_id,
            &admin_keypair.pubkey(),
            request.config_index,
            request.tick_spacing,
            request.trade_fee_rate,
            request.protocol_fee_rate,
            request.fund_fee_rate,
        )?;

        // 5. 构建、签名并发送交易
        let recent_blockhash = self
            .rpc_client
            .get_latest_blockhash()
            .map_err(|e| anyhow::anyhow!("获取最新区块哈希失败: {}", e))?;
        let transaction = solana_sdk::transaction::Transaction::new_signed_with_payer(
            &[create_instruction],
            Some(&admin_keypair.pubkey()),
            &[&admin_keypair],
            recent_blockhash,
        );

        // 6. 发送交易
        info!("📡 发送创建AMM配置交易...");
        let signature = self
            .rpc_client
            .send_and_confirm_transaction(&transaction)
            .map_err(|e| anyhow::anyhow!("发送交易失败: {}", e))?;

        info!("✅ AMM配置创建成功");
        info!("  交易签名: {}", signature);
        info!("  配置地址: {}", config_address);

        // 7. 异步保存配置到数据库（不阻塞主流程）
        info!("💾 启动异步保存配置到数据库...");
        let config_to_save = ClmmConfig {
            id: config_address.to_string(),
            index: request.config_index as u32,
            protocol_fee_rate: request.protocol_fee_rate as u64,
            trade_fee_rate: request.trade_fee_rate as u64,
            tick_spacing: request.tick_spacing as u32,
            fund_fee_rate: request.fund_fee_rate as u64,
            default_range: 0.1,
            default_range_point: vec![0.01, 0.05, 0.1, 0.2, 0.5],
        };

        let service_clone = self.clone();
        tokio::spawn(async move {
            if let Err(e) = service_clone.save_clmm_config(config_to_save).await {
                tracing::warn!("保存AMM配置到数据库失败: {}", e);
            } else {
                tracing::info!("✅ AMM配置已异步保存到数据库");
            }
        });

        // 8. 构建响应（立即返回，不等待数据库保存）
        let explorer_url = format!("https://explorer.solana.com/tx/{}", signature);
        let now = chrono::Utc::now().timestamp();

        let db_save_response = SaveClmmConfigResponse {
            id: config_address.to_string(),
            created: true,
            message: format!("交易已成功提交，配置正在异步保存到数据库"),
        };

        Ok(CreateAmmConfigAndSendTransactionResponse {
            signature: signature.to_string(),
            config_address: config_address.to_string(),
            config_index: request.config_index,
            tick_spacing: request.tick_spacing,
            trade_fee_rate: request.trade_fee_rate,
            protocol_fee_rate: request.protocol_fee_rate,
            fund_fee_rate: request.fund_fee_rate,
            explorer_url,
            db_save_response,
            timestamp: now,
        })
    }

    async fn get_config_by_address(&self, config_address: &str) -> Result<Option<ClmmConfig>> {
        info!("🔍 根据地址查询CLMM配置: {}", config_address);

        let repository = self.get_repository();

        match repository.get_config_by_address(config_address).await {
            Ok(Some(config)) => {
                info!("✅ 找到配置: {}", config_address);
                Ok(Some(ClmmConfig {
                    id: config.config_id,
                    index: config.index,
                    protocol_fee_rate: config.protocol_fee_rate,
                    trade_fee_rate: config.trade_fee_rate,
                    tick_spacing: config.tick_spacing,
                    fund_fee_rate: config.fund_fee_rate,
                    default_range: config.default_range,
                    default_range_point: config.default_range_point,
                }))
            }
            Ok(None) => {
                info!("🔍 配置不存在: {}", config_address);
                Ok(None)
            }
            Err(e) => {
                error!("❌ 查询配置失败 {}: {}", config_address, e);
                Err(e)
            }
        }
    }

    async fn get_configs_by_addresses(&self, config_addresses: &[String]) -> Result<Vec<ClmmConfig>> {
        let start_time = std::time::Instant::now();
        info!("🔍 批量查询CLMM配置，数量: {}", config_addresses.len());

        if config_addresses.is_empty() {
            info!("📋 配置地址列表为空，返回空结果");
            return Ok(Vec::new());
        }

        let repository = self.get_repository();

        // 使用真正的批量查询 (MongoDB $in 操作符)
        match repository.get_configs_by_addresses_batch(config_addresses).await {
            Ok(configs) => {
                let results: Vec<ClmmConfig> = configs
                    .into_iter()
                    .map(|config| ClmmConfig {
                        id: config.config_id,
                        index: config.index,
                        protocol_fee_rate: config.protocol_fee_rate,
                        trade_fee_rate: config.trade_fee_rate,
                        tick_spacing: config.tick_spacing,
                        fund_fee_rate: config.fund_fee_rate,
                        default_range: config.default_range,
                        default_range_point: config.default_range_point,
                    })
                    .collect();

                let duration = start_time.elapsed();
                info!(
                    "✅ 批量查询完成，查询{}个地址，找到{}个配置，总耗时{:?}",
                    config_addresses.len(),
                    results.len(),
                    duration
                );

                // 性能监控：如果总耗时超过200ms，记录警告
                if duration.as_millis() > 200 {
                    tracing::warn!("⚠️ 服务层批量查询耗时较长: {:?}", duration);
                }

                Ok(results)
            }
            Err(e) => {
                let duration = start_time.elapsed();
                error!("❌ 批量查询失败: {}，耗时{:?}", e, duration);
                Err(e)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use database::Database;
    use std::sync::Arc;
    use utils::config::AppConfig;

    async fn create_test_service() -> ClmmConfigService {
        // 创建一个简单的测试配置，避免解析命令行参数
        let config = Arc::new(AppConfig {
            cargo_env: utils::config::CargoEnv::Development,
            app_host: "0.0.0.0".to_string(),
            app_port: 8000,
            mongo_uri: "mongodb://localhost:27017".to_string(),
            mongo_db: "test_db".to_string(),
            rpc_url: "https://api.devnet.solana.com".to_string(),
            private_key: None,
            raydium_program_id: "FA1RJDDXysgwg5Gm3fJXWxt26JQzPkAzhTA114miqNUX".to_string(),
            amm_config_index: 0,
            rust_log: "info".to_string(),
            enable_pool_event_insert: false,
            event_listener_db_mode: "update_only".to_string(),
        });
        let database = Arc::new(Database::new(config).await.unwrap());
        let rpc_client = Arc::new(solana_client::rpc_client::RpcClient::new(
            "https://api.devnet.solana.com".to_string(),
        ));
        ClmmConfigService::new(database, rpc_client)
    }

    #[tokio::test]
    async fn test_calculate_config_pda() {
        let service = create_test_service().await;

        // 测试PDA计算
        let index = 0;
        let result = service.calculate_config_pda(index);

        assert!(result.is_ok());
        let config_id = result.unwrap();

        // 验证配置ID不为空且是有效的Pubkey字符串格式
        assert!(!config_id.is_empty());
        assert!(config_id.parse::<solana_sdk::pubkey::Pubkey>().is_ok());
    }

    #[tokio::test]
    async fn test_pda_consistency() {
        let service = create_test_service().await;

        let index = 1;

        // 多次计算同一索引的PDA，结果应该一致
        let config_id1 = service.calculate_config_pda(index).unwrap();
        let config_id2 = service.calculate_config_pda(index).unwrap();

        assert_eq!(config_id1, config_id2);
    }

    #[tokio::test]
    async fn test_different_indexes_different_pdas() {
        let service = create_test_service().await;

        // 不同索引应该产生不同的PDA
        let config_id0 = service.calculate_config_pda(0).unwrap();
        let config_id1 = service.calculate_config_pda(1).unwrap();
        let config_id2 = service.calculate_config_pda(2).unwrap();

        assert_ne!(config_id0, config_id1);
        assert_ne!(config_id1, config_id2);
        assert_ne!(config_id0, config_id2);
    }

    #[tokio::test]
    async fn test_batch_query_performance() {
        let service = create_test_service().await;

        // 测试批量查询方法
        let test_addresses = vec!["Address1".to_string(), "Address2".to_string(), "Address3".to_string()];

        let start_time = std::time::Instant::now();
        let result = service.get_configs_by_addresses(&test_addresses).await;
        let duration = start_time.elapsed();

        // 应该成功返回结果（即使数据库中没有这些配置）
        assert!(result.is_ok());
        let configs = result.unwrap();

        // 由于测试数据库中没有配置，应该返回空结果
        assert_eq!(configs.len(), 0);

        // 性能检查：批量查询应该很快完成（小于100ms）
        assert!(duration.as_millis() < 100, "批量查询耗时过长: {:?}", duration);

        println!("✅ 批量查询性能测试通过，耗时: {:?}", duration);
    }

    #[tokio::test]
    async fn test_empty_batch_query() {
        let service = create_test_service().await;

        // 测试空地址列表
        let empty_addresses: Vec<String> = vec![];
        let result = service.get_configs_by_addresses(&empty_addresses).await;

        assert!(result.is_ok());
        let configs = result.unwrap();
        assert_eq!(configs.len(), 0);

        println!("✅ 空批量查询测试通过");
    }

    #[tokio::test]
    async fn test_batch_vs_individual_query_consistency() {
        let service = create_test_service().await;

        // 准备测试地址
        let test_addresses = vec!["TestConfig1".to_string(), "TestConfig2".to_string()];

        // 测试批量查询
        let batch_result = service.get_configs_by_addresses(&test_addresses).await;
        assert!(batch_result.is_ok());
        let batch_configs = batch_result.unwrap();

        // 测试单个查询
        let mut individual_configs = Vec::new();
        for address in &test_addresses {
            let individual_result = service.get_config_by_address(address).await;
            assert!(individual_result.is_ok());
            if let Some(config) = individual_result.unwrap() {
                individual_configs.push(config);
            }
        }

        // 结果应该一致
        assert_eq!(batch_configs.len(), individual_configs.len());

        println!("✅ 批量查询与单个查询一致性测试通过");
    }
}
