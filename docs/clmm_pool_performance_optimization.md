# CLMM池子元数据存储系统性能优化指南

## 概述

本文档提供了CLMM池子元数据存储系统的性能优化建议和最佳实践，涵盖数据库优化、查询优化、同步优化和系统监控等方面。

## 数据库优化

### MongoDB索引策略

#### 1. 复合索引设计
```javascript
// 主要查询索引
db.clmm_pools.createIndex({ "pool_address": 1 }, { unique: true })
db.clmm_pools.createIndex({ "mint0.mint_address": 1, "mint1.mint_address": 1 })
db.clmm_pools.createIndex({ "creator_wallet": 1, "created_at": -1 })
db.clmm_pools.createIndex({ "status": 1, "created_at": -1 })

// 价格范围查询索引
db.clmm_pools.createIndex({ 
    "price_info.current_price": 1, 
    "status": 1, 
    "created_at": -1 
})

// 同步状态索引
db.clmm_pools.createIndex({ 
    "sync_status.needs_sync": 1, 
    "sync_status.last_sync_at": 1 
})

// 时间范围查询索引
db.clmm_pools.createIndex({ "created_at": -1 })
db.clmm_pools.createIndex({ "updated_at": -1 })
```

#### 2. 部分索引优化
```javascript
// 只为需要同步的池子创建索引
db.clmm_pools.createIndex(
    { "sync_status.last_sync_at": 1 },
    { partialFilterExpression: { "sync_status.needs_sync": true } }
)

// 只为活跃池子创建价格索引
db.clmm_pools.createIndex(
    { "price_info.current_price": 1 },
    { partialFilterExpression: { "status": "Active" } }
)
```

#### 3. 文本搜索索引
```javascript
// 为代币符号和名称创建文本索引
db.clmm_pools.createIndex({
    "mint0.symbol": "text",
    "mint0.name": "text",
    "mint1.symbol": "text",
    "mint1.name": "text"
})
```

### 连接池优化

```rust
// MongoDB连接池配置
let client_options = ClientOptions::parse(&mongodb_uri).await?
    .with_max_pool_size(Some(50))           // 最大连接数
    .with_min_pool_size(Some(5))            // 最小连接数
    .with_max_idle_time(Some(Duration::from_secs(300))) // 空闲超时
    .with_connect_timeout(Some(Duration::from_secs(10))) // 连接超时
    .with_server_selection_timeout(Some(Duration::from_secs(5))); // 服务器选择超时
```

## 查询优化

### 1. 分页查询优化

```rust
// 使用游标分页替代偏移分页
pub async fn get_pools_cursor_paginated(
    &self,
    last_id: Option<&str>,
    limit: i64
) -> AppResult<Vec<ClmmPool>> {
    let mut filter = doc! {};
    
    // 使用_id作为游标
    if let Some(id) = last_id {
        filter.insert("_id", doc! { "$gt": ObjectId::parse_str(id)? });
    }
    
    let options = FindOptions::builder()
        .limit(limit)
        .sort(doc! { "_id": 1 })
        .build();
    
    let cursor = self.collection.find(filter, options).await?;
    let pools: Vec<ClmmPool> = cursor.try_collect().await?;
    
    Ok(pools)
}
```

### 2. 聚合查询优化

```rust
// 使用聚合管道进行复杂统计
pub async fn get_pool_statistics_optimized(&self) -> AppResult<PoolStats> {
    let pipeline = vec![
        doc! {
            "$group": {
                "_id": "$status",
                "count": { "$sum": 1 },
                "avg_price": { "$avg": "$price_info.current_price" },
                "total_volume": { "$sum": "$volume_24h" }
            }
        },
        doc! {
            "$group": {
                "_id": null,
                "total_pools": { "$sum": "$count" },
                "status_breakdown": {
                    "$push": {
                        "status": "$_id",
                        "count": "$count",
                        "avg_price": "$avg_price",
                        "total_volume": "$total_volume"
                    }
                }
            }
        }
    ];
    
    let mut cursor = self.collection.aggregate(pipeline, None).await?;
    // 处理聚合结果...
}
```

### 3. 投影优化

