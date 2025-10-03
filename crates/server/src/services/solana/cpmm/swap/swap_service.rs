use crate::dtos::solana::common::TransactionStatus;
use crate::dtos::solana::cpmm::swap::{
    AmmConfigInfo, CpmmSwapBaseInCompute, CpmmSwapBaseInRequest, CpmmSwapBaseInResponse,
    CpmmSwapBaseInTransactionRequest, CpmmSwapBaseOutCompute, CpmmSwapBaseOutRequest, CpmmSwapBaseOutResponse,
    CpmmSwapBaseOutTransactionRequest, CpmmTransactionData, PoolStateInfo,
};
use crate::services::solana::clmm::referral_service::ReferralAccount;
use crate::services::solana::shared::{SharedContext, SolanaUtils};
use anyhow::Result;
use raydium_cp_swap::curve::{CurveCalculator, TradeDirection};
use raydium_cp_swap::instruction;
use raydium_cp_swap::states::{AmmConfig, PoolState};
use solana_sdk::compute_budget::ComputeBudgetInstruction;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use spl_associated_token_account;
use std::str::FromStr;
use std::sync::Arc;
use tracing::info;
use utils::{ConfigManager, PoolInfoManager, TokenUtils};

// 导入必要的Solana和SPL库
use anchor_lang::{AccountDeserialize, Discriminator};
use anchor_spl::token_2022::spl_token_2022::{
    extension::{transfer_fee::TransferFeeConfig, BaseStateWithExtensions, PodStateWithExtensions},
    pod::{PodAccount, PodMint},
};
use solana_sdk::instruction::{AccountMeta, Instruction};

// 本地工具函数（与CLI完全匹配）
fn deserialize_anchor_account<T: AccountDeserialize>(account: &solana_sdk::account::Account) -> Result<T> {
    let mut data: &[u8] = &account.data;
    T::try_deserialize(&mut data).map_err(Into::into)
}

fn unpack_token(token_data: &[u8]) -> Result<PodStateWithExtensions<'_, PodAccount>> {
    let token = PodStateWithExtensions::<PodAccount>::unpack(&token_data)?;
    Ok(token)
}

fn unpack_mint(token_data: &[u8]) -> Result<PodStateWithExtensions<'_, PodMint>> {
    let mint = PodStateWithExtensions::<PodMint>::unpack(&token_data)?;
    Ok(mint)
}

/// 获取转账费用（Token2022支持）- 100%匹配CLI实现
pub fn get_transfer_fee(mint: &PodStateWithExtensions<'_, PodMint>, epoch: u64, pre_fee_amount: u64) -> u64 {
    if let Ok(transfer_fee_config) = mint.get_extension::<TransferFeeConfig>() {
        transfer_fee_config
            .calculate_epoch_fee(epoch, pre_fee_amount)
            .unwrap_or(0)
    } else {
        0
    }
}

/// 获取反向转账费用（用于SwapBaseOut）- 100%匹配CLI实现
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

/// 计算考虑滑点的金额 - 100%匹配CLI实现
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
    // 使用idempotent版本，与CLI完全一致
    let instruction = spl_associated_token_account::instruction::create_associated_token_account_idempotent(
        owner,             // 付费者
        owner,             // 账户所有者
        mint,              // mint地址
        &token_program_id, // token程序
    );

    Ok(vec![instruction])
}

/// 创建SwapBaseInput指令 - 100%匹配CLI实现
///
/// 使用与CLI完全相同的账户结构和权限设置
pub fn swap_base_input_instr(
    cpmm_program_id: Pubkey,
    payer: Pubkey,
    pool_id: Pubkey,
    amm_config: Pubkey,
    observation_key: Pubkey,
    input_token_account: Pubkey,
    output_token_account: Pubkey,
    input_vault: Pubkey,
    output_vault: Pubkey,
    input_token_program: Pubkey,
    output_token_program: Pubkey,
    input_token_mint: Pubkey,
    output_token_mint: Pubkey,
    amount_in: u64,
    minimum_amount_out: u64,
    // 推荐系统相关参数
    reward_mint: &Pubkey,
    payer_referral: Option<&Pubkey>,
    upper: Option<&Pubkey>,
    upper_token_account: Option<&Pubkey>,
    upper_referral: Option<&Pubkey>,
    upper_upper: Option<&Pubkey>,
    upper_upper_token_account: Option<&Pubkey>,
    project_token_account: &Pubkey,
    referral_program_id: &Pubkey,
) -> Result<Vec<Instruction>> {
    // 计算authority地址，与CLI完全一致
    const AUTH_SEED: &str = "vault_and_lp_mint_auth_seed";
    let (authority, _bump) = Pubkey::find_program_address(&[AUTH_SEED.as_bytes()], &cpmm_program_id);

    // 构造指令数据（使用swap_base_input的discriminator）
    let mut instruction_data = Vec::new();
    // swap_base_input方法的discriminator：sha256("global:swap_base_input")[0..8]
    let discriminator = instruction::SwapBaseInput::DISCRIMINATOR;
    instruction_data.extend_from_slice(&discriminator);
    instruction_data.extend_from_slice(&amount_in.to_le_bytes());
    instruction_data.extend_from_slice(&minimum_amount_out.to_le_bytes());

    // 构建账户元数据，顺序与CLI完全一致
    let mut accounts = vec![
        AccountMeta::new(payer, true),                          // payer (signer)
        AccountMeta::new_readonly(authority, false),            // authority
        AccountMeta::new_readonly(amm_config, false),           // amm_config
        AccountMeta::new(pool_id, false),                       // pool_state
        AccountMeta::new(input_token_account, false),           // input_token_account
        AccountMeta::new(output_token_account, false),          // output_token_account
        AccountMeta::new(input_vault, false),                   // input_vault
        AccountMeta::new(output_vault, false),                  // output_vault
        AccountMeta::new_readonly(input_token_program, false),  // input_token_program
        AccountMeta::new_readonly(output_token_program, false), // output_token_program
        AccountMeta::new_readonly(input_token_mint, false),     // input_token_mint
        AccountMeta::new_readonly(output_token_mint, false),    // output_token_mint
        AccountMeta::new(observation_key, false),               // observation_state
    ];

    // 添加必传的reward_mint账户
    accounts.push(AccountMeta::new_readonly(*reward_mint, false)); // reward_mint

    // 添加可选的payer_referral账户
    if let Some(payer_referral_pubkey) = payer_referral {
        accounts.push(AccountMeta::new_readonly(*payer_referral_pubkey, false));
    } else {
        accounts.push(AccountMeta::new_readonly(cpmm_program_id, false)); // 占位符
    }

    // 添加可选的upper账户
    if let Some(upper_pubkey) = upper {
        accounts.push(AccountMeta::new_readonly(*upper_pubkey, false));
    } else {
        accounts.push(AccountMeta::new_readonly(cpmm_program_id, false)); // 占位符
    }

    // 添加可选的upper_token_account
    if let Some(upper_token_pubkey) = upper_token_account {
        accounts.push(AccountMeta::new(*upper_token_pubkey, false));
    } else {
        accounts.push(AccountMeta::new_readonly(cpmm_program_id, false)); // 占位符
    }

    // 添加可选的upper_referral账户
    if let Some(upper_referral_pubkey) = upper_referral {
        accounts.push(AccountMeta::new_readonly(*upper_referral_pubkey, false));
    } else {
        accounts.push(AccountMeta::new_readonly(cpmm_program_id, false)); // 占位符
    }

    // 添加可选的upper_upper账户
    if let Some(upper_upper_pubkey) = upper_upper {
        accounts.push(AccountMeta::new_readonly(*upper_upper_pubkey, false));
    } else {
        accounts.push(AccountMeta::new_readonly(cpmm_program_id, false)); // 占位符
    }

    // 添加可选的upper_upper_token_account
    if let Some(upper_upper_token_pubkey) = upper_upper_token_account {
        accounts.push(AccountMeta::new(*upper_upper_token_pubkey, false));
    } else {
        accounts.push(AccountMeta::new_readonly(cpmm_program_id, false)); // 占位符
    }

    // 添加必需的项目方代币账户
    accounts.push(AccountMeta::new(*project_token_account, false)); // project_token_account

    // 添加必传的system_program账户
    accounts.push(AccountMeta::new_readonly(solana_sdk::system_program::id(), false)); // system_program

    // 添加必传的associated_token_program账户
    accounts.push(AccountMeta::new_readonly(spl_associated_token_account::id(), false)); // associated_token_program

    // 添加必传的referral账户
    accounts.push(AccountMeta::new_readonly(*referral_program_id, false)); // referral

    // 调试：打印所有可写账户
    info!("🔍 检查所有账户:");
    for account in accounts.iter() {
        if account.is_writable {
            info!("  writable account: {}", account.pubkey);
        } else {
            info!("  readonly account: {}", account.pubkey);
        }
    }

    let instruction = Instruction {
        program_id: cpmm_program_id,
        accounts,
        data: instruction_data,
    };

    Ok(vec![instruction])
}

/// 创建SwapBaseOutput指令 - 100%匹配CLI实现
///
/// 用于固定输出金额的交换，指定期望输出金额和最大输入金额
pub fn swap_base_output_instr(
    cpmm_program_id: Pubkey,
    payer: Pubkey,
    pool_id: Pubkey,
    amm_config: Pubkey,
    observation_key: Pubkey,
    input_token_account: Pubkey,
    output_token_account: Pubkey,
    input_vault: Pubkey,
    output_vault: Pubkey,
    input_token_program: Pubkey,
    output_token_program: Pubkey,
    input_token_mint: Pubkey,
    output_token_mint: Pubkey,
    max_amount_in: u64,
    amount_out: u64,
    // 推荐系统相关参数
    reward_mint: &Pubkey,
    payer_referral: Option<&Pubkey>,
    upper: Option<&Pubkey>,
    upper_token_account: Option<&Pubkey>,
    upper_referral: Option<&Pubkey>,
    upper_upper: Option<&Pubkey>,
    upper_upper_token_account: Option<&Pubkey>,
    project_token_account: &Pubkey,
    referral_program_id: &Pubkey,
) -> Result<Vec<Instruction>> {
    // 计算authority地址，与CLI完全一致
    const AUTH_SEED: &str = "vault_and_lp_mint_auth_seed";
    let (authority, _bump) = Pubkey::find_program_address(&[AUTH_SEED.as_bytes()], &cpmm_program_id);

    // 构造指令数据（使用swap_base_output的discriminator）
    let mut instruction_data = Vec::new();
    // swap_base_output方法的discriminator：sha256("global:swap_base_output")[0..8]
    let discriminator = instruction::SwapBaseOutput::DISCRIMINATOR;
    instruction_data.extend_from_slice(&discriminator);
    instruction_data.extend_from_slice(&max_amount_in.to_le_bytes());
    instruction_data.extend_from_slice(&amount_out.to_le_bytes());

    // 构建账户元数据，顺序与CLI完全一致
    let mut accounts = vec![
        AccountMeta::new(payer, true),                          // payer (signer)
        AccountMeta::new_readonly(authority, false),            // authority
        AccountMeta::new_readonly(amm_config, false),           // amm_config
        AccountMeta::new(pool_id, false),                       // pool_state
        AccountMeta::new(input_token_account, false),           // input_token_account
        AccountMeta::new(output_token_account, false),          // output_token_account
        AccountMeta::new(input_vault, false),                   // input_vault
        AccountMeta::new(output_vault, false),                  // output_vault
        AccountMeta::new_readonly(input_token_program, false),  // input_token_program
        AccountMeta::new_readonly(output_token_program, false), // output_token_program
        AccountMeta::new_readonly(input_token_mint, false),     // input_token_mint
        AccountMeta::new_readonly(output_token_mint, false),    // output_token_mint
        AccountMeta::new(observation_key, false),               // observation_state
    ];

    // 添加必传的reward_mint账户
    accounts.push(AccountMeta::new_readonly(*reward_mint, false)); // reward_mint

    // 添加可选的payer_referral账户
    if let Some(payer_referral_pubkey) = payer_referral {
        accounts.push(AccountMeta::new_readonly(*payer_referral_pubkey, false));
    } else {
        accounts.push(AccountMeta::new_readonly(cpmm_program_id, false)); // 占位符
    }

    // 添加可选的upper账户
    if let Some(upper_pubkey) = upper {
        accounts.push(AccountMeta::new_readonly(*upper_pubkey, false));
    } else {
        accounts.push(AccountMeta::new_readonly(cpmm_program_id, false)); // 占位符
    }

    // 添加可选的upper_token_account
    if let Some(upper_token_pubkey) = upper_token_account {
        accounts.push(AccountMeta::new(*upper_token_pubkey, false));
    } else {
        accounts.push(AccountMeta::new_readonly(cpmm_program_id, false)); // 占位符
    }

    // 添加可选的upper_referral账户
    if let Some(upper_referral_pubkey) = upper_referral {
        accounts.push(AccountMeta::new_readonly(*upper_referral_pubkey, false));
    } else {
        accounts.push(AccountMeta::new_readonly(cpmm_program_id, false)); // 占位符
    }

    // 添加可选的upper_upper账户
    if let Some(upper_upper_pubkey) = upper_upper {
        accounts.push(AccountMeta::new_readonly(*upper_upper_pubkey, false));
    } else {
        accounts.push(AccountMeta::new_readonly(cpmm_program_id, false)); // 占位符
    }

    // 添加可选的upper_upper_token_account
    if let Some(upper_upper_token_pubkey) = upper_upper_token_account {
        accounts.push(AccountMeta::new(*upper_upper_token_pubkey, false));
    } else {
        accounts.push(AccountMeta::new_readonly(cpmm_program_id, false)); // 占位符
    }

    // 添加必需的项目方代币账户
    accounts.push(AccountMeta::new(*project_token_account, false)); // project_token_account

    // 添加必传的system_program账户
    accounts.push(AccountMeta::new_readonly(solana_sdk::system_program::id(), false)); // system_program

    // 添加必传的associated_token_program账户
    accounts.push(AccountMeta::new_readonly(spl_associated_token_account::id(), false)); // associated_token_program

    // 添加必传的referral账户
    accounts.push(AccountMeta::new_readonly(*referral_program_id, false)); // referral

    // 调试：打印所有可写账户
    info!("🔍 检查所有账户:");
    for account in accounts.iter() {
        if account.is_writable {
            info!("  writable account: {}", account.pubkey);
        } else {
            info!("  readonly account: {}", account.pubkey);
        }
    }

    let instruction = Instruction {
        program_id: cpmm_program_id,
        accounts,
        data: instruction_data,
    };

    Ok(vec![instruction])
}

/// CPMM交换服务
///
/// 提供基于Raydium恒定乘积做市商(CPMM)的代币交换功能
pub struct CpmmSwapService {
    shared: Arc<SharedContext>,
}

