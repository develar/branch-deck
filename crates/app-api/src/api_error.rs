//! `api_error`: a core's error as the `ApiError` a command of the app answers, the same in the app and in the e2e
//! server. One chain of downcasts over the typed errors that reach a command; the code of a typed error with several
//! comes from the type (`RegroupError::code`, `SyncDeferred::code`, `ParkError::code`, `GuardRefusal::code`,
//! `UnapplyRefusal::code`, `ReapplyRefusal::code`), which the commands of `bd` read too. An undo answers the code
//! `in_undo` (`undo_error`), as `bd undo` does. A code is never picked by reading a message; an error without a code of
//! its own is `internal` with its whole context chain.

use app_settings::service::SettingsError;
use cli_tools::install::ActionError;
use domain_types::error::{ApiError, ErrorCode, ErrorDetails, HolderTool, LockHolderInfo};
use domain_types::landing::{LandingRefusal, LandingRefused};
use file_diffs::accio::SearchCanceled;
use git_executor::git_command_executor::StoppedCommand;
use git_ops::index_lock::IndexLocked;
use git_ops::revs::DetachedHead;
use git_ops::rewrite_guard::HeadMoved;
use git_ops::unapply::{DropRefused, MoveBlocked};
use git_ops::update_branch::{BranchSwitched, FilesBlocked, GuardRefusal, HeadAttached, NotAnUpdate, PendingUpdate, RefLocked, UndoBlocked};
use github::device_flow::DeviceFlowError;
use history_rewrite::baseline::NoBaseline;
use history_rewrite::parked::ParkError;
use history_rewrite::regroup::RegroupError;
use history_rewrite::repository_lock::{LockHolder, LockWaitCanceled, RepositoryLocked};
use std::time::{SystemTime, UNIX_EPOCH};
use sync_core::reapply_branch::ReapplyRefusal;
use sync_core::sync::SyncDeferred;
use sync_core::unapply_branch::UnapplyRefusal;
use update_core::update_project::{FetchFailed, NoUpstream};
use youtrack::sign_in::SignInError;

/// The error a command answers for `error`: its code, the data of the code (`details`) and the retry of its class; by
/// value, for `map_err`
#[expect(clippy::needless_pass_by_value, reason = "used as `map_err(api_error)`")]
pub fn api_error(error: anyhow::Error) -> ApiError {
  let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| since.as_secs());
  api_error_at(&error, now)
}

/// The error an undo command answers for `error`: `api_error` with its code `in_undo` (`ErrorCode::in_undo`: a ref that
/// moved on since the operation makes the token stale, `undo_stale`, never retried), as `bd undo` answers it, and for a
/// stale one the words that say so instead of the advice to retry; by value, for `map_err`
pub fn undo_error(error: anyhow::Error) -> ApiError {
  let stale_words = error
    .downcast_ref::<HeadMoved>()
    .map(HeadMoved::undo_message)
    .or_else(|| error.downcast_ref::<RegroupError>().and_then(RegroupError::undo_message));
  let mut api_error = api_error(error).in_undo();
  if api_error.code == ErrorCode::UndoStale
    && let Some(words) = stale_words
  {
    api_error.message = words;
  }
  api_error
}

/// What a step of a command fails with, as its `ApiError`: a core's error through `api_error`, a message without a code
/// as `internal`, an `ApiError` as it is. Both adapters bound their blocking steps by it (`run_blocking` of the app,
/// `run_command` of the e2e server); a trait of this crate, because no `From` of a foreign error into `ApiError` can be
/// written outside `domain-types`
pub trait IntoApiError {
  fn into_api_error(self) -> ApiError;
}

impl IntoApiError for ApiError {
  fn into_api_error(self) -> ApiError {
    self
  }
}

impl IntoApiError for anyhow::Error {
  fn into_api_error(self) -> ApiError {
    api_error(self)
  }
}

impl IntoApiError for String {
  fn into_api_error(self) -> ApiError {
    ApiError::internal(self)
  }
}

impl IntoApiError for &str {
  fn into_api_error(self) -> ApiError {
    ApiError::internal(self)
  }
}

/// A sign-in to YouTrack: `canceled` when a newer sign-in or a cancel ended it
impl IntoApiError for SignInError {
  fn into_api_error(self) -> ApiError {
    api_error(self.into())
  }
}

/// A sign-in to GitHub: `canceled` when a newer sign-in or a cancel ended it
impl IntoApiError for DeviceFlowError {
  fn into_api_error(self) -> ApiError {
    api_error(self.into())
  }
}

