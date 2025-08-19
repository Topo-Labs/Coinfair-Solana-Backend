use crate::dtos::solana_dto::{
    GetMintCounterAndVerifyResponse, GetMintCounterRequest, GetMintCounterResponse, GetUpperAndVerifyResponse,
    GetUpperRequest, GetUpperResponse, MintCounterData, ReferralAccountData,
};

use super::super::shared::SharedContext;

use anyhow::{anyhow, Result};
use chrono::Utc;
use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use std::sync::Arc;
use tracing::{info, warn};

use anchor_lang::AccountDeserialize;

/// ReferralAccount状态结构，对应智能合约中的ReferralAccount
/// 必须与 /crates/solana/referral/src/states/referral_account.rs 完全一致
#[derive(Clone, Debug)]
pub struct ReferralAccount {
    pub user: Pubkey,                // 本人
    pub upper: Option<Pubkey>,       // 上级
    pub upper_upper: Option<Pubkey>, // 上上级
    pub nft_mint: Pubkey,            // 绑定用的NFT
    pub bump: u8,                    // PDA bump
}

impl AccountDeserialize for ReferralAccount {
    fn try_deserialize(buf: &mut &[u8]) -> anchor_lang::Result<Self> {
        Self::try_deserialize_unchecked(buf)
    }

    fn try_deserialize_unchecked(buf: &mut &[u8]) -> anchor_lang::Result<Self> {
        // 跳过discriminator (前8字节)
        if buf.len() < 8 {
            return Err(anchor_lang::error::ErrorCode::AccountDidNotDeserialize.into());
        }
        *buf = &buf[8..];

        // 反序列化字段
        use anchor_lang::AnchorDeserialize;

        let user = Pubkey::deserialize(buf)?;
        let upper = Option::<Pubkey>::deserialize(buf)?;
        let upper_upper = Option::<Pubkey>::deserialize(buf)?;
        let nft_mint = Pubkey::deserialize(buf)?;
        let bump = u8::deserialize(buf)?;

        Ok(Self {
            user,
            upper,
            upper_upper,
            nft_mint,
            bump,
        })
    }
}

/// MintCounter状态结构，对应智能合约中的MintCounter
/// 必须与 /crates/solana/referral/src/instructions/mint_nft.rs 完全一致
#[derive(Clone, Debug)]
pub struct MintCounter {
    pub minter: Pubkey,   // 用户地址
    pub total_mint: u64,  // 总mint数量
    pub remain_mint: u64, // 剩余可被claim的数量
    pub bump: u8,         // PDA bump
}

impl AccountDeserialize for MintCounter {
    fn try_deserialize(buf: &mut &[u8]) -> anchor_lang::Result<Self> {
        Self::try_deserialize_unchecked(buf)
    }

    fn try_deserialize_unchecked(buf: &mut &[u8]) -> anchor_lang::Result<Self> {
        // 跳过discriminator (前8字节)
        if buf.len() < 8 {
            return Err(anchor_lang::error::ErrorCode::AccountDidNotDeserialize.into());
        }
        *buf = &buf[8..];

        // 反序列化字段
        use anchor_lang::AnchorDeserialize;

        let minter = Pubkey::deserialize(buf)?;
        let total_mint = u64::deserialize(buf)?;
        let remain_mint = u64::deserialize(buf)?;
        let bump = u8::deserialize(buf)?;

        Ok(Self {
            minter,
            total_mint,
            remain_mint,
            bump,
        })
    }
}

/// Referral服务 - 处理推荐系统相关功能
pub struct ReferralService {
    shared: Arc<SharedContext>,
}

impl ReferralService {
    /// 创建新的Referral服务实例
    pub fn new(shared: Arc<SharedContext>) -> Self {
        Self { shared }
    }

    /// 获取用户的上级推荐人（100%复现CLI GetUpper逻辑）
    /// 对应CLI中第1715-1724行的CommandsName::GetUpper { user }逻辑
    pub async fn get_upper(&self, request: GetUpperRequest) -> Result<GetUpperResponse> {
        info!("🎯 开始查询用户的上级推荐人");
        info!("  用户钱包: {}", request.user_wallet);

        let user_wallet = Pubkey::from_str(&request.user_wallet)?;

        // 获取推荐程序ID (对应CLI配置中的referral_program)
        let referral_program_id = self.get_referral_program_id_internal()?;
        info!("  推荐程序ID: {}", referral_program_id);

        // 计算推荐账户PDA (完全对应CLI第1720行)
        let (referral_account, _) =
            Pubkey::find_program_address(&[b"referral", &user_wallet.to_bytes()], &referral_program_id);
        info!("  推荐账户PDA: {}", referral_account);

        // 使用RPC客户端查询账户 (对应CLI中的anchor client)
        let rpc_client = RpcClient::new(&self.shared.swap_config.rpc_url);

        match rpc_client.get_account(&referral_account) {
            Ok(account_data) => {
                // 反序列化ReferralAccount数据 (对应CLI第1722行)
                let referral_account_data = self.deserialize_referral_account(&account_data)?;

                // 提取upper字段 (对应CLI第1723行)
                let upper = referral_account_data.upper.map(|p| p.to_string());

                info!("✅ 成功查询到用户上级: {:?}", upper);

                Ok(GetUpperResponse {
                    user_wallet: request.user_wallet,
                    upper,
                    referral_account: referral_account.to_string(),
                    status: "Success".to_string(),
                    timestamp: Utc::now().timestamp(),
                })
            }
            Err(e) => {
                warn!("❌ 推荐账户不存在或查询失败: {}", e);

                // 账户不存在时返回None，而不是错误
                Ok(GetUpperResponse {
                    user_wallet: request.user_wallet,
                    upper: None,
                    referral_account: referral_account.to_string(),
                    status: "AccountNotFound".to_string(),
                    timestamp: Utc::now().timestamp(),
                })
            }
        }
    }

