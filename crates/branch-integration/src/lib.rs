pub mod archive;
pub mod cache;
pub mod common;
pub mod detector;
pub mod merge;
pub mod rebase;
pub mod squash;
pub mod strategy;

// Re-export commonly used items
pub use cache::DETECTION_CACHE_VERSION;

// No re-exports - import modules directly
