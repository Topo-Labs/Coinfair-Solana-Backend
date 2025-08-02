use crate::ClientConfig;

use super::super::read_keypair_file;

use anchor_client::{Client, Cluster};
use anchor_lang::prelude::Pubkey;
use anyhow::Result;

use referral::accounts as referral_accounts;
use referral::instruction as referral_instruction;
use referral::states::REFERRAL_CONFIG_SEED;

use solana_sdk::instruction::Instruction;
use solana_sdk::signature::Signer;
use solana_sdk::system_program;
use spl_associated_token_account::get_associated_token_address;
use std::rc::Rc;

pub fn update_nft_mint_instr(config: &ClientConfig, new_nft_mint: Pubkey) -> Result<Vec<Instruction>> {
    let admin = read_keypair_file(&config.admin_path)?;
    let admin_pubkey = admin.pubkey();
    let url = Cluster::Custom(config.http_url.clone(), config.ws_url.clone());
    let client = Client::new(url, Rc::new(admin));
    let program = client.program(config.referral_program)?;
    let (referral_config_key, __bump) = Pubkey::find_program_address(&[REFERRAL_CONFIG_SEED.as_bytes()], &program.id());
    let instructions = program
        .request()
        .accounts(referral_accounts::UpdateNftMint {
            config: referral_config_key,
            admin: admin_pubkey,
        })
        .args(referral_instruction::UpdateNftMint { new_nft_mint })
        .instructions()?;
    Ok(instructions)
}

pub fn init_referral_config_instr(config: &ClientConfig, admin: Pubkey, nft_mint: Pubkey, protocol_wallet: Pubkey, claim_fee: u64) -> Result<Vec<Instruction>> {
    let payer = read_keypair_file(&config.admin_path)?;
    let url = Cluster::Custom(config.http_url.clone(), config.ws_url.clone());
    let client = Client::new(url, Rc::new(payer));
    let program = client.program(config.referral_program)?;
    let (referral_config_key, __bump) = Pubkey::find_program_address(&[REFERRAL_CONFIG_SEED.as_bytes()], &program.id());
    let instructions = program
        .request()
        .accounts(referral_accounts::InitReferralConfig {
            payer: program.payer(),
            config: referral_config_key,
            system_program: system_program::id(),
        })
        .args(referral_instruction::InitConfig {
            admin,
            nft_mint,
            protocol_wallet,
            claim_fee,
        })
        .instructions()?;
    Ok(instructions)
}

pub fn mint_nft_instr(config: &ClientConfig, amount: u64) -> Result<Vec<Instruction>> {
    let upper = read_keypair_file(&config.upper_path)?;
    let url = Cluster::Custom(config.http_url.clone(), config.ws_url.clone());
    let upper_pubkey = upper.pubkey();
    let client = Client::new(url, Rc::new(upper));
    let program = client.program(config.referral_program)?;
    let (referral_config_key, __bump) = Pubkey::find_program_address(&[REFERRAL_CONFIG_SEED.as_bytes()], &program.id());
    // 读取 PDA: mint authority
    let (mint_authority, _) = Pubkey::find_program_address(&[b"mint_authority"], &program.id());
    println!("mint_authority: {}", mint_authority);

    // 推荐关系PDA
    let (user_referral, _) = Pubkey::find_program_address(&[b"referral", &upper_pubkey.to_bytes()], &program.id());

    // NFT 托管 PDA（每个用户独立池）
    let (nft_pool_authority, _) = Pubkey::find_program_address(&[b"nft_pool", upper_pubkey.as_ref()], &program.id());

    // 用户的Mint Counter
    let (mint_counter, _) = Pubkey::find_program_address(&[b"mint_counter", upper_pubkey.as_ref()], &program.id());

    // 托管 NFT 的 TokenAccount
    let nft_pool_account = get_associated_token_address(&nft_pool_authority, &config.coinfair_nft);

    let instructions = program
        .request()
        .accounts(referral_accounts::MintReferralNFT {
            authority: program.payer(),
            config: referral_config_key,
            user_referral,
            official_mint: config.coinfair_nft,
            user_ata: get_associated_token_address(&program.payer(), &config.coinfair_nft),
            mint_counter,
            mint_authority,
            nft_pool_authority,
            nft_pool_account,
            token_program: spl_token::id(),
            associated_token_program: spl_associated_token_account::id(),
            rent: solana_sdk::sysvar::rent::id(),
            system_program: system_program::id(),
        })
        .args(referral_instruction::MintNft { amount })
        .instructions()?;
    Ok(instructions)
}

