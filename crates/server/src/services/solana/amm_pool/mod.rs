// AMM pool service module for handling classic AMM pool creation operations

pub mod service;

#[cfg(test)]
pub mod tests;

// Re-export the main service
pub use service::AmmPoolService;