    /// 获取用户的上级推荐人并进行本地验证（用于测试）
    pub async fn get_upper_and_verify(&self, request: GetUpperRequest) -> Result<GetUpperAndVerifyResponse> {
        info!("🎯 开始查询用户的上级推荐人并验证");
        info!("  用户钱包: {}", request.user_wallet);

        let user_wallet = Pubkey::from_str(&request.user_wallet)?;

        // 获取推荐程序ID
        let referral_program_id = self.get_referral_program_id_internal()?;

        // 计算推荐账户PDA
        let (referral_account, _) =
            Pubkey::find_program_address(&[b"referral", &user_wallet.to_bytes()], &referral_program_id);

        // 使用RPC客户端查询账户
        let rpc_client = RpcClient::new(&self.shared.swap_config.rpc_url);

        let (account_exists, referral_account_data, upper) = match rpc_client.get_account(&referral_account) {
            Ok(account_data) => {
                // 反序列化ReferralAccount数据
                let referral_data = self.deserialize_referral_account(&account_data)?;

                let upper = referral_data.upper.map(|p| p.to_string());

                let account_data_dto = Some(ReferralAccountData {
                    user: referral_data.user.to_string(),
                    upper: referral_data.upper.map(|p| p.to_string()),
                    upper_upper: referral_data.upper_upper.map(|p| p.to_string()),
                    nft_mint: referral_data.nft_mint.to_string(),
                    bump: referral_data.bump,
                });

                (true, account_data_dto, upper)
            }
            Err(_) => (false, None, None),
        };

        let base_response = GetUpperResponse {
            user_wallet: request.user_wallet,
            upper,
            referral_account: referral_account.to_string(),
            status: if account_exists {
                "Success".to_string()
            } else {
                "AccountNotFound".to_string()
            },
            timestamp: Utc::now().timestamp(),
        };

        info!("✅ 查询完成，账户存在: {}", account_exists);

        Ok(GetUpperAndVerifyResponse {
            base: base_response,
            account_exists,
            referral_account_data,
        })
    }

    /// 获取用户的MintCounter信息（100%复现CLI GetMintCounter逻辑）
    /// 对应CLI中第1725-1734行的CommandsName::GetMintCounter { user }逻辑
    pub async fn get_mint_counter(&self, request: GetMintCounterRequest) -> Result<GetMintCounterResponse> {
        info!("🎯 开始查询用户的MintCounter信息");
        info!("  用户钱包: {}", request.user_wallet);

        let user_wallet = Pubkey::from_str(&request.user_wallet)?;

        // 获取推荐程序ID (对应CLI配置中的referral_program)
        let referral_program_id = self.get_referral_program_id_internal()?;
        info!("  推荐程序ID: {}", referral_program_id);

        // 计算mint_counter账户PDA (完全对应CLI第1730行)
        let (mint_counter_account, _) =
            Pubkey::find_program_address(&[b"mint_counter", &user_wallet.to_bytes()], &referral_program_id);
        info!("  MintCounter账户PDA: {}", mint_counter_account);

        // 使用RPC客户端查询账户 (对应CLI中的anchor client)
        let rpc_client = RpcClient::new(&self.shared.swap_config.rpc_url);

        match rpc_client.get_account(&mint_counter_account) {
            Ok(account_data) => {
                // 反序列化MintCounter数据 (对应CLI第1732行)
                let mint_counter_data = self.deserialize_mint_counter(&account_data)?;

                // 提取total_mint和remain_mint字段 (对应CLI第1733行)
                let total_mint = mint_counter_data.total_mint;
                let remain_mint = mint_counter_data.remain_mint;

                info!(
                    "✅ 成功查询到用户MintCounter: total_mint={}, remain_mint={}",
                    total_mint, remain_mint
                );

                Ok(GetMintCounterResponse {
                    user_wallet: request.user_wallet,
                    total_mint,
                    remain_mint,
                    mint_counter_account: mint_counter_account.to_string(),
                    status: "Success".to_string(),
                    timestamp: Utc::now().timestamp(),
                })
            }
            Err(e) => {
                warn!("❌ MintCounter账户不存在或查询失败: {}", e);

                // 账户不存在时返回0值，而不是错误
                Ok(GetMintCounterResponse {
                    user_wallet: request.user_wallet,
                    total_mint: 0,
                    remain_mint: 0,
                    mint_counter_account: mint_counter_account.to_string(),
                    status: "AccountNotFound".to_string(),
                    timestamp: Utc::now().timestamp(),
                })
            }
        }
    }

