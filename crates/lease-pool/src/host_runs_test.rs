use super::*;
use crate::host_slots::{HostProcess, Ticket};
use crate::host_turn::Need;
use pretty_assertions::assert_eq;
use sync_utils::cores::Cores;

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

/// The line of a wait names what the run waits for, what each holder holds and why, what the runs that are not heavy
/// use, and the runs ahead
#[test]
fn the_turn_line_names_the_wait_the_cores_of_each_holder_and_the_runs_ahead() {
  let entry = |run: &str, repo: &str, command: &str, since_ms: u64, expected: Option<u32>| TurnEntry {
    ticket: Ticket {
      run: run.to_owned(),
      pid: 1,
      process_started: None,
    },
    repo: repo.to_owned(),
    command: command.to_owned(),
    since_ms,
    affinity: None,
    command_key: None,
    expected: expected.map(Cores),
    live: None,
    exclusive: false,
  };
  let now_ms = 1_000_000_000;
  let live = TurnEntry {
    live: Some(Cores(60)),
    ..entry("1789-40", "/Users/u/projects/accio", "scripts/check.sh --smoke", now_ms - 30_000, Some(40))
  };
  let queue = TurnQueue {
    cores: 20,
    need: Need::Cores { cores: Cores(75), live: false },
    holders: vec![entry("1788-12", "/Users/u/projects/branch-deck", "pnpm e2e", now_ms - 250_000, Some(120)), live],
    light: Cores(20),
    outside: None,
    ahead: vec![entry("1790-03", "~/projects/idea-1", "./community/tools/bt.cmd --lane fast", now_ms - 60_000, None)],
  };
  assert_eq!(
    turn_line("1792-81", &queue, Duration::from_mins(3), Duration::from_mins(117), now_ms),
    "run 1792-81 waits 3 min for the heavy turn of the host, 7.5 of its 20 cores, 117 min left: run 1788-12 of branch-deck holds 12.0 for 4 min (pnpm e2e, measured), run 1789-40 of accio holds 6.0 for 30 s (scripts/check.sh --smoke, live, above its measure), runs that are not heavy use 2.0, 1 run ahead: 1790-03 of idea-1 (./community/tools/bt.cmd --lane fast)"
  );
  let whole = TurnQueue {
    cores: 20,
    need: Need::Unmeasured,
    holders: vec![entry("h", "/r/one", "bt --lane fast", now_ms, Some(210))],
    light: Cores(0),
    outside: None,
    ahead: Vec::new(),
  };
  assert_eq!(
    turn_line("c", &whole, Duration::ZERO, Duration::from_hours(2), now_ms),
    "run c waits 0 s for the heavy turn of the host, all its 20 cores, as the command has no measure yet, 120 min left: run h of one holds the whole host for 0 s (bt --lane fast, measured 21.0), no run ahead"
  );
  let free = TurnQueue {
    cores: 20,
    need: Need::Measured(Cores(200)),
    holders: Vec::new(),
    light: Cores(0),
    outside: None,
    ahead: vec![entry("a", "/r/one", "pnpm e2e", now_ms, None), entry("b", "/r/two", "pnpm lint", now_ms, None)],
  };
  assert_eq!(
    turn_line("c", &free, Duration::from_secs(5), Duration::from_secs(119), now_ms),
    "run c waits 5 s for the heavy turn of the host, all its 20 cores, as the command measured 20.0, 119 s left: no heavy run holds cores, 2 runs ahead: a of one (pnpm e2e), b of two (pnpm lint)"
  );
  let busy = TurnQueue {
    outside: Some(Cores(183)),
    ..free
  };
  assert_eq!(
    turn_line("c", &busy, Duration::from_secs(5), Duration::from_secs(119), now_ms),
    "run c waits 5 s for the heavy turn of the host, all its 20 cores, as the command measured 20.0, 119 s left: no heavy run holds cores, the host is busy with 18.3 of 20 cores outside the heavy runs, 2 runs ahead: a of one (pnpm e2e), b of two (pnpm lint)"
  );
}

/// An exclusive run waits for all the cores of a quiet host and holds the whole host for a measurement, and a run that is
/// not heavy says what it waits for
#[test]
fn the_lines_of_an_exclusive_run_and_of_a_run_that_waits_for_it() {
  assert_eq!(need_text(Need::Exclusive, 20), "all its 20 cores of a quiet host, as an exclusive run");
  assert_eq!(holds_text(Need::Exclusive), ("holds the whole host for a measurement".to_owned(), "exclusive".to_owned()));
  let now_ms = 1_000_000_000;
  let holder = TurnEntry {
    ticket: Ticket {
      run: "1788-12".to_owned(),
      pid: 1,
      process_started: None,
    },
    repo: "~/projects/idea-1".to_owned(),
    command: "zsh ./measure-ab.sh".to_owned(),
    since_ms: now_ms - 720_000,
    affinity: None,
    command_key: None,
    expected: None,
    live: None,
    exclusive: true,
  };
  assert_eq!(
    measurement_line("1792-81", &holder, Duration::from_mins(3), now_ms),
    "run 1792-81 waits 3 min for a measurement: run 1788-12 of idea-1 holds the host for 12 min (zsh ./measure-ab.sh); a measurement holds the host; this run starts when it ends"
  );
}
