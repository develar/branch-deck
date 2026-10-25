use crate::fixture_test::{environment, run_json};
use bd_core::error::CONFLICT_EXIT;
use domain_types::error::{ErrorCode, Retry};
use pretty_assertions::assert_eq;

/// Every code with its exit code and whether the same command again may succeed: 2 invalid input, 3 stale (retry), 4
/// refused, 5 git or network error. Exit 1 is never an error
const CODES: &[(ErrorCode, i32, bool)] = &[
  (ErrorCode::InvalidArguments, 2, false),
  (ErrorCode::NotARepository, 2, false),
  (ErrorCode::NoBranchPrefix, 2, false),
  (ErrorCode::UnknownBranch, 2, false),
  (ErrorCode::NoBaseline, 4, false),
  (ErrorCode::InvalidPlan, 2, false),
  (ErrorCode::NotAheadOfBaseline, 2, false),
  (ErrorCode::StackConflict, 4, false),
  (ErrorCode::StackNotBuilt, 4, false),
  (ErrorCode::TargetConflict, 4, false),
  (ErrorCode::HunkConflict, 4, false),
  (ErrorCode::BlockNotFound, 2, false),
  (ErrorCode::HeadMoved, 3, true),
  (ErrorCode::GroupsMoved, 3, true),
  (ErrorCode::StacksMoved, 3, true),
  (ErrorCode::ParkedMoved, 3, true),
  (ErrorCode::PrefixChanged, 3, true),
  (ErrorCode::NotParked, 2, false),
  (ErrorCode::Parked, 4, false),
  (ErrorCode::BranchNotBuilt, 4, false),
  (ErrorCode::BranchCheckedOut, 4, false),
  (ErrorCode::DropConflict, 4, false),
  (ErrorCode::Unapplied, 4, false),
  (ErrorCode::ReapplyConflict, 4, false),
  (ErrorCode::NothingToReapply, 4, false),
  (ErrorCode::LeaseRejected, 4, false),
  (ErrorCode::Locked, 3, true),
  (ErrorCode::OperationInProgress, 4, false),
  (ErrorCode::DetachedHead, 4, false),
  (ErrorCode::UndoStale, 4, false),
  (ErrorCode::RangeUnknown, 4, false),
  (ErrorCode::SourceOnNewerBaseline, 4, false),
  (ErrorCode::UncommittedOverlap, 4, false),
  (ErrorCode::NoSession, 2, false),
  (ErrorCode::RoundMoved, 3, true),
  (ErrorCode::SessionStale, 4, false),
  (ErrorCode::UnsupportedConflict, 4, false),
  (ErrorCode::UnresolvedMarkers, 2, false),
  (ErrorCode::ResolutionOutsideConflict, 2, false),
  (ErrorCode::MissingResolution, 2, false),
  (ErrorCode::EmptyResolution, 2, false),
  (ErrorCode::FinishPending, 4, false),
  (ErrorCode::PushRefused, 4, false),
  (ErrorCode::PushTooLarge, 4, false),
  (ErrorCode::NoRemote, 4, false),
  (ErrorCode::InstallRefused, 4, false),
  (ErrorCode::ApprovalsMissing, 4, false),
  (ErrorCode::PushDismissesApprovals, 4, false),
  (ErrorCode::RulesUnknown, 4, false),
  (ErrorCode::RobotRunning, 4, false),
  (ErrorCode::Landed, 4, false),
  (ErrorCode::DanglingReference, 4, false),
  (ErrorCode::MissingDeclaration, 4, false),
  (ErrorCode::NotOnSpace, 4, false),
  (ErrorCode::NotLoggedIn, 4, false),
  (ErrorCode::PatronusRefused, 4, false),
  (ErrorCode::NoPullRequest, 4, false),
  (ErrorCode::NoReviewer, 4, false),
  (ErrorCode::ReviewRefused, 4, false),
  (ErrorCode::NoLease, 2, false),
  (ErrorCode::LeaseMismatch, 2, false),
  (ErrorCode::NotASlot, 2, false),
  (ErrorCode::SlotInPool, 2, false),
  (ErrorCode::SlotBusy, 4, false),
  (ErrorCode::HostSlot, 4, false),
  (ErrorCode::UnlandedCommits, 4, false),
  (ErrorCode::DirtyWorktree, 4, false),
  (ErrorCode::BranchExists, 4, false),
  (ErrorCode::OtherVolume, 4, false),
  (ErrorCode::LowDiskSpace, 4, false),
  (ErrorCode::GitNotFound, 5, false),
  (ErrorCode::GitError, 5, false),
  (ErrorCode::NetworkError, 5, true),
  (ErrorCode::PatronusError, 5, false),
  (ErrorCode::Internal, 5, false),
  (ErrorCode::Canceled, 4, false),
];

#[test_log::test]
fn every_error_code_has_the_exit_code_and_the_retry_of_its_class() {
  for &(code, exit_code, retryable) in CODES {
    assert_eq!((code.exit_code(), code.retryable()), (exit_code, retryable), "{code:?}");
    // `retryable` is the retry class of the app's `ApiError`: a stale read retries after a new read, the rest as it is
    let retry = if !retryable {
      Retry::No
    } else if exit_code == 3 && code != ErrorCode::Locked {
      Retry::AfterReread
    } else {
      Retry::Same
    };
    assert_eq!(code.retry(), retry, "{code:?}");
    // a retry is for a stale read (exit 3) or the network, never for a refusal
    assert!(!retryable || exit_code == 3 || code == ErrorCode::NetworkError, "{code:?}");
    assert_ne!(exit_code, CONFLICT_EXIT, "{code:?}: a conflict is an output, never an error");
  }
  // every code is in the table: as many as `bd schema` names
  let directory = tempfile::tempdir().unwrap();
  let (_, schema) = run_json(&environment(directory.path()), &["schema", "--all"]);
  let codes = &schema["outputs"]["error"]["$defs"]["ErrorCode"];
  let count = codes["oneOf"].as_array().or_else(|| codes["enum"].as_array()).map(Vec::len).unwrap();
  assert_eq!(count, CODES.len());
}
