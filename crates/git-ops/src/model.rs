use crate::commit_list::Commit;
use anyhow::ensure;
use serde::{Deserialize, Serialize};
#[cfg(feature = "specta")]
use specta::Type;
use std::collections::BTreeMap;

/// Simple commit information with hash and message.
/// Used for passing commit data between frontend and backend.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "specta", derive(Type))]
pub struct CommitInfo {
  pub hash: String,
  pub message: String,
}

/// Status of a branch synchronization operation.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
pub enum BranchSyncStatus {
  Created,
  Updated,
  Unchanged,
  Error,
  MergeConflict,
  AnalyzingConflict,
  /// Not rebuilt: the parent branch of a stacked branch can't be built on
  Blocked,
}

/// Status of a commit synchronization.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
pub enum CommitSyncStatus {
  Pending,
  Created,
  Unchanged,
  Error,
  Blocked,
}

/// A conflicting file of a cherry-pick, as a conflict is reported: what the view lists before it reads the contents
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct ConflictFile {
  pub file: String,
  /// The conflicts of the merged file, its `<<<<<<<` marker lines (`count_conflicts`); 0 when one side deleted the
  /// file and the other changed it (a modify/delete conflict)
  pub conflict_count: u32,
}

/// What the conflict view shows of a conflicting file (`ConflictContents`)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct ConflictDetail {
  pub file: String,
  /// The file as the merge left it, with its conflict markers (`zdiff3`); empty when one side deleted it
  pub merged: crate::conflict_analysis::FileInfo,
  /// The change of the commit: its parent's version to its own, with both contents, for a file without markers
  pub commit_diff: crate::conflict_analysis::FileDiff,
}

/// A merge conflict of a cherry-pick, as a sync reports it: the commits, the conflicting files and the conflict
/// analysis, without file contents and diffs. Those are `ConflictContents`, which a view reads when it shows the
/// conflict (`get_conflict_contents`): a conflict in big files is megabytes, and the sync reports it at every run
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct MergeConflictInfo {
  /// Commit being applied (cherry-picked)
  pub commit: ConflictCommit,
  /// Original parent of the cherry-picked commit
  pub original_parent: ConflictCommit,
  /// Target branch HEAD where we're trying to apply the commit
  pub target_branch: ConflictCommit,
  pub conflicting_files: Vec<ConflictFile>,
  pub conflict_analysis: crate::conflict_analysis::ConflictAnalysis,
}

/// The contents of a conflict view, which `MergeConflictInfo` leaves out: the merged file and the change of the commit
/// of each conflicting file, and the diffs of the missing commits
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct ConflictContents {
  /// In the order of `MergeConflictInfo::conflicting_files`
  pub conflicting_files: Vec<ConflictDetail>,
  /// By missing commit: the diffs of the conflicting files it changes, in the order of its `files_touched`
  pub missing_commit_diffs: BTreeMap<String, Vec<crate::conflict_analysis::FileDiff>>,
}

impl MergeConflictInfo {
  /// A conflict known only by its commits and files, without conflict counts or analysis (`MergeMemo::light_conflicts`)
  pub fn light(commit: &str, original_parent: &str, target: &str, files: Vec<String>) -> Self {
    let commit_of = |hash: &str| ConflictCommit {
      hash: hash.to_string(),
      message: String::new(),
      author: String::new(),
      author_time: 0,
      committer_time: 0,
    };
    Self {
      commit: commit_of(commit),
      original_parent: commit_of(original_parent),
      target_branch: commit_of(target),
      conflicting_files: files.into_iter().map(|file| ConflictFile { file, conflict_count: 0 }).collect(),
      conflict_analysis: crate::conflict_analysis::ConflictAnalysis::default(),
    }
  }
}

/// A commit involved in a merge conflict: the applied commit, its original parent, the target, or one referenced in conflict markers
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct ConflictCommit {
  pub hash: String,
  pub message: String,
  pub author: String,
  pub author_time: u32,
  pub committer_time: u32,
}

impl From<&Commit> for ConflictCommit {
  fn from(commit: &Commit) -> Self {
    Self {
      hash: commit.id.clone(),
      message: commit.subject.clone(),
      author: commit.author_name.clone(),
      author_time: commit.author_timestamp,
      committer_time: commit.committer_timestamp,
    }
  }
}

/// Branch operation errors.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
pub enum BranchError {
  Generic(String),
  MergeConflict(Box<MergeConflictInfo>),
}

