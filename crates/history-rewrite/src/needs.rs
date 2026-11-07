//! Declared landing needs: a branch -> its required branches (`store_lines.rs`). The store is shared by every branch and
//! worktree; a write changes only the named branch and moves its ref with compare-and-swap.

use crate::baseline::NoBaseline;
use crate::issue_groups::validate_leaf_name;
use crate::repository_lock::{MovedHead, Prepared, RepositoryLocked, RewriteOptions, Witness};
use crate::snapshot::{CommitsAhead, group_commits, read_grouping_inputs};
use crate::store_lines::{LineStore, StoreDrop, StoreLines, StoreMoved, drop_store_lines, format_store_lines};
use anyhow::Result;
use domain_types::error::ErrorCode;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::commit_signing::Signing;
use git_ops::commit_writer::{WrittenCommits, write_commits_with_notes};
use git_ops::ref_transaction::RefTransaction;
use git_ops::revs::with_dirs_of;
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{Display, Formatter};
use tracing::{info, instrument};

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
  Moved(StoreMoved),
  #[error(transparent)]
  Locked(RepositoryLocked),
  #[error("{0:#}")]
  Other(anyhow::Error),
}

impl NeedError {
  pub fn code(&self) -> Option<ErrorCode> {
    Some(match self {
      Self::UnknownBranch(_) => ErrorCode::UnknownBranch,
      Self::Invalid(_) | Self::NotDeclared { .. } => ErrorCode::InvalidArguments,
      Self::Moved(_) => ErrorCode::NeedsMoved,
      Self::Locked(_) => ErrorCode::Locked,
      Self::Other(error) if error.downcast_ref::<NoBaseline>().is_some() => ErrorCode::NoBaseline,
      Self::Other(_) => return None,
    })
  }
}

impl MovedHead for NeedError {
  fn head_moved(&self) -> bool {
    matches!(self, Self::Other(error) if error.head_moved())
  }
}

impl From<anyhow::Error> for NeedError {
  fn from(error: anyhow::Error) -> Self {
    match (error.downcast_ref::<StoreMoved>().cloned(), error.downcast_ref::<RepositoryLocked>().cloned()) {
      (Some(moved), _) => Self::Moved(moved),
      (None, Some(locked)) => Self::Locked(locked),
      (None, None) => Self::Other(error),
    }
  }
}

/// Declare `needed` for `branch`; each name is a local branch or `companion:branch`. The branch must group a commit
/// ahead of the baseline. Its store commit is written before the repository lock, then moved under it with CAS.
#[instrument(skip(git_executor, options))]
pub fn need_core(git_executor: &GitCommandExecutor, repository_path: &str, branch: &str, needed: &[&str], options: &RewriteOptions) -> Result<NeedOutcome, NeedError> {
  validate_leaf_name(branch, "branch name").map_err(NeedError::Invalid)?;
  let keys = parse_request(branch, needed)?;
  let git_executor = &with_dirs_of(git_executor, repository_path)?;
  write_prepared(git_executor, repository_path, options, || {
    let grouping = read_grouping_inputs(git_executor, repository_path)?;
    let ahead = CommitsAhead::read(git_executor, repository_path)?;
    let grouped = group_commits(&ahead.commits, &grouping);
    if !grouped.groups.values().any(|group| group.as_deref() == Some(branch)) {
      return Err(NeedError::UnknownBranch(branch.to_string()));
    }
    let mut combined = grouping
      .needs
      .entries
      .get(branch)
      .map(|value| parse_needs(branch, value))
      .transpose()
      .map_err(NeedError::Invalid)?
      .unwrap_or_default();
    combined.extend(keys.iter().cloned());
    prepare_lines(git_executor, repository_path, &grouping.needs, branch, Some(&combined))
  })
}

/// Remove `needed` from the declared needs of `branch`; the branch may have no commits any more.
#[instrument(skip(git_executor, options))]
pub fn unneed_core(git_executor: &GitCommandExecutor, repository_path: &str, branch: &str, needed: &str, options: &RewriteOptions) -> Result<NeedOutcome, NeedError> {
  validate_leaf_name(branch, "branch name").map_err(NeedError::Invalid)?;
  let key = NeedKey::parse(needed).map_err(NeedError::Invalid)?;
  if key.companion.is_none() && key.branch == branch {
    return Err(NeedError::Invalid(format!("{branch} cannot need itself")));
  }
  let git_executor = &with_dirs_of(git_executor, repository_path)?;
  write_prepared(git_executor, repository_path, options, || {
    let read = NEEDS.read(git_executor, repository_path)?;
    let current = read
      .entries
      .get(branch)
      .map(|value| parse_needs(branch, value))
      .transpose()
      .map_err(NeedError::Invalid)?
      .unwrap_or_default();
    if !current.contains(&key) {
      return Err(NeedError::NotDeclared {
        branch: branch.to_string(),
        needed: needed.to_string(),
      });
    }
    let mut remaining = current;
    remaining.remove(&key);
    prepare_lines(git_executor, repository_path, &read, branch, (!remaining.is_empty()).then_some(&remaining))
  })
}

