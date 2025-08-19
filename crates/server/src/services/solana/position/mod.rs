// Position service module for handling all position management operations

pub mod service;

#[cfg(test)]
pub mod tests;

// Re-export the main service
pub use service::PositionService;
