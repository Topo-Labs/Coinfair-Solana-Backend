// Recovery模块：负责丢失事件的回填和恢复
pub mod backfill_manager;
pub mod backfill_handler;
pub mod backfill_task_context;
pub mod checkpoint_persistence;
pub mod scan_record_persistence;

// 导出主要的回填服务组件
pub use backfill_manager::BackfillManager;
pub use backfill_handler::{BackfillEventConfig, BackfillEventRegistry, EventBackfillHandler};
pub use backfill_task_context::BackfillTaskContext;
pub use checkpoint_persistence::CheckpointPersistence;
pub use scan_record_persistence::{ScanRecordPersistence, ScanStatistics};
