use crate::kept_statuses::KeptStatuses;
use domain_types::cli_tools::{CliTool, CliToolStatus};
use pretty_assertions::assert_eq;

fn youtrack(version: &str) -> Vec<CliToolStatus> {
  vec![CliToolStatus {
    tool: CliTool::Youtrack,
    binary: Some("/opt/homebrew/bin/youtrack".to_string()),
    version: Some(version.to_string()),
    servers: Vec::new(),
    skill: None,
    latest: Some("0.7.0".to_string()),
    manager: None,
    action: None,
    command: None,
    problem: None,
    sign_in_with: None,
  }]
}

fn kept_version(kept: &KeptStatuses) -> Option<String> {
  kept.get().map(|statuses| statuses[0].version.clone().unwrap_or_default())
}

#[test]
fn a_read_that_started_before_the_last_one_kept_keeps_nothing() {
  let kept = KeptStatuses::default();
  assert_eq!(kept_version(&kept), None);
  // the read of the Setup dot starts before an update, the read after the update ends first
  let before_update = kept.start_read();
  let after_update = kept.start_read();
  kept.keep(after_update, &youtrack("0.7.0"));
  kept.keep(before_update, &youtrack("0.6.0"));
  assert_eq!(kept_version(&kept), Some("0.7.0".to_string()));
  // in the order they started, each is kept
  let next = kept.start_read();
  let last = kept.start_read();
  kept.keep(next, &youtrack("0.7.1"));
  assert_eq!(kept_version(&kept), Some("0.7.1".to_string()));
  kept.keep(last, &youtrack("0.7.2"));
  assert_eq!(kept_version(&kept), Some("0.7.2".to_string()));
}

#[test]
fn forget_drops_the_statuses_and_the_reads_running() {
  let kept = KeptStatuses::default();
  let running = kept.start_read();
  kept.forget();
  kept.keep(running, &youtrack("0.6.0"));
  assert_eq!(kept_version(&kept), None);
  let after = kept.start_read();
  kept.keep(after, &youtrack("0.6.0"));
  assert_eq!(kept_version(&kept), Some("0.6.0".to_string()));
}
