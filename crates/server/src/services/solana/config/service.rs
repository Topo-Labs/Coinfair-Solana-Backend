use anyhow::Result;
use async_trait::async_trait;
use database::{clmm_config::ClmmConfigRepository, Database};
use std::sync::Arc;
use tracing::{error, info, warn};

use crate::dtos::static_dto::{ClmmConfig, ClmmConfigResponse, SaveClmmConfigRequest, SaveClmmConfigResponse};

/// CLMM配置服务trait
#[async_trait]
pub trait ClmmConfigServiceTrait {
    /// 获取CLMM配置列表
    async fn get_clmm_configs(&self) -> Result<ClmmConfigResponse>;

    /// 从链上同步CLMM配置到数据库
    async fn sync_clmm_configs_from_chain(&self) -> Result<u64>;

    /// 保存CLMM配置到数据库
    async fn save_clmm_config(&self, config: ClmmConfig) -> Result<String>;

    /// 保存新的CLMM配置（基于请求数据）
    async fn save_clmm_config_from_request(&self, request: SaveClmmConfigRequest) -> Result<SaveClmmConfigResponse>;
}

/// CLMM配置服务实现
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
            
        let (config_pda, bump) = utils::solana::calculators::PDACalculator::calculate_amm_config_pda(
            &raydium_program_id, 
            index
        );
        
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
            let config_pda = config_id.parse::<solana_sdk::pubkey::Pubkey>()
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use database::Database;
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
        });
        let database = Arc::new(Database::new(config).await.unwrap());
        let rpc_client = Arc::new(solana_client::rpc_client::RpcClient::new("https://api.devnet.solana.com".to_string()));
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
}
