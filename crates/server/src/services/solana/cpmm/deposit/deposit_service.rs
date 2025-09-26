use crate::dtos::solana::common::TransactionStatus;
use crate::dtos::solana::cpmm::deposit::{
    CpmmDepositAndSendRequest, CpmmDepositAndSendResponse, CpmmDepositCompute, CpmmDepositRequest, CpmmDepositResponse,
    DepositPoolInfo,
};
use crate::services::solana::shared::SharedContext;
use anyhow::Result;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use raydium_cp_swap::{curve::CurveCalculator, instruction, states::PoolState};
use solana_sdk::{
    instruction::Instruction, pubkey::Pubkey, signature::Keypair, signer::Signer, transaction::Transaction,
};
use spl_associated_token_account;
use std::str::FromStr;
use std::sync::Arc;
use tracing::info;

// 导入必要的库和工具函数
use anchor_lang::{AccountDeserialize, Discriminator};
use anchor_spl::token_2022::spl_token_2022::{
    extension::{transfer_fee::TransferFeeConfig, BaseStateWithExtensions, PodStateWithExtensions},
    pod::{PodAccount, PodMint},
};

/// 转账费信息结构体 - 100%匹配CLI定义
#[derive(Debug)]
pub struct TransferFeeInfo {
    pub mint: Pubkey,
    pub owner: Pubkey,
    pub transfer_fee: u64,
}

/// 反序列化Anchor账户 - 100%匹配CLI实现
pub fn deserialize_anchor_account<T: AccountDeserialize>(account: &solana_sdk::account::Account) -> Result<T> {
    let mut data: &[u8] = &account.data;
    T::try_deserialize(&mut data).map_err(Into::into)
}

/// 解包Token账户 - 100%匹配CLI实现
pub fn unpack_token(token_data: &[u8]) -> Result<PodStateWithExtensions<'_, PodAccount>> {
    let token = PodStateWithExtensions::<PodAccount>::unpack(&token_data)?;
    Ok(token)
}

/// 解包Mint账户 - 100%匹配CLI实现
pub fn unpack_mint(token_data: &[u8]) -> Result<PodStateWithExtensions<'_, PodMint>> {
    let mint = PodStateWithExtensions::<PodMint>::unpack(&token_data)?;
    Ok(mint)
}

/// 计算反向转账费（用于存款） - 100%匹配CLI实现
pub fn get_transfer_inverse_fee(mint: &PodStateWithExtensions<'_, PodMint>, epoch: u64, post_fee_amount: u64) -> u64 {
    use anchor_spl::token_2022::spl_token_2022::extension::transfer_fee::MAX_FEE_BASIS_POINTS;

    if let Ok(transfer_fee_config) = mint.get_extension::<TransferFeeConfig>() {
        let transfer_fee = transfer_fee_config.get_epoch_fee(epoch);
        if u16::from(transfer_fee.transfer_fee_basis_points) == MAX_FEE_BASIS_POINTS {
            u64::from(transfer_fee.maximum_fee)
        } else {
            transfer_fee_config
                .calculate_inverse_epoch_fee(epoch, post_fee_amount)
                .unwrap_or(0)
        }
    } else {
        0
    }
}

/// 获取池子代币的反向转账费 - 100%匹配CLI实现
pub fn get_pool_mints_inverse_fee(
    rpc_client: &solana_client::rpc_client::RpcClient,
    token_mint_0: Pubkey,
    token_mint_1: Pubkey,
    post_fee_amount_0: u64,
    post_fee_amount_1: u64,
) -> Result<(TransferFeeInfo, TransferFeeInfo)> {
    let load_accounts = vec![token_mint_0, token_mint_1];
    let rsps = rpc_client.get_multiple_accounts(&load_accounts)?;
    let epoch = rpc_client.get_epoch_info()?.epoch;

    let mint0_account = rsps[0].as_ref().ok_or_else(|| anyhow::anyhow!("无法加载mint0账户"))?;
    let mint1_account = rsps[1].as_ref().ok_or_else(|| anyhow::anyhow!("无法加载mint1账户"))?;

    let mint0_state = unpack_mint(&mint0_account.data)?;
    let mint1_state = unpack_mint(&mint1_account.data)?;

    Ok((
        TransferFeeInfo {
            mint: token_mint_0,
            owner: mint0_account.owner,
            transfer_fee: get_transfer_inverse_fee(&mint0_state, epoch, post_fee_amount_0),
        },
        TransferFeeInfo {
            mint: token_mint_1,
            owner: mint1_account.owner,
            transfer_fee: get_transfer_inverse_fee(&mint1_state, epoch, post_fee_amount_1),
        },
    ))
}

