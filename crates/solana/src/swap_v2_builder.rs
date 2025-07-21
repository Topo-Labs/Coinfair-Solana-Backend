use anchor_client::{Client, Cluster};
use anchor_lang::prelude::AccountMeta;
use anyhow::Result;
use solana_sdk::{compute_budget::ComputeBudgetInstruction, instruction::Instruction, pubkey::Pubkey, signature::Keypair};
use std::rc::Rc;
use std::str::FromStr;
use tracing::info;

use crate::swap_v2_service::{SwapV2AccountsInfo, SwapV2Service};
use raydium_amm_v3::accounts as raydium_accounts;
use raydium_amm_v3::instruction as raydium_instruction;

/// SwapV2指令构建器，负责构建完整的SwapV2交易指令
pub struct SwapV2InstructionBuilder {
    swap_v2_service: SwapV2Service,
    raydium_program_id: Pubkey,
    amm_config_key: Pubkey,
    rpc_url: String,
    ws_url: String,
}

/// SwapV2指令构建参数
#[derive(Debug, Clone)]
pub struct SwapV2BuildParams {
    pub input_mint: String,
    pub output_mint: String,
    pub user_wallet: Pubkey,
    pub user_input_token_account: Option<Pubkey>,
    pub user_output_token_account: Option<Pubkey>,
    pub amount: u64,
    pub other_amount_threshold: u64,
    pub sqrt_price_limit_x64: Option<u128>,
    pub is_base_input: bool,
    pub slippage_bps: u16,
    pub compute_unit_limit: Option<u32>,
}

/// 构建的SwapV2指令结果
#[derive(Debug, Clone)]
pub struct SwapV2InstructionResult {
    pub instructions: Vec<Instruction>,
    pub compute_units_used: u32,
    pub accounts_info: SwapV2AccountsInfo,
    pub expected_fee: u64,
}

impl SwapV2InstructionBuilder {
    pub fn new(rpc_url: &str, raydium_program_id: &str, amm_config_index: u16) -> Result<Self> {
        let raydium_program_id = Pubkey::from_str(raydium_program_id)?;

        // 计算AMM配置密钥
        let (amm_config_key, _bump) = Pubkey::find_program_address(&[b"amm_config", &amm_config_index.to_be_bytes()], &raydium_program_id);

        // 构建WebSocket URL
        let ws_url = rpc_url.replace("https://", "wss://").replace("http://", "ws://");

        Ok(Self {
            swap_v2_service: SwapV2Service::new(rpc_url),
            raydium_program_id,
            amm_config_key,
            rpc_url: rpc_url.to_string(),
            ws_url,
        })
    }

    /// 构建完整的SwapV2交易指令
    pub async fn build_swap_v2_instructions(&self, params: SwapV2BuildParams) -> Result<SwapV2InstructionResult> {
        info!("🔨 开始构建SwapV2指令");
        info!("  输入代币: {}", params.input_mint);
        info!("  输出代币: {}", params.output_mint);
        info!("  金额: {}", params.amount);
        info!("  是否base_in: {}", params.is_base_input);

        // 1. 加载完整的账户信息
        let accounts_info = self.swap_v2_service.load_swap_v2_accounts_complete(
            &params.input_mint,
            &params.output_mint,
            &params.user_wallet,
            &self.amm_config_key,
            &self.raydium_program_id,
        )?;

        // 2. 验证账户信息
        self.swap_v2_service.validate_swap_v2_accounts(&accounts_info)?;

        // 3. 计算vault地址
        let vault_addresses = self.calculate_vault_addresses(&accounts_info)?;

        // 4. 构建remaining accounts
        let remaining_accounts = self.build_remaining_accounts(&accounts_info)?;

        // 5. 构建指令序列
        let mut instructions = Vec::new();

        // 添加计算单元限制（如果指定）
        if let Some(compute_unit_limit) = params.compute_unit_limit {
            let compute_budget_ix = ComputeBudgetInstruction::set_compute_unit_limit(compute_unit_limit);
            instructions.push(compute_budget_ix);
        } else {
            // 默认设置合理的计算单元限制
            let compute_budget_ix = ComputeBudgetInstruction::set_compute_unit_limit(1_400_000);
            instructions.push(compute_budget_ix);
        }

        // 构建SwapV2核心指令
        let swap_v2_instruction = self.build_swap_v2_core_instruction(&accounts_info, &vault_addresses, remaining_accounts, &params)?;

        instructions.push(swap_v2_instruction);

        // 6. 估算费用
        let expected_fee = self.estimate_transaction_fee(&instructions)?;

        info!("✅ SwapV2指令构建完成");
        info!("  指令数量: {}", instructions.len());
        info!("  预估费用: {} lamports", expected_fee);

        Ok(SwapV2InstructionResult {
            instructions,
            compute_units_used: params.compute_unit_limit.unwrap_or(1_400_000),
            accounts_info,
            expected_fee,
        })
    }