/// An action of a CLI tool: `canceled` when the user ended it
impl IntoApiError for ActionError {
  fn into_api_error(self) -> ApiError {
    api_error(self.into())
  }
}

/// A request to Patronus that failed, in the words of its service (`PatronusService` answers a message):
/// `patronus_error`, as `bd` answers it
pub fn patronus_error(message: String) -> ApiError {
  ApiError::new(ErrorCode::PatronusError, message)
}

/// A sync of a repository whose branch prefix resolves to nothing (no app setting, no git config): `no_branch_prefix`,
/// as `bd` answers it, in words that say where to set one
pub fn no_branch_prefix() -> ApiError {
  ApiError::new(ErrorCode::NoBranchPrefix, "Cannot sync without a branch prefix: set one in Settings → General")
}

/// `api_error` with the clock at `now` (seconds since the epoch), which `held_for` of a lock holder counts from
pub(crate) fn api_error_at(error: &anyhow::Error, now: u64) -> ApiError {
  if let Some(api_error) = error.downcast_ref::<ApiError>() {
    return api_error.clone();
  }
  if let Some(locked) = repository_locked(error) {
    return locked_error(locked, now);
  }
  let (code, details) = classify(error).unwrap_or((ErrorCode::Internal, None));
  ApiError {
    details,
    ..ApiError::new(code, format!("{error:#}"))
  }
}

/// The held repository lock of `error`: its own, or the one a regroup or a park was refused by
fn repository_locked(error: &anyhow::Error) -> Option<&RepositoryLocked> {
  error.downcast_ref::<RepositoryLocked>().or_else(|| match error.downcast_ref::<RegroupError>() {
    Some(RegroupError::Locked(locked)) => Some(locked),
    _ => match error.downcast_ref::<ParkError>() {
      Some(ParkError::Locked(locked)) => Some(locked),
      _ => None,
    },
  })
}

/// `locked` with the holder of the lock; the words name the holder but not the lock file, an internal detail, and not
/// how long it was held, which `held_for` says
fn locked_error(locked: &RepositoryLocked, now: u64) -> ApiError {
  let message = locked.holder.as_ref().map_or_else(
    || "Branch Deck or bd is changing this repository; try again when it is done".to_string(),
    |holder| format!("`{}` (pid {}) is changing this repository; try again when it is done", holder.command, holder.pid),
  );
  ApiError::new(ErrorCode::Locked, message).with_details(ErrorDetails::Locked {
    holder: locked.holder.as_ref().map(|holder| lock_holder_info(holder, now)),
  })
}

/// The holder as the page names it: `bd` when its program is `bd` or the app binary runs the CLI (`branch-deck cli`),
/// else the app; `held_for` in whole seconds, 0 for a record ahead of this clock
fn lock_holder_info(holder: &LockHolder, now: u64) -> LockHolderInfo {
  let mut words = holder.command.split(' ');
  let program = words.next().unwrap_or_default();
  let is_bd = program.strip_suffix(".exe").unwrap_or(program) == "bd" || words.next() == Some("cli");
  LockHolderInfo {
    tool: if is_bd { HolderTool::Bd } else { HolderTool::App },
    command: holder.command.clone(),
    pid: holder.pid,
    held_for: u32::try_from(now.saturating_sub(holder.since)).unwrap_or(u32::MAX),
  }
}

type Classified = (ErrorCode, Option<ErrorDetails>);

