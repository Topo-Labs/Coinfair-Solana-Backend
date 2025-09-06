use chrono::{DateTime, Utc};
use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};

/// 检查点记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventScannerCheckpoints {
    /// MongoDB ObjectId
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    /// 程序ID（如果是程序级检查点）
    pub program_id: Option<String>,
    /// 事件名称
    pub event_name: Option<String>,
    /// 当前槽位
    pub slot: Option<u64>,
    /// 最后处理的签名
    pub last_signature: Option<String>,
    /// 更新时间
    #[serde(with = "bson_datetime")]
    pub updated_at: DateTime<Utc>,
    /// 创建时间
    #[serde(with = "bson_datetime")]
    pub created_at: DateTime<Utc>,
}

/// 扫描记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanRecords {
    /// MongoDB ObjectId
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    /// 扫描ID（字符串格式，与日志格式一致）
    pub scan_id: String,
    /// 起始槽位
    pub until_slot: Option<u64>,
    /// 结束槽位
    pub before_slot: Option<u64>,
    /// 起始槽位
    pub until_signature: String,
    /// 结束槽位
    pub before_signature: String,
    /// 扫描状态
    pub status: ScanStatus,
    /// 发现事件数
    pub events_found: u64,
    /// 回填事件数
    pub events_backfilled_count: u64,
    /// 回填事件签名列表
    pub events_backfilled_signatures: Vec<String>,
    /// 开始时间
    #[serde(with = "bson_datetime")]
    pub started_at: DateTime<Utc>,
    /// 完成时间
    #[serde(with = "bson_datetime_option", default)]
    pub completed_at: Option<DateTime<Utc>>,
    /// 错误信息（如果有）
    pub error_message: Option<String>,
    /// 处理的程序过滤器
    pub program_filters: Vec<String>,
}

/// 扫描状态枚举
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ScanStatus {
    /// 运行中
    Running,
    /// 已完成
    Completed,
    /// 失败
    Failed,
    /// 已取消
    Cancelled,
}
// MongoDB BSON DateTime 序列化辅助模块
pub(super) mod bson_datetime {
    use chrono::{DateTime, Utc};
    use mongodb::bson;
    use serde::{self, Deserialize, Deserializer, Serialize, Serializer};

    pub(super) fn serialize<S>(date: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // 使用 BSON DateTime，将 chrono::DateTime 转换为毫秒时间戳
        let millis = date.timestamp_millis();
        let bson_dt = bson::DateTime::from_millis(millis);
        bson_dt.serialize(serializer)
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let bson_dt = bson::DateTime::deserialize(deserializer)?;
        // 从毫秒时间戳转换回 chrono::DateTime
        let secs = bson_dt.timestamp_millis() / 1000;
        let nanos = ((bson_dt.timestamp_millis() % 1000) * 1_000_000) as u32;
        Ok(DateTime::from_timestamp(secs, nanos).unwrap_or_else(|| DateTime::from_timestamp(0, 0).unwrap()))
    }
}

pub(super) mod bson_datetime_option {
    use chrono::{DateTime, Utc};
    use mongodb::bson;
    use serde::{self, Deserialize, Deserializer, Serialize, Serializer};

    pub(super) fn serialize<S>(date: &Option<DateTime<Utc>>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match date {
            Some(dt) => {
                let millis = dt.timestamp_millis();
                let bson_dt = bson::DateTime::from_millis(millis);
                bson_dt.serialize(serializer)
            }
            None => serializer.serialize_none(),
        }
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<Option<DateTime<Utc>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let opt: Option<bson::DateTime> = Option::deserialize(deserializer)?;
        Ok(opt.map(|bson_dt| {
            let secs = bson_dt.timestamp_millis() / 1000;
            let nanos = ((bson_dt.timestamp_millis() % 1000) * 1_000_000) as u32;
            DateTime::from_timestamp(secs, nanos).unwrap_or_else(|| DateTime::from_timestamp(0, 0).unwrap())
        }))
    }
}
