// Position service module for handling all position management operations

pub mod position_service;

#[cfg(test)]
pub mod position_tests;

// Re-export the main service
pub use position_service::PositionService;