/// The code and the data of a typed error of a core; `None` for an error without a code of its own
fn classify(error: &anyhow::Error) -> Option<Classified> {
  let code = |code: ErrorCode| Some((code, None));
  if let Some(regroup) = error.downcast_ref::<RegroupError>() {
    return match regroup {
      RegroupError::Other(inner) => classify(inner),
      _ => regroup.code().and_then(code),
    };
  }
  if let Some(park) = error.downcast_ref::<ParkError>() {
    return match park {
      ParkError::Other(inner) => classify(inner),
      _ => park.code().and_then(code),
    };
  }
  if let Some(deferred) = error.downcast_ref::<SyncDeferred>() {
    return code(deferred.code());
  }
  if error.downcast_ref::<NoBaseline>().is_some() || error.downcast_ref::<NoUpstream>().is_some() {
    return code(ErrorCode::NoBaseline);
  }
  if is_canceled(error) {
    return code(ErrorCode::Canceled);
  }
  // a lock file of git, not the repository lock of Branch Deck: no holder to name (`ErrorDetails::Locked` is the
  // repository lock), so the page shows its words, which name the file (and the processes of an index lock) and say
  // to remove a stale one
  if error.downcast_ref::<RefLocked>().is_some() || error.downcast_ref::<IndexLocked>().is_some() {
    return code(ErrorCode::Locked);
  }
  if error.downcast_ref::<HeadMoved>().is_some() || error.downcast_ref::<BranchSwitched>().is_some() || error.downcast_ref::<HeadAttached>().is_some() {
    return code(ErrorCode::HeadMoved);
  }
  if let Some(refusal) = error.downcast_ref::<GuardRefusal>() {
    return code(refusal.code());
  }
  if error.downcast_ref::<DetachedHead>().is_some() {
    return code(ErrorCode::DetachedHead);
  }
  if error.downcast_ref::<FetchFailed>().is_some() {
    return code(ErrorCode::NetworkError);
  }
  if error.downcast_ref::<NotAnUpdate>().is_some() {
    return code(ErrorCode::InvalidArguments);
  }
  if error.downcast_ref::<PendingUpdate>().is_some() {
    return code(ErrorCode::FinishPending);
  }
  // a value of a setting Rust reads that is refused, not a save that failed
  if let Some(SettingsError::Invalid { .. } | SettingsError::NotAnObject { .. }) = error.downcast_ref::<SettingsError>() {
    return code(ErrorCode::InvalidArguments);
  }
  classify_branch_moves(error).or_else(|| classify_landing(error))
}

/// The user ended what `error` stopped: the wait for the repository lock, a sign-in, an action of a CLI tool, a symbol
/// search (or a newer search of its window did), or a git that was killed for it (a review fetch the window no longer
/// wants)
fn is_canceled(error: &anyhow::Error) -> bool {
  error.downcast_ref::<LockWaitCanceled>().is_some()
    || matches!(error.downcast_ref::<SignInError>(), Some(SignInError::Canceled))
    || matches!(error.downcast_ref::<DeviceFlowError>(), Some(DeviceFlowError::Canceled))
    || matches!(error.downcast_ref::<ActionError>(), Some(ActionError::Canceled))
    || matches!(error.downcast_ref::<StoppedCommand>(), Some(StoppedCommand::Cancelled))
    || error.is::<SearchCanceled>()
}

/// An unapply, a reapply or the move of the files under them: a refusal of the core, local changes in the way, a drop
/// or a commit that conflicts
fn classify_branch_moves(error: &anyhow::Error) -> Option<Classified> {
  let paths = |paths: &[String]| Some((ErrorCode::UncommittedOverlap, Some(ErrorDetails::Paths { paths: paths.to_vec() })));
  if let Some(refusal) = error.downcast_ref::<UnapplyRefusal>() {
    return Some((refusal.code(), None));
  }
  if let Some(refusal) = error.downcast_ref::<ReapplyRefusal>() {
    let details = match refusal {
      ReapplyRefusal::Conflict { commit, files, .. } => Some(ErrorDetails::Conflict {
        commits: vec![commit.clone()],
        paths: files.clone(),
      }),
      _ => None,
    };
    return Some((refusal.code(), details));
  }
  if error.downcast_ref::<DropRefused>().is_some() {
    return Some((ErrorCode::DropConflict, None));
  }
  if let Some(blocked) = error.downcast_ref::<MoveBlocked>() {
    return paths(&blocked.paths);
  }
  if let Some(blocked) = error.downcast_ref::<UndoBlocked>() {
    return paths(&blocked.paths);
  }
  error.downcast_ref::<FilesBlocked>().and_then(|blocked| paths(&blocked.paths))
}

/// A start refused by the landing gate: the code of its first refusal, the one its message names
fn classify_landing(error: &anyhow::Error) -> Option<Classified> {
  let refused = error.downcast_ref::<LandingRefused>()?;
  let code = match refused.refusals.first()? {
    LandingRefusal::DanglingReference { .. } => ErrorCode::DanglingReference,
    LandingRefusal::MissingDeclaration { .. } => ErrorCode::MissingDeclaration,
  };
  Some((code, None))
}

/// The `ApiError` JSON of each of `contract_cases`, by its name: what the app and the e2e server answer for it
#[cfg(any(test, feature = "fixtures"))]
pub const CONTRACT_JSON: &str = include_str!("../fixtures/api_errors.json");

/// When the holder of the contract's locks took them: ahead of any clock that runs the tests (2100-01-01), so their
/// `held_for` is 0 wherever and whenever the adapters convert them
#[cfg(any(test, feature = "fixtures"))]
const CONTRACT_HOLDER_SINCE: u64 = 4_102_444_800;