impl CpmmSwapService {
    /// 创建新的CPMM交换服务实例
    pub fn new(shared: Arc<SharedContext>) -> Self {
        Self { shared }
    }

    /// 执行CPMM SwapBaseIn交换
    ///
    /// 100%忠实地实现CLI的业务逻辑，包括：
    /// 1. 加载池子状态和多个账户信息
    /// 2. 确定交易方向和相关代币信息
    /// 3. 计算转账费和实际输入金额
    /// 4. 使用CurveCalculator进行交换计算
    /// 5. 应用滑点保护
    /// 6. 创建输出代币ATA账户
    /// 7. 构建并发送交换交易
    pub async fn build_and_send_swap_base_in(&self, request: CpmmSwapBaseInRequest) -> Result<CpmmSwapBaseInResponse> {
        info!(
            "执行CPMM SwapBaseIn: pool_id={}, user_input_token={}, amount={}",
            request.pool_id, request.user_input_token, request.user_input_amount
        );

        let pool_id = Pubkey::from_str(&request.pool_id)?;
        let user_input_token_raw = Pubkey::from_str(&request.user_input_token)?;
        let user_input_amount = request.user_input_amount;
        let slippage = request.slippage.unwrap_or(0.5) / 100.0; // 转换为小数

        info!("📝 SwapBaseIn接口输入参数分析:");
        info!("  pool_id: {}", pool_id);
        info!("  user_input_token_raw: {}", user_input_token_raw);
        info!("  user_input_amount: {}", user_input_amount);
        info!("  slippage: {}%", slippage * 100.0);

        // 1. 加载池子状态，添加详细验证
        let rpc_client = &self.shared.rpc_client;

        // 检查池子账户是否存在和有效
        let pool_account = match rpc_client.get_account(&pool_id) {
            Ok(account) => account,
            Err(e) => {
                info!("池子账户不存在或获取失败: pool_id={}, error={}", pool_id, e);
                return Err(anyhow::anyhow!("池子账户不存在或无法访问: {}, 错误: {}", pool_id, e));
            }
        };

        // 获取配置的CPMM程序ID
        let cpmm_program_id = ConfigManager::get_cpmm_program_id()?;

        // 验证账户所有者
        if pool_account.owner != cpmm_program_id {
            return Err(anyhow::anyhow!(
                "无效的池子地址，账户所有者不是CPMM程序: expected={}, actual={}",
                cpmm_program_id,
                pool_account.owner
            ));
        }

        // 添加详细的调试信息
        info!(
            "池子账户调试信息: pool_id={}, data_length={}, owner={}",
            pool_id,
            pool_account.data.len(),
            pool_account.owner
        );

        if pool_account.data.len() >= 8 {
            let discriminator = &pool_account.data[0..8];
            info!("账户discriminator: {:?}", discriminator);
        }

        // 反序列化池子状态
        let pool_state: PoolState = match deserialize_anchor_account::<PoolState>(&pool_account) {
            Ok(state) => {
                info!("✅ 池子状态反序列化成功");
                info!("🏊‍♀️ Pool详细信息:");
                info!("  amm_config: {}", state.amm_config);
                info!("  token_0_mint: {}", state.token_0_mint);
                info!("  token_1_mint: {}", state.token_1_mint);
                info!("  token_0_vault: {}", state.token_0_vault);
                info!("  token_1_vault: {}", state.token_1_vault);
                info!("  token_0_program: {}", state.token_0_program);
                info!("  token_1_program: {}", state.token_1_program);
                info!("  observation_key: {}", state.observation_key);
                info!("  auth_bump: {}", state.auth_bump);
                info!("  status: {}", state.status);
                info!("  lp_mint: {}", state.lp_mint);
                // 复制packed字段到本地变量以避免不对齐的引用
                let lp_supply = state.lp_supply;
                let protocol_fees_token_0 = state.protocol_fees_token_0;
                let protocol_fees_token_1 = state.protocol_fees_token_1;
                let fund_fees_token_0 = state.fund_fees_token_0;
                let fund_fees_token_1 = state.fund_fees_token_1;
                let open_time = state.open_time;
                info!("  lp_supply: {}", lp_supply);
                info!("  protocol_fees_token_0: {}", protocol_fees_token_0);
                info!("  protocol_fees_token_1: {}", protocol_fees_token_1);
                info!("  fund_fees_token_0: {}", fund_fees_token_0);
                info!("  fund_fees_token_1: {}", fund_fees_token_1);
                info!("  open_time: {}", open_time);
                state
            }
            Err(e) => {
                info!("❌ SwapBaseIn池子状态反序列化失败: pool_id={}, error={}", pool_id, e);

                // 输出详细的十六进制数据用于调试
                let data_len = pool_account.data.len();
                info!("📊 账户数据长度: {} bytes", data_len);
                if data_len >= 8 {
                    let discriminator_hex = hex::encode(&pool_account.data[0..8]);
                    info!("🔍 实际discriminator (hex): {}", discriminator_hex);
                    info!("🔍 实际discriminator (bytes): {:?}", &pool_account.data[0..8]);
                }
                if data_len >= 32 {
                    let first_32_hex = hex::encode(&pool_account.data[0..32]);
                    info!("📄 账户数据前32字节 (hex): {}", first_32_hex);
                }
                info!(
                    "📄 账户数据前32字节 (bytes): {:?}",
                    &pool_account.data[0..std::cmp::min(32, data_len)]
                );

                return Err(anyhow::anyhow!("无法反序列化池子状态，可能discriminator不匹配: {}", e));
            }
        };

        // 🔍 智能检测并确定用户代币账户地址（与compute函数相同的逻辑）
        let user_input_token = {
            info!("🧠 SwapBaseIn开始智能检测用户代币账户...");

            // 检查用户输入的地址是否是池子中的代币mint之一
            let is_token_0_mint = user_input_token_raw == pool_state.token_0_mint;
            let is_token_1_mint = user_input_token_raw == pool_state.token_1_mint;

            if is_token_0_mint || is_token_1_mint {
                // 用户输入的是mint地址，我们需要计算对应的ATA地址
                let wallet_keypair = Keypair::from_base58_string(
                    self.shared
                        .app_config
                        .private_key
                        .as_ref()
                        .ok_or_else(|| anyhow::anyhow!("私钥未配置"))?,
                );
                let wallet_pubkey = wallet_keypair.pubkey();

                let ata_address =
                    spl_associated_token_account::get_associated_token_address(&wallet_pubkey, &user_input_token_raw);

                info!("✅ SwapBaseIn检测到mint地址，已转换为ATA:");
                info!("  mint地址: {}", user_input_token_raw);
                info!("  钱包地址: {}", wallet_pubkey);
                info!("  ATA地址: {}", ata_address);
                info!("  是token_0_mint: {}", is_token_0_mint);
                info!("  是token_1_mint: {}", is_token_1_mint);

                ata_address
            } else {
                // 用户输入的可能已经是代币账户地址，直接使用
                info!(
                    "🔍 SwapBaseIn输入地址不是池子的mint，假设是代币账户地址: {}",
                    user_input_token_raw
                );
                user_input_token_raw
            }
        };

        // 2. 批量加载所有相关账户（与CLI完全相同的逻辑）
        let load_pubkeys = vec![
            pool_id,
            pool_state.amm_config,
            pool_state.token_0_vault,
            pool_state.token_1_vault,
            pool_state.token_0_mint,
            pool_state.token_1_mint,
            user_input_token,
        ];

        let accounts = rpc_client.get_multiple_accounts(&load_pubkeys)?;
        let epoch = rpc_client.get_epoch_info()?.epoch;

        // 3. 解码所有账户数据
        let pool_account = accounts[0].as_ref().unwrap();
        let amm_config_account = accounts[1].as_ref().unwrap();
        let token_0_vault_account = accounts[2].as_ref().unwrap();
        let token_1_vault_account = accounts[3].as_ref().unwrap();
        let token_0_mint_account = accounts[4].as_ref().unwrap();
        let token_1_mint_account = accounts[5].as_ref().unwrap();
        let user_input_token_account = accounts[6].as_ref().unwrap();

        let pool_state: PoolState = deserialize_anchor_account::<PoolState>(pool_account)?;
        let amm_config_state: AmmConfig = deserialize_anchor_account::<AmmConfig>(amm_config_account)?;
        let token_0_vault_info = unpack_token(&token_0_vault_account.data)?;
        let token_1_vault_info = unpack_token(&token_1_vault_account.data)?;
        let token_0_mint_info = unpack_mint(&token_0_mint_account.data)?;
        let token_1_mint_info = unpack_mint(&token_1_mint_account.data)?;
        let user_input_token_info = unpack_token(&user_input_token_account.data)?;

        // 4. 计算池子中的代币总量（扣除费用后）
        let (total_token_0_amount, total_token_1_amount) = pool_state
            .vault_amount_without_fee(
                token_0_vault_info.base.amount.into(),
                token_1_vault_info.base.amount.into(),
            )
            .unwrap();

        // 4.1. 获取私钥和钱包信息
        let private_key = self
            .shared
            .app_config
            .private_key
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("私钥未配置"))?;
        let payer = Keypair::from_base58_string(private_key);
        let payer_pubkey = payer.pubkey();

        // 5. 确定交易方向和相关信息（100%匹配CLI逻辑）
        let (
            trade_direction,
            total_input_token_amount,
            total_output_token_amount,
            user_output_token,
            input_vault,
            output_vault,
            input_token_mint,
            output_token_mint,
            input_token_program,
            output_token_program,
            transfer_fee,
        ) = if user_input_token_info.base.mint == token_0_vault_info.base.mint {
            (
                TradeDirection::ZeroForOne,
                total_token_0_amount,
                total_token_1_amount,
                spl_associated_token_account::get_associated_token_address(&payer_pubkey, &pool_state.token_1_mint),
                pool_state.token_0_vault,
                pool_state.token_1_vault,
                pool_state.token_0_mint,
                pool_state.token_1_mint,
                pool_state.token_0_program,
                pool_state.token_1_program,
                get_transfer_fee(&token_0_mint_info, epoch, user_input_amount),
            )
        } else {
            (
                TradeDirection::OneForZero,
                total_token_1_amount,
                total_token_0_amount,
                spl_associated_token_account::get_associated_token_address(&payer_pubkey, &pool_state.token_0_mint),
                pool_state.token_1_vault,
                pool_state.token_0_vault,
                pool_state.token_1_mint,
                pool_state.token_0_mint,
                pool_state.token_1_program,
                pool_state.token_0_program,
                get_transfer_fee(&token_1_mint_info, epoch, user_input_amount),
            )
        };

        // 6. 计算实际输入金额（扣除转账费）
        let actual_amount_in = user_input_amount.saturating_sub(transfer_fee);

        // 7. 使用CurveCalculator计算交换结果（与CLI完全相同）
        // 🔧 关键修复：需要根据池子的enable_creator_fee标志调整creator_fee_rate
        let creator_fee_rate = if pool_state.enable_creator_fee {
            amm_config_state.creator_fee_rate
        } else {
            0
        };

        let curve_result = CurveCalculator::swap_base_input(
            trade_direction,
            u128::from(actual_amount_in),
            u128::from(total_input_token_amount),
            u128::from(total_output_token_amount),
            amm_config_state.trade_fee_rate,
            creator_fee_rate,
            amm_config_state.protocol_fee_rate,
            amm_config_state.fund_fee_rate,
            pool_state.is_creator_fee_on_input(trade_direction).unwrap(),
        )
        .ok_or_else(|| anyhow::anyhow!("交换计算失败：零交易代币"))?;

        let amount_out = u64::try_from(curve_result.output_amount)?;

        // 8. 计算输出代币的转账费
        let output_transfer_fee = match trade_direction {
            TradeDirection::ZeroForOne => get_transfer_fee(&token_1_mint_info, epoch, amount_out),
            TradeDirection::OneForZero => get_transfer_fee(&token_0_mint_info, epoch, amount_out),
        };

        let amount_received = amount_out.checked_sub(output_transfer_fee).unwrap();

        // 9. 应用滑点保护计算最小输出金额
        let minimum_amount_out = amount_with_slippage(amount_received, slippage, false);

        info!("💰 SwapBaseIn计算结果:");
        info!("  user_input_amount: {}", user_input_amount);
        info!("  transfer_fee: {}", transfer_fee);
        info!("  actual_amount_in: {}", actual_amount_in);
        info!("  total_input_token_amount: {}", total_input_token_amount);
        info!("  total_output_token_amount: {}", total_output_token_amount);
        info!("  curve_result.output_amount: {}", curve_result.output_amount);
        info!("  amount_out: {}", amount_out);
        info!("  output_transfer_fee: {}", output_transfer_fee);
        info!("  amount_received (预计算): {}", amount_received);
        info!("  minimum_amount_out (传给合约): {}", minimum_amount_out);
        info!("  slippage: {}%", slippage * 100.0);

        // 10. 构建交易指令
        let mut instructions = Vec::new();
        instructions.push(ComputeBudgetInstruction::set_compute_unit_limit(800_000));

        // 创建输入代币ATA账户指令（如果不存在）
        info!("📝 确保输入代币ATA账户存在: {}", user_input_token);
        let create_user_input_token_instrs =
            create_ata_token_account_instr(input_token_program, &input_token_mint, &payer_pubkey)?;
        instructions.extend(create_user_input_token_instrs);

        // 创建输出代币ATA账户指令（如果不存在）
        info!("📝 确保输出代币ATA账户存在: {}", user_output_token);
        let create_user_output_token_instrs =
            create_ata_token_account_instr(output_token_program, &output_token_mint, &payer_pubkey)?;
        instructions.extend(create_user_output_token_instrs);

        // SwapV3独有的推荐系统处理
        let mut upper: Option<Pubkey> = None;
        let mut upper_token_account: Option<Pubkey> = None;
        let mut upper_referral: Option<Pubkey> = None;
        let mut upper_upper: Option<Pubkey> = None;
        let mut upper_upper_token_account: Option<Pubkey> = None;
        let mut payer_referral: Option<Pubkey> = None;
        let referral_program_id = ConfigManager::get_referral_program_id()?;

