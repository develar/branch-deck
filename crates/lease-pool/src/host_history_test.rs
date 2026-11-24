use super::*;
use crate::host_slots::Ticket;
use crate::host_turn::TurnEntry;
use pretty_assertions::assert_eq;

const MIN: u64 = 60_000;
const DAY: u64 = 24 * 60 * MIN;
/// 2026-11-24 00:00 UTC
const NOV_24: u64 = 20_781 * DAY;

fn phases(phases: &[(&str, u64)]) -> Vec<LinePhase> {
  phases
    .iter()
    .map(|(name, ms)| LinePhase {
      name: (*name).to_owned(),
      ms: *ms,
    })
    .collect()
}

/// A run of `command` that started at `started_ms` and ran `phases`, with `exit`
fn line(run: &str, command: &str, started_ms: u64, exit: i32, ran: &[(&str, u64)]) -> HistoryLine {
  let took: u64 = ran.iter().map(|(_, ms)| ms).sum();
  HistoryLine {
    schema: SCHEMA,
    run: run.to_owned(),
    repo: "/Users/u/idea-1".to_owned(),
    affinity: "checkout-1".to_owned(),
    commit: "a".repeat(40),
    command: command.to_owned(),
    command_key: command.to_owned(),
    started_ms,
    ended_ms: Some(started_ms + took),
    exit,
    phases: phases(ran),
    ..HistoryLine::default()
  }
}

fn heavy(mut line: HistoryLine, queue: QueueRecord) -> HistoryLine {
  line.heavy = true;
  line.queue = Some(queue);
  line
}

fn queue(place: u32, need: NeedKind, behind_ms: u64, load_ms: u64) -> QueueRecord {
  QueueRecord {
    place,
    holders: 1,
    cores: 20,
    expected: None,
    need,
    behind_ms,
    load_ms,
    short_ms: None,
    passes: 0,
  }
}

/// The summary splits the waits from the work by phase, the heavy wait by what it waited for, the outcomes by exit and
/// refusal, the runs without a command, and groups the commands with their spreads and their share of failures
#[test]
fn the_summary_splits_the_waits_from_the_work_and_groups_the_commands() {
  let mut lines = vec![
    heavy(
      line("1", "pnpm e2e", NOV_24, 0, &[("heavy", 30 * MIN), ("slot", 10), ("checkout", 500), ("command", 4 * MIN)]),
      queue(2, NeedKind::Cores, 20 * MIN, 10 * MIN),
    ),
    heavy(
      line(
        "2",
        "pnpm e2e",
        NOV_24 + 10 * MIN,
        1,
        &[("heavy", 10 * MIN), ("slot", 10), ("checkout", 500), ("command", 6 * MIN)],
      ),
      queue(0, NeedKind::Unmeasured, 0, 10 * MIN),
    ),
    line("3", "cargo nextest run", NOV_24 + 20 * MIN, 130, &[("slot", 10), ("command", MIN)]),
  ];
  let mut timed_out = heavy(
    line("4", "pnpm e2e", NOV_24 + 30 * MIN, 255, &[("heavy", 120 * MIN)]),
    QueueRecord {
      passes: 4,
      ..queue(3, NeedKind::Cores, 120 * MIN, 0)
    },
  );
  timed_out.refused = Some("heavy_timeout".to_owned());
  lines.push(timed_out);
  lines[0].cores = Some(LineCores {
    mean: Cores(80),
    peak: Cores(150),
    cpu_ms: 32 * MIN,
    wall_ms: 4 * MIN,
  });
  lines[0].bytes = LineBytes {
    pack: 100,
    output: 2000,
    fetch: 30,
  };
  // a run before the window
  lines.push(line("0", "pnpm e2e", NOV_24 - DAY, 0, &[("command", MIN)]));
  let bases = [
    BasePackLine {
      at_ms: NOV_24 + MIN,
      base: "checkout-1".to_owned(),
      bytes: 5000,
    },
    BasePackLine {
      at_ms: NOV_24 - DAY,
      base: "checkout-1".to_owned(),
      bytes: 1,
    },
  ];

  let report = aggregate(&lines, &bases, NOV_24, NOV_24 + DAY, 1);

  assert_eq!((report.runs, report.heavy, report.exclusive), (4, 3, 0));
  assert_eq!(report.wait_ms, 160 * MIN + 30);
  assert_eq!(report.work_ms, 11 * MIN + 1000);
  let phases: Vec<(&str, bool, u64, u32)> = report.phases.iter().map(|phase| (phase.name.as_str(), phase.wait, phase.ms, phase.runs)).collect();
  assert_eq!(
    phases,
    [
      ("heavy", true, 160 * MIN, 3),
      ("slot", true, 30, 3),
      ("checkout", false, 1000, 2),
      ("command", false, 11 * MIN, 3)
    ]
  );
  assert_eq!(
    report.queue,
    QueueTotals {
      runs: 3,
      behind_ms: 140 * MIN,
      load_ms: 20 * MIN,
      place: Some(Spread { p50: 2, p90: 3 }),
      whole_host: 1,
      short: 0,
      passes: 4,
    }
  );
  let outcomes: Vec<(i32, Option<&str>, u32)> = report.outcomes.iter().map(|outcome| (outcome.exit, outcome.refused.as_deref(), outcome.runs)).collect();
  assert_eq!(outcomes, [(0, None, 1), (1, None, 1), (130, None, 1), (255, Some("heavy_timeout"), 1)]);
  assert_eq!(report.without_command, WithoutCommand { runs: 1, wait_ms: 120 * MIN });
  assert_eq!(report.cores.cpu_ms, 32 * MIN);
  assert_eq!(report.cores.peak, Cores(150));
  // 32 min of CPU over a day
  assert_eq!(report.cores.mean_over_window, Cores(0));

  let [e2e, nextest] = report.commands.as_slice() else {
    panic!("two commands: {:?}", report.commands);
  };
  assert_eq!(
    (e2e.command.as_str(), e2e.runs, e2e.failed, e2e.wait_ms),
    ("pnpm e2e", 3, 1, 160 * MIN + 20),
    "a timeout and a cancel are no failure of the command"
  );
  assert_eq!(
    e2e.wait,
    Some(Spread {
      p50: 30 * MIN + 10,
      p90: 120 * MIN
    })
  );
  assert_eq!(e2e.command_ms, Some(Spread { p50: 4 * MIN, p90: 6 * MIN }));
  assert_eq!((e2e.cores, e2e.peak), (Some(Cores(80)), Some(Cores(150))));
  assert_eq!((nextest.runs, nextest.failed), (1, 0));
  assert_eq!(report.other, None);

  assert_eq!(
    report.bytes,
    BytesTotals {
      pack: 100,
      output: 2000,
      fetch: 30,
      base: 5000
    }
  );
  let days: Vec<(&str, u32)> = report.by_day.iter().map(|day| (day.day.as_str(), day.runs)).collect();
  assert_eq!(days, [("2026-11-24", 4)]);
}