/// 计算滑点金额 - 100%匹配CLI实现
use std::ops::Mul;
pub fn amount_with_slippage(amount: u64, slippage: f64, round_up: bool) -> u64 {
    if round_up {
        (amount as f64).mul(1_f64 + slippage).ceil() as u64
    } else {
        (amount as f64).mul(1_f64 - slippage).floor() as u64
    }
}

/// 创建关联代币账户指令 - 100%匹配CLI实现
pub fn create_ata_token_account_instr(
    token_program_id: Pubkey,
    mint: &Pubkey,
    owner: &Pubkey,
) -> Result<Vec<Instruction>> {
    let instruction = spl_associated_token_account::instruction::create_associated_token_account_idempotent(
        owner,             // 付费者
        owner,             // 账户所有者
        mint,              // mint地址
        &token_program_id, // token程序
    );

    Ok(vec![instruction])
}

/// 创建Deposit指令 - 100%匹配CLI实现
pub fn deposit_instr(
    cpmm_program_id: Pubkey,
    payer: Pubkey,
    pool_id: Pubkey,
    token_0_mint: Pubkey,
    token_1_mint: Pubkey,
    token_lp_mint: Pubkey,
    token_0_vault: Pubkey,
    token_1_vault: Pubkey,
    user_token_0_account: Pubkey,
    user_token_1_account: Pubkey,
    user_token_lp_account: Pubkey,
    lp_token_amount: u64,
    maximum_token_0_amount: u64,
    maximum_token_1_amount: u64,
) -> Result<Vec<Instruction>> {
    // 计算authority地址，与CLI完全一致
    const AUTH_SEED: &str = "vault_and_lp_mint_auth_seed";
    let (authority, _bump) = Pubkey::find_program_address(&[AUTH_SEED.as_bytes()], &cpmm_program_id);

    // 构造指令数据（使用deposit方法的discriminator）
    let mut instruction_data = Vec::new();
    let discriminator = instruction::Deposit::DISCRIMINATOR;
    instruction_data.extend_from_slice(&discriminator);
    instruction_data.extend_from_slice(&lp_token_amount.to_le_bytes());
    instruction_data.extend_from_slice(&maximum_token_0_amount.to_le_bytes());
    instruction_data.extend_from_slice(&maximum_token_1_amount.to_le_bytes());

    // 构建账户元数据，顺序与CLI完全一致
    let accounts = vec![
        solana_sdk::instruction::AccountMeta::new(payer, true), // owner (signer)
        solana_sdk::instruction::AccountMeta::new_readonly(authority, false), // authority
        solana_sdk::instruction::AccountMeta::new(pool_id, false), // pool_state
        solana_sdk::instruction::AccountMeta::new(user_token_lp_account, false), // owner_lp_token
        solana_sdk::instruction::AccountMeta::new(user_token_0_account, false), // token_0_account
        solana_sdk::instruction::AccountMeta::new(user_token_1_account, false), // token_1_account
        solana_sdk::instruction::AccountMeta::new(token_0_vault, false), // token_0_vault
        solana_sdk::instruction::AccountMeta::new(token_1_vault, false), // token_1_vault
        solana_sdk::instruction::AccountMeta::new_readonly(spl_token::id(), false), // token_program
        solana_sdk::instruction::AccountMeta::new_readonly(anchor_spl::token_2022::spl_token_2022::id(), false), // token_program_2022
        solana_sdk::instruction::AccountMeta::new_readonly(token_0_mint, false), // vault_0_mint
        solana_sdk::instruction::AccountMeta::new_readonly(token_1_mint, false), // vault_1_mint
        solana_sdk::instruction::AccountMeta::new(token_lp_mint, false),         // lp_mint
    ];

    let instruction = Instruction {
        program_id: cpmm_program_id,
        accounts,
        data: instruction_data,
    };

    Ok(vec![instruction])
}

