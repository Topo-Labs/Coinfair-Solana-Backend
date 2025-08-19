//! Position Utils 性能测试用例
//!
//! 用于对比原版本和优化版本的性能差异

use crate::solana::{PositionPerformanceStats, PositionUtils, PositionUtilsOptimized};
use anyhow::Result;
use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Instant;
use tracing::info;

/// 性能基准测试结构
#[allow(dead_code)]
pub struct PositionUtilsBenchmark<'a> {
    rpc_client: &'a RpcClient,
    original_utils: PositionUtils<'a>,
    optimized_utils: PositionUtilsOptimized<'a>,
    optimized_stats: Arc<PositionPerformanceStats>,
    test_wallets: Vec<Pubkey>,
}

impl<'a> PositionUtilsBenchmark<'a> {
    pub fn new(rpc_client: &'a RpcClient) -> Self {
        let optimized_stats = Arc::new(PositionPerformanceStats::default());
        Self {
            rpc_client,
            original_utils: PositionUtils::new(rpc_client),
            optimized_utils: PositionUtilsOptimized::with_stats(rpc_client, optimized_stats.clone()),
            optimized_stats,
            test_wallets: Self::generate_test_wallets(),
        }
    }

    fn generate_test_wallets() -> Vec<Pubkey> {
        vec![
            // 默认测试钱包地址
            Pubkey::from_str("8S2bcP66WehuF6cHryfZ7vfFpQWaUhYyAYSy5U3gX4Fy").unwrap(),
            // 可以添加更多测试地址
        ]
    }

    /// 对比单个查询性能
    pub async fn benchmark_single_query(&self, user_wallet: &Pubkey) -> Result<BenchmarkComparison> {
        info!("🔬 开始单个查询性能对比测试");
        info!("  测试钱包: {}", user_wallet);

        // 测试原版本
        let original_start = Instant::now();
        let original_result = self.original_utils.get_user_position_nfts(user_wallet).await;
        let original_time = original_start.elapsed();

        // 测试优化版本
        let optimized_start = Instant::now();
        let optimized_result = self.optimized_utils.get_user_position_nfts_optimized(user_wallet).await;
        let optimized_time = optimized_start.elapsed();

        let (original_nfts, optimized_nfts) = match (original_result, optimized_result) {
            (Ok(orig), Ok(opt)) => (orig, opt),
            (Err(e), _) => return Err(anyhow::anyhow!("原版本查询失败: {:?}", e)),
            (_, Err(e)) => return Err(anyhow::anyhow!("优化版本查询失败: {:?}", e)),
        };

        // 验证结果一致性
        if original_nfts.len() != optimized_nfts.len() {
            return Err(anyhow::anyhow!(
                "结果不一致：原版本找到{}个NFT，优化版本找到{}个NFT",
                original_nfts.len(),
                optimized_nfts.len()
            ));
        }

        let speedup = if optimized_time.as_millis() > 0 {
            original_time.as_millis() as f64 / optimized_time.as_millis() as f64
        } else {
            f64::INFINITY
        };

        Ok(BenchmarkComparison {
            original_time_ms: original_time.as_millis() as u64,
            optimized_time_ms: optimized_time.as_millis() as u64,
            nfts_found: original_nfts.len(),
            speedup_factor: speedup,
            success: true,
        })
    }

    /// 对比批量查询性能
    pub async fn benchmark_batch_queries(&self, batch_size: usize) -> Result<BatchBenchmarkComparison> {
        info!("🔬 开始批量查询性能对比测试");
        info!("  批量大小: {}", batch_size);

        let test_wallets = &self.test_wallets[..batch_size.min(self.test_wallets.len())];
        let mut comparisons = Vec::new();

        for wallet in test_wallets {
            match self.benchmark_single_query(wallet).await {
                Ok(comparison) => comparisons.push(comparison),
                Err(e) => {
                    info!("  查询失败: {:?}", e);
                    comparisons.push(BenchmarkComparison {
                        original_time_ms: 0,
                        optimized_time_ms: 0,
                        nfts_found: 0,
                        speedup_factor: 0.0,
                        success: false,
                    });
                }
            }
        }

        let successful_comparisons: Vec<_> = comparisons.iter().filter(|c| c.success).collect();
        let total_original_time: u64 = successful_comparisons.iter().map(|c| c.original_time_ms).sum();
        let total_optimized_time: u64 = successful_comparisons.iter().map(|c| c.optimized_time_ms).sum();
        let total_nfts: usize = successful_comparisons.iter().map(|c| c.nfts_found).sum();

        let average_speedup = if !successful_comparisons.is_empty() {
            successful_comparisons.iter().map(|c| c.speedup_factor).sum::<f64>() / successful_comparisons.len() as f64
        } else {
            0.0
        };

        Ok(BatchBenchmarkComparison {
            batch_size: test_wallets.len(),
            successful_queries: successful_comparisons.len(),
            total_original_time_ms: total_original_time,
            total_optimized_time_ms: total_optimized_time,
            total_nfts_found: total_nfts,
            average_speedup: average_speedup,
            individual_results: comparisons,
        })
    }

