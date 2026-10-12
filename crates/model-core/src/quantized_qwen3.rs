use crate::BranchNameResult;
use crate::constants::LOGITS_PROCESSOR_SEED;
use crate::utils::{clean_branch_name, detect_device, truncate_tokens_if_needed};
use anyhow::{Context, Error as E, Result};
use candle_core::quantized::gguf_file;
use candle_core::{Device, Tensor};
use candle_transformers::generation::{LogitsProcessor, Sampling};
use candle_transformers::models::quantized_qwen3::ModelWeights as Qwen3;
use candle_transformers::utils::apply_repeat_penalty;
use std::path::Path;
use tokenizers::Tokenizer;
use tracing::{debug, info, instrument};

/// End-of-turn token of the `ChatML` format used by Qwen3
const EOS_TOKEN: &str = "<|im_end|>";

/// Options for a single [`QuantizedQwen3BranchGenerator::generate_branch_name`] call
pub struct GenerationOptions<'a> {
  /// Maximum number of tokens to generate
  pub max_tokens: usize,
  /// Context window of the model; longer prompts are truncated
  pub max_context_tokens: usize,
  /// Use the higher sampling temperature of the alternative suggestion
  pub is_alternative: bool,
  /// Polled before the prompt pass and before every generated token; `true` stops the generation with
  /// [`GenerationCancelled`]
  pub should_cancel: &'a (dyn Fn() -> bool + Sync),
}

/// Error returned when [`GenerationOptions::should_cancel`] stopped the generation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GenerationCancelled;

impl std::fmt::Display for GenerationCancelled {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str("Generation cancelled")
  }
}

impl std::error::Error for GenerationCancelled {}

/// Quantized Qwen3 model generator for branch names (GGUF format)
pub struct QuantizedQwen3BranchGenerator {
  model: Option<Qwen3>,
  tokenizer: Option<Tokenizer>,
  device: Device,
  repeat_penalty: f32,
  repeat_last_n: usize,
}

// Default temperature for Quantized Qwen3 models
const DEFAULT_TEMPERATURE: f64 = 0.7;
const ALTERNATIVE_TEMPERATURE: f64 = 0.9;

impl QuantizedQwen3BranchGenerator {
  /// Create a new generator instance
  pub fn new() -> Self {
    let device = detect_device();
    Self {
      model: None,
      tokenizer: None,
      device,
      repeat_penalty: 1.1, // Default from candle example
      repeat_last_n: 64,   // Default from candle example
    }
  }

  /// Load the GGUF weights and the tokenizer.
  ///
  /// Blocking: reads the whole weights file.
  #[instrument(skip(self), fields(weights = %weights.display(), tokenizer = %tokenizer.display()))]
  pub fn load_model(&mut self, weights: &Path, tokenizer: &Path) -> Result<()> {
    // Load GGUF model
    debug!("Loading GGUF file...");
    let mut file = std::fs::File::open(weights).with_context(|| format!("Failed to open {}", weights.display()))?;
    let model_content = gguf_file::Content::read(&mut file).map_err(|e| anyhow::anyhow!("Failed to read GGUF: {e}"))?;

    // Log model info
    let mut total_size_in_bytes = 0;
    for tensor in model_content.tensor_infos.values() {
      let elem_count = tensor.shape.elem_count();
      total_size_in_bytes += elem_count * tensor.ggml_dtype.type_size() / tensor.ggml_dtype.block_size();
    }
    info!("GGUF model info: {} tensors, {} bytes total", model_content.tensor_infos.len(), total_size_in_bytes);

    // Load model weights
    debug!("Loading Qwen3 quantized model...");
    let model = Qwen3::from_gguf(model_content, &mut file, &self.device)?;

    // Load tokenizer
    let tokenizer = Tokenizer::from_file(tokenizer).map_err(E::msg)?;

    self.model = Some(model);
    self.tokenizer = Some(tokenizer);

    Ok(())
  }

