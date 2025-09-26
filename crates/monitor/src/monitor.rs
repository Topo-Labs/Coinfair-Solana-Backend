use super::util::{current_date_and_time, magic_number};
use database::clmm::reward::model::RewardItem;
use ethers::{abi::ParamType, prelude::*, providers::Provider, types::Address, utils::to_checksum};
use serde::Deserialize;
use server::services::Services;
use std::sync::Arc;
use tracing::{error, info};

#[derive(Clone)]
pub struct Monitor {
    pub http_provider: Arc<Provider<Http>>,
    pub ws_provider: Arc<Provider<Ws>>,
    pub pair: Address,
    pub hope: Address,
    pub nft: Address,
    pub batch: Address,
    pub current_block: u64,
    pub current_price: f64,
    pub value_threshold: f64, // 价格阈值(默认为100U，此处设为99.0U)
    pub time_threshold: u64,  // 时间阈值(默认为2025-03-10 00:00:00)
    pub services: Services,
}

impl Monitor {
    pub async fn default(services: Services) -> Self {
        let http_rpc_url = "https://rpc.ankr.com/bsc/9c9763b95d62a8269670b0aa089f1ba82604d70f86115ee5185f54c6a837166f";
        let ws_rpc_url = "wss://rpc.ankr.com/bsc/ws/9c9763b95d62a8269670b0aa089f1ba82604d70f86115ee5185f54c6a837166f";
        let pair = "0x7465858234db8ca7bdcadd0d655368c333a42768";
        let hope = "0x17480b68f3e6c8b25574e2db07bfeb17c8faa056";
        let nft = "0xC1F8700cC127688430eb26634f77ED6Bd175D9dE";
        let batch = "0xe76e0E5EDFbd2F588A40f5F3Ca0056f0707ea6fC";

        let http_provider = Arc::new(Self::get_http_provider(http_rpc_url).await);
        let ws_provider = Arc::new(Self::get_ws_provider(ws_rpc_url).await);

        let pair: Address = pair.parse().expect("Invalid pair address");
        let hope: Address = hope.parse().expect("Invalid HOPE address");
        let nft: Address = nft.parse().expect("Invalid NFT address");
        let batch: Address = batch.parse().expect("Invalid NFT address");

        let current_block = 0;
        let current_price = 0.0;
        let value_threshold = 99.90;
        let time_threshold = 1741536000; // 2025-03-10 00:00:00

        Self {
            http_provider,
            ws_provider,
            pair,
            hope,
            nft,
            batch,
            current_block,
            current_price,
            value_threshold,
            time_threshold,
            services,
        }
    }

