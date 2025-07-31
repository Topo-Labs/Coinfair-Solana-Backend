//! Metaplex API 集成服务
//!
//! 负责从 Metaplex API 获取代币元数据信息，包括名称、符号、Logo URI 等

use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use tracing::{debug, info, warn};

// Solana 相关导入
use borsh::{BorshDeserialize, BorshSerialize};
use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use spl_token::solana_program::program_pack::Pack;

// Token-2022 相关导入
use spl_token_2022::{
    extension::{metadata_pointer::MetadataPointer, BaseStateWithExtensions, StateWithExtensions},
    state::Mint as Mint2022,
};
use spl_token_metadata_interface::state::TokenMetadata as Token2022Metadata;

/// 简化的Metaplex元数据结构
#[derive(Debug, Clone, BorshDeserialize, BorshSerialize)]
pub struct SimpleMetadata {
    pub key: u8,
    pub update_authority: Pubkey,
    pub mint: Pubkey,
    pub data: SimpleData,
    pub primary_sale_happened: bool,
    pub is_mutable: bool,
}

/// 简化的元数据数据结构
#[derive(Debug, Clone, BorshDeserialize, BorshSerialize)]
pub struct SimpleData {
    pub name: String,
    pub symbol: String,
    pub uri: String,
    pub seller_fee_basis_points: u16,
    pub creators: Option<Vec<SimpleCreator>>,
}

/// 简化的创建者结构
#[derive(Debug, Clone, BorshDeserialize, BorshSerialize)]
pub struct SimpleCreator {
    pub address: Pubkey,
    pub verified: bool,
    pub share: u8,
}

/// Metaplex Token Metadata 程序ID
const METADATA_PROGRAM_ID: &str = "metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s";

/// Token-2022 程序ID
const TOKEN_2022_PROGRAM_ID: &str = "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb";

/// Metaplex API 配置
#[derive(Debug, Clone)]
pub struct MetaplexConfig {
    /// API 基础 URL
    pub base_url: String,
    /// 请求超时时间（秒）
    pub timeout_seconds: u64,
    /// 最大重试次数
    pub max_retries: u32,
    /// 批量请求大小
    pub batch_size: usize,
    /// Solana 网络环境 (mainnet, devnet, testnet)
    pub network: String,
}

impl Default for MetaplexConfig {
    fn default() -> Self {
        Self {
            base_url: "https://api.metaplex.com".to_string(),
            timeout_seconds: 30,
            max_retries: 3,
            batch_size: 50,
            network: "mainnet".to_string(),
        }
    }
}

/// 代币元数据信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenMetadata {
    /// 代币地址
    pub address: String,
    /// 代币符号
    pub symbol: Option<String>,
    /// 代币名称
    pub name: Option<String>,
    /// Logo URI
    pub logo_uri: Option<String>,
    /// 描述
    pub description: Option<String>,
    /// 外部链接
    pub external_url: Option<String>,
    /// 属性
    pub attributes: Option<Vec<TokenAttribute>>,
    /// 标签
    pub tags: Vec<String>,
}

/// 代币属性
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TokenAttribute {
    /// 属性名
    pub trait_type: String,
    /// 属性值
    pub value: String,
}

/// URI元数据结构（从链上URI获取的JSON数据）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UriMetadata {
    /// 代币名称
    pub name: Option<String>,
    /// 代币符号
    pub symbol: Option<String>,
    /// 描述
    pub description: Option<String>,
    /// 图片URL
    pub image: Option<String>,
    /// 动画URL
    pub animation_url: Option<String>,
    /// 外部链接
    pub external_url: Option<String>,
    /// 属性列表
    pub attributes: Option<Vec<TokenAttribute>>,
    /// 其他属性（用于兼容性）
    pub properties: Option<serde_json::Value>,
}

/// Metaplex API 服务
pub struct MetaplexService {
    client: Client,
    config: MetaplexConfig,
    /// 元数据缓存
    cache: HashMap<String, TokenMetadata>,
    /// Solana RPC 客户端
    rpc_client: Option<RpcClient>,
}

impl MetaplexService {
    /// 创建新的 Metaplex 服务实例
    pub fn new(config: Option<MetaplexConfig>) -> Result<Self> {
        let mut config = config.unwrap_or_default();

        // 从环境变量检测网络类型
        if let Ok(rpc_url) = std::env::var("RPC_URL") {
            if rpc_url.contains("devnet") {
                config.network = "devnet".to_string();
            } else if rpc_url.contains("testnet") {
                config.network = "testnet".to_string();
            } else {
                config.network = "mainnet".to_string();
            }
        }

        info!("🌐 Metaplex服务初始化，网络环境: {}", config.network);

        let client = Client::builder().timeout(Duration::from_secs(config.timeout_seconds)).build()?;

        // 创建Solana RPC客户端用于链上查询
        let rpc_client = if let Ok(rpc_url) = std::env::var("RPC_URL") {
            info!("🔗 连接到Solana RPC: {}", rpc_url);
            Some(RpcClient::new(rpc_url))
        } else {
            warn!("⚠️ 未找到RPC_URL环境变量，链上元数据查询将被跳过");
            None
        };

        Ok(Self {
            client,
            config,
            cache: HashMap::new(),
            rpc_client,
        })
    }