```rust
// 只查询需要的字段
pub async fn get_pool_summary(&self, pool_address: &str) -> AppResult<PoolSummary> {
    let filter = doc! { "pool_address": pool_address };
    let projection = doc! {
        "pool_address": 1,
        "mint0.mint_address": 1,
        "mint1.mint_address": 1,
        "price_info.current_price": 1,
        "status": 1,
        "created_at": 1
    };
    
    let options = FindOneOptions::builder()
        .projection(projection)
        .build();
    
    let result = self.collection.find_one(filter, options).await?;
    // 转换为PoolSummary...
}
```

## 同步优化

### 1. 批量同步策略

```rust
// 优化的批量同步实现
impl ClmmPoolSyncService {
    pub async fn sync_pools_batch_optimized(&self) -> AppResult<u64> {
        const BATCH_SIZE: usize = 20;
        const CONCURRENT_LIMIT: usize = 5;
        
        let pools = self.storage.get_pools_need_sync(Some(BATCH_SIZE as i64)).await?;
        
        if pools.is_empty() {
            return Ok(0);
        }
        
        // 使用信号量限制并发数
        let semaphore = Arc::new(Semaphore::new(CONCURRENT_LIMIT));
        let mut tasks = Vec::new();
        
        for pool in pools {
            let semaphore = semaphore.clone();
            let storage = self.storage.clone();
            let shared = self.shared.clone();
            
            let task = tokio::spawn(async move {
                let _permit = semaphore.acquire().await.unwrap();
                Self::sync_single_pool_with_context(&storage, &shared, &pool).await
            });
            
            tasks.push(task);
        }
        
        // 等待所有任务完成
        let results = futures::future::join_all(tasks).await;
        
        let mut success_count = 0;
        for result in results {
            match result {
                Ok(Ok(true)) => success_count += 1,
                Ok(Ok(false)) => {}, // 无需更新
                Ok(Err(e)) => error!("同步失败: {}", e),
                Err(e) => error!("任务执行失败: {}", e),
            }
        }
        
        Ok(success_count)
    }
}
```

### 2. 智能同步调度

```rust
// 基于优先级的同步调度
#[derive(Debug, Clone)]
pub struct SyncPriority {
    pub pool_address: String,
    pub priority: u8, // 0-255, 数值越大优先级越高
    pub last_sync_at: u64,
    pub sync_failures: u32,
}

impl ClmmPoolSyncService {
    pub async fn get_pools_by_priority(&self, limit: i64) -> AppResult<Vec<SyncPriority>> {
        let pipeline = vec![
            doc! {
                "$match": {
                    "sync_status.needs_sync": true
                }
            },
            doc! {
                "$addFields": {
                    "priority": {
                        "$add": [
                            // 基础优先级
                            50,
                            // 活跃池子优先级更高
                            { "$cond": [{ "$eq": ["$status", "Active"] }, 30, 0] },
                            // 同步失败次数越多优先级越低
                            { "$multiply": ["$sync_status.sync_failures", -5] },
                            // 距离上次同步时间越长优先级越高
                            { "$divide": [
                                { "$subtract": [{ "$toLong": "$$NOW" }, "$sync_status.last_sync_at"] },
                                3600000 // 每小时增加1点优先级
                            ]}
                        ]
                    }
                }
            },
            doc! {
                "$sort": { "priority": -1 }
            },
            doc! {
                "$limit": limit
            },
            doc! {
                "$project": {
                    "pool_address": 1,
                    "priority": 1,
                    "sync_status.last_sync_at": 1,
                    "sync_status.sync_failures": 1
                }
            }
        ];
        
        // 执行聚合查询...
    }
}
```

## 缓存策略

### 1. Redis缓存集成

```rust
use redis::{Client, Commands, Connection};

pub struct CacheService {
    redis_client: Client,
}

impl CacheService {
    pub async fn get_pool_cached(&self, pool_address: &str) -> AppResult<Option<ClmmPool>> {
        let mut conn = self.redis_client.get_connection()?;
        let cache_key = format!("pool:{}", pool_address);
        
        // 尝试从缓存获取
        let cached_data: Option<String> = conn.get(&cache_key)?;
        
        if let Some(data) = cached_data {
            let pool: ClmmPool = serde_json::from_str(&data)?;
            return Ok(Some(pool));
        }
        
        Ok(None)
    }
    
    pub async fn set_pool_cache(&self, pool: &ClmmPool, ttl_seconds: usize) -> AppResult<()> {
        let mut conn = self.redis_client.get_connection()?;
        let cache_key = format!("pool:{}", pool.pool_address);
        let data = serde_json::to_string(pool)?;
        
        conn.set_ex(&cache_key, data, ttl_seconds)?;
        Ok(())
    }
}
```

