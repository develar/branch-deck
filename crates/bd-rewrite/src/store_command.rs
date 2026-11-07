//! The write of a store of Branch Deck by `brd` (`brd park`, `brd unpark`, `brd need`): its rewrite intent recorded in
//! the operations registry of the repository before the lock, as every rewrite of `bd` (`bd_core::operation_record`),
//! its core run with the bounded wait for the repository lock, which the core takes after it read the store

use bd_core::context::RepoContext;
use bd_core::error::CliError;
use bd_core::operation_record::{asked_on, recorded};
use domain_types::error::ErrorCode;
use domain_types::operation::Intent;
use domain_types::rewrite::RewriteIntent;
use domain_types::rewrite::TableWitness;
use history_rewrite::needs::NeedError;
use history_rewrite::parked::ParkError;
use history_rewrite::repository_lock::{LockWait, RewriteOptions};
use std::fmt::Display;

/// `write`, the rewrite `intent` of the table `bd` asks it on (no HEAD, no store: the core reads them under the lock),
/// recorded, its error in the words and the code of `bd` (`store_error`)
pub(crate) fn store_write<O, E: CodedError>(
  context: &RepoContext,
  intent: impl FnOnce(TableWitness) -> RewriteIntent,
  write: impl FnOnce(&RewriteOptions) -> Result<O, E>,
) -> Result<O, CliError> {
  recorded(context, &Intent::Rewrite(Box::new(intent(asked_on(None)))), || {
    let options = RewriteOptions {
      lock_wait: LockWait::Bounded(context.lock_timeout),
      ..RewriteOptions::default()
    };
    write(&options).map_err(store_error)
  })
}

/// The error of the core of a write of a store, with the code the app answers too (`ParkError::code`, `NeedError::code`)
pub(crate) trait CodedError: Display {
  fn error_code(&self) -> Option<ErrorCode>;
}

impl CodedError for ParkError {
  fn error_code(&self) -> Option<ErrorCode> {
    self.code()
  }
}

impl CodedError for NeedError {
  fn error_code(&self) -> Option<ErrorCode> {
    self.code()
  }
}

/// The error of `bd` of a refused or failed write of a store: its code, `git_error` for a failure without one; by
/// value, for `map_err`
#[expect(clippy::needless_pass_by_value, reason = "used as `map_err(store_error)`")]
pub(crate) fn store_error<E: CodedError>(error: E) -> CliError {
  CliError::new(error.error_code().unwrap_or(ErrorCode::GitError), error.to_string())
}
