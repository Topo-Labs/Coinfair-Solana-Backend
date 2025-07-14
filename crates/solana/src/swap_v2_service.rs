use anyhow::Result;
use solana_client::rpc_client::RpcClient;
use solana_sdk::{account::Account, pubkey::Pubkey};
use spl_token_2022::{
    extension::{transfer_fee::{TransferFeeConfig, MAX_FEE_BASIS_POINTS}, BaseStateWithExtensions, StateWithExtensions},
    state::Mint,
};
use spl_token;
use spl_token::solana_program::program_pack::Pack;
use std::str::FromStr;
use tracing::{info, warn};

/// SwapV2专用服务，提供精确的transfer fee计算和账户状态管理
pub struct SwapV2Service {
    rpc_client: RpcClient,
}

/// 代币账户信息
#[derive(Debug, Clone)]
pub struct TokenAccountInfo {
    pub mint: Pubkey,
    pub decimals: u8,
    pub owner: Pubkey,
    pub is_token_2022: bool,
}

/// Transfer fee计算结果（与CLI TransferFeeInfo保持一致）
#[derive(Debug, Clone)]
pub struct TransferFeeResult {
    pub mint: Pubkey,
    pub owner: Pubkey,
    pub transfer_fee: u64,
}

/// SwapV2完整账户信息
#[derive(Debug, Clone)]
pub struct SwapV2AccountsInfo {
    pub input_token_account: Account,
    pub output_token_account: Account,
    pub amm_config_account: Account,
    pub pool_account: Account,
    pub tickarray_bitmap_extension_account: Account,
    pub mint0_account: Account,
    pub mint1_account: Account,
    pub epoch: u64,
    pub pool_address: Pubkey,
    pub input_mint_info: TokenAccountInfo,
    pub output_mint_info: TokenAccountInfo,
}

/// 用户代币账户信息
#[derive(Debug, Clone)]
pub struct UserTokenAccountInfo {
    pub account: Account,
    pub balance: u64,
    pub mint: Pubkey,
    pub owner: Pubkey,
}

impl SwapV2Service {
    pub fn new(rpc_url: &str) -> Self {
        Self {
            rpc_client: RpcClient::new(rpc_url.to_string()),
        }
    }

    /// 获取当前epoch
    pub fn get_current_epoch(&self) -> Result<u64> {
        let epoch_info = self.rpc_client.get_epoch_info()?;
        Ok(epoch_info.epoch)
    }

    /// 加载代币mint信息
    pub fn load_mint_info(&self, mint: &Pubkey) -> Result<TokenAccountInfo> {
        let account = self.rpc_client.get_account(mint)?;
        
        // 判断是否为Token-2022程序
        let is_token_2022 = account.owner == spl_token_2022::id();
        
        if is_token_2022 {
            // Token-2022 mint
            let mint_data = StateWithExtensions::<Mint>::unpack(&account.data)?;
            Ok(TokenAccountInfo {
                mint: *mint,
                decimals: mint_data.base.decimals,
                owner: account.owner,
                is_token_2022: true,
            })
        } else {
            // 标准Token程序
            let mint_data = spl_token::state::Mint::unpack(&account.data)?;
            Ok(TokenAccountInfo {
                mint: *mint,
                decimals: mint_data.decimals,
                owner: account.owner,
                is_token_2022: false,
            })
        }
    }