### 2. 内存缓存

```rust
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

pub struct MemoryCache<T> {
    data: Arc<RwLock<HashMap<String, CacheEntry<T>>>>,
    ttl: Duration,
}

struct CacheEntry<T> {
    value: T,
    created_at: Instant,
}

impl<T: Clone> MemoryCache<T> {
    pub fn new(ttl: Duration) -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::new())),
            ttl,
        }
    }
    
    pub fn get(&self, key: &str) -> Option<T> {
        let data = self.data.read().unwrap();
        
        if let Some(entry) = data.get(key) {
            if entry.created_at.elapsed() < self.ttl {
                return Some(entry.value.clone());
            }
        }
        
        None
    }
    
    pub fn set(&self, key: String, value: T) {
        let mut data = self.data.write().unwrap();
        data.insert(key, CacheEntry {
            value,
            created_at: Instant::now(),
        });
    }
}
```

## 监控和指标

### 1. 性能指标收集

```rust
use prometheus::{Counter, Histogram, Gauge, register_counter, register_histogram, register_gauge};

pub struct MetricsCollector {
    pub pool_creation_counter: Counter,
    pub pool_query_duration: Histogram,
    pub sync_duration: Histogram,
    pub active_pools_gauge: Gauge,
    pub sync_queue_size_gauge: Gauge,
}

impl MetricsCollector {
    pub fn new() -> AppResult<Self> {
        Ok(Self {
            pool_creation_counter: register_counter!(
                "clmm_pool_creations_total",
                "Total number of CLMM pools created"
            )?,
            pool_query_duration: register_histogram!(
                "clmm_pool_query_duration_seconds",
                "Duration of pool queries in seconds"
            )?,
            sync_duration: register_histogram!(
                "clmm_pool_sync_duration_seconds",
                "Duration of pool synchronization in seconds"
            )?,
            active_pools_gauge: register_gauge!(
                "clmm_active_pools",
                "Number of active CLMM pools"
            )?,
            sync_queue_size_gauge: register_gauge!(
                "clmm_sync_queue_size",
                "Number of pools waiting for synchronization"
            )?,
        })
    }
}
```

### 2. 健康检查端点

```rust
use axum::{Json, response::Json as ResponseJson};
use serde_json::{json, Value};

pub async fn health_check(
    storage: &ClmmPoolStorageService,
    sync: &ClmmPoolSyncService
) -> ResponseJson<Value> {
    let mut health_status = json!({
        "status": "healthy",
        "timestamp": chrono::Utc::now().timestamp(),
        "services": {}
    });
    
    // 检查数据库连接
    match storage.get_pool_statistics().await {
        Ok(stats) => {
            health_status["services"]["database"] = json!({
                "status": "healthy",
                "total_pools": stats.total_pools,
                "active_pools": stats.active_pools
            });
        }
        Err(e) => {
            health_status["services"]["database"] = json!({
                "status": "unhealthy",
                "error": e.to_string()
            });
            health_status["status"] = json!("unhealthy");
        }
    }
    
    // 检查同步服务
    match sync.get_sync_stats().await {
        Ok(stats) => {
            health_status["services"]["sync"] = json!({
                "status": "healthy",
                "pools_need_sync": stats.total_pools_need_sync,
                "last_sync_time": stats.last_sync_time
            });
        }
        Err(e) => {
            health_status["services"]["sync"] = json!({
                "status": "unhealthy",
                "error": e.to_string()
            });
            health_status["status"] = json!("unhealthy");
        }
    }
    
    ResponseJson(health_status)
}
```

## 部署优化

### 1. 数据库分片策略

```javascript
// MongoDB分片配置
sh.enableSharding("clmm_database")

// 基于池子地址进行分片
sh.shardCollection(
    "clmm_database.clmm_pools",
    { "pool_address": "hashed" }
)

// 基于创建时间进行分片（适合时间序列查询）
sh.shardCollection(
    "clmm_database.clmm_pools",
    { "created_at": 1 }
)
```

