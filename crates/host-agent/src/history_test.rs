use super::*;
use crate::cores::CoresFile;
use crate::run::{ExitRecord, KEEP};
use host_link::proto::{Phase, RunSpec};
use lease_pool::host_history::SlotRecord;
use lease_pool::host_slots::{SlotChoice, Ticket};
use lease_pool::host_turn::{HeavyTurn, TurnEntry};
use pretty_assertions::assert_eq;
use std::fs::File;
use sync_utils::process_scan::process_started;

fn spec(run: &str) -> RunSpec {
  RunSpec {
    run: run.to_owned(),
    repo: "~/repo".to_owned(),
    affinity: "checkout-1".to_owned(),
    commit: "a".repeat(40),
    command: vec!["pnpm".to_owned(), "e2e".to_owned(), "--output=out/e2e".to_owned()],
    fetch: vec!["out/e2e".to_owned()],
    heavy: true,
    group: Some("session".to_owned()),
    ..RunSpec::default()
  }
}

/// A temporary home of the agent, its history and its runs
struct Agent {
  dir: tempfile::TempDir,
  home: Home,
  history: History,
  runs: Runs,
}

impl Agent {
  fn new() -> Self {
    let dir = tempfile::tempdir().expect("a temporary home");
    let home = Home::new(dir.path().to_path_buf());
    Self {
      history: History::new(&home),
      runs: Runs::new(home.runs_dir()),
      home,
      dir,
    }
  }
}

/// A run that ended `ago` before `now`
fn ended(runs: &Runs, id: &str, now: SystemTime, ago: Duration) -> RunDir {
  let (run, _lock) = runs.create(&spec(id)).expect("a run");
  run
    .write_phases(&[Phase {
      name: "heavy".to_owned(),
      ms: 1200,
    }])
    .expect("the phases");
  run.write_exit(&ExitRecord::Code { code: 1 }).expect("its end");
  File::options()
    .write(true)
    .open(run.path().join("exit.json"))
    .and_then(|file| file.set_modified(now - ago))
    .expect("the time of its end");
  run
}

fn lines_in(path: &std::path::Path) -> Vec<HistoryLine> {
  fs::read_to_string(path)
    .unwrap_or_default()
    .lines()
    .map(|line| serde_json::from_str(line).expect("a line of the history"))
    .collect()
}

/// A run leaves its line in the month of its end as its directory goes: its command as the heavy turn names it, the key
/// of its measures, its phases, its cores, the bytes of its pack, its output and every fetch, its place in the queue,
/// and its slot with how the lend chose it
#[test]
fn a_run_leaves_its_line_in_the_month_of_its_end_as_its_directory_goes() {
  let Agent { dir, home, history, runs } = Agent::new();
  let now = SystemTime::now();
  let run = ended(&runs, "1-1", now, KEEP + Duration::from_mins(1));
  run
    .write_cores(&CoresFile {
      mean: Cores(75),
      peak: Cores(120),
      level: None,
      cpu_ms: 7500,
      wall_ms: 1000,
      ended: true,
      samples: Vec::new(),
    })
    .expect("the cores");
  run.add_pack_bytes(300).expect("a pack");
  run.add_fetched_bytes(40).expect("a fetch");
  run.add_fetched_bytes(2).expect("a second fetch");
  let queue = QueueRecord {
    place: 2,
    holders: 1,
    cores: 20,
    expected: Some(Cores(90)),
    need: NeedKind::Cores,
    behind_ms: 1000,
    load_ms: 200,
    short_ms: Some(50_000),
    passes: 0,
    drain_ms: None,
    drain_at_ms: None,
    pending_ms: None,
    pending_at_ms: None,
  };
  run.write_queue(&queue).expect("the queue");
  let slot = SlotRecord {
    n: 3,
    choice: SlotChoice::Affinity,
    waited: true,
    cleaned: false,
  };
  run.write_slot(slot).expect("the slot");
  let output = "{\"stream\":\"stdout\",\"text\":\"ok\"}\n";
  fs::write(run.events_path(), output).expect("the output");
  let (_goes_on, _lock) = runs.create(&spec("1-2")).expect("a run that goes on");

  assert_eq!(runs.remove_expired(now, &history), ["1-1"]);

  let ended_ms = epoch_ms(now - KEEP - Duration::from_mins(1));
  let lines = lines_in(&home.history_dir().join(format!("{}.jsonl", month_of(ended_ms))));
  let [line] = lines.as_slice() else { panic!("one line: {lines:?}") };
  assert_eq!(
    (line.run.as_str(), line.repo.clone(), line.command.as_str(), line.command_key.as_str()),
    (
      "1-1",
      dir.path().join("repo").display().to_string(),
      "pnpm e2e --output=out/e2e",
      measure_key(&spec("1-1").command, &["out/e2e".to_owned()], None).as_str()
    )
  );
  assert_eq!((line.heavy, line.exit, line.ended_ms, line.lost), (true, 1, Some(ended_ms), false));
  assert_eq!(line.group.as_deref(), Some("session"), "the group of the run in the heavy queue");
  assert_eq!(
    line.phases,
    [LinePhase {
      name: "heavy".to_owned(),
      ms: 1200
    }]
  );
  assert_eq!(
    line.cores,
    Some(LineCores {
      mean: Cores(75),
      peak: Cores(120),
      level: None,
      cpu_ms: 7500,
      wall_ms: 1000
    })
  );
  assert_eq!(
    line.bytes,
    LineBytes {
      pack: 300,
      output: output.len() as u64,
      fetch: 42
    }
  );
  assert_eq!(line.queue, Some(queue));
  assert_eq!(line.slot, Some(slot));
  let json = serde_json::to_value(line).expect("JSON");
  assert_eq!(
    (&json["repo"], &json["slot"]),
    (&serde_json::json!(line.repo), &serde_json::json!({"n": 3, "choice": "affinity", "waited": true})),
    "{json}"
  );
  assert_eq!(runs.list().len(), 1, "the run that goes on stays, and has no line");
}

