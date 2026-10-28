//! What a change of an account refreshes (`account_changed`), the one rule the app and the e2e server both hand their
//! sessions and windows

use super::account_changed;
use crate::events::AUTH_CHANGED_EVENT;
use crate::transport::{AppEvents, WindowId};
use domain_types::session::{FactFamily, FactRefreshReason};
use domain_types::sign_in::Account;
use pretty_assertions::assert_eq;
use serde_json::Value;
use std::cell::RefCell;
use std::sync::Mutex;

/// The events a host was told, by name: none goes to one window alone
#[derive(Default)]
struct Told(Mutex<Vec<String>>);

impl AppEvents for Told {
  fn emit(&self, event: &str, _payload: Value) {
    self.0.lock().unwrap().push(event.to_owned());
  }

  fn emit_to(&self, window: &WindowId, event: &str, _payload: Value) {
    panic!("{event} went to {window} alone");
  }
}

/// The families a change of `account` refreshed, in order, and the events every window heard
fn changed(account: Account) -> (Vec<(FactFamily, FactRefreshReason)>, Vec<String>) {
  let refreshed = RefCell::new(Vec::new());
  let told = Told::default();
  account_changed(account, |family, reason| refreshed.borrow_mut().push((family, reason)), &told);
  (refreshed.into_inner(), told.0.into_inner().unwrap())
}

/// The sign-ins are read again after every change, following up on a read in flight; a GitHub token of another user
/// answers other pull requests and review requests, so those are listed again too, and YouTrack changes neither
#[test]
fn a_change_of_an_account_reads_the_sign_ins_again_and_after_github_its_lists_too() {
  assert_eq!(
    changed(Account::GitHub),
    (
      vec![
        (FactFamily::Auth, FactRefreshReason::AfterWrite),
        (FactFamily::PullRequests, FactRefreshReason::Manual),
        (FactFamily::ReviewRequests, FactRefreshReason::Manual),
      ],
      vec![AUTH_CHANGED_EVENT.to_owned()],
    )
  );
  assert_eq!(
    changed(Account::YouTrack),
    (vec![(FactFamily::Auth, FactRefreshReason::AfterWrite)], vec![AUTH_CHANGED_EVENT.to_owned()])
  );
}
