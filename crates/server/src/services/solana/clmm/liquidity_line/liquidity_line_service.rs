use crate::dtos::solana::clmm::pool::liquidity_line::{
    LiquidityLinePoint, PoolLiquidityLineData, PoolLiquidityLineRequest,
};
use anchor_lang::AccountDeserialize;
use anyhow::{anyhow, Result};
use database::Database;
use raydium_amm_v3::states::{PoolState, TickArrayState};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{account::Account, pubkey::Pubkey};
use std::str::FromStr;
use std::sync::Arc;
use tracing::{info, warn};
use utils::ConfigManager;

const TICK_ARRAY_SIZE: i32 = 60;

/// 流动性线图服务
pub struct LiquidityLineService {
    rpc_client: Arc<RpcClient>,
    _database: Arc<Database>,
}

impl LiquidityLineService {
    pub fn new(rpc_client: Arc<RpcClient>, database: Arc<Database>) -> Self {
        Self {
            rpc_client,
            _database: database,
        }
    }

    /// 获取池子流动性分布线图
    pub async fn get_pool_liquidity_line(&self, request: &PoolLiquidityLineRequest) -> Result<PoolLiquidityLineData> {
        info!("🎯 开始获取流动性线图 - 池子地址: {}", request.id);

        // 1. 验证并解析池子地址
        let pool_address = Pubkey::from_str(&request.id).map_err(|_| anyhow!("无效的池子地址: {}", request.id))?;

        // 2. 获取池子状态
        let pool_state = self.get_pool_state(&pool_address).await?;
        let current_tick = pool_state.tick_current;
        let tick_spacing = pool_state.tick_spacing;
        let current_price = pool_state.current_price;

        info!(
            "📊 池子状态 - 当前tick: {}, tick间距: {}, 当前价格: {}",
            current_tick, tick_spacing, current_price
        );

        // 3. 计算需要查询的tick范围
        let range = request.range.unwrap_or(2000); // 默认查询范围
        let tick_lower = current_tick - range;
        let tick_upper = current_tick + range;

        info!("🔍 查询范围 - tick下限: {}, tick上限: {}", tick_lower, tick_upper);

        // 4. 获取范围内的流动性数据
        let liquidity_points = self
            .collect_liquidity_data(&pool_address, tick_lower, tick_upper, tick_spacing)
            .await?;

        // 5. 转换为响应格式
        let max_points = request.max_points.unwrap_or(100) as usize;
        let filtered_points = self.filter_and_limit_points(liquidity_points, max_points);

        let response_data = PoolLiquidityLineData {
            count: filtered_points.len() as u32,
            line: filtered_points,
        };

        info!("✅ 成功获取流动性线图 - 数据点数: {}", response_data.count);

        Ok(response_data)
    }

    /// 获取池子状态
    async fn get_pool_state(&self, pool_address: &Pubkey) -> Result<PoolStateData> {
        // TODO 首先尝试从数据库获取
        // 注意：这里需要使用Repository模式，但为了简化直接查询
        // 在实际环境中应该使用proper repository pattern

        // 直接从链上获取数据
        info!("📡 从链上获取池子状态...");
        let account = self
            .rpc_client
            .get_account(pool_address)
            .map_err(|e| anyhow!("获取池子账户失败: {}", e))?;

        self.parse_pool_state_from_account(&account)
    }

    /// 从账户数据解析池子状态
    fn parse_pool_state_from_account(&self, account: &Account) -> Result<PoolStateData> {
        // 使用真正的Raydium CLMM池子状态解析
        let pool_state: PoolState = self.deserialize_anchor_account(account)?;

        // 复制packed字段到局部变量以避免对齐问题
        let tick_current = pool_state.tick_current;
        let tick_spacing = pool_state.tick_spacing;
        let sqrt_price_x64 = pool_state.sqrt_price_x64;
        let current_price = raydium_amm_v3_client::sqrt_price_x64_to_price(sqrt_price_x64, 0, 0);

        info!(
            "📊 解析池子状态 - 当前tick: {}, tick间距: {}, 当前价格: {}",
            tick_current, tick_spacing, current_price
        );

        Ok(PoolStateData {
            tick_current,
            tick_spacing,
            current_price,
        })
    }

    /// 反序列化anchor账户
    fn deserialize_anchor_account<T: AccountDeserialize>(&self, account: &Account) -> Result<T> {
        let mut data: &[u8] = &account.data;
        T::try_deserialize(&mut data).map_err(|e| anyhow!("反序列化账户失败: {}", e))
    }

    // /// 将sqrt_price_x64转换为价格
    // fn sqrt_price_x64_to_price(&self, sqrt_price_x64: u128) -> Result<f64> {
    //     // sqrt_price_x64 = sqrt(price) * 2^64
    //     let sqrt_price = sqrt_price_x64 as f64 / (1u128 << 64) as f64;
    //     let price = sqrt_price * sqrt_price;
    //     Ok(price)
    // }