/// Only the 30 commands that waited longest come one by one; the rest is one entry, so the reply stays small
#[test]
fn the_commands_past_the_thirty_that_waited_longest_are_one_entry() {
  let lines: Vec<HistoryLine> = (0..35u64)
    .map(|n| line(&n.to_string(), &format!("command {n}"), NOV_24 + n, 0, &[("heavy", n * 1000), ("command", 1)]))
    .collect();
  let report = aggregate(&lines, &[], NOV_24, NOV_24 + DAY, 1);
  assert_eq!(report.commands.len(), TOP_COMMANDS);
  assert_eq!(report.commands[0].command, "command 34");
  assert_eq!(report.commands[29].command, "command 5");
  assert_eq!(
    report.other,
    Some(OtherCommands {
      commands: 5,
      runs: 5,
      failed: 0,
      wait_ms: 10_000
    })
  );
}

/// The need of a heavy run follows the measure of its command, as the heavy turn counts it
#[test]
fn the_need_of_a_heavy_run_follows_its_measure() {
  for (expected, exclusive, need) in [
    (None, false, NeedKind::Unmeasured),
    (Some(Cores(90)), false, NeedKind::Cores),
    (Some(Cores(200)), false, NeedKind::Measured),
    (Some(Cores(90)), true, NeedKind::Exclusive),
  ] {
    assert_eq!(NeedKind::of(expected, exclusive, 20), need, "{expected:?} {exclusive}");
  }
}

fn entry(run: &str) -> TurnEntry {
  TurnEntry {
    ticket: Ticket {
      run: run.to_owned(),
      pid: 1,
      process_started: None,
    },
    repo: "/r".to_owned(),
    command: "pnpm e2e".to_owned(),
    ..TurnEntry::default()
  }
}

/// A run in the queue waits behind a holder or a run before it, and for the load of the host at the head of a free turn
#[test]
fn a_run_in_the_queue_waits_behind_others_or_for_the_load_of_the_host() {
  let mut table = HeavyTurn {
    queue: vec![entry("head"), entry("second")],
    ..HeavyTurn::default()
  };
  assert_eq!(spot(&table, "head"), Some((0, QueueSpot::Load)));
  assert_eq!(spot(&table, "second"), Some((1, QueueSpot::Behind)));
  assert_eq!(spot(&table, "gone"), None);
  table.holders.push(entry("holder"));
  assert_eq!(spot(&table, "head"), Some((0, QueueSpot::Behind)));
}

