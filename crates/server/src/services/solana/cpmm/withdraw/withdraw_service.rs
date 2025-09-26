// CpmmWithdrawService handles classic AMM pool withdraw operations
// 100%忠实CLI的Withdraw逻辑实现

use super::super::super::shared::SharedContext;
use crate::dtos::solana::common::TransactionStatus;
use crate::dtos::solana::cpmm::withdraw::{
    CpmmWithdrawAndSendRequest, CpmmWithdrawAndSendResponse, CpmmWithdrawCompute, CpmmWithdrawRequest,
    CpmmWithdrawResponse, WithdrawPoolInfo,
};
use anchor_lang::Discriminator;
use anyhow::Result;
use arrayref::array_ref;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use bytemuck::Pod;
use raydium_cp_swap::{curve, instruction, states, AUTH_SEED};
use solana_sdk::{instruction::Instruction, pubkey::Pubkey, signature::Keypair, transaction::Transaction};
use spl_associated_token_account;
use spl_token_2022::extension::PodStateWithExtensions;
use spl_token_2022::extension::{transfer_fee::TransferFeeConfig, BaseState, BaseStateWithExtensions};
use spl_token_2022::pod::PodAccount;
use std::ops::Mul;
use std::str::FromStr;
use std::sync::Arc;
use tracing::info;

/// Transfer fee information - 完全按照CLI定义
#[derive(Debug)]
pub struct TransferFeeInfo {
    pub mint: Pubkey,
    pub owner: Pubkey,
    pub transfer_fee: u64,
}

/// CpmmWithdrawService handles classic AMM pool withdraw operations
pub struct CpmmWithdrawService {
    shared: Arc<SharedContext>,
}

impl CpmmWithdrawService {
    /// Create a new CpmmWithdrawService with shared context
    pub fn new(shared: Arc<SharedContext>) -> Self {
        Self { shared }
    }

