use super::*;
use crate::host_slots::{SlotChoice, SlotMiss, Ticket};
use crate::host_turn::{HeadDrain, TurnEntry};
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
    drain_ms: None,
    drain_at_ms: None,
    pending_ms: None,
    pending_at_ms: None,
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
    level: Some(Cores(120)),
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
  assert_eq!((e2e.cores, e2e.peak), (Some(Cores(120)), Some(Cores(150))), "the level of work, as the turn expects");
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

/// A run in the queue waits behind a holder or a run before it, and for the load of the host at the head of a free
/// turn, an exclusive head in its drain for the drain
#[test]
fn a_run_in_the_queue_waits_behind_others_or_for_the_load_of_the_host() {
  let mut table = HeavyTurn {
    queue: vec![entry("head"), entry("second")],
    ..HeavyTurn::default()
  };
  let draining = Some(HeadDrain::Draining { until_ms: 1 });
  assert_eq!(spot(&table, "head", None), Some((0, QueueSpot::Load)));
  assert_eq!(spot(&table, "head", draining), Some((0, QueueSpot::Drain)));
  assert_eq!(spot(&table, "head", Some(HeadDrain::Ended { ended_ms: 1 })), Some((0, QueueSpot::Load)));
  assert_eq!(spot(&table, "second", draining), Some((1, QueueSpot::Behind)));
  assert_eq!(spot(&table, "gone", None), None);
  table.holders.push(entry("holder"));
  assert_eq!(spot(&table, "head", None), Some((0, QueueSpot::Behind)));
  assert_eq!(
    spot(&table, "head", Some(HeadDrain::Pending)),
    Some((0, QueueSpot::Pending)),
    "an exclusive head waits for the holders"
  );
  assert_eq!(spot(&table, "second", Some(HeadDrain::Pending)), Some((1, QueueSpot::Behind)));
}

