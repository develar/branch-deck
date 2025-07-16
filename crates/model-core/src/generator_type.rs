use crate::{BranchNameResult, QuantizedQwen3BranchGenerator, Qwen25BranchGenerator, Qwen3BranchGenerator};
use anyhow::Result;
use std::path::PathBuf;

/// Unified generator enum to support different Qwen architectures
#[derive(Debug)]
pub enum GeneratorType {
  Qwen25(Qwen25BranchGenerator),
  Qwen3(Qwen3BranchGenerator),
  QuantizedQwen3(QuantizedQwen3BranchGenerator),
}

impl GeneratorType {
  /// Generate a branch name from a prompt
  pub async fn generate_branch_name(&mut self, prompt: &str, max_tokens: usize, temperature: f64) -> Result<BranchNameResult> {
    match self {
      GeneratorType::Qwen25(gen) => gen.generate_branch_name(prompt, max_tokens, temperature).await,
      GeneratorType::Qwen3(gen) => gen.generate_branch_name(prompt, max_tokens, temperature).await,
      GeneratorType::QuantizedQwen3(gen) => gen.generate_branch_name(prompt, max_tokens, temperature).await,
    }
  }

  /// Load model from specified directory
  pub async fn load_model(&mut self, model_path: PathBuf) -> Result<()> {
    match self {
      GeneratorType::Qwen25(gen) => gen.load_model(model_path).await,
      GeneratorType::Qwen3(gen) => gen.load_model(model_path).await,
      GeneratorType::QuantizedQwen3(gen) => gen.load_model(model_path).await,
    }
  }

  /// Check if the model is loaded
  pub fn is_loaded(&self) -> bool {
    match self {
      GeneratorType::Qwen25(gen) => gen.is_loaded(),
      GeneratorType::Qwen3(gen) => gen.is_loaded(),
      GeneratorType::QuantizedQwen3(gen) => gen.is_loaded(),
    }
  }

  /// Create a prompt from git output using model-specific formatting
  pub fn create_prompt(&self, git_output: &str) -> Result<String> {
    match self {
      GeneratorType::Qwen25(gen) => gen.create_prompt(git_output),
      GeneratorType::Qwen3(gen) => gen.create_prompt(git_output),
      GeneratorType::QuantizedQwen3(gen) => gen.create_prompt(git_output),
    }
  }

  /// Count tokens in text (if tokenizer is available)
  pub fn count_tokens(&self, text: &str) -> Option<usize> {
    match self {
      GeneratorType::Qwen25(gen) => gen.count_tokens(text),
      GeneratorType::Qwen3(gen) => gen.count_tokens(text),
      GeneratorType::QuantizedQwen3(gen) => gen.count_tokens(text),
    }
  }
}