        let payer_key = payer_pubkey;
        let reward_mint_pubkey = input_token_mint;
        info!("reward_mint_pubkey: {}", reward_mint_pubkey);
        let reward_token_program = TokenUtils::detect_mint_program(&self.shared.rpc_client, &reward_mint_pubkey)?;
        info!("reward_token_program: {}", reward_token_program);
        let pool_address_str = PoolInfoManager::calculate_cpmm_pool_address_pda(
            &input_token_mint.to_string(),
            &output_token_mint.to_string().to_string(),
        )?;
        let pool_address = Pubkey::from_str(&pool_address_str)?;
        let pool_account = self.shared.rpc_client.get_account(&pool_address)?;
        let pool_state: raydium_cp_swap::states::PoolState = SolanaUtils::deserialize_anchor_account(&pool_account)?;
        // 项目方奖励账户使用output_token（与reward_mint一致）
        let project_token_account = spl_associated_token_account::get_associated_token_address_with_program_id(
            &pool_state.pool_creator,
            &reward_mint_pubkey,
            &reward_token_program,
        );
        info!("project_token_account: {}", project_token_account);
        let (payer_referral_pda, _) =
            Pubkey::find_program_address(&[b"referral", &payer_key.to_bytes()], &referral_program_id);
        info!("payer_referral: {}", payer_referral_pda);
        let payer_referral_account_data = self.shared.rpc_client.get_account(&payer_referral_pda);
        match payer_referral_account_data {
            Ok(account_data) => {
                let payer_referral_account: ReferralAccount = SolanaUtils::deserialize_anchor_account(&account_data)?;
                payer_referral = Some(payer_referral_pda);
                match payer_referral_account.upper {
                    Some(upper_key) => {
                        upper = Some(upper_key);
                        // upper奖励账户也使用output_token（与reward_mint一致）
                        upper_token_account = Some(
                            spl_associated_token_account::get_associated_token_address_with_program_id(
                                &upper_key,
                                &reward_mint_pubkey,
                                &reward_token_program,
                            ),
                        );
                        let (upper_referral_pda, _) =
                            Pubkey::find_program_address(&[b"referral", &upper_key.to_bytes()], &referral_program_id);
                        upper_referral = Some(upper_referral_pda);
                        let upper_referral_account = self.shared.rpc_client.get_account(&upper_referral_pda)?;
                        let upper_referral_account: ReferralAccount =
                            SolanaUtils::deserialize_anchor_account(&upper_referral_account)?;

                        match upper_referral_account.upper {
                            Some(upper_upper_key) => {
                                upper_upper = Some(upper_upper_key);
                                // upper_upper奖励账户也使用output_token（与reward_mint一致）
                                upper_upper_token_account = Some(
                                    spl_associated_token_account::get_associated_token_address_with_program_id(
                                        &upper_upper_key,
                                        &reward_mint_pubkey,
                                        &reward_token_program,
                                    ),
                                );
                            }
                            None => {}
                        }
                    }
                    None => {}
                }
            }
            Err(_) => {
                info!("payer_referral_account not found, set it to None");
            }
        }

        // 为上级推荐用户创建奖励代币ATA账户（如果存在上级且不存在）
        if let Some(upper_account) = upper_token_account {
            info!("📝 确保上级推荐用户奖励代币ATA账户存在: {}", upper_account);
            let create_upper_ata_ix =
                spl_associated_token_account::instruction::create_associated_token_account_idempotent(
                    &payer_key,
                    &upper.unwrap(),
                    &reward_mint_pubkey,
                    &reward_token_program,
                );
            instructions.push(create_upper_ata_ix);
        }

        // 为上上级推荐用户创建奖励代币ATA账户（如果存在上上级且不存在上上级推荐用户）
        if let Some(upper_upper_account) = upper_upper_token_account {
            info!("📝 确保上上级推荐用户奖励代币ATA账户存在: {}", upper_upper_account);
            let create_upper_upper_ata_ix =
                spl_associated_token_account::instruction::create_associated_token_account_idempotent(
                    &payer_key,
                    &upper_upper.unwrap(),
                    &reward_mint_pubkey,
                    &reward_token_program,
                );
            instructions.push(create_upper_upper_ata_ix);
        }

        // 🔧 关键修复：创建项目方代币账户（如果不存在）
        info!("📝 确保项目方代币ATA账户存在: {}", project_token_account);
        let create_project_ata_ix =
            spl_associated_token_account::instruction::create_associated_token_account_idempotent(
                &payer_key,
                &pool_state.pool_creator,
                &reward_mint_pubkey,
                &reward_token_program,
            );
        instructions.push(create_project_ata_ix);

        // 创建SwapBaseIn指令（使用从CLI逻辑推导出的正确参数）
        info!("🔧 准备构建swap指令，参数:");
        info!("  user_input_amount (传给指令): {}", user_input_amount);
        info!("  minimum_amount_out (传给指令): {}", minimum_amount_out);

        let swap_base_in_instrs = swap_base_input_instr(
            cpmm_program_id,
            payer_pubkey,
            pool_id,
            pool_state.amm_config,
            pool_state.observation_key,
            user_input_token,
            user_output_token,
            input_vault,
            output_vault,
            input_token_program,
            output_token_program,
            input_token_mint,
            output_token_mint,
            user_input_amount,
            minimum_amount_out,
            &reward_mint_pubkey,
            payer_referral.as_ref(),
            upper.as_ref(),
            upper_token_account.as_ref(),
            upper_referral.as_ref(),
            upper_upper.as_ref(),
            upper_upper_token_account.as_ref(),
            &project_token_account,
            &referral_program_id,
        )?;

        // 调试：打印指令数据
        if let Some(instr) = swap_base_in_instrs.first() {
            info!("📋 Swap指令数据详情:");
            info!("  program_id: {}", instr.program_id);
            info!("  accounts数量: {}", instr.accounts.len());
            info!("  data长度: {}", instr.data.len());
            if instr.data.len() >= 24 {
                let discriminator = &instr.data[0..8];
                let amount_in_bytes = &instr.data[8..16];
                let min_out_bytes = &instr.data[16..24];

                info!("  discriminator: {:?}", discriminator);
                info!("  amount_in (bytes): {:?}", amount_in_bytes);
                info!("  minimum_amount_out (bytes): {:?}", min_out_bytes);

                let parsed_amount_in = u64::from_le_bytes(amount_in_bytes.try_into().unwrap());
                let parsed_min_out = u64::from_le_bytes(min_out_bytes.try_into().unwrap());

                info!("  ✅ 解析后amount_in: {}", parsed_amount_in);
                info!("  ✅ 解析后minimum_amount_out: {}", parsed_min_out);
            }
        }

        instructions.extend(swap_base_in_instrs);

        // 11. 构建并发送交易
        let recent_blockhash = rpc_client.get_latest_blockhash()?;
        let transaction =
            Transaction::new_signed_with_payer(&instructions, Some(&payer_pubkey), &[&payer], recent_blockhash);

        let signature = rpc_client.send_and_confirm_transaction(&transaction)?;

        info!("CPMM SwapBaseIn交易成功: {}", signature);

        // 12. 构建响应
        let explorer_url = format!("https://solscan.io/tx/{}", signature);
        let now = chrono::Utc::now().timestamp();

