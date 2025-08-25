use crate::prompt::{MAX_BRANCH_NAME_LENGTH, create_chatml_prompt, create_generic_prompt};
use crate::test_utils::{CommitDiff, CommitInfo, FileDiff, convert_to_raw_git_format};
use pretty_assertions::assert_eq;
use test_log::test;

#[test]
fn test_create_generic_prompt_basic() {
  let commits = vec![CommitInfo {
    message: "feat: Add user authentication".to_string(),
    hash: "abc123".to_string(),
  }];

  let diffs = vec![];

  let git_output = convert_to_raw_git_format(&commits, &diffs);
  let result = create_generic_prompt(&git_output);
  assert!(result.is_ok());

  let prompt = result.unwrap();
  assert!(prompt.contains("feat: Add user authentication"));
  assert!(prompt.contains("Create one branch name"));
  assert!(prompt.contains("Your turn:"));
}

#[test]
fn test_create_chatml_prompt_basic() {
  let commits = vec![CommitInfo {
    message: "feat: Add OAuth2 integration".to_string(),
    hash: "def456".to_string(),
  }];

  let diffs = vec![];

  let git_output = convert_to_raw_git_format(&commits, &diffs);
  let result = create_chatml_prompt(&git_output, None);
  assert!(result.is_ok());

  let prompt = result.unwrap();
  assert!(prompt.contains("<|im_start|>system"));
  assert!(prompt.contains("feat: Add OAuth2 integration"));
  assert!(prompt.ends_with("<|im_start|>assistant"));
}

#[test]
fn test_generic_prompt_includes_examples() {
  let commits = vec![];
  let diffs = vec![];

  let git_output = convert_to_raw_git_format(&commits, &diffs);
  let result = create_generic_prompt(&git_output);
  assert!(result.is_ok());

  let prompt = result.unwrap();
  assert!(prompt.contains("Create one branch name"));
  assert!(prompt.contains("Example:"));
  assert!(prompt.contains("Update payment gateway"));
  assert!(prompt.contains("Your turn:"));
}

#[test]
fn test_prompt_with_multiple_commits() {
  let commits = vec![
    CommitInfo {
      message: "feat: Add login functionality".to_string(),
      hash: "123".to_string(),
    },
    CommitInfo {
      message: "fix: Handle edge case in auth flow".to_string(),
      hash: "456".to_string(),
    },
  ];

  let diffs = vec![];

  let git_output = convert_to_raw_git_format(&commits, &diffs);
  let result = create_generic_prompt(&git_output);
  assert!(result.is_ok());

  let prompt = result.unwrap();
  assert!(prompt.contains("feat: Add login functionality"));
  assert!(prompt.contains("fix: Handle edge case in auth flow"));
}

#[test]
fn test_max_branch_name_length_constant() {
  // Ensure the constant is properly defined and reasonable
  assert_eq!(MAX_BRANCH_NAME_LENGTH, 50);
}

#[test]
fn test_create_enhanced_prompt() {
  let commits = vec![CommitInfo {
    message: "feat: Add user authentication with JWT".to_string(),
    hash: "abc123".to_string(),
  }];

  let diffs = vec![CommitDiff {
    commit_hash: "abc123".to_string(),
    files: vec![FileDiff {
      path: "src/auth/jwt.rs".to_string(),
      additions: 150,
      deletions: 0,
      patch: Some("+fn verify_token(token: &str) -> Result<Claims>".to_string()),
    }],
  }];

  let git_output = convert_to_raw_git_format(&commits, &diffs);
  let prompt = create_generic_prompt(&git_output).unwrap();

  // Check for prompt structure
  assert!(prompt.contains("Create one branch name"));
  assert!(prompt.contains("Your turn:"));
  assert!(prompt.contains("feat: Add user authentication with JWT"));
  assert!(prompt.contains("jwt.rs"));
}