/// CPMM存款服务
///
/// 提供基于Raydium恒定乘积做市商(CPMM)的流动性存款功能
pub struct CpmmDepositService {
    shared: Arc<SharedContext>,
}

impl CpmmDepositService {
    /// 创建新的CPMM存款服务实例
    pub fn new(shared: Arc<SharedContext>) -> Self {
        Self { shared }
    }

    /// 获取配置的CPMM程序ID
    fn get_cpmm_program_id(&self) -> Result<Pubkey> {
        Pubkey::from_str(&self.shared.app_config.raydium_cp_program_id)
            .map_err(|e| anyhow::anyhow!("无效的CPMM程序ID: {}", e))
    }

    /// 智能检测并转换代币地址
    ///
    /// 支持用户输入mint地址或ATA地址，自动转换为正确的ATA地址
    fn resolve_token_account(
        &self,
        input_address: &str,
        pool_state: &PoolState,
        user_wallet: &Pubkey,
    ) -> Result<Pubkey> {
        let input_pubkey = Pubkey::from_str(input_address)?;

        // 检查是否是池子中的代币mint
        if input_pubkey == pool_state.token_0_mint {
            Ok(
                spl_associated_token_account::get_associated_token_address_with_program_id(
                    user_wallet,
                    &pool_state.token_0_mint,
                    &pool_state.token_0_program,
                ),
            )
        } else if input_pubkey == pool_state.token_1_mint {
            Ok(
                spl_associated_token_account::get_associated_token_address_with_program_id(
                    user_wallet,
                    &pool_state.token_1_mint,
                    &pool_state.token_1_program,
                ),
            )
        } else {
            // 假设已经是ATA地址
            Ok(input_pubkey)
        }
    }

