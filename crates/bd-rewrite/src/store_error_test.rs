//! The codes of `store_error` for each refusal of a park and of a need: the codes of `update_core::error_cases`, which
//! the app's `api_error` answers too (`app-api/src/api_error_test.rs`); both read `ParkError::code` and `NeedError::code`
use crate::store_command::store_error;
use anyhow::anyhow;
use domain_types::error::ErrorCode;
use history_rewrite::needs::NeedError;
use history_rewrite::parked::ParkError;
use history_rewrite::store_write::StoreWriteError;
use pretty_assertions::assert_eq;
use update_core::error_cases::{CaseOf, error_cases};

#[test]
fn bd_answers_the_code_of_each_refusal_of_a_park_and_a_need_that_the_app_answers() {
  let cases: Vec<_> = error_cases().into_iter().filter(|case| matches!(case.of, CaseOf::Park | CaseOf::Need)).collect();
  let expected: Vec<_> = cases.iter().map(|case| (case.name, case.code)).collect();
  let actual: Vec<_> = cases
    .iter()
    .map(|case| {
      let error = (case.error)();
      let code = match case.of {
        CaseOf::Park => store_error(error.downcast::<ParkError>().expect("a park case is a ParkError")).code,
        _ => store_error(error.downcast::<NeedError>().expect("a need case is a NeedError")).code,
      };
      (case.name, code)
    })
    .collect();
  assert_eq!(actual, expected);
}

#[test]
fn a_write_of_a_store_that_fails_without_a_code_is_git_error_in_bd() {
  let failed = || StoreWriteError::Other(anyhow!("git update-ref exited with 128"));
  assert_eq!(store_error(ParkError::Write(failed())).code, ErrorCode::GitError);
  assert_eq!(store_error(NeedError::Write(failed())).code, ErrorCode::GitError);
}
