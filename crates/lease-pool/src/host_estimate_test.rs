use super::*;
use pretty_assertions::assert_eq;

const NOW: u64 = 10_000_000;
const MIN: u64 = 60_000;
const REPO: &str = "/Users/u/projects/idea-1";

/// An entry of the turn of the run `run`, since `since_ms`, expecting `tenths` of cores (`None`: no measure)
fn entry(run: &str, since_ms: u64, tenths: Option<u32>) -> TurnEntry {
  TurnEntry {
    ticket: Ticket {
      run: run.to_owned(),
      pid: 1,
      process_started: None,
    },
    repo: REPO.to_owned(),
    command: format!("command of {run}"),
    since_ms,
    affinity: Some(format!("checkout-{run}")),
    command_key: Some(format!("key-{run}")),
    expected: tenths.map(Cores),
    live: None,
    exclusive: false,
    passed: None,
  }
}

/// A host of 20 cores whose holders and queue are these, busy with `outside` tenths outside them over its last 30 s
fn sight(holders: Vec<TurnEntry>, queue: Vec<TurnEntry>, outside: u32) -> Sight {
  Sight {
    turn: HeavyTurn {
      holders,
      queue,
      ..HeavyTurn::default()
    },
    cores: 20,
    outside: Some(Cores(outside)),
    window: Some(Cores(outside)),
    sampled: true,
    marks: QueueMarks::default(),
    head_drain: None,
    now_ms: NOW,
  }
}

/// The asked run of the checkout `checkout` and the command `key`, expecting `tenths` of cores
fn asked<'a>(checkout: &'a str, key: &'a str, tenths: Option<u32>) -> Asked<'a> {
  Asked {
    repo: REPO,
    affinity: checkout,
    command_key: key,
    expected: tenths.map(Cores),
    passed_wall: None,
  }
}

/// The walls of the commands of the runs `a` to `d` and of `x`, none of `unmeasured`
fn wall(entry: &TurnEntry) -> Option<u64> {
  match entry.ticket.run.as_str() {
    "a" => Some(5 * MIN),
    "b" => Some(10 * MIN),
    "c" | "d" => Some(3 * MIN),
    _ => None,
  }
}

/// The estimate in minutes and the unknown runs ahead
fn minutes(sight: &Sight, asked: &Asked<'_>) -> (u64, u32) {
  let estimate = estimate(sight, asked, &wall);
  (estimate.start_in_ms / MIN, estimate.unknown_ahead)
}

/// The estimate plays the queue forward by the rule of the admission: the holders end after the walls of their commands,
/// the queue before the run is admitted in its order while it fits, and the runs whose wall is unknown count apart
#[test]
fn an_estimate_plays_the_queue_forward_by_the_walls_of_the_commands() {
  let mine = asked("checkout-mine", "key-mine", None);
  let measured = asked("checkout-mine", "key-mine", Some(60));
  let cases: Vec<(&str, Sight, Asked<'_>, (u64, u32))> = vec![
    ("a free quiet turn starts now", sight(vec![], vec![], 10), mine, (0, 0)),
    (
      "a holder of the whole host that ran 1 of its 5 min",
      sight(vec![entry("a", NOW - MIN, None)], vec![], 10),
      mine,
      (4, 0),
    ),
    (
      "a holder whose command has no wall takes no time and counts apart",
      sight(vec![entry("unmeasured", NOW - MIN, None)], vec![], 10),
      mine,
      (0, 1),
    ),
    (
      "the run waits for the queue before it, one after the other",
      sight(vec![entry("a", NOW - MIN, None)], vec![entry("b", NOW - MIN, None)], 10),
      mine,
      (14, 0),
    ),
    (
      "a measured run starts beside a holder of a measure while both fit",
      sight(vec![entry("a", NOW, Some(60))], vec![], 10),
      measured,
      (0, 0),
    ),
    (
      "the queue fills the room as its runs end, and the run starts once the holder ends",
      sight(vec![entry("a", NOW, Some(120))], vec![entry("c", NOW, Some(60)), entry("d", NOW, Some(60))], 10),
      measured,
      (5, 0),
    ),
    (
      "a whole host on a busy free turn waits for the bound of the load",
      sight(vec![], vec![], 150),
      mine,
      (LOAD_BOUND.as_secs() / 60, 0),
    ),
    ("a measured run fits beside the load of a busy free turn", sight(vec![], vec![], 100), measured, (0, 0)),
    (
      "the run takes the place of a waiting run of its checkout and command, which it supersedes",
      sight(vec![entry("a", NOW - MIN, None)], vec![of_mine(entry("b", NOW - MIN, None)), entry("c", NOW, None)], 10),
      mine,
      (4, 0),
    ),
  ];
  for (case, sight, asked, expected) in &cases {
    assert_eq!(minutes(sight, asked), *expected, "{case}");
  }
}

/// A short run takes its place ahead of the long runs of the queue that it may pass, as the queue places it: it starts
/// beside the holder, where a run of the same measure that is not short waits for them
#[test]
fn a_short_run_is_placed_ahead_of_the_long_runs_as_the_queue_places_it() {
  let queue = sight(vec![entry("a", NOW, Some(60))], vec![entry("b", NOW - MIN, None)], 10);
  let long = asked("checkout-mine", "key-mine", Some(60));
  let short = Asked {
    passed_wall: Some(Duration::from_mins(1)),
    ..long
  };
  assert_eq!(minutes(&queue, &short), (0, 0), "short: ahead of b, beside a");
  assert_eq!(minutes(&queue, &long), (15, 0), "not short: after b, which waits for a");
  let whole = Asked { expected: None, ..short };
  assert_eq!(minutes(&queue, &whole), (15, 0), "a run without a measure is never short");
}

/// An exclusive run before the run on a busy host cannot start: the runs behind its drain pass it, so it counts apart
#[test]
fn an_exclusive_run_ahead_on_a_busy_host_counts_apart() {
  let exclusive = TurnEntry {
    exclusive: true,
    ..entry("x", NOW - MIN, None)
  };
  let busy = sight(vec![], vec![exclusive.clone()], 100);
  assert_eq!(minutes(&busy, &asked("checkout-mine", "key-mine", Some(60))), (0, 1));
  let quiet = sight(vec![entry("a", NOW, None)], vec![exclusive], 10);
  // the exclusive run starts after the holder on a quiet host, and its own wall is unknown
  assert_eq!(minutes(&quiet, &asked("checkout-mine", "key-mine", None)), (5, 1));
}

/// `entry` as a waiting run of the checkout and the command of the asked run of the tests
fn of_mine(entry: TurnEntry) -> TurnEntry {
  TurnEntry {
    affinity: Some("checkout-mine".to_owned()),
    command_key: Some("key-mine".to_owned()),
    ..entry
  }
}
