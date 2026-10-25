use crate::branch_names::{
  BranchKind, baseline_on_remote, is_valid_branch_prefix, normalize_branch_prefix, qualified_branch_name, sanitize_branch_name, strip_branch_namespace, to_final_branch_name,
  to_stack_branch_name,
};

#[test]
fn test_to_final_branch_name_valid_inputs() {
  let result = to_final_branch_name("feature", "auth").unwrap();
  assert_eq!(result, "feature/virtual/auth");
}

#[test]
fn test_to_final_branch_name_with_trailing_slashes() {
  let result = to_final_branch_name("feature/", "auth/").unwrap();
  assert_eq!(result, "feature/virtual/auth");
}

#[test]
fn test_to_final_branch_name_with_spaces() {
  let result = to_final_branch_name("  feature  ", "  auth  ").unwrap();
  assert_eq!(result, "feature/virtual/auth");
}

#[test]
fn test_to_final_branch_name_empty_prefix() {
  let result = to_final_branch_name("", "auth");
  assert!(result.is_err());
  assert_eq!(result.unwrap_err().to_string(), "branch prefix cannot be blank");
}

#[test]
fn test_to_final_branch_name_whitespace_prefix() {
  let result = to_final_branch_name("   ", "auth");
  assert!(result.is_err());
  assert_eq!(result.unwrap_err().to_string(), "branch prefix cannot be blank");
}

#[test]
fn test_to_final_branch_name_empty_name() {
  let result = to_final_branch_name("feature", "");
  assert!(result.is_err());
  assert_eq!(result.unwrap_err().to_string(), "branch name cannot be blank");
}

#[test]
fn test_to_final_branch_name_whitespace_name() {
  let result = to_final_branch_name("feature", "   ");
  assert!(result.is_err());
  assert_eq!(result.unwrap_err().to_string(), "branch name cannot be blank");
}

#[test]
fn test_sanitize_branch_name_with_spaces() {
  assert_eq!(sanitize_branch_name("ui dispatcher"), "ui-dispatcher");
  assert_eq!(sanitize_branch_name("hello world test"), "hello-world-test");
}

#[test]
fn test_sanitize_branch_name_with_special_chars() {
  assert_eq!(sanitize_branch_name("test~branch"), "test-branch");
  assert_eq!(sanitize_branch_name("test^branch"), "test-branch");
  assert_eq!(sanitize_branch_name("test:branch"), "test-branch");
  assert_eq!(sanitize_branch_name("test?branch"), "test-branch");
  assert_eq!(sanitize_branch_name("test*branch"), "test-branch");
  assert_eq!(sanitize_branch_name("test[branch]"), "test-branch");
  assert_eq!(sanitize_branch_name("test\\branch"), "test-branch");
}

#[test]
fn test_sanitize_branch_name_consecutive_hyphens() {
  assert_eq!(sanitize_branch_name("test--branch"), "test-branch");
  assert_eq!(sanitize_branch_name("test   branch"), "test-branch");
  assert_eq!(sanitize_branch_name("test-~-branch"), "test-branch");
}

#[test]
fn test_sanitize_branch_name_edge_cases() {
  assert_eq!(sanitize_branch_name("-test-"), "test");
  assert_eq!(sanitize_branch_name(".test."), "test");
  assert_eq!(sanitize_branch_name("/test/"), "test");
  assert_eq!(sanitize_branch_name("---test---"), "test");
}

#[test]
fn test_qualified_branch_name_and_strip_namespace() {
  let unapplied = qualified_branch_name("user/", BranchKind::Unapplied, "feature-auth").unwrap();
  assert_eq!(unapplied, "user/unapplied/feature-auth");
  assert_eq!(strip_branch_namespace(&unapplied, "user", BranchKind::Unapplied), Some("feature-auth"));
  assert_eq!(strip_branch_namespace(&unapplied, "user", BranchKind::Virtual), None);
  assert_eq!(strip_branch_namespace("user/virtual/a/b", "user/", BranchKind::Virtual), Some("a/b"));
  assert_eq!(strip_branch_namespace("username/virtual/a", "user", BranchKind::Virtual), None);
}

/// The whole stack of a root is one branch of its own namespace: neither a virtual nor an archived branch
#[test]
fn the_stack_branch_of_a_root_is_in_its_own_namespace() {
  let stack = to_stack_branch_name("user/", "auth").unwrap();
  assert_eq!(stack, "user/stack/auth");
  assert_eq!(strip_branch_namespace(&stack, "user", BranchKind::Stack), Some("auth"));
  assert_eq!(strip_branch_namespace(&stack, "user", BranchKind::Virtual), None);
  assert_eq!(strip_branch_namespace("user/virtual/stack/auth", "user", BranchKind::Stack), None);
  assert_eq!(to_stack_branch_name("user", "").unwrap_err().to_string(), "branch name cannot be blank");
  to_stack_branch_name("user", "a b").unwrap_err();
}

#[test]
fn test_sanitize_branch_name_trims_before_replacing() {
  assert_eq!(sanitize_branch_name(" .test. "), ".test.");
  assert_eq!(sanitize_branch_name("/.test"), ".test");
  assert_eq!(sanitize_branch_name("a - b"), "a-b");
  assert_eq!(sanitize_branch_name("~~~"), "");
}

#[test]
fn test_to_final_branch_name_rejects_unsanitized_input() {
  // Should reject branch names with spaces
  let result = to_final_branch_name("develar", "ui dispatcher");
  assert!(result.is_err());
  assert!(result.unwrap_err().to_string().contains("pre-sanitized"));

  // Should reject branch names with special characters
  let result = to_final_branch_name("feature", "test~branch");
  result.unwrap_err();

  // Should accept pre-sanitized names
  let result = to_final_branch_name("develar", "ui-dispatcher").unwrap();
  assert_eq!(result, "develar/virtual/ui-dispatcher");
}

// the cases of `src/lib/branch-prefix.test.ts`
#[test]
fn normalize_branch_prefix_as_the_branch_is_named() {
  for (input, expected) in [("jane", "jane"), ("jane/", "jane"), (" jane// ", "jane"), ("team/jane", "team/jane"), ("  ", "")] {
    assert_eq!(normalize_branch_prefix(input), expected, "{input:?}");
  }
}

#[test]
fn is_valid_branch_prefix_accepts_names_git_takes_as_a_ref_part() {
  assert!(is_valid_branch_prefix("jane.doe"));
  assert!(is_valid_branch_prefix("team/jane"));
  for prefix in ["", "jane doe", "a..b", "a:b", "a~1", "a^", "a?", "a*", "a[b", "a\\b", "a@{b", "a\tb"] {
    assert!(!is_valid_branch_prefix(prefix), "{prefix:?}");
  }
}

/// The branch a baseline names on its remote, as the page reads the target of the readiness
#[test]
fn a_baseline_names_its_branch_on_the_remote() {
  assert_eq!(baseline_on_remote("origin/master"), "master");
  assert_eq!(baseline_on_remote("upstream/release/2026"), "release/2026");
  assert_eq!(baseline_on_remote("master"), "master");
}
