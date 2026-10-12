use serde::{Deserialize, Serialize};

/// Supported model configurations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, Hash)]
pub enum ModelConfig {
  /// Qwen3-1.7B model (1.7B params) - GGUF quantized format
  #[default]
  Qwen3_17B,
}

/// A file that has to be downloaded for a model
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelFile {
  /// File name inside the model directory
  pub name: &'static str,
  pub url: &'static str,
  /// Approximate size in bytes, shown before the download reports the real length
  pub approx_size: Option<u32>,
}

const QWEN3_17B_WEIGHTS: ModelFile = ModelFile {
  name: "Qwen3-1.7B-Q8_0.gguf",
  url: "https://huggingface.co/Qwen/Qwen3-1.7B-GGUF/resolve/main/Qwen3-1.7B-Q8_0.gguf",
  approx_size: Some(1_300_000_000),
};

const QWEN3_17B_TOKENIZER: ModelFile = ModelFile {
  name: "tokenizer.json",
  url: "https://huggingface.co/Qwen/Qwen3-1.7B/resolve/main/tokenizer.json",
  approx_size: None,
};

const QWEN3_17B_FILES: &[ModelFile] = &[QWEN3_17B_WEIGHTS, QWEN3_17B_TOKENIZER];

impl ModelConfig {
  /// All supported models
  pub const ALL: &[ModelConfig] = &[ModelConfig::Qwen3_17B];

  /// Name of the model directory inside the `models` cache directory
  pub fn model_id(&self) -> &'static str {
    match self {
      ModelConfig::Qwen3_17B => "qwen3-17b",
    }
  }

  pub fn model_name(&self) -> &'static str {
    match self {
      ModelConfig::Qwen3_17B => "Qwen3-1.7B",
    }
  }

  pub fn model_size(&self) -> &'static str {
    match self {
      ModelConfig::Qwen3_17B => "1.83GB",
    }
  }

  /// Every file the model needs, in download order
  pub fn files(&self) -> &'static [ModelFile] {
    match self {
      ModelConfig::Qwen3_17B => QWEN3_17B_FILES,
    }
  }

  /// The GGUF weights file
  pub fn weights_file(&self) -> &'static ModelFile {
    match self {
      ModelConfig::Qwen3_17B => &QWEN3_17B_WEIGHTS,
    }
  }

  pub fn tokenizer_file(&self) -> &'static ModelFile {
    match self {
      ModelConfig::Qwen3_17B => &QWEN3_17B_TOKENIZER,
    }
  }

  /// Returns the maximum context window size in tokens
  pub fn max_context_tokens(&self) -> usize {
    match self {
      // 32K context window
      ModelConfig::Qwen3_17B => 32_768,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::ModelConfig;

  #[test]
  fn weights_and_tokenizer_are_listed_in_files() {
    for config in ModelConfig::ALL {
      let files = config.files();
      assert!(files.contains(config.weights_file()));
      assert!(files.contains(config.tokenizer_file()));
      assert_ne!(config.weights_file().name, config.tokenizer_file().name);
    }
  }
}