    /// 收集指定范围内的流动性数据
    async fn collect_liquidity_data(
        &self,
        pool_address: &Pubkey,
        tick_lower: i32,
        tick_upper: i32,
        tick_spacing: u16,
    ) -> Result<Vec<LiquidityLinePoint>> {
        let mut liquidity_points = Vec::new();

        // 计算需要查询的TickArray起始索引
        let tick_array_starts = self.calculate_tick_array_starts(tick_lower, tick_upper, tick_spacing);

        info!("🔄 需要查询的TickArray数量: {}", tick_array_starts.len());

        for tick_array_start in tick_array_starts {
            match self.get_tick_array_liquidity(pool_address, tick_array_start).await {
                Ok(mut points) => {
                    // 过滤在范围内的tick
                    points.retain(|p| p.tick >= tick_lower && p.tick <= tick_upper);
                    liquidity_points.extend(points);
                }
                Err(e) => {
                    warn!("⚠️ 获取TickArray失败 - 起始索引: {}, 错误: {}", tick_array_start, e);
                    // 继续处理其他TickArray，不因单个失败而停止
                }
            }
        }

        // 按tick排序
        liquidity_points.sort_by(|a, b| a.tick.cmp(&b.tick));

        info!("📈 收集到流动性数据点: {} 个", liquidity_points.len());

        Ok(liquidity_points)
    }

    /// 计算需要查询的TickArray起始索引
    fn calculate_tick_array_starts(&self, tick_lower: i32, tick_upper: i32, tick_spacing: u16) -> Vec<i32> {
        let mut starts = Vec::new();

        // 使用真正的Raydium CLMM计算方法
        let start_index = raydium_amm_v3::states::TickArrayState::get_array_start_index(tick_lower, tick_spacing);
        let end_index = raydium_amm_v3::states::TickArrayState::get_array_start_index(tick_upper, tick_spacing);

        let mut current = start_index;
        while current <= end_index {
            starts.push(current);
            // tick array间距应该使用正确的spacing计算而不是固定的TICK_ARRAY_SIZE
            current =
                raydium_amm_v3::states::TickArrayState::get_array_start_index(current + TICK_ARRAY_SIZE, tick_spacing);
            if current <= end_index {
                // 避免无限循环，如果计算出的current没有增长则退出
                if starts.last() == Some(&current) {
                    break;
                }
            } else {
                break;
            }
        }

        // 确保end_index也被包含
        if !starts.is_empty() && starts.last() != Some(&end_index) && end_index > start_index {
            starts.push(end_index);
        }

        info!(
            "🔢 计算TickArray起始索引: {}..{} => {} 个数组 {:?}",
            start_index,
            end_index,
            starts.len(),
            starts
        );
        starts
    }

    /// 获取单个TickArray的流动性数据
    async fn get_tick_array_liquidity(
        &self,
        pool_address: &Pubkey,
        tick_array_start: i32,
    ) -> Result<Vec<LiquidityLinePoint>> {
        // 计算TickArray的PDA地址
        let tick_array_address = self.calculate_tick_array_address(pool_address, tick_array_start)?;

        // 获取TickArray账户
        match self.rpc_client.get_account(&tick_array_address) {
            Ok(account) => {
                // 解析TickArray数据
                self.parse_tick_array_liquidity(&account, tick_array_start)
            }
            Err(_) => {
                // TickArray不存在，返回空数据
                Ok(Vec::new())
            }
        }
    }

    /// 计算TickArray的PDA地址
    fn calculate_tick_array_address(&self, pool_address: &Pubkey, tick_array_start: i32) -> Result<Pubkey> {
        // 使用Raydium CLMM程序ID和种子计算PDA
        // let raydium_program_id = Pubkey::from_str("CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK")
        //     .map_err(|_| anyhow!("无效的Raydium程序ID"))?;
        let raydium_program_id = ConfigManager::get_raydium_program_id()?;

        let (tick_array_address, _bump) = Pubkey::find_program_address(
            &[
                "tick_array".as_bytes(),
                pool_address.as_ref(),
                &tick_array_start.to_be_bytes(),
            ],
            &raydium_program_id,
        );

        Ok(tick_array_address)
    }

