use chrono::{DateTime, Utc};
use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};

/// LP变更事件数据模型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LpChangeEvent {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,

    // 用户和池子信息
    pub user_wallet: String,
    pub pool_id: String,
    pub lp_mint: String,
    pub token_0_mint: String,
    pub token_1_mint: String,

    // 变更类型
    pub change_type: u8, // 0: deposit, 1: withdraw, 2: initialize

    // LP数量变化
    pub lp_amount_before: u64,
    pub lp_amount_after: u64,
    pub lp_amount_change: i64, // 可为负数

    // 代币数量
    pub token_0_amount: u64,
    pub token_1_amount: u64,
    pub token_0_transfer_fee: u64,
    pub token_1_transfer_fee: u64,

    // 池子状态
    pub token_0_vault_before: u64,
    pub token_1_vault_before: u64,
    pub token_0_vault_after: u64,
    pub token_1_vault_after: u64,

    // 程序ID和精度
    pub lp_mint_program_id: String,
    pub token_0_program_id: String,
    pub token_1_program_id: String,
    pub lp_mint_decimals: u8,
    pub token_0_decimals: u8,
    pub token_1_decimals: u8,

    // 交易信息
    pub signature: String,
    pub slot: u64,
    pub block_time: Option<i64>,

    // 时间戳
    pub created_at: DateTime<Utc>,
}

impl LpChangeEvent {
    /// 获取变更类型的名称
    pub fn get_change_type_name(&self) -> &'static str {
        match self.change_type {
            0 => "deposit",
            1 => "withdraw",
            2 => "initialize",
            _ => "unknown",
        }
    }

    /// 验证事件数据是否有效
    pub fn validate(&self) -> Result<(), String> {
        if self.user_wallet.is_empty() {
            return Err("用户钱包地址不能为空".to_string());
        }

        if self.pool_id.is_empty() {
            return Err("池子ID不能为空".to_string());
        }

        if self.lp_mint.is_empty() {
            return Err("LP mint地址不能为空".to_string());
        }

        if self.signature.is_empty() {
            return Err("交易签名不能为空".to_string());
        }

        if self.change_type > 2 {
            return Err(format!("无效的变更类型: {}", self.change_type));
        }

        // 验证数量一致性
        if self.change_type != 2 && self.lp_amount_before == 0 {
            return Err("非初始化操作但LP数量为0".to_string());
        }

        // 验证数量一致性
        if self.change_type == 2 && self.lp_amount_before != 0 {
            return Err("初始化操作但LP已有数量不为0".to_string());
        }

        // 验证精度范围
        if self.lp_mint_decimals > 18 || self.token_0_decimals > 18 || self.token_1_decimals > 18 {
            return Err("代币精度超出合理范围".to_string());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_change_type_name() {
        let mut event = create_test_event();

        event.change_type = 0;
        assert_eq!(event.get_change_type_name(), "deposit");

        event.change_type = 1;
        assert_eq!(event.get_change_type_name(), "withdraw");

        event.change_type = 2;
        assert_eq!(event.get_change_type_name(), "initialize");

        event.change_type = 99;
        assert_eq!(event.get_change_type_name(), "unknown");
    }

    #[test]
    fn test_validate_success() {
        let event = create_test_event();
        assert!(event.validate().is_ok());
    }

    #[test]
    fn test_validate_empty_user_wallet() {
        let mut event = create_test_event();
        event.user_wallet = String::new();
        assert!(event.validate().is_err());
    }

    #[test]
    fn test_validate_invalid_change_type() {
        let mut event = create_test_event();
        event.change_type = 5;
        assert!(event.validate().is_err());
    }

    #[test]
    fn test_validate_decimals_out_of_range() {
        let mut event = create_test_event();
        event.lp_mint_decimals = 20;
        assert!(event.validate().is_err());
    }

    fn create_test_event() -> LpChangeEvent {
        LpChangeEvent {
            id: None,
            user_wallet: "test_wallet".to_string(),
            pool_id: "test_pool".to_string(),
            lp_mint: "test_lp_mint".to_string(),
            token_0_mint: "test_token_0".to_string(),
            token_1_mint: "test_token_1".to_string(),
            change_type: 0,
            lp_amount_before: 1000,
            lp_amount_after: 2000,
            lp_amount_change: 1000,
            token_0_amount: 500,
            token_1_amount: 500,
            token_0_transfer_fee: 10,
            token_1_transfer_fee: 10,
            token_0_vault_before: 10000,
            token_1_vault_before: 10000,
            token_0_vault_after: 10500,
            token_1_vault_after: 10500,
            lp_mint_program_id: "test_program".to_string(),
            token_0_program_id: "test_program".to_string(),
            token_1_program_id: "test_program".to_string(),
            lp_mint_decimals: 9,
            token_0_decimals: 9,
            token_1_decimals: 9,
            signature: "test_signature".to_string(),
            slot: 12345,
            block_time: Some(1234567890),
            created_at: Utc::now(),
        }
    }
}
