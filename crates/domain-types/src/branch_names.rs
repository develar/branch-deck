//! The names of the branches Branch Deck manages under the user's prefix (`{prefix}/virtual/<name>`,
//! `{prefix}/stack/<root>`, `BranchKind`), the prefix itself, a group name made valid for a ref, and the check of a
//! name the user types for a branch (`check_branch_name`).

use anyhow::ensure;
use serde::{Deserialize, Serialize};

/// Namespace of a Branch Deck managed branch under the user's branch prefix
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BranchKind {
  /// `{prefix}/virtual/{name}`: branches rebuilt by sync
  Virtual,
  /// `{prefix}/unapplied/{name}`: branches removed from HEAD by unapply
  Unapplied,
  /// `{prefix}/archived/{YYYY-MM-DD}/{name}`: virtual branches a sync archived (the name here includes the date)
  Archived,
  /// `{prefix}/stack/{root}`: the whole stack of `root` as one branch, which a sync builds for a stack that forks
  Stack,
}

impl BranchKind {
  const fn segment(self) -> &'static str {
    match self {
      Self::Virtual => "virtual",
      Self::Unapplied => "unapplied",
      Self::Archived => "archived",
      Self::Stack => "stack",
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

/// Build the stack branch name `{prefix}/stack/{root}`: all commits of the stack of `root` in one branch
pub fn to_stack_branch_name(branch_prefix: &str, root: &str) -> anyhow::Result<String> {
  qualified_branch_name(branch_prefix, BranchKind::Stack, root)
}

/// Strip `{prefix}/{kind}/` from a full branch name, e.g. "user/virtual/feature-auth" -> "feature-auth"
pub fn strip_branch_namespace<'a>(full_branch_name: &'a str, branch_prefix: &str, kind: BranchKind) -> Option<&'a str> {
  full_branch_name
    .strip_prefix(branch_prefix.trim_end_matches('/'))?
    .strip_prefix('/')?
    .strip_prefix(kind.segment())?
    .strip_prefix('/')
}

/// The branch of the remote a baseline names, the part after its remote (`origin/master` -> `master`, a local `master`
/// -> `master`): the target the page names the readiness by (`baselineOnRemote` of `src/lib/stack.ts`), and the base of
/// a pull request whose base is unknown (empty). The Patronus target of a start is git's (the branch the baseline
/// tracks, `sync_core::patronus::find_target_branch`)
pub fn baseline_on_remote(baseline: &str) -> &str {
  baseline.split_once('/').map_or(baseline, |(_, branch)| branch)
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

/// What a typed branch name may be
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct BranchNameRules {
  /// Stacked branches are on: a `/` separates a parent from its branch
  #[serde(default)]
  pub stacks: bool,
  /// Only a branch name, not `parent/branch` (with stacked branches on: a rename)
  #[serde(default)]
  pub leaf_only: bool,
}

/// What a typed branch name is; the page words each (`src/presenters/branch-name.ts`)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum BranchNameState {
  /// Nothing typed yet
  Empty,
  Valid,
  /// Under two characters
  TooShort,
  /// A character other than a letter, a digit, `-`, `_`, `.`, `/` or a space, or a leading `.`
  InvalidChars,
  /// A `/` in a name that names only a branch (`BranchNameRules::leaf_only`)
  LeafOnly,
  /// The grouping would rename it (`sanitize_branch_name`: `-`, `.` or `/` at an end, `--`), or it has `..` or `//`,
  /// so the virtual branch would not get this name
  InvalidRef,
}

/// The check of a typed branch name: what it is, and the name the branch gets (`sanitized`)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct BranchNameCheck {
  pub state: BranchNameState,
  /// The name trimmed, its runs of whitespace turned into `-`
  pub sanitized: String,
}

/// The parameters of `validate_branch_name`
#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ValidateBranchNameParams {
  pub name: String,
  #[serde(default)]
  pub rules: BranchNameRules,
}

/// Checks a branch name the user types (the "Move to…" form, a rename): spaces may be typed, they become `-`; the rest
/// must be a name the grouping keeps as it is, so the branch gets exactly that name
pub fn check_branch_name(name: &str, rules: BranchNameRules) -> BranchNameCheck {
  let sanitized = name.split_whitespace().collect::<Vec<_>>().join("-");
  let state = if name.is_empty() {
    BranchNameState::Empty
  } else if rules.stacks && rules.leaf_only && name.contains('/') {
    BranchNameState::LeafOnly
  } else if sanitized.chars().count() < 2 {
    BranchNameState::TooShort
  } else if !name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ' ' | '/')) || name.trim().starts_with('.') {
    BranchNameState::InvalidChars
  } else if sanitize_branch_name(&sanitized) != sanitized || sanitized.contains("//") || sanitized.contains("..") {
    BranchNameState::InvalidRef
  } else {
    BranchNameState::Valid
  };
  BranchNameCheck { state, sanitized }
}

#[cfg(test)]
#[path = "branch_names_test.rs"]
mod branch_names_test;