    /// 提取流动性交易(未签名) - 100%忠实CLI逻辑
    pub async fn withdraw_liquidity(&self, request: CpmmWithdrawRequest) -> Result<CpmmWithdrawResponse> {
        info!("🏗️ 开始构建CPMM提取流动性交易 (基于CLI逻辑)");
        info!("  池子ID: {}", request.pool_id);
        info!("  LP代币账户: {}", request.user_lp_token);
        info!("  LP代币数量: {}", request.lp_token_amount);
        info!("  用户钱包: {}", request.user_wallet);

        let pool_id = Pubkey::from_str(&request.pool_id)?;
        let user_lp_token = Pubkey::from_str(&request.user_lp_token)?;
        let user_wallet = Pubkey::from_str(&request.user_wallet)?;
        let slippage = request.slippage.unwrap_or(0.5);

        // CLI逻辑第1步：获取池子状态
        let pool_state: states::PoolState = self.get_pool_state(pool_id).await?;
        info!("  Token0 Mint: {}", pool_state.token_0_mint);
        info!("  Token1 Mint: {}", pool_state.token_1_mint);
        info!("  LP Mint: {}", pool_state.lp_mint);

        // CLI逻辑第2步：批量加载账户信息确保数据一致性
        let load_pubkeys = vec![pool_id, pool_state.token_0_vault, pool_state.token_1_vault];
        let rsps = self.shared.rpc_client.get_multiple_accounts(&load_pubkeys)?;
        let [pool_account, token_0_vault_account, token_1_vault_account] = array_ref![rsps, 0, 3];

        // CLI逻辑第3步：解码账户数据
        let pool_state = self.deserialize_pool_state(pool_account.as_ref().unwrap())?;
        let token_0_vault_info = self.unpack_token_account(&token_0_vault_account.as_ref().unwrap().data)?;
        let token_1_vault_info = self.unpack_token_account(&token_1_vault_account.as_ref().unwrap().data)?;

        // CLI逻辑第4步：计算扣除费用后的金库总量
        let (total_token_0_amount, total_token_1_amount) = pool_state
            .vault_amount_without_fee(
                token_0_vault_info.base.amount.into(),
                token_1_vault_info.base.amount.into(),
            )
            .unwrap();

        // CLI逻辑第5步：LP代币到基础代币转换
        let results = curve::CurveCalculator::lp_tokens_to_trading_tokens(
            u128::from(request.lp_token_amount),
            u128::from(pool_state.lp_supply),
            u128::from(total_token_0_amount),
            u128::from(total_token_1_amount),
            curve::RoundDirection::Ceiling,
        )
        .ok_or_else(|| anyhow::anyhow!("无法计算LP代币转换，流动性可能为零"))?;

        let token_0_amount = results.token_0_amount as u64;
        let token_1_amount = results.token_1_amount as u64;

        info!("💰 LP代币转换结果:");
        info!("  Token0数量: {}", token_0_amount);
        info!("  Token1数量: {}", token_1_amount);

        // CLI逻辑第6步：应用滑点保护（round_up=false 用于提取）
        let amount_0_with_slippage = amount_with_slippage(token_0_amount, slippage / 100.0, false);
        let amount_1_with_slippage = amount_with_slippage(token_1_amount, slippage / 100.0, false);

        // CLI逻辑第7步：计算transfer fee
        let transfer_fee = get_pool_mints_transfer_fee(
            &self.shared.rpc_client,
            pool_state.token_0_mint,
            pool_state.token_1_mint,
            amount_0_with_slippage,
            amount_1_with_slippage,
        );

        info!("💸 转账费用:");
        info!("  Transfer fee 0: {}", transfer_fee.0.transfer_fee);
        info!("  Transfer fee 1: {}", transfer_fee.1.transfer_fee);

        // CLI逻辑第8步：计算最小输出数量（扣除转账费）
        let amount_0_min = amount_0_with_slippage
            .checked_sub(transfer_fee.0.transfer_fee)
            .unwrap_or(0);
        let amount_1_min = amount_1_with_slippage
            .checked_sub(transfer_fee.1.transfer_fee)
            .unwrap_or(0);

        info!("🔒 最小输出数量:");
        info!("  Amount 0 min: {}", amount_0_min);
        info!("  Amount 1 min: {}", amount_1_min);

        // CLI逻辑第9步：构建withdraw指令
        let instructions = self
            .build_withdraw_instructions(
                pool_id,
                &pool_state,
                &user_wallet,
                &user_lp_token,
                request.lp_token_amount,
                amount_0_min,
                amount_1_min,
            )
            .await?;

        // 计算用户ATA地址
        let user_token_0_ata =
            spl_associated_token_account::get_associated_token_address(&user_wallet, &pool_state.token_0_mint);
        let user_token_1_ata =
            spl_associated_token_account::get_associated_token_address(&user_wallet, &pool_state.token_1_mint);

        // 创建交易
        let mut transaction = Transaction::new_with_payer(&instructions, Some(&user_wallet));

        // 获取最新的blockhash
        let recent_blockhash = self.shared.rpc_client.get_latest_blockhash()?;
        transaction.message.recent_blockhash = recent_blockhash;

        // 序列化交易为Base64
        let serialized_transaction = bincode::serialize(&transaction)?;
        let transaction_base64 = BASE64_STANDARD.encode(&serialized_transaction);

        let now = chrono::Utc::now().timestamp();

        info!("✅ CPMM提取流动性交易构建成功");

        Ok(CpmmWithdrawResponse {
            transaction: transaction_base64,
            transaction_message: "CPMM提取流动性交易".to_string(),
            pool_id: request.pool_id,
            token_0_mint: pool_state.token_0_mint.to_string(),
            token_1_mint: pool_state.token_1_mint.to_string(),
            lp_mint: pool_state.lp_mint.to_string(),
            lp_token_amount: request.lp_token_amount,
            amount_0_min,
            amount_1_min,
            token_0_amount,
            token_1_amount,
            transfer_fee_0: transfer_fee.0.transfer_fee,
            transfer_fee_1: transfer_fee.1.transfer_fee,
            user_token_0_ata: user_token_0_ata.to_string(),
            user_token_1_ata: user_token_1_ata.to_string(),
            timestamp: now,
        })
    }