/// A forgotten run past the kept ones leaves its line too, its output with the bytes that the forget cut
#[test]
fn a_forgotten_run_past_the_kept_ones_leaves_its_line_with_its_whole_output() {
  let Agent { dir: _dir, home, history, runs } = Agent::new();
  let now = SystemTime::now();
  let old = ended(&runs, "1-1", now, Duration::from_mins(2));
  let event = "{\"stream\":\"stdout\",\"text\":\"x\"}\n";
  fs::write(old.events_path(), event.repeat(40_000)).expect("a long output");
  old.forget().expect("forgotten");
  let kept = ended(&runs, "1-2", now, Duration::from_mins(1));
  kept.forget().expect("forgotten");

  assert_eq!(runs.remove_forgotten_past(1, &history), ["1-1"]);

  let lines = lines_in(&home.history_dir().join(format!("{}.jsonl", month_of(epoch_ms(now - Duration::from_mins(2))))));
  assert_eq!(
    lines.iter().map(|line| (line.run.as_str(), line.bytes.output)).collect::<Vec<_>>(),
    [("1-1", (event.len() * 40_000) as u64)]
  );
}

/// The hourly cleanup removes the months of the history whose every line is past 90 days, and leaves other files
#[test]
fn the_months_of_the_history_past_ninety_days_go() {
  let dir = tempfile::tempdir().expect("a temporary home");
  let home = Home::new(dir.path().to_path_buf());
  let history_dir = home.history_dir();
  fs::create_dir_all(&history_dir).expect("the history");
  for name in ["2026-07.jsonl", "bases-2026-07.jsonl", "2026-08.jsonl", "bases-2026-11.jsonl", "notes.txt"] {
    fs::write(history_dir.join(name), "").expect("a file");
  }
  // 2026-11-24: 90 days before is 2026-08-26
  let now = 20_781 * DAY_MS;
  let mut said = History::new(&home).expire(now);
  said.sort();
  assert_eq!(
    said,
    ["removed the history 2026-07.jsonl, past 90 days", "removed the history bases-2026-07.jsonl, past 90 days"]
  );
  let mut left: Vec<String> = fs::read_dir(&history_dir)
    .expect("the history")
    .map(|entry| entry.expect("an entry").file_name().to_string_lossy().into_owned())
    .collect();
  left.sort();
  assert_eq!(left, ["2026-08.jsonl", "bases-2026-11.jsonl", "notes.txt"]);
}