pub fn claim_nft_instr(config: &ClientConfig, user: Pubkey, upper: Pubkey) -> Result<Vec<Instruction>> {
    let payer = read_keypair_file(&config.lower_path)?;
    let admin = read_keypair_file(&config.admin_path)?;
    let url = Cluster::Custom(config.http_url.clone(), config.ws_url.clone());
    let client = Client::new(url, Rc::new(payer));
    let program = client.program(config.referral_program)?;

    // 全局配置
    let (referral_config, _) = Pubkey::find_program_address(&[b"config"], &program.id());

    println!("Referral Config: {}", referral_config);

    // 推荐关系PDA
    let (user_referral, _) = Pubkey::find_program_address(&[b"referral", user.as_ref()], &program.id());
    let (upper_referral, _) = Pubkey::find_program_address(&[b"referral", upper.as_ref()], &program.id());
    let (upper_mint_counter, _) = Pubkey::find_program_address(&[b"mint_counter", upper.as_ref()], &program.id());

    println!("upper_mint_counter: {}", upper_mint_counter);

    //NFT统一托管
    let (nft_pool_authority, _) = Pubkey::find_program_address(&[b"nft_pool", upper.as_ref()], &program.id());
    // PDA 的 NFT TokenAccount
    let nft_pool_account = get_associated_token_address(&nft_pool_authority, &config.coinfair_nft);

    println!("upper_referral: {}", upper_referral);

    // NFT TokenAccounts
    let user_ata = get_associated_token_address(&user, &config.coinfair_nft);
    // let upper_ata = get_associated_token_address(&upper, &config.coinfair_nft);

    // 用户支付手续费的 SPL Token账户（默认 payer 就是 user）
    // let user_token_account = get_associated_token_address(&user, &config.claim_token_mint);

    // //TODO: Fetch from Referral Program PDA
    // let instructions = program
    //     .request()
    //     .accounts(referral_accounts::ClaimReferralNFT {
    //         user,
    //         upper,
    //         user_referral,
    //         upper_mint_counter,
    //         upper_referral,
    //         // upper_nft_account: upper_ata,
    //         config: referral_config,
    //         official_mint: config.coinfair_nft,
    //         user_ata,
    //         protocol_wallet: admin.pubkey(), // TODO
    //         nft_pool_authority,
    //         nft_pool_account,
    //         token_program: spl_token::id(),
    //         associated_token_program: spl_associated_token_account::id(),
    //         rent: solana_sdk::sysvar::rent::id(),
    //         system_program: solana_sdk::system_program::id(),
    //     })
    //     .args(referral_instruction::ClaimNft {})
    //     .instructions()?;

    // Ok(instructions)

    // 构造指令（带账户元信息）
    let ix = program
        .request()
        .accounts(referral_accounts::ClaimReferralNFT {
            user,
            upper,
            user_referral,
            upper_mint_counter,
            upper_referral,
            config: referral_config,
            official_mint: config.coinfair_nft,
            user_ata,
            protocol_wallet: admin.pubkey(),
            nft_pool_authority,
            nft_pool_account,
            token_program: spl_token::id(),
            associated_token_program: spl_associated_token_account::id(),
            rent: solana_sdk::sysvar::rent::id(),
            system_program: solana_sdk::system_program::id(),
        })
        .args(referral_instruction::ClaimNft {})
        .instructions()?
        .remove(0); // 提取单个 Instruction

    // ✅ Patch upper_mint_counter 为 writable
    let mut ix = ix;
    for account_meta in &mut ix.accounts {
        if account_meta.pubkey == upper_mint_counter {
            account_meta.is_writable = true;
        }
    }

    Ok(vec![ix])
}

// pub fn mint_referral_nft_instr(
//     config: &ClientConfig,
//     program_id: Pubkey, //TODO: 放在配置文件中
//     authority: &Keypair,
//     amount: u64,
// ) -> Result<Vec<Instruction>> {
//     let payer = read_keypair_file(&config.payer_path)?;
//     let url = Cluster::Custom(config.http_url.clone(), config.ws_url.clone());
//     let client = Client::new(url, Rc::new(payer));
//     let program = client.program(program_id)?;

