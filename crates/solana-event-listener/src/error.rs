use thiserror::Error;

/// Event-Listener 错误类型定义
#[derive(Error, Debug)]
pub enum EventListenerError {
    #[error("配置错误: {0}")]
    Config(String),

    #[error("WebSocket连接错误: {0}")]
    WebSocket(String),

    #[error("事件解析错误: {0}")]
    EventParsing(String),

    #[error("Discriminator不匹配")]
    DiscriminatorMismatch,

    #[error("持久化错误: {0}")]
    Persistence(String),

    #[error("检查点错误: {0}")]
    Checkpoint(String),

    #[error("指标收集错误: {0}")]
    Metrics(String),

    #[error("Solana RPC错误: {0}")]
    SolanaRpc(String),

    #[error("网络连接错误: {0}")]
    Network(String),

    #[error("数据库错误: {0}")]
    Database(#[from] mongodb::error::Error),

    #[error("序列化错误: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Base64解码错误: {0}")]
    Base64Decode(#[from] base64::DecodeError),

    #[error("IO/Borsh错误: {0}")]
    IO(#[from] std::io::Error),

    #[error("Solana SDK错误: {0}")]
    SolanaSDK(String),

    #[error("未知错误: {0}")]
    Unknown(String),
}

impl From<anyhow::Error> for EventListenerError {
    fn from(err: anyhow::Error) -> Self {
        EventListenerError::Unknown(err.to_string())
    }
}

impl From<solana_client::client_error::ClientError> for EventListenerError {
    fn from(err: solana_client::client_error::ClientError) -> Self {
        EventListenerError::SolanaRpc(err.to_string())
    }
}

impl From<solana_sdk::program_error::ProgramError> for EventListenerError {
    fn from(err: solana_sdk::program_error::ProgramError) -> Self {
        EventListenerError::SolanaSDK(err.to_string())
    }
}

/// Result类型别名
pub type Result<T> = std::result::Result<T, EventListenerError>;
