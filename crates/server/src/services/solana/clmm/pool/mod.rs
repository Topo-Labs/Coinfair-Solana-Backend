// CLMM pool service module for handling CLMM pool creation operations

pub mod chain_loader;
pub mod error_handler;
pub mod pool_service;
pub mod storage;
pub mod sync;

#[cfg(test)]
mod pool_tests;

#[cfg(test)]
mod integration_tests;

// Re-export the main service, storage, sync, chain loader and error handling
pub use chain_loader::ChainPoolLoader;
pub use error_handler::{
    ConsistencyChecker, ConsistencyIssue, ConsistencyIssueType, ErrorCategory, ErrorHandler, HealthChecker,
    HealthStatus, IssueSeverity, RetryConfig, TransactionManager,
};
pub use pool_service::ClmmPoolService;
pub use storage::{ClmmPoolStorageBuilder, ClmmPoolStorageService};
pub use sync::{ClmmPoolSyncBuilder, ClmmPoolSyncService, SyncConfig, SyncStats};
