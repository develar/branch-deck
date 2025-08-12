pub mod config;
mod constants;
pub mod generator_type;
pub mod prompt;
pub mod quantized_qwen3;
pub mod qwen25;
pub mod qwen3;
pub mod utils;

// Make test_utils available for standalone apps too
pub mod test_utils;

#[cfg(test)]
mod prompt_tests;

use serde::{Deserialize, Serialize};

/// Branch name generation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchNameResult {
  pub name: String,
  pub generation_time_ms: u64,
}

// All model implementations are now in separate files for clarity:
// - qwen25.rs: Qwen2.5-Coder models (uses candle_transformers::models::qwen2)
// - qwen3.rs: Qwen3 models (uses candle_transformers::models::qwen3)
// - quantized_qwen3.rs: Quantized Qwen3 GGUF models