fn parse_request(branch: &str, needed: &[&str]) -> Result<BTreeSet<NeedKey>, NeedError> {
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

fn write_prepared(
  git_executor: &GitCommandExecutor,
  repository_path: &str,
  options: &RewriteOptions,
  prepare: impl Fn() -> Result<PreparedNeeds, NeedError>,
) -> Result<NeedOutcome, NeedError> {
  let prepared = match prepare() {
    Ok(prepared) if prepared.written.is_none() => return Ok(prepared.outcome),
    prepared => prepared,
  };
  let (prepared, lock) = options.lock_prepared(git_executor, repository_path, prepared, &prepare)?;
  match prepared.write(git_executor, repository_path) {
    Err(NeedError::Moved(_)) if lock.is_some() => prepare()?.write(git_executor, repository_path),
    written => written,
  }
}

struct PreparedNeeds {
  outcome: NeedOutcome,
  written: Option<WrittenCommits>,
  removes: bool,
}

impl Prepared for PreparedNeeds {
  fn witness(&self) -> Witness {
    Witness::default()
  }

  fn discard(&self, git_executor: &GitCommandExecutor, repository_path: &str) {
    if let Some(written) = &self.written {
      written.discard(git_executor, repository_path);
    }
  }
}

impl PreparedNeeds {
  fn write(mut self, git_executor: &GitCommandExecutor, repository_path: &str) -> Result<NeedOutcome, NeedError> {
    let Some(written) = self.written.take() else {
      return Ok(self.outcome);
    };
    let previous = self.outcome.previous_commit.clone();
    let new_commit = written.store_commits.first().cloned().unwrap_or_default();
    let mut transaction = RefTransaction::new();
    match &previous {
      Some(previous) => transaction.update(NEEDS_REF, &new_commit, Some(previous)),
      None => transaction.create(NEEDS_REF, &new_commit),
    };
    written.delete_temp_refs(&mut transaction);
    if let Err(error) = transaction.commit(git_executor, repository_path, if self.removes { "unneed" } else { "need" }) {
      written.discard(git_executor, repository_path);
      return Err(match crate::store_lines::read_store_head(git_executor, repository_path, NEEDS_REF) {
        Ok(actual) if actual.as_ref().map(|(commit, _)| commit) != previous.as_ref() => NeedError::Moved(NEEDS.moved(previous, actual.map(|(commit, _)| commit))),
        _ => NeedError::from(error),
      });
    }
    self.outcome.new_commit = Some(new_commit);
    info!(branch = %self.outcome.branch, removes = self.removes, "Wrote declared needs");
    Ok(self.outcome)
  }
}

fn prepare_lines(git_executor: &GitCommandExecutor, repository_path: &str, read: &StoreLines, branch: &str, needs: Option<&BTreeSet<NeedKey>>) -> Result<PreparedNeeds, NeedError> {
  let previous_needs = read
    .entries
    .get(branch)
    .map(|value| parse_needs(branch, value))
    .transpose()
    .map_err(NeedError::Invalid)?
    .unwrap_or_default();
  let needs_text = needs.map(|keys| keys.iter().map(ToString::to_string).collect::<Vec<_>>().join(" "));
  let next_needs = needs.cloned().unwrap_or_default();
  let mut entries = read.entries.clone();
  match &needs_text {
    Some(value) => {
      entries.insert(branch.to_string(), value.clone());
    }
    None => {
      entries.remove(branch);
    }
  }
  let mut prepared = PreparedNeeds {
    outcome: NeedOutcome {
      branch: branch.to_string(),
      previous_needs: previous_needs.iter().map(ToString::to_string).collect(),
      needs: next_needs.iter().map(ToString::to_string).collect(),
      previous_commit: read.commit.clone(),
      new_commit: None,
    },
    written: None,
    removes: needs.is_none_or(|next| next.len() < previous_needs.len()),
  };
  let named = |key: &str| key == branch;
  let had_problem = read.problems.iter().any(|problem| problem.key.as_deref().is_some_and(named));
  if entries == read.entries && !had_problem {
    return Ok(prepared);
  }
  let kept: Vec<String> = read
    .problems
    .iter()
    .filter(|problem| !problem.key.as_deref().is_some_and(named))
    .map(|problem| problem.line.clone())
    .collect();
  let content = format_store_lines(&entries, &kept);
  let update = NEEDS.update(read.commit.as_deref(), &content);
  prepared.written = Some(write_commits_with_notes(
    git_executor,
    repository_path,
    &[],
    None,
    std::slice::from_ref(&update.commit),
    None,
    Signing::Off,
  )?);
  Ok(prepared)
}

/// Drop integrated local references from the needs store; each branch keeps its other declared needs.
pub fn drop_integrated_needs(git_executor: &GitCommandExecutor, repository_path: &str, integrated: &[&str]) {
  drop_store_lines(git_executor, repository_path, &[needs_drop(integrated)]);
}
