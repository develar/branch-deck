use super::*;
use crate::host_slots::now_ms;
use crate::host_turn::host_turn_test::{command, note_live, params};
use crate::host_turn::{DRAIN_BAR, HostWindow, QueueMarks, RunMark, TurnAnswer, TurnRequest, leave, note_host, note_host_window, take};
use pretty_assertions::assert_eq;

const MIN: u64 = 60_000;

/// A request of this process from a checkout of its own, whose command measured `tenths` (`None`: no measure)
fn request<'a>(run: &'a str, command: &'a [String], tenths: Option<u32>) -> TurnRequest<'a> {
  let pid = std::process::id();
  TurnRequest {
    run,
    pid,
    process_started: sync_utils::process_scan::process_started(pid),
    repo: "/Users/u/projects/branch-deck",
    command,
    affinity: run,
    command_key: run,
    expected: tenths.map(Cores),
    exclusive: false,
    passed_wall: None,
    measure_as: None,
    group: None,
  }
}

/// The walls of a turn whose runs have none
fn no_wall(_: &TurnEntry) -> Option<u64> {
  None
}

const NO_WALLS: Walls<'static> = Walls { wall: &no_wall, passed: &no_wall };

/// The walls of the tests: of every run of the queue and of the holders by its id, the passed ones of 3 runs at least
fn walls_of<'a>(wall: &'a dyn Fn(&TurnEntry) -> Option<u64>, passed: &'a dyn Fn(&TurnEntry) -> Option<u64>) -> Walls<'a> {
  Walls { wall, passed }
}

/// The minute of `dir` now, with the dry starts of `dry`
fn minute_with(dir: &Path, drain_bar: Duration, walls: &Walls<'_>, dry: &mut DryState) -> TurnMinute {
  let busy = MinuteBusy {
    mean: Cores(130),
    most: Cores(150),
  };
  minute(dir, 20, now_ms(), drain_bar, Some(busy), walls, dry).unwrap().expect("a minute of a turn with runs")
}

/// The minute of `dir` now, the first of a writer
fn minute_now(dir: &Path, drain_bar: Duration, walls: &Walls<'_>) -> TurnMinute {
  minute_with(dir, drain_bar, walls, &mut DryState::default())
}

/// The bytes of each file of `dir`
fn files(dir: &Path) -> BTreeMap<String, Vec<u8>> {
  std::fs::read_dir(dir)
    .unwrap()
    .map(|entry| entry.unwrap())
    .map(|entry| (entry.file_name().to_string_lossy().into_owned(), std::fs::read(entry.path()).unwrap()))
    .collect()
}

/// A minute names each holder with what it counts and uses, the head of the queue with its need and fit, the busy cores
/// outside the heavy runs, and why the head did not start: the cores, the holders before their command, the whole host
#[test]
fn a_minute_names_the_holders_the_head_and_why_no_run_started() {
  let dir = tempfile::tempdir().unwrap();
  let params = params(dir.path());
  let command = command();
  let walls = NO_WALLS;
  assert_eq!(
    minute(dir.path(), 20, now_ms(), DRAIN_BAR, None, &walls, &mut DryState::default()).unwrap(),
    None,
    "no table, no minute"
  );
  assert_eq!(take(&params, &request("lane", &command, Some(120))).unwrap(), TurnAnswer::Taken);
  assert_eq!(minute_now(dir.path(), DRAIN_BAR, &walls).queue, 0);

  assert!(matches!(take(&params, &request("next", &command, Some(100))).unwrap(), TurnAnswer::Queued(_)));
  note_host(dir.path(), Cores(0)).unwrap();
  let before_command = minute_now(dir.path(), DRAIN_BAR, &walls);
  assert_eq!(before_command.cause, Some(IdleCause::Slots), "12.0 counted before the command of the holder");

  note_live(&params, "lane", &command, 120);
  note_host(dir.path(), Cores(130)).unwrap();
  let minute = minute_now(dir.path(), DRAIN_BAR, &walls);
  assert_eq!(
    (minute.cores, minute.busy, minute.busy_most, minute.outside, minute.queue, minute.cause),
    (20, Some(Cores(130)), Some(Cores(150)), Some(Cores(10)), 1, Some(IdleCause::Cores))
  );
  assert_eq!(
    minute.holders,
    vec![MinuteHolder {
      run: "lane".to_owned(),
      repo: "branch-deck".to_owned(),
      command: "cargo nextest run --workspace".to_owned(),
      measure_as: None,
      exclusive: false,
      counted: Cores(120),
      whole: false,
      live: Some(Cores(120)),
      since_ms: minute.holders[0].since_ms,
    }]
  );
  let head = minute.head.clone().unwrap();
  assert_eq!((head.run.as_str(), head.need, head.whole, head.fits, head.drain), ("next", Cores(100), false, false, None));

  assert!(!leave(&params, "next").unwrap());
  assert!(matches!(take(&params, &request("first", &command, None)).unwrap(), TurnAnswer::Queued(_)));
  assert_eq!(minute_now(dir.path(), DRAIN_BAR, &walls).cause, Some(IdleCause::WholeHost));
  assert!(leave(&params, "lane").unwrap());
  note_host(dir.path(), Cores(0)).unwrap();
  let fits = minute_now(dir.path(), DRAIN_BAR, &walls);
  assert_eq!(
    (fits.cause, fits.head.map(|head| head.fits)),
    (None, Some(true)),
    "a head that fits starts at its next look"
  );
  let json = serde_json::to_string(&minute).unwrap();
  assert_eq!(serde_json::from_str::<TurnMinute>(&json).unwrap(), minute, "{json}");

  // a holder without a measure counts the whole host in its grace
  assert!(!leave(&params, "first").unwrap());
  assert_eq!(take(&params, &request("gate", &command, None)).unwrap(), TurnAnswer::Taken);
  assert!(matches!(take(&params, &request("lint", &command, Some(30))).unwrap(), TurnAnswer::Queued(_)));
  assert_eq!(minute_now(dir.path(), DRAIN_BAR, &walls).cause, Some(IdleCause::Slots), "before its command");
  note_live(&params, "gate", &command, 10);
  let whole = minute_now(dir.path(), DRAIN_BAR, &walls);
  assert_eq!(
    (whole.cause, whole.holders[0].whole, whole.holders[0].counted),
    (Some(IdleCause::WholeHolder), true, Cores(200)),
    "its command runs on 1 core"
  );
}

