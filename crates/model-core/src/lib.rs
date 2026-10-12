pub mod config;
mod constants;
pub mod prompt;
pub mod quantized_qwen3;
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
