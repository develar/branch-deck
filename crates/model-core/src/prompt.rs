use anyhow::Result;

/// Maximum length for branch names (Git's practical limit)
pub const MAX_BRANCH_NAME_LENGTH: usize = 50;

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

/// Create a ChatML-formatted prompt for Qwen3 models
/// This format uses conversation structure for better results with quantized models
pub fn create_chatml_prompt(git_output: &str) -> Result<String> {
  let prompt = format!(
    r#"<|im_start|>system
You are a Git branch name generator. Output only the branch name - no explanations, no thinking, no extra text. Use lowercase letters, numbers, and hyphens only. Maximum 50 characters.<|im_end|>
<|im_start|>user
{}<|im_end|>
<|im_start|>assistant<think></think>"#,
    git_output.trim()
  );
  Ok(prompt)
}
