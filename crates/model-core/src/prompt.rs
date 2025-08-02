use anyhow::Result;

/// Maximum length for branch names (Git's practical limit)
pub const MAX_BRANCH_NAME_LENGTH: usize = 50;

/// Extract meaningful words from a branch name to avoid repetition
/// Filters out short words, common prefixes, and non-alphabetic parts
fn extract_meaningful_words(branch_name: &str) -> Vec<String> {
  branch_name
    .split(&['-', '_', '/'][..])
    .filter_map(|word| {
      let clean_word = word.trim().to_lowercase();
      // Filter out short words, numbers, and common prefixes
      if clean_word.len() >= 3 && clean_word.chars().any(|c| c.is_alphabetic()) {
        Some(clean_word)
      } else {
        None
      }
    })
    .collect()
}

/// Create a generic prompt from raw git log output
/// This is the proven format that works well for most models
pub fn create_generic_prompt(git_output: &str) -> Result<String> {
  let prompt = format!(
    "Create one branch name for all commits (max {MAX_BRANCH_NAME_LENGTH} characters):
    
Example:
Update payment gateway

A       auth.js
M       payment.js

Add unit tests for payment flow

A       test-login.js

Update API documentation

M       README.md

Branch name: update-payment-gateway-tests-docs

Your turn:
{}

Branch name:",
    git_output.trim()
  );
  Ok(prompt)
}

/// Create a generic alternative prompt when a previous suggestion exists
/// This adds explicit instructions to avoid words from the previous suggestion
pub fn create_generic_alternative_prompt(git_output: &str, previous_suggestion: &str) -> Result<String> {
  let words_to_avoid = extract_meaningful_words(previous_suggestion);

  let avoid_instruction = if words_to_avoid.is_empty() {
    format!("Previous suggestion: {previous_suggestion}\nCreate a DIFFERENT branch name using completely different words.")
  } else {
    format!(
      "Previous suggestion: {}\nDO NOT use these words: {}\nCreate a DIFFERENT branch name using completely different words.",
      previous_suggestion,
      words_to_avoid.join(", ")
    )
  };

  let prompt = format!(
    "Create one branch name for all commits (max {MAX_BRANCH_NAME_LENGTH} characters):

{}

{}

Alternative branch name:",
    git_output.trim(),
    avoid_instruction
  );
  Ok(prompt)
}

// Constants for ChatML prompt construction
const CHATML_BASE_ROLE: &str = "You are a Git branch name generator.";
const CHATML_BASE_INSTRUCTIONS: &str = "Output only the branch name. Use lowercase letters, numbers, and hyphens only. Maximum 50 characters.";

/// Create a ChatML-formatted prompt for Qwen3 models
/// This format uses conversation structure for better results with quantized models
pub fn create_chatml_prompt(git_output: &str, previous_suggestion: Option<&str>) -> Result<String> {
  let mut system_message = String::from(CHATML_BASE_ROLE);

  if let Some(prev) = previous_suggestion {
    system_message.push_str(&format!(" Previous suggestion was '{prev}'."));
    system_message.push_str(" Create a DIFFERENT name using completely different words.");

    let words_to_avoid = extract_meaningful_words(prev);
    if !words_to_avoid.is_empty() {
      system_message.push_str(&format!(" DO NOT use these words: {}.", words_to_avoid.join(", ")));
    }
  }

  system_message.push(' ');
  system_message.push_str(CHATML_BASE_INSTRUCTIONS);

  let prompt = format!(
    r#"<|im_start|>system
{} /no_think<|im_end|>
<|im_start|>user
{}<|im_end|>
<|im_start|>assistant"#,
    system_message,
    git_output.trim()
  );
  Ok(prompt)
}
