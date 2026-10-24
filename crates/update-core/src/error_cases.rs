//! The typed errors of a move of HEAD, an update, an unapply, a reapply, a park and their undo, each with the one code
//! `bd` and the app answer for it. Both sides test their conversion on this table: the mappers of the bd commands
//! (`bd-update/src/error_code_test.rs`, `bd-rewrite/src/park_error_test.rs`) and `app_api::api_error`
//! (`app-api/src/api_error_test.rs`), which no crate can call both of (only the app and the e2e server depend on
//! `app-api`). A case whose code differs on one side fails that side's test
use crate::update_project::{FetchFailed, NoUpstream};
use domain_types::error::ErrorCode;
use git_ops::index_lock::{IndexLocked, LockHolder};
use git_ops::rewrite_guard::HeadMoved;
use git_ops::unapply::{DropRefused, MoveBlocked};
use git_ops::update_branch::{BranchSwitched, FilesBlocked, GuardRefusal, HeadAttached, NotAnUpdate, PendingUpdate, RefLocked, UndoBlocked};
use history_rewrite::baseline::NoBaseline;
use history_rewrite::parked::ParkError;
use history_rewrite::repository_lock::RepositoryLocked;
use history_rewrite::store_lines::StoreMoved;
use std::path::PathBuf;
use std::time::Duration;
use sync_core::reapply_branch::ReapplyRefusal;
use sync_core::unapply_branch::UnapplyRefusal;

/// The command whose mapper of `bd` answers a case: any move of HEAD (`core_error`, also under an unapply or a reapply,
/// and its undo), an unapply (`unapply_error`), a reapply (`reapply_error`) or a park (`park_error`)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaseOf {
  Move,
  Unapply,
  Reapply,
  Park,
}

pub struct ErrorCase {
  pub name: &'static str,
  pub of: CaseOf,
  pub error: fn() -> anyhow::Error,
  pub code: ErrorCode,
  /// The code when an undo fails with it (`ErrorCode::in_undo`); only for a move
  pub undo: Option<ErrorCode>,
}

const fn case(name: &'static str, of: CaseOf, error: fn() -> anyhow::Error, code: ErrorCode, undo: Option<ErrorCode>) -> ErrorCase {
  ErrorCase { name, of, error, code, undo }
}

fn head_moved() -> HeadMoved {
  HeadMoved {
    branch: "master".to_string(),
    expected: "1111111111111111111111111111111111111111".to_string(),
    actual: "2222222222222222222222222222222222222222".to_string(),
  }
}

fn repository_locked() -> RepositoryLocked {
  RepositoryLocked {
    path: PathBuf::from("/repo/.git/branch-deck.lock"),
    waited: Duration::from_secs(10),
    holder: None,
  }
}

/// The index lock of a worktree that an IDE's `git status` held for the whole wait
pub fn index_locked() -> IndexLocked {
  IndexLocked {
    worktree: "/repo".to_string(),
    path: "/repo/.git/index.lock".to_string(),
    held_for: Duration::from_secs(31),
    holders: vec![LockHolder {
      pid: 4242,
      name: "git".to_string(),
      command: "git status --porcelain -z".to_string(),
    }],
  }
}

fn checked_out() -> (String, String) {
  ("auth".to_string(), "/work".to_string())
}

fn paths() -> Vec<String> {
  vec!["src/auth.rs".to_string()]
}

/// A move of HEAD the files follow: an update, a pick, the finish of a resolve session, an unapply, a reapply
fn move_cases() -> Vec<ErrorCase> {
  use CaseOf::Move;
  use ErrorCode::{DetachedHead, FinishPending, HeadMoved, InvalidArguments, Locked, NetworkError, NoBaseline, OperationInProgress, UncommittedOverlap, UndoStale};
  vec![
    case("head_moved", Move, || head_moved().into(), HeadMoved, Some(UndoStale)),
    case(
      "head_moved_under_context",
      Move,
      || anyhow::Error::new(head_moved()).context("Cannot undo the update"),
      HeadMoved,
      Some(UndoStale),
    ),
    case(
      "branch_switched",
      Move,
      || {
        BranchSwitched {
          expected: "master".to_string(),
          actual: Some("feature".to_string()),
        }
        .into()
      },
      HeadMoved,
      Some(UndoStale),
    ),
    case("head_attached", Move, || HeadAttached { actual: "master".to_string() }.into(), HeadMoved, Some(UndoStale)),
    case(
      "operation_in_progress",
      Move,
      || GuardRefusal::OperationInProgress("A rebase is in progress".to_string()).into(),
      OperationInProgress,
      Some(OperationInProgress),
    ),
    case(
      "detached_head",
      Move,
      || GuardRefusal::DetachedHead("HEAD is detached".to_string()).into(),
      DetachedHead,
      Some(DetachedHead),
    ),
    case(
      "ref_locked",
      Move,
      || {
        RefLocked {
          path: "/repo/.git/refs/heads/master.lock".to_string(),
        }
        .into()
      },
      Locked,
      Some(Locked),
    ),
    case("index_locked", Move, || index_locked().into(), Locked, Some(Locked)),
    case("repository_locked", Move, || repository_locked().into(), Locked, Some(Locked)),
    case(
      "finish_pending",
      Move,
      || PendingUpdate { message: "finish it".to_string() }.into(),
      FinishPending,
      Some(FinishPending),
    ),
    case(
      "not_an_update",
      Move,
      || {
        NotAnUpdate {
          message: "not this update".to_string(),
        }
        .into()
      },
      InvalidArguments,
      Some(InvalidArguments),
    ),
    case("undo_blocked", Move, || UndoBlocked { paths: paths() }.into(), UncommittedOverlap, Some(UncommittedOverlap)),
    case(
      "files_blocked",
      Move,
      || FilesBlocked { paths: paths() }.into(),
      UncommittedOverlap,
      Some(UncommittedOverlap),
    ),
    case(
      "move_blocked",
      Move,
      || {
        MoveBlocked {
          operation: "unapply",
          paths: paths(),
          path_count: 1,
        }
        .into()
      },
      UncommittedOverlap,
      Some(UncommittedOverlap),
    ),
    case(
      "fetch_failed",
      Move,
      || {
        FetchFailed {
          upstream: "origin/master".to_string(),
          message: "Could not resolve host".to_string(),
        }
        .into()
      },
      NetworkError,
      Some(NetworkError),
    ),
    case("no_upstream", Move, || NoUpstream { baseline: "master".to_string() }.into(), NoBaseline, Some(NoBaseline)),
  ]
}