    /// 对比查找存在位置的性能
    pub async fn benchmark_find_existing_position(
        &self,
        user_wallet: &Pubkey,
        pool_address: &Pubkey,
        tick_lower: i32,
        tick_upper: i32,
    ) -> Result<FindPositionComparison> {
        info!("🔬 开始查找存在位置性能对比测试");
        info!("  用户钱包: {}", user_wallet);
        info!("  池子地址: {}", pool_address);
        info!("  Tick范围: {} - {}", tick_lower, tick_upper);

        // 测试原版本
        let original_start = Instant::now();
        let original_result = self
            .original_utils
            .find_existing_position(user_wallet, pool_address, tick_lower, tick_upper)
            .await;
        let original_time = original_start.elapsed();

        // 测试优化版本
        let optimized_start = Instant::now();
        let optimized_result = self
            .optimized_utils
            .find_existing_position_optimized(user_wallet, pool_address, tick_lower, tick_upper)
            .await;
        let optimized_time = optimized_start.elapsed();

        let (original_found, optimized_found) = match (original_result, optimized_result) {
            (Ok(orig), Ok(opt)) => (orig.is_some(), opt.is_some()),
            (Err(e), _) => return Err(anyhow::anyhow!("原版本查询失败: {:?}", e)),
            (_, Err(e)) => return Err(anyhow::anyhow!("优化版本查询失败: {:?}", e)),
        };

        // 验证结果一致性
        if original_found != optimized_found {
            return Err(anyhow::anyhow!(
                "结果不一致：原版本{}找到位置，优化版本{}找到位置",
                if original_found { "" } else { "未" },
                if optimized_found { "" } else { "未" }
            ));
        }

        let speedup = if optimized_time.as_millis() > 0 {
            original_time.as_millis() as f64 / optimized_time.as_millis() as f64
        } else {
            f64::INFINITY
        };

        Ok(FindPositionComparison {
            original_time_ms: original_time.as_millis() as u64,
            optimized_time_ms: optimized_time.as_millis() as u64,
            position_found: original_found,
            speedup_factor: speedup,
            success: true,
        })
    }