    /// 计算CPMM存款所需金额（不执行实际存款）
    ///
    /// 100%忠实地实现CLI的计算逻辑
    pub async fn compute_cpmm_deposit(&self, request: CpmmDepositRequest) -> Result<CpmmDepositCompute> {
        info!(
            "计算CPMM存款: pool_id={}, lp_token_amount={}",
            request.pool_id, request.lp_token_amount
        );

        let pool_id = Pubkey::from_str(&request.pool_id)?;
        let lp_token_amount = request.lp_token_amount;
        let slippage = request.slippage.unwrap_or(0.5) / 100.0; // 转换为小数

        // 获取配置的CPMM程序ID
        let cpmm_program_id = self.get_cpmm_program_id()?;

        // 加载池子状态
        let rpc_client = &self.shared.rpc_client;
        let pool_account = rpc_client.get_account(&pool_id)?;

        if pool_account.owner != cpmm_program_id {
            return Err(anyhow::anyhow!(
                "无效的池子地址，账户所有者不是CPMM程序: expected={}, actual={}",
                cpmm_program_id,
                pool_account.owner
            ));
        }

        let pool_state: PoolState = deserialize_anchor_account::<PoolState>(&pool_account)?;
        info!("✅ 池子状态加载成功");

        // CLI逻辑第2步：批量获取账户（与CLI完全相同）
        let load_pubkeys = vec![pool_id, pool_state.token_0_vault, pool_state.token_1_vault];
        let accounts = rpc_client.get_multiple_accounts(&load_pubkeys)?;

        let pool_account = accounts[0].as_ref().unwrap();
        let token_0_vault_account = accounts[1].as_ref().unwrap();
        let token_1_vault_account = accounts[2].as_ref().unwrap();

        // CLI逻辑第3步：解码账户数据
        let pool_state = deserialize_anchor_account::<PoolState>(pool_account)?;
        let token_0_vault_info = unpack_token(&token_0_vault_account.data)?;
        let token_1_vault_info = unpack_token(&token_1_vault_account.data)?;

        // CLI逻辑第4步：计算池子中的代币总量（扣除费用后）
        let (total_token_0_amount, total_token_1_amount) = pool_state
            .vault_amount_without_fee(
                token_0_vault_info.base.amount.into(),
                token_1_vault_info.base.amount.into(),
            )
            .unwrap();

        // 复制packed字段避免引用问题
        let lp_supply = pool_state.lp_supply;
        info!(
            "池子状态: total_token_0={}, total_token_1={}, lp_supply={}",
            total_token_0_amount, total_token_1_amount, lp_supply
        );

        // CLI逻辑第5步：使用CurveCalculator计算需要存入的代币数量
        let results = CurveCalculator::lp_tokens_to_trading_tokens(
            u128::from(lp_token_amount),
            u128::from(lp_supply),
            u128::from(total_token_0_amount),
            u128::from(total_token_1_amount),
            raydium_cp_swap::curve::RoundDirection::Ceiling,
        )
        .ok_or_else(|| anyhow::anyhow!("LP代币计算失败：零交易代币"))?;

        let token_0_amount = u64::try_from(results.token_0_amount)?;
        let token_1_amount = u64::try_from(results.token_1_amount)?;

        info!(
            "计算结果: token_0_amount={}, token_1_amount={}, lp_token_amount={}",
            token_0_amount, token_1_amount, lp_token_amount
        );

        // CLI逻辑第6步：计算含滑点的数量
        let amount_0_with_slippage = amount_with_slippage(token_0_amount, slippage, true);
        let amount_1_with_slippage = amount_with_slippage(token_1_amount, slippage, true);

        // CLI逻辑第7步：计算转账费
        let transfer_fee = get_pool_mints_inverse_fee(
            rpc_client,
            pool_state.token_0_mint,
            pool_state.token_1_mint,
            amount_0_with_slippage,
            amount_1_with_slippage,
        )?;

        info!(
            "转账费: transfer_fee_0={}, transfer_fee_1={}",
            transfer_fee.0.transfer_fee, transfer_fee.1.transfer_fee
        );

        // CLI逻辑第8步：计算最大输入金额（含转账费）
        let amount_0_max = amount_0_with_slippage.checked_add(transfer_fee.0.transfer_fee).unwrap();
        let amount_1_max = amount_1_with_slippage.checked_add(transfer_fee.1.transfer_fee).unwrap();

        info!("最终计算: amount_0_max={}, amount_1_max={}", amount_0_max, amount_1_max);

        Ok(CpmmDepositCompute {
            pool_id: request.pool_id,
            token_0_mint: pool_state.token_0_mint.to_string(),
            token_1_mint: pool_state.token_1_mint.to_string(),
            lp_mint: pool_state.lp_mint.to_string(),
            lp_token_amount,
            token_0_amount,
            token_1_amount,
            amount_0_with_slippage,
            amount_1_with_slippage,
            amount_0_max,
            amount_1_max,
            transfer_fee_0: transfer_fee.0.transfer_fee,
            transfer_fee_1: transfer_fee.1.transfer_fee,
            slippage: slippage * 100.0, // 转换回百分比
            pool_info: DepositPoolInfo {
                total_token_0_amount,
                total_token_1_amount,
                lp_supply,
                token_0_mint: pool_state.token_0_mint.to_string(),
                token_1_mint: pool_state.token_1_mint.to_string(),
            },
        })
    }

    /// 构建CPMM存款交易（不发送）
    ///
    /// 100%忠实地实现CLI的构建逻辑，生成可供客户端签名的交易
    pub async fn build_cpmm_deposit_transaction(&self, request: CpmmDepositRequest) -> Result<CpmmDepositResponse> {
        info!("🏗️ 构建CPMM存款交易: pool_id={}", request.pool_id);

        // 首先计算存款所需金额
        let compute_result = self.compute_cpmm_deposit(request.clone()).await?;

        let pool_id = Pubkey::from_str(&request.pool_id)?;
        let user_wallet = Keypair::from_base58_string(
            self.shared
                .app_config
                .private_key
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("私钥未配置"))?,
        )
        .pubkey();

        // 加载池子状态以获取详细信息
        let rpc_client = &self.shared.rpc_client;
        let pool_account = rpc_client.get_account(&pool_id)?;
        let pool_state: PoolState = deserialize_anchor_account::<PoolState>(&pool_account)?;

        // 解析用户代币账户地址
        let user_token_0 = self.resolve_token_account(&request.user_token_0, &pool_state, &user_wallet)?;
        let user_token_1 = self.resolve_token_account(&request.user_token_1, &pool_state, &user_wallet)?;
        let user_lp_token =
            spl_associated_token_account::get_associated_token_address(&user_wallet, &pool_state.lp_mint);

