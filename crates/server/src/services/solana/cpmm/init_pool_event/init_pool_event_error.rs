#[derive(Debug, thiserror::Error)]
pub enum InitPoolEventError {
    #[error("Init pool event not found")]
    EventNotFound,

    #[error("Event with pool_id {0} already exists")]
    DuplicatePoolId(String),

    #[error("Event with signature {0} already exists")]
    DuplicateSignature(String),

    #[error("Invalid pool_ids format: {0}")]
    InvalidPoolIdsFormat(String),

    #[error("Database error: {0}")]
    DatabaseError(#[from] mongodb::error::Error),

    #[error("Anyhow error: {0}")]
    AnyhowError(#[from] anyhow::Error),
}