    /// 计算输入代币的transfer fee（完全基于CLI实现）
    pub fn get_transfer_fee(&self, mint: &Pubkey, amount: u64) -> Result<TransferFeeResult> {
        let account = self.rpc_client.get_account(mint)?;
        
        // 如果是标准Token程序，没有transfer fee
        if account.owner == spl_token::id() {
            return Ok(TransferFeeResult {
                mint: *mint,
                owner: account.owner,
                transfer_fee: 0,
            });
        }

        // Token-2022程序处理
        if account.owner == spl_token_2022::id() {
            let mint_data = StateWithExtensions::<Mint>::unpack(&account.data)?;
            
            let transfer_fee = if let Ok(transfer_fee_config) = mint_data.get_extension::<TransferFeeConfig>() {
                let epoch = self.get_current_epoch()?;
                // 与CLI保持一致：使用unwrap而不是unwrap_or，确保错误能被正确传播
                transfer_fee_config
                    .calculate_epoch_fee(epoch, amount)
                    .ok_or_else(|| anyhow::anyhow!("Transfer fee calculation failed"))?
            } else {
                0
            };

            Ok(TransferFeeResult {
                mint: *mint,
                owner: account.owner,
                transfer_fee,
            })
        } else {
            // 未知token程序
            Ok(TransferFeeResult {
                mint: *mint,
                owner: account.owner,
                transfer_fee: 0,
            })
        }
    }

    /// 计算输出代币的inverse transfer fee（完全基于CLI实现）
    pub fn get_transfer_inverse_fee(&self, mint: &Pubkey, post_fee_amount: u64) -> Result<TransferFeeResult> {
        let account = self.rpc_client.get_account(mint)?;
        
        // 如果是标准Token程序，没有transfer fee
        if account.owner == spl_token::id() {
            return Ok(TransferFeeResult {
                mint: *mint,
                owner: account.owner,
                transfer_fee: 0,
            });
        }

        // Token-2022程序处理
        if account.owner == spl_token_2022::id() {
            let mint_data = StateWithExtensions::<Mint>::unpack(&account.data)?;
            
            let transfer_fee = if let Ok(transfer_fee_config) = mint_data.get_extension::<TransferFeeConfig>() {
                let epoch = self.get_current_epoch()?;
                let epoch_fee = transfer_fee_config.get_epoch_fee(epoch);
                
                // 关键修复：完全按照CLI逻辑处理边界情况
                if u16::from(epoch_fee.transfer_fee_basis_points) == MAX_FEE_BASIS_POINTS {
                    // 当费率达到最大值时，直接返回最大费用
                    u64::from(epoch_fee.maximum_fee)
                } else {
                    // 正常情况下进行反向计算
                    transfer_fee_config
                        .calculate_inverse_epoch_fee(epoch, post_fee_amount)
                        .ok_or_else(|| anyhow::anyhow!("Inverse transfer fee calculation failed"))?
                }
            } else {
                0
            };

            Ok(TransferFeeResult {
                mint: *mint,
                owner: account.owner,
                transfer_fee,
            })
        } else {
            // 未知token程序
            Ok(TransferFeeResult {
                mint: *mint,
                owner: account.owner,
                transfer_fee: 0,
            })
        }
    }

    /// 批量加载多个mint的信息
    pub fn load_multiple_mint_info(&self, mints: &[Pubkey]) -> Result<Vec<TokenAccountInfo>> {
        let accounts = self.rpc_client.get_multiple_accounts(mints)?;
        let mut results = Vec::new();

        for (i, account_opt) in accounts.iter().enumerate() {
            match account_opt {
                Some(account) => {
                    let mint = mints[i];
                    let is_token_2022 = account.owner == spl_token_2022::id();
                    
                    let decimals = if is_token_2022 {
                        let mint_data = StateWithExtensions::<Mint>::unpack(&account.data)?;
                        mint_data.base.decimals
                    } else {
                        let mint_data = spl_token::state::Mint::unpack(&account.data)?;
                        mint_data.decimals
                    };

                    results.push(TokenAccountInfo {
                        mint,
                        decimals,
                        owner: account.owner,
                        is_token_2022,
                    });
                }
                None => {
                    return Err(anyhow::anyhow!("无法加载mint账户: {}", mints[i]));
                }
            }
        }

        Ok(results)
    }

