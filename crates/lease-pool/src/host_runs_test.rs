use super::*;
use crate::host_slots::HostProcess;
use pretty_assertions::assert_eq;

#[test]
fn the_queue_line_names_the_wait_the_runs_ahead_the_lent_slots_the_quarantine_and_an_upgrade_of_git() {
  let queued = Queued {
    ahead: 2,
    slots: 4,
    lent: vec![(1, "run-a".to_owned()), (2, "run-b".to_owned())],
    quarantined: vec![(
      3,
      vec![HostProcess {
        pid: 9,
        started: None,
        name: "sh".to_owned(),
        command: "sh -c x".to_owned(),
      }],
    )],
    git_upgrading: Some("brew upgrade git".to_owned()),
  };
  assert_eq!(
    queue_line("1792-81", &queued, Duration::from_secs(60)),
    "run 1792-81 waits 60 s: 2 runs ahead, 4 slots, lent to run-a (1), run-b (2), 1 in quarantine, git is upgrading (`brew upgrade git`)"
  );
  let empty = Queued {
    ahead: 0,
    slots: 0,
    lent: Vec::new(),
    quarantined: Vec::new(),
    git_upgrading: None,
  };
  assert_eq!(queue_line("r", &empty, Duration::ZERO), "run r waits 0 s: 0 runs ahead, 0 slots, lent to none");
}
