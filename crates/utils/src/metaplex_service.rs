//! Metaplex API 集成服务
//!
//! 负责从 Metaplex API 获取代币元数据信息，包括名称、符号、Logo URI 等

use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use tracing::{debug, info, warn};

// 导入元数据相关类型
use crate::{ExternalTokenMetadata, TokenAttribute, TokenMetadata, TokenMetadataProvider};

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

/// URI元数据结构（从链上URI获取的JSON数据）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UriMetadata {
    /// 代币名称
    #[serde(rename = "tokenName")]
    pub token_name: Option<String>,
    /// 代币符号
    #[serde(rename = "tokenSymbol")]
    pub token_symbol: Option<String>,
    /// 描述
    pub description: Option<String>,
    /// 头像URL（Logo）
    #[serde(rename = "avatarUrl")]
    pub avatar_url: Option<String>,
    /// 社交链接
    #[serde(rename = "socialLinks")]
    pub social_links: Option<SocialLinks>,
    /// 白名单信息
    pub whitelist: Option<WhitelistInfo>,
    /// 购买限制
    #[serde(rename = "purchaseLimit")]
    pub purchase_limit: Option<String>,
    /// 众筹信息
    pub crowdfunding: Option<CrowdfundingInfo>,
}

/// 社交链接结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocialLinks {
    /// Twitter链接
    pub twitter: Option<String>,
    /// Telegram链接
    pub telegram: Option<String>,
    /// 网站链接
    pub website: Option<String>,
}

/// 白名单信息结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhitelistInfo {
    /// 是否启用白名单
    pub enabled: bool,
    /// 白名单地址列表
    pub addresses: Vec<String>,
}

/// 众筹信息结构
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CrowdfundingInfo {
    /// 开始时间 - 可以是字符串或数字格式
    #[serde(rename = "startTime", deserialize_with = "deserialize_flexible_timestamp")]
    pub start_time: Option<String>,
    /// 结束时间 - 可以是字符串或数字格式
    #[serde(rename = "endTime", deserialize_with = "deserialize_flexible_timestamp")]
    pub end_time: Option<String>,
    /// 持续时间（秒）
    pub duration: Option<u32>,
}