#[test]
fn test_create_prompt_with_diffs() {
  let commits = vec![
    CommitInfo {
      message: "feat: Add user authentication".to_string(),
      hash: "abc123".to_string(),
    },
    CommitInfo {
      message: "test: Add auth tests".to_string(),
      hash: "def456".to_string(),
    },
  ];

  let diffs = vec![
    CommitDiff {
      commit_hash: "abc123".to_string(),
      files: vec![
        FileDiff {
          path: "src/auth/login.rs".to_string(),
          additions: 150,
          deletions: 10,
          patch: None,
        },
        FileDiff {
          path: "src/auth/register.rs".to_string(),
          additions: 200,
          deletions: 0,
          patch: None,
        },
      ],
    },
    CommitDiff {
      commit_hash: "def456".to_string(),
      files: vec![FileDiff {
        path: "tests/auth_test.rs".to_string(),
        additions: 300,
        deletions: 0,
        patch: None,
      }],
    },
  ];

  let git_output = convert_to_raw_git_format(&commits, &diffs);
  let prompt = create_generic_prompt(&git_output).unwrap();

  // Check prompt structure
  assert!(prompt.contains("Create one branch name"));
  assert!(prompt.contains("Your turn:"));
  assert!(prompt.contains("feat: Add user authentication"));
  assert!(prompt.contains("test: Add auth tests"));
  // Verify file information is included in new format
  assert!(prompt.contains("register.rs"));
  assert!(prompt.contains("login.rs"));
  assert!(prompt.contains("auth_test.rs"));
}

#[test]
fn test_create_prompt_with_empty_diffs() {
  let commits = vec![CommitInfo {
    message: "feat: Add feature".to_string(),
    hash: "abc123".to_string(),
  }];

  // Test with empty diffs (which would happen if all commit hashes were invalid)
  let empty_diffs: Vec<CommitDiff> = vec![];

  let git_output = convert_to_raw_git_format(&commits, &empty_diffs);
  let prompt = create_generic_prompt(&git_output).unwrap();

  // Should still generate a valid prompt
  assert!(prompt.contains("Create one branch name"));
  assert!(prompt.contains("feat: Add feature"));
}

#[test]
fn test_chatml_format_structure() {
  let commits = vec![CommitInfo {
    message: "fix: resolve null pointer exception".to_string(),
    hash: "xyz789".to_string(),
  }];

  let diffs = vec![];
  let git_output = convert_to_raw_git_format(&commits, &diffs);
  let prompt = create_chatml_prompt(&git_output, None).unwrap();

  // Verify ChatML structure
  assert!(prompt.starts_with("<|im_start|>system"));
  assert!(prompt.contains("You are a Git branch name generator"));
  assert!(prompt.contains("Output only the branch name"));
  assert!(prompt.contains("Maximum 50 characters"));
  assert!(prompt.contains("<|im_end|>"));
  assert!(prompt.contains("<|im_start|>user"));
  assert!(prompt.contains("fix: resolve null pointer exception"));
  assert!(prompt.ends_with("<|im_start|>assistant"));
}

#[test]
fn test_create_chatml_prompt_alternative() {
  let commits = vec![CommitInfo {
    message: "fix: Update password hashing algorithm".to_string(),
    hash: "abc123".to_string(),
  }];

  let diffs = vec![CommitDiff {
    commit_hash: "abc123".to_string(),
    files: vec![FileDiff {
      path: "auth.js".to_string(),
      additions: 45,
      deletions: 12,
      patch: None,
    }],
  }];

  let git_output = convert_to_raw_git_format(&commits, &diffs);
  let result = create_chatml_prompt(&git_output, Some("password-hash"));
  assert!(result.is_ok());

  let prompt = result.unwrap();
  // Check it mentions the previous suggestion
  assert!(prompt.contains("Previous suggestion was 'password-hash'."));
  // Check for alternative instructions
  assert!(prompt.contains("DIFFERENT name"));
  assert!(prompt.contains("Maximum 50 characters"));
  // Still has ChatML structure
  assert!(prompt.contains("<|im_start|>system"));
  assert!(prompt.ends_with("<|im_start|>assistant"));
}