        Ok(CpmmSwapBaseInResponse {
            signature: signature.to_string(),
            pool_id: request.pool_id,
            input_token_mint: input_token_mint.to_string(),
            output_token_mint: output_token_mint.to_string(),
            actual_amount_in,
            amount_out,
            amount_received,
            minimum_amount_out,
            input_transfer_fee: transfer_fee,
            output_transfer_fee,
            status: TransactionStatus::Confirmed,
            explorer_url,
            timestamp: now,
        })
    }

    /// 计算CPMM SwapBaseIn交换结果（不执行实际交换）
    ///
    /// 用于获取报价和预计算结果
    pub async fn compute_cpmm_swap_base_in(&self, request: CpmmSwapBaseInRequest) -> Result<CpmmSwapBaseInCompute> {
        info!(
            "计算CPMM SwapBaseIn: pool_id={}, amount={}",
            request.pool_id, request.user_input_amount
        );

        let pool_id = Pubkey::from_str(&request.pool_id)?;
        let user_input_token_raw = Pubkey::from_str(&request.user_input_token)?;
        let user_input_amount = request.user_input_amount;
        let slippage = request.slippage.unwrap_or(0.5) / 100.0;

        info!("📝 输入参数分析:");
        info!("  pool_id: {}", pool_id);
        info!("  user_input_token_raw: {}", user_input_token_raw);
        info!("  user_input_amount: {}", user_input_amount);
        info!("  slippage: {}%", slippage * 100.0);

        // 执行与swap_base_in相同的计算逻辑，但不发送交易
        let rpc_client = &self.shared.rpc_client;

        // 获取配置的CPMM程序ID
        let cpmm_program_id = ConfigManager::get_cpmm_program_id()?;

        // 加载并验证池子账户
        let pool_account = match rpc_client.get_account(&pool_id) {
            Ok(account) => account,
            Err(e) => {
                info!("计算交换时池子账户不存在: pool_id={}, error={}", pool_id, e);
                return Err(anyhow::anyhow!("池子账户不存在或无法访问: {}", e));
            }
        };

        // 验证账户所有者
        if pool_account.owner != cpmm_program_id {
            return Err(anyhow::anyhow!(
                "无效的池子地址，账户所有者不正确: expected={}, actual={}",
                cpmm_program_id,
                pool_account.owner
            ));
        }

        let pool_state: PoolState = match deserialize_anchor_account::<PoolState>(&pool_account) {
            Ok(state) => {
                info!("✅ Compute函数池子状态反序列化成功");
                info!("🏊‍♀️ Compute Pool详细信息:");
                info!("  amm_config: {}", state.amm_config);
                info!("  token_0_mint: {}", state.token_0_mint);
                info!("  token_1_mint: {}", state.token_1_mint);
                info!("  token_0_vault: {}", state.token_0_vault);
                info!("  token_1_vault: {}", state.token_1_vault);
                info!("  token_0_program: {}", state.token_0_program);
                info!("  token_1_program: {}", state.token_1_program);
                info!("  observation_key: {}", state.observation_key);
                info!("  auth_bump: {}", state.auth_bump);
                info!("  status: {}", state.status);
                info!("  lp_mint: {}", state.lp_mint);
                // 复制packed字段到本地变量以避免不对齐的引用
                let lp_supply = state.lp_supply;
                let protocol_fees_token_0 = state.protocol_fees_token_0;
                let protocol_fees_token_1 = state.protocol_fees_token_1;
                let fund_fees_token_0 = state.fund_fees_token_0;
                let fund_fees_token_1 = state.fund_fees_token_1;
                let open_time = state.open_time;
                info!("  lp_supply: {}", lp_supply);
                info!("  protocol_fees_token_0: {}", protocol_fees_token_0);
                info!("  protocol_fees_token_1: {}", protocol_fees_token_1);
                info!("  fund_fees_token_0: {}", fund_fees_token_0);
                info!("  fund_fees_token_1: {}", fund_fees_token_1);
                info!("  open_time: {}", open_time);
                state
            }
            Err(e) => {
                info!("计算交换时池子状态反序列化失败: pool_id={}, error={}", pool_id, e);
                return Err(anyhow::anyhow!("无法反序列化池子状态: {}", e));
            }
        };

        // 🔍 智能检测并确定用户代币账户地址
        let user_input_token = {
            info!("🧠 开始智能检测用户代币账户...");

            // 检查用户输入的地址是否是池子中的代币mint之一
            let is_token_0_mint = user_input_token_raw == pool_state.token_0_mint;
            let is_token_1_mint = user_input_token_raw == pool_state.token_1_mint;

            if is_token_0_mint || is_token_1_mint {
                // 用户输入的是mint地址，我们需要计算对应的ATA地址
                // 这里假设交换是由配置的钱包执行的
                let wallet_keypair = Keypair::from_base58_string(
                    self.shared
                        .app_config
                        .private_key
                        .as_ref()
                        .ok_or_else(|| anyhow::anyhow!("私钥未配置"))?,
                );
                let wallet_pubkey = wallet_keypair.pubkey();

                let ata_address =
                    spl_associated_token_account::get_associated_token_address(&wallet_pubkey, &user_input_token_raw);

                info!("✅ 检测到mint地址，已转换为ATA:");
                info!("  mint地址: {}", user_input_token_raw);
                info!("  钱包地址: {}", wallet_pubkey);
                info!("  ATA地址: {}", ata_address);
                info!("  是token_0_mint: {}", is_token_0_mint);
                info!("  是token_1_mint: {}", is_token_1_mint);

                ata_address
            } else {
                // 用户输入的可能已经是代币账户地址，直接使用
                info!(
                    "🔍 输入地址不是池子的mint，假设是代币账户地址: {}",
                    user_input_token_raw
                );
                user_input_token_raw
            }
        };

        let load_pubkeys = vec![
            pool_id,
            pool_state.amm_config,
            pool_state.token_0_vault,
            pool_state.token_1_vault,
            pool_state.token_0_mint,
            pool_state.token_1_mint,
            user_input_token,
        ];

        let accounts = rpc_client.get_multiple_accounts(&load_pubkeys)?;
        let epoch = rpc_client.get_epoch_info()?.epoch;

        // 解码账户数据
        let pool_account = accounts[0].as_ref().unwrap();
        let amm_config_account = accounts[1].as_ref().unwrap();
        let token_0_vault_account = accounts[2].as_ref().unwrap();
        let token_1_vault_account = accounts[3].as_ref().unwrap();
        let token_0_mint_account = accounts[4].as_ref().unwrap();
        let token_1_mint_account = accounts[5].as_ref().unwrap();
        let user_input_token_account = accounts[6].as_ref().unwrap();

        info!("🔍 开始逐个账户反序列化...");
        info!("📊 账户数据详情:");
        info!(
            "  Pool账户: data_len={}, owner={}",
            pool_account.data.len(),
            pool_account.owner
        );
        info!(
            "  AmmConfig账户: data_len={}, owner={}",
            amm_config_account.data.len(),
            amm_config_account.owner
        );
        info!(
            "  Token0Vault账户: data_len={}, owner={}",
            token_0_vault_account.data.len(),
            token_0_vault_account.owner
        );
        info!(
            "  Token1Vault账户: data_len={}, owner={}",
            token_1_vault_account.data.len(),
            token_1_vault_account.owner
        );
        info!(
            "  Token0Mint账户: data_len={}, owner={}",
            token_0_mint_account.data.len(),
            token_0_mint_account.owner
        );
        info!(
            "  Token1Mint账户: data_len={}, owner={}",
            token_1_mint_account.data.len(),
            token_1_mint_account.owner
        );
        info!(
            "  UserInputToken账户: data_len={}, owner={}",
            user_input_token_account.data.len(),
            user_input_token_account.owner
        );

        info!("🔍 步骤1: 反序列化PoolState...");
        let pool_state: PoolState =
            deserialize_anchor_account(pool_account).map_err(|e| anyhow::anyhow!("PoolState反序列化失败: {}", e))?;
        info!("✅ PoolState反序列化成功");

        info!("🔍 步骤2: 反序列化AmmConfig...");
        let amm_config_state: AmmConfig = deserialize_anchor_account(amm_config_account)
            .map_err(|e| anyhow::anyhow!("AmmConfig反序列化失败: {}", e))?;
        info!("✅ AmmConfig反序列化成功");

        info!("🔍 步骤3: 解包Token0Vault...");
        let token_0_vault_info =
            unpack_token(&token_0_vault_account.data).map_err(|e| anyhow::anyhow!("Token0Vault解包失败: {}", e))?;
        info!("✅ Token0Vault解包成功");

        info!("🔍 步骤4: 解包Token1Vault...");
        let token_1_vault_info =
            unpack_token(&token_1_vault_account.data).map_err(|e| anyhow::anyhow!("Token1Vault解包失败: {}", e))?;
        info!("✅ Token1Vault解包成功");

        info!("🔍 步骤5: 解包Token0Mint...");
        let token_0_mint_info =
            unpack_mint(&token_0_mint_account.data).map_err(|e| anyhow::anyhow!("Token0Mint解包失败: {}", e))?;
        info!("✅ Token0Mint解包成功");

        info!("🔍 步骤6: 解包Token1Mint...");
        let token_1_mint_info =
            unpack_mint(&token_1_mint_account.data).map_err(|e| anyhow::anyhow!("Token1Mint解包失败: {}", e))?;
        info!("✅ Token1Mint解包成功");

        info!("🔍 步骤7: 解包UserInputToken...");
        let user_input_token_info = unpack_token(&user_input_token_account.data)
            .map_err(|e| anyhow::anyhow!("UserInputToken解包失败: {}", e))?;
        info!("✅ UserInputToken解包成功");

        info!("🎉 所有账户反序列化完成，继续后续计算...");

        let (total_token_0_amount, total_token_1_amount) = pool_state
            .vault_amount_without_fee(
                token_0_vault_info.base.amount.into(),
                token_1_vault_info.base.amount.into(),
            )
            .unwrap();

        let (
            trade_direction,
            total_input_token_amount,
            total_output_token_amount,
            input_token_mint,
            output_token_mint,
            transfer_fee,
        ) = if user_input_token_info.base.mint == token_0_vault_info.base.mint {
            (
                TradeDirection::ZeroForOne,
                total_token_0_amount,
                total_token_1_amount,
                pool_state.token_0_mint,
                pool_state.token_1_mint,
                get_transfer_fee(&token_0_mint_info, epoch, user_input_amount),
            )
        } else {
            (
                TradeDirection::OneForZero,
                total_token_1_amount,
                total_token_0_amount,
                pool_state.token_1_mint,
                pool_state.token_0_mint,
                get_transfer_fee(&token_1_mint_info, epoch, user_input_amount),
            )
        };

        let actual_amount_in = user_input_amount.saturating_sub(transfer_fee);

        // 🔧 关键修复：需要根据池子的enable_creator_fee标志调整creator_fee_rate
        let creator_fee_rate = if pool_state.enable_creator_fee {
            amm_config_state.creator_fee_rate
        } else {
            0
        };

        let curve_result = CurveCalculator::swap_base_input(
            trade_direction,
            u128::from(actual_amount_in),
            u128::from(total_input_token_amount),
            u128::from(total_output_token_amount),
            amm_config_state.trade_fee_rate,
            creator_fee_rate, // 使用调整后的creator_fee_rate
            amm_config_state.protocol_fee_rate,
            amm_config_state.fund_fee_rate,
            pool_state.is_creator_fee_on_input(trade_direction).unwrap(),
        )
        .ok_or_else(|| anyhow::anyhow!("交换计算失败：零交易代币"))?;

        let amount_out = u64::try_from(curve_result.output_amount)?;

        let output_transfer_fee = match trade_direction {
            TradeDirection::ZeroForOne => get_transfer_fee(&token_1_mint_info, epoch, amount_out),
            TradeDirection::OneForZero => get_transfer_fee(&token_0_mint_info, epoch, amount_out),
        };

        let amount_received = amount_out.checked_sub(output_transfer_fee).unwrap();
        let minimum_amount_out = amount_with_slippage(amount_received, slippage, false);

        // 计算价格比率和影响
        let price_ratio = if actual_amount_in > 0 {
            amount_received as f64 / actual_amount_in as f64
        } else {
            0.0
        };

        // 价格影响计算：基于输出金额占池子总量的百分比（与CLI保持一致，CLI没有复杂的价格影响计算）
        let price_impact_percent = (curve_result.output_amount as f64 / total_output_token_amount as f64) * 100.0;
        let trade_fee = u64::try_from(curve_result.trade_fee)?;

        let trade_direction_str = match trade_direction {
            TradeDirection::ZeroForOne => "ZeroForOne",
            TradeDirection::OneForZero => "OneForZero",
        };

        Ok(CpmmSwapBaseInCompute {
            pool_id: request.pool_id,
            input_token_mint: input_token_mint.to_string(),
            output_token_mint: output_token_mint.to_string(),
            user_input_amount,
            actual_amount_in,
            amount_out,
            amount_received,
            minimum_amount_out,
            input_transfer_fee: transfer_fee,
            output_transfer_fee,
            price_ratio,
            price_impact_percent,
            trade_fee,
            slippage: slippage * 100.0, // 转换回百分比
            pool_info: PoolStateInfo {
                total_token_0_amount,
                total_token_1_amount,
                token_0_mint: pool_state.token_0_mint.to_string(),
                token_1_mint: pool_state.token_1_mint.to_string(),
                trade_direction: trade_direction_str.to_string(),
                amm_config: AmmConfigInfo {
                    trade_fee_rate: amm_config_state.trade_fee_rate,
                    creator_fee_rate: amm_config_state.creator_fee_rate,
                    protocol_fee_rate: amm_config_state.protocol_fee_rate,
                    fund_fee_rate: amm_config_state.fund_fee_rate,
                },
            },
        })
    }

    /// 构建CPMM SwapBaseIn交易（不发送）
    ///
    /// 基于计算结果构建交易数据，供客户端签名和发送
    pub async fn build_cpmm_swap_base_in_transaction(
        &self,
        request: CpmmSwapBaseInTransactionRequest,
    ) -> Result<CpmmTransactionData> {
        info!(
            "构建CPMM SwapBaseIn交易: wallet={}, pool_id={}",
            request.wallet, request.swap_compute.pool_id
        );

        let wallet = Pubkey::from_str(&request.wallet)?;
        let pool_id = Pubkey::from_str(&request.swap_compute.pool_id)?;
        let swap_compute = &request.swap_compute;

        // 从计算结果中提取必要信息
        let input_token_mint = Pubkey::from_str(&swap_compute.input_token_mint)?;
        let output_token_mint = Pubkey::from_str(&swap_compute.output_token_mint)?;

        // 加载池子状态以获取必要的账户信息，添加详细的验证和错误诊断
        let rpc_client = &self.shared.rpc_client;

        // 获取配置的CPMM程序ID
        let cpmm_program_id = ConfigManager::get_cpmm_program_id()?;

        // 首先检查账户是否存在
        let pool_account = match rpc_client.get_account(&pool_id) {
            Ok(account) => account,
            Err(e) => {
                info!("池子账户不存在或获取失败: pool_id={}, error={}", pool_id, e);
                return Err(anyhow::anyhow!("池子账户不存在或无法访问: {}, 错误: {}", pool_id, e));
            }
        };

        // 检查账户所有者是否是CPMM程序
        if pool_account.owner != cpmm_program_id {
            info!(
                "池子账户所有者不正确: pool_id={}, expected_owner={}, actual_owner={}",
                pool_id, cpmm_program_id, pool_account.owner
            );
            return Err(anyhow::anyhow!(
                "无效的池子地址，账户所有者不是CPMM程序: expected={}, actual={}",
                cpmm_program_id,
                pool_account.owner
            ));
        }

        // 检查账户数据长度
        info!(
            "池子账户信息: pool_id={}, owner={}, data_length={}, lamports={}",
            pool_id,
            pool_account.owner,
            pool_account.data.len(),
            pool_account.lamports
        );

        if pool_account.data.len() < 8 {
            return Err(anyhow::anyhow!(
                "池子账户数据长度不足，无法包含discriminator: length={}",
                pool_account.data.len()
            ));
        }

        // 检查discriminator
        let discriminator = &pool_account.data[0..8];
        info!("账户discriminator: {:?}", discriminator);

        // 尝试反序列化池子状态
        let pool_state: PoolState = match deserialize_anchor_account::<PoolState>(&pool_account) {
            Ok(state) => {
                info!("✅ 构建交易池子状态反序列化成功");
                info!("🏊‍♀️ 构建交易 Pool详细信息:");
                info!("  amm_config: {}", state.amm_config);
                info!("  token_0_mint: {}", state.token_0_mint);
                info!("  token_1_mint: {}", state.token_1_mint);
                info!("  token_0_vault: {}", state.token_0_vault);
                info!("  token_1_vault: {}", state.token_1_vault);
                info!("  token_0_program: {}", state.token_0_program);
                info!("  token_1_program: {}", state.token_1_program);
                info!("  observation_key: {}", state.observation_key);
                info!("  auth_bump: {}", state.auth_bump);
                info!("  status: {}", state.status);
                info!("  lp_mint: {}", state.lp_mint);
                // 复制packed字段到本地变量以避免不对齐的引用
                let lp_supply = state.lp_supply;
                let protocol_fees_token_0 = state.protocol_fees_token_0;
                let protocol_fees_token_1 = state.protocol_fees_token_1;
                let fund_fees_token_0 = state.fund_fees_token_0;
                let fund_fees_token_1 = state.fund_fees_token_1;
                let open_time = state.open_time;
                info!("  lp_supply: {}", lp_supply);
                info!("  protocol_fees_token_0: {}", protocol_fees_token_0);
                info!("  protocol_fees_token_1: {}", protocol_fees_token_1);
                info!("  fund_fees_token_0: {}", fund_fees_token_0);
                info!("  fund_fees_token_1: {}", fund_fees_token_1);
                info!("  open_time: {}", open_time);
                state
            }
            Err(e) => {
                info!(
                    "池子状态反序列化失败: pool_id={}, error={}, data_hex={}",
                    pool_id,
                    e,
                    hex::encode(&pool_account.data[0..std::cmp::min(32, pool_account.data.len())])
                );
                return Err(anyhow::anyhow!("无法反序列化池子状态，可能不是有效的CPMM池子: {}", e));
            }
        };

        // 计算用户代币账户地址
        let user_input_token = spl_associated_token_account::get_associated_token_address(&wallet, &input_token_mint);
        let user_output_token = spl_associated_token_account::get_associated_token_address(&wallet, &output_token_mint);

        let mut instructions = Vec::new();
        instructions.push(ComputeBudgetInstruction::set_compute_unit_limit(800_000));

        // 确定交易方向和对应的vault/program（基于swap_compute的mint信息）
        let (input_vault, output_vault, input_token_program, output_token_program) =
            if input_token_mint == pool_state.token_0_mint {
                // ZeroForOne方向: input=token0, output=token1
                (
                    pool_state.token_0_vault,
                    pool_state.token_1_vault,
                    pool_state.token_0_program,
                    pool_state.token_1_program,
                )
            } else {
                // OneForZero方向: input=token1, output=token0
                (
                    pool_state.token_1_vault,
                    pool_state.token_0_vault,
                    pool_state.token_1_program,
                    pool_state.token_0_program,
                )
            };

        // 创建输入代币ATA账户指令（如果不存在）
        info!("📝 确保输入代币ATA账户存在: {}", user_input_token);
        let create_input_ata_instrs = create_ata_token_account_instr(input_token_program, &input_token_mint, &wallet)?;
        instructions.extend(create_input_ata_instrs);

        // 创建输出代币ATA账户指令
        info!("📝 确保输出代币ATA账户存在: {}", user_output_token);
        let create_output_ata_instrs =
            create_ata_token_account_instr(output_token_program, &output_token_mint, &wallet)?;
        instructions.extend(create_output_ata_instrs);

        // SwapV3独有的推荐系统处理
        let mut upper: Option<Pubkey> = None;
        let mut upper_token_account: Option<Pubkey> = None;
        let mut upper_referral: Option<Pubkey> = None;
        let mut upper_upper: Option<Pubkey> = None;
        let mut upper_upper_token_account: Option<Pubkey> = None;
        let mut payer_referral: Option<Pubkey> = None;
        let referral_program_id = ConfigManager::get_referral_program_id()?;

        let payer_key = wallet;
        // 🔧 关键修复：奖励使用output_token，避免与input_token_mint账户重复
        let reward_mint_pubkey = input_token_mint;
        let reward_token_program = TokenUtils::detect_mint_program(&self.shared.rpc_client, &reward_mint_pubkey)?;
        // 仍然需要input_mint_pubkey用于某些推荐系统逻辑
        let input_mint_pubkey = input_token_mint;
        let input_token_program = TokenUtils::detect_mint_program(&self.shared.rpc_client, &input_mint_pubkey)?;
        let pool_address_str = PoolInfoManager::calculate_cpmm_pool_address_pda(
            &input_token_mint.to_string(),
            &output_token_mint.to_string().to_string(),
        )?;
        let pool_address = Pubkey::from_str(&pool_address_str)?;
        let pool_account = self.shared.rpc_client.get_account(&pool_address)?;
        let pool_state: raydium_cp_swap::states::PoolState = SolanaUtils::deserialize_anchor_account(&pool_account)?;
        // 项目方奖励账户使用output_token（与reward_mint一致）
        let project_token_account = spl_associated_token_account::get_associated_token_address_with_program_id(
            &pool_state.pool_creator,
            &reward_mint_pubkey,
            &reward_token_program,
        );
        info!("project_token_account: {}", project_token_account);
        let (payer_referral_pda, _) =
            Pubkey::find_program_address(&[b"referral", &payer_key.to_bytes()], &referral_program_id);
        info!("payer_referral: {}", payer_referral_pda);
        let payer_referral_account_data = self.shared.rpc_client.get_account(&payer_referral_pda);
        match payer_referral_account_data {
            Ok(account_data) => {
                let payer_referral_account: ReferralAccount = SolanaUtils::deserialize_anchor_account(&account_data)?;
                payer_referral = Some(payer_referral_pda);
                match payer_referral_account.upper {
                    Some(upper_key) => {
                        upper = Some(upper_key);
                        // upper奖励账户也使用output_token（与reward_mint一致）
                        upper_token_account = Some(
                            spl_associated_token_account::get_associated_token_address_with_program_id(
                                &upper_key,
                                &reward_mint_pubkey,
                                &reward_token_program,
                            ),
                        );
                        let (upper_referral_pda, _) =
                            Pubkey::find_program_address(&[b"referral", &upper_key.to_bytes()], &referral_program_id);
                        upper_referral = Some(upper_referral_pda);
                        let upper_referral_account = self.shared.rpc_client.get_account(&upper_referral_pda)?;
                        let upper_referral_account: ReferralAccount =
                            SolanaUtils::deserialize_anchor_account(&upper_referral_account)?;

                        match upper_referral_account.upper {
                            Some(upper_upper_key) => {
                                upper_upper = Some(upper_upper_key);
                                // upper_upper奖励账户也使用output_token（与reward_mint一致）
                                upper_upper_token_account = Some(
                                    spl_associated_token_account::get_associated_token_address_with_program_id(
                                        &upper_upper_key,
                                        &reward_mint_pubkey,
                                        &reward_token_program,
                                    ),
                                );
                            }
                            None => {}
                        }
                    }
                    None => {}
                }
            }
            Err(_) => {
                info!("payer_referral_account not found, set it to None");
            }
        }

        // 为上级推荐用户创建奖励代币ATA账户（如果存在上级且不存在）
        if let Some(upper_account) = upper_token_account {
            info!("📝 确保上级推荐用户奖励代币ATA账户存在: {}", upper_account);
            let create_upper_ata_ix =
                spl_associated_token_account::instruction::create_associated_token_account_idempotent(
                    &payer_key,
                    &upper.unwrap(),
                    &reward_mint_pubkey,
                    &reward_token_program,
                );
            instructions.push(create_upper_ata_ix);
        }

        // 为上上级推荐用户创建奖励代币ATA账户（如果存在上上级且不存在）
        if let Some(upper_upper_account) = upper_upper_token_account {
            info!("📝 确保上上级推荐用户奖励代币ATA账户存在: {}", upper_upper_account);
            let create_upper_upper_ata_ix =
                spl_associated_token_account::instruction::create_associated_token_account_idempotent(
                    &payer_key,
                    &upper_upper.unwrap(),
                    &reward_mint_pubkey,
                    &reward_token_program,
                );
            instructions.push(create_upper_upper_ata_ix);
        }

        // 🔧 关键修复：创建项目方代币账户（如果不存在）
        info!("📝 确保项目方代币ATA账户存在: {}", project_token_account);
        let create_project_ata_ix =
            spl_associated_token_account::instruction::create_associated_token_account_idempotent(
                &payer_key,
                &pool_state.pool_creator,
                &reward_mint_pubkey,
                &reward_token_program,
            );
        instructions.push(create_project_ata_ix);

        // 创建SwapBaseIn指令（使用正确的参数顺序）
        let swap_instrs = swap_base_input_instr(
            cpmm_program_id,                 // cpmm_program_id
            wallet,                          // payer
            pool_id,                         // pool_id
            pool_state.amm_config,           // amm_config
            pool_state.observation_key,      // observation_key
            user_input_token,                // input_token_account
            user_output_token,               // output_token_account
            input_vault,                     // input_vault
            output_vault,                    // output_vault
            input_token_program,             // input_token_program
            output_token_program,            // output_token_program
            input_token_mint,                // input_token_mint
            output_token_mint,               // output_token_mint
            swap_compute.user_input_amount,  // amount_in
            swap_compute.minimum_amount_out, // minimum_amount_out
            &input_mint_pubkey,
            payer_referral.as_ref(),
            upper.as_ref(),
            upper_token_account.as_ref(),
            upper_referral.as_ref(),
            upper_upper.as_ref(),
            upper_upper_token_account.as_ref(),
            &project_token_account,
            &referral_program_id,
        )?;
        instructions.extend(swap_instrs);

        // 构建交易
        let recent_blockhash = rpc_client.get_latest_blockhash()?;
        let mut transaction = Transaction::new_with_payer(&instructions, Some(&wallet));
        transaction.message.recent_blockhash = recent_blockhash;

        // 序列化交易
        let transaction_data = bincode::serialize(&transaction)?;
        use base64::{engine::general_purpose, Engine as _};
        let transaction_base64 = general_purpose::STANDARD.encode(&transaction_data);

        Ok(CpmmTransactionData {
            transaction: transaction_base64,
            transaction_size: transaction_data.len(),
            description: "CPMM SwapBaseIn交易".to_string(),
        })
    }

    /// 执行CPMM SwapBaseOut交换
    ///
    /// 100%忠实地实现CLI的SwapBaseOut业务逻辑，包括：
    /// 1. 加载池子状态和多个账户信息
    /// 2. 确定交易方向和相关代币信息
    /// 3. 计算输出转账费，加上期望输出得到实际输出
    /// 4. 使用CurveCalculator::swap_base_output进行交换计算
    /// 5. 计算输入转账费和最大输入金额（含滑点保护）
    /// 6. 创建输出代币ATA账户
    /// 7. 构建并发送交换交易
    pub async fn build_and_send_cpmm_swap_base_out(
        &self,
        request: CpmmSwapBaseOutRequest,
    ) -> Result<CpmmSwapBaseOutResponse> {
        info!(
            "执行CPMM SwapBaseOut: pool_id={}, user_input_token={}, amount_out_less_fee={}",
            request.pool_id, request.user_input_token, request.amount_out_less_fee
        );

        let pool_id = Pubkey::from_str(&request.pool_id)?;
        let user_input_token_raw = Pubkey::from_str(&request.user_input_token)?;
        let amount_out_less_fee = request.amount_out_less_fee;
        let slippage = request.slippage.unwrap_or(0.5) / 100.0; // 转换为小数

        info!("📝 SwapBaseOut接口输入参数分析:");
        info!("  pool_id: {}", pool_id);
        info!("  user_input_token_raw: {}", user_input_token_raw);
        info!("  amount_out_less_fee: {}", amount_out_less_fee);
        info!("  slippage: {}%", slippage * 100.0);

        // 1. 加载池子状态，添加详细验证
        let rpc_client = &self.shared.rpc_client;

        // 检查池子账户是否存在和有效
        let pool_account = match rpc_client.get_account(&pool_id) {
            Ok(account) => account,
            Err(e) => {
                info!("池子账户不存在或获取失败: pool_id={}, error={}", pool_id, e);
                return Err(anyhow::anyhow!("池子账户不存在或无法访问: {}, 错误: {}", pool_id, e));
            }
        };

        // 获取配置的CPMM程序ID
        let cpmm_program_id = ConfigManager::get_cpmm_program_id()?;

        // 验证账户所有者
        if pool_account.owner != cpmm_program_id {
            return Err(anyhow::anyhow!(
                "无效的池子地址，账户所有者不是CPMM程序: expected={}, actual={}",
                cpmm_program_id,
                pool_account.owner
            ));
        }

        // 反序列化池子状态
        let pool_state: PoolState = match deserialize_anchor_account::<PoolState>(&pool_account) {
            Ok(state) => {
                info!("✅ SwapBaseOut池子状态反序列化成功");
                info!("🏊‍♀️ SwapBaseOut Pool详细信息:");
                info!("  amm_config: {}", state.amm_config);
                info!("  token_0_mint: {}", state.token_0_mint);
                info!("  token_1_mint: {}", state.token_1_mint);
                info!("  token_0_vault: {}", state.token_0_vault);
                info!("  token_1_vault: {}", state.token_1_vault);
                info!("  token_0_program: {}", state.token_0_program);
                info!("  token_1_program: {}", state.token_1_program);
                info!("  observation_key: {}", state.observation_key);
                state
            }
            Err(e) => {
                info!("❌ SwapBaseOut池子状态反序列化失败: pool_id={}, error={}", pool_id, e);
                return Err(anyhow::anyhow!("无法反序列化池子状态，可能discriminator不匹配: {}", e));
            }
        };

        // 🔍 智能检测并确定用户代币账户地址
        let user_input_token = {
            info!("🧠 SwapBaseOut开始智能检测用户代币账户...");

            // 检查用户输入的地址是否是池子中的代币mint之一
            let is_token_0_mint = user_input_token_raw == pool_state.token_0_mint;
            let is_token_1_mint = user_input_token_raw == pool_state.token_1_mint;

            if is_token_0_mint || is_token_1_mint {
                // 用户输入的是mint地址，我们需要计算对应的ATA地址
                let wallet_keypair = Keypair::from_base58_string(
                    self.shared
                        .app_config
                        .private_key
                        .as_ref()
                        .ok_or_else(|| anyhow::anyhow!("私钥未配置"))?,
                );
                let wallet_pubkey = wallet_keypair.pubkey();

                let ata_address =
                    spl_associated_token_account::get_associated_token_address(&wallet_pubkey, &user_input_token_raw);

                info!("✅ SwapBaseOut检测到mint地址，已转换为ATA:");
                info!("  mint地址: {}", user_input_token_raw);
                info!("  钱包地址: {}", wallet_pubkey);
                info!("  ATA地址: {}", ata_address);
                info!("  是token_0_mint: {}", is_token_0_mint);
                info!("  是token_1_mint: {}", is_token_1_mint);

                ata_address
            } else {
                // 用户输入的可能已经是代币账户地址，直接使用
                info!(
                    "🔍 SwapBaseOut输入地址不是池子的mint，假设是代币账户地址: {}",
                    user_input_token_raw
                );
                user_input_token_raw
            }
        };

        // 2. 批量加载所有相关账户（与CLI完全相同的逻辑）
        let load_pubkeys = vec![
            pool_id,
            pool_state.amm_config,
            pool_state.token_0_vault,
            pool_state.token_1_vault,
            pool_state.token_0_mint,
            pool_state.token_1_mint,
            user_input_token,
        ];

        let accounts = rpc_client.get_multiple_accounts(&load_pubkeys)?;
        let epoch = rpc_client.get_epoch_info()?.epoch;

        // 3. 解码所有账户数据
        let pool_account = accounts[0].as_ref().unwrap();
        let amm_config_account = accounts[1].as_ref().unwrap();
        let token_0_vault_account = accounts[2].as_ref().unwrap();
        let token_1_vault_account = accounts[3].as_ref().unwrap();
        let token_0_mint_account = accounts[4].as_ref().unwrap();
        let token_1_mint_account = accounts[5].as_ref().unwrap();
        let user_input_token_account = accounts[6].as_ref().unwrap();

        let pool_state: PoolState = deserialize_anchor_account::<PoolState>(pool_account)?;
        let amm_config_state: AmmConfig = deserialize_anchor_account::<AmmConfig>(amm_config_account)?;
        let token_0_vault_info = unpack_token(&token_0_vault_account.data)?;
        let token_1_vault_info = unpack_token(&token_1_vault_account.data)?;
        let token_0_mint_info = unpack_mint(&token_0_mint_account.data)?;
        let token_1_mint_info = unpack_mint(&token_1_mint_account.data)?;
        let user_input_token_info = unpack_token(&user_input_token_account.data)?;

        // 4. 计算池子中的代币总量（扣除费用后）
        let (total_token_0_amount, total_token_1_amount) = pool_state
            .vault_amount_without_fee(
                token_0_vault_info.base.amount.into(),
                token_1_vault_info.base.amount.into(),
            )
            .unwrap();

        // 4.1. 获取私钥和钱包信息
        let private_key = self
            .shared
            .app_config
            .private_key
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("私钥未配置"))?;
        let payer = Keypair::from_base58_string(private_key);
        let payer_pubkey = payer.pubkey();

        // 5. 确定交易方向和相关信息（100%匹配CLI逻辑）
        let (
            trade_direction,
            total_input_token_amount,
            total_output_token_amount,
            user_output_token,
            input_vault,
            output_vault,
            input_token_mint,
            output_token_mint,
            input_token_program,
            output_token_program,
            out_transfer_fee,
        ) = if user_input_token_info.base.mint == token_0_vault_info.base.mint {
            (
                TradeDirection::ZeroForOne,
                total_token_0_amount,
                total_token_1_amount,
                spl_associated_token_account::get_associated_token_address(&payer_pubkey, &pool_state.token_1_mint),
                pool_state.token_0_vault,
                pool_state.token_1_vault,
                pool_state.token_0_mint,
                pool_state.token_1_mint,
                pool_state.token_0_program,
                pool_state.token_1_program,
                get_transfer_inverse_fee(&token_1_mint_info, epoch, amount_out_less_fee),
            )
        } else {
            (
                TradeDirection::OneForZero,
                total_token_1_amount,
                total_token_0_amount,
                spl_associated_token_account::get_associated_token_address(&payer_pubkey, &pool_state.token_0_mint),
                pool_state.token_1_vault,
                pool_state.token_0_vault,
                pool_state.token_1_mint,
                pool_state.token_0_mint,
                pool_state.token_1_program,
                pool_state.token_0_program,
                get_transfer_inverse_fee(&token_0_mint_info, epoch, amount_out_less_fee),
            )
        };

        // 6. 计算实际输出金额（包含转账费）
        let actual_amount_out = amount_out_less_fee.checked_add(out_transfer_fee).unwrap();

        // 7. 使用CurveCalculator::swap_base_output计算交换结果（与CLI完全相同）
        // 🔧 关键修复：需要根据池子的enable_creator_fee标志调整creator_fee_rate
        let creator_fee_rate = if pool_state.enable_creator_fee {
            amm_config_state.creator_fee_rate
        } else {
            0
        };

        let curve_result = CurveCalculator::swap_base_output(
            trade_direction,
            u128::from(actual_amount_out),
            u128::from(total_input_token_amount),
            u128::from(total_output_token_amount),
            amm_config_state.trade_fee_rate,
            creator_fee_rate, // 使用调整后的creator_fee_rate
            amm_config_state.protocol_fee_rate,
            amm_config_state.fund_fee_rate,
            pool_state.is_creator_fee_on_input(trade_direction).unwrap(),
        )
        .ok_or_else(|| anyhow::anyhow!("交换计算失败：零交易代币"))?;

        let source_amount_swapped = u64::try_from(curve_result.input_amount)?;

        // 8. 计算输入代币的转账费
        let amount_in_transfer_fee = match trade_direction {
            TradeDirection::ZeroForOne => get_transfer_inverse_fee(&token_0_mint_info, epoch, source_amount_swapped),
            TradeDirection::OneForZero => get_transfer_inverse_fee(&token_1_mint_info, epoch, source_amount_swapped),
        };

        let input_transfer_amount = source_amount_swapped.checked_add(amount_in_transfer_fee).unwrap();

        // 9. 应用滑点保护计算最大输入金额
        let max_amount_in = amount_with_slippage(input_transfer_amount, slippage, true);

        // 10. 构建交易指令
        let mut instructions = Vec::new();
        instructions.push(ComputeBudgetInstruction::set_compute_unit_limit(800_000));

        // 创建输入代币ATA账户指令（如果不存在）
        info!("📝 确保输入代币ATA账户存在: {}", user_input_token);
        let create_user_input_token_instrs =
            create_ata_token_account_instr(input_token_program, &input_token_mint, &payer_pubkey)?;
        instructions.extend(create_user_input_token_instrs);

        // 创建输出代币ATA账户指令
        info!("📝 确保输出代币ATA账户存在: {}", user_output_token);
        let create_user_output_token_instrs =
            create_ata_token_account_instr(output_token_program, &output_token_mint, &payer_pubkey)?;
        instructions.extend(create_user_output_token_instrs);

        // SwapV3独有的推荐系统处理
        let mut upper: Option<Pubkey> = None;
        let mut upper_token_account: Option<Pubkey> = None;
        let mut upper_referral: Option<Pubkey> = None;
        let mut upper_upper: Option<Pubkey> = None;
        let mut upper_upper_token_account: Option<Pubkey> = None;
        let mut payer_referral: Option<Pubkey> = None;
        let referral_program_id = ConfigManager::get_referral_program_id()?;

        let payer_key = payer_pubkey;
        // 🔧 关键修复：奖励使用output_token，避免与input_token_mint账户重复
        let reward_mint_pubkey = output_token_mint;
        info!("reward_mint_pubkey: {}", reward_mint_pubkey);
        let reward_token_program = TokenUtils::detect_mint_program(&self.shared.rpc_client, &reward_mint_pubkey)?;
        info!("reward_token_program: {}", reward_token_program);
        let pool_address_str = PoolInfoManager::calculate_cpmm_pool_address_pda(
            &input_token_mint.to_string(),
            &output_token_mint.to_string().to_string(),
        )?;
        let pool_address = Pubkey::from_str(&pool_address_str)?;
        let pool_account = self.shared.rpc_client.get_account(&pool_address)?;
        let pool_state: raydium_cp_swap::states::PoolState = SolanaUtils::deserialize_anchor_account(&pool_account)?;
        // 项目方奖励账户使用output_token（与reward_mint一致）
        let project_token_account = spl_associated_token_account::get_associated_token_address_with_program_id(
            &pool_state.pool_creator,
            &reward_mint_pubkey,
            &reward_token_program,
        );
        info!("project_token_account: {}", project_token_account);
        let (payer_referral_pda, _) =
            Pubkey::find_program_address(&[b"referral", &payer_key.to_bytes()], &referral_program_id);
        info!("payer_referral: {}", payer_referral_pda);
        let payer_referral_account_data = self.shared.rpc_client.get_account(&payer_referral_pda);
        match payer_referral_account_data {
            Ok(account_data) => {
                let payer_referral_account: ReferralAccount = SolanaUtils::deserialize_anchor_account(&account_data)?;
                payer_referral = Some(payer_referral_pda);
                match payer_referral_account.upper {
                    Some(upper_key) => {
                        upper = Some(upper_key);
                        // upper奖励账户也使用output_token（与reward_mint一致）
                        upper_token_account = Some(
                            spl_associated_token_account::get_associated_token_address_with_program_id(
                                &upper_key,
                                &reward_mint_pubkey,
                                &reward_token_program,
                            ),
                        );
                        let (upper_referral_pda, _) =
                            Pubkey::find_program_address(&[b"referral", &upper_key.to_bytes()], &referral_program_id);
                        upper_referral = Some(upper_referral_pda);
                        let upper_referral_account = self.shared.rpc_client.get_account(&upper_referral_pda)?;
                        let upper_referral_account: ReferralAccount =
                            SolanaUtils::deserialize_anchor_account(&upper_referral_account)?;

                        match upper_referral_account.upper {
                            Some(upper_upper_key) => {
                                upper_upper = Some(upper_upper_key);
                                // upper_upper奖励账户也使用output_token（与reward_mint一致）
                                upper_upper_token_account = Some(
                                    spl_associated_token_account::get_associated_token_address_with_program_id(
                                        &upper_upper_key,
                                        &reward_mint_pubkey,
                                        &reward_token_program,
                                    ),
                                );
                            }
                            None => {}
                        }
                    }
                    None => {}
                }
            }
            Err(_) => {
                info!("payer_referral_account not found, set it to None");
            }
        }

        // 为上级推荐用户创建奖励代币ATA账户（如果存在上级且不存在）
        if let Some(upper_account) = upper_token_account {
            info!("📝 确保上级推荐用户奖励代币ATA账户存在: {}", upper_account);
            let create_upper_ata_ix =
                spl_associated_token_account::instruction::create_associated_token_account_idempotent(
                    &payer_key,
                    &upper.unwrap(),
                    &reward_mint_pubkey,
                    &reward_token_program,
                );
            instructions.push(create_upper_ata_ix);
        }

        // 为上上级推荐用户创建奖励代币ATA账户（如果存在上上级且不存在）
        if let Some(upper_upper_account) = upper_upper_token_account {
            info!("📝 确保上上级推荐用户奖励代币ATA账户存在: {}", upper_upper_account);
            let create_upper_upper_ata_ix =
                spl_associated_token_account::instruction::create_associated_token_account_idempotent(
                    &payer_key,
                    &upper_upper.unwrap(),
                    &reward_mint_pubkey,
                    &reward_token_program,
                );
            instructions.push(create_upper_upper_ata_ix);
        }

        // 🔧 关键修复：创建项目方代币账户（如果不存在）
        info!("📝 确保项目方代币ATA账户存在: {}", project_token_account);
        let create_project_ata_ix =
            spl_associated_token_account::instruction::create_associated_token_account_idempotent(
                &payer_key,
                &pool_state.pool_creator,
                &reward_mint_pubkey,
                &reward_token_program,
            );
        instructions.push(create_project_ata_ix);

        // 创建SwapBaseOutput指令（使用从CLI逻辑推导出的正确参数）
        let swap_base_out_instrs = swap_base_output_instr(
            cpmm_program_id,
            payer_pubkey,
            pool_id,
            pool_state.amm_config,
            pool_state.observation_key,
            user_input_token,
            user_output_token,
            input_vault,
            output_vault,
            input_token_program,
            output_token_program,
            input_token_mint,
            output_token_mint,
            max_amount_in,
            amount_out_less_fee,
            &reward_mint_pubkey, // reward_mint: 使用output_token避免重复
            payer_referral.as_ref(),
            upper.as_ref(),
            upper_token_account.as_ref(),
            upper_referral.as_ref(),
            upper_upper.as_ref(),
            upper_upper_token_account.as_ref(),
            &project_token_account,
            &referral_program_id,
        )?;
        instructions.extend(swap_base_out_instrs);

        // 11. 构建并发送交易
        let recent_blockhash = rpc_client.get_latest_blockhash()?;
        let transaction =
            Transaction::new_signed_with_payer(&instructions, Some(&payer_pubkey), &[&payer], recent_blockhash);

        let signature = rpc_client.send_and_confirm_transaction(&transaction)?;

        info!("CPMM SwapBaseOut交易成功: {}", signature);

        // 12. 构建响应
        let explorer_url = format!("https://solscan.io/tx/{}", signature);
        let now = chrono::Utc::now().timestamp();

        Ok(CpmmSwapBaseOutResponse {
            signature: signature.to_string(),
            pool_id: request.pool_id,
            input_token_mint: input_token_mint.to_string(),
            output_token_mint: output_token_mint.to_string(),
            amount_out_less_fee,
            actual_amount_out,
            source_amount_swapped,
            input_transfer_amount,
            max_amount_in,
            input_transfer_fee: amount_in_transfer_fee,
            output_transfer_fee: out_transfer_fee,
            status: TransactionStatus::Confirmed,
            explorer_url,
            timestamp: now,
        })
    }

    /// 计算CPMM SwapBaseOut交换结果（不执行实际交换）
    ///
    /// 用于获取报价和预计算结果
    pub async fn compute_cpmm_swap_base_out(&self, request: CpmmSwapBaseOutRequest) -> Result<CpmmSwapBaseOutCompute> {
        info!(
            "计算CPMM SwapBaseOut: pool_id={}, amount_out_less_fee={}",
            request.pool_id, request.amount_out_less_fee
        );

        let pool_id = Pubkey::from_str(&request.pool_id)?;
        let user_input_token_raw = Pubkey::from_str(&request.user_input_token)?;
        let amount_out_less_fee = request.amount_out_less_fee;
        let slippage = request.slippage.unwrap_or(0.5) / 100.0;

        info!("📝 SwapBaseOut计算输入参数分析:");
        info!("  pool_id: {}", pool_id);
        info!("  user_input_token_raw: {}", user_input_token_raw);
        info!("  amount_out_less_fee: {}", amount_out_less_fee);
        info!("  slippage: {}%", slippage * 100.0);

        // 执行与swap_base_out相同的计算逻辑，但不发送交易
        let rpc_client = &self.shared.rpc_client;

        // 获取配置的CPMM程序ID
        let cpmm_program_id = ConfigManager::get_cpmm_program_id()?;

        // 加载并验证池子账户
        let pool_account = match rpc_client.get_account(&pool_id) {
            Ok(account) => account,
            Err(e) => {
                info!("计算SwapBaseOut时池子账户不存在: pool_id={}, error={}", pool_id, e);
                return Err(anyhow::anyhow!("池子账户不存在或无法访问: {}", e));
            }
        };

        // 验证账户所有者
        if pool_account.owner != cpmm_program_id {
            return Err(anyhow::anyhow!(
                "无效的池子地址，账户所有者不正确: expected={}, actual={}",
                cpmm_program_id,
                pool_account.owner
            ));
        }

        let pool_state: PoolState = match deserialize_anchor_account::<PoolState>(&pool_account) {
            Ok(state) => {
                info!("✅ SwapBaseOut Compute函数池子状态反序列化成功");
                state
            }
            Err(e) => {
                info!(
                    "计算SwapBaseOut时池子状态反序列化失败: pool_id={}, error={}",
                    pool_id, e
                );
                return Err(anyhow::anyhow!("无法反序列化池子状态: {}", e));
            }
        };

        // 🔍 智能检测并确定用户代币账户地址
        let user_input_token = {
            info!("🧠 SwapBaseOut Compute开始智能检测用户代币账户...");

            // 检查用户输入的地址是否是池子中的代币mint之一
            let is_token_0_mint = user_input_token_raw == pool_state.token_0_mint;
            let is_token_1_mint = user_input_token_raw == pool_state.token_1_mint;

            if is_token_0_mint || is_token_1_mint {
                // 用户输入的是mint地址，我们需要计算对应的ATA地址
                let wallet_keypair = Keypair::from_base58_string(
                    self.shared
                        .app_config
                        .private_key
                        .as_ref()
                        .ok_or_else(|| anyhow::anyhow!("私钥未配置"))?,
                );
                let wallet_pubkey = wallet_keypair.pubkey();

                let ata_address =
                    spl_associated_token_account::get_associated_token_address(&wallet_pubkey, &user_input_token_raw);

                info!("✅ SwapBaseOut Compute检测到mint地址，已转换为ATA:");
                info!("  mint地址: {}", user_input_token_raw);
                info!("  钱包地址: {}", wallet_pubkey);
                info!("  ATA地址: {}", ata_address);

                ata_address
            } else {
                // 用户输入的可能已经是代币账户地址，直接使用
                info!(
                    "🔍 SwapBaseOut Compute输入地址不是池子的mint，假设是代币账户地址: {}",
                    user_input_token_raw
                );
                user_input_token_raw
            }
        };

        let load_pubkeys = vec![
            pool_id,
            pool_state.amm_config,
            pool_state.token_0_vault,
            pool_state.token_1_vault,
            pool_state.token_0_mint,
            pool_state.token_1_mint,
            user_input_token,
        ];

        let accounts = rpc_client.get_multiple_accounts(&load_pubkeys)?;
        let epoch = rpc_client.get_epoch_info()?.epoch;

        // 解码账户数据
        let pool_account = accounts[0].as_ref().unwrap();
        let amm_config_account = accounts[1].as_ref().unwrap();
        let token_0_vault_account = accounts[2].as_ref().unwrap();
        let token_1_vault_account = accounts[3].as_ref().unwrap();
        let token_0_mint_account = accounts[4].as_ref().unwrap();
        let token_1_mint_account = accounts[5].as_ref().unwrap();
        let user_input_token_account = accounts[6].as_ref().unwrap();

        let pool_state: PoolState = deserialize_anchor_account(pool_account)?;
        let amm_config_state: AmmConfig = deserialize_anchor_account(amm_config_account)?;
        let token_0_vault_info = unpack_token(&token_0_vault_account.data)?;
        let token_1_vault_info = unpack_token(&token_1_vault_account.data)?;
        let token_0_mint_info = unpack_mint(&token_0_mint_account.data)?;
        let token_1_mint_info = unpack_mint(&token_1_mint_account.data)?;
        let user_input_token_info = unpack_token(&user_input_token_account.data)?;

        let (total_token_0_amount, total_token_1_amount) = pool_state
            .vault_amount_without_fee(
                token_0_vault_info.base.amount.into(),
                token_1_vault_info.base.amount.into(),
            )
            .unwrap();

        let (
            trade_direction,
            total_input_token_amount,
            total_output_token_amount,
            input_token_mint,
            output_token_mint,
            out_transfer_fee,
        ) = if user_input_token_info.base.mint == token_0_vault_info.base.mint {
            (
                TradeDirection::ZeroForOne,
                total_token_0_amount,
                total_token_1_amount,
                pool_state.token_0_mint,
                pool_state.token_1_mint,
                get_transfer_inverse_fee(&token_1_mint_info, epoch, amount_out_less_fee),
            )
        } else {
            (
                TradeDirection::OneForZero,
                total_token_1_amount,
                total_token_0_amount,
                pool_state.token_1_mint,
                pool_state.token_0_mint,
                get_transfer_inverse_fee(&token_0_mint_info, epoch, amount_out_less_fee),
            )
        };

        let actual_amount_out = amount_out_less_fee.checked_add(out_transfer_fee).unwrap();

        // 🔧 关键修复：需要根据池子的enable_creator_fee标志调整creator_fee_rate
        let creator_fee_rate = if pool_state.enable_creator_fee {
            amm_config_state.creator_fee_rate
        } else {
            0
        };

        let curve_result = CurveCalculator::swap_base_output(
            trade_direction,
            u128::from(actual_amount_out),
            u128::from(total_input_token_amount),
            u128::from(total_output_token_amount),
            amm_config_state.trade_fee_rate,
            creator_fee_rate, // 使用调整后的creator_fee_rate
            amm_config_state.protocol_fee_rate,
            amm_config_state.fund_fee_rate,
            pool_state.is_creator_fee_on_input(trade_direction).unwrap(),
        )
        .ok_or_else(|| anyhow::anyhow!("交换计算失败：零交易代币"))?;

        let source_amount_swapped = u64::try_from(curve_result.input_amount)?;

        let amount_in_transfer_fee = match trade_direction {
            TradeDirection::ZeroForOne => get_transfer_inverse_fee(&token_0_mint_info, epoch, source_amount_swapped),
            TradeDirection::OneForZero => get_transfer_inverse_fee(&token_1_mint_info, epoch, source_amount_swapped),
        };

        let input_transfer_amount = source_amount_swapped.checked_add(amount_in_transfer_fee).unwrap();
        let max_amount_in = amount_with_slippage(input_transfer_amount, slippage, true);

        // 计算价格比率和影响
        let price_ratio = if source_amount_swapped > 0 {
            amount_out_less_fee as f64 / source_amount_swapped as f64
        } else {
            0.0
        };

        // 价格影响计算：基于输入金额占池子总量的百分比（与CLI保持一致，CLI没有复杂的价格影响计算）
        let price_impact_percent = (curve_result.input_amount as f64 / total_input_token_amount as f64) * 100.0;
        let trade_fee = u64::try_from(curve_result.trade_fee)?;

        let trade_direction_str = match trade_direction {
            TradeDirection::ZeroForOne => "ZeroForOne",
            TradeDirection::OneForZero => "OneForZero",
        };

        Ok(CpmmSwapBaseOutCompute {
            pool_id: request.pool_id,
            input_token_mint: input_token_mint.to_string(),
            output_token_mint: output_token_mint.to_string(),
            amount_out_less_fee,
            actual_amount_out,
            source_amount_swapped,
            input_transfer_amount,
            max_amount_in,
            input_transfer_fee: amount_in_transfer_fee,
            output_transfer_fee: out_transfer_fee,
            price_ratio,
            price_impact_percent,
            trade_fee,
            slippage: slippage * 100.0, // 转换回百分比
            pool_info: PoolStateInfo {
                total_token_0_amount,
                total_token_1_amount,
                token_0_mint: pool_state.token_0_mint.to_string(),
                token_1_mint: pool_state.token_1_mint.to_string(),
                trade_direction: trade_direction_str.to_string(),
                amm_config: AmmConfigInfo {
                    trade_fee_rate: amm_config_state.trade_fee_rate,
                    creator_fee_rate: amm_config_state.creator_fee_rate,
                    protocol_fee_rate: amm_config_state.protocol_fee_rate,
                    fund_fee_rate: amm_config_state.fund_fee_rate,
                },
            },
        })
    }

    /// 构建CPMM SwapBaseOut交易（不发送）
    ///
    /// 基于计算结果构建交易数据，供客户端签名和发送
    pub async fn build_cpmm_swap_base_out_transaction(
        &self,
        request: CpmmSwapBaseOutTransactionRequest,
    ) -> Result<CpmmTransactionData> {
        info!(
            "构建CPMM SwapBaseOut交易: wallet={}, pool_id={}",
            request.wallet, request.swap_compute.pool_id
        );

        let wallet = Pubkey::from_str(&request.wallet)?;
        let pool_id = Pubkey::from_str(&request.swap_compute.pool_id)?;
        let swap_compute = &request.swap_compute;

        // 从计算结果中提取必要信息
        let input_token_mint = Pubkey::from_str(&swap_compute.input_token_mint)?;
        let output_token_mint = Pubkey::from_str(&swap_compute.output_token_mint)?;

        // 加载池子状态以获取必要的账户信息
        let rpc_client = &self.shared.rpc_client;

        // 获取配置的CPMM程序ID
        let cpmm_program_id = ConfigManager::get_cpmm_program_id()?;

        // 首先检查账户是否存在
        let pool_account = match rpc_client.get_account(&pool_id) {
            Ok(account) => account,
            Err(e) => {
                info!("构建SwapBaseOut交易时池子账户不存在: pool_id={}, error={}", pool_id, e);
                return Err(anyhow::anyhow!("池子账户不存在或无法访问: {}, 错误: {}", pool_id, e));
            }
        };

        // 检查账户所有者是否是CPMM程序
        if pool_account.owner != cpmm_program_id {
            return Err(anyhow::anyhow!(
                "无效的池子地址，账户所有者不是CPMM程序: expected={}, actual={}",
                cpmm_program_id,
                pool_account.owner
            ));
        }

        // 尝试反序列化池子状态
        let pool_state: PoolState = match deserialize_anchor_account::<PoolState>(&pool_account) {
            Ok(state) => {
                info!("✅ 构建SwapBaseOut交易池子状态反序列化成功");
                state
            }
            Err(e) => {
                info!(
                    "构建SwapBaseOut交易池子状态反序列化失败: pool_id={}, error={}",
                    pool_id, e
                );
                return Err(anyhow::anyhow!("无法反序列化池子状态，可能不是有效的CPMM池子: {}", e));
            }
        };

        // 计算用户代币账户地址
        let user_input_token = spl_associated_token_account::get_associated_token_address(&wallet, &input_token_mint);
        let user_output_token = spl_associated_token_account::get_associated_token_address(&wallet, &output_token_mint);

        let mut instructions = Vec::new();
        instructions.push(ComputeBudgetInstruction::set_compute_unit_limit(800_000));

        // 确定交易方向和对应的vault/program（基于swap_compute的mint信息）
        let (input_vault, output_vault, input_token_program, output_token_program) =
            if input_token_mint == pool_state.token_0_mint {
                // ZeroForOne方向: input=token0, output=token1
                (
                    pool_state.token_0_vault,
                    pool_state.token_1_vault,
                    pool_state.token_0_program,
                    pool_state.token_1_program,
                )
            } else {
                // OneForZero方向: input=token1, output=token0
                (
                    pool_state.token_1_vault,
                    pool_state.token_0_vault,
                    pool_state.token_1_program,
                    pool_state.token_0_program,
                )
            };

        // 创建输入代币ATA账户指令（如果不存在）
        info!("📝 确保输入代币ATA账户存在: {}", user_input_token);
        let create_input_ata_instrs = create_ata_token_account_instr(input_token_program, &input_token_mint, &wallet)?;
        instructions.extend(create_input_ata_instrs);

        // 创建输出代币ATA账户指令
        info!("📝 确保输出代币ATA账户存在: {}", user_output_token);
        let create_output_ata_instrs =
            create_ata_token_account_instr(output_token_program, &output_token_mint, &wallet)?;
        instructions.extend(create_output_ata_instrs);

        // SwapV3独有的推荐系统处理
        let mut upper: Option<Pubkey> = None;
        let mut upper_token_account: Option<Pubkey> = None;
        let mut upper_referral: Option<Pubkey> = None;
        let mut upper_upper: Option<Pubkey> = None;
        let mut upper_upper_token_account: Option<Pubkey> = None;
        let mut payer_referral: Option<Pubkey> = None;
        let referral_program_id = ConfigManager::get_referral_program_id()?;

        let payer_key = wallet;
        // 🔧 关键修复：奖励使用output_token，避免与input_token_mint账户重复
        let reward_mint_pubkey = output_token_mint;
        let reward_token_program = TokenUtils::detect_mint_program(&self.shared.rpc_client, &reward_mint_pubkey)?;
        // 仍然需要input_mint_pubkey用于某些推荐系统逻辑
        let input_mint_pubkey = input_token_mint;
        let input_token_program = TokenUtils::detect_mint_program(&self.shared.rpc_client, &input_mint_pubkey)?;
        let pool_address_str = PoolInfoManager::calculate_cpmm_pool_address_pda(
            &input_token_mint.to_string(),
            &output_token_mint.to_string().to_string(),
        )?;
        let pool_address = Pubkey::from_str(&pool_address_str)?;
        let pool_account = self.shared.rpc_client.get_account(&pool_address)?;
        let pool_state: raydium_cp_swap::states::PoolState = SolanaUtils::deserialize_anchor_account(&pool_account)?;
        // 项目方奖励账户使用output_token（与reward_mint一致）
        let project_token_account = spl_associated_token_account::get_associated_token_address_with_program_id(
            &pool_state.pool_creator,
            &reward_mint_pubkey,
            &reward_token_program,
        );
        info!("project_token_account: {}", project_token_account);
        let (payer_referral_pda, _) =
            Pubkey::find_program_address(&[b"referral", &payer_key.to_bytes()], &referral_program_id);
        info!("payer_referral: {}", payer_referral_pda);
        let payer_referral_account_data = self.shared.rpc_client.get_account(&payer_referral_pda);
        match payer_referral_account_data {
            Ok(account_data) => {
                let payer_referral_account: ReferralAccount = SolanaUtils::deserialize_anchor_account(&account_data)?;
                payer_referral = Some(payer_referral_pda);
                match payer_referral_account.upper {
                    Some(upper_key) => {
                        upper = Some(upper_key);
                        // upper奖励账户也使用output_token（与reward_mint一致）
                        upper_token_account = Some(
                            spl_associated_token_account::get_associated_token_address_with_program_id(
                                &upper_key,
                                &reward_mint_pubkey,
                                &reward_token_program,
                            ),
                        );
                        let (upper_referral_pda, _) =
                            Pubkey::find_program_address(&[b"referral", &upper_key.to_bytes()], &referral_program_id);
                        upper_referral = Some(upper_referral_pda);
                        let upper_referral_account = self.shared.rpc_client.get_account(&upper_referral_pda)?;
                        let upper_referral_account: ReferralAccount =
                            SolanaUtils::deserialize_anchor_account(&upper_referral_account)?;

                        match upper_referral_account.upper {
                            Some(upper_upper_key) => {
                                upper_upper = Some(upper_upper_key);
                                // upper_upper奖励账户也使用output_token（与reward_mint一致）
                                upper_upper_token_account = Some(
                                    spl_associated_token_account::get_associated_token_address_with_program_id(
                                        &upper_upper_key,
                                        &reward_mint_pubkey,
                                        &reward_token_program,
                                    ),
                                );
                            }
                            None => {}
                        }
                    }
                    None => {}
                }
            }
            Err(_) => {
                info!("payer_referral_account not found, set it to None");
            }
        }

        // 为上级推荐用户创建奖励代币ATA账户（如果存在上级且不存在）
        if let Some(upper_account) = upper_token_account {
            info!("📝 确保上级推荐用户奖励代币ATA账户存在: {}", upper_account);
            let create_upper_ata_ix =
                spl_associated_token_account::instruction::create_associated_token_account_idempotent(
                    &payer_key,
                    &upper.unwrap(),
                    &reward_mint_pubkey,
                    &reward_token_program,
                );
            instructions.push(create_upper_ata_ix);
        }

        // 为上上级推荐用户创建奖励代币ATA账户（如果存在上上级且不存在）
        if let Some(upper_upper_account) = upper_upper_token_account {
            info!("📝 确保上上级推荐用户奖励代币ATA账户存在: {}", upper_upper_account);
            let create_upper_upper_ata_ix =
                spl_associated_token_account::instruction::create_associated_token_account_idempotent(
                    &payer_key,
                    &upper_upper.unwrap(),
                    &reward_mint_pubkey,
                    &reward_token_program,
                );
            instructions.push(create_upper_upper_ata_ix);
        }

        // 🔧 关键修复：创建项目方代币账户（如果不存在）
        info!("📝 确保项目方代币ATA账户存在: {}", project_token_account);
        let create_project_ata_ix =
            spl_associated_token_account::instruction::create_associated_token_account_idempotent(
                &payer_key,
                &pool_state.pool_creator,
                &reward_mint_pubkey,
                &reward_token_program,
            );
        instructions.push(create_project_ata_ix);

        // 创建SwapBaseOutput指令（使用正确的参数顺序）
        let swap_instrs = swap_base_output_instr(
            cpmm_program_id,                  // cpmm_program_id
            wallet,                           // payer
            pool_id,                          // pool_id
            pool_state.amm_config,            // amm_config
            pool_state.observation_key,       // observation_key
            user_input_token,                 // input_token_account
            user_output_token,                // output_token_account
            input_vault,                      // input_vault
            output_vault,                     // output_vault
            input_token_program,              // input_token_program
            output_token_program,             // output_token_program
            input_token_mint,                 // input_token_mint
            output_token_mint,                // output_token_mint
            swap_compute.max_amount_in,       // max_amount_in
            swap_compute.amount_out_less_fee, // amount_out
            &input_mint_pubkey,
            payer_referral.as_ref(),
            upper.as_ref(),
            upper_token_account.as_ref(),
            upper_referral.as_ref(),
            upper_upper.as_ref(),
            upper_upper_token_account.as_ref(),
            &project_token_account,
            &referral_program_id,
        )?;
        instructions.extend(swap_instrs);

        // 构建交易
        let recent_blockhash = rpc_client.get_latest_blockhash()?;
        let mut transaction = Transaction::new_with_payer(&instructions, Some(&wallet));
        transaction.message.recent_blockhash = recent_blockhash;

        // 序列化交易
        let transaction_data = bincode::serialize(&transaction)?;
        use base64::{engine::general_purpose, Engine as _};
        let transaction_base64 = general_purpose::STANDARD.encode(&transaction_data);

        Ok(CpmmTransactionData {
            transaction: transaction_base64,
            transaction_size: transaction_data.len(),
            description: "CPMM SwapBaseOut交易".to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_client::rpc_client::RpcClient;
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
    fn test_get_transfer_fee_with_no_extension() {
        // 创建最小有效的mint数据（没有transfer fee extension）
        let minimal_mint_data = vec![0u8; 82]; // PodMint的最小大小

        if let Ok(mint_info) = unpack_mint(&minimal_mint_data) {
            let fee = get_transfer_fee(&mint_info, 100, 1000000);
            assert_eq!(fee, 0, "没有extension的mint应该返回0费用");
        }
    }

    #[test]
    fn test_get_transfer_inverse_fee_with_no_extension() {
        // 测试SwapBaseOut新增的反向转账费计算函数
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
        let slippage = 0.005; // 0.5% (注意：应该是小数形式，而不是百分比)

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

        // 大滑点 (注意：使用小数形式，而不是百分比)
        let large_slippage = 0.1; // 10%
        let large_slippage_up = amount_with_slippage(amount, large_slippage, true);
        let large_slippage_down = amount_with_slippage(amount, large_slippage, false);
        // 100 * 1.1 = 110.0 -> ceil(110.0) = 110，但浮点计算可能产生110.00000...01，ceil后是111
        assert_eq!(large_slippage_up, 111, "10%向上滑点应该是111 (由于浮点精度)");
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
    fn test_swap_base_input_instr() {
        // 测试创建SwapBaseInput指令
        let cpmm_program_id = solana_sdk::pubkey::Pubkey::new_unique();
        let payer = solana_sdk::pubkey::Pubkey::new_unique();
        let pool_id = solana_sdk::pubkey::Pubkey::new_unique();
        let amm_config = solana_sdk::pubkey::Pubkey::new_unique();
        let observation_key = solana_sdk::pubkey::Pubkey::new_unique();
        let input_token_account = solana_sdk::pubkey::Pubkey::new_unique();
        let output_token_account = solana_sdk::pubkey::Pubkey::new_unique();
        let input_vault = solana_sdk::pubkey::Pubkey::new_unique();
        let output_vault = solana_sdk::pubkey::Pubkey::new_unique();
        let _input_token_program = spl_token::id();
        let output_token_program = spl_token::id();
        let input_token_mint = solana_sdk::pubkey::Pubkey::new_unique();
        let output_token_mint = solana_sdk::pubkey::Pubkey::new_unique();
        let amount_in = 1000000u64;
        let minimum_amount_out = 950000u64;
        let rpc_client = RpcClient::new("https://api.devnet.solana.com");

        // let raydium_cpmm_program_id = ConfigManager::get_cpmm_program_id().unwrap();
        let reward_mint_pubkey = output_token_mint;
        let reward_token_program = TokenUtils::detect_mint_program(&rpc_client, &reward_mint_pubkey).unwrap();

        // SwapV3独有的推荐系统处理
        let mut upper: Option<Pubkey> = None;
        let mut upper_token_account: Option<Pubkey> = None;
        let mut upper_referral: Option<Pubkey> = None;
        let mut upper_upper: Option<Pubkey> = None;
        let mut upper_upper_token_account: Option<Pubkey> = None;
        let mut payer_referral: Option<Pubkey> = None;
        let referral_program_id = ConfigManager::get_referral_program_id().unwrap();

        let payer_key = payer;
        let input_mint_pubkey = input_token_mint;
        let input_token_program = TokenUtils::detect_mint_program(&rpc_client, &input_mint_pubkey).unwrap();
        let pool_address_str = PoolInfoManager::calculate_cpmm_pool_address_pda(
            &input_token_mint.to_string(),
            &output_token_mint.to_string().to_string(),
        )
        .unwrap();
        let pool_address = Pubkey::from_str(&pool_address_str).unwrap();
        let pool_account = rpc_client.get_account(&pool_address).unwrap();
        let pool_state: raydium_cp_swap::states::PoolState =
            SolanaUtils::deserialize_anchor_account(&pool_account).unwrap();
        // 项目方奖励账户使用output_token（与reward_mint一致）
        let project_token_account = spl_associated_token_account::get_associated_token_address_with_program_id(
            &pool_state.pool_creator,
            &reward_mint_pubkey,
            &reward_token_program,
        );
        info!("project_token_account: {}", project_token_account);
        let (payer_referral_pda, _) =
            Pubkey::find_program_address(&[b"referral", &payer_key.to_bytes()], &referral_program_id);
        info!("payer_referral: {}", payer_referral_pda);
        let payer_referral_account_data = rpc_client.get_account(&payer_referral_pda);
        match payer_referral_account_data {
            Ok(account_data) => {
                let payer_referral_account: ReferralAccount =
                    SolanaUtils::deserialize_anchor_account(&account_data).unwrap();
                payer_referral = Some(payer_referral_pda);
                match payer_referral_account.upper {
                    Some(upper_key) => {
                        upper = Some(upper_key);
                        // upper奖励账户也使用output_token（与reward_mint一致）
                        upper_token_account = Some(
                            spl_associated_token_account::get_associated_token_address_with_program_id(
                                &upper_key,
                                &reward_mint_pubkey,
                                &reward_token_program,
                            ),
                        );
                        let (upper_referral_pda, _) =
                            Pubkey::find_program_address(&[b"referral", &upper_key.to_bytes()], &referral_program_id);
                        upper_referral = Some(upper_referral_pda);
                        let upper_referral_account = rpc_client.get_account(&upper_referral_pda).unwrap();
                        let upper_referral_account: ReferralAccount =
                            SolanaUtils::deserialize_anchor_account(&upper_referral_account).unwrap();

                        match upper_referral_account.upper {
                            Some(upper_upper_key) => {
                                upper_upper = Some(upper_upper_key);
                                // upper_upper奖励账户也使用output_token（与reward_mint一致）
                                upper_upper_token_account = Some(
                                    spl_associated_token_account::get_associated_token_address_with_program_id(
                                        &upper_upper_key,
                                        &reward_mint_pubkey,
                                        &reward_token_program,
                                    ),
                                );
                            }
                            None => {}
                        }
                    }
                    None => {}
                }
            }
            Err(_) => {
                info!("payer_referral_account not found, set it to None");
            }
        }

        // 为上级推荐用户创建输入代币ATA账户（如果存在上级且不存在）
        if let Some(upper_account) = upper_token_account {
            info!("📝 确保上级推荐用户输入代币ATA账户存在: {}", upper_account);
            let _create_upper_ata_ix =
                spl_associated_token_account::instruction::create_associated_token_account_idempotent(
                    &payer_key,
                    &upper.unwrap(),
                    &input_mint_pubkey,
                    &input_token_program,
                );
            // instructions.push(create_upper_ata_ix);
        }

        // 为上上级推荐用户创建输入代币ATA账户（如果存在上上级且不存在）
        if let Some(upper_upper_account) = upper_upper_token_account {
            info!("📝 确保上上级推荐用户输入代币ATA账户存在: {}", upper_upper_account);
            let _create_upper_upper_ata_ix =
                spl_associated_token_account::instruction::create_associated_token_account_idempotent(
                    &payer_key,
                    &upper_upper.unwrap(),
                    &input_mint_pubkey,
                    &input_token_program,
                );
            // instructions.push(create_upper_upper_ata_ix);
        }

        let result = swap_base_input_instr(
            cpmm_program_id,
            payer,
            pool_id,
            amm_config,
            observation_key,
            input_token_account,
            output_token_account,
            input_vault,
            output_vault,
            input_token_program,
            output_token_program,
            input_token_mint,
            output_token_mint,
            amount_in,
            minimum_amount_out,
            &input_mint_pubkey,
            payer_referral.as_ref(),
            upper.as_ref(),
            upper_token_account.as_ref(),
            upper_referral.as_ref(),
            upper_upper.as_ref(),
            upper_upper_token_account.as_ref(),
            &project_token_account,
            &referral_program_id,
        );

        assert!(result.is_ok(), "应该成功创建SwapBaseInput指令");

        let instructions = result.unwrap();
        assert_eq!(instructions.len(), 1, "应该返回一个指令");

        let instruction = &instructions[0];
        assert_eq!(instruction.program_id, cpmm_program_id, "指令程序ID应该匹配");
        assert_eq!(instruction.accounts.len(), 13, "应该有13个账户");

        // 检查 discriminator
        assert_eq!(
            instruction.data[0..8],
            [0x8f, 0xbe, 0x5a, 0xda, 0xc4, 0x1e, 0x33, 0xde],
            "discriminator应该匹配"
        );

        // 检查参数
        let amount_in_bytes = &instruction.data[8..16];
        let minimum_amount_out_bytes = &instruction.data[16..24];
        assert_eq!(u64::from_le_bytes(amount_in_bytes.try_into().unwrap()), amount_in);
        assert_eq!(
            u64::from_le_bytes(minimum_amount_out_bytes.try_into().unwrap()),
            minimum_amount_out
        );
    }

    #[test]
    fn test_swap_base_output_instr() {
        // 测试创建SwapBaseOutput指令（SwapBaseOut新增）
        let cpmm_program_id = solana_sdk::pubkey::Pubkey::new_unique();
        let payer = solana_sdk::pubkey::Pubkey::new_unique();
        let pool_id = solana_sdk::pubkey::Pubkey::new_unique();
        let amm_config = solana_sdk::pubkey::Pubkey::new_unique();
        let observation_key = solana_sdk::pubkey::Pubkey::new_unique();
        let input_token_account = solana_sdk::pubkey::Pubkey::new_unique();
        let output_token_account = solana_sdk::pubkey::Pubkey::new_unique();
        let input_vault = solana_sdk::pubkey::Pubkey::new_unique();
        let output_vault = solana_sdk::pubkey::Pubkey::new_unique();
        let _input_token_program = spl_token::id();
        let output_token_program = spl_token::id();
        let input_token_mint = solana_sdk::pubkey::Pubkey::new_unique();
        let output_token_mint = solana_sdk::pubkey::Pubkey::new_unique();
        let max_amount_in = 1050000u64;
        let amount_out = 1000000u64;
        let rpc_client = RpcClient::new("https://api.devnet.solana.com");

        // let raydium_cpmm_program_id = ConfigManager::get_cpmm_program_id().unwrap();
        // 🔧 关键修复：奖励使用output_token，避免与input_token_mint账户重复
        let reward_mint_pubkey = output_token_mint;
        let reward_token_program = TokenUtils::detect_mint_program(&rpc_client, &reward_mint_pubkey).unwrap();

        // SwapV3独有的推荐系统处理
        let mut upper: Option<Pubkey> = None;
        let mut upper_token_account: Option<Pubkey> = None;
        let mut upper_referral: Option<Pubkey> = None;
        let mut upper_upper: Option<Pubkey> = None;
        let mut upper_upper_token_account: Option<Pubkey> = None;
        let mut payer_referral: Option<Pubkey> = None;
        let referral_program_id = ConfigManager::get_referral_program_id().unwrap();

        let payer_key = payer;
        let input_mint_pubkey = input_token_mint;
        let input_token_program = TokenUtils::detect_mint_program(&rpc_client, &input_mint_pubkey).unwrap();
        let pool_address_str = PoolInfoManager::calculate_cpmm_pool_address_pda(
            &input_token_mint.to_string(),
            &output_token_mint.to_string().to_string(),
        )
        .unwrap();
        let pool_address = Pubkey::from_str(&pool_address_str).unwrap();
        let pool_account = rpc_client.get_account(&pool_address).unwrap();
        let pool_state: raydium_cp_swap::states::PoolState =
            SolanaUtils::deserialize_anchor_account(&pool_account).unwrap();
        // 项目方奖励账户使用output_token（与reward_mint一致）
        let project_token_account = spl_associated_token_account::get_associated_token_address_with_program_id(
            &pool_state.pool_creator,
            &reward_mint_pubkey,
            &reward_token_program,
        );
        info!("project_token_account: {}", project_token_account);
        let (payer_referral_pda, _) =
            Pubkey::find_program_address(&[b"referral", &payer_key.to_bytes()], &referral_program_id);
        info!("payer_referral: {}", payer_referral_pda);
        let payer_referral_account_data = rpc_client.get_account(&payer_referral_pda);
        match payer_referral_account_data {
            Ok(account_data) => {
                let payer_referral_account: ReferralAccount =
                    SolanaUtils::deserialize_anchor_account(&account_data).unwrap();
                payer_referral = Some(payer_referral_pda);
                match payer_referral_account.upper {
                    Some(upper_key) => {
                        upper = Some(upper_key);
                        // upper奖励账户也使用output_token（与reward_mint一致）
                        upper_token_account = Some(
                            spl_associated_token_account::get_associated_token_address_with_program_id(
                                &upper_key,
                                &reward_mint_pubkey,
                                &reward_token_program,
                            ),
                        );
                        let (upper_referral_pda, _) =
                            Pubkey::find_program_address(&[b"referral", &upper_key.to_bytes()], &referral_program_id);
                        upper_referral = Some(upper_referral_pda);
                        let upper_referral_account = rpc_client.get_account(&upper_referral_pda).unwrap();
                        let upper_referral_account: ReferralAccount =
                            SolanaUtils::deserialize_anchor_account(&upper_referral_account).unwrap();

                        match upper_referral_account.upper {
                            Some(upper_upper_key) => {
                                upper_upper = Some(upper_upper_key);
                                // upper_upper奖励账户也使用output_token（与reward_mint一致）
                                upper_upper_token_account = Some(
                                    spl_associated_token_account::get_associated_token_address_with_program_id(
                                        &upper_upper_key,
                                        &reward_mint_pubkey,
                                        &reward_token_program,
                                    ),
                                );
                            }
                            None => {}
                        }
                    }
                    None => {}
                }
            }
            Err(_) => {
                info!("payer_referral_account not found, set it to None");
            }
        }

        // 为上级推荐用户创建输入代币ATA账户（如果存在上级且不存在）
        if let Some(upper_account) = upper_token_account {
            info!("📝 确保上级推荐用户输入代币ATA账户存在: {}", upper_account);
            let _create_upper_ata_ix =
                spl_associated_token_account::instruction::create_associated_token_account_idempotent(
                    &payer_key,
                    &upper.unwrap(),
                    &input_mint_pubkey,
                    &input_token_program,
                );
            // instructions.push(create_upper_ata_ix);
        }

        // 为上上级推荐用户创建输入代币ATA账户（如果存在上上级且不存在）
        if let Some(upper_upper_account) = upper_upper_token_account {
            info!("📝 确保上上级推荐用户输入代币ATA账户存在: {}", upper_upper_account);
            let _create_upper_upper_ata_ix =
                spl_associated_token_account::instruction::create_associated_token_account_idempotent(
                    &payer_key,
                    &upper_upper.unwrap(),
                    &input_mint_pubkey,
                    &input_token_program,
                );
            // instructions.push(create_upper_upper_ata_ix);
        }

        let result = swap_base_output_instr(
            cpmm_program_id,
            payer,
            pool_id,
            amm_config,
            observation_key,
            input_token_account,
            output_token_account,
            input_vault,
            output_vault,
            input_token_program,
            output_token_program,
            input_token_mint,
            output_token_mint,
            max_amount_in,
            amount_out,
            &input_mint_pubkey,
            payer_referral.as_ref(),
            upper.as_ref(),
            upper_token_account.as_ref(),
            upper_referral.as_ref(),
            upper_upper.as_ref(),
            upper_upper_token_account.as_ref(),
            &project_token_account,
            &referral_program_id,
        );

        assert!(result.is_ok(), "应该成功创建SwapBaseOutput指令");

        let instructions = result.unwrap();
        assert_eq!(instructions.len(), 1, "应该返回一个指令");

        let instruction = &instructions[0];
        assert_eq!(instruction.program_id, cpmm_program_id, "指令程序ID应该匹配");
        assert_eq!(instruction.accounts.len(), 13, "应该有13个账户");

        // 检查discriminator（SwapBaseOutput的discriminator）
        assert_eq!(
            instruction.data[0..8],
            [0x37, 0xd9, 0x62, 0x56, 0xa3, 0x4a, 0xb4, 0xad],
            "discriminator应该匹配SwapBaseOutput"
        );

        // 检查参数
        let max_amount_in_bytes = &instruction.data[8..16];
        let amount_out_bytes = &instruction.data[16..24];
        assert_eq!(
            u64::from_le_bytes(max_amount_in_bytes.try_into().unwrap()),
            max_amount_in
        );
        assert_eq!(u64::from_le_bytes(amount_out_bytes.try_into().unwrap()), amount_out);
    }

    #[test]
    fn test_swap_instruction_discriminators() {
        // 确保SwapBaseIn和SwapBaseOut使用不同的discriminator
        let swap_base_input_discriminator = [0x8f, 0xbe, 0x5a, 0xda, 0xc4, 0x1e, 0x33, 0xde];
        let swap_base_output_discriminator = [0x0e, 0x32, 0xc1, 0x9d, 0x8b, 0x24, 0x0e, 0x0e];

        assert_ne!(
            swap_base_input_discriminator, swap_base_output_discriminator,
            "SwapBaseInput和SwapBaseOutput应该使用不同的discriminator"
        );
    }
}
