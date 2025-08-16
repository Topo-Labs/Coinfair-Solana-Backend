//! 共享的代币元数据结构体
//!
//! 定义了在整个项目中使用的标准化代币元数据结构，
//! 支持多种来源的元数据（链上、Metaplex、Token List等）

use serde::{Deserialize, Serialize};

/// 标准化的代币元数据结构
/// 
/// 用于整个项目中的代币元数据表示，支持从多种来源获取的数据
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TokenMetadata {
    /// 代币地址
    pub address: String,
    /// 代币小数位数
    pub decimals: u8,
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
    /// 属性列表
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

impl TokenMetadata {
    /// 创建新的代币元数据实例
    pub fn new(address: String, decimals: u8) -> Self {
        Self {
            address,
            decimals,
            symbol: None,
            name: None,
            logo_uri: None,
            description: None,
            external_url: None,
            attributes: None,
            tags: Vec::new(),
        }
    }

    /// 创建基础代币元数据（仅包含地址和decimals）
    pub fn basic(address: String, decimals: u8) -> Self {
        Self::new(address, decimals)
    }

    /// 创建完整的代币元数据
    pub fn full(
        address: String,
        decimals: u8,
        symbol: Option<String>,
        name: Option<String>,
        logo_uri: Option<String>,
    ) -> Self {
        Self {
            address,
            decimals,
            symbol,
            name,
            logo_uri,
            description: None,
            external_url: None,
            attributes: None,
            tags: Vec::new(),
        }
    }

    /// 检查是否有完整的元数据信息
    pub fn is_complete(&self) -> bool {
        self.name.is_some() && self.symbol.is_some() && self.logo_uri.is_some()
    }

    /// 检查是否为基础元数据（仅有地址和decimals）
    pub fn is_basic(&self) -> bool {
        self.symbol.is_none() && self.name.is_none() && self.logo_uri.is_none()
    }

    /// 合并两个TokenMetadata，优先使用非None值
    pub fn merge_with(mut self, other: TokenMetadata) -> Self {
        if self.symbol.is_none() && other.symbol.is_some() {
            self.symbol = other.symbol;
        }
        if self.name.is_none() && other.name.is_some() {
            self.name = other.name;
        }
        if self.logo_uri.is_none() && other.logo_uri.is_some() {
            self.logo_uri = other.logo_uri;
        }
        if self.description.is_none() && other.description.is_some() {
            self.description = other.description;
        }
        if self.external_url.is_none() && other.external_url.is_some() {
            self.external_url = other.external_url;
        }
        if self.attributes.is_none() && other.attributes.is_some() {
            self.attributes = other.attributes;
        }
        
        // 合并标签，去重
        for tag in other.tags {
            if !self.tags.contains(&tag) {
                self.tags.push(tag);
            }
        }
        
        self
    }

    /// 添加标签
    pub fn add_tag(&mut self, tag: String) {
        if !self.tags.contains(&tag) {
            self.tags.push(tag);
        }
    }

    /// 添加属性
    pub fn add_attribute(&mut self, trait_type: String, value: String) {
        let attribute = TokenAttribute { trait_type, value };
        
        match &mut self.attributes {
            Some(attrs) => {
                // 检查是否已存在同样的属性，如果存在则更新值
                if let Some(existing) = attrs.iter_mut().find(|attr| attr.trait_type == attribute.trait_type) {
                    existing.value = attribute.value;
                } else {
                    attrs.push(attribute);
                }
            }
            None => {
                self.attributes = Some(vec![attribute]);
            }
        }
    }

    /// 获取显示名称（优先返回name，否则返回symbol，最后返回地址缩写）
    pub fn display_name(&self) -> String {
        self.name
            .as_ref()
            .or(self.symbol.as_ref())
            .map(|s| s.clone())
            .unwrap_or_else(|| {
                // 返回地址的缩写形式
                if self.address.len() > 8 {
                    format!("{}...{}", &self.address[..4], &self.address[self.address.len()-4..])
                } else {
                    self.address.clone()
                }
            })
    }

    /// 获取显示符号（优先返回symbol，否则返回地址缩写）
    pub fn display_symbol(&self) -> String {
        self.symbol
            .as_ref()
            .map(|s| s.clone())
            .unwrap_or_else(|| {
                if self.address.len() > 6 {
                    format!("{}..{}", &self.address[..3], &self.address[self.address.len()-3..])
                } else {
                    self.address.clone()
                }
            })
    }
}

impl Default for TokenMetadata {
    fn default() -> Self {
        Self {
            address: String::new(),
            decimals: 6, // 默认6位小数
            symbol: None,
            name: None,
            logo_uri: None,
            description: None,
            external_url: None,
            attributes: None,
            tags: Vec::new(),
        }
    }
}

/// 元数据来源类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MetadataSource {
    /// 链上数据（包括mint账户信息）
    OnChain,
    /// Token-2022原生元数据扩展
    Token2022Native,
    /// Metaplex元数据程序
    Metaplex,
    /// Jupiter Token List
    JupiterTokenList,
    /// Solana Token List
    SolanaTokenList,
    /// 内存缓存
    Cache,
    /// 数据库
    Database,
    /// 默认/回退数据
    Fallback,
}

/// 元数据获取结果
#[derive(Debug, Clone)]
pub struct MetadataResult {
    /// 元数据
    pub metadata: TokenMetadata,
    /// 来源
    pub source: MetadataSource,
    /// 是否来自缓存
    pub cached: bool,
    /// 获取时间戳（毫秒）
    pub timestamp: u64,
}

impl MetadataResult {
    /// 创建新的元数据结果
    pub fn new(metadata: TokenMetadata, source: MetadataSource) -> Self {
        Self {
            metadata,
            source,
            cached: false,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
        }
    }