### 2. 读写分离

```rust
pub struct DatabaseCluster {
    primary: Database,
    secondaries: Vec<Database>,
    read_preference: ReadPreference,
}

impl DatabaseCluster {
    pub async fn read_operation<T>(&self, operation: impl Fn(&Database) -> T) -> T {
        // 根据负载选择读副本
        let db = self.select_read_replica();
        operation(db)
    }
    
    pub async fn write_operation<T>(&self, operation: impl Fn(&Database) -> T) -> T {
        // 写操作总是使用主库
        operation(&self.primary)
    }
    
    fn select_read_replica(&self) -> &Database {
        // 简单的轮询策略
        let index = rand::random::<usize>() % (self.secondaries.len() + 1);
        if index == 0 {
            &self.primary
        } else {
            &self.secondaries[index - 1]
        }
    }
}
```

## 性能基准测试

### 1. 基准测试框架

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn benchmark_pool_queries(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let storage = rt.block_on(async {
        // 初始化存储服务
        setup_test_storage().await
    });
    
    c.bench_function("pool_query_by_address", |b| {
        b.to_async(&rt).iter(|| async {
            let result = storage.get_pool_by_address(
                black_box("test_pool_address")
            ).await;
            black_box(result)
        })
    });
    
    c.bench_function("pool_query_batch", |b| {
        b.to_async(&rt).iter(|| async {
            let params = PoolQueryParams {
                limit: Some(100),
                ..Default::default()
            };
            let result = storage.query_pools(black_box(&params)).await;
            black_box(result)
        })
    });
}

criterion_group!(benches, benchmark_pool_queries);
criterion_main!(benches);
```

### 2. 负载测试

```rust
use tokio::time::{interval, Duration};
use std::sync::atomic::{AtomicU64, Ordering};

pub async fn load_test_concurrent_queries(
    storage: &ClmmPoolStorageService,
    concurrent_users: usize,
    duration_seconds: u64
) -> LoadTestResult {
    let success_count = Arc::new(AtomicU64::new(0));
    let error_count = Arc::new(AtomicU64::new(0));
    let total_response_time = Arc::new(AtomicU64::new(0));
    
    let mut handles = Vec::new();
    
    for _ in 0..concurrent_users {
        let storage = storage.clone();
        let success_count = success_count.clone();
        let error_count = error_count.clone();
        let total_response_time = total_response_time.clone();
        
        let handle = tokio::spawn(async move {
            let mut interval = interval(Duration::from_millis(100));
            let end_time = Instant::now() + Duration::from_secs(duration_seconds);
            
            while Instant::now() < end_time {
                interval.tick().await;
                
                let start = Instant::now();
                match storage.get_pool_statistics().await {
                    Ok(_) => {
                        success_count.fetch_add(1, Ordering::Relaxed);
                        let duration = start.elapsed().as_millis() as u64;
                        total_response_time.fetch_add(duration, Ordering::Relaxed);
                    }
                    Err(_) => {
                        error_count.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
        });
        
        handles.push(handle);
    }
    
    // 等待所有任务完成
    futures::future::join_all(handles).await;
    
    let success = success_count.load(Ordering::Relaxed);
    let errors = error_count.load(Ordering::Relaxed);
    let total_time = total_response_time.load(Ordering::Relaxed);
    
    LoadTestResult {
        total_requests: success + errors,
        successful_requests: success,
        failed_requests: errors,
        average_response_time_ms: if success > 0 { total_time / success } else { 0 },
        requests_per_second: success as f64 / duration_seconds as f64,
    }
}
```

## 总结

通过实施以上优化策略，CLMM池子元数据存储系统可以实现：

1. **查询性能提升**: 通过合理的索引设计和查询优化，查询响应时间可降低80%以上
2. **同步效率提升**: 通过批量处理和并发控制，同步效率可提升5-10倍
3. **系统可扩展性**: 通过分片和读写分离，支持更大规模的数据和并发访问
4. **系统可靠性**: 通过监控和健康检查，及时发现和解决性能问题

建议根据实际业务需求和系统负载情况，逐步实施这些优化措施，并持续监控系统性能指标。