    /// 提取流动性并发送交易 - 100%忠实CLI逻辑
    pub async fn withdraw_liquidity_and_send_transaction(
        &self,
        request: CpmmWithdrawAndSendRequest,
    ) -> Result<CpmmWithdrawAndSendResponse> {
        info!("🚀 开始提取CPMM流动性并发送交易 (基于CLI逻辑)");
        info!("  池子ID: {}", request.pool_id);
        info!("  LP代币账户: {}", request.user_lp_token);
        info!("  LP代币数量: {}", request.lp_token_amount);
        info!("  用户钱包: {}", request.user_wallet);

        let pool_id = Pubkey::from_str(&request.pool_id)?;
        let user_lp_token = Pubkey::from_str(&request.user_lp_token)?;
        let user_wallet = Pubkey::from_str(&request.user_wallet)?;
        let slippage = request.slippage.unwrap_or(0.5);

        // 从环境配置中获取私钥
        let private_key = self
            .shared
            .app_config
            .private_key
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("私钥未配置，请检查 .env.development 文件中的 PRIVATE_KEY"))?;

        let user_keypair = Keypair::from_base58_string(private_key);

        // CLI逻辑第1步：获取池子状态
        let pool_state: states::PoolState = self.get_pool_state(pool_id).await?;

        // CLI逻辑第2步：批量加载账户信息确保数据一致性
        let load_pubkeys = vec![pool_id, pool_state.token_0_vault, pool_state.token_1_vault];
        let rsps = self.shared.rpc_client.get_multiple_accounts(&load_pubkeys)?;
        let [pool_account, token_0_vault_account, token_1_vault_account] = array_ref![rsps, 0, 3];

        // CLI逻辑第3步：解码账户数据
        let pool_state = self.deserialize_pool_state(pool_account.as_ref().unwrap())?;
        let token_0_vault_info = self.unpack_token_account(&token_0_vault_account.as_ref().unwrap().data)?;
        let token_1_vault_info = self.unpack_token_account(&token_1_vault_account.as_ref().unwrap().data)?;

        // CLI逻辑第4步：计算扣除费用后的金库总量
        let (total_token_0_amount, total_token_1_amount) = pool_state
            .vault_amount_without_fee(
                token_0_vault_info.base.amount.into(),
                token_1_vault_info.base.amount.into(),
            )
            .unwrap();

        // CLI逻辑第5步：LP代币到基础代币转换
        let results = curve::CurveCalculator::lp_tokens_to_trading_tokens(
            u128::from(request.lp_token_amount),
            u128::from(pool_state.lp_supply),
            u128::from(total_token_0_amount),
            u128::from(total_token_1_amount),
            curve::RoundDirection::Ceiling,
        )
        .ok_or_else(|| anyhow::anyhow!("无法计算LP代币转换，流动性可能为零"))?;

        let token_0_amount = results.token_0_amount as u64;
        let token_1_amount = results.token_1_amount as u64;

        // CLI逻辑第6步：应用滑点保护（round_up=false 用于提取）
        let amount_0_with_slippage = amount_with_slippage(token_0_amount, slippage / 100.0, false);
        let amount_1_with_slippage = amount_with_slippage(token_1_amount, slippage / 100.0, false);

        // CLI逻辑第7步：计算transfer fee
        let transfer_fee = get_pool_mints_transfer_fee(
            &self.shared.rpc_client,
            pool_state.token_0_mint,
            pool_state.token_1_mint,
            amount_0_with_slippage,
            amount_1_with_slippage,
        );

        // CLI逻辑第8步：计算最小输出数量（扣除转账费）
        let amount_0_min = amount_0_with_slippage
            .checked_sub(transfer_fee.0.transfer_fee)
            .unwrap_or(0);
        let amount_1_min = amount_1_with_slippage
            .checked_sub(transfer_fee.1.transfer_fee)
            .unwrap_or(0);

