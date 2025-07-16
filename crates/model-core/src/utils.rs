use anyhow::Result;
use candle_core::Device;
use tracing::{debug, info};

/// Clean up generated branch name to follow Git conventions
/// Provides fallbacks and handles edge cases to always return a valid branch name
pub fn clean_branch_name(raw_name: &str) -> Result<String> {
  tracing::debug!("Cleaning branch name from raw: '{}'", raw_name);

  let cleaned = raw_name
    .trim()
    .lines()
    .next() // Take only first line
    .unwrap_or("")
    .trim();

  // Remove common prefixes/suffixes and code artifacts
  let cleaned = if cleaned.starts_with("```") || cleaned.contains("git ") {
    cleaned
      .split_whitespace()
      .filter(|word| !word.contains("git") && !word.contains("```"))
      .collect::<Vec<_>>()
      .join("-")
  } else {
    cleaned.to_string()
  };

  // Basic character filtering
  let cleaned = cleaned
    .chars()
    .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_' || *c == '/' || *c == '.')
    .collect::<String>();

  // Remove leading/trailing special chars
  let cleaned = cleaned.trim_matches(&['-', '_', '.', '/'][..]);

  // Check if name is empty and return error
  if cleaned.is_empty() {
    tracing::debug!("Cleaned name is empty after processing");
    return Err(anyhow::anyhow!("Could not generate a valid branch name"));
  }

  // Limit length if needed
  let cleaned = if cleaned.len() > 50 {
    cleaned[..50].trim_end_matches(&['-', '_'][..]).to_string()
  } else {
    cleaned.to_string()
  };

  Ok(cleaned)
}

/// Calculate confidence score based on generation quality
pub fn calculate_confidence(generated_text: &str, token_count: usize) -> f32 {
  let mut confidence: f32 = 0.8; // Base confidence

  // Higher confidence for reasonable length
  if (3..=10).contains(&token_count) {
    confidence += 0.1;
  }

  // Lower confidence for very short or very long outputs
  if !(2..=20).contains(&token_count) {
    confidence -= 0.2;
  }

  // Higher confidence for branch-like patterns
  if generated_text.contains('-') || generated_text.contains('_') {
    confidence += 0.05;
  }

  // Lower confidence for code-like patterns
  if generated_text.contains("```") || generated_text.contains("git ") {
    confidence -= 0.3;
  }

  // Ensure confidence is in valid range
  confidence.clamp(0.1, 1.0)
}

/// Detect the best available device for ML inference
/// Tries CUDA (if feature enabled), Metal, then falls back to CPU
///
/// Metal support now works correctly with all candle crates having Metal features enabled
/// To enable CUDA support, compile with: cargo build --features cuda
/// Requires NVIDIA CUDA toolkit to be installed
pub fn detect_device() -> Device {
  debug!("Detecting best available device for ML inference");

  // Try CUDA first (best performance on NVIDIA GPUs)
  #[cfg(feature = "cuda")]
  {
    match Device::new_cuda(0) {
      Ok(device) => {
        info!("Using CUDA GPU acceleration (device 0)");
        return device;
      }
      Err(e) => {
        debug!("CUDA GPU not available: {}", e);
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
        debug!("Metal GPU not available: {}", e);
      }
    }
  }

  // Fall back to CPU (with Accelerate framework optimization on macOS)
  info!("Using CPU for ML inference (with Accelerate framework if available)");
  Device::Cpu
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
    ); // Truncated at 50 chars, trailing hyphen trimmed
    assert_eq!(clean_branch_name("main").unwrap(), "main"); // main is a valid branch name
  }

  #[test]
  fn test_calculate_confidence() {
    assert!(calculate_confidence("fix-bug", 2) > 0.8);
    assert!(calculate_confidence("", 0) < 0.7);
    assert_eq!(calculate_confidence("```code```", 3), 0.6); // Base 0.8 + reasonable length 0.1 - code pattern 0.3 = 0.6
    assert!(calculate_confidence("feature-oauth2-support", 3) > 0.85);
  }
}
