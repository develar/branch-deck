use super::*;
use crate::host_slots::{HostProcess, Ticket};
use crate::host_turn::{HeadDrain, LightEntry, Need};
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

/// The line of a wait names what the run waits for, what each holder holds and why, a holder without a measure past its
/// grace its live use, what the runs that are not heavy use, and the runs ahead
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
    passed: None,
    measure_as: None,
    recent: None,
  };
  assert_eq!(
    holds_text(Need::Recent(Cores(12))),
    (
      "holds 1.2".to_owned(),
      "no measure: it held the whole host for its first 10 min, now its live use of the last 5 min".to_owned()
    ),
    "a holder without a measure past its grace"
  );
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
    window: None,
    light_runs: Vec::new(),
    ahead: vec![entry("1790-03", "~/projects/idea-1", "./community/tools/bt.cmd --lane fast", now_ms - 60_000, None)],
    head_drain: None,
  };
  assert_eq!(
    turn_line("1792-81", &queue, None, Duration::from_mins(3), Duration::from_mins(117), now_ms),
    "run 1792-81 waits 3 min for the heavy turn of the host, 7.5 of its 20 cores, 117 min left: run 1788-12 of branch-deck holds 12.0 for 4 min (pnpm e2e, measured), run 1789-40 of accio holds 6.0 for 30 s (scripts/check.sh --smoke, live, above its measure), runs that are not heavy use 2.0, 1 run ahead: 1790-03 of idea-1 (./community/tools/bt.cmd --lane fast)"
  );
  let whole = TurnQueue {
    cores: 20,
    need: Need::Unmeasured,
    holders: vec![entry("h", "/r/one", "bt --lane fast", now_ms, Some(210))],
    light: Cores(0),
    outside: None,
    window: None,
    light_runs: Vec::new(),
    ahead: Vec::new(),
    head_drain: None,
  };
  assert_eq!(
    turn_line("c", &whole, None, Duration::ZERO, Duration::from_hours(2), now_ms),
    "run c waits 0 s for the heavy turn of the host, all its 20 cores, as the command has no measure yet (a command whose text changes every run never gets one: `--measure-as <class>` gives it one), 120 min left: run h of one holds the whole host for 0 s (bt --lane fast, measured 21.0), no run ahead"
  );
  assert_eq!(
    turn_line("c", &whole, Some("accio-gate"), Duration::ZERO, Duration::from_hours(2), now_ms),
    "run c waits 0 s for the heavy turn of the host, all its 20 cores, as the class accio-gate has no measure yet, 120 min left: run h of one holds the whole host for 0 s (bt --lane fast, measured 21.0), no run ahead",
    "a run of a class of measures names its class"
  );
  let free = TurnQueue {
    cores: 20,
    need: Need::Measured(Cores(200)),
    holders: Vec::new(),
    light: Cores(0),
    outside: None,
    window: None,
    light_runs: Vec::new(),
    ahead: vec![entry("a", "/r/one", "pnpm e2e", now_ms, None), entry("b", "/r/two", "pnpm lint", now_ms, None)],
    head_drain: None,
  };
  assert_eq!(
    turn_line("c", &free, None, Duration::from_secs(5), Duration::from_secs(119), now_ms),
    "run c waits 5 s for the heavy turn of the host, all its 20 cores, as the command measured 20.0, 119 s left: no heavy run holds cores, 2 runs ahead: a of one (pnpm e2e), b of two (pnpm lint)"
  );
  let busy = TurnQueue {
    outside: Some(Cores(183)),
    ..free
  };
  assert_eq!(
    turn_line("c", &busy, None, Duration::from_secs(5), Duration::from_secs(119), now_ms),
    "run c waits 5 s for the heavy turn of the host, all its 20 cores, as the command measured 20.0, 119 s left: no heavy run holds cores, the host is busy with 18.3 of 20 cores outside the heavy runs, 2 runs ahead: a of one (pnpm e2e), b of two (pnpm lint)"
  );
}

/// An exclusive run waits for all the cores of a quiet host and holds the whole host for a measurement, and a run that is
/// not heavy says what it waits for
#[test]
fn the_lines_of_an_exclusive_run_and_of_a_run_that_waits_for_it() {
  assert_eq!(need_text(Need::Exclusive, 20, None), "all its 20 cores of a quiet host, as an exclusive run");
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
    passed: None,
    measure_as: None,
    recent: None,
  };
  assert_eq!(
    drain_line("1792-81", &holder, Duration::from_mins(2), now_ms + 780_000, now_ms),
    "run 1792-81 waits 2 min for a measurement: run 1788-12 of idea-1 waits for the host to drain (zsh ./measure-ab.sh); this run starts when the measurement ends, or in at most 13 min"
  );
  assert_eq!(
    measurement_line("1792-81", &holder, Duration::from_mins(3), now_ms),
    "run 1792-81 waits 3 min for a measurement: run 1788-12 of idea-1 holds the host for 12 min (zsh ./measure-ab.sh); a measurement holds the host; this run starts when it ends"
  );
}