        // CLI逻辑第9步：构建withdraw指令
        let instructions = self
            .build_withdraw_instructions(
                pool_id,
                &pool_state,
                &user_wallet,
                &user_lp_token,
                request.lp_token_amount,
                amount_0_min,
                amount_1_min,
            )
            .await?;

        // 计算用户ATA地址
        let user_token_0_ata =
            spl_associated_token_account::get_associated_token_address(&user_wallet, &pool_state.token_0_mint);
        let user_token_1_ata =
            spl_associated_token_account::get_associated_token_address(&user_wallet, &pool_state.token_1_mint);

        // CLI逻辑第10步：构建并发送交易（完全按照CLI逻辑）
        let signers = vec![&user_keypair];
        let recent_hash = self.shared.rpc_client.get_latest_blockhash()?;
        let transaction = Transaction::new_signed_with_payer(&instructions, Some(&user_wallet), &signers, recent_hash);

        // 发送交易
        let signature = self.shared.rpc_client.send_and_confirm_transaction(&transaction)?;

        info!("✅ CPMM提取流动性成功，交易签名: {}", signature);

        // 构建响应
        let explorer_url = format!("https://explorer.solana.com/tx/{}", signature);
        let now = chrono::Utc::now().timestamp();

        Ok(CpmmWithdrawAndSendResponse {
            signature: signature.to_string(),
            pool_id: request.pool_id,
            token_0_mint: pool_state.token_0_mint.to_string(),
            token_1_mint: pool_state.token_1_mint.to_string(),
            lp_mint: pool_state.lp_mint.to_string(),
            lp_token_amount: request.lp_token_amount,
            actual_amount_0: token_0_amount,
            actual_amount_1: token_1_amount,
            amount_0_min,
            amount_1_min,
            transfer_fee_0: transfer_fee.0.transfer_fee,
            transfer_fee_1: transfer_fee.1.transfer_fee,
            user_token_0_ata: user_token_0_ata.to_string(),
            user_token_1_ata: user_token_1_ata.to_string(),
            status: TransactionStatus::Finalized,
            explorer_url,
            timestamp: now,
        })
    }

    /// 计算提取流动性结果 - 预览功能
    pub async fn compute_withdraw(
        &self,
        pool_id: &str,
        lp_token_amount: u64,
        slippage: Option<f64>,
    ) -> Result<CpmmWithdrawCompute> {
        let pool_id = Pubkey::from_str(pool_id)?;
        let slippage = slippage.unwrap_or(0.5);

        // 获取池子状态
        let pool_state: states::PoolState = self.get_pool_state(pool_id).await?;

        // 批量加载账户信息
        let load_pubkeys = vec![pool_id, pool_state.token_0_vault, pool_state.token_1_vault];
        let rsps = self.shared.rpc_client.get_multiple_accounts(&load_pubkeys)?;
        let [pool_account, token_0_vault_account, token_1_vault_account] = array_ref![rsps, 0, 3];

        let pool_state = self.deserialize_pool_state(pool_account.as_ref().unwrap())?;
        let token_0_vault_info = self.unpack_token_account(&token_0_vault_account.as_ref().unwrap().data)?;
        let token_1_vault_info = self.unpack_token_account(&token_1_vault_account.as_ref().unwrap().data)?;

        let (total_token_0_amount, total_token_1_amount) = pool_state
            .vault_amount_without_fee(
                token_0_vault_info.base.amount.into(),
                token_1_vault_info.base.amount.into(),
            )
            .unwrap();

        let results = curve::CurveCalculator::lp_tokens_to_trading_tokens(
            u128::from(lp_token_amount),
            u128::from(pool_state.lp_supply),
            u128::from(total_token_0_amount),
            u128::from(total_token_1_amount),
            curve::RoundDirection::Ceiling,
        )
        .ok_or_else(|| anyhow::anyhow!("无法计算LP代币转换"))?;

        let token_0_amount = results.token_0_amount as u64;
        let token_1_amount = results.token_1_amount as u64;

        let amount_0_with_slippage = amount_with_slippage(token_0_amount, slippage / 100.0, false);
        let amount_1_with_slippage = amount_with_slippage(token_1_amount, slippage / 100.0, false);

        let transfer_fee = get_pool_mints_transfer_fee(
            &self.shared.rpc_client,
            pool_state.token_0_mint,
            pool_state.token_1_mint,
            amount_0_with_slippage,
            amount_1_with_slippage,
        );

        let amount_0_min = amount_0_with_slippage
            .checked_sub(transfer_fee.0.transfer_fee)
            .unwrap_or(0);
        let amount_1_min = amount_1_with_slippage
            .checked_sub(transfer_fee.1.transfer_fee)
            .unwrap_or(0);

        Ok(CpmmWithdrawCompute {
            pool_id: pool_id.to_string(),
            token_0_mint: pool_state.token_0_mint.to_string(),
            token_1_mint: pool_state.token_1_mint.to_string(),
            lp_mint: pool_state.lp_mint.to_string(),
            lp_token_amount,
            token_0_amount,
            token_1_amount,
            amount_0_with_slippage,
            amount_1_with_slippage,
            amount_0_min,
            amount_1_min,
            transfer_fee_0: transfer_fee.0.transfer_fee,
            transfer_fee_1: transfer_fee.1.transfer_fee,
            slippage,
            pool_info: WithdrawPoolInfo {
                total_token_0_amount,
                total_token_1_amount,
                lp_supply: pool_state.lp_supply,
                token_0_mint: pool_state.token_0_mint.to_string(),
                token_1_mint: pool_state.token_1_mint.to_string(),
                lp_mint: pool_state.lp_mint.to_string(),
                token_0_vault: pool_state.token_0_vault.to_string(),
                token_1_vault: pool_state.token_1_vault.to_string(),
            },
        })
    }

    /// 构建withdraw指令 - 忠实CLI的withdraw_instr逻辑
    async fn build_withdraw_instructions(
        &self,
        pool_id: Pubkey,
        pool_state: &states::PoolState,
        user_wallet: &Pubkey,
        user_lp_token: &Pubkey,
        lp_token_amount: u64,
        minimum_token_0_amount: u64,
        minimum_token_1_amount: u64,
    ) -> Result<Vec<Instruction>> {
        let raydium_cp_program = self.get_raydium_cp_program_id()?;

        // 计算权限PDA
        let (authority, _) = Pubkey::find_program_address(&[AUTH_SEED.as_bytes()], &raydium_cp_program);

        // 计算用户Token ATA地址
        let user_token_0_account =
            spl_associated_token_account::get_associated_token_address(user_wallet, &pool_state.token_0_mint);
        let user_token_1_account =
            spl_associated_token_account::get_associated_token_address(user_wallet, &pool_state.token_1_mint);

        let mut instructions = Vec::new();

        // 1. 创建用户Token0 ATA账户（如果不存在）
        info!("📝 确保Token0 ATA账户存在: {}", user_token_0_account);
        let create_token0_ata_ix =
            spl_associated_token_account::instruction::create_associated_token_account_idempotent(
                user_wallet,
                user_wallet,
                &pool_state.token_0_mint,
                &spl_token::id(),
            );
        instructions.push(create_token0_ata_ix);

        // 2. 创建用户Token1 ATA账户（如果不存在）
        info!("📝 确保Token1 ATA账户存在: {}", user_token_1_account);
        let create_token1_ata_ix =
            spl_associated_token_account::instruction::create_associated_token_account_idempotent(
                user_wallet,
                user_wallet,
                &pool_state.token_1_mint,
                &spl_token::id(),
            );
        instructions.push(create_token1_ata_ix);

        // 3. 构建Withdraw指令 - 完全按照CLI的withdraw_instr
        let withdraw_instruction = self.build_withdraw_instruction(
            raydium_cp_program,
            *user_wallet,
            authority,
            pool_id,
            *user_lp_token,
            user_token_0_account,
            user_token_1_account,
            pool_state.token_0_vault,
            pool_state.token_1_vault,
            pool_state.token_0_mint,
            pool_state.token_1_mint,
            pool_state.lp_mint,
            lp_token_amount,
            minimum_token_0_amount,
            minimum_token_1_amount,
        )?;
        instructions.push(withdraw_instruction);

        info!("✅ 构建完成，共{}条指令: 2个ATA创建 + 1个Withdraw", instructions.len());

        Ok(instructions)
    }

    /// 构建单个Withdraw指令 - 忠实CLI逻辑
    fn build_withdraw_instruction(
        &self,
        program_id: Pubkey,
        owner: Pubkey,
        authority: Pubkey,
        pool_state: Pubkey,
        owner_lp_token: Pubkey,
        token_0_account: Pubkey,
        token_1_account: Pubkey,
        token_0_vault: Pubkey,
        token_1_vault: Pubkey,
        vault_0_mint: Pubkey,
        vault_1_mint: Pubkey,
        lp_mint: Pubkey,
        lp_token_amount: u64,
        minimum_token_0_amount: u64,
        minimum_token_1_amount: u64,
    ) -> Result<Instruction> {
        // 构建账户 - 按照CLI中raydium_cp_accounts::Withdraw的顺序
        let accounts = vec![
            solana_sdk::instruction::AccountMeta::new(owner, true), // owner (signer)
            solana_sdk::instruction::AccountMeta::new_readonly(authority, false), // authority
            solana_sdk::instruction::AccountMeta::new(pool_state, false), // pool_state
            solana_sdk::instruction::AccountMeta::new(owner_lp_token, false), // owner_lp_token
            solana_sdk::instruction::AccountMeta::new(token_0_account, false), // token_0_account
            solana_sdk::instruction::AccountMeta::new(token_1_account, false), // token_1_account
            solana_sdk::instruction::AccountMeta::new(token_0_vault, false), // token_0_vault
            solana_sdk::instruction::AccountMeta::new(token_1_vault, false), // token_1_vault
            solana_sdk::instruction::AccountMeta::new_readonly(spl_token::id(), false), // token_program
            solana_sdk::instruction::AccountMeta::new_readonly(spl_token_2022::id(), false), // token_program_2022
            solana_sdk::instruction::AccountMeta::new_readonly(vault_0_mint, false), // vault_0_mint
            solana_sdk::instruction::AccountMeta::new_readonly(vault_1_mint, false), // vault_1_mint
            solana_sdk::instruction::AccountMeta::new(lp_mint, false), // lp_mint
            solana_sdk::instruction::AccountMeta::new_readonly(spl_memo::id(), false), // memo_program
        ];

        // 构建指令数据
        let instruction_data =
            self.build_withdraw_instruction_data(lp_token_amount, minimum_token_0_amount, minimum_token_1_amount)?;

        Ok(Instruction {
            program_id,
            accounts,
            data: instruction_data,
        })
    }

    /// 构建withdraw指令数据 - 忠实CLI逻辑
    fn build_withdraw_instruction_data(
        &self,
        lp_token_amount: u64,
        minimum_token_0_amount: u64,
        minimum_token_1_amount: u64,
    ) -> Result<Vec<u8>> {
        // CPMM Withdraw指令的discriminator
        let discriminator = instruction::Withdraw::DISCRIMINATOR;

        let mut data = Vec::new();
        data.extend_from_slice(&discriminator);
        data.extend_from_slice(&lp_token_amount.to_le_bytes());
        data.extend_from_slice(&minimum_token_0_amount.to_le_bytes());
        data.extend_from_slice(&minimum_token_1_amount.to_le_bytes());

        info!("🔧 构建的CPMM Withdraw指令数据长度: {} bytes", data.len());

        Ok(data)
    }

    /// 获取池子状态 - 使用anchor反序列化
    async fn get_pool_state(&self, pool_id: Pubkey) -> Result<states::PoolState> {
        let account = self.shared.rpc_client.get_account(&pool_id)?;
        self.deserialize_pool_state(&account)
    }

    /// 反序列化池子状态
    fn deserialize_pool_state(&self, account: &solana_sdk::account::Account) -> Result<states::PoolState> {
        let mut data: &[u8] = &account.data;
        anchor_lang::AccountDeserialize::try_deserialize(&mut data).map_err(Into::into)
    }

    /// 解包token账户
    fn unpack_token_account<'a>(&self, token_data: &'a [u8]) -> Result<PodStateWithExtensions<'a, PodAccount>> {
        PodStateWithExtensions::<PodAccount>::unpack(token_data).map_err(Into::into)
    }

    /// 获取Raydium CP程序ID
    fn get_raydium_cp_program_id(&self) -> Result<Pubkey> {
        let program_id_str = std::env::var("RAYDIUM_CP_PROGRAM_ID")
            .unwrap_or_else(|_| "FairxoKThzWcDy9avKPsADqzni18LrXxKAZEHdXVo5gi".to_string());
        info!("🔍 获取CPMM程序ID: {}", program_id_str);
        Pubkey::from_str(&program_id_str).map_err(Into::into)
    }

    /// 使用unpack_mint解包mint数据 - 从CLI移植
    #[allow(dead_code)]
    fn unpack_mint<'a>(
        &self,
        token_data: &'a [u8],
    ) -> Result<PodStateWithExtensions<'a, spl_token_2022::pod::PodMint>> {
        use spl_token_2022::pod::PodMint;
        PodStateWithExtensions::<PodMint>::unpack(token_data).map_err(Into::into)
    }
}

