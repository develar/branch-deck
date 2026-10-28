//! `bd park` and `bd unpark`: keep a branch local (`refs/branch-deck/parked`), so that `bd push` and `bd safe-push` never
//! send it and the app shows it apart from the branches to land (spec/branch/parked-branches.spec.md); recorded in the
//! operations registry of the repository before the lock, as every rewrite of `bd` (`bd_core::operation_record`), the
//! lock taken by the core after it read the store

use bd_core::context::RepoContext;
use bd_core::error::CliError;
use bd_core::operation_record::{asked_on, recorded};
use bd_core::output::SchemaVersion;
use domain_types::error::ErrorCode;
use domain_types::operation::Intent;
use domain_types::rewrite::{ParkBranchLine, RewriteIntent};
use history_rewrite::parked::{ParkError, ParkLine, ParkOutcome, park_core, unpark_core};
use history_rewrite::repository_lock::{LockWait, RewriteOptions};
use schemars::JsonSchema;
use serde::Serialize;

#[derive(Debug, clap::Args)]
pub struct ParkArgs {
  /// The branch (group name)
  pub branch: String,
  /// Why it stays local, one line, e.g. "waits for the fix of IJPL-1"; parking a parked branch changes its note
  #[arg(long, default_value = "")]
  pub note: String,
}

#[derive(Debug, clap::Args)]
pub struct UnparkArgs {
  /// The branch (group name); also one that no longer groups a commit
  pub branch: String,
}

/// `bd park` and `bd unpark`
#[derive(Debug, Serialize, JsonSchema)]
pub struct ParkOutput {
  pub schema: SchemaVersion,
  pub repository: String,
  pub branch: String,
  /// Whether the branch is parked afterwards
  pub parked: bool,
  /// The note afterwards; omitted when not parked
  #[serde(skip_serializing_if = "Option::is_none")]
  pub note: Option<String>,
  /// The note before; omitted when it was not parked
  #[serde(skip_serializing_if = "Option::is_none")]
  pub previous_note: Option<String>,
  /// Whether this call wrote the store
  pub changed: bool,
}

pub fn park(context: &RepoContext, args: &ParkArgs) -> Result<ParkOutput, CliError> {
  let intent = RewriteIntent::Park {
    branches: vec![ParkBranchLine {
      branch_name: args.branch.clone(),
      note: args.note.clone(),
    }],
    witness: asked_on(None),
  };
  locked(context, intent, |options| {
    park_core(
      &context.git,
      &context.repository,
      &[ParkLine {
        branch: &args.branch,
        note: &args.note,
      }],
      options,
    )
  })
}

pub fn unpark(context: &RepoContext, args: &UnparkArgs) -> Result<ParkOutput, CliError> {
  let intent = RewriteIntent::Unpark {
    branch_names: vec![args.branch.clone()],
    witness: asked_on(None),
  };
  locked(context, intent, |options| unpark_core(&context.git, &context.repository, &[args.branch.as_str()], options))
}

/// `write`, the rewrite `intent`, recorded in the operations registry, and its output; its core takes the repository
/// lock after it read the store, for the move of the store only
fn locked(context: &RepoContext, intent: RewriteIntent, write: impl FnOnce(&RewriteOptions) -> Result<ParkOutcome, ParkError>) -> Result<ParkOutput, CliError> {
  recorded(context, &Intent::Rewrite(Box::new(intent)), || written(context, write))
}

/// `locked` once recorded
fn written(context: &RepoContext, write: impl FnOnce(&RewriteOptions) -> Result<ParkOutcome, ParkError>) -> Result<ParkOutput, CliError> {
  let options = RewriteOptions {
    lock_wait: LockWait::Bounded(context.lock_timeout),
    ..RewriteOptions::default()
  };
  let outcome = write(&options).map_err(park_error)?;
  let changed = outcome.new_commit.is_some();
  // one branch named, so one change
  let change = outcome
    .branches
    .into_iter()
    .next()
    .ok_or_else(|| CliError::new(ErrorCode::GitError, "The park reported no branch"))?;
  Ok(ParkOutput {
    schema: SchemaVersion,
    repository: context.repository.clone(),
    parked: change.note.is_some(),
    changed,
    branch: change.branch,
    note: change.note,
    previous_note: change.previous_note,
  })
}

/// The error code of a refused or failed park; by value, for `map_err`
#[expect(clippy::needless_pass_by_value, reason = "used as `map_err(park_error)`")]
pub(crate) fn park_error(error: ParkError) -> CliError {
  CliError::new(error.code().unwrap_or(ErrorCode::GitError), error.to_string())
}
