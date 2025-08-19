use crate::config::SwapConfig;
use anyhow::Result;
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    signature::{Keypair, Signature},
    transaction::Transaction,
};
use tracing::{error, info};

pub struct SolanaClient {
    rpc_client: RpcClient,
    wallet: Keypair,
}

impl SolanaClient {
    pub fn new(config: &SwapConfig) -> Result<Self> {
        let rpc_client = RpcClient::new_with_commitment(config.rpc_url.clone(), CommitmentConfig::confirmed());

        let wallet = Keypair::from_base58_string(&config.private_key);

        Ok(Self { rpc_client, wallet })
    }

    pub fn get_rpc_client(&self) -> &RpcClient {
        &self.rpc_client
    }

    pub fn get_wallet(&self) -> &Keypair {
        &self.wallet
    }

    pub async fn send_transaction(&self, transaction: &Transaction) -> Result<Signature> {
        info!("发送交易到 Solana 网络...");

        match self.rpc_client.send_and_confirm_transaction(transaction) {
            Ok(signature) => {
                info!("交易成功! 签名: {}", signature);
                Ok(signature)
            }
            Err(e) => {
                error!("交易失败: {:?}", e);
                Err(anyhow::anyhow!("交易失败: {}", e))
            }
        }
    }

    pub fn get_latest_blockhash(&self) -> Result<solana_sdk::hash::Hash> {
        self.rpc_client
            .get_latest_blockhash()
            .map_err(|e| anyhow::anyhow!("获取最新区块哈希失败: {}", e))
    }

    pub fn get_account_data(&self, pubkey: &solana_sdk::pubkey::Pubkey) -> Result<Vec<u8>> {
        self.rpc_client
            .get_account_data(pubkey)
            .map_err(|e| anyhow::anyhow!("获取账户数据失败: {}", e))
    }
}