    /// 验证pool的mint顺序（确保mint0 < mint1）
    pub fn normalize_mint_order(&self, mint_a: &str, mint_b: &str) -> Result<(Pubkey, Pubkey)> {
        let mint_a_pubkey = Pubkey::from_str(mint_a)?;
        let mint_b_pubkey = Pubkey::from_str(mint_b)?;

        if mint_a_pubkey < mint_b_pubkey {
            Ok((mint_a_pubkey, mint_b_pubkey))
        } else {
            Ok((mint_b_pubkey, mint_a_pubkey))
        }
    }

    /// 计算池子地址的PDA
    pub fn calculate_pool_address_pda(
        &self,
        mint0: &Pubkey,
        mint1: &Pubkey,
        amm_config_key: &Pubkey,
        program_id: &Pubkey,
    ) -> Result<Pubkey> {
        let (pool_pda, _bump) = Pubkey::find_program_address(
            &[
                b"pool",
                amm_config_key.as_ref(),
                mint0.as_ref(),
                mint1.as_ref(),
            ],
            program_id,
        );
        Ok(pool_pda)
    }

    /// 计算bitmap extension地址
    pub fn calculate_bitmap_extension_address(
        &self,
        pool_id: &Pubkey,
        program_id: &Pubkey,
    ) -> Result<Pubkey> {
        let (bitmap_extension, _bump) = Pubkey::find_program_address(
            &[
                b"pool_tick_array_bitmap_extension",
                pool_id.as_ref(),
            ],
            program_id,
        );
        Ok(bitmap_extension)
    }

    /// 加载SwapV2所需的完整账户信息（模拟CLI中的完整加载过程）
    pub fn load_swap_v2_accounts_complete(
        &self,
        input_mint: &str,
        output_mint: &str,
        user_wallet: &Pubkey,
        amm_config_key: &Pubkey,
        program_id: &Pubkey,
    ) -> Result<SwapV2AccountsInfo> {
        info!("🔍 开始加载SwapV2完整账户信息");
        
        // 1. 解析和标准化mint顺序
        let (mint0, mint1) = self.normalize_mint_order(input_mint, output_mint)?;
        let input_mint_pubkey = Pubkey::from_str(input_mint)?;
        let output_mint_pubkey = Pubkey::from_str(output_mint)?;
        
        // 2. 计算相关地址
        let pool_address = self.calculate_pool_address_pda(&mint0, &mint1, amm_config_key, program_id)?;
        let bitmap_extension_address = self.calculate_bitmap_extension_address(&pool_address, program_id)?;
        
        // 3. 计算用户代币账户地址（ATA）
        let input_token_account = spl_associated_token_account::get_associated_token_address(
            user_wallet,
            &input_mint_pubkey,
        );
        let output_token_account = spl_associated_token_account::get_associated_token_address(
            user_wallet,
            &output_mint_pubkey,
        );
        
        // 4. 批量加载所有账户
        let accounts_to_load = vec![
            input_token_account,
            output_token_account,
            *amm_config_key,
            pool_address,
            bitmap_extension_address,
            mint0,
            mint1,
        ];
        
        info!("📊 批量加载{}个账户", accounts_to_load.len());
        let loaded_accounts = self.rpc_client.get_multiple_accounts(&accounts_to_load)?;
        
        // 5. 验证所有账户都存在
        if loaded_accounts.len() != 7 {
            return Err(anyhow::anyhow!("账户加载失败，期望7个账户，实际获得{}", loaded_accounts.len()));
        }
        
        let mut accounts_iter = loaded_accounts.into_iter();
        let input_token_account = accounts_iter.next().unwrap()
            .ok_or_else(|| anyhow::anyhow!("用户输入代币账户不存在"))?;
        let output_token_account = accounts_iter.next().unwrap()
            .ok_or_else(|| anyhow::anyhow!("用户输出代币账户不存在"))?;
        let amm_config_account = accounts_iter.next().unwrap()
            .ok_or_else(|| anyhow::anyhow!("AMM配置账户不存在"))?;
        let pool_account = accounts_iter.next().unwrap()
            .ok_or_else(|| anyhow::anyhow!("池子账户不存在"))?;
        let tickarray_bitmap_extension_account = accounts_iter.next().unwrap()
            .ok_or_else(|| anyhow::anyhow!("Bitmap扩展账户不存在"))?;
        let mint0_account = accounts_iter.next().unwrap()
            .ok_or_else(|| anyhow::anyhow!("Mint0账户不存在"))?;
        let mint1_account = accounts_iter.next().unwrap()
            .ok_or_else(|| anyhow::anyhow!("Mint1账户不存在"))?;
        
        // 6. 获取当前epoch
        let epoch = self.get_current_epoch()?;
        
        // 7. 解析mint信息
        let input_mint_info = self.parse_mint_account(&input_mint_pubkey, 
            if input_mint_pubkey == mint0 { &mint0_account } else { &mint1_account })?;
        let output_mint_info = self.parse_mint_account(&output_mint_pubkey,
            if output_mint_pubkey == mint0 { &mint0_account } else { &mint1_account })?;
        
        info!("✅ SwapV2账户信息加载完成");
        info!("  池子地址: {}", pool_address);
        info!("  输入代币: {} (decimals: {})", input_mint_info.mint, input_mint_info.decimals);
        info!("  输出代币: {} (decimals: {})", output_mint_info.mint, output_mint_info.decimals);
        info!("  当前epoch: {}", epoch);
        
        Ok(SwapV2AccountsInfo {
            input_token_account,
            output_token_account,
            amm_config_account,
            pool_account,
            tickarray_bitmap_extension_account,
            mint0_account,
            mint1_account,
            epoch,
            pool_address,
            input_mint_info,
            output_mint_info,
        })
    }
    