/// A cause, a drain state of a later build reads as `Other`, so a minute and a summary of a later agent still read
#[test]
fn a_value_of_a_later_build_reads_as_other() {
  let minute: TurnMinute = serde_json::from_str(
    r#"{"at_ms":1,"cores":20,"queue":2,"cause":"memory","head":{"run":"r","repo":"x","command":"c","need":2.0,"since_ms":0,"drain":{"state":"paused","for_ms":3}}}"#,
  )
  .unwrap();
  assert_eq!((minute.cause, minute.head.unwrap().drain), (Some(IdleCause::Other), Some(MinuteDrain::Other)));
  let totals: SeriesTotals = serde_json::from_str(r#"{"minutes":1,"waiting":1,"causes":[{"cause":"memory","minutes":1}]}"#).unwrap();
  assert_eq!(totals.causes[0].cause, Some(IdleCause::Other));
  assert!(totals.line(1).contains("a cause of a later build 1 min"), "{}", totals.line(1));
}

/// An exclusive run that holds, an exclusive head that waits for the holders, drains, or waits for a quiet host with no
/// run behind it to pass, each is a cause; a run behind a drain that ended and fits passes at the next look
#[test]
fn a_minute_names_the_phase_of_an_exclusive_run() {
  let dir = tempfile::tempdir().unwrap();
  let params = params(dir.path());
  let command = command();
  let walls = NO_WALLS;
  let exclusive = TurnRequest {
    exclusive: true,
    ..request("ab", &command, None)
  };
  assert_eq!(take(&params, &exclusive).unwrap(), TurnAnswer::Taken, "a free turn without a sample of the host");
  assert!(matches!(take(&params, &request("lint", &command, Some(20))).unwrap(), TurnAnswer::Queued(_)));
  let held = minute_now(dir.path(), DRAIN_BAR, &walls);
  assert_eq!((held.cause, held.backfills.len()), (Some(IdleCause::Exclusive), 0), "no backfill beside a measurement");
  assert!(leave(&params, "ab").unwrap());
  assert!(!leave(&params, "lint").unwrap());

  assert_eq!(take(&params, &request("gate", &command, Some(80))).unwrap(), TurnAnswer::Taken);
  assert!(matches!(take(&params, &exclusive).unwrap(), TurnAnswer::Queued(_)));
  assert_eq!(minute_now(dir.path(), DRAIN_BAR, &walls).cause, Some(IdleCause::Pending));
  assert!(leave(&params, "gate").unwrap());

  note_live(&params, "bench", &command, 0);
  assert!(matches!(take(&params, &exclusive).unwrap(), TurnAnswer::Queued(_)));
  let draining = minute_now(dir.path(), DRAIN_BAR, &walls);
  assert_eq!((draining.cause, draining.backfills.len()), (Some(IdleCause::Drain), 0), "no backfill in a drain");
  assert!(matches!(draining.head.unwrap().drain, Some(MinuteDrain::Draining { .. })));
  assert_eq!(minute_now(dir.path(), Duration::ZERO, &walls).cause, Some(IdleCause::Quiet), "no run behind it");
  assert!(matches!(take(&params, &request("lint", &command, Some(20))).unwrap(), TurnAnswer::Queued(_)));
  let ended = minute_now(dir.path(), Duration::ZERO, &walls);
  assert_eq!(
    (ended.cause, ended.head.unwrap().passer.as_deref()),
    (None, Some("lint")),
    "the run behind an ended drain fits and passes at its next look"
  );

  // behind an ended drain the head of the dry backfill is the run that may pass the exclusive head
  assert!(!leave(&params, "lint").unwrap());
  for (run, tenths) in [("big", 180), ("small", 10)] {
    assert!(matches!(take(&params, &request(run, &command, Some(tenths))).unwrap(), TurnAnswer::Queued(_)));
  }
  // the estimate of the start of the head reads the most of 30 s
  let mut window = HostWindow::default();
  for at in (0..=30_000).step_by(2_000) {
    window.push(now_ms() - 30_000 + at, Cores(100));
  }
  note_host_window(dir.path(), &window).unwrap();
  let passed = |entry: &TurnEntry| (entry.ticket.run == "small").then_some(2 * MIN);
  let past = minute_now(dir.path(), Duration::ZERO, &walls_of(&no_wall, &passed));
  assert_eq!(past.cause, Some(IdleCause::Cores), "18.0 beside 10.0 outside");
  let heads: Vec<(&str, &str)> = past.backfills.iter().map(|backfill| (backfill.run.as_str(), backfill.head.as_str())).collect();
  assert_eq!(heads, [("small", "big")]);
}

/// The dry backfill of design 4.1 names the first run behind the head that fits now, of a measure, not exclusive, whose
/// passed wall is at most 30 min and whose 1.25 times fit before the start of the head, and nothing acts on it: it
/// needs a known wall of every holder and of the run, a head passed fewer than 4 times, and no drain
#[test]
fn the_dry_backfill_names_the_run_that_would_start_behind_the_head() {
  let dir = tempfile::tempdir().unwrap();
  let params = params(dir.path());
  let command = command();
  assert_eq!(take(&params, &request("gate", &command, Some(80))).unwrap(), TurnAnswer::Taken);
  note_live(&params, "gate", &command, 80);
  let exclusive = TurnRequest {
    exclusive: true,
    ..request("ab", &command, None)
  };
  assert!(matches!(take(&params, &exclusive).unwrap(), TurnAnswer::Queued(_)));
  for (run, tenths) in [("whole", None), ("long", Some(30)), ("lint", Some(30)), ("e2e", Some(30))] {
    assert!(matches!(take(&params, &request(run, &command, tenths)).unwrap(), TurnAnswer::Queued(_)));
  }
  let wall = |entry: &TurnEntry| match entry.ticket.run.as_str() {
    "gate" => Some(60 * MIN),
    _ => Some(10 * MIN),
  };
  let passed = |entry: &TurnEntry| match entry.ticket.run.as_str() {
    "long" => Some(40 * MIN),
    "lint" => Some(2 * MIN),
    "e2e" => Some(6 * MIN),
    _ => None,
  };
  let before = files(dir.path());
  let mut dry = DryState::default();
  let minute = minute_with(dir.path(), DRAIN_BAR, &walls_of(&wall, &passed), &mut dry);
  assert_eq!(files(dir.path()), before, "a minute writes nothing of the turn");
  assert_eq!(minute.cause, Some(IdleCause::Pending));
  let started: Vec<(&str, u64, Cores, &str)> = minute
    .backfills
    .iter()
    .map(|backfill| (backfill.run.as_str(), backfill.wall_ms, backfill.cores, backfill.head.as_str()))
    .collect();
  assert_eq!(
    started,
    [("lint", 2 * MIN, Cores(30), "ab"), ("e2e", 6 * MIN, Cores(30), "ab")],
    "past a run of the whole host and a gate, each beside the one before"
  );
  for backfill in &minute.backfills {
    assert!(
      (59 * MIN..=60 * MIN).contains(&backfill.head_start_ms),
      "the drain begins at the end of the gate: {}",
      backfill.head_start_ms
    );
  }

  // the next minute keeps the dry starts: they hold, count passes of the head, and are no candidates again
  let next = minute_with(dir.path(), DRAIN_BAR, &walls_of(&wall, &passed), &mut dry);
  assert_eq!((next.backfills.len(), next.dry_holders), (0, 2));
  for (run, tenths) in [("late", 30), ("more", 30), ("over", 10)] {
    assert!(matches!(take(&params, &request(run, &command, Some(tenths))).unwrap(), TurnAnswer::Queued(_)));
  }
  let late = |entry: &TurnEntry| match entry.ticket.run.as_str() {
    "late" | "more" | "over" => Some(3 * MIN),
    _ => passed(entry),
  };
  let budget = minute_with(dir.path(), DRAIN_BAR, &walls_of(&wall, &late), &mut dry);
  let runs: Vec<&str> = budget.backfills.iter().map(|backfill| backfill.run.as_str()).collect();
  assert_eq!(runs, ["late", "more"], "8.0 held and 4 dry starts of 3.0 fill 20, and the head has its 4 passes");
  assert!(!leave(&params, "lint").unwrap());
  let gone = minute_with(dir.path(), DRAIN_BAR, &walls_of(&wall, &late), &mut dry);
  assert_eq!(gone.dry_holders, 3, "a run that left the queue holds no more");

  let soon = |entry: &TurnEntry| match entry.ticket.run.as_str() {
    "gate" => Some(2 * MIN),
    _ => Some(10 * MIN),
  };
  assert_eq!(
    minute_now(dir.path(), DRAIN_BAR, &walls_of(&soon, &passed)).backfills,
    [],
    "1.25 times 2 min end after the head"
  );
  let unknown = |entry: &TurnEntry| (entry.ticket.run != "gate").then_some(10 * MIN);
  assert_eq!(minute_now(dir.path(), DRAIN_BAR, &walls_of(&unknown, &passed)).backfills, [], "a holder without a wall");

  let mut marks = QueueMarks::default();
  marks.runs.insert(
    "ab".to_owned(),
    RunMark {
      passes: PASS_BUDGET - 1,
      ..RunMark::default()
    },
  );
  std::fs::write(dir.path().join("passes.json"), serde_json::to_vec(&marks).unwrap()).unwrap();
  let one = minute_now(dir.path(), DRAIN_BAR, &walls_of(&wall, &passed));
  assert_eq!(one.backfills.len(), 1, "a head passed 3 times by short runs takes one dry start");
  std::fs::remove_file(dir.path().join("passes.json")).unwrap();

  let mut window = HostWindow::default();
  window.push(now_ms(), Cores(180));
  note_host_window(dir.path(), &window).unwrap();
  let busy = minute_now(dir.path(), DRAIN_BAR, &walls_of(&wall, &passed));
  assert_eq!(busy.backfills, [], "8.0 held, 10.0 outside and 3.0 do not fit 20");
}

/// The summary of the minutes: the minutes in which heavy runs waited, their idle cores by the mean busy cores of each
/// minute, by cause with the most idle core minutes first, and the runs that the dry backfill found
#[test]
fn the_summary_of_the_minutes_gives_the_idle_cores_by_cause() {
  let at = |cause: Option<IdleCause>, queue: u32, busy: Option<u32>, backfill: Option<&str>| TurnMinute {
    schema: SCHEMA,
    at_ms: 0,
    cores: 20,
    busy: busy.map(Cores),
    queue,
    cause,
    backfills: backfill
      .map(|run| DryBackfill {
        run: run.to_owned(),
        ..DryBackfill::default()
      })
      .into_iter()
      .collect(),
    ..TurnMinute::default()
  };
  let minutes = [
    at(None, 0, Some(50), None),
    at(Some(IdleCause::Drain), 3, Some(40), None),
    at(Some(IdleCause::Drain), 3, Some(60), None),
    at(Some(IdleCause::Cores), 5, Some(190), Some("lint")),
    at(Some(IdleCause::Cores), 5, None, Some("lint")),
    at(None, 2, Some(100), Some("e2e")),
  ];
  let totals = aggregate(&minutes);
  assert_eq!(
    totals,
    SeriesTotals {
      minutes: 6,
      waiting: 5,
      idle: Some(Cores(103)),
      causes: vec![
        CauseTotal {
          cause: Some(IdleCause::Drain),
          minutes: 2,
          idle: Some(Cores(150))
        },
        CauseTotal {
          cause: None,
          minutes: 1,
          idle: Some(Cores(100))
        },
        CauseTotal {
          cause: Some(IdleCause::Cores),
          minutes: 2,
          idle: Some(Cores(10))
        },
      ],
      backfill_minutes: 3,
      backfill_runs: 2,
    }
  );
  assert_eq!(
    totals.line(90),
    "  heavy turn by the minute in the last 30 day(s): 6 min recorded, heavy runs waited in 5, 10.3 cores idle on average: an exclusive head drains 2 min at 15.0, the run that can start fitted 1 min at 10.0, the cores 2 min at 1.0; a backfill would have started 2 run(s) in 3 min (a dry run)"
  );
}
