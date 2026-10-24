//! The codes of the mappers of bd for the typed errors of a move, an update, an unapply, a reapply and their undo
//! (`core_error`, `unapply_error`, `reapply_error`, `undo_core_error`): the codes of `update_core::error_cases`, which
//! the app's `api_error` answers too (`app-api/src/api_error_test.rs`)
use crate::unapply_command::{reapply_error, unapply_error};
use crate::update_command::{core_error, undo_core_error};
use anyhow::anyhow;
use domain_types::error::ErrorCode;
use pretty_assertions::assert_eq;
use update_core::error_cases::{CaseOf, ErrorCase, error_cases};

/// The code of each case by its name
type Codes = Vec<(&'static str, ErrorCode)>;

/// `(expected, answered)` codes of each case of `of` through `bd`
fn answered(of: &[CaseOf], code: impl Fn(anyhow::Error) -> ErrorCode, expected: impl Fn(&ErrorCase) -> Option<ErrorCode>) -> (Codes, Codes) {
  error_cases()
    .into_iter()
    .filter(|case| of.contains(&case.of))
    .filter_map(|case| expected(&case).map(|expected| ((case.name, expected), (case.name, code((case.error)())))))
    .unzip()
}

#[test]
fn bd_answers_the_code_of_each_refusal_of_a_move_an_unapply_and_a_reapply_that_the_app_answers() {
  let (expected, actual) = answered(&[CaseOf::Move], |error| core_error(error).code, |case| Some(case.code));
  assert_eq!(actual, expected);
  let (expected, actual) = answered(&[CaseOf::Move, CaseOf::Unapply], |error| unapply_error(error).code, |case| Some(case.code));
  assert_eq!(actual, expected);
  let (expected, actual) = answered(&[CaseOf::Move, CaseOf::Reapply], |error| reapply_error(error).code, |case| Some(case.code));
  assert_eq!(actual, expected);
}

#[test]
fn bd_undo_answers_the_code_of_each_refusal_that_the_app_answers_and_a_moved_head_makes_it_stale() {
  let (expected, actual) = answered(&[CaseOf::Move], |error| undo_core_error(error).code, |case| case.undo);
  assert_eq!(actual, expected);
  assert!(expected.contains(&("head_moved", ErrorCode::UndoStale)) && !ErrorCode::UndoStale.retryable());
}

#[test]
fn an_error_without_a_code_is_git_error_in_bd() {
  let error = || anyhow!("git cat-file exited with 128").context("Cannot read the commits");
  assert_eq!((core_error(error()).code, undo_core_error(error()).code), (ErrorCode::GitError, ErrorCode::GitError));
}