/// The summary reads the lines of its window and the ended runs that the agent keeps, but no run that goes on, and the
/// packs of the bases; a line that does not parse is skipped
#[test]
fn the_summary_reads_the_lines_of_its_window_and_the_ended_runs_that_the_agent_keeps() {
  let Agent { dir: _dir, home, history, runs } = Agent::new();
  let now = SystemTime::now();
  let now_ms = epoch_ms(now);
  let kept = ended(&runs, "1-1", now, Duration::from_mins(1));
  let old = HistoryLine {
    started_ms: now_ms - 3 * DAY_MS,
    ended_ms: Some(now_ms - 3 * DAY_MS),
    ..history.line_of(&kept).expect("a line").expect("an end")
  };
  let recent = HistoryLine {
    run: "1-0".to_owned(),
    started_ms: now_ms - 3_600_000,
    ended_ms: Some(now_ms - 3_000_000),
    ..old.clone()
  };
  history.append(&old);
  history.append(&recent);
  append_private(&home.history_dir().join(format!("{}.jsonl", month_of(now_ms))))
    .and_then(|mut file| file.write_all(b"not a line\n"))
    .expect("a broken line");
  history.note_base_pack("checkout-1", 500, now_ms - 60_000);
  let (_goes_on, _lock) = runs.create(&spec("1-2")).expect("a run that goes on");
  let waiting = |at_ms| lease_pool::host_series::TurnMinute {
    at_ms,
    cores: 20,
    busy: Some(Cores(50)),
    queue: 1,
    cause: Some(lease_pool::host_series::IdleCause::Drain),
    ..lease_pool::host_series::TurnMinute::default()
  };
  for at_ms in [now_ms - 2 * DAY_MS, now_ms - 120_000, now_ms - 60_000] {
    crate::series::append(&home.history_dir(), &waiting(at_ms)).expect("a minute of the series");
  }

  let Reply::Stats { report } = stats(&home, 1, now_ms).expect("the summary") else {
    panic!("a summary")
  };
  let report: StatsReport = serde_json::from_value(report).expect("the report");
  assert_eq!((report.runs, report.heavy, report.days), (2, 2, 1));
  assert_eq!(report.bytes.base, 500);
  assert_eq!(report.commands.len(), 1);
  let series = report.series.expect("the minutes of the window");
  assert_eq!((series.minutes, series.waiting, series.idle), (2, 2, Some(Cores(150))));
  assert_eq!(stats(&home, 91, now_ms).unwrap_err().code, Code::BadRequest);
}

/// The watch of an exclusive run at the head of the queue behind a holder writes its wait for the holders before its
/// drain, a part of its wait behind them
#[test]
fn the_watch_of_an_exclusive_run_writes_its_wait_for_the_holders() {
  let dir = tempfile::tempdir().expect("a temporary home");
  let home = Home::new(dir.path().to_path_buf());
  fs::create_dir_all(home.heavy_dir()).expect("the heavy turn");
  let turn = HeavyTurn {
    cores: 20,
    holders: vec![entry("holder")],
    queue: vec![TurnEntry { exclusive: true, ..entry("1-9") }],
    ..HeavyTurn::default()
  };
  fs::write(home.heavy_dir().join("turn.json"), serde_json::to_vec(&turn).expect("JSON")).expect("the table");
  let command = ["zsh".to_owned(), "./measure-ab.sh".to_owned()];
  let before = now_ms();
  let mut watch = QueueWatch::start(&home, &turn_run(&command, true));
  watch.look();
  std::thread::sleep(Duration::from_millis(20));
  let queue = finished(&home, watch);
  let pending = queue.pending_ms.expect("the wait of an exclusive run for the holders");
  assert!(pending >= 20 && queue.behind_ms >= pending && queue.drain_ms == Some(0), "{queue:?}");
  assert!(queue.pending_at_ms.is_some_and(|at| at >= before), "{queue:?}");
}

/// The `queue.json` that `watch` writes as the wait of the run 1-9 ends
fn finished(home: &Home, watch: QueueWatch) -> QueueRecord {
  let runs = Runs::new(home.runs_dir());
  let (run, _lock) = runs.create(&spec("1-9")).expect("a run");
  watch.finish(&run);
  run.queue().expect("queue.json")
}

/// The heavy run 1-9 of `command` on a host of 20 cores, `exclusive` or not
fn turn_run(command: &[String], exclusive: bool) -> crate::turn::TurnRun<'_> {
  crate::turn::TurnRun {
    run: "1-9",
    repo: "/r",
    affinity: "checkout-1",
    command,
    cores: 20,
    exclusive,
    fetch: &[],
    measure_as: None,
    group: None,
  }
}