  /// Generate a branch name from a prompt.
  ///
  /// Blocking: runs the whole inference. Returns [`GenerationCancelled`] (inside the `anyhow::Error`) as soon as
  /// `options.should_cancel` reports `true`.
  #[instrument(skip(self, prompt, options), fields(prompt_len = prompt.len(), max_tokens = options.max_tokens, is_alternative = options.is_alternative))]
  pub fn generate_branch_name(&mut self, prompt: &str, options: &GenerationOptions<'_>) -> Result<BranchNameResult> {
    let GenerationOptions {
      max_tokens,
      max_context_tokens,
      is_alternative,
      should_cancel,
    } = *options;
    let start_time = std::time::Instant::now();

    let model = self.model.as_mut().ok_or_else(|| anyhow::anyhow!("Model not loaded. Call load_model() first."))?;
    let tokenizer = self.tokenizer.as_ref().ok_or_else(|| anyhow::anyhow!("Tokenizer not loaded. Call load_model() first."))?;

    debug!("Generating branch name with {} tokens max", max_tokens);

    // Clear KV cache for fresh context on each generation
    model.clear_kv_cache();

    // Tokenize prompt
    let mut tokens = tokenizer.encode(prompt, true).map_err(E::msg)?.get_ids().to_vec();

    // Check if prompt exceeds context limit and truncate if necessary
    truncate_tokens_if_needed(&mut tokens, max_context_tokens);

    // Setup generation parameters
    let eos_token = tokenizer.token_to_id(EOS_TOKEN).ok_or_else(|| anyhow::anyhow!("Tokenizer has no {EOS_TOKEN} token"))?;

    // Use standard sampling parameters: Temperature=0.7, TopP=0.8, TopK=20, MinP=0 (MinP not supported in candle)
    let temperature = if is_alternative { ALTERNATIVE_TEMPERATURE } else { DEFAULT_TEMPERATURE };
    let sampling = if temperature <= 0.0 {
      Sampling::ArgMax
    } else {
      Sampling::TopKThenTopP { k: 20, p: 0.8, temperature }
    };

    let mut logits_processor = LogitsProcessor::from_sampling(LOGITS_PROCESSOR_SEED, sampling);

    if should_cancel() {
      debug!("Generation cancelled before the prompt pass");
      return Err(GenerationCancelled.into());
    }

    // Process prompt tokens
    let input = Tensor::new(tokens.as_slice(), &self.device)?.unsqueeze(0)?;
    let logits = model.forward(&input, 0)?;
    let logits = logits.squeeze(0)?;

    // Sample first token
    let mut next_token = logits_processor.sample(&logits)?;
    let mut all_tokens = vec![next_token]; // Track all tokens for repeat penalty
    let mut generated_tokens = vec![next_token]; // Track generated tokens for decoding

    // Generation loop - follow candle example pattern
    let timeout_ms = 10_000; // 10 second timeout
    let to_sample = max_tokens.saturating_sub(1);

    for index in 0..to_sample {
      // Check timeout
      if start_time.elapsed().as_millis() > timeout_ms {
        debug!("Generation timeout after {} seconds", timeout_ms / 1000);
        break;
      }

      // Check cancellation
      if should_cancel() {
        debug!("Generation cancelled after {} tokens", generated_tokens.len());
        return Err(GenerationCancelled.into());
      }

      let input = Tensor::new(&[next_token], &self.device)?.unsqueeze(0)?;
      let logits = model.forward(&input, tokens.len() + index)?;
      let logits = logits.squeeze(0)?;

      // Apply repeat penalty like candle example
      let logits = if (self.repeat_penalty - 1.0).abs() < f32::EPSILON {
        logits
      } else {
        let start_at = all_tokens.len().saturating_sub(self.repeat_last_n);
        apply_repeat_penalty(&logits, self.repeat_penalty, &all_tokens[start_at..])?
      };

      // Sample next token
      next_token = logits_processor.sample(&logits)?;
      all_tokens.push(next_token);
      generated_tokens.push(next_token);

      // Check for EOS token
      if next_token == eos_token {
        debug!("EOS token encountered, stopping generation");
        break;
      }
    }

    // Decode the generated tokens
    let generated_text = tokenizer.decode(&generated_tokens, true).map_err(E::msg)?;

    debug!("Generated text: '{}' from {} tokens", generated_text, generated_tokens.len());

    let generation_time = start_time.elapsed();

    // Clean up the generated branch name
    let cleaned_name = clean_branch_name(&generated_text)?;

    let result = BranchNameResult {
      name: cleaned_name,
      generation_time_ms: u64::try_from(generation_time.as_millis()).unwrap_or(u64::MAX),
    };

    debug!("Generated branch name: '{}' in {}ms", result.name, result.generation_time_ms);

    Ok(result)
  }

  /// Check if model is loaded and ready
  pub fn is_loaded(&self) -> bool {
    self.model.is_some() && self.tokenizer.is_some()
  }

  /// Create model-specific prompt for quantized Qwen3 models
  /// Uses `ChatML` format which works better with quantized models
  pub fn create_prompt(&self, git_output: &str) -> anyhow::Result<String> {
    crate::prompt::create_chatml_prompt(git_output, None)
  }

  /// Create alternative prompt when a previous suggestion exists
  /// Uses `ChatML` format with modified system prompt
  pub fn create_alternative_prompt(&self, git_output: &str, previous_suggestion: &str) -> anyhow::Result<String> {
    crate::prompt::create_chatml_prompt(git_output, Some(previous_suggestion))
  }
}

impl Default for QuantizedQwen3BranchGenerator {
  fn default() -> Self {
    Self::new()
  }
}

impl std::fmt::Debug for QuantizedQwen3BranchGenerator {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("QuantizedQwen3BranchGenerator")
      .field("model_loaded", &self.model.is_some())
      .field("tokenizer_loaded", &self.tokenizer.is_some())
      .field("device", &format!("{:?}", self.device))
      .field("repeat_penalty", &self.repeat_penalty)
      .field("repeat_last_n", &self.repeat_last_n)
      .finish()
  }
}
