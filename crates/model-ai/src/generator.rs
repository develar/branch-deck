use crate::path_provider::ModelPathProvider;
use anyhow::{Context, Result};
use model_core::{BranchNameResult, GeneratorType, ModelConfig, QuantizedQwen3BranchGenerator, Qwen25BranchGenerator, Qwen3BranchGenerator};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;

// Generation constants - standard parameters: Temperature=0.7, TopP=0.8, TopK=20, MinP=0
pub const MAX_NEW_TOKENS: usize = 1000; // Increased to allow for thinking tags and complete generation

// Confidence scores for suggestions

/// Model loading state to prevent race conditions
#[derive(Debug, Clone)]
enum ModelLoadingState {
  NotLoaded,
  Loading,
  Loaded,
  Failed(String),
}

// QwenGenerator is replaced by GeneratorType from model_core

/// Core model-based branch name generator without git dependencies
#[derive(Debug)]
pub struct ModelBasedBranchGenerator {
  generator: Option<GeneratorType>,
  model_path: Option<PathBuf>,
  model_config: ModelConfig,
  loading_state: Arc<Mutex<ModelLoadingState>>,
}

impl ModelBasedBranchGenerator {
  pub fn new() -> Result<Self> {
    Self::with_config(ModelConfig::default())
  }

  pub fn with_config(model_config: ModelConfig) -> Result<Self> {
    // All models in the simplified ModelConfig are Qwen models

    Ok(Self {
      generator: None,
      model_path: None,
      model_config,
      loading_state: Arc::new(Mutex::new(ModelLoadingState::NotLoaded)),
    })
  }

  /// Get the path where the model files are stored
  /// Creates the path structure if it doesn't exist
  pub fn get_model_path(&mut self, provider: &dyn ModelPathProvider) -> Result<PathBuf> {
    if let Some(ref path) = self.model_path {
      return Ok(path.clone());
    }

    let cache_dir = provider.get_cache_dir().unwrap_or_else(|e| {
      tracing::warn!("Failed to get cache dir: {}, using current directory", e);
      std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    });

    let path = cache_dir.join("models").join(self.model_config.model_id());

    self.model_path = Some(path.clone());
    Ok(path)
  }

  pub fn get_model_config(&self) -> ModelConfig {
    self.model_config
  }

  /// Set the model configuration - for developer use
  /// This will require re-downloading the model if it's different
  pub async fn set_model_config(&mut self, config: ModelConfig) -> Result<()> {
    if self.model_config != config {
      // Clear loaded model when switching
      self.generator = None;
      self.model_path = None;
      self.model_config = config;

      // Reset loading state
      let mut loading_state = self.loading_state.lock().await;
      *loading_state = ModelLoadingState::NotLoaded;

      info!("Model configuration changed to {}", config.model_name());
    }
    Ok(())
  }

  pub async fn ensure_model_loaded(&mut self, provider: &dyn ModelPathProvider) -> Result<()> {
    // Quick check: if already loaded, return immediately
    if let Some(ref generator) = self.generator {
      if generator.is_loaded() {
        let mut loading_state = self.loading_state.lock().await;
        *loading_state = ModelLoadingState::Loaded;
        return Ok(());
      }
    }

    // Check loading state with coordination
    loop {
      let mut loading_state = self.loading_state.lock().await;

      match &*loading_state {
        ModelLoadingState::Loaded => {
          // Another thread loaded it - verify our generator is set
          if let Some(ref generator) = self.generator {
            if generator.is_loaded() {
              return Ok(());
            }
          }
          // Fall through to reload if generator is inconsistent
          *loading_state = ModelLoadingState::NotLoaded;
        }

        ModelLoadingState::Loading => {
          // Another thread is loading - drop lock and wait
          drop(loading_state);
          tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
          continue;
        }

        ModelLoadingState::Failed(error) => {
          let error_msg = error.clone();
          *loading_state = ModelLoadingState::NotLoaded; // Reset for retry
          return Err(anyhow::anyhow!("Model loading previously failed: {}", error_msg));
        }

        ModelLoadingState::NotLoaded => {
          // We can attempt to load - set loading state and break
          *loading_state = ModelLoadingState::Loading;
          break;
        }
      }
    }

    // We have the loading lock - attempt to load the model
    info!("Loading {} model for branch name generation", self.model_config.model_name());

    let result = self.load_model_internal(provider).await;

    // Update loading state based on result
    let mut loading_state = self.loading_state.lock().await;
    match result {
      Ok(()) => {
        *loading_state = ModelLoadingState::Loaded;
        info!("{} model loaded successfully", self.model_config.model_name());
        Ok(())
      }
      Err(ref error) => {
        *loading_state = ModelLoadingState::Failed(error.to_string());
        Err(anyhow::anyhow!("Failed to load model: {}", error))
      }
    }
  }

