/// Token type enumeration for classification
#[derive(Debug, Clone, PartialEq)]
pub enum TokenType {
    Sol,
    Usdc,
    Other(String),
}

/// Constants used across the service
pub mod constants {
    pub const DEFAULT_RAYDIUM_PROGRAM_ID: &str = "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8";
    pub const USDC_MINT_STANDARD: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
}
