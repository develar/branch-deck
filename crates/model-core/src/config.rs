use serde::{Deserialize, Serialize};

/// Supported model configurations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, Hash)]
pub enum ModelConfig {
  /// Qwen3-4B-Instruct-2507 (4B params, non-thinking) - GGUF `Q4_K_M` quantized format
  #[default]
  Qwen3_4BInstruct2507,
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

// Qwen publishes no official GGUF for this model; candle's own examples use the unsloth GGUFs
const QWEN3_4B_INSTRUCT_2507_WEIGHTS: ModelFile = ModelFile {
  name: "Qwen3-4B-Instruct-2507-Q4_K_M.gguf",
  url: "https://huggingface.co/unsloth/Qwen3-4B-Instruct-2507-GGUF/resolve/main/Qwen3-4B-Instruct-2507-Q4_K_M.gguf",
  approx_size: Some(2_497_281_120),
};

const QWEN3_4B_INSTRUCT_2507_TOKENIZER: ModelFile = ModelFile {
  name: "tokenizer.json",
  url: "https://huggingface.co/Qwen/Qwen3-4B-Instruct-2507/resolve/main/tokenizer.json",
  approx_size: Some(11_422_654),
};

const QWEN3_4B_INSTRUCT_2507_FILES: &[ModelFile] = &[QWEN3_4B_INSTRUCT_2507_WEIGHTS, QWEN3_4B_INSTRUCT_2507_TOKENIZER];

impl ModelConfig {
  /// All supported models
  pub const ALL: &[ModelConfig] = &[ModelConfig::Qwen3_4BInstruct2507];

  /// Name of the model directory inside the `models` cache directory
  pub fn model_id(&self) -> &'static str {
    match self {
      ModelConfig::Qwen3_4BInstruct2507 => "qwen3-4b-instruct-2507",
    }
  }

  pub fn model_name(&self) -> &'static str {
    match self {
      ModelConfig::Qwen3_4BInstruct2507 => "Qwen3-4B-Instruct-2507",
    }
  }

  /// Human-readable total download size of [`Self::files`], e.g. "2.5 GB"
  pub fn model_size(&self) -> String {
    let total_bytes: u64 = self.files().iter().filter_map(|file| file.approx_size).map(u64::from).sum();
    // Tenths of a GB, rounded to the nearest
    let tenths = (total_bytes + 50_000_000) / 100_000_000;
    format!("{}.{} GB", tenths / 10, tenths % 10)
  }

  /// Model card on Hugging Face
  pub fn model_page_url(&self) -> &'static str {
    match self {
      ModelConfig::Qwen3_4BInstruct2507 => "https://huggingface.co/Qwen/Qwen3-4B-Instruct-2507",
    }
  }

  /// Every file the model needs, in download order
  pub fn files(&self) -> &'static [ModelFile] {
    match self {
      ModelConfig::Qwen3_4BInstruct2507 => QWEN3_4B_INSTRUCT_2507_FILES,
    }
  }

  /// The GGUF weights file
  pub fn weights_file(&self) -> &'static ModelFile {
    match self {
      ModelConfig::Qwen3_4BInstruct2507 => &QWEN3_4B_INSTRUCT_2507_WEIGHTS,
    }
  }

  pub fn tokenizer_file(&self) -> &'static ModelFile {
    match self {
      ModelConfig::Qwen3_4BInstruct2507 => &QWEN3_4B_INSTRUCT_2507_TOKENIZER,
    }
  }

  /// Returns the maximum context window size in tokens
  pub fn max_context_tokens(&self) -> usize {
    match self {
      // The model supports 262K tokens; the 32K cap bounds KV-cache memory and the prompt truncation
      ModelConfig::Qwen3_4BInstruct2507 => 32_768,
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
      assert!(files.iter().all(|file| file.approx_size.is_some()), "every file of {config:?} has an approximate size");
    }
  }

  #[test]
  fn model_size_is_the_sum_of_file_sizes() {
    assert_eq!(ModelConfig::Qwen3_4BInstruct2507.model_size(), "2.5 GB");
  }
}
