//! Parked branches for the app and `bd` (spec/branch/parked-branches.spec.md): park and unpark over the cores of
//! `history_rewrite::parked`. Whether a branch is parked as a sync places it is the rule `domain_policy::parked`.

use anyhow::Result;
use domain_types::rewrite::{ParkBranchLine, ParkResult, ParkedBranchChange};
use git_executor::git_command_executor::GitCommandExecutor;
use history_rewrite::parked::{ParkLine, ParkOutcome, park_core, unpark_core};
use history_rewrite::repository_lock::{LockWait, RewriteOptions};
use tracing::instrument;

#[derive(Debug, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ParkBranchesParams {
  pub repository_path: String,
  /// Written in one store commit: an unknown branch parks none
  pub branches: Vec<ParkBranchLine>,
}

#[derive(Debug, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct UnparkBranchesParams {
  pub repository_path: String,
  /// Dropped in one store commit: a branch that is not parked unparks none
  pub branch_names: Vec<String>,
}

/// What a park or an unpark that wrote `outcome` did
pub fn park_result(outcome: ParkOutcome) -> ParkResult {
  ParkResult {
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

/// Park branches, or change the notes of parked ones, under the repository lock (`park_core`), waited for as
/// `lock_wait` says
#[instrument(skip(git_executor, params), fields(branches = params.branches.len()))]
pub fn park_branches(git_executor: &GitCommandExecutor, params: &ParkBranchesParams, lock_wait: LockWait) -> Result<ParkResult> {
  let lines: Vec<ParkLine<'_>> = params
    .branches
    .iter()
    .map(|line| ParkLine {
      branch: &line.branch_name,
      note: &line.note,
    })
    .collect();
  let options = RewriteOptions {
    lock_wait,
    ..RewriteOptions::default()
  };
  Ok(park_result(park_core(git_executor, &params.repository_path, &lines, &options)?))
}

/// Unpark branches under the repository lock (`unpark_core`), waited for as `lock_wait` says
#[instrument(skip(git_executor, params), fields(branches = params.branch_names.len()))]
pub fn unpark_branches(git_executor: &GitCommandExecutor, params: &UnparkBranchesParams, lock_wait: LockWait) -> Result<ParkResult> {
  let branches: Vec<&str> = params.branch_names.iter().map(String::as_str).collect();
  let options = RewriteOptions {
    lock_wait,
    ..RewriteOptions::default()
  };
  Ok(park_result(unpark_core(git_executor, &params.repository_path, &branches, &options)?))
}