    pub async fn run(&self) -> eyre::Result<()> {
        println!("👀 Monitoring the NFT:Claim & Swap:Buy event of $HOPE in Coinfair...\n\n");

        let event_claim = "Claim(address,address)";
        let event_swap = "Swap(address,uint256,uint256,uint256,uint256,address)";
        let event_batch_reward = "BatchTokenTransferred(address,address,uint256,uint256,uint256)";

        let filter = Filter::new()
            .address(vec![self.nft, self.pair, self.batch])
            .events(vec![event_claim, event_swap, event_batch_reward]);

        let mut stream = self.ws_provider.subscribe_logs(&filter).await?;
        while let Some(log) = stream.next().await {
            if let Some(topics) = log.topics.first() {
                match *topics {
                    // NFT Claim
                    x if x == magic_number(event_claim) => {
                        let from: Address = H256::into(log.topics[1].into());
                        let to: Address = H256::into(log.topics[2].into());

                        let from: String = to_checksum(&from, None);
                        let to: String = to_checksum(&to, None);

                        info!("{:?} Claim Event: {} -> {}", current_date_and_time(), from, to);

                        match self.handle_claim(from.clone(), to.clone()).await {
                            Ok(()) => {
                                info!("Claim handled successfully for {} -> {}", from, to);
                            }
                            Err(e) => {
                                error!("Failed to handle claim for {} -> {}: {:?}", from, to, e);
                                // 这里可以添加其他恢复逻辑，或者什么都不做，继续循环
                            }
                        }
                    }

                    // HOPE Swap:Buy
                    x if x == magic_number(event_swap) => {
                        let decoded = ethers::abi::decode(
                            &[
                                ParamType::Uint(256), // amount0In
                                ParamType::Uint(256), // amount1In
                                ParamType::Uint(256), // amount0Out
                                ParamType::Uint(256), // amount1Out
                            ],
                            &log.data,
                        )
                        .unwrap();

                        let sender: Address = H256::into(log.topics[1].into());
                        let to: Address = H256::into(log.topics[2].into());

                        let sender = to_checksum(&sender, None);
                        let to = to_checksum(&to, None);

                        let amount0_in: U256 = decoded[0].clone().into_uint().unwrap();
                        let amount1_in: U256 = decoded[1].clone().into_uint().unwrap();
                        let amount0_out: U256 = decoded[2].clone().into_uint().unwrap();
                        let amount1_out: U256 = decoded[3].clone().into_uint().unwrap();

                        if self.is_buy(amount0_in, amount1_in, amount0_out, amount1_out) {
                            info!("{:?} Buy Event: {} -> {}", current_date_and_time(), sender, to.clone());

                            // amount1_in: （兑换所需的）BNB数量
                            // amount0_out: （兑换出来的）HOPE数量
                            match self.handler_buy(to.clone(), amount1_in, amount0_out).await {
                                Ok(()) => {
                                    info!("Buy handled successfully for {}", to);
                                }
                                Err(e) => {
                                    error!("Failed to handle buy for {}: {:?}", to.clone(), e);
                                    // 这里可以添加其他恢复逻辑，或者什么都不做，继续循环
                                }
                            }
                        }
                    }

                    // BatchTokenTransferred(Rewards)
                    x if x == magic_number(event_batch_reward) => {
                        info!("event_batch_reward");
                        match self.handler_batch_rewards().await {
                            Ok(()) => {
                                info!("Batch_rewards handled successfully");
                            }
                            Err(e) => {
                                error!("Failed to handle batch_rewards: {:?}", e);
                                // 这里可以添加其他恢复逻辑，或者什么都不做，继续循环
                            }
                        }
                    }
                    _ => println!("Unknown event received"),
                }
            }
        }

        Ok(())
    }
}

impl Monitor {
    async fn handle_claim(&self, minter: String, claimer: String) -> eyre::Result<()> {
        self.services
            .refer
            .create_refer(&claimer.to_lowercase(), &minter.to_lowercase())
            .await?;

        Ok(())
    }

    //NOTE 地址要转小写
    async fn handler_buy(&self, user: String, bnb_count: U256, hope_count: U256) -> eyre::Result<()> {
        if self.is_valid_user(user.clone(), bnb_count).await {
            let price_by_usdt = self.price_by_usdt(bnb_count, hope_count).await;

            self.services
                .user
                .create_user(user.to_string(), hope_count.as_u128() as f64 / 1e9, price_by_usdt)
                .await?;

            // 2. 存储该有效新用户所触发的奖励(上级 8U对应的HOPE数量，上上级 2U对应的HOPE数量)
            let rewards = self.gen_rewards(user.clone(), price_by_usdt).await;
            self.services
                .reward
                .create_reward(user.to_string().to_lowercase(), rewards)
                .await?;

            Ok(())
        } else {
            //Err(eyre::eyre!("Invalid user or insufficient bnb_count: {}", user))
            Ok(())
        }
    }

    async fn handler_batch_rewards(&self) -> eyre::Result<()> {
        let _ = self.services.reward.set_all_rewards().await;

        Ok(())
    }

    //
    // async fn handler_reward(&self, user: Address) -> eyre::Result<()> {
    //     let price = self.get_bnb_price().await?;
    //     // uppers可能为1个，可能为2个
    //     let uppers = self.services.refer.get_uppers(user.to_string()).await?;
    //     Ok(())
    // }
}