/// The wait of an exclusive run names each run that is not heavy in its command, and the busy cores of the host over its
/// last 30 s
#[test]
fn the_wait_of_an_exclusive_run_names_the_runs_in_their_command_and_the_window() {
  let light = |run: &str, repo: &str, command: &str| LightEntry {
    ticket: Ticket {
      run: run.to_owned(),
      pid: 1,
      process_started: None,
    },
    repo: repo.to_owned(),
    command: command.to_owned(),
    live: Cores(0),
  };
  let queue = TurnQueue {
    cores: 20,
    need: Need::Exclusive,
    holders: Vec::new(),
    light: Cores(0),
    outside: Some(Cores(12)),
    window: Some(Cores(112)),
    light_runs: vec![light("1790-03", "/Users/u/projects/accio", "scripts/bench ab-run")],
    ahead: Vec::new(),
    head_drain: None,
  };
  assert_eq!(
    turn_line("ab", &queue, None, Duration::from_secs(5), Duration::from_secs(119), 1_000_000_000),
    "run ab waits 5 s for the heavy turn of the host, all its 20 cores of a quiet host, as an exclusive run, 119 s left: no heavy run holds cores, the host is busy with 1.2 of 20 cores outside the heavy runs, waits for run 1790-03 of accio to end: scripts/bench ab-run, the host was busy with up to 11.2 of 20 cores in the last 30 s, no run ahead"
  );
  let two = TurnQueue {
    light_runs: vec![light("1790-03", "/r/accio", "scripts/bench ab-run"), light("1791-04", "/r/idea-1", "bt --lane fast")],
    window: None,
    ..queue
  };
  assert!(
    turn_line("ab", &two, None, Duration::ZERO, Duration::ZERO, 1_000_000_000)
      .contains(", waits for 2 runs to end: run 1790-03 of accio (scripts/bench ab-run), run 1791-04 of idea-1 (bt --lane fast), no run ahead"),
    "{}",
    turn_line("ab", &two, None, Duration::ZERO, Duration::ZERO, 1_000_000_000)
  );
}

/// A holder that started past an exclusive run names it, an exclusive run ahead is marked, and the line of the exclusive
/// head and of each run behind it ends with its drain: no run passes it before its drain ends, and after it the heavy
/// runs behind it start past it in their order while they fit
#[test]
fn the_turn_line_names_the_drain_of_an_exclusive_head_and_the_run_that_passed_it() {
  let now_ms = 1_000_000_000;
  let entry = |run: &str, command: &str, exclusive: bool| TurnEntry {
    ticket: Ticket {
      run: run.to_owned(),
      pid: 1,
      process_started: None,
    },
    repo: "~/projects/idea-1".to_owned(),
    command: command.to_owned(),
    since_ms: now_ms - 180_000,
    affinity: None,
    command_key: None,
    expected: Some(Cores(40)),
    live: None,
    exclusive,
    passed: None,
    measure_as: None,
    recent: None,
  };
  let passer = TurnEntry {
    passed: Some("1788-12".to_owned()),
    ..entry("1789-40", "cargo nextest run -p git-ops", false)
  };
  let behind = TurnQueue {
    cores: 20,
    need: Need::Cores { cores: Cores(80), live: false },
    holders: vec![passer],
    light: Cores(0),
    outside: Some(Cores(120)),
    window: None,
    light_runs: Vec::new(),
    ahead: vec![entry("1788-12", "zsh ./measure-ab.sh", true)],
    head_drain: Some(HeadDrain::Ended { ended_ms: now_ms - 240_000 }),
  };
  assert_eq!(
    turn_line("1792-81", &behind, None, Duration::from_mins(20), Duration::from_mins(100), now_ms),
    "run 1792-81 waits 20 min for the heavy turn of the host, 8.0 of its 20 cores, 100 min left: run 1789-40 of idea-1 holds 4.0 for 3 min (cargo nextest run -p git-ops, measured, past the exclusive run 1788-12), the host is busy with 12.0 of 20 cores outside the heavy runs, 1 run ahead: 1788-12 of idea-1 (zsh ./measure-ab.sh, exclusive); the drain of run 1788-12 ended 4 min ago without a quiet host: the heavy runs behind it start past it in their order while they fit"
  );
  let draining = TurnQueue {
    head_drain: Some(HeadDrain::Draining { until_ms: now_ms + 540_000 }),
    holders: Vec::new(),
    ..behind.clone()
  };
  assert!(
    turn_line("1792-81", &draining, None, Duration::ZERO, Duration::ZERO, now_ms).ends_with("; run 1788-12 drains the host for 9 min more, and no run starts past it before then"),
    "{}",
    turn_line("1792-81", &draining, None, Duration::ZERO, Duration::ZERO, now_ms)
  );
  let pending = TurnQueue {
    head_drain: Some(HeadDrain::Pending),
    ..behind.clone()
  };
  assert!(
    turn_line("1792-81", &pending, None, Duration::ZERO, Duration::ZERO, now_ms)
      .ends_with("; run 1788-12 drains the host once no heavy run holds cores, and no run starts past it before its drain ends")
  );

  let head = |head_drain| TurnQueue {
    need: Need::Exclusive,
    ahead: Vec::new(),
    head_drain: Some(head_drain),
    ..behind.clone()
  };
  let line = |head_drain| turn_line("1788-12", &head(head_drain), None, Duration::ZERO, Duration::ZERO, now_ms);
  assert!(
    line(HeadDrain::Ended { ended_ms: now_ms - 240_000 })
      .ends_with(", no run ahead; its drain ended 4 min ago without a quiet host: the heavy runs behind it that fit start past it, and it starts once the host is quiet"),
    "{}",
    line(HeadDrain::Ended { ended_ms: now_ms - 240_000 })
  );
  assert!(
    line(HeadDrain::Draining { until_ms: now_ms + 540_000 })
      .ends_with("; it drains the host for 9 min more: the new runs that are not heavy wait for it, and no heavy run starts past it")
  );
  assert!(line(HeadDrain::Pending).ends_with("; it drains the host once no heavy run holds cores, and no heavy run starts past it before its drain ends"));
}