//     // Fetch the config account PDA
//     let (config_pda, _bump) = Pubkey::find_program_address(&[b"config"], &program_id);

//     // Fetch the official NFT mint from the config account
//     let config_account: Account<ReferralConfig> = program.account(config_pda)?;
//     let official_nft_mint = config_account.official_nft_mint;

//     // Derive the user's associated token account
//     let user_ata = get_associated_token_address(&authority.pubkey(), &official_nft_mint);

//     // Build the instruction
//     let instruction = Instruction {
//         program_id,
//         accounts: vec![
//             AccountMeta::new(authority.pubkey(), true), // authority (signer)
//             AccountMeta::new_readonly(config_pda, false), // config
//             AccountMeta::new(official_nft_mint, false), // official_mint
//             AccountMeta::new(user_ata, false),          // user_ata
//             AccountMeta::new_readonly(spl_token::id(), false), // token_program
//             AccountMeta::new_readonly(solana_sdk::system_program::id(), false), // system_program
//             AccountMeta::new_readonly(spl_associated_token_account::id(), false), // associated_token_program
//             AccountMeta::new_readonly(solana_sdk::sysvar::rent::id(), false),     // rent
//         ],
//         data: {
//             // Anchor instruction discriminator + amount
//             let discriminator = anchor_lang::Discriminator::from_str("mint_referral_nft")?;
//             let mut data = discriminator.to_vec();
//             data.extend_from_slice(&amount.to_le_bytes());
//             data
//         },
//     };

//     let instruction = program
//         .request()
//         .accounts(referral_accounts::MintReferralNFT {
//             authority: program.payer(),
//             config: config_pda,
//             official_mint: official_nft_mint,
//             user_ata: user_ata,

//             token_program: ,
//             system_program: system_program::id(),
//         })
//         .args(referral_instruction::MintReferralNFT {
//             amount
//         })
//         .instructions()?;

//     Ok(instruction)
// }

// /// 创建 ClaimReferralNFT 指令
// pub fn claim_referral_nft_instr(
//     config: &ClientConfig,
//     program_id: Pubkey,
//     user: &Keypair,
// ) -> Result<Vec<Instruction>> {
//     let payer = read_keypair_file(&config.payer_path)?;
//     let url = Cluster::Custom(config.http_url.clone(), config.ws_url.clone());
//     let client = Client::new(url, Rc::new(payer.clone()));
//     let program = client.program(program_id)?;

//     // 获取 config 账户的 PDA
//     let (config_pda, _bump) = Pubkey::find_program_address(&[b"config"], &program_id);

//     // 从 config 账户获取 official_nft_mint 和 protocol_receive_wallet
//     let config_account: Account<ReferralConfig> = program.account(config_pda)?;
//     let official_nft_mint = config_account.official_nft_mint;
//     let protocol_receive_wallet = config_account.protocol_receive_wallet;

//     // 推导用户的关联代币账户 (user_ata)
//     let user_ata = get_associated_token_address(&user.pubkey(), &official_nft_mint);

//     // 假设 user_token_account 是用户的 SOL 代币账户，用于支付手续费
//     let user_token_account = get_associated_token_address(&user.pubkey(), &spl_token::id());

//     // 构建指令
//     let instruction = Instruction {
//         program_id,
//         accounts: vec![
//             AccountMeta::new(user.pubkey(), true),        // user (签名者)
//             AccountMeta::new_readonly(config_pda, false), // config
//             AccountMeta::new(official_nft_mint, false),   // official_mint
//             AccountMeta::new(user_ata, false),            // user_ata
//             AccountMeta::new(user_token_account, false),  // user_token_account
//             AccountMeta::new(protocol_receive_wallet, false), // protocol_receive_wallet
//             AccountMeta::new_readonly(spl_token::id(), false), // token_program
//             AccountMeta::new_readonly(solana_sdk::system_program::id(), false), // system_program
//             AccountMeta::new_readonly(solana_sdk::sysvar::rent::id(), false), // rent
//         ],
//         data: {
//             // Anchor 指令鉴别器
//             let discriminator = anchor_lang::Discriminator::from_str("claim_referral_nft")?;
//             discriminator.to_vec()
//         },
//     };

//     Ok(vec![instruction])
// }
