use crate::BranchNameResult;
use crate::constants::LOGITS_PROCESSOR_SEED;
use crate::utils::{clean_branch_name, detect_device, truncate_tokens_if_needed};
use anyhow::{Error as E, Result};
use candle_core::quantized::gguf_file;
use candle_core::{Device, Tensor};
use candle_transformers::generation::{LogitsProcessor, Sampling};
use candle_transformers::models::quantized_qwen3::ModelWeights as Qwen3;
use candle_transformers::utils::apply_repeat_penalty;
use std::path::PathBuf;
use tokenizers::Tokenizer;
use tracing::{debug, info, instrument};

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

  /// Load model from specified directory (expects GGUF format)
  #[instrument(skip(self), fields(model_path = %model_path.display()))]
  pub async fn load_model(&mut self, model_path: PathBuf) -> Result<()> {
    let tokenizer_path = model_path.join("tokenizer.json");
    let model_file = model_path.join("Qwen3-1.7B-Q8_0.gguf");

    // Verify all required files exist
    if !tokenizer_path.exists() || !model_file.exists() {
      return Err(anyhow::anyhow!("Model files incomplete. Expected: tokenizer.json, Qwen3-1.7B-Q8_0.gguf"));
    }

    // Load GGUF model
    debug!("Loading GGUF file...");
    let mut file = std::fs::File::open(&model_file)?;
    let model_content = gguf_file::Content::read(&mut file).map_err(|e| anyhow::anyhow!("Failed to read GGUF: {}", e))?;

    // Log model info
    let mut total_size_in_bytes = 0;
    for (_, tensor) in model_content.tensor_infos.iter() {
      let elem_count = tensor.shape.elem_count();
      total_size_in_bytes += elem_count * tensor.ggml_dtype.type_size() / tensor.ggml_dtype.block_size();
    }
    info!("GGUF model info: {} tensors, {} bytes total", model_content.tensor_infos.len(), total_size_in_bytes);

    // Load model weights
    debug!("Loading Qwen3 quantized model...");
    let model = Qwen3::from_gguf(model_content, &mut file, &self.device)?;

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

    // Format prompt with /no_think and explicit instruction for brevity
    // Tokenize prompt
    let mut tokens = tokenizer.encode(prompt, true).map_err(E::msg)?.get_ids().to_vec();

    // Check if prompt exceeds context limit and truncate if necessary
    const MAX_CONTEXT_TOKENS: usize = 32_768; // 32K context window for Qwen3
    truncate_tokens_if_needed(&mut tokens, MAX_CONTEXT_TOKENS);

    // Setup generation parameters
    let eos_token = *tokenizer.get_vocab(true).get("<|im_end|>").unwrap_or(&0);

    // Use standard sampling parameters: Temperature=0.7, TopP=0.8, TopK=20, MinP=0 (MinP not supported in candle)
    let temperature = if is_alternative { ALTERNATIVE_TEMPERATURE } else { DEFAULT_TEMPERATURE };
    let sampling = if temperature <= 0.0 {
      Sampling::ArgMax
    } else {
      Sampling::TopKThenTopP { k: 20, p: 0.8, temperature }
    };

    let mut logits_processor = LogitsProcessor::from_sampling(LOGITS_PROCESSOR_SEED, sampling);

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

      let input = Tensor::new(&[next_token], &self.device)?.unsqueeze(0)?;
      let logits = model.forward(&input, tokens.len() + index)?;
      let logits = logits.squeeze(0)?;

      // Apply repeat penalty like candle example
      let logits = if self.repeat_penalty == 1.0 {
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
      generation_time_ms: generation_time.as_millis() as u64,
    };

    debug!("Generated branch name: '{}' in {}ms", result.name, result.generation_time_ms);

    Ok(result)
  }

  /// Check if model is loaded and ready
  pub fn is_loaded(&self) -> bool {
    self.model.is_some() && self.tokenizer.is_some()
  }

  /// Create model-specific prompt for quantized Qwen3 models
  /// Uses ChatML format which works better with quantized models
  pub fn create_prompt(&self, git_output: &str) -> anyhow::Result<String> {
    crate::prompt::create_chatml_prompt(git_output, None)
  }

  /// Create alternative prompt when a previous suggestion exists
  /// Uses ChatML format with modified system prompt
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
      .finish()
  }
}
