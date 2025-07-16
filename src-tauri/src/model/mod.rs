pub mod tauri_download;
pub mod tauri_generator;
pub mod tauri_path_provider;

#[cfg(test)]
mod tauri_generator_tests;

// Re-export commonly used items
pub use model_ai::path_provider::ModelPathProvider;
pub use model_core::ModelConfig;
pub use tauri_generator::{ModelBasedBranchGenerator, ModelGeneratorState};
pub use tauri_path_provider::TauriModelPathProvider;
