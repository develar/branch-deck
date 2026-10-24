//! The codes of `park_error` for each refusal of a park: the codes of `update_core::error_cases`, which the app's
//! `api_error` answers too (`app-api/src/api_error_test.rs`); both read `ParkError::code`
use crate::park_command::park_error;
use anyhow::anyhow;
use domain_types::error::ErrorCode;
use history_rewrite::parked::ParkError;
use pretty_assertions::assert_eq;
use update_core::error_cases::{CaseOf, error_cases};

#[test]
fn bd_answers_the_code_of_each_refusal_of_a_park_that_the_app_answers() {
  let cases: Vec<_> = error_cases().into_iter().filter(|case| case.of == CaseOf::Park).collect();
  let expected: Vec<_> = cases.iter().map(|case| (case.name, case.code)).collect();
  let actual: Vec<_> = cases
    .iter()
    .map(|case| {
      let error = (case.error)().downcast::<ParkError>().expect("a park case is a ParkError");
      (case.name, park_error(error).code)
    })
    .collect();
  assert_eq!(actual, expected);
}

#[test]
fn a_park_that_fails_without_a_code_is_git_error_in_bd() {
  assert_eq!(park_error(ParkError::Other(anyhow!("git update-ref exited with 128"))).code, ErrorCode::GitError);
}