fn entry(run: &str) -> TurnEntry {
  let pid = std::process::id();
  TurnEntry {
    ticket: Ticket {
      run: run.to_owned(),
      pid,
      process_started: process_started(pid),
    },
    repo: "/r".to_owned(),
    command: "pnpm e2e".to_owned(),
    ..TurnEntry::default()
  }
}

/// The watch of a heavy run counts the holders as it joins, finds its place at its first look in the queue, and writes
/// what it needed, how long it waited behind the others, its class and how often short runs passed it
#[test]
fn the_watch_of_a_heavy_run_writes_its_place_and_its_wait_in_the_queue() {
  let dir = tempfile::tempdir().expect("a temporary home");
  let home = Home::new(dir.path().to_path_buf());
  fs::create_dir_all(home.heavy_dir()).expect("the heavy turn");
  let table = |queue: Vec<TurnEntry>| {
    let turn = HeavyTurn {
      cores: 20,
      holders: vec![entry("holder")],
      queue,
      ..HeavyTurn::default()
    };
    fs::write(home.heavy_dir().join("turn.json"), serde_json::to_vec(&turn).expect("JSON")).expect("the table");
  };
  table(vec![entry("first")]);
  let command = ["pnpm".to_owned(), "e2e".to_owned()];
  let of = turn_run(&command, false);
  let mut watch = QueueWatch::start(&home, &of);
  table(vec![entry("first"), entry("1-9")]);
  // a short run passed it twice, by the marks of the queue
  let marks = lease_pool::host_turn::QueueMarks {
    runs: [(
      "1-9".to_owned(),
      lease_pool::host_turn::RunMark {
        passes: 2,
        ..lease_pool::host_turn::RunMark::default()
      },
    )]
    .into(),
  };
  fs::write(home.heavy_dir().join("passes.json"), serde_json::to_vec(&marks).expect("JSON")).expect("the marks");
  watch.look();
  let queue = finished(&home, watch);
  assert_eq!(
    (
      queue.place,
      queue.holders,
      queue.cores,
      queue.expected,
      queue.need,
      queue.load_ms,
      queue.short_ms,
      queue.passes
    ),
    (1, 1, 20, None, NeedKind::Unmeasured, 0, None, 2)
  );
  assert_eq!(
    (queue.drain_ms, queue.drain_at_ms, queue.pending_ms, queue.pending_at_ms),
    (None, None, None, None),
    "a run that is not exclusive drains nothing"
  );
}

/// The watch of an exclusive run at the head of the queue writes the part of its wait for the load of the host in which
/// it drained the host, and when a look first found that drain; a run that is not exclusive writes none
#[test]
fn the_watch_of_an_exclusive_run_writes_its_drain() {
  let dir = tempfile::tempdir().expect("a temporary home");
  let home = Home::new(dir.path().to_path_buf());
  fs::create_dir_all(home.heavy_dir()).expect("the heavy turn");
  let head = TurnEntry { exclusive: true, ..entry("1-9") };
  let turn = HeavyTurn {
    cores: 20,
    queue: vec![head.clone()],
    ..HeavyTurn::default()
  };
  fs::write(home.heavy_dir().join("turn.json"), serde_json::to_vec(&turn).expect("JSON")).expect("the table");
  let mut drain = serde_json::to_value(&head).expect("JSON");
  drain["drain_since_ms"] = serde_json::json!(now_ms());
  fs::write(home.heavy_dir().join("drain.json"), serde_json::to_vec(&drain).expect("JSON")).expect("the drain");
  let command = ["zsh".to_owned(), "./measure-ab.sh".to_owned()];
  let of = turn_run(&command, true);
  let before = now_ms();
  let mut watch = QueueWatch::start(&home, &of);
  watch.look();
  std::thread::sleep(Duration::from_millis(20));
  let queue = finished(&home, watch);
  assert_eq!(queue.need, NeedKind::Exclusive);
  let drained = queue.drain_ms.expect("the drain of an exclusive run");
  assert!(drained >= 20 && queue.load_ms >= drained, "{queue:?}");
  assert!(queue.drain_at_ms.is_some_and(|at| at >= before), "{queue:?}");
}
