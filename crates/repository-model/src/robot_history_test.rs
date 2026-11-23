use crate::robot_history::{read_history, record_history};
use domain_types::patronus::{FinishedRobot, PatronusPushMode, PatronusRobotStatus, SampleProfile};
use pretty_assertions::assert_eq;

fn finished(id: &str, finish_time: u32, profile: SampleProfile) -> FinishedRobot {
  FinishedRobot {
    id: id.to_owned(),
    status: PatronusRobotStatus::Success,
    push_mode: PatronusPushMode::RebaseAutosquash,
    target_branch: "master".to_owned(),
    start_time: finish_time - 2400,
    finish_time,
    profile,
  }
}

/// Two processes keep their robots in one history: each write merges with what the cache has, so neither loses the
/// other's, and a profile one learnt stays when the other names none
#[test]
fn the_history_of_the_repository_merges_the_writes_of_every_process_by_robot_id() {
  let dir = tempfile::tempdir().unwrap();
  let cache = test_utils::process_budget::cache_in(dir.path());
  assert_eq!(read_history(&cache), Vec::new());

  record_history(&cache, &[finished("a", 10_000, SampleProfile::Id("lsc".to_owned()))]);
  // the other process saw `a` in a list, which names no profile, and `b`
  record_history(&cache, &[finished("a", 10_000, SampleProfile::Unknown), finished("b", 20_000, SampleProfile::Unknown)]);
  assert_eq!(
    read_history(&cache),
    [finished("b", 20_000, SampleProfile::Unknown), finished("a", 10_000, SampleProfile::Id("lsc".to_owned()))]
  );
}
