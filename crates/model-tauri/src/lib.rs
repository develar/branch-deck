pub mod commands;
pub mod download;
pub mod generator;
pub mod path_provider;
pub mod types;

// Re-export commonly used items
pub use generator::{ModelBasedBranchGenerator, ModelGeneratorState};
pub use model_ai::path_provider::ModelPathProvider;
pub use model_core::ModelConfig;
pub use path_provider::TauriModelPathProvider;

#[cfg(test)]
mod commands_test;

#[cfg(test)]
mod generator_test;