    /// 解析mint账户信息
    fn parse_mint_account(&self, mint_pubkey: &Pubkey, account: &Account) -> Result<TokenAccountInfo> {
        let is_token_2022 = account.owner == spl_token_2022::id();
        
        let decimals = if is_token_2022 {
            let mint_data = StateWithExtensions::<Mint>::unpack(&account.data)?;
            mint_data.base.decimals
        } else {
            let mint_data = spl_token::state::Mint::unpack(&account.data)?;
            mint_data.decimals
        };
        
        Ok(TokenAccountInfo {
            mint: *mint_pubkey,
            decimals,
            owner: account.owner,
            is_token_2022,
        })
    }
    
    /// 获取用户代币账户余额
    pub fn get_user_token_balance(&self, user_wallet: &Pubkey, mint: &Pubkey) -> Result<u64> {
        let token_account = spl_associated_token_account::get_associated_token_address(user_wallet, mint);
        
        match self.rpc_client.get_account(&token_account) {
            Ok(account) => {
                if account.owner == spl_token::id() {
                    let token_data = spl_token::state::Account::unpack(&account.data)?;
                    Ok(token_data.amount)
                } else if account.owner == spl_token_2022::id() {
                    let token_data = StateWithExtensions::<spl_token_2022::state::Account>::unpack(&account.data)?;
                    Ok(token_data.base.amount)
                } else {
                    Err(anyhow::anyhow!("未知的代币账户类型"))
                }
            }
            Err(_) => {
                // 账户不存在，余额为0
                Ok(0)
            }
        }
    }