    /// 获取用户的MintCounter信息并进行本地验证（用于测试）
    pub async fn get_mint_counter_and_verify(
        &self,
        request: GetMintCounterRequest,
    ) -> Result<GetMintCounterAndVerifyResponse> {
        info!("🎯 开始查询用户的MintCounter信息并验证");
        info!("  用户钱包: {}", request.user_wallet);

        let user_wallet = Pubkey::from_str(&request.user_wallet)?;

        // 获取推荐程序ID
        let referral_program_id = self.get_referral_program_id_internal()?;

        // 计算mint_counter账户PDA
        let (mint_counter_account, _) =
            Pubkey::find_program_address(&[b"mint_counter", &user_wallet.to_bytes()], &referral_program_id);

        // 使用RPC客户端查询账户
        let rpc_client = RpcClient::new(&self.shared.swap_config.rpc_url);

        let (account_exists, mint_counter_data, total_mint, remain_mint) =
            match rpc_client.get_account(&mint_counter_account) {
                Ok(account_data) => {
                    // 反序列化MintCounter数据
                    let mint_counter = self.deserialize_mint_counter(&account_data)?;

                    let total_mint = mint_counter.total_mint;
                    let remain_mint = mint_counter.remain_mint;

                    let counter_data_dto = Some(MintCounterData {
                        minter: mint_counter.minter.to_string(),
                        total_mint: mint_counter.total_mint,
                        remain_mint: mint_counter.remain_mint,
                        bump: mint_counter.bump,
                    });

                    (true, counter_data_dto, total_mint, remain_mint)
                }
                Err(_) => (false, None, 0, 0),
            };

        let base_response = GetMintCounterResponse {
            user_wallet: request.user_wallet,
            total_mint,
            remain_mint,
            mint_counter_account: mint_counter_account.to_string(),
            status: if account_exists {
                "Success".to_string()
            } else {
                "AccountNotFound".to_string()
            },
            timestamp: Utc::now().timestamp(),
        };

        info!(
            "✅ 查询完成，账户存在: {}, total_mint={}, remain_mint={}",
            account_exists, total_mint, remain_mint
        );

        Ok(GetMintCounterAndVerifyResponse {
            base: base_response,
            account_exists,
            mint_counter_data,
        })
    }

    /// 获取推荐程序ID
    #[cfg(test)]
    pub fn get_referral_program_id(&self) -> Result<Pubkey> {
        self.get_referral_program_id_internal()
    }

    /// 获取推荐程序ID（内部实现）
    fn get_referral_program_id_internal(&self) -> Result<Pubkey> {
        let program_id_str = std::env::var("REFERRAL_PROGRAM_ID")
            .unwrap_or_else(|_| "REFxcjx4pKym9j5Jzbo9wh92CtYTzHt9fqcjgvZGvUL".to_string());
        Pubkey::from_str(&program_id_str).map_err(Into::into)
    }

    /// 反序列化Anchor账户数据
    fn deserialize_referral_account(&self, account: &solana_sdk::account::Account) -> Result<ReferralAccount> {
        let mut data: &[u8] = &account.data;
        ReferralAccount::try_deserialize(&mut data).map_err(|e| anyhow!("反序列化ReferralAccount失败: {}", e))
    }

    /// 反序列化MintCounter账户数据
    fn deserialize_mint_counter(&self, account: &solana_sdk::account::Account) -> Result<MintCounter> {
        let mut data: &[u8] = &account.data;
        MintCounter::try_deserialize(&mut data).map_err(|e| anyhow!("反序列化MintCounter失败: {}", e))
    }

    /// 计算推荐账户PDA（测试用）
    #[cfg(test)]
    pub fn calculate_referral_account_pda(&self, user_wallet: &Pubkey) -> Result<(Pubkey, u8)> {
        let referral_program_id = self.get_referral_program_id_internal()?;
        let (pda, bump) = Pubkey::find_program_address(&[b"referral", user_wallet.as_ref()], &referral_program_id);
        Ok((pda, bump))
    }

    /// 计算MintCounter账户PDA（测试用）
    #[cfg(test)]
    pub fn calculate_mint_counter_pda(&self, user_wallet: &Pubkey) -> Result<(Pubkey, u8)> {
        let referral_program_id = self.get_referral_program_id_internal()?;
        let (pda, bump) = Pubkey::find_program_address(&[b"mint_counter", user_wallet.as_ref()], &referral_program_id);
        Ok((pda, bump))
    }
}
