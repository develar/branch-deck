//! The record of a run of `bd` in the operations registry of its repository (docs/design/git-operations.md,
//! "Operations registry"; docs/design/operations.md, decision 7), as the operation of the command
//! (`RepoContext::operation_id`, the `operation_id` of its JSON and of its log lines), with what the app's admission of
//! the same intent holds (`operations::engine::request_of`, `unplanned_request`), and its entry in the journal of the
//! operations once it ended, with what it cost from its admission on (`Entry::finish`):
//!
//! - an operation of the remote (`bd push`, `bd safe-push`, `bd request-review`) is admitted once its plan holds
//!   (`admit_planned`), `busy` while another process (the app, another `bd`) runs what it needs, a start whose answer was
//!   lost in its way settled first over the services of the run; its run is the engine's (`run_admitted`);
//! - a rewrite of the checked out branch (`bd group`, `rename`, `restack`, `unstack`, `reword`, `fold`, `park`,
//!   `unpark`, `unapply`, `reapply`, `undo`, `move-hunk`), `bd update` and `bd pick` are recorded before the command
//!   takes any lock (`recorded`): HEAD, which they hold, makes no other process busy (the repository lock orders them,
//!   `domain_policy::admission::holds_across`), so the record is what the app's windows show of them ("bd: Unstacking
//!   beta") and what the journal keeps; only a ref one of them holds besides (the unapplied branch a reapply takes) makes
//!   an operation of another process on it busy.

use crate::context::RepoContext;
use crate::error::CliError;
use crate::services::session_key;
use domain_types::error::{ApiError, ErrorCode, HolderTool};
use domain_types::operation::{Intent, OperationId, OperationResult, OperationStats, Outcome};
use domain_types::rewrite::TableWitness;
use domain_types::session::StoreTips;
use operations::engine::{request_of, unplanned_request};
use operations::land_execute::{Planned, StartServices, Started};
use operations::reconcile::{ReconcileSources, SettleWith, admit_settled};
use operations::registry::OperationRequest;
use operations::registry_file::{Entry, ProcessRegistry};
use sync_utils::timestamp::unix_now;

/// The operation of the command in the words of its record
fn operation_of(context: &RepoContext) -> OperationId {
  OperationId(context.operation_id.to_string())
}

/// The record of the command asking `request` in the operations registry of its repository, admitted unless an
/// operation of another process holds what it needs; a start whose answer was lost in its way is settled first as
/// `settle` says (`reconcile::admit_settled`: one that started no robot leaves, one that did refuses the run as
/// `plan_changed`, one not known yet is `busy` with how to forget it; none: it stays `busy`). The command holds the
/// record until its run ends it (`run_admitted`, `Entry::finish`)
///
/// # Errors
/// As `admit_settled`: `busy`, naming the holder; `locked` while another process keeps the lock of the registry;
/// `git_error` when the file can't be read or written
pub fn admit(context: &RepoContext, request: &OperationRequest, settle: Option<SettleWith<'_>>) -> Result<Entry, ApiError> {
  let registry = ProcessRegistry::of(&context.git_common_dir);
  let runtime = context.runtime().map_err(|error| ApiError::internal(error.message))?;
  let admitted = runtime.block_on(admit_settled(&registry, &operation_of(context), HolderTool::Bd, &session_key(context), request, settle));
  admitted.entry
}

/// The record of the run of `planned`, a plan of an operation of the remote, over the services of the run (`services`):
/// what the app's admission of the same plan holds (`engine::request_of`), a start whose answer was lost settled over
/// those services
///
/// # Errors
/// As `admit`
pub fn admit_planned(context: &RepoContext, services: &StartServices, planned: &Planned) -> Result<Entry, ApiError> {
  let key = session_key(context);
  let sources = ReconcileSources::of_start(services, &key, context.patronus_new_api, unix_now());
  admit(context, &request_of(&context.prefix, planned), Some(SettleWith::Sources(sources)))
}

/// Runs `run`, the run of the operation `entry` admitted, on the runtime of the command, counting what it costs from its
/// admission on (`OperationTally`); then its record leaves the registry with its entry in the journal of the operations
/// (`Entry::finish`), as the app's run ends
///
/// # Errors
/// What `run` answers
pub fn run_admitted(context: &RepoContext, entry: Entry, run: impl Future<Output = Result<Started, ApiError>>) -> Result<Started, ApiError> {
  let started = match context.runtime() {
    Ok(runtime) => runtime.block_on(entry.tally().counting(run)),
    Err(error) => Err(ApiError::internal(error.message)),
  };
  let stats = entry.stats();
  entry.finish(started.as_ref().map(|started| &started.outcome), stats);
  started
}

/// Runs `run`, the command, as the operation `intent` of a family whose request names no plan (a rewrite, an update, a
/// pick: `engine::unplanned_request`) recorded in the operations registry of its repository before it takes any lock,
/// counting what it costs; then the record leaves with its entry in the journal: the error of the command, or an outcome
/// that tells no effect (its JSON tells what it did)
///
/// # Errors
/// What `run` answers; `busy` while an operation of another process holds a ref it needs besides HEAD, as `admit`
pub fn recorded<T>(context: &RepoContext, intent: &Intent, run: impl FnOnce() -> Result<T, CliError>) -> Result<T, CliError> {
  let Some(request) = unplanned_request(&context.prefix, intent) else {
    return Err(CliError::new(ErrorCode::InvalidArguments, format!("bd records no {:?} without its plan", intent.kind())));
  };
  let op = operation_of(context);
  let entry = ProcessRegistry::of(&context.git_common_dir).admit(&op, HolderTool::Bd, &session_key(context), &request)?;
  let ran = entry.tally().scope(run);
  let stats = entry.stats();
  match &ran {
    Ok(_) => entry.finish(Ok(&told_by_its_output(op)), stats),
    Err(error) => entry.finish(Err(&ApiError::new(error.code, error.message.clone())), stats),
  }
  ran
}

/// `run` as `recorded` does, but when it writes nothing (`reads_only`: a dry run, a status): then as it is, no record
///
/// # Errors
/// As `recorded`
pub fn recorded_unless<T>(context: &RepoContext, reads_only: bool, intent: &Intent, run: impl FnOnce() -> Result<T, CliError>) -> Result<T, CliError> {
  if reads_only { run() } else { recorded(context, intent, run) }
}

/// The table a command asks its rewrite on, as its record names it: the HEAD of its `--expect-head`, none else (its core
/// rewrites HEAD as it finds it under the lock); the stores it never names
#[must_use]
pub fn asked_on(expect_head: Option<&str>) -> TableWitness {
  TableWitness {
    head: expect_head.unwrap_or_default().to_owned(),
    store_tips: StoreTips::default(),
  }
}

/// The outcome of a command whose JSON tells what it did: no effect, no answer of a family
fn told_by_its_output(op: OperationId) -> Outcome {
  Outcome {
    op,
    effects: Vec::new(),
    news_key: None,
    version_after: None,
    result: OperationResult::None,
    undo: None,
    stats: OperationStats::default(),
  }
}
