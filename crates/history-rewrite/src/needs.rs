//! Declared landing needs: a branch -> its required branches (`store_lines.rs`). The store is shared by every branch and
//! worktree; a write changes only the named branch and moves its ref with compare-and-swap.

use crate::issue_groups::validate_leaf_name;
use crate::repository_lock::RewriteOptions;
use crate::store_lines::{LineStore, StoreDrop, StoreLines, drop_store_lines};
use crate::store_write::{PreparedLines, StoreWriteError, grouped_branches, store_error, write_prepared_lines};
use anyhow::Result;
use domain_types::error::ErrorCode;
use git_executor::git_command_executor::GitCommandExecutor;
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{Display, Formatter};
use tracing::instrument;

pub const NEEDS_REF: &str = "refs/branch-deck/needs";
pub const NEEDS_FILE: &str = "needs";
pub const NEEDS_DROP_MESSAGE: &str = "Declared needs of Branch Deck: integrated needs dropped";

/// A branch -> space-separated list of required branch keys. A malformed line or repeated key is a problem.
pub const NEEDS: LineStore = LineStore {
  store_ref: NEEDS_REF,
  file: NEEDS_FILE,
  name: "declared needs",
  message: "Declared needs of Branch Deck",
  shape: "branch<TAB>needed needed…",
  validate: validate_line,
};

/// A local branch (`branch`) or one in a companion (`companion:branch`). Ordered as it is written, so that a line
/// reads as the user sorts names, not with every local name first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NeedKey {
  pub companion: Option<String>,
  pub branch: String,
}

impl Ord for NeedKey {
  fn cmp(&self, other: &Self) -> Ordering {
    self.written().cmp(other.written())
  }
}

impl PartialOrd for NeedKey {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(self.cmp(other))
  }
}

impl NeedKey {
  /// The bytes of `Display`, for the order of a line
  fn written(&self) -> impl Iterator<Item = u8> + '_ {
    self
      .companion
      .iter()
      .flat_map(|companion| companion.bytes().chain(std::iter::once(b':')))
      .chain(self.branch.bytes())
  }

  pub fn parse(text: &str) -> Result<Self, String> {
    let (companion, branch) = match text.split_once(':') {
      Some((companion, branch)) if !companion.is_empty() && !branch.is_empty() && !branch.contains(':') => (Some(companion), branch),
      Some(_) => return Err(format!("{text:?} is not a branch or companion:branch name")),
      None => (None, text),
    };
    if let Some(companion) = companion {
      validate_leaf_name(companion, "companion name")?;
    }
    validate_leaf_name(branch, "needed branch name")?;
    Ok(Self {
      companion: companion.map(str::to_string),
      branch: branch.to_string(),
    })
  }
}

impl Display for NeedKey {
  fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
    if let Some(companion) = &self.companion {
      write!(formatter, "{companion}:{}", self.branch)
    } else {
      formatter.write_str(&self.branch)
    }
  }
}

/// The request and result of changing a branch's declared needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NeedOutcome {
  pub branch: String,
  pub previous_needs: Vec<String>,
  pub needs: Vec<String>,
  pub previous_commit: Option<String>,
  pub new_commit: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum NeedError {
  #[error("No branch named {0:?} among the commits ahead of the baseline")]
  UnknownBranch(String),
  #[error("{0}")]
  Invalid(String),
  #[error("{needed} is not declared as a need of {branch}")]
  NotDeclared { branch: String, needed: String },
  #[error(transparent)]
  Write(StoreWriteError),
}

impl NeedError {
  pub fn code(&self) -> Option<ErrorCode> {
    Some(match self {
      Self::UnknownBranch(_) => ErrorCode::UnknownBranch,
      Self::Invalid(_) | Self::NotDeclared { .. } => ErrorCode::InvalidArguments,
      Self::Write(error) => return error.code(ErrorCode::NeedsMoved),
    })
  }
}

store_error!(NeedError);

/// Declare `needed` for `branch`; each name is a local branch or `companion:branch`. The branch must group a commit
/// ahead of the baseline. Its store commit is written before the repository lock, then moved under it with CAS.
#[instrument(skip(git_executor, options))]
pub fn need_core(git_executor: &GitCommandExecutor, repository_path: &str, branch: &str, needed: &[&str], options: &RewriteOptions) -> Result<NeedOutcome, NeedError> {
  let keys = parse_request(branch, needed)?;
  write_prepared(git_executor, repository_path, options, |git_executor| {
    let (grouping, branches) = grouped_branches(git_executor, repository_path)?;
    if !branches.contains(branch) {
      return Err(NeedError::UnknownBranch(branch.to_string()));
    }
    let mut combined = declared(&grouping.needs, branch)?;
    combined.extend(keys.iter().cloned());
    prepare_lines(git_executor, repository_path, &grouping.needs, branch, Some(&combined), "need")
  })
}