/// 滑点计算工具函数 - 完全按照CLI实现
fn amount_with_slippage(amount: u64, slippage: f64, round_up: bool) -> u64 {
    if round_up {
        (amount as f64).mul(1_f64 + slippage).ceil() as u64
    } else {
        (amount as f64).mul(1_f64 - slippage).floor() as u64
    }
}

/// 获取池子mints的transfer fee - 完全按照CLI实现
fn get_pool_mints_transfer_fee(
    rpc_client: &solana_client::rpc_client::RpcClient,
    token_mint_0: Pubkey,
    token_mint_1: Pubkey,
    pre_fee_amount_0: u64,
    pre_fee_amount_1: u64,
) -> (TransferFeeInfo, TransferFeeInfo) {
    let load_accounts = vec![token_mint_0, token_mint_1];
    let rsps = rpc_client.get_multiple_accounts(&load_accounts).unwrap();
    let epoch = rpc_client.get_epoch_info().unwrap().epoch;
    let mint0_account = rsps[0].clone().ok_or("load mint0 rps error!").unwrap();
    let mint1_account = rsps[1].clone().ok_or("load mint0 rps error!").unwrap();

    use spl_token_2022::pod::PodMint;
    let mint0_state = PodStateWithExtensions::<PodMint>::unpack(&mint0_account.data).unwrap();
    let mint1_state = PodStateWithExtensions::<PodMint>::unpack(&mint1_account.data).unwrap();

    (
        TransferFeeInfo {
            mint: token_mint_0,
            owner: mint0_account.owner,
            transfer_fee: get_transfer_fee(&mint0_state, epoch, pre_fee_amount_0),
        },
        TransferFeeInfo {
            mint: token_mint_1,
            owner: mint1_account.owner,
            transfer_fee: get_transfer_fee(&mint1_state, epoch, pre_fee_amount_1),
        },
    )
}

/// 计算输入金额的transfer fee - 完全按照CLI实现
fn get_transfer_fee<'data, S: BaseState + Pod>(
    account_state: &PodStateWithExtensions<'data, S>,
    epoch: u64,
    pre_fee_amount: u64,
) -> u64 {
    let fee = if let Ok(transfer_fee_config) = account_state.get_extension::<TransferFeeConfig>() {
        transfer_fee_config.calculate_epoch_fee(epoch, pre_fee_amount).unwrap()
    } else {
        0
    };
    fee
}