impl Monitor {
    async fn get_http_provider(rpc_url: &str) -> Provider<Http> {
        Provider::<Http>::try_from(rpc_url).expect("Cannot establish http connection")
    }

    async fn get_ws_provider(rpc_url: &str) -> Provider<Ws> {
        Provider::<Ws>::connect(rpc_url)
            .await
            .expect("Cannot establish ws connection")
    }

    // 有效新用户：
    // 1. 已经在Refer关系中(作为lower)
    // 2. Swap:Buy的HOPE价值大于100U
    // 3. 其Refer关系创建的时间戳在活动开启时间之后
    async fn is_valid_user(&self, user: String, bnb_count: U256) -> bool {
        let refer_result = self.services.refer.get_user(user.to_lowercase()).await;

        let is_valid_usdt = self.is_valid_buy(bnb_count).await;

        // 处理 `get_user` 可能的错误
        let refer = match refer_result {
            Ok(Some(r)) => r,  // 成功获取用户
            _ => return false, // 用户不存在或者查询出错
        };

        // 判断时间戳是否有效
        let is_valid_timestamp = refer.timestamp >= self.time_threshold;

        is_valid_usdt && is_valid_timestamp
    }

    fn _price_by_bnb(&self, bnb_count: U256, hope_count: U256) -> f64 {
        let hope = hope_count.as_u128() as f64 / 1e9; // 10^9
        let bnb = bnb_count.as_u128() as f64 / 1e18; // 10^18

        if bnb == 0.0 {
            return 0.0; // 避免除以零
        }

        hope / bnb
    }

    async fn price_by_usdt(&self, bnb_count: U256, hope_count: U256) -> f64 {
        let bnb_price = self.get_bnb_price().await.unwrap();

        let hope_price_by_usdt = bnb_price * (bnb_count.as_u128() as f64) / (hope_count.as_u128() as f64) / 1e9;

        hope_price_by_usdt
    }

    async fn is_valid_buy(&self, bnb_count: U256) -> bool {
        let bnb_count = bnb_count.as_u128() as f64 / 1e18; // 10^18
        let bnb_price_by_usdt = self.get_bnb_price().await.unwrap();

        let flag = bnb_count * bnb_price_by_usdt > self.value_threshold;

        flag
    }

    fn is_buy(&self, amount0_in: U256, amount1_in: U256, amount0_out: U256, amount1_out: U256) -> bool {
        amount1_in > U256::zero() && amount0_out > U256::zero() && amount0_in.is_zero() && amount1_out.is_zero()
    }

    async fn get_bnb_price(&self) -> Result<f64, reqwest::Error> {
        let url = "https://min-api.cryptocompare.com/data/price?fsym=BNB&tsyms=USD";

        let response = reqwest::Client::new()
            .get(url)
            .header("Accept", "application/json")
            .send()
            .await?;

        // 解析 JSON
        let crypto_price: CryptoPrice = response.json().await?;

        let price = crypto_price.USD;

        Ok(price)
    }

    async fn gen_rewards(&self, user: String, price: f64) -> Vec<RewardItem> {
        let uppers_result = self.services.refer.get_uppers(user.to_string()).await;

        let uppers = match uppers_result {
            Ok(list) => list,
            Err(_) => return vec![],
        };

        let reward_hope = 10.0 / price; //TODO: 常量

        let mut rewards = Vec::new();

        // 分配 80% 给第一个推荐人
        if let Some(first) = uppers.get(0) {
            if !first.is_empty() {
                rewards.push(RewardItem {
                    address: first.clone(),
                    amount: reward_hope * 0.8,
                });
            }
        }

        // 分配 20% 给第二个推荐人（如果存在）
        if let Some(second) = uppers.get(1) {
            if !second.is_empty() {
                rewards.push(RewardItem {
                    address: second.clone(),
                    amount: reward_hope * 0.2,
                });
            }
        }

        rewards
    }
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
struct CryptoPrice {
    USD: f64,
}