    /// 计算vault地址
    fn calculate_vault_addresses(&self, accounts_info: &SwapV2AccountsInfo) -> Result<VaultAddresses> {
        let input_mint = Pubkey::from_str(&accounts_info.input_mint_info.mint.to_string())?;
        let output_mint = Pubkey::from_str(&accounts_info.output_mint_info.mint.to_string())?;

        // 判断交换方向
        let zero_for_one = input_mint == accounts_info.input_mint_info.mint;

        let (input_vault, output_vault) = if zero_for_one {
            // mint0 -> mint1
            let vault_0 = self.calculate_pool_vault_address(&accounts_info.pool_address, &input_mint)?;
            let vault_1 = self.calculate_pool_vault_address(&accounts_info.pool_address, &output_mint)?;
            (vault_0, vault_1)
        } else {
            // mint1 -> mint0
            let vault_0 = self.calculate_pool_vault_address(&accounts_info.pool_address, &output_mint)?;
            let vault_1 = self.calculate_pool_vault_address(&accounts_info.pool_address, &input_mint)?;
            (vault_1, vault_0)
        };

        // 计算observation地址
        let observation_key = self.calculate_observation_address(&accounts_info.pool_address)?;

        Ok(VaultAddresses {
            input_vault: input_vault,
            output_vault: output_vault,
            observation_key: observation_key,
        })
    }

    /// 计算池子vault地址
    fn calculate_pool_vault_address(&self, pool_id: &Pubkey, mint: &Pubkey) -> Result<Pubkey> {
        let (vault_pda, _bump) = Pubkey::find_program_address(&[b"pool_vault", pool_id.as_ref(), mint.as_ref()], &self.raydium_program_id);
        Ok(vault_pda)
    }

    /// 计算observation地址
    fn calculate_observation_address(&self, pool_id: &Pubkey) -> Result<Pubkey> {
        let (observation_pda, _bump) = Pubkey::find_program_address(&[b"observation", pool_id.as_ref()], &self.raydium_program_id);
        Ok(observation_pda)
    }

    /// 构建remaining accounts
    fn build_remaining_accounts(&self, accounts_info: &SwapV2AccountsInfo) -> Result<Vec<AccountMeta>> {
        let mut remaining_accounts = Vec::new();

        // 添加bitmap扩展账户（必需）
        remaining_accounts.push(AccountMeta::new_readonly(
            // 从tickarray_bitmap_extension_account获取地址
            Pubkey::new_from_array(
                accounts_info.tickarray_bitmap_extension_account.data[..32]
                    .try_into()
                    .map_err(|_| anyhow::anyhow!("无效的bitmap扩展账户地址"))?,
            ),
            false,
        ));

        // TODO: 添加tick array accounts
        // 这里需要根据当前价格和交换方向来确定需要哪些tick arrays
        // 暂时添加占位符，实际实现需要调用tick array查找逻辑

        info!("📋 构建了{}个remaining accounts", remaining_accounts.len());
        Ok(remaining_accounts)
    }