    /// 验证SwapV2账户信息的完整性
    pub fn validate_swap_v2_accounts(&self, accounts: &SwapV2AccountsInfo) -> Result<()> {
        // 验证输入和输出代币账户的有效性
        if accounts.input_token_account.owner != spl_token::id() 
            && accounts.input_token_account.owner != spl_token_2022::id() {
            return Err(anyhow::anyhow!("无效的输入代币账户"));
        }
        
        if accounts.output_token_account.owner != spl_token::id() 
            && accounts.output_token_account.owner != spl_token_2022::id() {
            return Err(anyhow::anyhow!("无效的输出代币账户"));
        }
        
        // 验证mint账户的有效性
        if accounts.mint0_account.owner != spl_token::id() 
            && accounts.mint0_account.owner != spl_token_2022::id() {
            return Err(anyhow::anyhow!("无效的mint0账户"));
        }
        
        if accounts.mint1_account.owner != spl_token::id() 
            && accounts.mint1_account.owner != spl_token_2022::id() {
            return Err(anyhow::anyhow!("无效的mint1账户"));
        }
        
        // 验证epoch有效性
        if accounts.epoch == 0 {
            return Err(anyhow::anyhow!("无效的epoch信息"));
        }
        
        info!("✅ SwapV2账户信息验证通过");
        Ok(())
    }

    /// 批量计算pool mints的transfer fee（对应CLI的get_pool_mints_transfer_fee）
    pub fn get_pool_mints_transfer_fee(
        &self,
        mint0: &Pubkey,
        mint1: &Pubkey,
        amount0: u64,
        amount1: u64,
    ) -> Result<(TransferFeeResult, TransferFeeResult)> {
        info!("📊 批量计算transfer fee: mint0={}, mint1={}", mint0, mint1);
        
        // 批量加载两个mint账户
        let load_accounts = vec![*mint0, *mint1];
        let accounts = self.rpc_client.get_multiple_accounts(&load_accounts)?;
        let epoch = self.get_current_epoch()?;
        
        let mint0_account = accounts[0].as_ref()
            .ok_or_else(|| anyhow::anyhow!("Failed to load mint0 account"))?;
        let mint1_account = accounts[1].as_ref()
            .ok_or_else(|| anyhow::anyhow!("Failed to load mint1 account"))?;
        
        // 计算mint0的transfer fee
        let transfer_fee_0 = self.calculate_transfer_fee_from_account(
            mint0, 
            mint0_account, 
            epoch, 
            amount0
        )?;
        
        // 计算mint1的transfer fee
        let transfer_fee_1 = self.calculate_transfer_fee_from_account(
            mint1, 
            mint1_account, 
            epoch, 
            amount1
        )?;
        
        Ok((
            TransferFeeResult {
                mint: *mint0,
                owner: mint0_account.owner,
                transfer_fee: transfer_fee_0,
            },
            TransferFeeResult {
                mint: *mint1,
                owner: mint1_account.owner,
                transfer_fee: transfer_fee_1,
            },
        ))
    }

    /// 批量计算pool mints的inverse transfer fee（对应CLI的get_pool_mints_inverse_fee）
    pub fn get_pool_mints_inverse_fee(
        &self,
        mint0: &Pubkey,
        mint1: &Pubkey,
        post_amount0: u64,
        post_amount1: u64,
    ) -> Result<(TransferFeeResult, TransferFeeResult)> {
        info!("📊 批量计算inverse transfer fee: mint0={}, mint1={}", mint0, mint1);
        
        // 批量加载两个mint账户
        let load_accounts = vec![*mint0, *mint1];
        let accounts = self.rpc_client.get_multiple_accounts(&load_accounts)?;
        let epoch = self.get_current_epoch()?;
        
        let mint0_account = accounts[0].as_ref()
            .ok_or_else(|| anyhow::anyhow!("Failed to load mint0 account"))?;
        let mint1_account = accounts[1].as_ref()
            .ok_or_else(|| anyhow::anyhow!("Failed to load mint1 account"))?;
        
        // 计算mint0的inverse transfer fee
        let transfer_fee_0 = self.calculate_inverse_transfer_fee_from_account(
            mint0, 
            mint0_account, 
            epoch, 
            post_amount0
        )?;
        
        // 计算mint1的inverse transfer fee
        let transfer_fee_1 = self.calculate_inverse_transfer_fee_from_account(
            mint1, 
            mint1_account, 
            epoch, 
            post_amount1
        )?;
        
        Ok((
            TransferFeeResult {
                mint: *mint0,
                owner: mint0_account.owner,
                transfer_fee: transfer_fee_0,
            },
            TransferFeeResult {
                mint: *mint1,
                owner: mint1_account.owner,
                transfer_fee: transfer_fee_1,
            },
        ))
    }

