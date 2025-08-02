use crate::constants::LOGITS_PROCESSOR_SEED;
use crate::utils::{clean_branch_name, detect_device, truncate_tokens_if_needed};
use crate::BranchNameResult;
use anyhow::{Error as E, Result};
use candle_core::{DType, Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::generation::{LogitsProcessor, Sampling};
use candle_transformers::models::qwen2::{Config as QwenConfig, ModelForCausalLM as QwenModel};
use std::path::PathBuf;
use tokenizers::Tokenizer;
use tracing::{debug, instrument};

/// Qwen2.5-Coder model generator for branch names
pub struct Qwen25BranchGenerator {
  model: Option<QwenModel>,
  tokenizer: Option<Tokenizer>,
  device: Device,
  dtype: DType,
}

// Default temperature for Qwen2.5 models
const DEFAULT_TEMPERATURE: f64 = 0.7;
const ALTERNATIVE_TEMPERATURE: f64 = 0.9;

impl Qwen25BranchGenerator {
  /// Create a new generator instance
  pub fn new() -> Self {
    let device = detect_device();
    Self {
      model: None,
      tokenizer: None,
      device,
      dtype: DType::F32,
    }
  }

  /// Load model from specified directory (expects Qwen2.5-Coder format)
  #[instrument(skip(self), fields(model_path = %model_path.display()))]
  pub async fn load_model(&mut self, model_path: PathBuf) -> Result<()> {
    let tokenizer_path = model_path.join("tokenizer.json");
    let config_path = model_path.join("config.json");
    let model_file = model_path.join("model.safetensors");

    // Verify all required files exist
    if !tokenizer_path.exists() || !config_path.exists() || !model_file.exists() {
      return Err(anyhow::anyhow!("Model files incomplete. Expected: tokenizer.json, config.json, model.safetensors"));
    }

    // Load config
    let config: QwenConfig = serde_json::from_slice(&std::fs::read(config_path)?)?;
    debug!("Model config loaded: {} layers, {} vocab", config.num_hidden_layers, config.vocab_size);

    // Load weights
    let vb = unsafe { VarBuilder::from_mmaped_safetensors(&[model_file], self.dtype, &self.device)? };

    // Load model
    debug!("Loading Qwen2 model...");
    let model = QwenModel::new(&config, vb)?;

    // Load tokenizer
    let tokenizer = Tokenizer::from_file(tokenizer_path).map_err(E::msg)?;

    self.model = Some(model);
    self.tokenizer = Some(tokenizer);

    Ok(())
  }

  /// Generate branch name from commit message and optional diff
  #[instrument(skip(self, prompt), fields(prompt_len = prompt.len()))]
  pub async fn generate_branch_name(&mut self, prompt: &str, max_tokens: usize, is_alternative: bool) -> Result<BranchNameResult> {
    let start_time = std::time::Instant::now();

    let model = self.model.as_mut().ok_or_else(|| anyhow::anyhow!("Model not loaded. Call load_model() first."))?;

    let tokenizer = self.tokenizer.as_ref().ok_or_else(|| anyhow::anyhow!("Tokenizer not loaded. Call load_model() first."))?;

    debug!("Generating branch name with {} tokens max", max_tokens);

    // Clear KV cache for fresh context on each generation
    model.clear_kv_cache();

    // Tokenize prompt
    let mut tokens = tokenizer.encode(prompt, true).map_err(E::msg)?.get_ids().to_vec();

    // Check if prompt exceeds context limit and truncate if necessary
    const MAX_CONTEXT_TOKENS: usize = 32_768; // 32K context window for Qwen2.5
    truncate_tokens_if_needed(&mut tokens, MAX_CONTEXT_TOKENS);

    let prompt_len = tokens.len();
    debug!("Prompt tokens: {}", prompt_len);

    // Setup generation parameters
    let eos_token_ids = [tokenizer.token_to_id("<|endoftext|>"), tokenizer.token_to_id("<|im_end|>")];

    let temperature = if is_alternative { ALTERNATIVE_TEMPERATURE } else { DEFAULT_TEMPERATURE };
    let sampling = if temperature <= 0.0 { Sampling::ArgMax } else { Sampling::All { temperature } };

    let mut logits_processor = LogitsProcessor::from_sampling(LOGITS_PROCESSOR_SEED, sampling);

    // Generation loop with timeout protection
    let mut generated_tokens = 0;
    let timeout_ms = 10_000; // 10 second timeout

    for _index in 0..max_tokens {
      // Check timeout
      if start_time.elapsed().as_millis() > timeout_ms {
        debug!("Generation timeout after {} seconds", timeout_ms / 1000);
        break;
      }

      let context_size = if generated_tokens > 0 { 1 } else { tokens.len() };
      let start_pos = tokens.len().saturating_sub(context_size);
      let ctxt = &tokens[start_pos..];
      let input = Tensor::new(ctxt, &self.device)?.unsqueeze(0)?;

      // Forward pass
      let logits = model.forward(&input, start_pos)?;
      let logits = logits.squeeze(0)?.squeeze(0)?.to_dtype(DType::F32)?;

      // Sample next token
      let next_token = logits_processor.sample(&logits)?;
      generated_tokens += 1;
      tokens.push(next_token);

      // Check for EOS tokens
      if eos_token_ids.contains(&Some(next_token)) {
        debug!("EOS token encountered, stopping generation");
        break;
      }

      // Early stopping for newlines or common separators in branch names
      if let Ok(token_text) = tokenizer.decode(&[next_token], false) {
        if token_text.contains('\n') || token_text.contains("```") {
          debug!("Found separator, stopping generation");
          break;
        }
      }
    }

    // Decode generated text
    let generated_text = if generated_tokens > 0 {
      tokenizer.decode(&tokens[prompt_len..], true).map_err(E::msg)?
    } else {
      String::new()
    };

    let generation_time = start_time.elapsed();

    // Clean up the generated branch name
    let cleaned_name = clean_branch_name(&generated_text)?;

    let result = BranchNameResult {
      name: cleaned_name,
      generation_time_ms: generation_time.as_millis() as u64,
    };

    debug!("Generated branch name: '{}' in {}ms", result.name, result.generation_time_ms);

    Ok(result)
  }

  /// Check if model is loaded and ready
  pub fn is_loaded(&self) -> bool {
    self.model.is_some() && self.tokenizer.is_some()
  }

  /// Create model-specific prompt for Qwen2.5 models
  /// Uses generic format which has proven to work well with Qwen2.5 architecture
  pub fn create_prompt(&self, git_output: &str) -> anyhow::Result<String> {
    crate::prompt::create_generic_prompt(git_output)
  }

  /// Create alternative prompt when a previous suggestion exists
  pub fn create_alternative_prompt(&self, git_output: &str, previous_suggestion: &str) -> anyhow::Result<String> {
    crate::prompt::create_generic_alternative_prompt(git_output, previous_suggestion)
  }
}

impl Default for Qwen25BranchGenerator {
  fn default() -> Self {
    Self::new()
  }
}

impl std::fmt::Debug for Qwen25BranchGenerator {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("Qwen25BranchGenerator")
      .field("model_loaded", &self.model.is_some())
      .field("tokenizer_loaded", &self.tokenizer.is_some())
      .field("device", &format!("{:?}", self.device))
      .field("dtype", &format!("{:?}", self.dtype))
      .finish()
  }
}