/// 灵活的时间戳反序列化器 - 支持字符串和数字两种格式
fn deserialize_flexible_timestamp<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Visitor;
    use std::fmt;
    
    struct FlexibleTimestampVisitor;
    
    impl<'de> Visitor<'de> for FlexibleTimestampVisitor {
        type Value = Option<String>;
        
        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a string or number representing a timestamp")
        }
        
        fn visit_none<E>(self) -> Result<Self::Value, E> {
            Ok(None)
        }
        
        fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: Deserializer<'de>,
        {
            deserializer.deserialize_any(FlexibleTimestampValueVisitor)
                .map(Some)
        }
        
        fn visit_unit<E>(self) -> Result<Self::Value, E> {
            Ok(None)
        }
    }
    
    struct FlexibleTimestampValueVisitor;
    
    impl<'de> Visitor<'de> for FlexibleTimestampValueVisitor {
        type Value = String;
        
        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a string or number representing a timestamp")
        }
        
        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E> {
            Ok(value.to_string())
        }
        
        fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
            Ok(value)
        }
        
        fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E> {
            Ok(value.to_string())
        }
        
        fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
            Ok(value.to_string())
        }
        
        fn visit_i32<E>(self, value: i32) -> Result<Self::Value, E> {
            Ok(value.to_string())
        }
        
        fn visit_u32<E>(self, value: u32) -> Result<Self::Value, E> {
            Ok(value.to_string())
        }
        
        fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E> {
            Ok((value as i64).to_string())
        }
        
        fn visit_f32<E>(self, value: f32) -> Result<Self::Value, E> {
            Ok((value as i64).to_string())
        }
    }
    
    deserializer.deserialize_option(FlexibleTimestampVisitor)
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

        let client = Client::builder()
            .timeout(Duration::from_secs(config.timeout_seconds))
            .pool_max_idle_per_host(10) // 每个主机保持10个空闲连接
            .pool_idle_timeout(Duration::from_secs(90)) // 空闲连接保持90秒
            .tcp_keepalive(Duration::from_secs(60)) // TCP keepalive
            .build()?;

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
            info!(
                "✅ 成功获取代币元数据: {} - {}",
                mint_address,
                meta.symbol.as_deref().unwrap_or("Unknown")
            );
        } else {
            warn!("⚠️ 未找到代币元数据: {}", mint_address);
        }

        Ok(metadata)
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

    /// 从URI直接获取代币元数据（公开方法）
    pub async fn fetch_metadata_from_uri(&self, uri: &str) -> Result<Option<UriMetadata>> {
        info!("🔍 从URI获取代币元数据: {}", uri);
        self.fetch_uri_metadata(uri).await
    }

    /// 从 Jupiter Token List 获取元数据
    async fn fetch_from_jupiter_token_list(&self, mint_address: &str) -> Result<Option<TokenMetadata>> {
        // Jupiter API 包含所有网络的代币，不区分网络环境
        let url = format!("https://token.jup.ag/strict");

        info!(
            "🔍 从Jupiter Token List获取数据: {}， mint_address: {}",
            url, mint_address
        );

        #[derive(Deserialize)]
        struct JupiterToken {
            address: String,
            symbol: String,
            name: String,
            #[serde(rename = "logoURI")]
            logo_uri: Option<String>,
            tags: Option<Vec<String>>,
            decimals: u8,
        }

        let response: Vec<JupiterToken> = self.client.get(&url).send().await?.json().await?;

        for token in response {
            if token.address == mint_address {
                return Ok(Some(TokenMetadata {
                    address: token.address,
                    decimals: token.decimals,
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

        let url = format!(
            "https://raw.githubusercontent.com/solana-labs/token-list/main/src/tokens/{}",
            token_list_filename
        );

        info!(
            "🔍 从Solana Token List获取数据: {} 网络: {}, mint_address: {}",
            url, self.config.network, mint_address
        );

        #[derive(Deserialize)]
        struct TokenList {
            tokens: Vec<SolanaToken>,
        }

        #[derive(Deserialize)]
        struct SolanaToken {
            address: String,
            symbol: String,
            name: String,
            decimals: u8,
            #[serde(rename = "logoURI")]
            logo_uri: Option<String>,
            tags: Option<Vec<String>>,
        }

        let response: TokenList = self.client.get(&url).send().await?.json().await?;

        for token in response.tokens {
            if token.address == mint_address {
                return Ok(Some(TokenMetadata {
                    address: token.address,
                    decimals: token.decimals,
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

    /// 获取链上元数据
    pub async fn fetch_onchain_metadata(&self, mint_address: &str) -> Result<Option<TokenMetadata>> {
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

    /// 检测代币是否为Token-2022标准
    async fn is_token_2022(&self, rpc_client: &RpcClient, mint_pubkey: &Pubkey) -> Result<bool> {
        // 获取mint账户信息
        let account = match rpc_client.get_account(mint_pubkey) {
            Ok(account) => account,
            Err(_) => return Ok(false),
        };

        // 检查所有者是否为Token-2022程序
        let token_2022_program_id = TOKEN_2022_PROGRAM_ID
            .parse::<Pubkey>()
            .map_err(|e| anyhow::anyhow!("解析Token-2022程序ID失败: {}", e))?;

        Ok(account.owner == token_2022_program_id)
    }

    /// 从Token-2022原生元数据扩展获取元数据
    async fn fetch_token_2022_metadata(
        &self,
        rpc_client: &RpcClient,
        mint_pubkey: &Pubkey,
    ) -> Result<Option<TokenMetadata>> {
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

        // 获取decimals信息（需要从mint数据中获取）
        let decimals = self.get_mint_decimals_sync(mint_pubkey).unwrap_or(6);

        TokenMetadata {
            address: mint_pubkey.to_string(),
            decimals,
            symbol: if symbol.is_empty() { None } else { Some(symbol) },
            name: if name.is_empty() { None } else { Some(name) },
            logo_uri: if uri.is_empty() { None } else { Some(uri) },
            description,
            external_url: None,
            attributes: if attributes.is_empty() { None } else { Some(attributes) },
            tags,
        }
    }

    /// 同步获取mint的decimals信息（用于内部调用）
    fn get_mint_decimals_sync(&self, mint_pubkey: &Pubkey) -> Result<u8> {
        let rpc_client = self
            .rpc_client
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("RPC客户端未初始化"))?;

        let account_data = rpc_client
            .get_account_data(mint_pubkey)
            .map_err(|e| anyhow::anyhow!("获取mint账户数据失败: {}", e))?;

        let mint =
            spl_token::state::Mint::unpack(&account_data).map_err(|e| anyhow::anyhow!("解析mint数据失败: {}", e))?;

        Ok(mint.decimals)
    }

    /// 获取Metaplex元数据
    async fn fetch_metaplex_metadata(
        &self,
        rpc_client: &RpcClient,
        mint_pubkey: &Pubkey,
    ) -> Result<Option<TokenMetadata>> {
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
                // 设置mint地址和decimals
                token_metadata.address = mint_pubkey.to_string();

                // 获取decimals信息
                if let Ok(decimals) = self.get_mint_decimals_sync(mint_pubkey) {
                    token_metadata.decimals = decimals;
                }

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

    /// 增强的元数据解析
    async fn parse_metadata_simple(&self, data: &[u8]) -> Result<Option<TokenMetadata>> {
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

        // 创建基础的链上元数据
        let mut chain_metadata = TokenMetadata {
            address: "".to_string(), // 将在调用者中设置
            decimals: 6,             // 默认值，将在调用者中覆盖
            symbol: symbol,
            name: name,
            logo_uri: uri.clone(),
            description: if seller_fee_basis_points > 0 {
                Some(format!(
                    "Metaplex NFT with {}% royalty",
                    seller_fee_basis_points as f64 / 100.0
                ))
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
    fn parse_string_field(
        &self,
        data: &[u8],
        offset: usize,
        max_len: usize,
        field_name: &str,
    ) -> Result<(Option<String>, usize)> {
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
            warn!("⚠️ URI不是HTTP格式，跳过: {}", uri);
            return Ok(None);
        }

        info!("🔍 尝试获取URI元数据: {}", uri);

        // 重试机制：失败后重试6次，使用合理的递增延迟
        for attempt in 1..=6 {
            match self.client.get(uri).send().await {
                Ok(response) => {
                    let status = response.status();
                    if status.is_success() {
                        // 首先尝试完整解析
                        match response.json::<UriMetadata>().await {
                            Ok(metadata) => {
                                info!("✅ 成功获取URI元数据: {} (尝试第{}次)", uri, attempt);
                                return Ok(Some(metadata));
                            }
                            Err(json_error) => {
                                // 如果完整解析失败，尝试 fallback 解析
                                debug!("🔄 完整解析失败，尝试 fallback 解析: {}", json_error);
                                
                                // 重新获取响应文本进行 fallback 解析
                                match self.client.get(uri).send().await {
                                    Ok(fallback_response) if fallback_response.status().is_success() => {
                                        match fallback_response.text().await {
                                            Ok(text) => {
                                                match Self::parse_metadata_fallback(&text) {
                                                    Some(metadata) => {
                                                        info!("✅ Fallback解析成功: {} (尝试第{}次)", uri, attempt);
                                                        return Ok(Some(metadata));
                                                    }
                                                    None => {
                                                        if attempt == 6 {
                                                            warn!("⚠️ 解析URI元数据JSON失败: {} - {} (最终失败)", uri, json_error);
                                                            return Ok(None);
                                                        }
                                                        let delay = Self::calculate_retry_delay(attempt, &status);
                                                        warn!("⚠️ 解析URI元数据JSON失败: {} - {} (第{}次，{}秒后重试)", uri, json_error, attempt, delay);
                                                        tokio::time::sleep(Duration::from_secs(delay)).await;
                                                    }
                                                }
                                            }
                                            Err(_) => {
                                                if attempt == 6 {
                                                    warn!("⚠️ 解析URI元数据JSON失败: {} - {} (最终失败)", uri, json_error);
                                                    return Ok(None);
                                                }
                                                let delay = Self::calculate_retry_delay(attempt, &status);
                                                warn!("⚠️ 解析URI元数据JSON失败: {} - {} (第{}次，{}秒后重试)", uri, json_error, attempt, delay);
                                                tokio::time::sleep(Duration::from_secs(delay)).await;
                                            }
                                        }
                                    }
                                    Ok(_) => {
                                        // 处理非成功状态码的情况
                                        if attempt == 6 {
                                            warn!("⚠️ 解析URI元数据JSON失败: {} - {} (最终失败)", uri, json_error);
                                            return Ok(None);
                                        }
                                        let delay = Self::calculate_retry_delay(attempt, &status);
                                        warn!("⚠️ 解析URI元数据JSON失败: {} - {} (第{}次，{}秒后重试)", uri, json_error, attempt, delay);
                                        tokio::time::sleep(Duration::from_secs(delay)).await;
                                    }
                                    Err(_) => {
                                        if attempt == 6 {
                                            warn!("⚠️ 解析URI元数据JSON失败: {} - {} (最终失败)", uri, json_error);
                                            return Ok(None);
                                        }
                                        let delay = Self::calculate_retry_delay(attempt, &status);
                                        warn!("⚠️ 解析URI元数据JSON失败: {} - {} (第{}次，{}秒后重试)", uri, json_error, attempt, delay);
                                        tokio::time::sleep(Duration::from_secs(delay)).await;
                                    }
                                }
                            }
                        }
                    } else {
                        if attempt == 6 {
                            warn!("⚠️ URI元数据请求失败: {} - {} (最终失败)", uri, status);
                            return Ok(None);
                        }
                        let delay = Self::calculate_retry_delay(attempt, &status);
                        warn!("⚠️ URI元数据请求失败: {} - {} (第{}次，{}秒后重试)", uri, status, attempt, delay);
                        tokio::time::sleep(Duration::from_secs(delay)).await;
                    }
                }
                Err(e) => {
                    if attempt == 6 {
                        warn!("⚠️ 无法访问URI: {} - {} (最终失败)", uri, e);
                        return Ok(None);
                    }
                    let delay = Self::calculate_retry_delay(attempt, &reqwest::StatusCode::INTERNAL_SERVER_ERROR);
                    warn!("⚠️ 无法访问URI: {} - {} (第{}次，{}秒后重试)", uri, e, attempt, delay);
                    tokio::time::sleep(Duration::from_secs(delay)).await;
                }
            }
        }

        Ok(None)
    }

    /// Fallback 元数据解析器 - 从损坏的JSON中尽可能提取信息
    fn parse_metadata_fallback(json_text: &str) -> Option<UriMetadata> {
        use serde_json::Value;
        
        // 尝试解析为任意JSON值
        let json_value: Value = match serde_json::from_str(json_text) {
            Ok(value) => value,
            Err(_) => return None,
        };
        
        // 如果是对象，尝试提取可用字段
        if let Value::Object(obj) = json_value {
            let mut metadata = UriMetadata {
                token_name: None,
                token_symbol: None,
                description: None,
                avatar_url: None,
                social_links: None,
                whitelist: None,
                purchase_limit: None,
                crowdfunding: None,
            };
            
            // 安全提取字符串字段
            if let Some(Value::String(s)) = obj.get("tokenName") {
                metadata.token_name = Some(s.clone());
            }
            
            if let Some(Value::String(s)) = obj.get("tokenSymbol") {
                metadata.token_symbol = Some(s.clone());
            }
            
            if let Some(Value::String(s)) = obj.get("description") {
                metadata.description = Some(s.clone());
            }
            
            if let Some(Value::String(s)) = obj.get("avatarUrl") {
                metadata.avatar_url = Some(s.clone());
            }
            
            // 尝试解析社交链接
            if let Some(social_obj) = obj.get("socialLinks").and_then(|v| v.as_object()) {
                metadata.social_links = Some(SocialLinks {
                    twitter: social_obj.get("twitter").and_then(|v| v.as_str()).map(|s| s.to_string()),
                    telegram: social_obj.get("telegram").and_then(|v| v.as_str()).map(|s| s.to_string()),
                    website: social_obj.get("website").and_then(|v| v.as_str()).map(|s| s.to_string()),
                });
            }
            
            // 尝试解析白名单信息
            if let Some(whitelist_obj) = obj.get("whitelist").and_then(|v| v.as_object()) {
                let enabled = whitelist_obj.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false);
                let addresses = whitelist_obj.get("addresses")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default();
                
                metadata.whitelist = Some(WhitelistInfo { enabled, addresses });
            }
            
            // 安全提取购买限制（可能是字符串或对象）
            if let Some(purchase_val) = obj.get("purchaseLimit") {
                metadata.purchase_limit = match purchase_val {
                    Value::String(s) => Some(s.clone()),
                    Value::Object(_) => Some(purchase_val.to_string()),
                    _ => None,
                };
            }
            
            // 鲁棒地解析众筹信息（主要问题字段）
            if let Some(crowdfunding_obj) = obj.get("crowdfunding").and_then(|v| v.as_object()) {
                let start_time = match crowdfunding_obj.get("startTime") {
                    Some(Value::String(s)) => Some(s.clone()),
                    Some(Value::Number(n)) => {
                        if let Some(i) = n.as_i64() {
                            Some(i.to_string())
                        } else if let Some(f) = n.as_f64() {
                            Some((f as i64).to_string())
                        } else {
                            None
                        }
                    }
                    _ => None,
                };
                
                let end_time = match crowdfunding_obj.get("endTime") {
                    Some(Value::String(s)) => Some(s.clone()),
                    Some(Value::Number(n)) => {
                        if let Some(i) = n.as_i64() {
                            Some(i.to_string())
                        } else if let Some(f) = n.as_f64() {
                            Some((f as i64).to_string())
                        } else {
                            None
                        }
                    }
                    _ => None,
                };
                
                let duration = crowdfunding_obj.get("duration").and_then(|v| v.as_u64()).map(|v| v as u32);
                
                metadata.crowdfunding = Some(CrowdfundingInfo {
                    start_time,
                    end_time,
                    duration,
                });
            }
            
            info!("🛡️ Fallback解析提取到字段数: {}", 
                [metadata.token_name.is_some(), metadata.token_symbol.is_some(), 
                 metadata.description.is_some(), metadata.avatar_url.is_some(),
                 metadata.social_links.is_some(), metadata.whitelist.is_some(),
                 metadata.purchase_limit.is_some(), metadata.crowdfunding.is_some()]
                .iter().filter(|&&b| b).count()
            );
            
            Some(metadata)
        } else {
            None
        }
    }
    
    /// 计算重试延迟时间（线性递增策略）
    fn calculate_retry_delay(attempt: u32, status: &reqwest::StatusCode) -> u64 {
        match status {
            // 429 Too Many Requests - 使用线性递增延迟: 1,3,5,7,9,11秒
            &reqwest::StatusCode::TOO_MANY_REQUESTS => {
                match attempt {
                    1 => 1,
                    2 => 3,
                    3 => 5,
                    4 => 7,
                    5 => 9,
                    6 => 11,
                    _ => 11, // 备用，不过不应该到达这里
                }
            }
            // 5xx服务器错误 - 较短延迟: 2,4,6,8,10,12秒
            status if status.is_server_error() => {
                (attempt * 2) as u64
            }
            // 网络错误和超时 - 线性递增: 1,2,3,4,5,6秒
            &reqwest::StatusCode::INTERNAL_SERVER_ERROR => {
                attempt as u64
            }
            // 其他错误 - 线性递增: 1,2,3,4,5,6秒  
            _ => {
                attempt as u64
            }
        }
    }

    /// 合并链上元数据和URI元数据
    fn merge_metadata(&self, chain_metadata: TokenMetadata, uri_metadata: Option<UriMetadata>) -> TokenMetadata {
        if let Some(uri_meta) = uri_metadata {
            let mut tags = chain_metadata.tags;

            // 检查动画URL
            if uri_meta.avatar_url.is_some() {
                tags.push("avatar_url".to_string());
            }

            // 检查属性
            if let Some(_) = uri_meta.social_links {
                tags.push("rich-metadata".to_string());
            }

            TokenMetadata {
                address: chain_metadata.address,
                decimals: chain_metadata.decimals,
                symbol: chain_metadata.symbol.or(uri_meta.token_symbol),
                name: chain_metadata.name.or(uri_meta.token_name),
                logo_uri: chain_metadata.logo_uri.or(uri_meta.avatar_url.clone()),
                description: chain_metadata.description.or(uri_meta.description),
                external_url: chain_metadata.external_url.or(uri_meta.avatar_url),
                attributes: chain_metadata.attributes.or(None),
                tags,
            }
        } else {
            chain_metadata
        }
    }

    /// 计算元数据程序派生地址(PDA)
    fn find_metadata_pda(&self, mint: &Pubkey) -> Result<Pubkey> {
        let metadata_program_id = METADATA_PROGRAM_ID
            .parse::<Pubkey>()
            .map_err(|e| anyhow::anyhow!("解析元数据程序ID失败: {}", e))?;

        // 计算元数据账户的PDA
        let seeds = &["metadata".as_bytes(), metadata_program_id.as_ref(), mint.as_ref()];

        let (metadata_pubkey, _bump) = Pubkey::find_program_address(seeds, &metadata_program_id);
        Ok(metadata_pubkey)
    }

    /// 创建回退元数据
    fn create_fallback_metadata(&self, mint_address: &str) -> TokenMetadata {
        // 对于一些知名代币，提供硬编码的信息
        match mint_address {
            "So11111111111111111111111111111111111111112" => TokenMetadata {
                address: mint_address.to_string(),
                decimals: 9,
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
                decimals: 6,
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
                decimals: 6,
                symbol: Some("RAY".to_string()),
                name: Some("Raydium".to_string()),
                logo_uri: Some("https://img-v1.raydium.io/icon/4k3Dyjzvzp8eMZWUXbBCjEvwSkkk59S5iCNLY3QrkX6R.png".to_string()),
                description: Some("Raydium Protocol Token".to_string()),
                external_url: Some("https://raydium.io".to_string()),
                attributes: None,
                tags: vec![],
            },
            "CKgtJw9y47qAgxRHBdgjABY7DP4u6bLHXM1G68anWwJm" => TokenMetadata {
                address: mint_address.to_string(),
                decimals: 6,
                symbol: Some("JM-M1".to_string()),
                name: Some("JM-M1".to_string()),
                logo_uri: Some("https://localhost:8000/static/coin.png".to_string()),
                description: Some("JM-M1".to_string()),
                external_url: Some("JM-M1".to_string()),
                attributes: None,
                tags: vec![],
            },
            "5pbcULDGXotRZjJvmoiqj3qYaHJeDYAWpsaT58j6Ao56" => TokenMetadata {
                address: mint_address.to_string(),
                decimals: 6,
                symbol: Some("56-M0".to_string()),
                name: Some("56-M0".to_string()),
                logo_uri: Some("https://localhost:8000/static/coin.png".to_string()),
                description: Some("56-M0".to_string()),
                external_url: Some("56-M0".to_string()),
                attributes: None,
                tags: vec![],
            },
            "9C57seuQ3B6yNTmxwU4TdxmCwHEQWq8SMQUn6MYKXxUU" => TokenMetadata {
                address: mint_address.to_string(),
                decimals: 9,
                symbol: Some("CFT1".to_string()),
                name: Some("cftest1".to_string()),
                logo_uri: Some("https://localhost:8000/static/coin.png".to_string()),
                description: Some("CFT1".to_string()),
                external_url: Some("CFT1".to_string()),
                attributes: None,
                tags: vec![],
            },
            "4W4WpXG85nsZEGBdFJsnAR1BgFhR688BgHUqmvwnjgNE" => TokenMetadata {
                address: mint_address.to_string(),
                decimals: 9,
                symbol: Some("CFT2".to_string()),
                name: Some("cftest2".to_string()),
                logo_uri: Some("https://localhost:8000/static/coin.png".to_string()),
                description: Some("CFT2".to_string()),
                external_url: Some("CFT2".to_string()),
                attributes: None,
                tags: vec![],
            },
            "4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU" => TokenMetadata {
                address: mint_address.to_string(),
                decimals: 6,
                symbol: Some("USDC".to_string()),
                name: Some("USD Coin".to_string()),
                logo_uri: Some("https://localhost:8000/static/coin.png".to_string()),
                description: Some("USDC".to_string()),
                external_url: Some("USDC".to_string()),
                attributes: None,
                tags: vec![],
            },
            "CF1Ms9vjvGEiSHqoj1jLadoLNXD9EqtnR6TZp1w8CeHz" => TokenMetadata {
                address: mint_address.to_string(),
                decimals: 9,
                symbol: Some("FAIR".to_string()),
                name: Some("FAIR".to_string()),
                logo_uri: Some("https://localhost:8000/static/coin.png".to_string()),
                description: Some("FAIR".to_string()),
                external_url: Some("FAIR".to_string()),
                attributes: None,
                tags: vec![],
            },
            _ => {
                info!("🔍 创建为空的数据: {}", mint_address);
                TokenMetadata {
                    address: mint_address.to_string(),
                    decimals: 6, // 默认6位小数
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

    /// 批量获取多个代币的元数据
    ///
    /// 用于向后兼容server crate中的调用方式
    pub async fn get_tokens_metadata(
        &mut self,
        mint_addresses: &[String],
    ) -> anyhow::Result<HashMap<String, TokenMetadata>> {
        let mut result = HashMap::new();

        info!("🔍 批量获取 {} 个代币的元数据", mint_addresses.len());

        for mint_address in mint_addresses {
            match self.get_token_metadata(mint_address).await {
                Ok(Some(metadata)) => {
                    result.insert(mint_address.clone(), metadata);
                }
                Ok(None) => {
                    info!("⚠️ 未找到代币元数据: {}", mint_address);
                    // 对于没有找到的代币，我们不插入到结果中
                }
                Err(e) => {
                    warn!("❌ 获取代币元数据失败: {} - {}", mint_address, e);
                    // 继续处理其他代币，不中断整个批量操作
                }
            }
        }

        info!("✅ 批量获取完成，成功获取 {} 个代币的元数据", result.len());
        Ok(result)
    }
}

/// 为 MetaplexService 实现 TokenMetadataProvider trait
#[async_trait::async_trait]
impl TokenMetadataProvider for MetaplexService {
    async fn get_token_metadata(&mut self, mint_address: &str) -> anyhow::Result<Option<ExternalTokenMetadata>> {
        // 检查缓存
        if let Some(metadata) = self.cache.get(mint_address) {
            info!("📦 从缓存获取代币元数据: {}", mint_address);
            let external_metadata = ExternalTokenMetadata::from_token_metadata(metadata.clone());
            return Ok(Some(external_metadata));
        }

        info!("🔍 获取代币元数据: {}", mint_address);

        // 尝试从多个来源获取元数据
        match self.fetch_metadata_with_fallback(mint_address).await? {
            Some(metadata) => {
                // 缓存结果
                self.cache.insert(mint_address.to_string(), metadata.clone());
                info!(
                    "✅ 成功获取代币元数据: {} - {}",
                    mint_address,
                    metadata.symbol.as_deref().unwrap_or("Unknown")
                );

                // 将 TokenMetadata 转换为 ExternalTokenMetadata
                let external_metadata = ExternalTokenMetadata::from_token_metadata(metadata);
                Ok(Some(external_metadata))
            }
            None => {
                warn!("⚠️ 未找到代币元数据: {}", mint_address);
                Ok(None)
            }
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_metaplex_service_creation() {
        // 测试 MetaplexService 创建
        let service = MetaplexService::new(None);
        assert!(service.is_ok());

        let service = service.unwrap();
        assert_eq!(service.cache.len(), 0);
    }

    #[tokio::test]
    async fn test_token_metadata_provider_trait() {
        // 测试 TokenMetadataProvider trait 实现
        let mut service = MetaplexService::new(None).unwrap();

        // 测试 WSOL 的元数据获取（可能来自链上或fallback）
        let result = service
            .get_token_metadata("So11111111111111111111111111111111111111112")
            .await;
        assert!(result.is_ok());

        let metadata = result.unwrap();
        assert!(metadata.is_some());

        let metadata = metadata.unwrap();
        assert_eq!(metadata.address, "So11111111111111111111111111111111111111112");
        // 注意：symbol 可能是 "SOL" (来自token list) 或 "WSOL" (来自fallback)
        assert!(metadata.symbol.is_some());
        let symbol = metadata.symbol.unwrap();
        assert!(
            symbol == "SOL" || symbol == "WSOL",
            "Expected SOL or WSOL, got: {}",
            symbol
        );
        // name 可能是 "Solana" 或 "Wrapped SOL"
        assert!(metadata.name.is_some());
    }

    #[tokio::test]
    async fn test_external_token_metadata_conversion() {
        // 测试 ExternalTokenMetadata 转换
        let token_metadata = TokenMetadata {
            address: "test123".to_string(),
            decimals: 6,
            symbol: Some("TEST".to_string()),
            name: Some("Test Token".to_string()),
            logo_uri: Some("https://example.com/logo.png".to_string()),
            description: Some("A test token".to_string()),
            external_url: Some("https://example.com".to_string()),
            attributes: Some(vec![TokenAttribute {
                trait_type: "type".to_string(),
                value: "utility".to_string(),
            }]),
            tags: vec!["test".to_string()],
        };

        // 转换为 ExternalTokenMetadata
        let external = ExternalTokenMetadata::from_token_metadata(token_metadata.clone());
        assert_eq!(external.address, "test123");
        assert_eq!(external.symbol, Some("TEST".to_string()));
        assert_eq!(external.name, Some("Test Token".to_string()));
        assert_eq!(external.tags, vec!["test".to_string()]);

        // 转换回 TokenMetadata
        let converted_back = external.to_token_metadata(6);
        assert_eq!(converted_back.address, token_metadata.address);
        assert_eq!(converted_back.decimals, token_metadata.decimals);
        assert_eq!(converted_back.symbol, token_metadata.symbol);
        assert_eq!(converted_back.name, token_metadata.name);
    }

    #[test]
    fn test_metaplex_config_default() {
        let config = MetaplexConfig::default();
        assert_eq!(config.base_url, "https://api.metaplex.com");
        assert_eq!(config.timeout_seconds, 30);
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.batch_size, 50);
        assert_eq!(config.network, "mainnet");
    }

    #[test]
    fn test_fallback_metadata_creation() {
        let service = MetaplexService::new(None).unwrap();

        // 测试 WSOL fallback
        let wsol_metadata = service.create_fallback_metadata("So11111111111111111111111111111111111111112");
        assert_eq!(wsol_metadata.symbol, Some("WSOL".to_string()));
        assert_eq!(wsol_metadata.name, Some("Wrapped SOL".to_string()));
        assert_eq!(wsol_metadata.decimals, 9);

        // 测试 USDC fallback
        let usdc_metadata = service.create_fallback_metadata("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
        assert_eq!(usdc_metadata.symbol, Some("USDC".to_string()));
        assert_eq!(usdc_metadata.name, Some("USD Coin".to_string()));
        assert_eq!(usdc_metadata.decimals, 6);

        // 测试未知代币 fallback
        let unknown_metadata = service.create_fallback_metadata("UnknownToken123456789");
        assert_eq!(unknown_metadata.symbol, None);
        assert_eq!(unknown_metadata.name, None);
        assert_eq!(unknown_metadata.decimals, 6);
    }

    #[test]
    fn test_cache_operations() {
        let mut service = MetaplexService::new(None).unwrap();

        // 测试缓存统计
        let (size, _capacity) = service.get_cache_stats();
        assert_eq!(size, 0);

        // 测试清除缓存
        service.clear_cache();
        let (size, _) = service.get_cache_stats();
        assert_eq!(size, 0);
    }

    #[test]
    fn test_flexible_timestamp_deserialization() {
        // 测试自定义时间戳反序列化器处理各种格式
        
        // 测试数字格式的时间戳（原始问题案例）
        let json_with_numbers = r#"{
            "tokenName": "10min",
            "tokenSymbol": "Mten", 
            "description": "十分钟过期测试",
            "crowdfunding": {
                "startTime": 1756791015,
                "endTime": 1757391,
                "duration": 600
            }
        }"#;
        
        let result: Result<UriMetadata, _> = serde_json::from_str(json_with_numbers);
        assert!(result.is_ok(), "应该能够解析数字时间戳: {:?}", result.err());
        
        let metadata = result.unwrap();
        assert_eq!(metadata.token_name, Some("10min".to_string()));
        assert_eq!(metadata.token_symbol, Some("Mten".to_string()));
        assert!(metadata.crowdfunding.is_some());
        
        let crowdfunding = metadata.crowdfunding.unwrap();
        assert_eq!(crowdfunding.start_time, Some("1756791015".to_string()));
        assert_eq!(crowdfunding.end_time, Some("1757391".to_string()));
        assert_eq!(crowdfunding.duration, Some(600));

        // 测试字符串格式的时间戳
        let json_with_strings = r#"{
            "tokenName": "TestToken",
            "crowdfunding": {
                "startTime": "1756791015",
                "endTime": "1757391",
                "duration": 600
            }
        }"#;
        
        let result: Result<UriMetadata, _> = serde_json::from_str(json_with_strings);
        assert!(result.is_ok(), "应该能够解析字符串时间戳");
        
        let metadata = result.unwrap();
        assert!(metadata.crowdfunding.is_some());
        let crowdfunding = metadata.crowdfunding.unwrap();
        assert_eq!(crowdfunding.start_time, Some("1756791015".to_string()));
        assert_eq!(crowdfunding.end_time, Some("1757391".to_string()));

        // 测试混合格式
        let json_mixed = r#"{
            "tokenName": "MixedToken",
            "crowdfunding": {
                "startTime": 1756791015,
                "endTime": "1757391",
                "duration": 600
            }
        }"#;
        
        let result: Result<UriMetadata, _> = serde_json::from_str(json_mixed);
        assert!(result.is_ok(), "应该能够解析混合格式");

        // 测试空值处理
        let json_with_nulls = r#"{
            "tokenName": "NullToken",
            "crowdfunding": {
                "startTime": null,
                "endTime": null,
                "duration": 600
            }
        }"#;
        
        let result: Result<UriMetadata, _> = serde_json::from_str(json_with_nulls);
        assert!(result.is_ok(), "应该能够处理null值");
        
        let metadata = result.unwrap();
        let crowdfunding = metadata.crowdfunding.unwrap();
        assert_eq!(crowdfunding.start_time, None);
        assert_eq!(crowdfunding.end_time, None);
    }

    #[test]
    fn test_fallback_metadata_parser() {
        // 测试fallback解析器能够从部分损坏的JSON中提取信息
        
        // 测试完整的JSON（应该成功解析）
        let complete_json = r#"{
            "tokenName": "Complete Token",
            "tokenSymbol": "COMPLETE",
            "description": "A complete token",
            "avatarUrl": "https://example.com/avatar.png",
            "socialLinks": {
                "twitter": "https://twitter.com/token",
                "telegram": "https://t.me/token",
                "website": "https://token.com"
            },
            "whitelist": {
                "enabled": true,
                "addresses": ["addr1", "addr2"]
            },
            "purchaseLimit": "100 SOL",
            "crowdfunding": {
                "startTime": 1756791015,
                "endTime": 1757391,
                "duration": 600
            }
        }"#;
        
        let result = MetaplexService::parse_metadata_fallback(complete_json);
        assert!(result.is_some(), "完整JSON应该能够解析");
        
        let metadata = result.unwrap();
        assert_eq!(metadata.token_name, Some("Complete Token".to_string()));
        assert_eq!(metadata.token_symbol, Some("COMPLETE".to_string()));
        assert_eq!(metadata.description, Some("A complete token".to_string()));
        assert_eq!(metadata.avatar_url, Some("https://example.com/avatar.png".to_string()));
        assert_eq!(metadata.purchase_limit, Some("100 SOL".to_string()));
        
        // 检查社交链接
        assert!(metadata.social_links.is_some());
        let social_links = metadata.social_links.unwrap();
        assert_eq!(social_links.twitter, Some("https://twitter.com/token".to_string()));
        assert_eq!(social_links.telegram, Some("https://t.me/token".to_string()));
        assert_eq!(social_links.website, Some("https://token.com".to_string()));
        
        // 检查白名单
        assert!(metadata.whitelist.is_some());
        let whitelist = metadata.whitelist.unwrap();
        assert_eq!(whitelist.enabled, true);
        assert_eq!(whitelist.addresses, vec!["addr1".to_string(), "addr2".to_string()]);
        
        // 检查众筹信息（重点测试数字时间戳转换）
        assert!(metadata.crowdfunding.is_some());
        let crowdfunding = metadata.crowdfunding.unwrap();
        assert_eq!(crowdfunding.start_time, Some("1756791015".to_string()));
        assert_eq!(crowdfunding.end_time, Some("1757391".to_string()));
        assert_eq!(crowdfunding.duration, Some(600));

        // 测试最小JSON（只有基本字段）
        let minimal_json = r#"{
            "tokenName": "Minimal Token",
            "tokenSymbol": "MIN"
        }"#;
        
        let result = MetaplexService::parse_metadata_fallback(minimal_json);
        assert!(result.is_some(), "最小JSON应该能够解析");
        
        let metadata = result.unwrap();
        assert_eq!(metadata.token_name, Some("Minimal Token".to_string()));
        assert_eq!(metadata.token_symbol, Some("MIN".to_string()));
        assert_eq!(metadata.description, None);
        assert_eq!(metadata.crowdfunding, None);

        // 测试无效JSON
        let invalid_json = "invalid json data";
        let result = MetaplexService::parse_metadata_fallback(invalid_json);
        assert!(result.is_none(), "无效JSON应该返回None");

        // 测试非对象JSON
        let array_json = r#"["not", "an", "object"]"#;
        let result = MetaplexService::parse_metadata_fallback(array_json);
        assert!(result.is_none(), "非对象JSON应该返回None");
    }

    #[test]
    fn test_purchase_limit_flexible_parsing() {
        // 测试purchaseLimit字段的灵活解析（可能是字符串或对象）
        
        // 字符串格式
        let json_string_limit = r#"{
            "tokenName": "StringLimit Token",
            "purchaseLimit": "100 SOL"
        }"#;
        
        let result = MetaplexService::parse_metadata_fallback(json_string_limit);
        assert!(result.is_some());
        let metadata = result.unwrap();
        assert_eq!(metadata.purchase_limit, Some("100 SOL".to_string()));

        // 对象格式
        let json_object_limit = r#"{
            "tokenName": "ObjectLimit Token", 
            "purchaseLimit": { "tier1": { "max": 100, "currency": "SOL" } }
        }"#;
        
        let result = MetaplexService::parse_metadata_fallback(json_object_limit);
        assert!(result.is_some());
        let metadata = result.unwrap();
        assert!(metadata.purchase_limit.is_some());
        let limit = metadata.purchase_limit.unwrap();
        assert!(limit.contains("tier1"), "应该包含对象内容的字符串表示");
    }

    #[test]
    fn test_real_problematic_data() {
        // 测试实际的问题数据
        let real_problem_json = r#"{
            "tokenName": "10min",
            "tokenSymbol": "Mten",
            "description": "十分钟过期测试",
            "avatarUrl": "https://gateway.pinata.cloud/ipfs/bafkreieoqkd274daskgwgvjzwi5w6u5q4hbfsvj62f4b7yw332rfsav4am",
            "socialLinks": {
                "twitter": "",
                "telegram": "",
                "website": ""
            },
            "whitelist": {
                "enabled": false,
                "addresses": []
            },
            "purchaseLimit": "{ \"tier1\": {} }",
            "crowdfunding": {
                "startTime": 1756791015,
                "endTime": 1757391,
                "duration": 600
            }
        }"#;
        
        // 测试标准Serde反序列化（应该成功）
        let serde_result: Result<UriMetadata, _> = serde_json::from_str(real_problem_json);
        assert!(serde_result.is_ok(), "修复后应该能够解析实际问题数据: {:?}", serde_result.err());
        
        let metadata = serde_result.unwrap();
        assert_eq!(metadata.token_name, Some("10min".to_string()));
        assert_eq!(metadata.token_symbol, Some("Mten".to_string()));
        assert_eq!(metadata.description, Some("十分钟过期测试".to_string()));
        
        // 验证关键的crowdfunding数据正确解析
        assert!(metadata.crowdfunding.is_some());
        let crowdfunding = metadata.crowdfunding.unwrap();
        assert_eq!(crowdfunding.start_time, Some("1756791015".to_string())); // 数字转为字符串
        assert_eq!(crowdfunding.end_time, Some("1757391".to_string()));
        assert_eq!(crowdfunding.duration, Some(600));
        
        // 测试Fallback解析器也能处理（双重保险）
        let fallback_result = MetaplexService::parse_metadata_fallback(real_problem_json);
        assert!(fallback_result.is_some(), "Fallback解析器也应该能够处理");
        
        let fallback_metadata = fallback_result.unwrap();
        assert_eq!(fallback_metadata.token_name, Some("10min".to_string()));
        assert_eq!(fallback_metadata.token_symbol, Some("Mten".to_string()));
        
        // 验证fallback解析的crowdfunding数据
        let fallback_crowdfunding = fallback_metadata.crowdfunding.unwrap();
        assert_eq!(fallback_crowdfunding.start_time, Some("1756791015".to_string()));
        assert_eq!(fallback_crowdfunding.end_time, Some("1757391".to_string()));
    }

    #[tokio::test]
    async fn test_metaplex_service_as_token_metadata_provider() {
        // 测试 MetaplexService 能够成功作为 TokenMetadataProvider 使用
        let service = MetaplexService::new(None).unwrap();
        let _provider: Box<dyn TokenMetadataProvider> = Box::new(service);

        // 这个测试确保 MetaplexService 正确实现了 TokenMetadataProvider trait
        // 即使没有网络连接，fallback机制也应该能工作
        assert!(true); // 如果编译通过，说明 trait 实现正确
    }

    #[tokio::test]
    async fn test_fetch_metadata_from_uri() {
        let service = MetaplexService::new(None).unwrap();

        // 测试无效的URI
        let invalid_uri = "not-a-valid-url";
        let result = service.fetch_metadata_from_uri(invalid_uri).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());

        // 测试非HTTP URI
        let ipfs_uri = "ipfs://QmTest123";
        let result = service.fetch_metadata_from_uri(ipfs_uri).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none()); // IPFS URI应该被跳过
    }

    #[test]
    fn test_uri_metadata_structure() {
        // 测试UriMetadata结构的序列化/反序列化
        let uri_metadata = UriMetadata {
            token_name: Some("Test Token".to_string()),
            token_symbol: Some("TEST".to_string()),
            avatar_url: Some("https://example.com/test.png".to_string()),
            social_links: Some(SocialLinks {
                twitter: Some("https://twitter.com/test".to_string()),
                telegram: Some("https://t.me/test".to_string()),
                website: Some("https://example.com".to_string()),
            }),
            description: Some("A test token from URI".to_string()),
            whitelist: Some(WhitelistInfo {
                enabled: true,
                addresses: vec!["test1".to_string(), "test2".to_string()],
            }),
            purchase_limit: Some("100".to_string()),
            crowdfunding: Some(CrowdfundingInfo {
                start_time: Some("2021-01-01T00:00:00Z".to_string()),
                end_time: Some("2021-01-02T00:00:00Z".to_string()),
                duration: Some(1),
            }),
        };

        // 测试序列化
        let json = serde_json::to_string(&uri_metadata);
        assert!(json.is_ok());

        // 测试反序列化
        let deserialized: Result<UriMetadata, _> = serde_json::from_str(&json.unwrap());
        assert!(deserialized.is_ok());

        let deserialized = deserialized.unwrap();
        assert_eq!(deserialized.token_name, Some("Test Token".to_string()));
        assert_eq!(deserialized.token_symbol, Some("TEST".to_string()));
        assert_eq!(deserialized.description, Some("A test token from URI".to_string()));
    }
}