    /// 创建缓存的元数据结果
    pub fn cached(metadata: TokenMetadata, source: MetadataSource) -> Self {
        Self {
            metadata,
            source,
            cached: true,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
        }
    }
}

/// 代币元数据提供者trait（抽象MetaplexService）
#[async_trait::async_trait]
pub trait TokenMetadataProvider: Send + Sync {
    /// 获取单个代币的元数据
    async fn get_token_metadata(&mut self, mint_address: &str) -> anyhow::Result<Option<ExternalTokenMetadata>>;
    
    /// 支持向下转型的方法（用于测试）
    fn as_any(&self) -> &dyn std::any::Any;
}

/// 外部代币元数据结构（从MetaplexService获取的格式）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalTokenMetadata {
    pub address: String,
    pub symbol: Option<String>,
    pub name: Option<String>,
    pub logo_uri: Option<String>,
    pub description: Option<String>,
    pub external_url: Option<String>,
    pub attributes: Option<Vec<ExternalTokenAttribute>>,
    pub tags: Vec<String>,
}

/// 外部代币属性结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalTokenAttribute {
    pub trait_type: String,
    pub value: String,
}

impl ExternalTokenMetadata {
    /// 从 TokenMetadata 转换为 ExternalTokenMetadata
    pub fn from_token_metadata(metadata: TokenMetadata) -> Self {
        Self {
            address: metadata.address,
            symbol: metadata.symbol,
            name: metadata.name,
            logo_uri: metadata.logo_uri,
            description: metadata.description,
            external_url: metadata.external_url,
            attributes: metadata.attributes.map(|attrs| {
                attrs.into_iter().map(|attr| ExternalTokenAttribute {
                    trait_type: attr.trait_type,
                    value: attr.value,
                }).collect()
            }),
            tags: metadata.tags,
        }
    }

    /// 转换为 TokenMetadata（需要提供 decimals）
    pub fn to_token_metadata(self, decimals: u8) -> TokenMetadata {
        TokenMetadata {
            address: self.address,
            decimals,
            symbol: self.symbol,
            name: self.name,
            logo_uri: self.logo_uri,
            description: self.description,
            external_url: self.external_url,
            attributes: self.attributes.map(|attrs| {
                attrs.into_iter().map(|attr| TokenAttribute {
                    trait_type: attr.trait_type,
                    value: attr.value,
                }).collect()
            }),
            tags: self.tags,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_metadata_creation() {
        let metadata = TokenMetadata::new("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(), 6);
        
        assert_eq!(metadata.address, "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
        assert_eq!(metadata.decimals, 6);
        assert!(metadata.is_basic());
        assert!(!metadata.is_complete());
    }

    #[test]
    fn test_full_metadata_creation() {
        let metadata = TokenMetadata::full(
            "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(),
            6,
            Some("USDC".to_string()),
            Some("USD Coin".to_string()),
            Some("https://example.com/usdc.png".to_string()),
        );

        assert!(metadata.is_complete());
        assert!(!metadata.is_basic());
        assert_eq!(metadata.symbol.unwrap(), "USDC");
        assert_eq!(metadata.name.unwrap(), "USD Coin");
    }

    #[test]
    fn test_metadata_merge() {
        let mut base = TokenMetadata::new("test".to_string(), 6);
        base.symbol = Some("TEST".to_string());

        let additional = TokenMetadata {
            address: "test".to_string(),
            decimals: 6,
            symbol: Some("OVERRIDE".to_string()), // 不会覆盖，因为base已有symbol
            name: Some("Test Token".to_string()),  // 会添加，因为base没有name
            logo_uri: Some("https://example.com/logo.png".to_string()),
            description: None,
            external_url: None,
            attributes: None,
            tags: vec!["test".to_string()],
        };

        let merged = base.merge_with(additional);

        assert_eq!(merged.symbol.unwrap(), "TEST"); // 保持原值
        assert_eq!(merged.name.unwrap(), "Test Token"); // 添加新值
        assert!(merged.tags.contains(&"test".to_string()));
    }

    #[test]
    fn test_display_functions() {
        let metadata = TokenMetadata::full(
            "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(),
            6,
            Some("USDC".to_string()),
            Some("USD Coin".to_string()),
            None,
        );

        assert_eq!(metadata.display_name(), "USD Coin");
        assert_eq!(metadata.display_symbol(), "USDC");

        // 测试没有name和symbol的情况
        let basic_metadata = TokenMetadata::new("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(), 6);
        assert_eq!(basic_metadata.display_name(), "EPjF...Dt1v");
        assert_eq!(basic_metadata.display_symbol(), "EPj..t1v");
    }

    #[test]
    fn test_add_attribute() {
        let mut metadata = TokenMetadata::new("test".to_string(), 6);
        
        metadata.add_attribute("decimals".to_string(), "6".to_string());
        metadata.add_attribute("type".to_string(), "utility".to_string());
        
        assert_eq!(metadata.attributes.as_ref().unwrap().len(), 2);
        
        // 测试更新已存在的属性
        metadata.add_attribute("decimals".to_string(), "9".to_string());
        assert_eq!(metadata.attributes.as_ref().unwrap().len(), 2); // 长度不变
        assert_eq!(metadata.attributes.as_ref().unwrap()[0].value, "9"); // 值被更新
    }

    #[test]
    fn test_metadata_result() {
        let metadata = TokenMetadata::new("test".to_string(), 6);
        let result = MetadataResult::new(metadata.clone(), MetadataSource::OnChain);
        
        assert_eq!(result.source, MetadataSource::OnChain);
        assert!(!result.cached);
        assert!(result.timestamp > 0);

        let cached_result = MetadataResult::cached(metadata, MetadataSource::Cache);
        assert!(cached_result.cached);
    }
}