use crate::session::landed::hide_under;
use app_settings::service::SettingsService;
use domain_types::error::ErrorCode;
use domain_types::worktree_epoch::WorktreeEpoch;
use pretty_assertions::assert_eq;
use repository_model::session::AnotherLease;
use serde_json::Value;
use std::time::Duration;

const PATH: &str = "/work/slot";

/// The settings of a worktree whose slot holds `lease-a`, written to by nothing else within the test
fn settings_of_lease_a() -> (tempfile::TempDir, SettingsService) {
  let directory = tempfile::tempdir().unwrap();
  let service = SettingsService::open_with_debounce(directory.path().to_path_buf(), Duration::from_hours(1)).unwrap();
  service.set_worktree_epochs(|_| WorktreeEpoch::Lease("lease-a".to_owned()));
  (directory, service)
}

fn hidden_until(service: &SettingsService) -> Value {
  service.value("repositoryState")[PATH]["landedHiddenUntil"].clone()
}

/// A hide asked on the view of a session that knew no lease of its slot, which reads one now (`AnotherLease::ReadAgain`),
/// is refused as `lease_read_again` and hides nothing: the page asks again on the view of the lease read. One of a view
/// of another lease hides nothing and answers nothing, one of the view of the lease of the slot hides under it
#[test]
fn a_hide_of_a_session_that_knew_no_lease_of_its_slot_is_asked_again_and_hides_nothing() {
  let (_directory, service) = settings_of_lease_a();
  let refused = hide_under(&service, PATH, Err(AnotherLease::ReadAgain), 500).unwrap_err();
  assert_eq!(refused.code, ErrorCode::LeaseReadAgain, "{refused:?}");
  assert_eq!(hidden_until(&service), Value::Null, "nothing hidden");

  assert_eq!(hide_under(&service, PATH, Err(AnotherLease::Held), 500).unwrap(), None);
  assert_eq!(hidden_until(&service), Value::Null, "nothing hidden of the view of another lease");

  assert!(hide_under(&service, PATH, Ok(WorktreeEpoch::Lease("lease-a".to_owned())), 500).unwrap().is_some());
  assert_eq!(hidden_until(&service), Value::from(500), "the view of the lease of the slot hides");
}
