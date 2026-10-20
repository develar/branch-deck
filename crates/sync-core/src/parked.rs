//! Parked branches for the app and `bd` (spec/branch/parked-branches.spec.md): park and unpark over the cores of
//! `history_rewrite::parked`, and whether a branch is parked as a sync places it: its own line, or the line of a branch
//! it is built on, whose commits it carries.

use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use history_rewrite::parked::{ParkOutcome, park_core, unpark_core};
use history_rewrite::repository_lock::RewriteOptions;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use sync_types::GroupedBranchInfo;
use tracing::instrument;

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ParkBranchParams {
  pub repository_path: String,
  /// The branch (a leaf with stacked branches on)
  pub branch_name: String,
  /// Why it is parked, one line; empty for none. Parking a parked branch changes its note
  pub note: String,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct UnparkBranchParams {
  pub repository_path: String,
  pub branch_name: String,
}

/// What a park or an unpark did
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ParkResult {
  pub branch_name: String,
  /// The note afterwards; `None` when the branch is not parked any more
  pub note: Option<String>,
  /// The note before; `None` when the branch was not parked
  pub previous_note: Option<String>,
  /// Whether the store changed
  pub changed: bool,
}

impl From<ParkOutcome> for ParkResult {
  fn from(outcome: ParkOutcome) -> Self {
    Self {
      changed: outcome.new_commit.is_some(),
      branch_name: outcome.branch,
      note: outcome.note,
      previous_note: outcome.previous_note,
    }
  }
}

/// Park a branch, or change the note of a parked one, under the repository lock (`park_core`)
#[instrument(skip(git_executor, params), fields(branch = %params.branch_name))]
pub fn park_branch(git_executor: &GitCommandExecutor, params: &ParkBranchParams) -> Result<ParkResult> {
  Ok(park_core(git_executor, &params.repository_path, &params.branch_name, &params.note, &RewriteOptions::default())?.into())
}

/// Unpark a branch under the repository lock (`unpark_core`)
#[instrument(skip(git_executor, params), fields(branch = %params.branch_name))]
pub fn unpark_branch(git_executor: &GitCommandExecutor, params: &UnparkBranchParams) -> Result<ParkResult> {
  Ok(unpark_core(git_executor, &params.repository_path, &params.branch_name, &RewriteOptions::default())?.into())
}

/// Why a branch is parked
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parked {
  /// The note of its line, or of the line of `with`
  pub note: String,
  /// The parked branch it is built on, whose commits it carries; `None` when its own line parks it
  pub with: Option<String>,
}

impl Parked {
  /// "auth is parked: waits for IJPL-1", "oauth is built on parked auth"
  pub fn reason(&self, name: &str) -> String {
    let note = if self.note.is_empty() { String::new() } else { format!(": {}", self.note) };
    match &self.with {
      None => format!("{name} is parked{note}"),
      Some(parent) => format!("{name} is built on parked {parent}{note}"),
    }
  }
}

/// Whether `name` is parked among `branches` as a sync placed them, by `parked` (branch -> note): its own line, else the
/// nearest branch it is built on (`effective_base`, else the declared parent of a blocked branch) that has one
pub fn parked_state(branches: &[GroupedBranchInfo], parked: &BTreeMap<String, String>, name: &str) -> Option<Parked> {
  if let Some(note) = parked.get(name) {
    return Some(Parked { note: note.clone(), with: None });
  }
  let parent_of = |name: &str| {
    let stack = branches.iter().find(|branch| branch.name == name)?.stack.as_ref()?;
    stack.effective_base.clone().or_else(|| stack.declared_parent.clone())
  };
  let mut seen = vec![name.to_string()];
  let mut parent = parent_of(name);
  while let Some(ancestor) = parent {
    if seen.contains(&ancestor) {
      return None;
    }
    if let Some(note) = parked.get(&ancestor) {
      return Some(Parked {
        note: note.clone(),
        with: Some(ancestor),
      });
    }
    parent = parent_of(&ancestor);
    seen.push(ancestor);
  }
  None
}
