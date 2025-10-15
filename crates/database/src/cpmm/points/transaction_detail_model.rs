use chrono::{DateTime, Utc};
use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};

/// 自定义日期时间反序列化器，兼容字符串和MongoDB日期对象格式
mod flexible_datetime {
    use chrono::{DateTime, Utc};
    use mongodb::bson;
    use serde::{Deserialize, Deserializer};

    pub fn deserialize<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = bson::Bson::deserialize(deserializer)?;

        match value {
            // 处理字符串格式的日期时间
            bson::Bson::String(s) => s
                .parse::<DateTime<Utc>>()
                .map_err(|e| serde::de::Error::custom(format!("Failed to parse datetime string '{}': {}", s, e))),
            // 处理MongoDB BSON日期对象格式
            bson::Bson::DateTime(dt) => Ok(DateTime::<Utc>::from_timestamp_millis(dt.timestamp_millis())
                .ok_or_else(|| serde::de::Error::custom("Invalid timestamp"))?),
            // 处理其他可能的格式
            other => Err(serde::de::Error::custom(format!(
                "Expected datetime string or BSON DateTime, found: {:?}",
                other
            ))),
        }
    }
}

/// 用户交易积分详情表模型
///
/// 业务规则：
/// - 首笔交易获得200积分（is_first_transaction=true）
/// - 后续每笔交易获得10积分（is_first_transaction=false）
/// - 唯一主键：user_wallet + signature
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserTransactionPointsDetail {
    /// MongoDB对象ID
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,

    /// 用户钱包地址
    #[serde(rename = "userWallet")]
    pub user_wallet: String,

    /// 交易签名（交易hash）
    pub signature: String,

    /// 是否是在本平台的首笔交易
    /// true: 首笔交易，获得200积分
    /// false: 后续交易，获得10积分
    #[serde(rename = "isFirstTransaction")]
    pub is_first_transaction: bool,

    /// 积分获得数量
    /// 首笔交易：200积分
    /// 后续交易：10积分
    #[serde(rename = "pointsGainedAmount")]
    pub points_gained_amount: u64,

    /// 积分获得时间
    #[serde(rename = "pointsGainedTime", deserialize_with = "flexible_datetime::deserialize")]
    pub points_gained_time: DateTime<Utc>,
}

impl UserTransactionPointsDetail {
    /// 创建首笔交易的积分记录
    pub fn new_first_transaction(user_wallet: String, signature: String) -> Self {
        Self {
            id: None,
            user_wallet,
            signature,
            is_first_transaction: true,
            points_gained_amount: 200, // 首笔交易200积分
            points_gained_time: Utc::now(),
        }
    }

    /// 创建后续交易的积分记录
    pub fn new_subsequent_transaction(user_wallet: String, signature: String) -> Self {
        Self {
            id: None,
            user_wallet,
            signature,
            is_first_transaction: false,
            points_gained_amount: 10, // 后续交易10积分
            points_gained_time: Utc::now(),
        }
    }

    /// 验证数据有效性
    pub fn validate(&self) -> Result<(), String> {
        if self.user_wallet.is_empty() {
            return Err("用户钱包地址不能为空".to_string());
        }

        if self.signature.is_empty() {
            return Err("交易签名不能为空".to_string());
        }

        // 验证积分数量与类型是否一致
        if self.is_first_transaction && self.points_gained_amount != 200 {
            return Err(format!(
                "首笔交易积分应为200，实际为{}",
                self.points_gained_amount
            ));
        }

        if !self.is_first_transaction && self.points_gained_amount != 10 {
            return Err(format!(
                "后续交易积分应为10，实际为{}",
                self.points_gained_amount
            ));
        }

        Ok(())
    }
}

/// 用户交易积分查询参数
#[derive(Debug, Clone, Default)]
pub struct TransactionPointsQuery {
    /// 用户钱包地址过滤
    pub user_wallet: Option<String>,
    /// 是否只查询首笔交易
    pub first_transaction_only: Option<bool>,
    /// 排序字段（points_gained_time等）
    pub sort_by: Option<String>,
    /// 排序方向（asc, desc）
    pub sort_order: Option<String>,
    /// 分页参数
    pub page: Option<i64>,
    pub limit: Option<i64>,
}

/// 用户交易积分统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserTransactionStats {
    /// 用户钱包地址
    pub user_wallet: String,
    /// 总交易次数
    pub total_transactions: u64,
    /// 总积分获得数量
    pub total_points_gained: u64,
    /// 首笔交易时间
    pub first_transaction_time: Option<DateTime<Utc>>,
    /// 最近交易时间
    pub latest_transaction_time: Option<DateTime<Utc>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_first_transaction() {
        let detail = UserTransactionPointsDetail::new_first_transaction(
            "wallet1".to_string(),
            "sig1".to_string(),
        );

        assert_eq!(detail.user_wallet, "wallet1");
        assert_eq!(detail.signature, "sig1");
        assert!(detail.is_first_transaction);
        assert_eq!(detail.points_gained_amount, 200);
        assert!(detail.validate().is_ok());
    }

    #[test]
    fn test_new_subsequent_transaction() {
        let detail = UserTransactionPointsDetail::new_subsequent_transaction(
            "wallet1".to_string(),
            "sig2".to_string(),
        );

        assert_eq!(detail.user_wallet, "wallet1");
        assert_eq!(detail.signature, "sig2");
        assert!(!detail.is_first_transaction);
        assert_eq!(detail.points_gained_amount, 10);
        assert!(detail.validate().is_ok());
    }

    #[test]
    fn test_validate_empty_wallet() {
        let mut detail = UserTransactionPointsDetail::new_first_transaction(
            String::new(),
            "sig1".to_string(),
        );
        detail.user_wallet = String::new();
        assert!(detail.validate().is_err());
    }

    #[test]
    fn test_validate_empty_signature() {
        let mut detail = UserTransactionPointsDetail::new_first_transaction(
            "wallet1".to_string(),
            String::new(),
        );
        detail.signature = String::new();
        assert!(detail.validate().is_err());
    }

    #[test]
    fn test_validate_first_transaction_wrong_points() {
        let mut detail = UserTransactionPointsDetail::new_first_transaction(
            "wallet1".to_string(),
            "sig1".to_string(),
        );
        detail.points_gained_amount = 100; // 错误的积分数量
        assert!(detail.validate().is_err());
    }

    #[test]
    fn test_validate_subsequent_transaction_wrong_points() {
        let mut detail = UserTransactionPointsDetail::new_subsequent_transaction(
            "wallet1".to_string(),
            "sig1".to_string(),
        );
        detail.points_gained_amount = 20; // 错误的积分数量
        assert!(detail.validate().is_err());
    }
}
