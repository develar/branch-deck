use super::*;
use crate::host_turn::{DRAIN_BAR, GroupMark, Joined, TURN_PASS_BUDGET, TurnAnswer, TurnParams, TurnRequest, group_key, join, leave, read_groups, read_live};
use pretty_assertions::assert_eq;
use std::path::Path;

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
    measure_as: None,
    recent: None,
    group: None,
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
    files: QueueFiles::default(),
    head_drain: None,
    drain_bar: DRAIN_BAR,
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
    group: checkout,
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

/// A run of another group takes its place by the turns of the groups, as the queue places it: behind the first waiting
/// run of a group of many, not behind all of them; a run of that group waits behind all of them
#[test]
fn a_run_of_another_group_is_placed_by_the_turns_as_the_queue_places_it() {
  let mut queue = sight(
    vec![entry("a", NOW - MIN, None)],
    vec![entry("b", NOW, None), entry("c", NOW, None), entry("d", NOW, None)],
    10,
  );
  for run in ["b", "c", "d"] {
    let mark = GroupMark {
      group: "session".to_owned(),
      ..GroupMark::default()
    };
    queue.files.groups.runs.insert(run.to_owned(), mark);
  }
  let other = asked("checkout-mine", "key-mine", None);
  assert_eq!(minutes(&queue, &other), (14, 0), "after a and b, ahead of c and d");
  let same = Asked { group: "session", ..other };
  assert_eq!(minutes(&queue, &same), (20, 0), "after a, b, c and d");
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

/// A generator of numbers for the generated queues: xorshift of a fixed seed, so a failure repeats
struct Draw(u64);

impl Draw {
  fn below(&mut self, n: u64) -> u64 {
    self.0 ^= self.0 << 13;
    self.0 ^= self.0 >> 7;
    self.0 ^= self.0 << 17;
    self.0 % n
  }
}

/// The runs before which the estimate of the run of `request` places it in the queue of `dir`, then those before which
/// its join puts it, and whether it supersedes a run of another group that went ahead of a run at its budget: the case
/// in which a second copy of the rule placed it one run off
fn estimate_then_join(params: &TurnParams<'_>, request: &TurnRequest<'_>) -> (Vec<String>, Option<Vec<String>>, bool) {
  let dir = params.dir;
  let sight = Sight::of(dir, 20, crate::host_slots::now_ms(), DRAIN_BAR).unwrap();
  let group = group_key(request.group, request.affinity, request.run);
  let passes = sight.files.groups.passes_in(&sight.turn.queue);
  let crosses = sight.turn.queue.iter().any(|entry| {
    entry.affinity.as_deref() == Some(request.affinity)
      && entry.command_key.as_deref() == Some(request.command_key)
      && sight.files.groups.group_of(entry) != group
      && sight
        .files
        .groups
        .runs
        .get(&entry.ticket.run)
        .is_some_and(|mark| mark.passed.iter().any(|passed| passes.get(passed).copied() == Some(TURN_PASS_BUDGET)))
  });
  let asked = Asked {
    repo: request.repo,
    affinity: request.affinity,
    command_key: request.command_key,
    expected: request.expected,
    passed_wall: request.passed_wall,
    group,
  };
  let estimated: Vec<String> = place(&sight, &asked).0.into_iter().map(|entry| entry.ticket.run).collect();
  let joined: Joined = join(params, request).unwrap();
  assert!(matches!(joined.answer, TurnAnswer::Queued(_)), "{}", request.run);
  (estimated, ahead_in(dir, request.run), crosses)
}

/// The runs before the run `run` in the queue of `dir`, or `None` when it is not there
fn ahead_in(dir: &Path, run: &str) -> Option<Vec<String>> {
  let queue = read_live(dir).unwrap().queue;
  let at = queue.iter().position(|entry| entry.ticket.run == run)?;
  Some(queue[..at].iter().map(|entry| entry.ticket.run.clone()).collect())
}

/// The estimate places a run where the join puts it, over generated queues: runs of groups and of checkouts without
/// one, short runs, exclusive runs at the head, supersedes of a run of the same and of another group, runs that leave,
/// and runs at their budget of passes, behind a holder of the whole host. Both read one rule (`host_turn::join_queue`),
/// so no third copy of it can drift unnoticed
#[test]
fn the_estimate_places_a_run_where_the_join_puts_it_over_generated_queues() {
  let dir = tempfile::tempdir().unwrap();
  let params = TurnParams {
    dir: dir.path(),
    lock_timeout: Duration::from_secs(10),
    recheck: crate::host_turn::RECHECK,
    cores: 20,
    load_bound: LOAD_BOUND,
    drain_bar: DRAIN_BAR,
  };
  let pid = std::process::id();
  let started = sync_utils::process_scan::process_started(pid);
  let commands = [vec!["cargo".to_owned(), "nextest".to_owned()], vec!["pnpm".to_owned(), "e2e".to_owned()]];
  let keys = ["key-of-nextest", "key-of-e2e"];
  let groups = ["session", "other", "third"];
  let checkouts: Vec<String> = (0..16).map(|n| format!("checkout-{n:02}")).collect();
  let checkouts: &'static [String] = Box::leak(checkouts.into_boxed_slice());
  let runs: Vec<String> = (0..250).map(|n| format!("run-{n}")).collect();
  let request = |run: &'static str, draw: &mut Draw| {
    let command = usize::try_from(draw.below(2)).unwrap();
    let short = draw.below(5) == 0;
    TurnRequest {
      run,
      pid,
      process_started: started,
      repo: REPO,
      command: &commands[command],
      affinity: checkouts[usize::try_from(draw.below(16)).unwrap()].as_str(),
      command_key: keys[command],
      expected: short.then_some(Cores(40)),
      exclusive: draw.below(20) == 0,
      passed_wall: short.then_some(Duration::from_secs(50)),
      measure_as: None,
      group: match draw.below(6) {
        0..=2 => Some(groups[0]),
        3 => Some(groups[1]),
        4 => Some(groups[2]),
        _ => None,
      },
    }
  };
  let holder = TurnRequest {
    run: "holder",
    pid,
    process_started: started,
    repo: REPO,
    command: &commands[0],
    affinity: "checkout-holder",
    command_key: "key-of-holder",
    expected: None,
    exclusive: false,
    passed_wall: None,
    measure_as: None,
    group: None,
  };
  assert_eq!(join(&params, &holder).unwrap().answer, TurnAnswer::Taken);
  // the case of the review: x went ahead of big-2, which seven more runs passed, and a run of x's checkout and command
  // of another group supersedes it, so big-2 may be passed again
  let of_group = |run: &'static str, affinity: &'static str, group: &'static str| TurnRequest {
    run,
    affinity,
    group: Some(group),
    ..holder
  };
  for (run, affinity, group) in [("big-1", "checkout-big-1", "big"), ("big-2", "checkout-big-2", "big"), ("x", "checkout-x", "gx1")] {
    join(&params, &of_group(run, affinity, group)).unwrap();
  }
  for run in ["o0", "o1", "o2", "o3", "o4", "o5", "o6"] {
    join(&params, &of_group(run, run, run)).unwrap();
  }
  let (estimated, joined, crosses) = estimate_then_join(&params, &of_group("x2", "checkout-x", "gx2"));
  assert!(crosses, "the case is the one of the review");
  assert_eq!(Some(estimated), joined, "the estimate and the join place x2 alike");
  for run in read_live(dir.path()).unwrap().queue {
    leave(&params, &run.ticket.run).unwrap();
  }
  let runs: &'static [String] = Box::leak(runs.into_boxed_slice());
  let mut draw = Draw(0x9e37_79b9_7f4a_7c15);
  let mut compared = 0;
  let mut crossed = 0;
  let mut most = 0;
  for run in runs {
    let queue = read_live(dir.path()).unwrap().queue;
    if !queue.is_empty() && draw.below(12) == 0 {
      let gone = &queue[usize::try_from(draw.below(queue.len() as u64)).unwrap()].ticket.run;
      leave(&params, gone).unwrap();
    }
    let asked_request = request(run.as_str(), &mut draw);
    if asked_request.exclusive {
      // an estimate is of a run that is not exclusive
      join(&params, &asked_request).unwrap();
    } else {
      let (estimated, joined, crosses) = estimate_then_join(&params, &asked_request);
      assert_eq!(joined, Some(estimated), "{run}");
      crossed += usize::from(crosses);
      compared += 1;
    }
    let passes = read_groups(dir.path()).0.passes_in(&read_live(dir.path()).unwrap().queue);
    most = passes.values().copied().max().unwrap_or_default().max(most);
  }
  assert!(compared > 180, "{compared}");
  assert!(crossed > 0, "the generator made the supersede of a run of another group that passed a run at its budget");
  assert_eq!(most, TURN_PASS_BUDGET, "some run reached its budget, and none went past it");
}
