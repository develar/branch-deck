//! `brd need`: declare that a branch lands only after the branches it names (`refs/branch-deck/needs`), so that a
//! Safe Push, a dry run and a land of it refuse to start before them (spec/branch/declared-needs.spec.md); recorded in
//! the operations registry of the repository before the lock, as every rewrite of `bd` (`bd_core::operation_record`),
//! the lock taken by the core after it read the store

use bd_core::context::RepoContext;
use bd_core::error::CliError;
use bd_core::operation_record::{asked_on, recorded};
use bd_core::output::SchemaVersion;
use domain_types::error::ErrorCode;
use domain_types::operation::Intent;
use domain_types::rewrite::RewriteIntent;
use history_rewrite::needs::{NeedError, NeedOutcome, need_core, unneed_core};
use history_rewrite::repository_lock::{LockWait, RewriteOptions};
use schemars::JsonSchema;
use serde::Serialize;

#[derive(Debug, clap::Args)]
pub struct NeedArgs {
  /// The branch (group name) that lands last
  pub branch: String,
  /// What lands before it: a branch of this repository, or `<companion>:<branch>`
  #[arg(required = true, num_args = 1..)]
  pub needs: Vec<String>,
  /// Remove the named needs instead of declaring them
  #[arg(long)]
  pub remove: bool,
}

/// `brd need`
#[derive(Debug, Serialize, JsonSchema)]
pub struct NeedOutput {
  pub schema: SchemaVersion,
  pub repository: String,
  pub branch: String,
  /// What lands before the branch afterwards, sorted
  pub needs: Vec<String>,
  /// What landed before it until now, sorted
  pub previous_needs: Vec<String>,
  /// Whether this call wrote the store
  pub changed: bool,
}

pub fn need(context: &RepoContext, args: &NeedArgs) -> Result<NeedOutput, CliError> {
  if args.remove {
    return remove(context, args);
  }
  let intent = RewriteIntent::Need {
    branch_name: args.branch.clone(),
    needs: args.needs.clone(),
    witness: asked_on(None),
  };
  let needs: Vec<&str> = args.needs.iter().map(String::as_str).collect();
  locked(context, intent, |options| need_core(&context.git, &context.repository, &args.branch, &needs, options))
}

/// `brd need --remove`: every named need goes in one write, the first one that is not declared refuses
fn remove(context: &RepoContext, args: &NeedArgs) -> Result<NeedOutput, CliError> {
  let intent = RewriteIntent::Unneed {
    branch_name: args.branch.clone(),
    needs: args.needs.clone(),
    witness: asked_on(None),
  };
  locked(context, intent, |options| {
    let mut outcome: Option<NeedOutcome> = None;
    for needed in &args.needs {
      let removed = unneed_core(&context.git, &context.repository, &args.branch, needed, options)?;
      outcome = Some(match outcome.take() {
        Some(first) => NeedOutcome {
          previous_needs: first.previous_needs,
          previous_commit: first.previous_commit,
          ..removed
        },
        None => removed,
      });
    }
    outcome.ok_or_else(|| NeedError::Invalid("At least one needed branch is required".to_string()))
  })
}

/// `write`, the rewrite `intent`, recorded in the operations registry, and its output; its core takes the repository
/// lock after it read the store, for the move of the store only
fn locked(context: &RepoContext, intent: RewriteIntent, write: impl FnOnce(&RewriteOptions) -> Result<NeedOutcome, NeedError>) -> Result<NeedOutput, CliError> {
  recorded(context, &Intent::Rewrite(Box::new(intent)), || written(context, write))
}

/// `locked` once recorded
fn written(context: &RepoContext, write: impl FnOnce(&RewriteOptions) -> Result<NeedOutcome, NeedError>) -> Result<NeedOutput, CliError> {
  let options = RewriteOptions {
    lock_wait: LockWait::Bounded(context.lock_timeout),
    ..RewriteOptions::default()
  };
  let outcome = write(&options).map_err(need_error)?;
  Ok(NeedOutput {
    schema: SchemaVersion,
    repository: context.repository.clone(),
    changed: outcome.needs != outcome.previous_needs,
    branch: outcome.branch,
    needs: outcome.needs,
    previous_needs: outcome.previous_needs,
  })
}

/// The error code of a refused or failed need; by value, for `map_err`
#[expect(clippy::needless_pass_by_value, reason = "used as `map_err(need_error)`")]
pub(crate) fn need_error(error: NeedError) -> CliError {
  CliError::new(error.code().unwrap_or(ErrorCode::GitError), error.to_string())
}