    /// 生成完整的性能报告
    pub async fn generate_performance_report(&self) -> Result<String> {
        let mut report = String::new();
        report.push_str("=== Position Utils 性能对比测试报告 ===\n\n");

        // 单查询测试
        if let Some(test_wallet) = self.test_wallets.first() {
            match self.benchmark_single_query(test_wallet).await {
                Ok(single_result) => {
                    report.push_str(&format!(
                        "1. 单查询性能对比:\n\
                         - 原版本时间: {}ms\n\
                         - 优化版本时间: {}ms\n\
                         - 性能提升: {:.2}倍\n\
                         - 找到NFT数: {}\n\
                         - 状态: {}\n\n",
                        single_result.original_time_ms,
                        single_result.optimized_time_ms,
                        single_result.speedup_factor,
                        single_result.nfts_found,
                        if single_result.success { "成功" } else { "失败" }
                    ));
                }
                Err(e) => {
                    report.push_str(&format!("1. 单查询性能对比: 测试失败 - {:?}\n\n", e));
                }
            }
        }

        // 批量查询测试
        let batch_size = self.test_wallets.len().min(3);
        match self.benchmark_batch_queries(batch_size).await {
            Ok(batch_result) => {
                report.push_str(&format!(
                    "2. 批量查询性能对比 ({}个查询):\n\
                     - 原版本总时间: {}ms\n\
                     - 优化版本总时间: {}ms\n\
                     - 平均性能提升: {:.2}倍\n\
                     - 成功查询数: {}/{}\n\
                     - 总找到NFT数: {}\n\n",
                    batch_result.batch_size,
                    batch_result.total_original_time_ms,
                    batch_result.total_optimized_time_ms,
                    batch_result.average_speedup,
                    batch_result.successful_queries,
                    batch_result.batch_size,
                    batch_result.total_nfts_found
                ));
            }
            Err(e) => {
                report.push_str(&format!("2. 批量查询性能对比: 测试失败 - {:?}\n\n", e));
            }
        }

        // 查找位置测试
        if let Some(test_wallet) = self.test_wallets.first() {
            let test_pool = Pubkey::default(); // 使用默认池子地址进行测试
            match self
                .benchmark_find_existing_position(test_wallet, &test_pool, -1000, 1000)
                .await
            {
                Ok(find_result) => {
                    report.push_str(&format!(
                        "3. 查找位置性能对比:\n\
                         - 原版本时间: {}ms\n\
                         - 优化版本时间: {}ms\n\
                         - 性能提升: {:.2}倍\n\
                         - 找到位置: {}\n\
                         - 状态: {}\n\n",
                        find_result.original_time_ms,
                        find_result.optimized_time_ms,
                        find_result.speedup_factor,
                        if find_result.position_found { "是" } else { "否" },
                        if find_result.success { "成功" } else { "失败" }
                    ));
                }
                Err(e) => {
                    report.push_str(&format!("3. 查找位置性能对比: 测试失败 - {:?}\n\n", e));
                }
            }
        }

        // 优化版本统计
        if let Some(stats) = self.optimized_utils.get_performance_stats() {
            report.push_str("4. 优化版本详细统计:\n");
            report.push_str(&stats);
            report.push_str("\n\n");
        }

        // 性能改进总结
        report.push_str("5. 优化效果总结:\n");
        report.push_str("✅ 实现的优化:\n");
        report.push_str("  - 批量RPC调用：减少网络往返次数\n");
        report.push_str("  - 并发处理：同时获取多种Token类型的NFT\n");
        report.push_str("  - 智能过滤：预过滤潜在NFT账户\n");
        report.push_str("  - 内存优化：使用紧凑数据结构\n");
        report.push_str("  - 性能监控：完整的统计和监控\n");
        report.push_str("📊 关键指标:\n");
        report.push_str(&format!(
            "  - 缓存命中率: {:.1}%\n",
            self.optimized_stats.get_cache_hit_rate() * 100.0
        ));
        report.push_str(&format!(
            "  - 平均RPC响应时间: {:.1}ms\n",
            self.optimized_stats.get_average_rpc_duration()
        ));
        report.push_str(&format!(
            "  - 内存节省: {:.2}MB\n",
            self.optimized_stats
                .memory_saved_bytes
                .load(std::sync::atomic::Ordering::Relaxed) as f64
                / 1024.0
                / 1024.0
        ));

        Ok(report)
    }
}

#[derive(Debug)]
pub struct BenchmarkComparison {
    pub original_time_ms: u64,
    pub optimized_time_ms: u64,
    pub nfts_found: usize,
    pub speedup_factor: f64,
    pub success: bool,
}

#[derive(Debug)]
pub struct BatchBenchmarkComparison {
    pub batch_size: usize,
    pub successful_queries: usize,
    pub total_original_time_ms: u64,
    pub total_optimized_time_ms: u64,
    pub total_nfts_found: usize,
    pub average_speedup: f64,
    pub individual_results: Vec<BenchmarkComparison>,
}

#[derive(Debug)]
pub struct FindPositionComparison {
    pub original_time_ms: u64,
    pub optimized_time_ms: u64,
    pub position_found: bool,
    pub speedup_factor: f64,
    pub success: bool,
}

/// 便捷的性能测试函数
pub async fn run_position_utils_benchmark(rpc_endpoint: &str) -> Result<String> {
    let rpc_client = RpcClient::new(rpc_endpoint.to_string());
    let benchmark = PositionUtilsBenchmark::new(&rpc_client);
    benchmark.generate_performance_report().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_benchmark_creation() {
        let rpc_client = RpcClient::new("https://api.devnet.solana.com".to_string());
        let benchmark = PositionUtilsBenchmark::new(&rpc_client);

        assert!(!benchmark.test_wallets.is_empty());
        assert!(benchmark.optimized_utils.get_performance_stats().is_some());
    }

    #[tokio::test]
    async fn test_benchmark_comparison_structure() {
        let comparison = BenchmarkComparison {
            original_time_ms: 1000,
            optimized_time_ms: 100,
            nfts_found: 5,
            speedup_factor: 10.0,
            success: true,
        };

        assert_eq!(comparison.speedup_factor, 10.0);
        assert!(comparison.success);
    }
}
