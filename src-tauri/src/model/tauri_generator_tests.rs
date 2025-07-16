use super::tauri_generator::*;
use git_ops::git_command::GitCommandExecutor;
use model_ai::types::CommitInfo;
use model_core::ModelConfig;
use model_core::utils::clean_branch_name;

#[test]
fn test_clean_branch_name() {
  // Test basic cleaning
  let result = clean_branch_name("Feature/User Authentication!").unwrap();
  assert_eq!(result, "Feature/UserAuthentication"); // No space becomes no separator

  // Test length limit
  let long_name = "this-is-a-very-long-branch-name-that-exceeds-fifty-characters-limit";
  let result = clean_branch_name(long_name).unwrap();
  assert!(result.len() <= 50);
  assert!(!result.ends_with('-'));

  // Test special characters removal (@ # $ get filtered out)
  let result = clean_branch_name("feature@user#auth$system").unwrap();
  assert_eq!(result, "featureuserauthsystem");

  // Test valid branch names (formerly considered "reserved")
  let result = clean_branch_name("master").unwrap();
  assert_eq!(result, "master");

  // Test empty after cleaning - should return error
  let result = clean_branch_name("@#$%");
  assert!(result.is_err());

  // Test git command removal
  let result = clean_branch_name("git checkout feature-branch").unwrap();
  assert_eq!(result, "checkout-feature-branch");

  // Test code block removal (all words containing ``` get filtered out, leaving empty -> error)
  let result = clean_branch_name("```git branch```");
  assert!(result.is_err());
}

#[test]
fn test_model_creation() {
  // Test successful creation with default config
  let generator = ModelBasedBranchGenerator::new();
  assert!(generator.is_ok());

  // Test creation with Qwen config
  let generator = ModelBasedBranchGenerator::with_config(ModelConfig::Qwen25Coder05B);
  assert!(generator.is_ok());

  // Test creation with different Qwen configs - all should succeed now
  let generator = ModelBasedBranchGenerator::with_config(ModelConfig::Qwen3_17B);
  assert!(generator.is_ok());
}

#[tokio::test]
async fn test_model_not_loaded_error() {
  let mut generator = ModelBasedBranchGenerator::new().unwrap();

  let commits = vec![CommitInfo {
    hash: "abc123".to_string(),
    message: "feat: Add feature".to_string(),
  }];

  // Should fail because model is not loaded
  let git_executor = GitCommandExecutor::new();
  let result = generator.generate_branch_names(&git_executor, &commits, ".").await;
  assert!(result.is_err());
  assert!(result.unwrap_err().to_string().contains("Model not loaded"));
}

#[tokio::test]
async fn test_generate_branch_names_with_empty_hashes() {
  let mut generator = ModelBasedBranchGenerator::new().unwrap();

  // Test with commits that have empty hashes
  let commits_with_empty = vec![
    CommitInfo {
      hash: "".to_string(),
      message: "feat: Add feature".to_string(),
    },
    CommitInfo {
      hash: "".to_string(),
      message: "fix: Fix bug".to_string(),
    },
  ];

  let git_executor = GitCommandExecutor::new();
  let result = generator.generate_branch_names(&git_executor, &commits_with_empty, ".").await;
  assert!(result.is_err());
  assert!(result.unwrap_err().to_string().contains("No valid commits provided"));
}

#[tokio::test]
async fn test_generate_branch_names_mixed_valid_invalid() {
  let mut generator = ModelBasedBranchGenerator::new().unwrap();

  // Test with mix of valid and invalid commit hashes
  let mixed_commits = vec![
    CommitInfo {
      hash: "".to_string(),
      message: "feat: Add feature".to_string(),
    },
    CommitInfo {
      hash: "abc123".to_string(),
      message: "fix: Fix bug".to_string(),
    },
    CommitInfo {
      hash: "".to_string(),
      message: "docs: Update docs".to_string(),
    },
  ];

  // This should work because we have at least one valid commit
  // Note: This will fail without model loaded, but that's expected
  let git_executor = GitCommandExecutor::new();
  let result = generator.generate_branch_names(&git_executor, &mixed_commits, ".").await;
  assert!(result.is_err()); // Expected to fail due to "Model not loaded"
  assert!(result.unwrap_err().to_string().contains("Model not loaded"));
}

#[tokio::test]
async fn test_model_config_handling() {
  let mut generator = ModelBasedBranchGenerator::new().unwrap();

  // Test getting default config
  assert_eq!(generator.get_model_config(), ModelConfig::Qwen25Coder05B);

  // Test setting different Qwen configs - all should work
  let result = generator.set_model_config(ModelConfig::Qwen25Coder15B).await;
  assert!(result.is_ok());
  assert_eq!(generator.get_model_config(), ModelConfig::Qwen25Coder15B);

  // Test setting another Qwen config
  let result = generator.set_model_config(ModelConfig::Qwen3_17B).await;
  assert!(result.is_ok());
  assert_eq!(generator.get_model_config(), ModelConfig::Qwen3_17B);
}
