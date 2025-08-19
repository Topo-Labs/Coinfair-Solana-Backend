use anyhow::Result;
use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;

use super::ConfigManager;

/// 核心交换账户结构体
pub struct SwapCoreAccounts {
    pub amm_config_state: raydium_amm_v3::states::AmmConfig,
    pub pool_state: raydium_amm_v3::states::PoolState,
    pub tickarray_bitmap_extension: raydium_amm_v3::states::TickArrayBitmapExtension,
    pub mint0_state: spl_token_2022::state::Mint,
    pub mint1_state: spl_token_2022::state::Mint,
    pub mint0: Pubkey,
    pub mint1: Pubkey,
    pub zero_for_one: bool,
}

/// 账户加载器 - 统一管理账户加载和反序列化
pub struct AccountLoader<'a> {
    rpc_client: &'a RpcClient,
}

impl<'a> AccountLoader<'a> {
    pub fn new(rpc_client: &'a RpcClient) -> Self {
        Self { rpc_client }
    }

    /// 批量加载账户
    pub async fn load_multiple_accounts(
        &self,
        pubkeys: &[Pubkey],
    ) -> Result<Vec<Option<solana_sdk::account::Account>>> {
        self.rpc_client.get_multiple_accounts(pubkeys).map_err(Into::into)
    }

    /// 加载并反序列化单个账户
    pub async fn load_and_deserialize<T: anchor_lang::AccountDeserialize>(&self, pubkey: &Pubkey) -> Result<T> {
        let account = self.rpc_client.get_account(pubkey)?;
        self.deserialize_anchor_account(&account)
    }

    /// 反序列化anchor账户
    pub fn deserialize_anchor_account<T: anchor_lang::AccountDeserialize>(
        &self,
        account: &solana_sdk::account::Account,
    ) -> Result<T> {
        let mut data: &[u8] = &account.data;
        T::try_deserialize(&mut data).map_err(Into::into)
    }

    /// 批量加载和反序列化核心交换账户
    pub async fn load_swap_core_accounts(
        &self,
        pool_pubkey: &Pubkey,
        input_mint: &Pubkey,
        output_mint: &Pubkey,
    ) -> Result<SwapCoreAccounts> {
        let raydium_program_id = ConfigManager::get_raydium_program_id()?;
        let amm_config_index = ConfigManager::get_amm_config_index();

        // 计算所需的PDA地址
        let (amm_config_key, _) = Pubkey::find_program_address(
            &["amm_config".as_bytes(), &amm_config_index.to_be_bytes()],
            &raydium_program_id,
        );
        let (tickarray_bitmap_extension_pda, _) = Pubkey::find_program_address(
            &["pool_tick_array_bitmap_extension".as_bytes(), pool_pubkey.as_ref()],
            &raydium_program_id,
        );

        // 标准化mint顺序
        let (mint0, mint1) = if input_mint < output_mint {
            (*input_mint, *output_mint)
        } else {
            (*output_mint, *input_mint)
        };

        // 批量加载账户
        let load_accounts = vec![
            amm_config_key,
            *pool_pubkey,
            tickarray_bitmap_extension_pda,
            mint0,
            mint1,
        ];
        let accounts = self.load_multiple_accounts(&load_accounts).await?;

        // 验证和反序列化
        let amm_config_account = accounts[0]
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("无法加载AMM配置账户"))?;
        let pool_account = accounts[1]
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("无法加载池子账户"))?;
        let tickarray_bitmap_extension_account = accounts[2]
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("无法加载bitmap扩展账户"))?;
        let mint0_account = accounts[3]
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("无法加载mint0账户"))?;
        let mint1_account = accounts[4]
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("无法加载mint1账户"))?;

        Ok(SwapCoreAccounts {
            amm_config_state: self.deserialize_anchor_account(amm_config_account)?,
            pool_state: self.deserialize_anchor_account(pool_account)?,
            tickarray_bitmap_extension: self.deserialize_anchor_account(tickarray_bitmap_extension_account)?,
            mint0_state: spl_token_2022::extension::StateWithExtensions::<spl_token_2022::state::Mint>::unpack(
                &mint0_account.data,
            )?
            .base,
            mint1_state: spl_token_2022::extension::StateWithExtensions::<spl_token_2022::state::Mint>::unpack(
                &mint1_account.data,
            )?
            .base,
            mint0,
            mint1,
            zero_for_one: *input_mint == mint0,
        })
    }
}
