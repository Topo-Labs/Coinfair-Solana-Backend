// CLMM pool service module for handling CLMM pool creation operations

pub mod service;
pub mod storage;
pub mod sync;
pub mod error_handler;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod integration_tests;

// Re-export the main service, storage, sync and error handling
pub use service::ClmmPoolService;
pub use storage::{ClmmPoolStorageService, ClmmPoolStorageBuilder};
pub use sync::{ClmmPoolSyncService, ClmmPoolSyncBuilder, SyncConfig, SyncStats};
pub use error_handler::{
    ErrorHandler, ConsistencyChecker, TransactionManager, HealthChecker,
    ErrorCategory, RetryConfig, ConsistencyIssue, ConsistencyIssueType, 
    IssueSeverity, HealthStatus
};