fn refusal_cases() -> Vec<ErrorCase> {
  use CaseOf::{Reapply, Unapply};
  use ErrorCode::{BranchCheckedOut, BranchNotBuilt, DropConflict, HeadMoved, NothingToReapply, ReapplyConflict, UnknownBranch};
  vec![
    case(
      "unapply_unknown_branch",
      Unapply,
      || UnapplyRefusal::UnknownBranch("auth".to_string()).into(),
      UnknownBranch,
      None,
    ),
    case(
      "unapply_branch_changed",
      Unapply,
      || UnapplyRefusal::BranchChanged("auth".to_string()).into(),
      HeadMoved,
      None,
    ),
    case(
      "unapply_not_built",
      Unapply,
      || {
        UnapplyRefusal::NotBuilt {
          branch: "auth".to_string(),
          reason: "a conflict".to_string(),
        }
        .into()
      },
      BranchNotBuilt,
      None,
    ),
    case(
      "unapply_checked_out",
      Unapply,
      || {
        let (branch, worktree) = checked_out();
        UnapplyRefusal::CheckedOut { branch, worktree }.into()
      },
      BranchCheckedOut,
      None,
    ),
    case("unapply_drop_conflict", Unapply, || DropRefused("src/auth.rs".to_string()).into(), DropConflict, None),
    case(
      "reapply_unknown_branch",
      Reapply,
      || ReapplyRefusal::UnknownBranch("auth".to_string()).into(),
      UnknownBranch,
      None,
    ),
    case(
      "reapply_checked_out",
      Reapply,
      || {
        let (branch, worktree) = checked_out();
        ReapplyRefusal::CheckedOut { branch, worktree }.into()
      },
      BranchCheckedOut,
      None,
    ),
    case(
      "reapply_no_common_history",
      Reapply,
      || ReapplyRefusal::NoCommonHistory("auth".to_string()).into(),
      NothingToReapply,
      None,
    ),
    case(
      "reapply_already_in_head",
      Reapply,
      || ReapplyRefusal::AlreadyInHead("auth".to_string()).into(),
      NothingToReapply,
      None,
    ),
    case(
      "reapply_conflict",
      Reapply,
      || {
        ReapplyRefusal::Conflict {
          branch: "auth".to_string(),
          commit: "3333333333333333333333333333333333333333".to_string(),
          subject: "Check the token".to_string(),
          files: paths(),
        }
        .into()
      },
      ReapplyConflict,
      None,
    ),
  ]
}

fn park_cases() -> Vec<ErrorCase> {
  use CaseOf::Park;
  vec![
    case(
      "park_unknown_branch",
      Park,
      || ParkError::UnknownBranch("auth".to_string()).into(),
      ErrorCode::UnknownBranch,
      None,
    ),
    case("park_not_parked", Park, || ParkError::NotParked("auth".to_string()).into(), ErrorCode::NotParked, None),
    case(
      "park_invalid_note",
      Park,
      || ParkError::InvalidNote("a note of one line".to_string()).into(),
      ErrorCode::InvalidArguments,
      None,
    ),
    case(
      "park_moved",
      Park,
      || {
        ParkError::Moved(StoreMoved {
          name: "parked branches",
          store_ref: "refs/branch-deck/parked",
          expected: None,
          actual: Some("1111111111111111111111111111111111111111".to_string()),
        })
        .into()
      },
      ErrorCode::ParkedMoved,
      None,
    ),
    case("park_locked", Park, || ParkError::Locked(repository_locked()).into(), ErrorCode::Locked, None),
    case(
      "park_no_baseline",
      Park,
      || ParkError::Other(NoBaseline::NoCommits.into()).into(),
      ErrorCode::NoBaseline,
      None,
    ),
  ]
}

/// Every case, by its name
pub fn error_cases() -> Vec<ErrorCase> {
  [move_cases(), refusal_cases(), park_cases()].into_iter().flatten().collect()
}
