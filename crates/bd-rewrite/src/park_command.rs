//! `bd park` and `bd unpark`: keep a branch local (`refs/branch-deck/parked`), so that `bd push` and `bd safe-push` never
//! send it and the app shows it apart from the branches to land (spec/branch/parked-branches.spec.md)

use bd_core::context::RepoContext;
use bd_core::error::{CliError, ErrorCode};
use bd_core::output::SchemaVersion;
use bd_core::sync_command::lock;
use history_rewrite::baseline::NoBaseline;
use history_rewrite::parked::{ParkError, ParkOutcome, park_core, unpark_core};
use history_rewrite::repository_lock::RewriteOptions;
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
  locked(context, |options| park_core(&context.git, &context.repository, &args.branch, &args.note, options))
}

pub fn unpark(context: &RepoContext, args: &UnparkArgs) -> Result<ParkOutput, CliError> {
  locked(context, |options| unpark_core(&context.git, &context.repository, &args.branch, options))
}

/// `write` under the repository lock that bd takes (a wait for it reads the config again), and its output
fn locked(context: &RepoContext, write: impl FnOnce(&RewriteOptions) -> Result<ParkOutcome, ParkError>) -> Result<ParkOutput, CliError> {
  let held = lock(context)?;
  let options = RewriteOptions {
    held_lock: Some(&held),
    lock_timeout: context.lock_timeout,
    ..RewriteOptions::default()
  };
  let outcome = write(&options).map_err(park_error)?;
  Ok(ParkOutput {
    schema: SchemaVersion,
    repository: context.repository.clone(),
    parked: outcome.note.is_some(),
    changed: outcome.new_commit.is_some(),
    branch: outcome.branch,
    note: outcome.note,
    previous_note: outcome.previous_note,
  })
}

/// The error code of a refused or failed park; by value, for `map_err`
#[expect(clippy::needless_pass_by_value, reason = "used as `map_err(park_error)`")]
fn park_error(error: ParkError) -> CliError {
  let code = match &error {
    ParkError::UnknownBranch(_) => ErrorCode::UnknownBranch,
    ParkError::NotParked(_) => ErrorCode::NotParked,
    ParkError::InvalidNote(_) => ErrorCode::InvalidArguments,
    ParkError::Moved(_) => ErrorCode::ParkedMoved,
    ParkError::Locked(_) => ErrorCode::Locked,
    ParkError::Other(error) if error.downcast_ref::<NoBaseline>().is_some() => ErrorCode::NoBaseline,
    ParkError::Other(_) => ErrorCode::GitError,
  };
  CliError::new(code, error.to_string())
}