    /// 解析TickArray账户数据，提取流动性信息
    fn parse_tick_array_liquidity(&self, account: &Account, tick_array_start: i32) -> Result<Vec<LiquidityLinePoint>> {
        // 使用真正的Raydium CLMM TickArray状态解析
        let tick_array_state: TickArrayState = self.deserialize_anchor_account(account)?;
        let mut points = Vec::new();

        // 复制packed字段到局部变量以避免对齐问题
        let start_tick_index = tick_array_state.start_tick_index;
        let initialized_tick_count = tick_array_state.initialized_tick_count;

        info!(
            "🔍 解析TickArray - 起始索引: {}, 已初始化tick数: {}",
            start_tick_index, initialized_tick_count
        );

        // 遍历TickArray中的所有tick
        for (i, tick) in tick_array_state.ticks.iter().enumerate() {
            // 检查tick是否有流动性（通过liquidity_gross判断）
            let liquidity_gross = tick.liquidity_gross;
            if liquidity_gross > 0 {
                let tick_index = tick_array_start + (i as i32);

                // 将流动性从u128转换为字符串（保持精度）
                let liquidity = liquidity_gross.to_string();

                let price = self.calculate_price_from_tick(tick_index)?;

                points.push(LiquidityLinePoint {
                    price,
                    liquidity: liquidity.clone(),
                    tick: tick_index,
                });

                info!(
                    "  ✅ 找到流动性点 - tick: {}, 流动性: {}, 价格: {:.8}",
                    tick_index, liquidity, price
                );
            }
        }

        info!("📈 从TickArray提取到 {} 个流动性点", points.len());
        Ok(points)
    }

    /// 根据tick计算价格
    fn calculate_price_from_tick(&self, tick: i32) -> Result<f64> {
        // 使用真正的Raydium tick数学库计算价格
        let sqrt_price_x64 = raydium_amm_v3::libraries::tick_math::get_sqrt_price_at_tick(tick)
            .map_err(|e| anyhow!("tick {}转换为sqrt价格失败: {:?}", tick, e))?;

        // 从sqrt_price_x64转换为价格
        // self.sqrt_price_x64_to_price(sqrt_price_x64)
        let result = raydium_amm_v3_client::sqrt_price_x64_to_price(sqrt_price_x64, 0, 0);
        Ok(result)
    }

    /// 过滤和限制数据点数量
    fn filter_and_limit_points(
        &self,
        mut points: Vec<LiquidityLinePoint>,
        max_points: usize,
    ) -> Vec<LiquidityLinePoint> {
        // 过滤掉流动性为0的点
        points.retain(|p| p.liquidity != "0");

        // 如果数据点太多，需要采样
        if points.len() > max_points {
            let step = points.len() / max_points;
            points = points.into_iter().step_by(step).collect();
        }

        points
    }
}

/// 池子状态数据
#[derive(Debug, Clone)]
struct PoolStateData {
    tick_current: i32,
    tick_spacing: u16,
    current_price: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_calculate_tick_array_starts() {
        let service = create_test_service().await;

        // 测试基本范围计算
        let starts = service.calculate_tick_array_starts(-120, 120, 60);
        assert!(starts.contains(&-120));
        assert!(starts.contains(&0));
        assert!(starts.contains(&60));

        // 测试边界情况
        let starts = service.calculate_tick_array_starts(0, 0, 60);
        assert_eq!(starts, vec![0]);
    }

    #[tokio::test]
    async fn test_calculate_price_from_tick() {
        let service = create_test_service().await;

        // 测试tick 0应该对应价格1
        let price = service.calculate_price_from_tick(0).unwrap();
        assert!((price - 1.0).abs() < 0.0001);

        // 测试正tick
        let price = service.calculate_price_from_tick(1000).unwrap();
        assert!(price > 1.0);

        // 测试负tick
        let price = service.calculate_price_from_tick(-1000).unwrap();
        assert!(price < 1.0);
    }

    async fn create_test_service() -> LiquidityLineService {
        // 创建测试用的服务实例（使用模拟数据）
        let rpc_client = Arc::new(RpcClient::new("https://api.devnet.solana.com".to_string()));

        // 为测试创建简化的AppConfig
        use utils::{AppConfig, CargoEnv};

        let config = Arc::new(AppConfig {
            app_host: "localhost".to_string(),
            app_port: 8000,
            mongo_uri: "mongodb://localhost:27017".to_string(),
            mongo_db: "test_db".to_string(),
            cargo_env: CargoEnv::Development,
            rpc_url: "https://api.devnet.solana.com".to_string(),
            raydium_program_id: "FA1RJDDXysgwg5Gm3fJXWxt26JQzPkAzhTA114miqNUX".to_string(),
            raydium_cp_program_id: "DRaycpLY18LhpbydsBWbVJtxpNv9oXPgjRSfpF2bWpYb".to_string(),
            private_key: None,
            amm_config_index: 0,
            rust_log: "info".to_string(),
            enable_pool_event_insert: false,
            event_listener_db_mode: "update_only".to_string(),
        });

        // 使用正确的方式创建Database
        let database = Arc::new(Database::new(config).await.expect("测试数据库创建失败"));

        LiquidityLineService::new(rpc_client, database)
    }
}
