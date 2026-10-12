use crate::path_provider::ModelPathProvider;
use anyhow::{Context, Result};
use model_core::BranchNameResult;
use model_core::config::ModelConfig;
use model_core::quantized_qwen3::{GenerationOptions, QuantizedQwen3BranchGenerator};
use std::path::PathBuf;
use tracing::{info, instrument};

/// A branch name is one line of at most 50 characters; the model has no thinking mode that needs more room
pub const MAX_NEW_TOKENS: usize = 128;

/// Error returned by [`ModelBasedBranchGenerator::ensure_model_loaded`] when a model file is not downloaded yet
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelNotDownloaded {
  /// Name of the first missing file
  pub missing: &'static str,
}

impl std::fmt::Display for ModelNotDownloaded {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "Model not downloaded: missing {}", self.missing)
  }
}

impl std::error::Error for ModelNotDownloaded {}

/// Directory that holds the files of `config`: `<cache dir>/models/<model id>`
pub fn model_dir(provider: &dyn ModelPathProvider, config: ModelConfig) -> Result<PathBuf> {
  let cache_dir = provider.get_cache_dir().context("Failed to get cache dir")?;
  Ok(cache_dir.join("models").join(config.model_id()))
}

/// Core model-based branch name generator without git dependencies.
///
/// All methods are blocking (model loading and inference); call them from a blocking thread.
#[derive(Debug)]
pub struct ModelBasedBranchGenerator {
  generator: QuantizedQwen3BranchGenerator,
  model_path: Option<PathBuf>,
  model_config: ModelConfig,
}

impl ModelBasedBranchGenerator {
  pub fn new() -> Result<Self> {
    Self::with_config(ModelConfig::default())
  }

  pub fn with_config(model_config: ModelConfig) -> Result<Self> {
    Ok(Self {
      generator: QuantizedQwen3BranchGenerator::new(),
      model_path: None,
      model_config,
    })
  }

  /// Get the path where the model files are stored
  pub fn get_model_path(&mut self, provider: &dyn ModelPathProvider) -> Result<PathBuf> {
    if let Some(ref path) = self.model_path {
      return Ok(path.clone());
    }

    let path = model_dir(provider, self.model_config)?;
    self.model_path = Some(path.clone());
    Ok(path)
  }

  pub fn get_model_config(&self) -> ModelConfig {
    self.model_config
  }

  /// Load the model unless it is already loaded.
  ///
  /// Fails with [`ModelNotDownloaded`] (inside the `anyhow::Error`) when a model file is missing.
  #[instrument(skip(self, provider), fields(model = self.model_config.model_name()))]
  pub fn ensure_model_loaded(&mut self, provider: &dyn ModelPathProvider) -> Result<()> {
    if self.generator.is_loaded() {
      return Ok(());
    }

    let model_path = self.get_model_path(provider)?;
    std::fs::create_dir_all(&model_path).context("Failed to create model directory")?;

    if let Some(file) = self.model_config.files().iter().find(|file| !model_path.join(file.name).exists()) {
      info!("Model files not found: missing {}", file.name);
      return Err(anyhow::Error::new(ModelNotDownloaded { missing: file.name }));
    }

    info!("Loading {} model for branch name generation", self.model_config.model_name());
    let weights = model_path.join(self.model_config.weights_file().name);
    let tokenizer = model_path.join(self.model_config.tokenizer_file().name);
    self
      .generator
      .load_model(&weights, &tokenizer)
      .with_context(|| format!("Failed to load {} model", self.model_config.model_name()))?;
    info!("{} model loaded successfully", self.model_config.model_name());
    Ok(())
  }

  pub fn is_loaded(&self) -> bool {
    self.generator.is_loaded()
  }

  /// Create an enhanced prompt from raw git output using model-specific formatting
  fn create_enhanced_prompt(&self, git_output: &str, previous_suggestion: Option<&str>) -> Result<String> {
    let prompt = match previous_suggestion {
      None => self.generator.create_prompt(git_output.trim())?,
      Some(prev) => self.generator.create_alternative_prompt(git_output.trim(), prev)?,
    };

    info!(
      "Generated {} prompt with {} characters (model: {})",
      if previous_suggestion.is_some() { "alternative" } else { "primary" },
      prompt.len(),
      self.model_config.model_name()
    );

    Ok(prompt)
  }

  /// Generate a branch name from git output
  ///
  /// # Arguments
  /// * `git_output` - The raw git changes output
  /// * `previous_suggestion` - Optional previous suggestion to generate an alternative
  /// * `should_cancel` - Polled during the generation; `true` stops it with
  ///   [`model_core::quantized_qwen3::GenerationCancelled`]
  pub fn generate_branch_name(&mut self, git_output: &str, previous_suggestion: Option<&str>, should_cancel: &(dyn Fn() -> bool + Sync)) -> Result<BranchNameResult> {
    if !self.generator.is_loaded() {
      return Err(anyhow::anyhow!("Model not loaded"));
    }

    let prompt = self.create_enhanced_prompt(git_output, previous_suggestion)?;
    let options = GenerationOptions {
      max_tokens: MAX_NEW_TOKENS,
      max_context_tokens: self.model_config.max_context_tokens(),
      is_alternative: previous_suggestion.is_some(),
      should_cancel,
    };
    self.generator.generate_branch_name(&prompt, &options)
  }
}