    /// 从已加载的账户计算transfer fee（内部辅助方法）
    fn calculate_transfer_fee_from_account(
        &self,
        mint: &Pubkey,
        account: &Account,
        epoch: u64,
        amount: u64,
    ) -> Result<u64> {
        // 如果是标准Token程序，没有transfer fee
        if account.owner == spl_token::id() {
            return Ok(0);
        }

        // Token-2022程序处理
        if account.owner == spl_token_2022::id() {
            let mint_data = StateWithExtensions::<Mint>::unpack(&account.data)?;
            
            if let Ok(transfer_fee_config) = mint_data.get_extension::<TransferFeeConfig>() {
                let transfer_fee = transfer_fee_config
                    .calculate_epoch_fee(epoch, amount)
                    .ok_or_else(|| anyhow::anyhow!("Transfer fee calculation failed for {}", mint))?;
                Ok(transfer_fee)
            } else {
                Ok(0)
            }
        } else {
            Ok(0)
        }
    }

    /// 从已加载的账户计算inverse transfer fee（内部辅助方法）
    fn calculate_inverse_transfer_fee_from_account(
        &self,
        mint: &Pubkey,
        account: &Account,
        epoch: u64,
        post_fee_amount: u64,
    ) -> Result<u64> {
        // 如果是标准Token程序，没有transfer fee
        if account.owner == spl_token::id() {
            return Ok(0);
        }

        // Token-2022程序处理
        if account.owner == spl_token_2022::id() {
            let mint_data = StateWithExtensions::<Mint>::unpack(&account.data)?;
            
            if let Ok(transfer_fee_config) = mint_data.get_extension::<TransferFeeConfig>() {
                let epoch_fee = transfer_fee_config.get_epoch_fee(epoch);
                
                // 完全按照CLI逻辑处理边界情况
                let transfer_fee = if u16::from(epoch_fee.transfer_fee_basis_points) == MAX_FEE_BASIS_POINTS {
                    // 当费率达到最大值时，直接返回最大费用
                    u64::from(epoch_fee.maximum_fee)
                } else {
                    // 正常情况下进行反向计算
                    transfer_fee_config
                        .calculate_inverse_epoch_fee(epoch, post_fee_amount)
                        .ok_or_else(|| anyhow::anyhow!("Inverse transfer fee calculation failed for {}", mint))?
                };
                Ok(transfer_fee)
            } else {
                Ok(0)
            }
        } else {
            Ok(0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_mint_order() {
        let service = SwapV2Service::new("https://api.mainnet-beta.solana.com");
        
        let mint_a = "So11111111111111111111111111111111111111112"; // SOL
        let mint_b = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"; // USDC
        
        let mint_a_pubkey = solana_sdk::pubkey::Pubkey::from_str(mint_a).unwrap();
        let mint_b_pubkey = solana_sdk::pubkey::Pubkey::from_str(mint_b).unwrap();
        
        let (mint0, mint1) = service.normalize_mint_order(mint_a, mint_b).unwrap();
        
        // normalize_mint_order总是返回较小的地址在前，较大的在后
        assert!(mint0 < mint1);
        
        // 根据实际比较结果调整断言：SOL地址小于USDC地址
        if mint_a_pubkey < mint_b_pubkey {
            assert_eq!(mint0.to_string(), mint_a); // SOL在前
            assert_eq!(mint1.to_string(), mint_b); // USDC在后
        } else {
            assert_eq!(mint0.to_string(), mint_b); // USDC在前
            assert_eq!(mint1.to_string(), mint_a); // SOL在后
        }
    }
}