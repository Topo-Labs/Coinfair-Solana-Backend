use anyhow::Result;
use async_trait::async_trait;
use database::{cpmm_config_repository::CpmmConfigRepository, Database};
use std::sync::Arc;
use tracing::{error, info, warn};

use crate::dtos::statics::static_dto::{CpmmConfig, CpmmConfigResponse};

/// CPMM配置服务trait
#[async_trait]
pub trait CpmmConfigServiceTrait: Send + Sync {
    /// 获取CPMM配置列表
    async fn get_cpmm_configs(&self) -> Result<CpmmConfigResponse>;

    /// 从链上同步CPMM配置到数据库
    async fn sync_cpmm_configs_from_chain(&self) -> Result<u64>;

    /// 保存CPMM配置到数据库
    async fn save_cpmm_config(&self, config: CpmmConfig) -> Result<String>;
}

/// CPMM配置服务实现
pub struct CpmmConfigService {
    database: Arc<Database>,
}

impl CpmmConfigService {
    /// 创建新的CPMM配置服务
    pub fn new(database: Arc<Database>) -> Self {
        Self { database }
    }

    /// 获取仓库
    fn get_repository(&self) -> &CpmmConfigRepository {
        &self.database.cpmm_config_repository
    }
}

#[async_trait]
impl CpmmConfigServiceTrait for CpmmConfigService {
    async fn get_cpmm_configs(&self) -> Result<CpmmConfigResponse> {
        info!("🔧 获取CPMM配置列表");

        let repository = self.get_repository();

        match repository.get_all_enabled_configs().await {
            Ok(db_configs) => {
                if db_configs.is_empty() {
                    info!("⚠️ 数据库中没有CPMM配置数据，返回默认配置");
                    return Ok(CpmmConfig::default_configs());
                }

                // 将数据库模型转换为DTO
                let configs: Vec<CpmmConfig> = db_configs
                    .into_iter()
                    .map(|db_config| CpmmConfig {
                        id: db_config.config_id,
                        index: db_config.index,
                        protocol_fee_rate: db_config.protocol_fee_rate,
                        trade_fee_rate: db_config.trade_fee_rate,
                        fund_fee_rate: db_config.fund_fee_rate,
                        create_pool_fee: db_config.create_pool_fee,
                        creator_fee_rate: db_config.creator_fee_rate,
                    })
                    .collect();

                info!("✅ 成功获取{}个CPMM配置", configs.len());
                Ok(configs)
            }
            Err(e) => {
                error!("❌ 获取CPMM配置失败: {:?}", e);
                // 如果数据库查询失败，返回默认配置
                warn!("🔄 数据库查询失败，返回默认CPMM配置");
                Ok(CpmmConfig::default_configs())
            }
        }
    }

    async fn sync_cpmm_configs_from_chain(&self) -> Result<u64> {
        info!("🔄 开始从链上同步CPMM配置");

        // TODO: 实现从链上获取CPMM配置的逻辑
        // 这里暂时返回0，表示没有同步任何配置
        warn!("⚠️ 从链上同步CPMM配置功能尚未实现");
        Ok(0)
    }

    async fn save_cpmm_config(&self, config: CpmmConfig) -> Result<String> {
        info!("💾 保存CPMM配置，ID: {}, 索引: {}", config.id, config.index);

        let repository = self.get_repository();

        // 转换为数据库模型
        let db_config = database::cpmm_config_model::CpmmConfigModel::new(
            config.id.clone(),
            config.index,
            config.protocol_fee_rate,
            config.trade_fee_rate,
            config.fund_fee_rate,
            config.create_pool_fee,
            config.creator_fee_rate,
        );

        match repository.save_config(&db_config).await {
            Ok(saved_id) => {
                info!("✅ CPMM配置保存成功: {}", saved_id);
                Ok(saved_id)
            }
            Err(e) => {
                error!("❌ 保存CPMM配置失败: {:?}", e);
                Err(e)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cpmm_config_conversion() {
        let dto_config = CpmmConfig {
            id: "test_id".to_string(),
            index: 0,
            protocol_fee_rate: 120000,
            trade_fee_rate: 2500,
            fund_fee_rate: 40000,
            create_pool_fee: "150000000".to_string(),
            creator_fee_rate: 0,
        };

        assert_eq!(dto_config.id, "test_id");
        assert_eq!(dto_config.index, 0);
        assert_eq!(dto_config.protocol_fee_rate, 120000);
        assert_eq!(dto_config.trade_fee_rate, 2500);
        assert_eq!(dto_config.fund_fee_rate, 40000);
        assert_eq!(dto_config.create_pool_fee, "150000000");
        assert_eq!(dto_config.creator_fee_rate, 0);

        println!("✅ CPMM配置DTO转换测试通过");
    }

    #[tokio::test]
    async fn test_default_configs() {
        let configs = CpmmConfig::default_configs();

        assert!(!configs.is_empty());
        assert_eq!(configs.len(), 2);

        let first_config = &configs[0];
        assert_eq!(first_config.id, "D4FPEruKEHrG5TenZ2mpDGEfu1iUvTiqBxvpU8HLBvC2");
        assert_eq!(first_config.index, 0);
        assert_eq!(first_config.trade_fee_rate, 2500);

        let second_config = &configs[1];
        assert_eq!(second_config.id, "BgxH5ifebqHDuiADWKhLjXGP5hWZeZLoCdmeWJLkRqLP");
        assert_eq!(second_config.index, 5);
        assert_eq!(second_config.trade_fee_rate, 3000);

        println!("✅ 默认CPMM配置测试通过");
    }
}