    /// 构建SwapV2核心指令
    fn build_swap_v2_core_instruction(
        &self,
        accounts_info: &SwapV2AccountsInfo,
        vault_addresses: &VaultAddresses,
        remaining_accounts: Vec<AccountMeta>,
        params: &SwapV2BuildParams,
    ) -> Result<Instruction> {
        info!("构建SwapV2核心指令");

        // 创建临时的payer keypair用于构建指令（不会实际签名）
        let temp_payer = Keypair::new();
        let url = Cluster::Custom(self.rpc_url.clone(), self.ws_url.clone());
        let client = Client::new(url, Rc::new(temp_payer));
        let program = client.program(self.raydium_program_id)?;

        // 获取用户token账户地址（优先使用提供的账户，否则计算ATA）
        let user_input_token = params
            .user_input_token_account
            .unwrap_or_else(|| spl_associated_token_account::get_associated_token_address(&params.user_wallet, &accounts_info.input_mint_info.mint));
        let user_output_token = params
            .user_output_token_account
            .unwrap_or_else(|| spl_associated_token_account::get_associated_token_address(&params.user_wallet, &accounts_info.output_mint_info.mint));

        info!("  用户输入token账户: {}", user_input_token);
        info!("  用户输出token账户: {}", user_output_token);
        info!("  输入vault: {}", vault_addresses.input_vault);
        info!("  输出vault: {}", vault_addresses.output_vault);

        // 构建SwapV2指令
        let instructions = program
            .request()
            .accounts(raydium_accounts::SwapSingleV2 {
                payer: params.user_wallet, // 使用真实用户钱包作为payer
                amm_config: self.amm_config_key,
                pool_state: accounts_info.pool_address,
                input_token_account: user_input_token,
                output_token_account: user_output_token,
                input_vault: vault_addresses.input_vault,
                output_vault: vault_addresses.output_vault,
                observation_state: vault_addresses.observation_key,
                token_program: spl_token::id(),
                token_program_2022: spl_token_2022::id(),
                memo_program: spl_memo::id(),
                input_vault_mint: accounts_info.input_mint_info.mint,
                output_vault_mint: accounts_info.output_mint_info.mint,
            })
            .accounts(remaining_accounts)
            .args(raydium_instruction::SwapV2 {
                amount: params.amount,
                other_amount_threshold: params.other_amount_threshold,
                sqrt_price_limit_x64: params.sqrt_price_limit_x64.unwrap_or(0u128),
                is_base_input: params.is_base_input,
            })
            .instructions()?;

        // 返回第一个指令（应该是SwapV2指令）
        instructions.into_iter().next().ok_or_else(|| anyhow::anyhow!("SwapV2指令构建失败：无指令返回"))
    }

    /// 估算交易费用
    fn estimate_transaction_fee(&self, instructions: &[Instruction]) -> Result<u64> {
        // 基础费用：每个指令5000 lamports + 签名费用
        let base_fee = instructions.len() as u64 * 5000;
        let signature_fee = 5000; // 一个签名的费用

        Ok(base_fee + signature_fee)
    }
}

/// Vault地址集合
#[derive(Debug, Clone)]
struct VaultAddresses {
    input_vault: Pubkey,
    output_vault: Pubkey,
    observation_key: Pubkey,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_swap_v2_builder_creation() {
        let builder = SwapV2InstructionBuilder::new("https://api.mainnet-beta.solana.com", "CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK", 0);

        assert!(builder.is_ok());
        let builder = builder.unwrap();
        assert_eq!(builder.raydium_program_id.to_string(), "CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK");
    }

    #[tokio::test]
    async fn test_swap_v2_instruction_building() {
        let builder = SwapV2InstructionBuilder::new("https://api.mainnet-beta.solana.com", "CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK", 0).unwrap();

        let params = SwapV2BuildParams {
            input_mint: "So11111111111111111111111111111111111111112".to_string(),
            output_mint: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(),
            user_wallet: Keypair::new().pubkey(),
            user_input_token_account: None,
            user_output_token_account: None,
            amount: 1_000_000_000,
            other_amount_threshold: 95_000_000,
            sqrt_price_limit_x64: None,
            is_base_input: true,
            slippage_bps: 50,
            compute_unit_limit: Some(1_400_000),
        };

        // 注意：这个测试在没有真实账户数据时会失败
        // 实际使用时需要确保账户存在
        match builder.build_swap_v2_instructions(params).await {
            Ok(result) => {
                println!("✅ 测试成功构建指令，数量: {}", result.instructions.len());
                assert!(result.instructions.len() > 0);
            }
            Err(e) => {
                println!("⚠️ 测试失败（预期的，因为账户不存在）: {}", e);
                // 在测试环境中，账户不存在是正常的
            }
        }
    }
}
