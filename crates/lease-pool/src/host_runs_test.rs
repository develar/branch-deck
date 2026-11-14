use super::*;
use crate::host_slots::{HostProcess, Ticket};
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

#[test]
fn the_turn_line_names_the_wait_the_time_left_the_holder_and_the_runs_ahead() {
  let entry = |run: &str, repo: &str, command: &str, since_ms: u64| TurnEntry {
    ticket: Ticket {
      run: run.to_owned(),
      pid: 1,
      process_started: None,
    },
    repo: repo.to_owned(),
    command: command.to_owned(),
    since_ms,
  };
  let now_ms = 1_000_000_000;
  let queue = TurnQueue {
    holder: Some(entry("1788-12", "/Users/u/projects/branch-deck", "cargo nextest run --workspace", now_ms - 250_000)),
    ahead: vec![entry("1790-03", "~/projects/idea-1", "./community/tools/bt.cmd --lane fast", now_ms - 60_000)],
  };
  assert_eq!(
    turn_line("1792-81", &queue, Duration::from_mins(3), Duration::from_mins(117), now_ms),
    "run 1792-81 waits 3 min for the heavy turn of the host, 117 min left: run 1788-12 of branch-deck holds it 4 min (cargo nextest run --workspace), 1 run ahead: 1790-03 of idea-1 (./community/tools/bt.cmd --lane fast)"
  );
  let free = TurnQueue {
    holder: None,
    ahead: vec![entry("a", "/r/one", "pnpm e2e", now_ms), entry("b", "/r/two", "pnpm lint", now_ms)],
  };
  assert_eq!(
    turn_line("c", &free, Duration::from_secs(5), Duration::from_secs(119), now_ms),
    "run c waits 5 s for the heavy turn of the host, 119 s left: the turn is free, 2 runs ahead: a of one (pnpm e2e), b of two (pnpm lint)"
  );
  let alone = TurnQueue {
    holder: Some(entry("h", "/r/one", "pnpm e2e", now_ms)),
    ahead: Vec::new(),
  };
  assert_eq!(
    turn_line("c", &alone, Duration::ZERO, Duration::from_hours(2), now_ms),
    "run c waits 0 s for the heavy turn of the host, 120 min left: run h of one holds it 0 s (pnpm e2e), no run ahead"
  );
}
