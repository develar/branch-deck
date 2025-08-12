use crate::BranchNameResult;
use crate::quantized_qwen3::QuantizedQwen3BranchGenerator;
use crate::qwen3::Qwen3BranchGenerator;
use crate::qwen25::Qwen25BranchGenerator;
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
  pub async fn generate_branch_name(&mut self, prompt: &str, max_tokens: usize, is_alternative: bool) -> Result<BranchNameResult> {
    match self {
      GeneratorType::Qwen25(generator) => generator.generate_branch_name(prompt, max_tokens, is_alternative).await,
      GeneratorType::Qwen3(generator) => generator.generate_branch_name(prompt, max_tokens, is_alternative).await,
      GeneratorType::QuantizedQwen3(generator) => generator.generate_branch_name(prompt, max_tokens, is_alternative).await,
    }
  }

  /// Load model from specified directory
  pub async fn load_model(&mut self, model_path: PathBuf) -> Result<()> {
    match self {
      GeneratorType::Qwen25(generator) => generator.load_model(model_path).await,
      GeneratorType::Qwen3(generator) => generator.load_model(model_path).await,
      GeneratorType::QuantizedQwen3(generator) => generator.load_model(model_path).await,
    }
  }

  /// Check if the model is loaded
  pub fn is_loaded(&self) -> bool {
    match self {
      GeneratorType::Qwen25(generator) => generator.is_loaded(),
      GeneratorType::Qwen3(generator) => generator.is_loaded(),
      GeneratorType::QuantizedQwen3(generator) => generator.is_loaded(),
    }
  }

  /// Create a prompt from git output using model-specific formatting
  pub fn create_prompt(&self, git_output: &str) -> Result<String> {
    match self {
      GeneratorType::Qwen25(generator) => generator.create_prompt(git_output),
      GeneratorType::Qwen3(generator) => generator.create_prompt(git_output),
      GeneratorType::QuantizedQwen3(generator) => generator.create_prompt(git_output),
    }
  }

  /// Create an alternative prompt when a previous suggestion exists
  pub fn create_alternative_prompt(&self, git_output: &str, previous_suggestion: &str) -> Result<String> {
    match self {
      GeneratorType::Qwen25(generator) => generator.create_alternative_prompt(git_output, previous_suggestion),
      GeneratorType::Qwen3(generator) => generator.create_alternative_prompt(git_output, previous_suggestion),
      GeneratorType::QuantizedQwen3(generator) => generator.create_alternative_prompt(git_output, previous_suggestion),
    }
  }
}