/// Remove the names of `needed` from the declared needs of `branch`, in one store commit or not at all: each must be
/// declared. The branch may have no commits any more
#[instrument(skip(git_executor, options))]
pub fn unneed_core(git_executor: &GitCommandExecutor, repository_path: &str, branch: &str, needed: &[&str], options: &RewriteOptions) -> Result<NeedOutcome, NeedError> {
  let keys = parse_request(branch, needed)?;
  write_prepared(git_executor, repository_path, options, |git_executor| {
    let read = NEEDS.read(git_executor, repository_path)?;
    let mut remaining = declared(&read, branch)?;
    if let Some(undeclared) = keys.iter().find(|key| !remaining.contains(*key)) {
      return Err(NeedError::NotDeclared {
        branch: branch.to_string(),
        needed: undeclared.to_string(),
      });
    }
    remaining.retain(|key| !keys.contains(key));
    prepare_lines(git_executor, repository_path, &read, branch, (!remaining.is_empty()).then_some(&remaining), "unneed")
  })
}

/// The branch and the names of a need or of its removal, checked: a branch name, at least one name, none the branch itself
fn parse_request(branch: &str, needed: &[&str]) -> Result<BTreeSet<NeedKey>, NeedError> {
  validate_leaf_name(branch, "branch name").map_err(NeedError::Invalid)?;
  if needed.is_empty() {
    return Err(NeedError::Invalid("At least one needed branch is required".to_string()));
  }
  needed.iter().try_fold(BTreeSet::new(), |mut keys, text| {
    let key = NeedKey::parse(text).map_err(NeedError::Invalid)?;
    if key.companion.is_none() && key.branch == branch {
      return Err(NeedError::Invalid(format!("{branch} cannot need itself")));
    }
    keys.insert(key);
    Ok(keys)
  })
}

/// What `branch` needs as the store `read` has it: none without a line
fn declared(read: &StoreLines, branch: &str) -> Result<BTreeSet<NeedKey>, NeedError> {
  read
    .entries
    .get(branch)
    .map_or_else(|| Ok(BTreeSet::new()), |value| parse_needs(branch, value).map_err(NeedError::Invalid))
}

fn validate_line(branch: &str, value: &str) -> Result<(), String> {
  validate_leaf_name(branch, "branch name")?;
  parse_needs(branch, value).map(|_| ())
}

fn parse_needs(branch: &str, value: &str) -> Result<BTreeSet<NeedKey>, String> {
  let mut needs = BTreeSet::new();
  for text in value.split_whitespace() {
    let key = NeedKey::parse(text)?;
    if key.companion.is_none() && key.branch == branch {
      return Err(format!("{branch} cannot need itself"));
    }
    if !needs.insert(key.clone()) {
      return Err(format!("{key} is listed more than once"));
    }
  }
  if needs.is_empty() {
    return Err("A needs line must name at least one branch".to_string());
  }
  Ok(needs)
}

/// The drop of local needed branches that integration detection found; companion references are not touched.
pub const fn needs_drop<'a>(integrated: &'a [&'a str]) -> StoreDrop<'a> {
  StoreDrop {
    store: &NEEDS,
    message: NEEDS_DROP_MESSAGE,
    keys: &[],
    drop_values: Some(integrated),
  }
}

/// The unqualified targets named by the needs store, for the sync's one integration-detection result.
pub fn local_targets(entries: &BTreeMap<String, String>) -> BTreeSet<String> {
  entries
    .iter()
    .flat_map(|(branch, value)| parse_needs(branch, value).unwrap_or_default())
    .filter(|key| key.companion.is_none())
    .map(|key| key.branch)
    .collect()
}

/// The store commit `prepare` computed and wrote before the repository lock, moved under it (`write_prepared_lines`),
/// as a park
fn write_prepared(
  git_executor: &GitCommandExecutor,
  repository_path: &str,
  options: &RewriteOptions,
  prepare: impl Fn(&GitCommandExecutor) -> Result<PreparedLines<NeedOutcome>, NeedError>,
) -> Result<NeedOutcome, NeedError> {
  let (mut outcome, new_commit) = write_prepared_lines(git_executor, repository_path, options, prepare)?;
  outcome.new_commit = new_commit;
  Ok(outcome)
}

/// Set the line of `branch` to `needs`, or drop it with `None`, on the store as `read`, in one commit written on a
/// temporary ref (`PreparedLines::new`)
fn prepare_lines(
  git_executor: &GitCommandExecutor,
  repository_path: &str,
  read: &StoreLines,
  branch: &str,
  needs: Option<&BTreeSet<NeedKey>>,
  operation: &'static str,
) -> Result<PreparedLines<NeedOutcome>, NeedError> {
  let previous_needs = declared(read, branch)?;
  let written = |keys: &BTreeSet<NeedKey>| keys.iter().map(ToString::to_string).collect::<Vec<_>>();
  let outcome = NeedOutcome {
    branch: branch.to_string(),
    previous_needs: written(&previous_needs),
    needs: needs.map_or_default(written),
    previous_commit: read.commit.clone(),
    new_commit: None,
  };
  let line = needs.map(|keys| written(keys).join(" "));
  Ok(PreparedLines::new(git_executor, repository_path, &NEEDS, read, &[(branch, line)], operation, outcome)?)
}

/// Drop integrated local references from the needs store; each branch keeps its other declared needs.
pub fn drop_integrated_needs(git_executor: &GitCommandExecutor, repository_path: &str, integrated: &[&str]) {
  drop_store_lines(git_executor, repository_path, &[needs_drop(integrated)]);
}