/// A need of a later build reads as `Other`, so the line of a later agent still counts
#[test]
fn a_need_of_a_later_build_reads_as_other() {
  let queue: QueueRecord = serde_json::from_str(r#"{"place":0,"holders":0,"cores":20,"need":"memory","behind_ms":0,"load_ms":0}"#).unwrap();
  assert_eq!(queue.need, NeedKind::Other);
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
  clock.note(4200, Some(QueueSpot::Drain));
  clock.note(4500, Some(QueueSpot::Pending));
  clock.close(4600);
  assert_eq!(
    (clock.behind_ms, clock.load_ms, clock.drain_ms, clock.pending_ms),
    (3100, 1000, 300, 100),
    "a drain is a wait for the load too, and the wait of an exclusive head for the holders one behind them"
  );
}

/// The cost of an exclusive command: the hold of its runs after their phase `heavy`, their drains, the other runs whose
/// wait for the heavy turn or for a measurement overlapped either, and its outcomes; a run whose line names no drain
/// counts apart
#[test]
fn the_summary_gives_the_cost_of_each_exclusive_command() {
  let exclusive = |run: &str, started_ms: u64, drain: Option<(u64, u64)>| {
    let mut line = heavy(
      line(
        run,
        "bt --lane fast --no-cache --json",
        started_ms,
        3,
        &[("heavy", 10 * MIN), ("slot", MIN), ("command", 13 * MIN)],
      ),
      QueueRecord {
        drain_at_ms: drain.map(|(at, _)| at),
        drain_ms: drain.map(|(_, ms)| ms),
        // it waits for the holders from minute 1 to 7, before its drain
        pending_at_ms: drain.map(|_| started_ms + MIN),
        pending_ms: drain.map(|_| 6 * MIN),
        ..queue(0, NeedKind::Exclusive, 5 * MIN, 5 * MIN)
      },
    );
    line.exclusive = true;
    line
  };
  let lines = [
    // drains 3 min from minute 7 and holds from minute 10 to 24
    exclusive("x1", NOV_24, Some((NOV_24 + 7 * MIN, 3 * MIN))),
    // an older supervisor: no drain of record; holds from minute 70 to 84
    exclusive("x2", NOV_24 + 60 * MIN, None),
    // waits for the heavy turn from minute 5 to 15: 2 min behind the head before its drain, 3 min in the drain and 5 min
    // in the hold
    heavy(
      line("h1", "pnpm e2e", NOV_24 + 5 * MIN, 0, &[("heavy", 10 * MIN), ("command", 5 * MIN)]),
      queue(1, NeedKind::Cores, 10 * MIN, 0),
    ),
    // waits for the heavy turn from minute 0 to 4: 3 min behind the head before its drain
    heavy(
      line("h2", "pnpm lint:backend", NOV_24, 0, &[("heavy", 4 * MIN), ("command", MIN)]),
      queue(1, NeedKind::Cores, 4 * MIN, 0),
    ),
    // waits for the measurement from minute 20 to 22, then runs
    line("l1", "pnpm lint", NOV_24 + 20 * MIN, 0, &[("exclusive", 2 * MIN), ("command", MIN)]),
    // runs from minute 30, after the hold
    line("l2", "pnpm lint", NOV_24 + 30 * MIN, 0, &[("exclusive", 0), ("command", MIN)]),
    // waits from minute 75 to 76 behind the second
    line("l3", "pnpm lint", NOV_24 + 75 * MIN, 0, &[("slot", 0), ("exclusive", MIN), ("command", MIN)]),
  ];
  let report = aggregate(&lines, &[], NOV_24, NOV_24 + DAY, 1);
  assert_eq!(
    report.exclusive_costs,
    vec![ExclusiveCost {
      repo: "/Users/u/idea-1".to_owned(),
      command_key: "bt --lane fast --no-cache --json".to_owned(),
      command: "bt --lane fast --no-cache --json".to_owned(),
      measure_as: None,
      runs: 2,
      held_ms: 28 * MIN,
      drain_ms: 3 * MIN,
      drain_unknown: 1,
      waiters: 3,
      waited_ms: 11 * MIN,
      pending_ms: 6 * MIN,
      pending_waiters: 2,
      pending_waited_ms: 5 * MIN,
      outcomes: vec![Outcome {
        exit: 3,
        refused: None,
        cancelled: false,
        runs: 2,
      }],
    }]
  );
  assert!(
    report
      .lines("studio")
      .contains(&"    2 run(s) held the host 28 min and drained it 3 min (1 run(s) without a drain of record), 3 run(s) waited 11 min in them; before the drains they waited 6 min for the holders, while 2 run(s) waited 5 min behind them, outcomes exit 3 2: bt --lane fast --no-cache --json".to_owned()),
    "{:#?}",
    report.lines("studio")
  );
}

#[test]
fn a_day_is_its_utc_date() {
  assert_eq!(utc_date(0), "1970-01-01");
  assert_eq!(utc_date(NOV_24 / DAY), "2026-11-24");
  assert_eq!(utc_date(19_782), "2024-02-29");
  assert_eq!(utc_date(19_783), "2024-03-01");
}

/// The lines of a summary say the waits, the queue, the outcomes, the cores, the commands, the groups and the bytes
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
      "  groups by their wait:",
      "    2 run(s), 1 heavy, waited 90 min (90 min for the heavy turn), worked 3 min: checkout-1",
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

/// The summary sums the waits and the work of the runs by their group of the heavy queue: the runs of `--group` of
/// several checkouts are one group, a run without one is of the group of its affinity, the longest wait first, and the
/// groups past the 30 that waited longest are one entry
#[test]
fn the_summary_sums_the_waits_and_the_work_by_group() {
  let of_group = |run: &str, affinity: &str, group: Option<&str>, heavy_ms: u64| HistoryLine {
    affinity: affinity.to_owned(),
    group: group.map(str::to_owned),
    heavy: true,
    ..line(run, "./gate.sh", NOV_24, 0, &[("heavy", heavy_ms), ("slot", MIN), ("command", 10 * MIN)])
  };
  let lines = [
    of_group("1", "checkout-a", Some("session"), 60 * MIN),
    of_group("2", "checkout-b", Some("session"), 50 * MIN),
    of_group("3", "checkout-c", None, 5 * MIN),
  ];
  let report = aggregate(&lines, &[], NOV_24, NOV_24 + DAY, 1);
  assert_eq!(
    report.groups,
    [
      GroupStats {
        group: "session".to_owned(),
        runs: 2,
        heavy: 2,
        wait_ms: 112 * MIN,
        heavy_ms: 110 * MIN,
        work_ms: 20 * MIN,
      },
      GroupStats {
        group: "checkout-c".to_owned(),
        runs: 1,
        heavy: 1,
        wait_ms: 6 * MIN,
        heavy_ms: 5 * MIN,
        work_ms: 10 * MIN,
      },
    ]
  );
  assert_eq!(report.other_groups, None);
  let text = report.lines("studio");
  assert!(
    text.contains(&"    2 run(s), 2 heavy, waited 112 min (110 min for the heavy turn), worked 20 min: session".to_owned()),
    "{text:?}"
  );

  let many: Vec<HistoryLine> = (0..32u64).map(|n| of_group(&n.to_string(), &format!("checkout-{n}"), None, (n + 1) * MIN)).collect();
  let report = aggregate(&many, &[], NOV_24, NOV_24 + DAY, 1);
  assert_eq!((report.groups.len(), report.groups[0].group.as_str()), (TOP_GROUPS, "checkout-31"));
  assert_eq!(
    report.other_groups,
    Some(OtherGroups {
      groups: 2,
      runs: 2,
      wait_ms: 5 * MIN,
      work_ms: 20 * MIN,
    })
  );
  assert!(report.lines("studio").contains(&"    2 other group(s): 2 run(s), waited 5 min, worked 20 min".to_owned()));
}

/// A run `run` of `repo` and `command` whose lend gave it `slot`, and whose checkout took 1 s, its prepare 10 s and its
/// command `command_ms`
fn lent(run: &str, repo: &str, command: &str, slot: SlotRecord, command_ms: u64) -> HistoryLine {
  HistoryLine {
    repo: repo.to_owned(),
    slot: Some(slot),
    ..line(run, command, NOV_24, 0, &[("slot", 10), ("checkout", 1000), ("prepare", 10_000), ("command", command_ms)])
  }
}

/// The slot 1, lent at once by `choice`, not cleaned since its last run, and a miss of no slot of the affinity for
/// `SlotChoice::Idle`
fn slot(choice: SlotChoice) -> SlotRecord {
  SlotRecord {
    n: 1,
    choice,
    waited: false,
    cleaned: false,
    missed: (choice == SlotChoice::Idle).then_some(SlotMiss::NoSlot),
  }
}

const BUILD: &str = "bazel build //...";
const IDEA: &str = "/Users/u/idea-1";

/// Per repository the summary counts the lends: warm hits, hits on a slot cleaned since, and the misses by their cause,
/// `first` and `taken` by the earlier lines of the affinity; per command it gives the runs, the hits and the spreads of
/// the warm hits, the cleaned hits and the misses, each of the runs whose command ran to its end with their count: a
/// cancel whatever its exit, a refusal and a lost supervisor leave a run out, and a cancel is no failure either. A spread
/// below `MIN_SPREAD_RUNS` prints its runs only, and a part of no run leaves the line
#[test]
fn the_summary_counts_per_repository_the_runs_that_got_the_slot_of_their_affinity() {
  let mut lines: Vec<HistoryLine> = [2, 3, 4, 5, 6]
    .iter()
    .enumerate()
    .map(|(at, minutes)| lent(&format!("0{at}"), IDEA, BUILD, slot(SlotChoice::Affinity), minutes * MIN))
    .collect();
  let cut = |run: &str, edit: &dyn Fn(&mut HistoryLine)| {
    let mut line = lent(run, IDEA, BUILD, slot(SlotChoice::Affinity), MIN);
    edit(&mut line);
    line
  };
  lines.push(cut("05", &|line| {
    line.exit = 8;
    line.cancelled = true;
  }));
  lines.push(cut("06", &|line| {
    line.exit = 255;
    line.refused = Some("agent_error".to_owned());
  }));
  lines.push(cut("07", &|line| {
    line.exit = 255;
    line.lost = true;
    line.refused = Some("agent_error".to_owned());
  }));
  let cleaned = SlotRecord {
    cleaned: true,
    ..slot(SlotChoice::Affinity)
  };
  lines.push(lent("08", IDEA, BUILD, cleaned, 8 * MIN));
  // checkout-2 has no earlier line, checkout-1 has: its slot went to another checkout since
  lines.push(HistoryLine {
    affinity: "checkout-2".to_owned(),
    ..lent("09", IDEA, BUILD, slot(SlotChoice::Idle), 10 * MIN)
  });
  lines.push(lent("10", IDEA, BUILD, slot(SlotChoice::Idle), 9 * MIN));
  let lent_to_another = SlotRecord {
    missed: Some(SlotMiss::Lent),
    waited: true,
    ..slot(SlotChoice::Idle)
  };
  lines.push(lent("11", IDEA, BUILD, lent_to_another, 11 * MIN));
  let unavailable = SlotRecord {
    missed: Some(SlotMiss::Unavailable),
    ..slot(SlotChoice::Idle)
  };
  lines.push(lent("12", IDEA, "cargo nextest run", unavailable, MIN));
  lines.push(HistoryLine {
    slot: serde_json::from_value(serde_json::json!({"n": 2, "choice": "cold", "missed": "gone"})).unwrap(),
    ..lent("13", IDEA, "cargo nextest run", slot(SlotChoice::Idle), MIN)
  });
  let waited = SlotRecord {
    waited: true,
    ..slot(SlotChoice::Affinity)
  };
  lines.push(lent("a", "/Users/u/accio", BUILD, waited, MIN));
  lines.push(line("b", BUILD, NOV_24, 0, &[("command", MIN)]));
  assert_eq!(
    lines[13].slot.map(|slot| (slot.choice, slot.missed)),
    Some((SlotChoice::Other, Some(SlotMiss::Other))),
    "a choice and a cause of a later build read as other"
  );

  let report = aggregate(&lines, &[], NOV_24, NOV_24 + DAY, 1);

  let [accio, idea] = report.slots.as_slice() else {
    panic!("two repositories: {:?}", report.slots)
  };
  assert_eq!(
    (accio.repo.as_str(), accio.runs, accio.affinity, accio.cleaned, accio.waited, accio.misses),
    ("/Users/u/accio", 1, 1, 0, 1, MissTotals::default())
  );
  assert_eq!(
    (idea.runs, idea.affinity, idea.cleaned, idea.waited, idea.misses),
    (
      14,
      9,
      1,
      1,
      MissTotals {
        first: 1,
        taken: 1,
        lent: 1,
        unavailable: 1,
        other: 1,
      }
    )
  );
  let [build, nextest] = idea.commands.as_slice() else {
    panic!("two commands: {:?}", idea.commands)
  };
  let bucket = |runs: u32, p50: u64, p90: u64| SlotBucket {
    runs,
    command_ms: Spread { p50, p90 },
    checkout_ms: Some(Spread { p50: 1000, p90: 1000 }),
    prepare_ms: Some(Spread { p50: 10_000, p90: 10_000 }),
  };
  assert_eq!(
    build,
    &SlotCommandStats {
      command_key: BUILD.to_owned(),
      command: BUILD.to_owned(),
      measure_as: None,
      runs: 12,
      affinity: 9,
      cleaned: 1,
      warm: Some(bucket(5, 4 * MIN, 6 * MIN)),
      after_clean: Some(bucket(1, 8 * MIN, 8 * MIN)),
      missed: Some(bucket(3, 10 * MIN, 11 * MIN)),
    }
  );
  assert_eq!(
    (nextest.runs, nextest.affinity, nextest.warm, nextest.missed.map(|bucket| bucket.runs)),
    (2, 0, None, Some(2))
  );
  assert_eq!(idea.other_commands, None);
  let outcomes: Vec<String> = report.outcomes.iter().map(Outcome::text).collect();
  assert!(
    outcomes.contains(&"cancelled at exit 8 1".to_owned()) && !outcomes.iter().any(|outcome| outcome.starts_with("exit 8")),
    "a cancel is an outcome of its own: {outcomes:?}"
  );
  let failed = |report: &StatsReport| {
    report
      .commands
      .iter()
      .find(|command| command.command_key == BUILD && command.repo == IDEA)
      .map(|command| command.failed)
  };
  assert_eq!(failed(&report), Some(0), "the cancel that exited 8 is no failure, nor a refusal");
  let not_cancelled = HistoryLine {
    cancelled: false,
    ..lines[5].clone()
  };
  assert_eq!(
    failed(&aggregate(&[not_cancelled], &[], NOV_24, NOV_24 + DAY, 1)),
    Some(1),
    "the exit 8 of a command that no cancel cut"
  );

  let slot_lines: Vec<String> = report
    .lines("studio")
    .into_iter()
    .filter(|line| line.starts_with("  slots of ") || line.contains(" on the slot of their checkout"))
    .collect();
  assert_eq!(
    slot_lines,
    [
      "  slots of /Users/u/accio: 1 run(s) with a slot: 1 warm on the slot of their checkout (100 %); 1 waited in the queue of the slots",
      "    1 run(s), 1 on the slot of their checkout; warm 1 run(s), too few for a p50: bazel build //...",
      "  slots of /Users/u/idea-1: 14 run(s) with a slot: 8 warm on the slot of their checkout (57 %), 1 on it cleaned since, 5 on another slot (1 first of their checkout in the window, 1 whose slot another checkout took since, 1 whose slot another run of their checkout had, 1 whose slot was not ready, 1 of another cause); 1 waited in the queue of the slots",
      "    12 run(s), 9 on the slot of their checkout; warm 5 run(s), command p50 4 min p90 6 min, checkout p50 1 s, prepare p50 10 s; after a clean 1 run(s), too few for a p50; missed 3 run(s), too few for a p50: bazel build //...",
      "    2 run(s), 0 on the slot of their checkout; missed 2 run(s), too few for a p50: cargo nextest run",
    ]
  );
  let json = serde_json::to_value(&report).unwrap();
  assert_eq!(
    json["slots"][1]["commands"][0]["warm"],
    serde_json::json!({
      "runs": 5,
      "command_ms": {"p50": 4 * MIN, "p90": 6 * MIN},
      "checkout_ms": {"p50": 1000, "p90": 1000},
      "prepare_ms": {"p50": 10_000, "p90": 10_000},
    })
  );
  assert_eq!(
    json["slots"][1]["misses"],
    serde_json::json!({"first": 1, "taken": 1, "lent": 1, "unavailable": 1, "other": 1})
  );
  assert_eq!(
    (serde_json::to_value(lines[11].slot).unwrap(), serde_json::to_value(lines[8].slot).unwrap()),
    (
      serde_json::json!({"n": 1, "choice": "idle", "waited": true, "missed": "lent"}),
      serde_json::json!({"n": 1, "choice": "affinity", "cleaned": true})
    ),
    "the slot of a line"
  );
  assert!(aggregate(&lines[15..], &[], NOV_24, NOV_24 + DAY, 1).slots.is_empty(), "no line names its slot");
}

/// The lends of a repository name its `TOP_SLOT_COMMANDS` commands with the most runs, and the rest in one entry
#[test]
fn the_lends_of_a_repository_name_the_commands_with_the_most_runs_and_the_rest_in_one_entry() {
  let mut lines: Vec<HistoryLine> = (0..12)
    .map(|n| lent(&format!("{n:02}"), IDEA, &format!("command {n:02}"), slot(SlotChoice::Affinity), MIN))
    .collect();
  lines.push(lent("12", IDEA, "command 11", slot(SlotChoice::Affinity), MIN));
  let report = aggregate(&lines, &[], NOV_24, NOV_24 + DAY, 1);
  let [idea] = report.slots.as_slice() else { panic!("one repository") };
  assert_eq!(idea.commands.len(), TOP_SLOT_COMMANDS);
  assert_eq!((idea.commands[0].command.as_str(), idea.commands[0].runs), ("command 11", 2), "the most runs first");
  assert_eq!(idea.other_commands, Some(OtherSlotCommands { commands: 2, runs: 2 }));
  assert!(report.lines("studio").contains(&"    2 other command(s): 2 run(s)".to_owned()));
}

/// `first` and `taken` follow the lends, not the starts: a heavy run of a checkout that started first and got the slot
/// of its checkout after a later run that is not heavy leaves that later run the first of its checkout
#[test]
fn the_first_run_of_a_checkout_is_the_first_lend_not_the_first_start() {
  let heavy = HistoryLine {
    affinity: "checkout-3".to_owned(),
    phases: phases(&[("heavy", 30 * MIN), ("slot", 10), ("command", MIN)]),
    ..lent("1", IDEA, BUILD, slot(SlotChoice::Affinity), MIN)
  };
  let later = HistoryLine {
    affinity: "checkout-3".to_owned(),
    started_ms: NOV_24 + MIN,
    ..lent("2", IDEA, BUILD, slot(SlotChoice::Idle), MIN)
  };
  let report = aggregate(&[heavy, later], &[], NOV_24, NOV_24 + DAY, 1);
  let [idea] = report.slots.as_slice() else { panic!("one repository") };
  assert_eq!((idea.misses.first, idea.misses.taken), (1, 0));
}

/// A report of an agent before the causes of a miss has no `misses`: its line names the runs on another slot without
/// empty parentheses
#[test]
fn the_slots_of_a_report_without_the_causes_of_its_misses_read_without_them() {
  let report: StatsReport = serde_json::from_value(serde_json::json!({
    "schema": 1, "since_ms": 0, "until_ms": 0, "days": 1, "runs": 3, "heavy": 0, "exclusive": 0, "wait_ms": 0, "work_ms": 0,
    "phases": [], "queue": {"runs": 0, "behind_ms": 0, "load_ms": 0, "whole_host": 0}, "outcomes": [{"exit": 0, "runs": 3}],
    "without_command": {"runs": 0, "wait_ms": 0}, "cores": {"cpu_ms": 0, "command_ms": 0, "mean_over_window": 0, "peak": 0},
    "commands": [], "bytes": {"pack": 0, "output": 0, "fetch": 0, "base": 0}, "by_day": [],
    "slots": [{"repo": "/Users/u/idea-1", "runs": 3, "affinity": 1, "waited": 0}],
  }))
  .expect("a report of an agent at 50e855e99");
  let lines = report.lines("studio");
  assert!(
    lines.contains(&"  slots of /Users/u/idea-1: 3 run(s) with a slot: 1 warm on the slot of their checkout (33 %), 2 on another slot".to_owned()),
    "{lines:?}"
  );
}