        // 构建交易指令
        let mut instructions = Vec::new();

        // CLI逻辑：创建用户LP代币ATA账户（如果不存在）
        info!("📝 确保LP代币ATA账户存在: {}", user_lp_token);
        let create_user_lp_token_instrs =
            create_ata_token_account_instr(spl_token::id(), &pool_state.lp_mint, &user_wallet)?;
        instructions.extend(create_user_lp_token_instrs);

        // 获取CPMM程序ID
        let cpmm_program_id = self.get_cpmm_program_id()?;

        // CLI逻辑：创建存款指令
        let deposit_instrs = deposit_instr(
            cpmm_program_id,
            user_wallet,
            pool_id,
            pool_state.token_0_mint,
            pool_state.token_1_mint,
            pool_state.lp_mint,
            pool_state.token_0_vault,
            pool_state.token_1_vault,
            user_token_0,
            user_token_1,
            user_lp_token,
            request.lp_token_amount,
            compute_result.amount_0_max,
            compute_result.amount_1_max,
        )?;
        instructions.extend(deposit_instrs);

        // 创建交易
        let mut transaction = Transaction::new_with_payer(&instructions, Some(&user_wallet));

        // 获取最新的blockhash
        let recent_blockhash = rpc_client.get_latest_blockhash()?;
        transaction.message.recent_blockhash = recent_blockhash;

        // 序列化交易为Base64
        let serialized_transaction = bincode::serialize(&transaction)?;
        let transaction_base64 = BASE64_STANDARD.encode(&serialized_transaction);

        let now = chrono::Utc::now().timestamp();

        info!("✅ CPMM存款交易构建成功");