/// The errors of the adapters' contract test, by name: a code of each kind of data, an error without a code, a typed
/// error under context and an `ApiError` passed through. `CONTRACT_JSON` holds what `api_error` makes of each
#[cfg(any(test, feature = "fixtures"))]
pub fn contract_cases() -> Vec<(&'static str, anyhow::Error)> {
  use anyhow::anyhow;
  use domain_types::landing::{LandingFix, StackCommit};
  use std::path::PathBuf;
  use std::time::Duration;

  let holder = |command: &str| LockHolder {
    command: command.to_string(),
    pid: 4242,
    process_started: None,
    since: CONTRACT_HOLDER_SINCE,
  };
  let locked = |holder: Option<LockHolder>| RepositoryLocked {
    path: PathBuf::from("/repo/.git/branch-deck.lock"),
    waited: Duration::from_secs(10),
    holder,
  };
  let commit = |id: &str, subject: &str, branch: &str| StackCommit {
    id: id.to_string(),
    subject: subject.to_string(),
    branch: branch.to_string(),
  };
  vec![
    ("internal", anyhow!("git cat-file exited with 128").context("Cannot read the commits ahead of the baseline")),
    ("operation_in_progress", SyncDeferred::OperationInProgress("a rebase").into()),
    ("detached_head", SyncDeferred::DetachedHead.into()),
    ("no_baseline", NoBaseline::NoCommits.into()),
    (
      "head_moved",
      RegroupError::HeadMoved(HeadMoved {
        branch: "master".to_string(),
        expected: "1111111111111111111111111111111111111111".to_string(),
        actual: "2222222222222222222222222222222222222222".to_string(),
      })
      .into(),
    ),
    (
      "invalid_plan",
      RegroupError::InvalidPlan {
        message: "The plan names auth twice".to_string(),
        commits: Vec::new(),
      }
      .into(),
    ),
    ("locked_by_bd", locked(Some(holder("bd push auth"))).into()),
    (
      "locked_by_the_app",
      anyhow::Error::new(RegroupError::Locked(locked(Some(holder("branch-deck"))))).context("Cannot move the commits"),
    ),
    ("locked_without_holder", locked(None).into()),
    (
      "ref_locked",
      RefLocked {
        path: "/repo/.git/refs/heads/master.lock".to_string(),
      }
      .into(),
    ),
    ("index_locked", update_core::error_cases::index_locked().into()),
    (
      "canceled",
      LockWaitCanceled {
        path: PathBuf::from("/repo/.git/branch-deck.lock"),
      }
      .into(),
    ),
    (
      "uncommitted_overlap",
      MoveBlocked {
        operation: "unapply",
        paths: vec!["src/auth.rs".to_string(), "src/login.rs".to_string()],
        path_count: 2,
      }
      .into(),
    ),
    (
      "reapply_conflict",
      ReapplyRefusal::Conflict {
        branch: "auth".to_string(),
        commit: "3333333333333333333333333333333333333333".to_string(),
        subject: "Check the token".to_string(),
        files: vec!["src/auth.rs".to_string()],
      }
      .into(),
    ),
    (
      "network_error",
      FetchFailed {
        upstream: "origin/master".to_string(),
        message: "Could not resolve host: github.com".to_string(),
      }
      .into(),
    ),
    (
      "missing_declaration",
      LandingRefused {
        subject: "auth".to_string(),
        refusals: vec![LandingRefusal::MissingDeclaration {
          branch: "auth".to_string(),
          identifier: "TokenCheck".to_string(),
          file: "src/auth.rs".to_string(),
          line: 12,
          used_by: commit("4444444444444444444444444444444444444444", "Check the token", "auth"),
          declared_by: commit("5555555555555555555555555555555555555555", "Add TokenCheck", "tokens"),
          fix: LandingFix::Restack {
            branch: "auth".to_string(),
            onto: "tokens".to_string(),
          },
          landing_in: None,
        }],
      }
      .into(),
    ),
    ("youtrack_sign_in_canceled", SignInError::Canceled.into()),
    ("github_sign_in_canceled", DeviceFlowError::Canceled.into()),
    ("cli_tool_action_canceled", ActionError::Canceled.into()),
    (
      "review_fetch_canceled",
      anyhow::Error::new(StoppedCommand::Cancelled).context("Cannot fetch pull request #42"),
    ),
    ("sign_in_failed", SignInError::Denied("access_denied".to_string()).into()),
    ("passed_through", ApiError::new(ErrorCode::Canceled, "The sign-in was canceled").into()),
  ]
}

#[cfg(test)]
#[path = "api_error_test.rs"]
mod api_error_test;