    /// 获取单个代币的元数据
    pub async fn get_token_metadata(&mut self, mint_address: &str) -> Result<Option<TokenMetadata>> {
        // 检查缓存
        if let Some(metadata) = self.cache.get(mint_address) {
            info!("📦 从缓存获取代币元数据: {}", mint_address);
            return Ok(Some(metadata.clone()));
        }

        info!("🔍 获取代币元数据: {}", mint_address);

        // 尝试从多个来源获取元数据
        let metadata = self.fetch_metadata_with_fallback(mint_address).await?;

        if let Some(ref meta) = metadata {
            // 缓存结果
            self.cache.insert(mint_address.to_string(), meta.clone());
            info!("✅ 成功获取代币元数据: {} - {}", mint_address, meta.symbol.as_deref().unwrap_or("Unknown"));
        } else {
            warn!("⚠️ 未找到代币元数据: {}", mint_address);
        }

        Ok(metadata)
    }

    /// 批量获取代币元数据
    pub async fn get_tokens_metadata(&mut self, mint_addresses: &[String]) -> Result<HashMap<String, TokenMetadata>> {
        let mut results = HashMap::new();
        let mut pending_addresses = Vec::new();

        // 首先检查缓存
        for address in mint_addresses {
            if let Some(metadata) = self.cache.get(address) {
                results.insert(address.clone(), metadata.clone());
            } else {
                pending_addresses.push(address.clone());
            }
        }

        if pending_addresses.is_empty() {
            info!("📦 所有代币元数据都在缓存中");
            return Ok(results);
        }

        info!("🔍 批量获取 {} 个代币的元数据", pending_addresses.len());

        // 分批处理待获取的地址
        for chunk in pending_addresses.chunks(self.config.batch_size) {
            let batch_results = self.fetch_batch_metadata(chunk).await?;

            for (address, metadata) in batch_results {
                // 缓存结果
                self.cache.insert(address.clone(), metadata.clone());
                results.insert(address, metadata);
            }

            // 避免请求过于频繁
            if chunk.len() == self.config.batch_size {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }

        info!("✅ 批量获取代币元数据完成，共 {} 个", results.len());
        Ok(results)
    }

    /// 从多个来源获取元数据（带回退机制）
    async fn fetch_metadata_with_fallback(&self, mint_address: &str) -> Result<Option<TokenMetadata>> {
        // 先尝试直接获取链上元数据（如果有的话）
        if let Ok(Some(metadata)) = self.fetch_onchain_metadata(mint_address).await {
            return Ok(Some(metadata));
        } else {
            warn!("🔍 从链上获取数据失败");
        }

        // 尝试从 Jupiter Token List 获取
        if let Ok(Some(metadata)) = self.fetch_from_jupiter_token_list(mint_address).await {
            return Ok(Some(metadata));
        } else {
            warn!("🔍 从Jupiter Token List获取数据失败");
        }

        // 尝试从 Solana Token List 获取
        if let Ok(Some(metadata)) = self.fetch_from_solana_token_list(mint_address).await {
            return Ok(Some(metadata));
        } else {
            warn!("🔍 从Solana Token List获取数据失败");
        }

        // 如果都失败了，返回基本信息
        Ok(Some(self.create_fallback_metadata(mint_address)))
    }

    /// 从 Jupiter Token List 获取元数据
    async fn fetch_from_jupiter_token_list(&self, mint_address: &str) -> Result<Option<TokenMetadata>> {
        // Jupiter API 包含所有网络的代币，不区分网络环境
        let url = format!("https://token.jup.ag/strict");

        info!("🔍 从Jupiter Token List获取数据: {}， mint_address: {}", url, mint_address);

        #[derive(Deserialize)]
        struct JupiterToken {
            address: String,
            symbol: String,
            name: String,
            #[serde(rename = "logoURI")]
            logo_uri: Option<String>,
            tags: Option<Vec<String>>,
        }

        let response: Vec<JupiterToken> = self.client.get(&url).send().await?.json().await?;

        for token in response {
            if token.address == mint_address {
                return Ok(Some(TokenMetadata {
                    address: token.address,
                    symbol: Some(token.symbol),
                    name: Some(token.name),
                    logo_uri: token.logo_uri,
                    description: None,
                    external_url: None,
                    attributes: None,
                    tags: token.tags.unwrap_or_default(),
                }));
            }
        }

        Ok(None)
    }

    /// 从 Solana Token List 获取元数据
    async fn fetch_from_solana_token_list(&self, mint_address: &str) -> Result<Option<TokenMetadata>> {
        // 根据网络环境选择对应的token list
        // 注意：目前Solana Labs只提供mainnet版本，devnet/testnet会回退到mainnet列表
        let token_list_filename = match self.config.network.as_str() {
            "devnet" | "testnet" => {
                info!("⚠️ {}环境回退使用mainnet token list", self.config.network);
                "solana.tokenlist.json"
            }
            _ => "solana.tokenlist.json", // mainnet 默认
        };

        let url = format!("https://raw.githubusercontent.com/solana-labs/token-list/main/src/tokens/{}", token_list_filename);

        info!("🔍 从Solana Token List获取数据: {} 网络: {}, mint_address: {}", url, self.config.network, mint_address);

        #[derive(Deserialize)]
        struct TokenList {
            tokens: Vec<SolanaToken>,
        }

        #[derive(Deserialize)]
        struct SolanaToken {
            address: String,
            symbol: String,
            name: String,
            #[serde(rename = "logoURI")]
            logo_uri: Option<String>,
            tags: Option<Vec<String>>,
        }

        let response: TokenList = self.client.get(&url).send().await?.json().await?;

        for token in response.tokens {
            if token.address == mint_address {
                return Ok(Some(TokenMetadata {
                    address: token.address,
                    symbol: Some(token.symbol),
                    name: Some(token.name),
                    logo_uri: token.logo_uri,
                    description: None,
                    external_url: None,
                    attributes: None,
                    tags: token.tags.unwrap_or_default(),
                }));
            }
        }

        Ok(None)
    }

    /// 检测代币是否为Token-2022标准
    async fn is_token_2022(&self, rpc_client: &RpcClient, mint_pubkey: &Pubkey) -> Result<bool> {
        // 获取mint账户信息
        let account = match rpc_client.get_account(mint_pubkey) {
            Ok(account) => account,
            Err(_) => return Ok(false),
        };

        // 检查所有者是否为Token-2022程序
        let token_2022_program_id = TOKEN_2022_PROGRAM_ID.parse::<Pubkey>().map_err(|e| anyhow::anyhow!("解析Token-2022程序ID失败: {}", e))?;

        Ok(account.owner == token_2022_program_id)
    }

    /// 从Token-2022原生元数据扩展获取元数据
    async fn fetch_token_2022_metadata(&self, rpc_client: &RpcClient, mint_pubkey: &Pubkey) -> Result<Option<TokenMetadata>> {
        info!("🔗 尝试从Token-2022原生元数据扩展获取元数据: {}", mint_pubkey);

        // 获取mint账户数据
        let account_data = match rpc_client.get_account_data(mint_pubkey) {
            Ok(data) => data,
            Err(e) => {
                info!("❌ 获取Token-2022 mint账户失败: {}", e);
                return Ok(None);
            }
        };

        // 尝试解析为Token-2022 mint账户
        let mint_state = match StateWithExtensions::<Mint2022>::unpack(&account_data) {
            Ok(state) => state,
            Err(e) => {
                debug!("❌ 解析Token-2022 mint状态失败: {}", e);
                return Ok(None);
            }
        };

        // 检查是否有元数据指针扩展
        let metadata_pointer = match mint_state.get_extension::<MetadataPointer>() {
            Ok(pointer) => pointer,
            Err(_) => {
                debug!("⚠️ Token-2022 mint没有元数据指针扩展");
                return Ok(None);
            }
        };

        // 获取元数据地址
        let metadata_address = match metadata_pointer.metadata_address.into() {
            Some(addr) => addr,
            None => {
                debug!("⚠️ Token-2022元数据指针为空");
                return Ok(None);
            }
        };

        info!("🔍 Token-2022元数据地址: {}", metadata_address);

        // 如果元数据存储在mint账户本身
        if metadata_address == *mint_pubkey {
            // 尝试从mint账户的扩展中获取元数据
            if let Ok(metadata) = mint_state.get_variable_len_extension::<Token2022Metadata>() {
                return Ok(Some(self.convert_token_2022_metadata(mint_pubkey, &metadata)));
            }
        } else {
            // 从单独的元数据账户获取数据
            let metadata_account_data = match rpc_client.get_account_data(&metadata_address) {
                Ok(data) => data,
                Err(e) => {
                    info!("❌ 获取Token-2022元数据账户失败: {}", e);
                    return Ok(None);
                }
            };

            // 尝试解析元数据
            if let Ok(metadata) = Token2022Metadata::try_from_slice(&metadata_account_data) {
                return Ok(Some(self.convert_token_2022_metadata(mint_pubkey, &metadata)));
            }
        }

        Ok(None)
    }

    /// 将Token-2022元数据转换为TokenMetadata结构
    fn convert_token_2022_metadata(&self, mint_pubkey: &Pubkey, metadata: &Token2022Metadata) -> TokenMetadata {
        let name = metadata.name.clone();
        let symbol = metadata.symbol.clone();
        let uri = metadata.uri.clone();

        // 查找其他可用字段
        let mut description = None;
        let mut attributes = Vec::new();

        // 检查其他字段
        for (key, value) in &metadata.additional_metadata {
            match key.as_str() {
                "description" => description = Some(value.clone()),
                _ => {
                    attributes.push(TokenAttribute {
                        trait_type: key.clone(),
                        value: value.clone(),
                    });
                }
            }
        }

        let mut tags = vec!["token-2022".to_string(), "native-metadata".to_string()];

        if !uri.is_empty() {
            tags.push("metadata-uri".to_string());
        }

        TokenMetadata {
            address: mint_pubkey.to_string(),
            symbol: if symbol.is_empty() { None } else { Some(symbol) },
            name: if name.is_empty() { None } else { Some(name) },
            logo_uri: if uri.is_empty() { None } else { Some(uri) },
            description,
            external_url: None,
            attributes: if attributes.is_empty() { None } else { Some(attributes) },
            tags,
        }
    }

    /// 获取链上元数据
    async fn fetch_onchain_metadata(&self, mint_address: &str) -> Result<Option<TokenMetadata>> {
        info!("🔗 尝试从链上获取元数据: {}", mint_address);

        // 检查是否有RPC客户端
        let rpc_client = match &self.rpc_client {
            Some(client) => client,
            None => {
                info!("⚠️ 没有RPC客户端，跳过链上查询");
                return Ok(None);
            }
        };

        // 解析mint地址
        let mint_pubkey = match mint_address.parse::<Pubkey>() {
            Ok(pubkey) => pubkey,
            Err(e) => {
                info!("❌ 无效的mint地址: {} - {}", mint_address, e);
                return Ok(None);
            }
        };

        // 优先检查是否为Token-2022标准
        match self.is_token_2022(&rpc_client, &mint_pubkey).await {
            Ok(true) => {
                info!("✅ 检测到Token-2022代币: {}", mint_address);
                // 尝试从Token-2022原生元数据扩展获取
                match self.fetch_token_2022_metadata(&rpc_client, &mint_pubkey).await {
                    Ok(Some(token_metadata)) => {
                        info!("✅ 成功从Token-2022原生元数据获取元数据: {}", mint_address);
                        return Ok(Some(token_metadata));
                    }
                    Ok(None) => {
                        info!("⚠️ Token-2022代币没有原生元数据扩展，尝试Metaplex");
                    }
                    Err(e) => {
                        info!("❌ 获取Token-2022元数据失败: {} - {}", mint_address, e);
                    }
                }
            }
            Ok(false) => {
                debug!("⚠️ 不是Token-2022代币，使用标准Token程序: {}", mint_address);
            }
            Err(e) => {
                debug!("❌ 检测Token-2022失败: {} - {}", mint_address, e);
            }
        }

        // 回退到Metaplex元数据获取（适用于标准Token和没有原生元数据的Token-2022）
        match self.fetch_metaplex_metadata(&rpc_client, &mint_pubkey).await {
            Ok(Some(token_metadata)) => {
                info!("✅ 成功从Metaplex获取元数据: {}", mint_address);
                return Ok(Some(token_metadata));
            }
            Ok(None) => {
                debug!("⚠️ 无法获取Metaplex元数据: {}", mint_address);
            }
            Err(e) => {
                debug!("❌ 获取Metaplex元数据失败: {} - {}", mint_address, e);
            }
        }

        Ok(None)
    }

    /// 获取SPL Token基本信息
    async fn _fetch_spl_token_info(&self, rpc_client: &RpcClient, mint_pubkey: &Pubkey) -> Result<Option<TokenMetadata>> {
        use spl_token::state::Mint;

        // 获取mint账户信息
        let account_data = rpc_client.get_account_data(mint_pubkey).map_err(|e| anyhow::anyhow!("获取mint账户失败: {}", e))?;

        // 解析mint账户数据
        let mint_info = Mint::unpack(&account_data).map_err(|e| anyhow::anyhow!("解析mint账户失败: {}", e))?;

        // 创建基本的代币信息
        let token_metadata = TokenMetadata {
            address: mint_pubkey.to_string(),
            symbol: None, // SPL Token不包含symbol信息
            name: None,   // SPL Token不包含name信息
            logo_uri: None,
            description: Some(format!("SPL Token with {} decimals", mint_info.decimals)),
            external_url: None,
            attributes: Some(vec![
                TokenAttribute {
                    trait_type: "decimals".to_string(),
                    value: mint_info.decimals.to_string(),
                },
                TokenAttribute {
                    trait_type: "supply".to_string(),
                    value: mint_info.supply.to_string(),
                },
                TokenAttribute {
                    trait_type: "is_initialized".to_string(),
                    value: mint_info.is_initialized.to_string(),
                },
            ]),
            tags: vec!["spl-token".to_string()],
        };

        Ok(Some(token_metadata))
    }

    /// 获取Metaplex元数据
    async fn fetch_metaplex_metadata(&self, rpc_client: &RpcClient, mint_pubkey: &Pubkey) -> Result<Option<TokenMetadata>> {
        // 计算元数据账户地址
        let metadata_pubkey = self.find_metadata_pda(mint_pubkey)?;

        info!("🔍 查询Metaplex元数据账户: {}", metadata_pubkey);

        // 获取账户数据
        let account_data = match rpc_client.get_account_data(&metadata_pubkey) {
            Ok(data) => data,
            Err(_) => {
                info!("⚠️ Metaplex元数据账户不存在: {}", metadata_pubkey);
                return Ok(None);
            }
        };

        // 尝试解析元数据（使用增强的异步解析）
        match self.parse_metadata_simple(&account_data).await {
            Ok(Some(mut token_metadata)) => {
                // 设置mint地址
                token_metadata.address = mint_pubkey.to_string();
                Ok(Some(token_metadata))
            }
            Ok(None) => {
                info!("⚠️ 无法解析Metaplex元数据");
                Ok(None)
            }
            Err(e) => {
                info!("❌ 解析Metaplex元数据失败: {}", e);
                Ok(None)
            }
        }
    }

    /// 增强的元数据解析（不依赖外部库）- 支持解析更多字段
    async fn parse_metadata_simple(&self, data: &[u8]) -> Result<Option<TokenMetadata>> {
        // 这是一个增强的解析器，尝试从raw数据中提取更多字段信息
        // 实际的Metaplex元数据结构更复杂，这里尽力解析主要字段

        if data.len() < 200 {
            debug!("🔍 数据长度不足 {} bytes，跳过解析", data.len());
            return Ok(None);
        }

        // 跳过前面的固定字段，尝试查找字符串数据
        let mut offset = 1 + 32 + 32; // key + update_authority + mint

        if offset + 16 > data.len() {
            return Ok(None);
        }

        // 解析name字段
        let (name, new_offset) = match self.parse_string_field(data, offset, 200, "name") {
            Ok((value, next_offset)) => (value, next_offset),
            Err(_) => return Ok(None),
        };
        offset = new_offset;

        // 解析symbol字段
        let (symbol, new_offset) = match self.parse_string_field(data, offset, 50, "symbol") {
            Ok((value, next_offset)) => (value, next_offset),
            Err(_) => return Ok(None),
        };
        offset = new_offset;

        // 解析uri字段
        let (uri, new_offset) = match self.parse_string_field(data, offset, 500, "uri") {
            Ok((value, next_offset)) => (value, next_offset),
            Err(_) => {
                // URI解析失败不会导致整个解析失败
                debug!("⚠️ 无法解析URI字段，继续处理其他字段");
                (None, offset)
            }
        };
        offset = new_offset;

        // 尝试解析seller_fee_basis_points (u16)
        let seller_fee_basis_points = if offset + 2 <= data.len() {
            u16::from_le_bytes([data[offset], data[offset + 1]])
        } else {
            0
        };

        // 跳过seller_fee_basis_points
        // if offset + 2 <= data.len() {
        //     offset += 2;
        // }

        // 创建基础的链上元数据
        let mut chain_metadata = TokenMetadata {
            address: "".to_string(), // 将在调用者中设置
            symbol: symbol,
            name: name,
            logo_uri: uri.clone(),
            description: if seller_fee_basis_points > 0 {
                Some(format!("Metaplex NFT with {}% royalty", seller_fee_basis_points as f64 / 100.0))
            } else {
                Some("Token with Metaplex metadata".to_string())
            },
            external_url: None,
            attributes: if seller_fee_basis_points > 0 {
                Some(vec![
                    TokenAttribute {
                        trait_type: "seller_fee_basis_points".to_string(),
                        value: seller_fee_basis_points.to_string(),
                    },
                    TokenAttribute {
                        trait_type: "royalty_percentage".to_string(),
                        value: format!("{:.2}%", seller_fee_basis_points as f64 / 100.0),
                    },
                ])
            } else {
                None
            },
            tags: {
                let mut tags = vec!["metaplex".to_string()];

                // 根据解析到的信息添加更多标签
                if seller_fee_basis_points > 0 {
                    tags.push("royalty".to_string());
                }

                if uri.is_some() {
                    tags.push("metadata-uri".to_string());
                }

                tags
            },
        };

        // 如果有URI，尝试获取更详细的元数据
        if let Some(ref uri_str) = uri {
            match self.fetch_uri_metadata(uri_str).await {
                Ok(Some(uri_metadata)) => {
                    info!("🔗 成功从URI获取扩展元数据");
                    chain_metadata = self.merge_metadata(chain_metadata, Some(uri_metadata));
                }
                Ok(None) => {
                    debug!("⚠️ 无法从URI获取扩展元数据，使用链上数据");
                }
                Err(e) => {
                    debug!("❌ 获取URI元数据时发生错误: {}", e);
                }
            }
        }

        // 检查是否解析到有效数据
        if chain_metadata.name.is_none() && chain_metadata.symbol.is_none() && uri.is_none() {
            debug!("⚠️ 未解析到任何有效字段");
            return Ok(None);
        }

        info!(
            "✅ 成功解析Metaplex元数据: name={:?}, symbol={:?}, uri={:?}, royalty={}%",
            chain_metadata.name,
            chain_metadata.symbol,
            uri,
            seller_fee_basis_points as f64 / 100.0
        );

        Ok(Some(chain_metadata))
    }

    /// 辅助函数：解析字符串字段
    fn parse_string_field(&self, data: &[u8], offset: usize, max_len: usize, field_name: &str) -> Result<(Option<String>, usize)> {
        if offset + 4 > data.len() {
            return Err(anyhow::anyhow!("数据不足以读取{}长度", field_name));
        }

        // 读取字符串长度
        let str_len = u32::from_le_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]]) as usize;

        let mut new_offset = offset + 4;

        // 验证长度合理性
        if str_len > max_len || new_offset + str_len > data.len() {
            debug!("⚠️ {}字段长度异常: {} (max: {})", field_name, str_len, max_len);
            return Ok((None, new_offset));
        }

        if str_len == 0 {
            return Ok((None, new_offset));
        }

        // 读取字符串内容
        let str_content = match String::from_utf8(data[new_offset..new_offset + str_len].to_vec()) {
            Ok(s) => s.trim_end_matches('\0').to_string(),
            Err(e) => {
                debug!("⚠️ {}字段UTF-8解码失败: {}", field_name, e);
                return Ok((None, new_offset + str_len));
            }
        };

        new_offset += str_len;

        let result = if str_content.is_empty() {
            None
        } else {
            debug!("✅ 解析{}字段: {}", field_name, str_content);
            Some(str_content)
        };

        Ok((result, new_offset))
    }

    /// 从URI获取扩展元数据（JSON格式）
    async fn fetch_uri_metadata(&self, uri: &str) -> Result<Option<UriMetadata>> {
        if !uri.starts_with("http") {
            debug!("⚠️ URI不是HTTP格式，跳过: {}", uri);
            return Ok(None);
        }

        // 设置较短的超时时间，避免阻塞
        let client = Client::builder().timeout(Duration::from_secs(5)).build()?;

        debug!("🔍 尝试获取URI元数据: {}", uri);

        match client.get(uri).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    match response.json::<UriMetadata>().await {
                        Ok(metadata) => {
                            info!("✅ 成功获取URI元数据: {}", uri);
                            Ok(Some(metadata))
                        }
                        Err(e) => {
                            debug!("⚠️ 解析URI元数据JSON失败: {} - {}", uri, e);
                            Ok(None)
                        }
                    }
                } else {
                    debug!("⚠️ URI元数据请求失败: {} - {}", uri, response.status());
                    Ok(None)
                }
            }
            Err(e) => {
                debug!("⚠️ 无法访问URI: {} - {}", uri, e);
                Ok(None)
            }
        }
    }

    /// 合并链上元数据和URI元数据
    fn merge_metadata(&self, chain_metadata: TokenMetadata, uri_metadata: Option<UriMetadata>) -> TokenMetadata {
        if let Some(uri_meta) = uri_metadata {
            let mut tags = chain_metadata.tags;

            // 检查动画URL
            if uri_meta.animation_url.is_some() {
                tags.push("animated".to_string());
            }

            // 检查属性
            if let Some(ref attrs) = uri_meta.attributes {
                if !attrs.is_empty() {
                    tags.push("rich-metadata".to_string());
                }
            }

            TokenMetadata {
                address: chain_metadata.address,
                symbol: chain_metadata.symbol.or(uri_meta.symbol),
                name: chain_metadata.name.or(uri_meta.name),
                logo_uri: chain_metadata.logo_uri.or(uri_meta.image),
                description: chain_metadata.description.or(uri_meta.description),
                external_url: chain_metadata.external_url.or(uri_meta.external_url),
                attributes: chain_metadata.attributes.or(uri_meta.attributes),
                tags,
            }
        } else {
            chain_metadata
        }
    }

    /// 计算元数据程序派生地址(PDA)
    fn find_metadata_pda(&self, mint: &Pubkey) -> Result<Pubkey> {
        let metadata_program_id = METADATA_PROGRAM_ID.parse::<Pubkey>().map_err(|e| anyhow::anyhow!("解析元数据程序ID失败: {}", e))?;

        // 计算元数据账户的PDA
        let seeds = &["metadata".as_bytes(), metadata_program_id.as_ref(), mint.as_ref()];

        let (metadata_pubkey, _bump) = Pubkey::find_program_address(seeds, &metadata_program_id);
        Ok(metadata_pubkey)
    }

    /// 解析元数据账户数据
    fn _parse_metadata_account(&self, data: &[u8]) -> Result<SimpleData> {
        // 尝试反序列化元数据账户
        let metadata = SimpleMetadata::try_from_slice(data).map_err(|e| anyhow::anyhow!("反序列化元数据失败: {}", e))?;

        Ok(metadata.data)
    }

    /// 批量获取元数据
    async fn fetch_batch_metadata(&self, mint_addresses: &[String]) -> Result<HashMap<String, TokenMetadata>> {
        let mut results = HashMap::new();

        // 简单的并发处理
        let futures: Vec<_> = mint_addresses.iter().map(|address| self.fetch_metadata_with_fallback(address)).collect();

        let responses = futures::future::join_all(futures).await;

        for (i, response) in responses.into_iter().enumerate() {
            if let Ok(Some(metadata)) = response {
                results.insert(mint_addresses[i].clone(), metadata);
            }
        }

        Ok(results)
    }

    /// 创建回退元数据
    fn create_fallback_metadata(&self, mint_address: &str) -> TokenMetadata {
        // 对于一些知名代币，提供硬编码的信息
        match mint_address {
            "So11111111111111111111111111111111111111112" => TokenMetadata {
                address: mint_address.to_string(),
                symbol: Some("WSOL".to_string()),
                name: Some("Wrapped SOL".to_string()),
                logo_uri: Some("https://raw.githubusercontent.com/solana-labs/token-list/main/assets/mainnet/So11111111111111111111111111111111111111112/logo.png".to_string()),
                description: Some("Wrapped Solana".to_string()),
                external_url: Some("https://solana.com".to_string()),
                attributes: None,
                tags: vec![],
            },
            "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v" => TokenMetadata {
                address: mint_address.to_string(),
                symbol: Some("USDC".to_string()),
                name: Some("USD Coin".to_string()),
                logo_uri: Some("https://raw.githubusercontent.com/solana-labs/token-list/main/assets/mainnet/EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v/logo.png".to_string()),
                description: Some("USD Coin".to_string()),
                external_url: Some("https://www.centre.io".to_string()),
                attributes: None,
                tags: vec!["hasFreeze".to_string()],
            },
            "4k3Dyjzvzp8eMZWUXbBCjEvwSkkk59S5iCNLY3QrkX6R" => TokenMetadata {
                address: mint_address.to_string(),
                symbol: Some("RAY".to_string()),
                name: Some("Raydium".to_string()),
                logo_uri: Some("https://img-v1.raydium.io/icon/4k3Dyjzvzp8eMZWUXbBCjEvwSkkk59S5iCNLY3QrkX6R.png".to_string()),
                description: Some("Raydium Protocol Token".to_string()),
                external_url: Some("https://raydium.io".to_string()),
                attributes: None,
                tags: vec![],
            },
            _ => {
                info!("🔍 创建为空的数据: {}", mint_address);
                TokenMetadata {
                    address: mint_address.to_string(),
                    symbol: None,
                    name: None,
                    logo_uri: None,
                    description: Some("Token without metadata".to_string()),
                    external_url: None,
                    attributes: None,
                    tags: vec![],
                }
            }
        }
    }

    /// 清除缓存
    pub fn clear_cache(&mut self) {
        self.cache.clear();
        info!("🗑️ 已清除代币元数据缓存");
    }

    /// 获取缓存统计
    pub fn get_cache_stats(&self) -> (usize, usize) {
        (self.cache.len(), self.cache.capacity())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use spl_pod::optional_keys::OptionalNonZeroPubkey;
    #[tokio::test]
    async fn test_enhanced_metadata_parsing() {
        let service = MetaplexService::new(None).unwrap();

        // 创建一个模拟的Metaplex元数据账户数据
        // 这个测试展示了增强解析器的能力
        let mut mock_data = vec![0u8; 300];

        // 设置固定字段
        mock_data[0] = 4; // key: Metaplex metadata账户类型
                          // update_authority (32字节) 和 mint (32字节) 已经是零值

        let mut offset = 1 + 32 + 32; // 跳过key, update_authority, mint

        // 写入name字段
        let name = "Enhanced Test Token";
        let name_bytes = name.as_bytes();
        let name_len = name_bytes.len() as u32;
        mock_data[offset..offset + 4].copy_from_slice(&name_len.to_le_bytes());
        offset += 4;
        mock_data[offset..offset + name_bytes.len()].copy_from_slice(name_bytes);
        offset += name_bytes.len();

        // 写入symbol字段
        let symbol = "ETT";
        let symbol_bytes = symbol.as_bytes();
        let symbol_len = symbol_bytes.len() as u32;
        mock_data[offset..offset + 4].copy_from_slice(&symbol_len.to_le_bytes());
        offset += 4;
        mock_data[offset..offset + symbol_bytes.len()].copy_from_slice(symbol_bytes);
        offset += symbol_bytes.len();

        // 写入uri字段
        let uri = "https://example.com/metadata.json";
        let uri_bytes = uri.as_bytes();
        let uri_len = uri_bytes.len() as u32;
        mock_data[offset..offset + 4].copy_from_slice(&uri_len.to_le_bytes());
        offset += 4;
        mock_data[offset..offset + uri_bytes.len()].copy_from_slice(uri_bytes);
        offset += uri_bytes.len();

        // 写入seller_fee_basis_points (5% = 500)
        let royalty: u16 = 500;
        mock_data[offset..offset + 2].copy_from_slice(&royalty.to_le_bytes());

        // 测试解析
        let result = service.parse_metadata_simple(&mock_data).await.unwrap();

        assert!(result.is_some());
        let metadata = result.unwrap();

        assert_eq!(metadata.name, Some("Enhanced Test Token".to_string()));
        assert_eq!(metadata.symbol, Some("ETT".to_string()));
        assert_eq!(metadata.logo_uri, Some("https://example.com/metadata.json".to_string()));

        // 检查描述内容应该包含royalty信息
        let description = metadata.description.as_ref().unwrap();
        assert!(description.contains("5% royalty"));

        assert!(metadata.tags.contains(&"metaplex".to_string()));
        assert!(metadata.tags.contains(&"royalty".to_string()));
        assert!(metadata.tags.contains(&"metadata-uri".to_string()));

        // 检查属性
        let attributes = metadata.attributes.unwrap();
        assert_eq!(attributes.len(), 2);
        assert_eq!(attributes[0].trait_type, "seller_fee_basis_points");
        assert_eq!(attributes[0].value, "500");
        assert_eq!(attributes[1].trait_type, "royalty_percentage");
        assert_eq!(attributes[1].value, "5.00%");
    }

    #[tokio::test]
    async fn test_parse_string_field() {
        let service = MetaplexService::new(None).unwrap();

        // 测试正常的字符串解析
        let mut data = vec![0u8; 20];
        let test_string = "Hello";
        let test_len = test_string.len() as u32;

        data[0..4].copy_from_slice(&test_len.to_le_bytes());
        data[4..4 + test_string.len()].copy_from_slice(test_string.as_bytes());

        let (result, new_offset) = service.parse_string_field(&data, 0, 10, "test").unwrap();
        assert_eq!(result, Some("Hello".to_string()));
        assert_eq!(new_offset, 4 + test_string.len());

        // 测试空字符串
        data[0..4].copy_from_slice(&0u32.to_le_bytes());
        let (result, new_offset) = service.parse_string_field(&data, 0, 10, "test").unwrap();
        assert_eq!(result, None);
        assert_eq!(new_offset, 4);
    }

    #[tokio::test]
    async fn test_fallback_metadata() {
        let service = MetaplexService::new(None).unwrap();

        // 测试 WSOL
        let wsol_metadata = service.create_fallback_metadata("So11111111111111111111111111111111111111112");
        assert_eq!(wsol_metadata.symbol, Some("WSOL".to_string()));
        assert_eq!(wsol_metadata.name, Some("Wrapped SOL".to_string()));

        // 测试 USDC
        let usdc_metadata = service.create_fallback_metadata("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
        assert_eq!(usdc_metadata.symbol, Some("USDC".to_string()));
        assert_eq!(usdc_metadata.name, Some("USD Coin".to_string()));

        // 测试未知代币
        let unknown_metadata = service.create_fallback_metadata("UnknownMintAddress123456789");
        assert_eq!(unknown_metadata.symbol, None);
        assert_eq!(unknown_metadata.name, None);
    }

    #[test]
    fn test_config_default() {
        let config = MetaplexConfig::default();
        assert_eq!(config.base_url, "https://api.metaplex.com");
        assert_eq!(config.timeout_seconds, 30);
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.batch_size, 50);
        assert_eq!(config.network, "mainnet");
    }

    #[test]
    fn test_network_detection() {
        // 测试devnet网络检测
        std::env::set_var("RPC_URL", "https://api.devnet.solana.com");
        let service = MetaplexService::new(None).unwrap();
        assert_eq!(service.config.network, "devnet");

        // 测试testnet网络检测
        std::env::set_var("RPC_URL", "https://api.testnet.solana.com");
        let service = MetaplexService::new(None).unwrap();
        assert_eq!(service.config.network, "testnet");

        // 测试mainnet网络检测
        std::env::set_var("RPC_URL", "https://api.mainnet-beta.solana.com");
        let service = MetaplexService::new(None).unwrap();
        assert_eq!(service.config.network, "mainnet");

        // 清理环境变量
        std::env::remove_var("RPC_URL");
    }

    #[test]
    fn test_token_2022_program_id_parsing() {
        // 测试Token-2022程序ID解析
        let program_id = TOKEN_2022_PROGRAM_ID.parse::<Pubkey>();
        assert!(program_id.is_ok());
        assert_eq!(program_id.unwrap().to_string(), "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb");
    }

    #[test]
    fn test_convert_token_2022_metadata() {
        let service = MetaplexService::new(None).unwrap();
        let mint_pubkey = Pubkey::new_unique();

        // 模拟Token-2022元数据
        let mut additional_metadata = Vec::new();
        additional_metadata.push(("description".to_string(), "Test token description".to_string()));
        additional_metadata.push(("website".to_string(), "https://example.com".to_string()));

        let mock_metadata = Token2022Metadata {
            mint: mint_pubkey,
            name: "Test Token 2022".to_string(),
            symbol: "TT22".to_string(),
            uri: "https://example.com/metadata.json".to_string(),
            additional_metadata,
            update_authority: OptionalNonZeroPubkey::try_from(Some(mint_pubkey)).unwrap(),
        };

        let result = service.convert_token_2022_metadata(&mint_pubkey, &mock_metadata);

        assert_eq!(result.name, Some("Test Token 2022".to_string()));
        assert_eq!(result.symbol, Some("TT22".to_string()));
        assert_eq!(result.logo_uri, Some("https://example.com/metadata.json".to_string()));
        assert_eq!(result.description, Some("Test token description".to_string()));
        assert!(result.tags.contains(&"token-2022".to_string()));
        assert!(result.tags.contains(&"native-metadata".to_string()));
        assert!(result.tags.contains(&"metadata-uri".to_string()));

        // 检查属性
        let attributes = result.attributes.unwrap();
        assert_eq!(attributes.len(), 1);
        assert_eq!(attributes[0].trait_type, "website");
        assert_eq!(attributes[0].value, "https://example.com");
    }

    #[test]
    fn test_convert_empty_token_2022_metadata() {
        let service = MetaplexService::new(None).unwrap();
        let mint_pubkey = Pubkey::new_unique();

        // 模拟空的Token-2022元数据
        let mock_metadata = Token2022Metadata {
            mint: mint_pubkey,
            name: "".to_string(),
            symbol: "".to_string(),
            uri: "".to_string(),
            additional_metadata: Vec::new(),
            update_authority: OptionalNonZeroPubkey::try_from(Some(mint_pubkey)).unwrap(),
        };

        let result = service.convert_token_2022_metadata(&mint_pubkey, &mock_metadata);

        assert_eq!(result.name, None);
        assert_eq!(result.symbol, None);
        assert_eq!(result.logo_uri, None);
        assert!(result.tags.contains(&"token-2022".to_string()));
        assert!(result.tags.contains(&"native-metadata".to_string()));
        assert!(!result.tags.contains(&"metadata-uri".to_string()));
        assert_eq!(result.attributes, None);
    }
}