        Ok(CpmmDepositResponse {
            transaction: transaction_base64,
            transaction_message: "CPMM存款交易".to_string(),
            pool_id: request.pool_id,
            token_0_mint: compute_result.token_0_mint,
            token_1_mint: compute_result.token_1_mint,
            lp_mint: compute_result.lp_mint,
            lp_token_amount: request.lp_token_amount,
            amount_0_max: compute_result.amount_0_max,
            amount_1_max: compute_result.amount_1_max,
            token_0_amount: compute_result.token_0_amount,
            token_1_amount: compute_result.token_1_amount,
            transfer_fee_0: compute_result.transfer_fee_0,
            transfer_fee_1: compute_result.transfer_fee_1,
            timestamp: now,
        })
    }

    /// 执行CPMM存款并发送交易
    ///
    /// 100%忠实地实现CLI的业务逻辑，使用本地私钥签名并发送交易
    pub async fn cpmm_deposit_and_send_transaction(
        &self,
        request: CpmmDepositAndSendRequest,
    ) -> Result<CpmmDepositAndSendResponse> {
        info!("🚀 执行CPMM存款并发送交易: pool_id={}", request.pool_id);

        let pool_id = Pubkey::from_str(&request.pool_id)?;
        let lp_token_amount = request.lp_token_amount;
        let slippage = request.slippage.unwrap_or(0.5) / 100.0;

        // 获取私钥和钱包信息
        let private_key = self
            .shared
            .app_config
            .private_key
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("私钥未配置，请检查 .env.development 文件中的 PRIVATE_KEY"))?;
        let user_keypair = Keypair::from_base58_string(private_key);
        let user_wallet = user_keypair.pubkey();

        // 加载池子状态
        let rpc_client = &self.shared.rpc_client;
        let pool_account = rpc_client.get_account(&pool_id)?;

        // 获取配置的CPMM程序ID
        let cpmm_program_id = self.get_cpmm_program_id()?;

        if pool_account.owner != cpmm_program_id {
            return Err(anyhow::anyhow!("无效的池子地址，账户所有者不是CPMM程序"));
        }

        let pool_state: PoolState = deserialize_anchor_account::<PoolState>(&pool_account)?;

        // 解析用户代币账户地址
        let user_token_0 = self.resolve_token_account(&request.user_token_0, &pool_state, &user_wallet)?;
        let user_token_1 = self.resolve_token_account(&request.user_token_1, &pool_state, &user_wallet)?;

        // CLI逻辑：批量获取账户（与CLI完全相同）
        let load_pubkeys = vec![pool_id, pool_state.token_0_vault, pool_state.token_1_vault];
        let accounts = rpc_client.get_multiple_accounts(&load_pubkeys)?;

        let pool_account = accounts[0].as_ref().unwrap();
        let token_0_vault_account = accounts[1].as_ref().unwrap();
        let token_1_vault_account = accounts[2].as_ref().unwrap();

        // 解码账户数据
        let pool_state = deserialize_anchor_account::<PoolState>(pool_account)?;
        let token_0_vault_info = unpack_token(&token_0_vault_account.data)?;
        let token_1_vault_info = unpack_token(&token_1_vault_account.data)?;

        // 计算池子中的代币总量
        let (total_token_0_amount, total_token_1_amount) = pool_state
            .vault_amount_without_fee(
                token_0_vault_info.base.amount.into(),
                token_1_vault_info.base.amount.into(),
            )
            .unwrap();

        // 使用CurveCalculator计算需要存入的代币数量
        let lp_supply = pool_state.lp_supply; // 复制packed字段
        let results = CurveCalculator::lp_tokens_to_trading_tokens(
            u128::from(lp_token_amount),
            u128::from(lp_supply),
            u128::from(total_token_0_amount),
            u128::from(total_token_1_amount),
            raydium_cp_swap::curve::RoundDirection::Ceiling,
        )
        .ok_or_else(|| anyhow::anyhow!("LP代币计算失败"))?;

        let token_0_amount = u64::try_from(results.token_0_amount)?;
        let token_1_amount = u64::try_from(results.token_1_amount)?;

        info!(
            "计算结果: token_0_amount={}, token_1_amount={}, lp_token_amount={}",
            token_0_amount, token_1_amount, lp_token_amount
        );

        // 计算含滑点的数量
        let amount_0_with_slippage = amount_with_slippage(token_0_amount, slippage, true);
        let amount_1_with_slippage = amount_with_slippage(token_1_amount, slippage, true);

        // 计算转账费
        let transfer_fee = get_pool_mints_inverse_fee(
            rpc_client,
            pool_state.token_0_mint,
            pool_state.token_1_mint,
            amount_0_with_slippage,
            amount_1_with_slippage,
        )?;

        let amount_0_max = amount_0_with_slippage.checked_add(transfer_fee.0.transfer_fee).unwrap();
        let amount_1_max = amount_1_with_slippage.checked_add(transfer_fee.1.transfer_fee).unwrap();

        info!("最终计算: amount_0_max={}, amount_1_max={}", amount_0_max, amount_1_max);

        // 构建交易指令
        let mut instructions = Vec::new();
        let user_lp_token =
            spl_associated_token_account::get_associated_token_address(&user_wallet, &pool_state.lp_mint);

        // 创建用户LP代币ATA账户
        let create_user_lp_token_instrs =
            create_ata_token_account_instr(spl_token::id(), &pool_state.lp_mint, &user_wallet)?;
        instructions.extend(create_user_lp_token_instrs);

        // 创建存款指令
        let deposit_instrs = deposit_instr(
            cpmm_program_id,
            user_wallet,
            pool_id,
            pool_state.token_0_mint,
            pool_state.token_1_mint,
            pool_state.lp_mint,
            pool_state.token_0_vault,
            pool_state.token_1_vault,
            user_token_0,
            user_token_1,
            user_lp_token,
            lp_token_amount,
            amount_0_max,
            amount_1_max,
        )?;
        instructions.extend(deposit_instrs);

        // CLI逻辑：构建并发送交易（完全按照CLI逻辑）
        let signers = vec![&user_keypair];
        let recent_hash = rpc_client.get_latest_blockhash()?;
        let transaction = Transaction::new_signed_with_payer(&instructions, Some(&user_wallet), &signers, recent_hash);

        // 发送交易
        let signature = rpc_client.send_and_confirm_transaction(&transaction)?;

        info!("✅ 创建CPMM存款成功，交易签名: {}", signature);

        // 构建响应
        let explorer_url = format!("https://explorer.solana.com/tx/{}", signature);
        let now = chrono::Utc::now().timestamp();

        Ok(CpmmDepositAndSendResponse {
            signature: signature.to_string(),
            pool_id: request.pool_id,
            token_0_mint: pool_state.token_0_mint.to_string(),
            token_1_mint: pool_state.token_1_mint.to_string(),
            lp_mint: pool_state.lp_mint.to_string(),
            lp_token_amount,
            actual_amount_0: token_0_amount,
            actual_amount_1: token_1_amount,
            amount_0_max,
            amount_1_max,
            transfer_fee_0: transfer_fee.0.transfer_fee,
            transfer_fee_1: transfer_fee.1.transfer_fee,
            status: TransactionStatus::Finalized,
            explorer_url,
            timestamp: now,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{
        amount_with_slippage, create_ata_token_account_instr, deposit_instr, deserialize_anchor_account,
        get_transfer_inverse_fee, unpack_mint, unpack_token, TransferFeeInfo,
    };
    use solana_sdk::account::Account;
    use std::vec;

    #[test]
    fn test_deserialize_anchor_account_with_invalid_data() {
        // 测试无效的账户数据处理
        let invalid_account = Account {
            lamports: 1000,
            data: vec![1, 2, 3], // 无效的数据
            owner: solana_sdk::pubkey::Pubkey::new_unique(),
            executable: false,
            rent_epoch: 0,
        };

        // 这应该返回错误而不是panic
        let result = deserialize_anchor_account::<raydium_cp_swap::states::PoolState>(&invalid_account);
        assert!(result.is_err(), "应该返回错误而不是成功解析");
    }

    #[test]
    fn test_unpack_token_with_invalid_data() {
        // 测试无效的Token数据处理
        let invalid_data = vec![1, 2, 3]; // 无效的token数据

        // 这应该返回错误而不是panic
        let result = unpack_token(&invalid_data);
        assert!(result.is_err(), "应该返回错误而不是成功解析");
    }

    #[test]
    fn test_unpack_mint_with_invalid_data() {
        // 测试无效的Mint数据处理
        let invalid_data = vec![1, 2, 3]; // 无效的mint数据

        // 这应该返回错误而不是panic
        let result = unpack_mint(&invalid_data);
        assert!(result.is_err(), "应该返回错误而不是成功解析");
    }

    #[test]
    fn test_get_transfer_inverse_fee_with_no_extension() {
        // 测试没有transfer fee extension的mint
        let minimal_mint_data = vec![0u8; 82]; // PodMint的最小大小

        if let Ok(mint_info) = unpack_mint(&minimal_mint_data) {
            let fee = get_transfer_inverse_fee(&mint_info, 100, 1000000);
            assert_eq!(fee, 0, "没有extension的mint反向转账费应该返回0");
        }
    }

    #[test]
    fn test_amount_with_slippage() {
        // 测试滑点计算函数
        let amount = 1000000u64;
        let slippage = 0.005; // 0.5%

        // 向上舍入（用于最大输入金额）
        let max_amount = amount_with_slippage(amount, slippage, true);
        assert!(max_amount > amount, "向上舍入应该增加金额");
        assert_eq!(max_amount, 1005000); // 1000000 * 1.005 = 1005000

        // 向下舍入（用于最小输出金额）
        let min_amount = amount_with_slippage(amount, slippage, false);
        assert!(min_amount < amount, "向下舍入应该减少金额");
        assert_eq!(min_amount, 995000); // 1000000 * 0.995 = 995000
    }

    #[test]
    fn test_amount_with_slippage_edge_cases() {
        // 测试滑点计算的边界情况
        let amount = 100u64;

        // 零滑点
        let zero_slippage_up = amount_with_slippage(amount, 0.0, true);
        let zero_slippage_down = amount_with_slippage(amount, 0.0, false);
        assert_eq!(zero_slippage_up, amount, "零滑点向上舍入应该保持原值");
        assert_eq!(zero_slippage_down, amount, "零滑点向下舍入应该保持原值");

        // 大滑点
        let large_slippage = 0.1; // 10%
        let large_slippage_up = amount_with_slippage(amount, large_slippage, true);
        let large_slippage_down = amount_with_slippage(amount, large_slippage, false);
        assert_eq!(large_slippage_up, 111, "10%向上滑点应该是111");
        assert_eq!(large_slippage_down, 90, "10%向下滑点应该是90");
    }

    #[test]
    fn test_create_ata_token_account_instr() {
        // 测试创建关联代币账户指令
        let token_program_id = spl_token::id();
        let mint = solana_sdk::pubkey::Pubkey::new_unique();
        let owner = solana_sdk::pubkey::Pubkey::new_unique();

        let result = create_ata_token_account_instr(token_program_id, &mint, &owner);
        assert!(result.is_ok(), "应该成功创建ATA指令");

        let instructions = result.unwrap();
        assert_eq!(instructions.len(), 1, "应该返回一个指令");

        let instruction = &instructions[0];
        assert_eq!(
            instruction.program_id,
            spl_associated_token_account::id(),
            "指令程序ID应该是关联代币账户程序"
        );
    }

    #[test]
    fn test_deposit_instr() {
        // 测试创建Deposit指令
        let cpmm_program_id = solana_sdk::pubkey::Pubkey::new_unique();
        let payer = solana_sdk::pubkey::Pubkey::new_unique();
        let pool_id = solana_sdk::pubkey::Pubkey::new_unique();
        let token_0_mint = solana_sdk::pubkey::Pubkey::new_unique();
        let token_1_mint = solana_sdk::pubkey::Pubkey::new_unique();
        let token_lp_mint = solana_sdk::pubkey::Pubkey::new_unique();
        let token_0_vault = solana_sdk::pubkey::Pubkey::new_unique();
        let token_1_vault = solana_sdk::pubkey::Pubkey::new_unique();
        let user_token_0_account = solana_sdk::pubkey::Pubkey::new_unique();
        let user_token_1_account = solana_sdk::pubkey::Pubkey::new_unique();
        let user_token_lp_account = solana_sdk::pubkey::Pubkey::new_unique();
        let lp_token_amount = 1000000u64;
        let maximum_token_0_amount = 1050000u64;
        let maximum_token_1_amount = 2100000u64;

        let result = deposit_instr(
            cpmm_program_id,
            payer,
            pool_id,
            token_0_mint,
            token_1_mint,
            token_lp_mint,
            token_0_vault,
            token_1_vault,
            user_token_0_account,
            user_token_1_account,
            user_token_lp_account,
            lp_token_amount,
            maximum_token_0_amount,
            maximum_token_1_amount,
        );

        assert!(result.is_ok(), "应该成功创建Deposit指令");

        let instructions = result.unwrap();
        assert_eq!(instructions.len(), 1, "应该返回一个指令");

        let instruction = &instructions[0];
        assert_eq!(instruction.program_id, cpmm_program_id, "指令程序ID应该匹配");
        assert_eq!(instruction.accounts.len(), 13, "应该有13个账户");

        // 检查discriminator
        assert_eq!(
            instruction.data[0..8],
            [0xf2, 0x23, 0xc6, 0x8b, 0x25, 0x22, 0xb5, 0x12],
            "discriminator应该匹配deposit指令"
        );

        // 检查参数
        let lp_token_amount_bytes = &instruction.data[8..16];
        let maximum_token_0_amount_bytes = &instruction.data[16..24];
        let maximum_token_1_amount_bytes = &instruction.data[24..32];

        assert_eq!(
            u64::from_le_bytes(lp_token_amount_bytes.try_into().unwrap()),
            lp_token_amount
        );
        assert_eq!(
            u64::from_le_bytes(maximum_token_0_amount_bytes.try_into().unwrap()),
            maximum_token_0_amount
        );
        assert_eq!(
            u64::from_le_bytes(maximum_token_1_amount_bytes.try_into().unwrap()),
            maximum_token_1_amount
        );
    }

    #[test]
    fn test_transfer_fee_info() {
        // 测试TransferFeeInfo结构体
        let mint = solana_sdk::pubkey::Pubkey::new_unique();
        let owner = solana_sdk::pubkey::Pubkey::new_unique();
        let transfer_fee = 12345u64;

        let fee_info = TransferFeeInfo {
            mint,
            owner,
            transfer_fee,
        };

        assert_eq!(fee_info.mint, mint);
        assert_eq!(fee_info.owner, owner);
        assert_eq!(fee_info.transfer_fee, transfer_fee);
    }
}
