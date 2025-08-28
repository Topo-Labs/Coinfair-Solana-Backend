// Main solana service module
// This module provides a modular architecture for Solana-related services

pub mod amm_pool;
pub mod clmm_pool;
pub mod config;
pub mod event;
pub mod launch_event;
pub mod launch_migration;
pub mod liquidity;
pub mod liquidity_line;
pub mod nft;
pub mod position;
pub mod referral;
pub mod service;
pub mod shared;
pub mod swap;
pub mod token;

// Re-export the main service and trait for external use
pub use service::{DynSolanaService, SolanaService, SolanaServiceTrait};

// Re-export commonly used types from shared module
pub use shared::types::*;

// Re-export launch event service
pub use launch_event::LaunchEventService;

// Re-export launch migration service
pub use launch_migration::LaunchMigrationService;
