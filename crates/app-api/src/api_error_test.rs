use crate::api_error::{CONTRACT_JSON, api_error, api_error_at, contract_cases, undo_error};
use anyhow::{Context, anyhow};
use domain_types::error::{ErrorCode, ErrorDetails, HolderTool, LockHolderInfo, Retry};
use file_diffs::accio::SearchCanceled;
use git_ops::rewrite_guard::HeadMoved;
use history_rewrite::regroup::RegroupError;
use history_rewrite::repository_lock::{LockHolder, RepositoryLocked};
use history_rewrite::store_lines::StoreMoved;
use pretty_assertions::assert_eq;
use serde_json::{Map, Value};
use std::path::PathBuf;
use std::time::Duration;
use sync_core::sync::SyncDeferred;
use update_core::error_cases::error_cases;

const NOW: u64 = 1_800_000_000;

fn locked_by(command: &str, since: u64) -> anyhow::Error {
  RepositoryLocked {
    path: PathBuf::from("/repo/.git/branch-deck.lock"),
    waited: Duration::from_secs(10),
    holder: Some(LockHolder {
      command: command.to_string(),
      pid: 4242,
      process_started: None,
      since,
    }),
  }
  .into()
}

fn holder_of(error: &anyhow::Error) -> LockHolderInfo {
  match api_error_at(error, NOW).details {
    Some(ErrorDetails::Locked { holder: Some(holder) }) => holder,
    details => panic!("no holder in {details:?}"),
  }
}

#[test]
fn api_error_of_each_contract_case_is_its_json_in_the_fixture() {
  let expected: Map<String, Value> = serde_json::from_str(CONTRACT_JSON).expect("fixtures/api_errors.json is no JSON object");
  let cases = contract_cases();
  let actual: Map<String, Value> = cases
    .into_iter()
    .map(|(name, error)| (name.to_string(), serde_json::to_value(api_error(error)).unwrap()))
    .collect();
  assert_eq!(Value::Object(actual), Value::Object(expected));
}

#[test]
fn api_error_names_the_holder_of_a_lock_with_how_long_it_has_held_it() {
  assert_eq!(
    holder_of(&locked_by("bd push auth", NOW - 40)),
    LockHolderInfo {
      tool: HolderTool::Bd,
      command: "bd push auth".to_string(),
      pid: 4242,
      held_for: 40,
    }
  );
  let error = api_error_at(&locked_by("bd push auth", NOW - 40), NOW);
  assert_eq!((error.code, error.retry), (ErrorCode::Locked, Retry::Same));
  assert_eq!(error.message, "`bd push auth` (pid 4242) is changing this repository; try again when it is done");
}

#[test]
fn api_error_tells_a_bd_holder_by_its_program_from_the_app() {
  let tool = |command: &str| holder_of(&locked_by(command, NOW)).tool;
  assert_eq!(tool("bd check --fix"), HolderTool::Bd);
  assert_eq!(tool("bd.exe sync"), HolderTool::Bd);
  assert_eq!(tool("branch-deck cli push auth"), HolderTool::Bd);
  assert_eq!(tool("branch-deck"), HolderTool::App);
  assert_eq!(tool("bdx push"), HolderTool::App);
}

#[test]
fn api_error_finds_a_typed_error_under_context() {
  let error = anyhow::Error::new(SyncDeferred::DetachedHead).context("Cannot sync").context("Cannot refresh");
  let error = api_error_at(&error, NOW);
  assert_eq!(error.code, ErrorCode::DetachedHead);
  assert_eq!(error.message, "Cannot refresh: Cannot sync: Not synced: HEAD is detached");
}

#[test]
fn api_error_answers_a_canceled_symbol_search_canceled() {
  let error = anyhow::Error::new(SearchCanceled).context("Cannot find usages");
  assert_eq!(api_error_at(&error, NOW).code, ErrorCode::Canceled);
}

#[test]
fn api_error_picks_no_code_by_the_message() {
  for message in ["canceled", "locked", "Not synced: HEAD is detached"] {
    let error = api_error_at(&anyhow!("{message}"), NOW);
    assert_eq!((error.code, error.message.as_str(), error.details), (ErrorCode::Internal, message, None));
  }
  let error: anyhow::Result<()> = Err(anyhow!("exit 128")).context("canceled");
  assert_eq!(api_error_at(&error.unwrap_err(), NOW).code, ErrorCode::Internal);
}

/// The app answers the code of each typed error that `bd` answers: both are tested on `update_core::error_cases`
/// (`bd-update/src/error_code_test.rs`, `bd-rewrite/src/park_error_test.rs`), an undo through `undo_error`
#[test]
fn api_error_answers_the_code_of_each_typed_error_that_bd_answers_and_an_undo_its_code_in_undo() {
  let cases = error_cases();
  let expected: Vec<_> = cases.iter().map(|case| (case.name, case.code)).collect();
  let actual: Vec<_> = cases.iter().map(|case| (case.name, api_error((case.error)()).code)).collect();
  assert_eq!(actual, expected);
  let expected: Vec<_> = cases.iter().filter_map(|case| case.undo.map(|undo| (case.name, undo, undo.retry()))).collect();
  let actual: Vec<_> = cases
    .iter()
    .filter(|case| case.undo.is_some())
    .map(|case| {
      let error = undo_error((case.error)());
      (case.name, error.code, error.retry)
    })
    .collect();
  assert_eq!(actual, expected);
  assert!(actual.contains(&("head_moved", ErrorCode::UndoStale, Retry::No)));
}

/// A stale undo says so in its words too: what moved and that it can't be undone, never the advice to read the state
/// again and retry that the same error gives a rewrite; any other undo error keeps its words
#[test]
fn a_stale_undo_says_it_cannot_be_undone_instead_of_advising_a_retry() {
  let head_moved = || HeadMoved {
    branch: "master".to_string(),
    expected: "1".repeat(40),
    actual: "2".repeat(40),
  };
  let groups_moved = StoreMoved {
    name: "issue groups",
    store_ref: "refs/branch-deck/issue-groups",
    expected: Some("3".repeat(40)),
    actual: None,
  };
  let cases = [
    anyhow::Error::new(head_moved()).context("Cannot undo the squash"),
    anyhow::Error::new(RegroupError::HeadMoved(head_moved())),
    anyhow::Error::new(RegroupError::GroupsMoved(groups_moved)),
  ];
  let words: Vec<_> = cases
    .into_iter()
    .map(|error| {
      let error = undo_error(error);
      (error.message, error.code)
    })
    .collect();
  assert_eq!(
    words,
    [
      (
        "Branch master moved: HEAD is 22222222, not 11111111; it can't be undone any more".to_string(),
        ErrorCode::UndoStale
      ),
      (
        "Branch master moved: HEAD is 22222222, not 11111111; it can't be undone any more".to_string(),
        ErrorCode::UndoStale
      ),
      (
        "The issue groups (refs/branch-deck/issue-groups) moved: expected 33333333, found nothing; it can't be undone any more".to_string(),
        ErrorCode::UndoStale
      ),
    ]
  );
  // a rewrite keeps the advice; an undo error of another code keeps its words
  assert!(api_error(anyhow::Error::new(head_moved())).message.ends_with("; read it again and retry"));
  let locked = undo_error(anyhow!("exit 128").context("Cannot move branch master back"));
  assert_eq!((locked.code, locked.message.as_str()), (ErrorCode::Internal, "Cannot move branch master back: exit 128"));
}
