# MongoDB聚合查询陷阱和最佳实践

## ⚠️ 重要发现：$count操作符兼容性问题

### 问题描述

在开发DepositEvent API的统计功能时，发现使用MongoDB的`$count`聚合操作符在某些环境下会返回错误结果（返回0而不是实际计数），导致统计查询失败。

### 问题复现

```rust
// ❌ 有问题的代码 - $count操作符可能不工作
let unique_users_pipeline = vec![
    doc! { "$group": { "_id": "$user" } },
    doc! { "$count": "unique_users" }  // 在某些MongoDB版本/环境中返回错误结果
];
let mut cursor = collection.aggregate(unique_users_pipeline, None).await?;
let unique_users = if let Some(doc) = cursor.try_next().await? {
    doc.get_i64("unique_users").unwrap_or(0) as u64  // 可能总是返回0
} else {
    0
};
```

**测试环境表现**：

- 数据库中确实有7条记录
- 手动查询显示有1个独特用户和1个独特代币
- 但`$count`聚合返回`unique_users=0, unique_tokens=0`

### 解决方案

使用标准的`$group` + `$sum`模式，然后手动计数结果：

```rust
// ✅ 推荐的兼容性解决方案
let unique_users_pipeline = vec![
    doc! { "$group": { "_id": "$user", "count": { "$sum": 1 } } }
];
let mut cursor = collection.aggregate(unique_users_pipeline, None).await?;
let mut unique_users = 0u64;
while let Some(_doc) = cursor.try_next().await? {
    unique_users += 1;  // 手动计数，兼容性更好
}
```

### 根本原因分析

1. **版本兼容性**：`$count`操作符在某些MongoDB版本中行为不一致
2. **环境差异**：测试环境、Docker环境、不同的MongoDB驱动版本可能有不同表现
3. **复杂管道**：在多阶段聚合管道中，`$count`可能受到前面阶段的影响

### 最佳实践建议

#### ✅ 推荐做法

1. **使用$sum进行计数**：

   ```rust
   doc! { "$group": { "_id": "$field", "count": { "$sum": 1 } } }
   ```

2. **客户端手动计数**：

   ```rust
   let mut count = 0u64;
   while let Some(_) = cursor.try_next().await? {
       count += 1;
   }
   ```

3. **简单查询优先**：

   ```rust
   // 对于简单计数，直接使用count_documents
   let total = collection.count_documents(filter, None).await?;
   ```

#### ❌ 避免做法

1. **依赖$count操作符**：特别是在生产环境中
2. **复杂的聚合管道**：除非确实需要，避免过度复杂的管道
3. **假设操作符行为一致**：不同MongoDB版本可能有差异

### 其他聚合陷阱

1. **$lookup性能**：在大数据集上可能很慢，考虑应用层join
2. **$unwind内存使用**：可能导致内存溢出，使用allowDiskUse
3. **索引优化**：确保聚合管道的第一阶段能使用索引

### 兼容性测试建议

```rust
#[cfg(test)]
mod aggregation_compatibility_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_count_vs_sum_aggregation() {
        // 插入测试数据
        // 同时测试$count和$sum方法
        // 验证结果一致性
        assert_eq!(count_result, sum_result, 
                  "$count和$sum应该返回相同的结果");
    }
}
```

### 教训总结

- **不要回避问题**：发现聚合查询异常时，要深入调查而不是简化测试
- **兼容性优先**：选择更兼容的实现方式，而不是最新的语法糖
- **充分测试**：在不同环境下测试聚合查询的行为
- **监控生产环境**：统计查询结果异常可能是聚合操作符问题的信号

---
*记录时间：2025-08-29*  
*发现于：DepositEvent API开发过程中*  
*影响范围：所有使用MongoDB聚合查询的统计功能*