/// Namespace of a Branch Deck managed branch under the user's branch prefix
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BranchKind {
  /// `{prefix}/virtual/{name}`: branches rebuilt by sync
  Virtual,
  /// `{prefix}/unapplied/{name}`: branches removed from HEAD by unapply
  Unapplied,
  /// `{prefix}/archived/{YYYY-MM-DD}/{name}`: virtual branches a sync archived (the name here includes the date)
  Archived,
}

impl BranchKind {
  const fn segment(self) -> &'static str {
    match self {
      Self::Virtual => "virtual",
      Self::Unapplied => "unapplied",
      Self::Archived => "archived",
    }
  }
}

/// Build `{prefix}/{kind}/{name}`, e.g. "user", Virtual, "feature-auth" -> "user/virtual/feature-auth".
/// The name must already be sanitized.
pub fn qualified_branch_name(branch_prefix: &str, kind: BranchKind, branch_name: &str) -> anyhow::Result<String> {
  let prefix = branch_prefix.trim_end_matches('/').trim();
  ensure!(!prefix.is_empty(), "branch prefix cannot be blank");

  let name = branch_name.trim_end_matches('/').trim();
  ensure!(!name.is_empty(), "branch name cannot be blank");

  // Validate that the branch name is already sanitized
  let sanitized_name = sanitize_branch_name(name);
  ensure!(sanitized_name == name, "Branch name must be pre-sanitized. Got '{name}', expected '{sanitized_name}'");

  Ok(format!("{prefix}/{}/{name}", kind.segment()))
}

/// Build the virtual branch name `{prefix}/virtual/{name}`
pub fn to_final_branch_name(branch_prefix: &str, branch_name: &str) -> anyhow::Result<String> {
  qualified_branch_name(branch_prefix, BranchKind::Virtual, branch_name)
}

/// Strip `{prefix}/{kind}/` from a full branch name, e.g. "user/virtual/feature-auth" -> "feature-auth"
pub fn strip_branch_namespace<'a>(full_branch_name: &'a str, branch_prefix: &str, kind: BranchKind) -> Option<&'a str> {
  full_branch_name
    .strip_prefix(branch_prefix.trim_end_matches('/'))?
    .strip_prefix('/')?
    .strip_prefix(kind.segment())?
    .strip_prefix('/')
}

/// As `qualified_branch_name` builds `{prefix}/virtual/{name}`: `jane/` and ` jane ` are `jane`
/// (`normalizeBranchPrefix` of `src/lib/branch-prefix.ts`)
pub fn normalize_branch_prefix(prefix: &str) -> &str {
  prefix.trim().trim_end_matches('/').trim()
}

/// Whether git takes a normalized prefix as the leading part of a ref name: not empty, no whitespace, none of
/// `~^:?*[\`, no `..` and no `@{`; a `/` separates the parts (`isValidBranchPrefix` of `src/lib/branch-prefix.ts`)
pub fn is_valid_branch_prefix(prefix: &str) -> bool {
  !prefix.is_empty() && !prefix.chars().any(|c| c.is_whitespace() || matches!(c, '~' | '^' | ':' | '?' | '*' | '[' | '\\')) && !prefix.contains("..") && !prefix.contains("@{")
}

/// Sanitizes a branch name to make it valid for Git references
/// Git reference names cannot contain spaces, certain special characters, etc.
pub fn sanitize_branch_name(name: &str) -> String {
  // Problematic characters become hyphens; neither they nor hyphens are dots or slashes,
  // so trimming dots and slashes first gives the same result as trimming after the replacement.
  let trimmed = name.trim_matches('.').trim_matches('/');
  let mut result = String::with_capacity(trimmed.len());
  for c in trimmed.chars() {
    let c = if matches!(c, ' ' | '~' | '^' | ':' | '?' | '*' | '[' | ']' | '\\') { '-' } else { c };
    // Collapse consecutive hyphens and never start with one
    if c == '-' && (result.is_empty() || result.ends_with('-')) {
      continue;
    }
    result.push(c);
  }
  // Never end with a hyphen (at most one trailing hyphen remains after collapsing)
  if result.ends_with('-') {
    result.pop();
  }
  result
}

#[cfg(test)]
mod tests {
  use super::*;

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
  fn test_branch_sync_status_equality() {
    assert_eq!(BranchSyncStatus::Created, BranchSyncStatus::Created);
    assert_ne!(BranchSyncStatus::Created, BranchSyncStatus::Updated);
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
}
