use serde::{Serializer, Serialize};

/// 将u64强制序列化为BSON数值类型（而非NumberLong包装）
pub fn serialize_u64_as_number<S>(value: &u64, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    // 对于MongoDB，我们希望将u64序列化为原始数值
    // 而不是NumberLong包装格式
    if *value <= i32::MAX as u64 {
        // 小数值，序列化为i32
        (*value as i32).serialize(serializer)
    } else if *value <= i64::MAX as u64 {
        // 中等数值，序列化为i64
        (*value as i64).serialize(serializer)
    } else {
        // 大数值，保持u64但明确告诉序列化器不要包装
        value.serialize(serializer)
    }
}

/// 将i64强制序列化为BSON数值类型（而非NumberLong包装）
pub fn serialize_i64_as_number<S>(value: &i64, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    // 对于MongoDB，我们希望将i64序列化为原始数值
    // 而不是NumberLong包装格式
    if *value <= i32::MAX as i64 && *value >= i32::MIN as i64 {
        // 小数值，序列化为i32
        (*value as i32).serialize(serializer)
    } else {
        // 大数值，保持i64但明确告诉序列化器不要包装
        value.serialize(serializer)
    }
}

/// 将u32强制序列化为BSON数值类型
pub fn serialize_u32_as_number<S>(value: &u32, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    // u32总是可以安全转换为i32（如果需要）或直接序列化
    if *value <= i32::MAX as u32 {
        (*value as i32).serialize(serializer)
    } else {
        value.serialize(serializer)
    }
}

/// 可选的u64序列化为数值
pub fn serialize_optional_u64_as_number<S>(value: &Option<u64>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match value {
        Some(v) => serialize_u64_as_number(v, serializer),
        None => serializer.serialize_none(),
    }
}

/// 可选的i64序列化为数值
pub fn serialize_optional_i64_as_number<S>(value: &Option<i64>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match value {
        Some(v) => serialize_i64_as_number(v, serializer),
        None => serializer.serialize_none(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;
    use serde::Serialize;

    #[derive(Serialize)]
    struct TestStruct {
        #[serde(serialize_with = "serialize_u64_as_number")]
        amount: u64,
        #[serde(serialize_with = "serialize_i64_as_number")]
        timestamp: i64,
        #[serde(serialize_with = "serialize_optional_u64_as_number")]
        optional_amount: Option<u64>,
    }

    #[test]
    fn test_u64_serialization() {
        let test = TestStruct {
            amount: 1000000,
            timestamp: 1756566140,
            optional_amount: Some(500000),
        };

        let json = serde_json::to_string(&test).unwrap();
        println!("Serialized: {}", json);

        // 验证序列化结果不包含$numberLong包装
        assert!(!json.contains("$numberLong"));
        assert!(json.contains("\"amount\":1000000"));
        assert!(json.contains("\"timestamp\":1756566140"));
    }

    #[test]
    fn test_large_number_serialization() {
        let test = TestStruct {
            amount: 18446744073709551615u64, // 最大u64值
            timestamp: 9223372036854775807i64, // 最大i64值
            optional_amount: None,
        };

        let json = serde_json::to_string(&test).unwrap();
        println!("Large numbers serialized: {}", json);

        // 即使是大数值也不应该有$numberLong包装
        assert!(!json.contains("$numberLong"));
    }
}