/// Each span between two looks goes to the spot of the first of them; a look outside the queue starts no span
#[test]
fn the_clock_of_the_queue_splits_the_wait_by_the_spot_of_each_look() {
  let mut clock = QueueClock::default();
  clock.note(0, Some(QueueSpot::Behind));
  clock.note(1000, Some(QueueSpot::Behind));
  clock.note(3000, Some(QueueSpot::Load));
  clock.note(3500, None);
  clock.note(4000, Some(QueueSpot::Load));
  clock.close(4200);
  assert_eq!((clock.behind_ms, clock.load_ms), (3000, 700));
}

#[test]
fn a_day_is_its_utc_date() {
  assert_eq!(utc_date(0), "1970-01-01");
  assert_eq!(utc_date(NOV_24 / DAY), "2026-11-24");
  assert_eq!(utc_date(19_782), "2024-02-29");
  assert_eq!(utc_date(19_783), "2024-03-01");
}

/// The lines of a summary say the waits, the queue, the outcomes, the cores, the commands and the bytes
#[test]
fn the_lines_of_a_summary() {
  let lines = [
    heavy(
      line("1", "pnpm e2e", NOV_24, 1, &[("heavy", 90 * MIN), ("command", 3 * MIN)]),
      QueueRecord {
        short_ms: Some(50_000),
        ..queue(1, NeedKind::Cores, 80 * MIN, 10 * MIN)
      },
    ),
    line("2", "pnpm lint", NOV_24 + MIN, 0, &[("slot", 2000), ("command", 45_000)]),
  ];
  let report = aggregate(&lines, &[], NOV_24, NOV_24 + DAY, 1);
  assert_eq!(
    report.lines("studio"),
    [
      "studio: 2 run(s) in the last 1 day(s), 1 heavy, 0 exclusive: waited 90 min, worked 3 min",
      "  waits: heavy 90 min in 1, slot 2 s in 1",
      "  work: command 3 min in 2",
      "  heavy queue of 1 run(s): 80 min behind holders or runs before them, 10 min at the head of a free turn for the load of the host, joined at place 1 (p90 1), 0 for the whole host, 1 short, passed 0 time(s)",
      "  outcomes: exit 0 1, exit 1 1",
      "  cores: 0 s of CPU in 0 s of commands, 0.0 on average over the window, peak 0.0",
      "  commands by their wait:",
      "    1 run(s), 1 failed, wait p50 90 min p90 90 min, command p50 3 min p90 3 min: pnpm e2e",
      "    1 run(s), wait p50 2 s p90 2 s, command p50 45 s p90 45 s: pnpm lint",
      "  bytes: packs 0 B, output 0 B, fetched 0 B, packs of bases 0 B",
    ]
  );
  assert_eq!(hours(53 * 3_600_000 + 9 * MIN), "53 h 09 min");
  assert_eq!(size(3_565_158), "3.3 MiB");
}

/// The runs of a class of measures (`brd-host run --measure-as`) are one command of the summary, whatever their command
/// lines, and its line names the class instead of the command of the last run
#[test]
fn the_runs_of_a_class_are_one_command_named_by_the_class() {
  let classed = |run: &str, command: &str, started_ms: u64| HistoryLine {
    command_key: "key-of-accio-gate".to_owned(),
    measure_as: Some("accio-gate".to_owned()),
    ..line(run, command, started_ms, 0, &[("command", 4 * MIN)])
  };
  let lines = [
    classed("1", "bash -c ./gate.sh 1a2b 3c4d", NOV_24),
    classed("2", "bash -c ./gate.sh 5e6f 7a8b", NOV_24 + MIN),
  ];
  let report = aggregate(&lines, &[], NOV_24, NOV_24 + DAY, 1);
  let [command] = report.commands.as_slice() else {
    panic!("one command: {:?}", report.commands)
  };
  assert_eq!(
    (command.runs, command.command.as_str(), command.measure_as.as_deref()),
    (2, "bash -c ./gate.sh 5e6f 7a8b", Some("accio-gate"))
  );
  let text = report.lines("studio");
  assert!(
    text.contains(&"    2 run(s), wait p50 0 s p90 0 s, command p50 4 min p90 4 min: --measure-as accio-gate".to_owned()),
    "{text:?}"
  );
}
