use serde::{Deserialize, Serialize};

/// Supported Qwen model configurations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, Hash)]
pub enum ModelConfig {
  /// Qwen3-1.7B model (1.7B params) - GGUF quantized format
  #[default]
  Qwen3_17B,
  /// Qwen2.5-Coder 1.5B model (1.5B params) - Advanced mode, professional features
  Qwen25Coder15B,
  /// Qwen2.5-Coder 3B model (3B params) - Best quality but large
  Qwen25Coder3B,
}

impl ModelConfig {
  pub fn model_id(&self) -> &'static str {
    match self {
      ModelConfig::Qwen25Coder15B => "qwen25-coder-15b",
      ModelConfig::Qwen25Coder3B => "qwen25-coder-3b",
      ModelConfig::Qwen3_17B => "qwen3-17b",
    }
  }

  pub fn model_name(&self) -> &'static str {
    match self {
      ModelConfig::Qwen25Coder15B => "Qwen2.5-Coder-1.5B",
      ModelConfig::Qwen25Coder3B => "Qwen2.5-Coder-3B",
      ModelConfig::Qwen3_17B => "Qwen3-1.7B",
    }
  }

  pub fn model_size(&self) -> &'static str {
    match self {
      ModelConfig::Qwen25Coder15B => "1.5GB",
      ModelConfig::Qwen25Coder3B => "3GB",
      ModelConfig::Qwen3_17B => "1.83GB",
    }
  }

  pub fn download_urls(&self) -> Vec<(&'static str, &'static str, Option<u32>)> {
    match self {
      ModelConfig::Qwen25Coder15B => vec![
        ("config.json", "https://huggingface.co/Qwen/Qwen2.5-Coder-1.5B-Instruct/resolve/main/config.json", None),
        (
          "model.safetensors",
          "https://huggingface.co/Qwen/Qwen2.5-Coder-1.5B-Instruct/resolve/main/model.safetensors",
          Some(1_500_000_000),
        ),
        (
          "tokenizer.json",
          "https://huggingface.co/Qwen/Qwen2.5-Coder-1.5B-Instruct/resolve/main/tokenizer.json",
          None,
        ),
        (
          "tokenizer_config.json",
          "https://huggingface.co/Qwen/Qwen2.5-Coder-1.5B-Instruct/resolve/main/tokenizer_config.json",
          None,
        ),
        ("merges.txt", "https://huggingface.co/Qwen/Qwen2.5-Coder-1.5B-Instruct/resolve/main/merges.txt", None),
      ],
      ModelConfig::Qwen25Coder3B => vec![
        ("config.json", "https://huggingface.co/Qwen/Qwen2.5-Coder-3B-Instruct/resolve/main/config.json", None),
        (
          "model.safetensors",
          "https://huggingface.co/Qwen/Qwen2.5-Coder-3B-Instruct/resolve/main/model.safetensors",
          Some(3_000_000_000),
        ),
        ("tokenizer.json", "https://huggingface.co/Qwen/Qwen2.5-Coder-3B-Instruct/resolve/main/tokenizer.json", None),
        (
          "tokenizer_config.json",
          "https://huggingface.co/Qwen/Qwen2.5-Coder-3B-Instruct/resolve/main/tokenizer_config.json",
          None,
        ),
        ("merges.txt", "https://huggingface.co/Qwen/Qwen2.5-Coder-3B-Instruct/resolve/main/merges.txt", None),
      ],

      ModelConfig::Qwen3_17B => vec![
        (
          "Qwen3-1.7B-Q8_0.gguf",
          "https://huggingface.co/Qwen/Qwen3-1.7B-GGUF/resolve/main/Qwen3-1.7B-Q8_0.gguf",
          Some(1_300_000_000), // 1.3GB
        ),
        ("tokenizer.json", "https://huggingface.co/Qwen/Qwen3-1.7B/resolve/main/tokenizer.json", None),
      ],
    }
  }

  pub fn is_qwen3_architecture(&self) -> bool {
    matches!(self, ModelConfig::Qwen3_17B)
  }

  pub fn is_qwen25_architecture(&self) -> bool {
    matches!(self, ModelConfig::Qwen25Coder15B | ModelConfig::Qwen25Coder3B)
  }

  pub fn is_gguf_format(&self) -> bool {
    matches!(self, ModelConfig::Qwen3_17B)
  }

  /// Returns the maximum context window size in tokens for each model
  pub fn max_context_tokens(&self) -> usize {
    match self {
      ModelConfig::Qwen25Coder15B => 32_768, // 32K context window
      ModelConfig::Qwen25Coder3B => 32_768,  // 32K context window
      ModelConfig::Qwen3_17B => 32_768,      // 32K context window
    }
  }
}
