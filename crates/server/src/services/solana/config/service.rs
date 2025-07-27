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

        // 获取Raydium程序ID
        let raydium_program_id = utils::solana::ConfigManager::get_raydium_program_id()?;

        // 计算所有AMM配置PDA
        let mut pda_addresses = Vec::new();
        for &index in &amm_config_indexes {
            let (pda, _bump) = utils::solana::calculators::PDACalculator::calculate_amm_config_pda(&raydium_program_id, index);
            pda_addresses.push(pda);
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

                        // 创建配置模型
                        let config_model = database::clmm_config::ClmmConfigModel::new(
                            pda_addresses[i].to_string(),
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

        // 生成配置ID (如果是新配置，生成一个临时ID，实际应该从链上获取)
        let config_id = if let Some(existing) = &existing_config {
            existing.config_id.clone()
        } else {
            // 对于新配置，我们生成一个基于索引的临时ID
            // 在实际应用中，这个ID应该从区块链上计算得出
            format!("temp_config_{}", request.index)
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
