//! Parked branches for the app and `bd` (spec/branch/parked-branches.spec.md): park and unpark over the cores of
//! `history_rewrite::parked`, and whether a branch is parked as a sync places it: its own line, or the line of a branch
//! it is built on, whose commits it carries.

use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use history_rewrite::parked::{ParkLine, ParkOutcome, park_core, unpark_core};
use history_rewrite::repository_lock::RewriteOptions;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use sync_types::GroupedBranchInfo;
use tracing::instrument;

/// A branch to park and its note
#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ParkBranchLine {
  /// The branch (a leaf with stacked branches on)
  pub branch_name: String,
  /// Why it is parked, one line; empty for none. Parking a parked branch changes its note
  pub note: String,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ParkBranchesParams {
  pub repository_path: String,
  /// Written in one store commit: an unknown branch parks none
  pub branches: Vec<ParkBranchLine>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct UnparkBranchesParams {
  pub repository_path: String,
  /// Dropped in one store commit: a branch that is not parked unparks none
  pub branch_names: Vec<String>,
}

/// What a park or an unpark did to one branch
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ParkedBranchChange {
  pub branch_name: String,
  /// The note afterwards; `None` when the branch is not parked any more
  pub note: Option<String>,
  /// The note before; `None` when it was not parked
  pub previous_note: Option<String>,
}

/// What a park or an unpark did
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ParkResult {
  /// One per branch named, in the order named
  pub branches: Vec<ParkedBranchChange>,
  /// Whether the store changed
  pub changed: bool,
}

impl From<ParkOutcome> for ParkResult {
  fn from(outcome: ParkOutcome) -> Self {
    Self {
      changed: outcome.new_commit.is_some(),
      branches: outcome
        .branches
        .into_iter()
        .map(|change| ParkedBranchChange {
          branch_name: change.branch,
          note: change.note,
          previous_note: change.previous_note,
        })
        .collect(),
    }
  }
}

/// Park branches, or change the notes of parked ones, under the repository lock (`park_core`)
#[instrument(skip(git_executor, params), fields(branches = params.branches.len()))]
pub fn park_branches(git_executor: &GitCommandExecutor, params: &ParkBranchesParams) -> Result<ParkResult> {
  let lines: Vec<ParkLine<'_>> = params
    .branches
    .iter()
    .map(|line| ParkLine {
      branch: &line.branch_name,
      note: &line.note,
    })
    .collect();
  Ok(park_core(git_executor, &params.repository_path, &lines, &RewriteOptions::default())?.into())
}

/// Unpark branches under the repository lock (`unpark_core`)
#[instrument(skip(git_executor, params), fields(branches = params.branch_names.len()))]
pub fn unpark_branches(git_executor: &GitCommandExecutor, params: &UnparkBranchesParams) -> Result<ParkResult> {
  let branches: Vec<&str> = params.branch_names.iter().map(String::as_str).collect();
  Ok(unpark_core(git_executor, &params.repository_path, &branches, &RewriteOptions::default())?.into())
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
