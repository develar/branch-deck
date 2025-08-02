use crate::constants::GENERATION_BUFFER;
use crate::prompt::MAX_BRANCH_NAME_LENGTH;
use anyhow::Result;
use candle_core::Device;
use tracing::{info, instrument};

/// Clean up generated branch name to follow Git conventions
/// Provides fallbacks and handles edge cases to always return a valid branch name
#[instrument(level = "debug")]
pub fn clean_branch_name(raw_name: &str) -> Result<String> {
  // Remove thinking tags if present
  let without_tags = if let Some(end_pos) = raw_name.rfind("</think>") {
    // Take everything after the closing tag
    &raw_name[end_pos + 8..]
  } else if let Some(think_start) = raw_name.find("<think>") {
    // If we have opening but no closing, take everything before it
    &raw_name[..think_start]
  } else {
    raw_name
  };

  // Take first non-empty line and trim
  let first_line = without_tags.lines().find(|line| !line.trim().is_empty()).map(|line| line.trim()).unwrap_or("");

  // Early return for empty input
  if first_line.is_empty() {
    return Err(anyhow::anyhow!("Could not generate a valid branch name"));
  }

  // Process the name: handle special cases and filter characters in one pass
  let processed = if first_line.starts_with("```") || first_line.contains("git ") {
    // Special handling for code blocks and git commands
    first_line
      .split_whitespace()
      .filter(|word| !word.contains("git") && !word.contains("```"))
      .collect::<Vec<_>>()
      .join("-")
  } else {
    first_line.to_string()
  };

  // Filter valid characters and build final string
  let mut result = String::with_capacity(processed.len());
  for c in processed.chars() {
    if c.is_alphanumeric() || matches!(c, '-' | '_' | '/' | '.') {
      result.push(c);
    }
  }

  // Trim special characters from both ends
  let trimmed = result.trim_matches(&['-', '_', '.', '/'][..]);

  // Check if result is empty after filtering
  if trimmed.is_empty() {
    return Err(anyhow::anyhow!("Could not generate a valid branch name"));
  }

  // Handle length limit
  let final_name = if trimmed.len() > MAX_BRANCH_NAME_LENGTH {
    let truncated = &trimmed[..MAX_BRANCH_NAME_LENGTH];
    // Trim any trailing special chars from truncation
    truncated.trim_end_matches(&['-', '_'][..])
  } else {
    trimmed
  };

  Ok(final_name.to_string())
}

/// Detect the best available device for ML inference
/// Tries CUDA (if feature enabled), Metal, then falls back to CPU
///
/// Metal support now works correctly with all candle crates having Metal features enabled
/// To enable CUDA support, compile with: cargo build --features cuda
/// Requires NVIDIA CUDA toolkit to be installed
#[instrument(level = "debug")]
pub fn detect_device() -> Device {
  // Try CUDA first (best performance on NVIDIA GPUs)
  #[cfg(feature = "cuda")]
  {
    match Device::new_cuda(0) {
      Ok(device) => {
        info!("Using CUDA GPU acceleration (device 0)");
        return device;
      }
      Err(e) => {
        tracing::debug!("CUDA GPU not available: {}", e);
      }
    }
  }

  // Try Metal on macOS (good performance on Apple Silicon)
  #[cfg(feature = "metal")]
  {
    match Device::new_metal(0) {
      Ok(device) => {
        info!("Using Metal GPU acceleration (device 0)");
        return device;
      }
      Err(e) => {
        tracing::debug!("Metal GPU not available: {}", e);
      }
    }
  }

  // Fall back to CPU (with Accelerate framework optimization on macOS)
  info!("Using CPU for ML inference (with Accelerate framework if available)");
  Device::Cpu
}

/// Truncate tokens if they exceed the context limit
/// Returns the truncated token count for logging purposes
#[instrument(level = "debug", skip(tokens), fields(token_count = tokens.len()), ret)]
pub fn truncate_tokens_if_needed(tokens: &mut Vec<u32>, max_context_tokens: usize) -> Option<usize> {
  if tokens.len() > max_context_tokens - GENERATION_BUFFER {
    let original_len = tokens.len();
    let max_tokens = max_context_tokens - GENERATION_BUFFER;
    tokens.truncate(max_tokens);
    Some(original_len)
  } else {
    None
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_clean_branch_name() {
    assert_eq!(clean_branch_name("fix-user-service").unwrap(), "fix-user-service");
    assert_eq!(clean_branch_name("2.0.0 - fix: null pointer").unwrap(), "2.0.0-fixnullpointer"); // Spaces and colons filtered out
    assert_eq!(clean_branch_name("```git checkout branch```").unwrap(), "checkout"); // git and ``` filtered out, remaining words joined
    assert_eq!(clean_branch_name("git checkout feature").unwrap(), "checkout-feature"); // Test word filtering
    assert!(clean_branch_name("").is_err()); // Empty input returns error
    assert!(clean_branch_name("```git```").is_err()); // All words filtered out, returns error
    assert_eq!(
      clean_branch_name("very-long-branch-name-that-should-be-truncated-because-it-exceeds-fifty-characters").unwrap(),
      "very-long-branch-name-that-should-be-truncated-bec"
    ); // Truncated at MAX_BRANCH_NAME_LENGTH chars, trailing hyphen trimmed
    assert_eq!(clean_branch_name("main").unwrap(), "main"); // main is a valid branch name

    // Test thinking tag removal
    assert_eq!(clean_branch_name("\n\n</think>\n\nuser-service-secure-hash").unwrap(), "user-service-secure-hash");
    assert_eq!(clean_branch_name("<think>some thinking</think>feature-branch").unwrap(), "feature-branch");
    assert_eq!(clean_branch_name("</think>config-refactor").unwrap(), "config-refactor");
  }
}
