mod config;
mod generator_type;
pub mod prompt;
mod quantized_qwen3;
mod qwen25;
mod qwen3;
pub mod utils;

// Make test_utils available for standalone apps too
pub mod test_utils;

#[cfg(test)]
mod prompt_tests;

use serde::{Deserialize, Serialize};

pub use config::ModelConfig;
pub use generator_type::GeneratorType;
pub use quantized_qwen3::QuantizedQwen3BranchGenerator;
pub use qwen25::Qwen25BranchGenerator;
pub use qwen3::Qwen3BranchGenerator;
pub use utils::detect_device;

/// Branch name generation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchNameResult {
  pub name: String,
  pub confidence: f32,
  pub generation_time_ms: u64,
}

// All model implementations are now in separate files for clarity:
// - qwen25.rs: Qwen2.5-Coder models (uses candle_transformers::models::qwen2)
// - qwen3.rs: Qwen3 models (uses candle_transformers::models::qwen3)
// - quantized_qwen3.rs: Quantized Qwen3 GGUF models