  /// Internal model loading logic without state coordination
  async fn load_model_internal(&mut self, provider: &dyn ModelPathProvider) -> Result<()> {
    let model_path = self.get_model_path(provider)?;

    // Create model directory if it doesn't exist
    tokio::fs::create_dir_all(&model_path).await.context("Failed to create model directory")?;

    // Check for required model files based on config
    let required_files = if self.model_config.is_gguf_format() {
      // GGUF models need different files
      match self.model_config {
        ModelConfig::Qwen3_17B => vec!["Qwen3-1.7B-Q8_0.gguf", "tokenizer.json"],
        _ => vec!["model.gguf", "tokenizer.json"],
      }
    } else {
      // SafeTensors models need these files
      vec!["config.json", "model.safetensors", "tokenizer.json"]
    };

    for file in &required_files {
      if !model_path.join(file).exists() {
        info!("Model files not found: missing {}", file);
        return Err(anyhow::anyhow!("Model not downloaded: missing {}", file));
      }
    }

    // Route to correct generator based on model architecture and format
    if self.model_config.is_qwen3_architecture() {
      if self.model_config.is_gguf_format() {
        // Load GGUF Qwen3 models using QuantizedQwen3BranchGenerator
        let mut generator = QuantizedQwen3BranchGenerator::new();
        generator.load_model(model_path).await.context("Failed to load Quantized Qwen3 model")?;
        self.generator = Some(GeneratorType::QuantizedQwen3(generator));
      } else {
        // Load SafeTensors Qwen3 models using Qwen3BranchGenerator
        let mut generator = Qwen3BranchGenerator::new();
        generator.load_model(model_path).await.context("Failed to load Qwen3 model")?;
        self.generator = Some(GeneratorType::Qwen3(generator));
      }
    } else if self.model_config.is_qwen25_architecture() {
      if self.model_config.is_gguf_format() {
        return Err(anyhow::anyhow!("GGUF format not supported for Qwen2.5 models - use SafeTensors format"));
      }
      // Load Qwen2.5 models using Qwen25BranchGenerator
      let mut generator = Qwen25BranchGenerator::new();
      generator.load_model(model_path).await.context("Failed to load Qwen2.5 model")?;
      self.generator = Some(GeneratorType::Qwen25(generator));
    } else {
      return Err(anyhow::anyhow!("Unsupported model architecture: {}", self.model_config.model_name()));
    }

    Ok(())
  }

  pub fn is_loaded(&self) -> bool {
    self.generator.as_ref().is_some_and(|g| g.is_loaded())
  }

  /// Create an enhanced prompt from raw git output using model-specific formatting
  /// This is now internal - external callers should use generate_branch_name directly
  async fn create_enhanced_prompt(&self, git_output: &str, previous_suggestion: Option<&str>) -> Result<String> {
    let generator = self.generator.as_ref().ok_or_else(|| anyhow::anyhow!("Model not loaded"))?;

    let prompt = match previous_suggestion {
      None => generator.create_prompt(git_output.trim())?,
      Some(prev) => generator.create_alternative_prompt(git_output.trim(), prev)?,
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
  pub async fn generate_branch_name(&mut self, git_output: &str, previous_suggestion: Option<&str>) -> Result<BranchNameResult> {
    // Create appropriate prompt based on whether this is an alternative
    let prompt = self.create_enhanced_prompt(git_output, previous_suggestion).await?;

    // Get mutable reference to generator after creating prompt
    let generator = self.generator.as_mut().ok_or_else(|| anyhow::anyhow!("Model not loaded"))?;

    generator.generate_branch_name(&prompt, MAX_NEW_TOKENS, previous_suggestion.is_some()).await
  }
}
