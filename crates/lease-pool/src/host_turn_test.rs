use super::*;
use pretty_assertions::assert_eq;
use sync_utils::cores::Cores;

pub(crate) fn params(dir: &Path) -> TurnParams<'_> {
  TurnParams {
    dir,
    lock_timeout: Duration::from_secs(10),
    recheck: RECHECK,
    cores: 20,
    load_bound: LOAD_BOUND,
    drain_bar: DRAIN_BAR,
  }
}

/// The command of each request of the tests
pub(crate) fn command() -> Vec<String> {
  vec!["cargo".to_owned(), "nextest".to_owned(), "run".to_owned(), "--workspace".to_owned()]
}

/// The checkout of the requests of the tests
const CHECKOUT: &str = "checkout-0123456789ab";

/// A request of this process, whose pid lives as long as the test, from a checkout of its own (the run id), so no run
/// supersedes another
pub(crate) fn request<'a>(run: &'a str, command: &'a [String]) -> TurnRequest<'a> {
  let pid = std::process::id();
  TurnRequest {
    commit: "",
    measure_key: "",
    priority: None,
    expectation: None,
    run,
    pid,
    process_started: sync_utils::process_scan::process_started(pid),
    repo: "/Users/u/projects/branch-deck",
    command,
    affinity: run,
    command_key: if command == ["pnpm", "e2e"] { "key-of-e2e" } else { "key-of-nextest" },
    expected: None,
    exclusive: false,
    passed_wall: None,
    measure_as: None,
    group: None,
  }
}

/// An exclusive request as `request`: a measurement that needs a quiet host
fn measurement<'a>(run: &'a str, command: &'a [String]) -> TurnRequest<'a> {
  TurnRequest {
    exclusive: true,
    ..request(run, command)
  }
}

/// A request as `request` from `CHECKOUT`, the checkout of every run that asks with it
fn of_checkout<'a>(run: &'a str, command: &'a [String]) -> TurnRequest<'a> {
  TurnRequest {
    affinity: CHECKOUT,
    ..request(run, command)
  }
}

/// The holder and the runs of the queue
fn runs(table: &HeavyTurn) -> (Option<&str>, Vec<&str>) {
  (
    table.holders.first().map(|holder| holder.ticket.run.as_str()),
    table.queue.iter().map(|entry| entry.ticket.run.as_str()).collect(),
  )
}

/// The holder and the runs ahead of a queued answer
fn queued(answer: &TurnAnswer) -> (Option<&str>, Vec<&str>) {
  match answer {
    TurnAnswer::Queued(queue) => (
      queue.holders.first().map(|holder| holder.ticket.run.as_str()),
      queue.ahead.iter().map(|entry| entry.ticket.run.as_str()).collect(),
    ),
    TurnAnswer::Taken => panic!("the run took the turn"),
    TurnAnswer::Passed { exclusive } => panic!("the run took the turn past {}", exclusive.ticket.run),
    TurnAnswer::Superseded { by } => panic!("{by} superseded the run"),
  }
}

/// run-1 takes the turn, then run-2 and run-3 wait for it in their order
fn three_in_line(params: &TurnParams<'_>, command: &[String]) {
  assert_eq!(take(params, &request("run-1", command)).unwrap(), TurnAnswer::Taken);
  assert_eq!(queued(&take(params, &request("run-2", command)).unwrap()), (Some("run-1"), vec![]));
  assert_eq!(queued(&take(params, &request("run-3", command)).unwrap()), (Some("run-1"), vec!["run-2"]));
}

/// One heavy run holds the turn, and the others wait for it first come first: a run of the queue never takes a turn that
/// a run before it has yet to take, a repeat of the holder holds it still, and the run that leaves gives it to the next
#[test]
fn heavy_runs_take_the_turn_one_at_a_time_first_come_first() {
  let dir = tempfile::tempdir().unwrap();
  let params = params(dir.path());
  let command = command();
  three_in_line(&params, &command);
  assert_eq!(take(&params, &request("run-1", &command)).unwrap(), TurnAnswer::Taken, "a repeat of the holder");
  let table = read_live(dir.path()).unwrap();
  assert_eq!(runs(&table), (Some("run-1"), vec!["run-2", "run-3"]));
  let holder = table.holders.into_iter().next().unwrap();
  assert_eq!(
    (holder.repo.as_str(), holder.command.as_str()),
    ("/Users/u/projects/branch-deck", "cargo nextest run --workspace")
  );

  assert!(leave(&params, "run-1").unwrap(), "the holder gives the turn back");
  assert_eq!(
    queued(&take(&params, &request("run-3", &command)).unwrap()),
    (None, vec!["run-2"]),
    "the first of the queue takes it first"
  );
  assert_eq!(take(&params, &request("run-2", &command)).unwrap(), TurnAnswer::Taken);
  assert!(leave(&params, "run-2").unwrap());
  assert_eq!(take(&params, &request("run-3", &command)).unwrap(), TurnAnswer::Taken);
}

/// A run of a class of measures (`brd-host run --measure-as`) names its class on its entry, as a holder and in the
/// queue, for the lines of the turn; a table of an older build has no class and reads as before
#[test]
fn a_run_of_a_class_names_its_class_on_its_entry() {
  let dir = tempfile::tempdir().unwrap();
  let params = params(dir.path());
  let command = command();
  let classed = |run| TurnRequest {
    measure_as: Some("accio-gate"),
    ..request(run, &command)
  };
  assert_eq!(take(&params, &classed("run-1")).unwrap(), TurnAnswer::Taken);
  assert_eq!(queued(&take(&params, &classed("run-2")).unwrap()), (Some("run-1"), vec![]));
  assert_eq!(queued(&take(&params, &request("run-3", &command)).unwrap()), (Some("run-1"), vec!["run-2"]));
  let table = read_live(dir.path()).unwrap();
  let classes = |entries: &[TurnEntry]| entries.iter().map(|entry| entry.measure_as.clone()).collect::<Vec<_>>();
  assert_eq!(classes(&table.holders), [Some("accio-gate".to_owned())]);
  assert_eq!(classes(&table.queue), [Some("accio-gate".to_owned()), None]);
  let old: TurnEntry = serde_json::from_str(r#"{"run":"1-2","pid":1,"repo":"/r","command":"pnpm e2e","since_ms":5}"#).unwrap();
  assert_eq!(old.measure_as, None);
  assert!(!serde_json::to_string(&old).unwrap().contains("measure_as"));
}

/// A run whose process is gone, or whose pid another process took, holds no turn and waits in no queue: the next run
/// takes the turn of a gone holder at its next look, a gone ticket ahead is passed over, and `read_live` shows neither
#[test]
fn a_gone_holder_frees_the_turn_and_a_gone_ticket_leaves_the_queue() {
  let dir = tempfile::tempdir().unwrap();
  let params = params(dir.path());
  let command = command();
  // the pid of this process with another start: the process of the run is gone, and the system gave its pid again
  let gone = |run| TurnRequest {
    process_started: Some(1),
    ..request(run, &command)
  };
  assert_eq!(take(&params, &gone("killed")).unwrap(), TurnAnswer::Taken, "a request asks no liveness of its own");
  assert_eq!(runs(&read_live(dir.path()).unwrap()), (None, vec![]), "a gone holder shows no turn");
  assert_eq!(take(&params, &request("run-2", &command)).unwrap(), TurnAnswer::Taken, "the turn of a gone holder is free");

  assert_eq!(queued(&take(&params, &gone("gone-ticket")).unwrap()), (Some("run-2"), vec![]));
  assert_eq!(
    queued(&take(&params, &request("run-3", &command)).unwrap()),
    (Some("run-2"), vec![]),
    "a gone ticket is not ahead"
  );
  assert!(leave(&params, "run-2").unwrap());
  assert_eq!(take(&params, &request("run-3", &command)).unwrap(), TurnAnswer::Taken);
}

/// A run that stops waiting (a cancel, the time up) leaves the queue, and the runs after it move up; a run that holds no
/// turn gives none back
#[test]
fn a_run_that_leaves_the_queue_waits_no_more() {
  let dir = tempfile::tempdir().unwrap();
  let params = params(dir.path());
  let command = command();
  three_in_line(&params, &command);
  assert!(!leave(&params, "run-2").unwrap(), "a ticket is no turn");
  assert_eq!(runs(&read_live(dir.path()).unwrap()), (Some("run-1"), vec!["run-3"]));
  assert_eq!(queued(&take(&params, &request("run-3", &command)).unwrap()), (Some("run-1"), vec![]));
}

/// A wait whose time is up answers the place in the queue with its ticket kept, which `leave` takes out
#[test]
fn a_wait_whose_time_is_up_answers_its_place_and_keeps_its_ticket() {
  let dir = tempfile::tempdir().unwrap();
  let params = params(dir.path());
  let command = command();
  let watched = watch(&params);
  assert_eq!(take(&params, &request("run-1", &command)).unwrap(), TurnAnswer::Taken);
  let answer = take_waiting(&params, &request("run-2", &command), Duration::ZERO, &watched).unwrap();
  assert_eq!(queued(&answer), (Some("run-1"), vec![]), "the time is up");
  assert_eq!(runs(&read_live(dir.path()).unwrap()), (Some("run-1"), vec!["run-2"]), "the ticket stays");
  assert!(!leave(&params, "run-2").unwrap(), "the ticket leaves");
}

/// `waiting`, a wait for the turn of run-2 started in a thread of its own, takes the turn after run-1, which holds it,
/// leaves once the first look of the wait queued run-2
fn takes_the_turn_once_the_holder_leaves(params: &TurnParams<'_>, waiting: impl FnOnce() -> TurnAnswer + Send) -> TurnAnswer {
  let command = command();
  assert_eq!(take(params, &request("run-1", &command)).unwrap(), TurnAnswer::Taken);
  std::thread::scope(|scope| {
    let waiting = scope.spawn(waiting);
    while runs(&read_live(params.dir).unwrap()).1 != ["run-2"] && !waiting.is_finished() {
      std::thread::yield_now();
    }
    assert!(leave(params, "run-1").unwrap());
    waiting.join().unwrap()
  })
}

/// A wait for the turn ends as the holder gives it back: woken by the file event of the table, else at its next look
/// (`RECHECK` of the agent's waits). The event is the way, the look the bound: `FSEvents` of macOS delays and drops events
/// under load (at load 100 a wait saw no event of the table for 30 s while the events of another file of the directory
/// came a second late), and this test waited for the event alone, with a look an hour off, until its timeout
#[test]
fn a_waiting_run_takes_the_turn_once_the_holder_leaves() {
  let dir = tempfile::tempdir().unwrap();
  let params = params(dir.path());
  let command = command();
  let waited = takes_the_turn_once_the_holder_leaves(&params, || {
    let watched = watch(&params);
    take_waiting(&params, &request("run-2", &command), Duration::from_hours(1), &watched).unwrap()
  });
  assert_eq!(waited, TurnAnswer::Taken);
}

/// A wait whose file event never comes, a watch of another directory, takes the turn at its next look after the holder
/// left: no event of the system is needed for a wait to end
#[test]
fn a_wait_without_the_event_takes_the_turn_at_its_next_look() {
  let dir = tempfile::tempdir().unwrap();
  let elsewhere = tempfile::tempdir().unwrap();
  let looking = TurnParams {
    recheck: Duration::from_millis(50),
    ..params(dir.path())
  };
  let deaf = params(elsewhere.path());
  let command = command();
  let waited = takes_the_turn_once_the_holder_leaves(&looking, || {
    let watched = watch(&deaf);
    take_waiting(&looking, &request("run-2", &command), Duration::from_hours(1), &watched).unwrap()
  });
  assert_eq!(waited, TurnAnswer::Taken);
}

/// The time of a holder is from the moment it took the turn, not from the one it joined the queue: a run that waited an
/// hour holds it for seconds
#[test]
fn a_holder_holds_the_turn_from_the_moment_it_took_it() {
  let dir = tempfile::tempdir().unwrap();
  let params = params(dir.path());
  let command = command();
  let waited_long = request("run-1", &command);
  let joined = TurnEntry {
    expectation: None,
    ticket: Ticket {
      run: "run-1".to_owned(),
      pid: waited_long.pid,
      process_started: waited_long.process_started,
    },
    repo: waited_long.repo.to_owned(),
    command: "cargo nextest run --workspace".to_owned(),
    since_ms: 1,
    affinity: None,
    command_key: None,
    expected: None,
    live: None,
    exclusive: false,
    passed: None,
    measure_as: None,
    recent: None,
    group: None,
  };
  write_table(dir.path(), &serde_json::json!({ "schema": 2, "queue": [joined] }));
  let before = now_ms();
  assert_eq!(take(&params, &waited_long).unwrap(), TurnAnswer::Taken);
  let holder = read_live(dir.path()).unwrap().holders.into_iter().next().unwrap();
  assert!(holder.since_ms >= before, "{} is the time it joined the queue", holder.since_ms);
}

/// The runs of the queue of `table`, each with the run that superseded it
fn superseded(table: &HeavyTurn) -> Vec<(&str, &str)> {
  table.superseded.iter().map(|entry| (entry.ticket.run.as_str(), entry.by.as_str())).collect()
}

/// A later run of the same checkout and command supersedes the run of the queue that waits with them: it takes its
/// place, and the next look of the superseded run answers the later one once and takes it out of the table
#[test]
fn a_later_run_of_the_same_checkout_and_command_supersedes_the_waiting_one() {
  let dir = tempfile::tempdir().unwrap();
  let params = params(dir.path());
  let (nextest, e2e) = (command(), vec!["pnpm".to_owned(), "e2e".to_owned()]);
  assert_eq!(take(&params, &of_checkout("holder", &nextest)).unwrap(), TurnAnswer::Taken);
  assert_eq!(queued(&take(&params, &of_checkout("old", &e2e)).unwrap()), (Some("holder"), vec![]));
  assert_eq!(queued(&take(&params, &of_checkout("other", &nextest)).unwrap()), (Some("holder"), vec!["old"]));

  let joined = join(&params, &of_checkout("new", &e2e)).unwrap();
  assert_eq!(queued(&joined.answer), (Some("holder"), vec![]), "the later run takes the place of the superseded one");
  let runs_of = |entries: &[TurnEntry]| entries.iter().map(|entry| entry.ticket.run.clone()).collect::<Vec<_>>();
  assert_eq!(runs_of(&joined.replaced), ["old"]);
  assert_eq!(runs_of(&joined.beside), ["other"], "a run of the checkout with another command waits beside it");
  let table = read_live(dir.path()).unwrap();
  assert_eq!(runs(&table), (Some("holder"), vec!["new", "other"]));
  assert_eq!(superseded(&table), [("old", "new")]);
  let new = &table.queue[0];
  assert_eq!((new.affinity.as_deref(), new.command_key.as_deref()), (Some(CHECKOUT), Some("key-of-e2e")));

  assert_eq!(take(&params, &of_checkout("old", &e2e)).unwrap(), TurnAnswer::Superseded { by: "new".to_owned() });
  assert_eq!(superseded(&read_live(dir.path()).unwrap()), [], "the answer takes it out");
  let repeat = join(&params, &of_checkout("new", &e2e)).unwrap();
  assert_eq!((repeat.replaced, repeat.beside), (vec![], vec![]), "a repeat of a queued run supersedes nothing");
}

/// Another command of the checkout, the same command of another checkout or of an older build without a key, and the
/// holder, whose command may run, are never superseded; a superseded run that leaves or whose process is gone is out
#[test]
fn another_command_another_checkout_and_the_holder_are_never_superseded() {
  let dir = tempfile::tempdir().unwrap();
  let params = params(dir.path());
  let command = command();
  assert_eq!(take(&params, &of_checkout("holder", &command)).unwrap(), TurnAnswer::Taken);
  let elsewhere = TurnRequest {
    affinity: "checkout-ba9876543210",
    ..of_checkout("elsewhere", &command)
  };
  assert_eq!(queued(&take(&params, &elsewhere).unwrap()), (Some("holder"), vec![]));
  let unnamed = TurnRequest {
    affinity: "",
    ..of_checkout("unnamed", &command)
  };
  assert_eq!(queued(&take(&params, &unnamed).unwrap()), (Some("holder"), vec!["elsewhere"]));
  let joined = join(&params, &of_checkout("same", &command)).unwrap();
  assert_eq!(queued(&joined.answer), (Some("holder"), vec!["elsewhere", "unnamed"]), "nobody is superseded");
  assert_eq!((joined.replaced, joined.beside), (vec![], vec![]));
  assert_eq!(superseded(&read_live(dir.path()).unwrap()), []);

  // an entry of an older build names no checkout and no key
  let mut table = read_live(dir.path()).unwrap();
  for entry in &mut table.queue {
    (entry.affinity, entry.command_key) = (None, None);
  }
  write_table(dir.path(), &serde_json::to_value(&table).unwrap());
  let later = join(&params, &of_checkout("later", &command)).unwrap();
  assert_eq!(later.replaced, vec![], "an entry without a key is no run of this command");

  let gone = TurnRequest {
    process_started: Some(1),
    ..of_checkout("gone", &command)
  };
  write_table(
    dir.path(),
    &serde_json::json!({ "schema": 2, "queue": [], "superseded": [{ "run": "gone", "pid": gone.pid, "process_started": 1, "by": "later" }] }),
  );
  assert_eq!(superseded(&read_live(dir.path()).unwrap()), [], "a gone process is no superseded run");
  write_table(
    dir.path(),
    &serde_json::json!({ "schema": 2, "queue": [], "superseded": [{ "run": "left", "pid": gone.pid, "by": "later" }] }),
  );
  assert!(!leave(&params, "left").unwrap());
  assert_eq!(superseded(&read_live(dir.path()).unwrap()), [], "the end of a superseded run takes it out");
}

/// A request as `request` whose command measured `tenths` tenths of cores
fn measured<'a>(run: &'a str, command: &'a [String], tenths: u32) -> TurnRequest<'a> {
  TurnRequest {
    expected: Some(Cores(tenths)),
    ..request(run, command)
  }
}

/// The holders of a table
fn holders(table: &HeavyTurn) -> Vec<&str> {
  table.holders.iter().map(|holder| holder.ticket.run.as_str()).collect()
}

/// Heavy runs of a measure share the cores of the host while their expectations fit: the third waits until a holder
/// leaves, and its wait names what each holder holds and why
#[test]
fn heavy_runs_of_a_measure_share_the_cores_while_they_fit() {
  let dir = tempfile::tempdir().unwrap();
  let params = params(dir.path());
  let command = command();
  assert_eq!(take(&params, &measured("a", &command, 80)).unwrap(), TurnAnswer::Taken);
  assert_eq!(take(&params, &measured("b", &command, 100)).unwrap(), TurnAnswer::Taken, "8 and 10 of 20");
  let TurnAnswer::Queued(queue) = take(&params, &measured("c", &command, 40)).unwrap() else {
    panic!("c fits");
  };
  assert_eq!(queue.need, Need::Cores { cores: Cores(40), live: false });
  assert_eq!(
    queue.holders.iter().map(|holder| holder.need(20, now_ms())).collect::<Vec<_>>(),
    [Need::Cores { cores: Cores(80), live: false }, Need::Cores { cores: Cores(100), live: false }]
  );
  assert_eq!(holders(&read_live(dir.path()).unwrap()), ["a", "b"]);
  assert!(leave(&params, "a").unwrap());
  assert_eq!(take(&params, &measured("c", &command, 40)).unwrap(), TurnAnswer::Taken, "10 and 4 of 20");
  assert_eq!(holders(&read_live(dir.path()).unwrap()), ["b", "c"]);
}

/// A run without a measure of its command, or one that measured the whole host, waits for every holder to leave, and no
/// run joins it; the head of the queue blocks the runs after it, which would fit, so none overtakes a larger one
#[test]
fn a_run_without_a_measure_takes_the_whole_host_and_the_head_of_the_queue_goes_first() {
  let dir = tempfile::tempdir().unwrap();
  let params = params(dir.path());
  let command = command();
  assert_eq!(take(&params, &measured("small", &command, 40)).unwrap(), TurnAnswer::Taken);
  let TurnAnswer::Queued(unknown) = take(&params, &request("unknown", &command)).unwrap() else {
    panic!("a run without a measure waits for the holder");
  };
  assert_eq!(unknown.need, Need::Unmeasured);
  assert_eq!(queued(&take(&params, &measured("whole", &command, 200)).unwrap()), (Some("small"), vec!["unknown"]));
  assert_eq!(
    queued(&take(&params, &measured("tiny", &command, 10)).unwrap()),
    (Some("small"), vec!["unknown", "whole"]),
    "it would fit, and waits its turn"
  );
  assert!(leave(&params, "small").unwrap());
  assert_eq!(take(&params, &request("unknown", &command)).unwrap(), TurnAnswer::Taken);
  let TurnAnswer::Queued(whole) = take(&params, &measured("whole", &command, 200)).unwrap() else {
    panic!("a run of the whole host waits");
  };
  assert_eq!((whole.need, whole.need.is_whole_host()), (Need::Measured(Cores(200)), true));
  assert!(leave(&params, "unknown").unwrap());
  assert_eq!(take(&params, &measured("whole", &command, 200)).unwrap(), TurnAnswer::Taken);
  assert_eq!(
    queued(&take(&params, &measured("tiny", &command, 10)).unwrap()),
    (Some("whole"), vec![]),
    "no run joins the whole host"
  );
}

/// A holder holds the larger of its expectation and its live use, and the live cores of the runs that are not heavy
/// count too: a run that fits by the expectations alone waits until they drop
#[test]
fn the_live_use_of_a_holder_and_of_the_runs_that_are_not_heavy_count() {
  let dir = tempfile::tempdir().unwrap();
  let params = params(dir.path());
  let command = command();
  let note = |run: &str, tenths: u32| {
    let pid = std::process::id();
    note_use(
      &params,
      &UseNote {
        run,
        pid,
        process_started: sync_utils::process_scan::process_started(pid),
        repo: "/Users/u/projects/idea-1",
        command: &command,
        live: Cores(tenths),
      },
    )
    .unwrap();
  };
  assert_eq!(take(&params, &measured("a", &command, 80)).unwrap(), TurnAnswer::Taken);
  note("a", 140);
  let TurnAnswer::Queued(queue) = take(&params, &measured("b", &command, 80)).unwrap() else {
    panic!("14 and 8 do not fit 20");
  };
  assert_eq!(queue.holders[0].need(20, now_ms()), Need::Cores { cores: Cores(140), live: true });
  note("a", 60);
  assert_eq!(take(&params, &measured("b", &command, 80)).unwrap(), TurnAnswer::Taken, "a holds its 8 again: 8 and 8");
  assert!(leave(&params, "b").unwrap());
  note("light", 50);
  let table = read_live(dir.path()).unwrap();
  assert_eq!((table.light.len(), table.light_cores()), (1, Cores(50)), "a run that neither holds nor waits is not heavy");
  let TurnAnswer::Queued(queue) = take(&params, &measured("b", &command, 80)).unwrap() else {
    panic!("8, 5 that are not heavy and 8 do not fit 20");
  };
  assert_eq!(queue.light, Cores(50));
  assert!(!leave(&params, "light").unwrap());
  assert_eq!(take(&params, &measured("b", &command, 80)).unwrap(), TurnAnswer::Taken);
}

/// The busy cores of the host outside the live use of the holders hold a heavy run back, whatever their processes: a run
/// of a measure waits while it does not fit beside them, a run that takes the whole host waits for a quiet host, and a
/// sample older than 10 s counts as none
#[test]
fn the_busy_cores_of_the_host_outside_the_holders_hold_a_heavy_run_back() {
  let dir = tempfile::tempdir().unwrap();
  let params = params(dir.path());
  let command = command();
  note_host(dir.path(), Cores(150)).unwrap();
  let TurnAnswer::Queued(queue) = take(&params, &measured("a", &command, 80)).unwrap() else {
    panic!("15 busy and 8 do not fit 20");
  };
  assert_eq!(queue.outside, Some(Cores(150)));
  assert!(
    matches!(take(&params, &request("whole", &command)).unwrap(), TurnAnswer::Queued(_)),
    "a busy host is no quiet one"
  );
  assert!(leave(&params, "a").unwrap() || !holders(&read_live(dir.path()).unwrap()).contains(&"a"));
  note_host(dir.path(), quiet(20)).unwrap();
  assert_eq!(quiet(20), Cores(40));
  assert_eq!(take(&params, &request("whole", &command)).unwrap(), TurnAnswer::Taken, "4.0 busy is quiet on 20");
  assert!(leave(&params, "whole").unwrap());

  assert_eq!(take(&params, &measured("a", &command, 80)).unwrap(), TurnAnswer::Taken);
  let pid = std::process::id();
  note_use(
    &params,
    &UseNote {
      run: "a",
      pid,
      process_started: sync_utils::process_scan::process_started(pid),
      repo: "/Users/u/projects/branch-deck",
      command: &command,
      live: Cores(60),
    },
  )
  .unwrap();
  note_host(dir.path(), Cores(160)).unwrap();
  let TurnAnswer::Queued(queue) = take(&params, &measured("b", &command, 40)).unwrap() else {
    panic!("8 of a, 10 busy outside its 6 and 4 do not fit 20");
  };
  assert_eq!(queue.outside, Some(Cores(100)));
  note_host(dir.path(), Cores(100)).unwrap();
  assert_eq!(take(&params, &measured("b", &command, 40)).unwrap(), TurnAnswer::Taken, "8 of a, 4 outside and 4");
  assert_eq!(host_busy(dir.path(), now_ms() + HOST_FRESH_MS + 1_000), None, "an old sample counts as none");
}

/// An exclusive run takes the whole host of a quiet host: it waits for every holder and for a quiet host, and while it
/// holds no heavy run is admitted, a run of a measure that would fit neither; the runs that are not heavy find it
#[test]
fn an_exclusive_run_waits_for_a_quiet_host_and_holds_it_alone() {
  let dir = tempfile::tempdir().unwrap();
  let params = params(dir.path());
  let command = command();
  let exclusive = |run| TurnRequest {
    exclusive: true,
    ..measured(run, &command, 40)
  };
  assert_eq!(take(&params, &measured("small", &command, 40)).unwrap(), TurnAnswer::Taken);
  let TurnAnswer::Queued(queue) = take(&params, &exclusive("ab")).unwrap() else {
    panic!("an exclusive run waits for every holder");
  };
  assert_eq!(queue.need, Need::Exclusive);
  assert!(leave(&params, "small").unwrap());
  note_host(dir.path(), Cores(60)).unwrap();
  assert!(matches!(take(&params, &exclusive("ab")).unwrap(), TurnAnswer::Queued(_)), "6.0 busy is no quiet host");
  quiet_for_30_s(dir.path(), Cores(30));
  assert_eq!(take(&params, &exclusive("ab")).unwrap(), TurnAnswer::Taken);
  assert_eq!(exclusive_holder(dir.path()).map(|holder| holder.ticket.run), Some("ab".to_owned()));
  assert!(
    matches!(take(&params, &measured("tiny", &command, 10)).unwrap(), TurnAnswer::Queued(_)),
    "no heavy run joins it"
  );
  assert!(leave(&params, "ab").unwrap());
  assert_eq!(exclusive_holder(dir.path()), None);
  assert_eq!(take(&params, &measured("tiny", &command, 10)).unwrap(), TurnAnswer::Taken);
}

/// Samples of the host of 30 s, each `busy`, as `serve` keeps them
fn quiet_for_30_s(dir: &Path, busy: Cores) {
  let now = now_ms();
  let mut window = HostWindow::default();
  for at in (0..=30_000).step_by(2_000) {
    window.push(now - 30_000 + at, busy);
  }
  note_host_window(dir, &window).unwrap();
}

/// An exclusive run waits until no run that is not heavy is in its command, of any repository, and its line names each;
/// then it waits until every sample of the last 30 s is quiet: a lull of one sample of a bursty run does not admit it,
/// nor a window that spans less than 30 s
#[test]
fn an_exclusive_run_waits_for_the_runs_in_their_command_and_a_quiet_window() {
  let dir = tempfile::tempdir().unwrap();
  let params = params(dir.path());
  let command = command();
  bench(&params, "bench");
  quiet_for_30_s(dir.path(), Cores(10));
  let exclusive = measurement("ab", &command);
  let TurnAnswer::Queued(queue) = take(&params, &exclusive).unwrap() else {
    panic!("a run in its command holds an exclusive run back, at 0 cores in a lull too");
  };
  assert_eq!(queue.light_runs.iter().map(|light| light.ticket.run.as_str()).collect::<Vec<_>>(), ["bench"]);
  assert!(!leave(&params, "bench").unwrap());

  let now = now_ms();
  let mut lull = HostWindow::default();
  for (at, busy) in [(now - 30_000, 110), (now - 20_000, 105), (now - 10_000, 98), (now, 10)] {
    lull.push(at, Cores(busy));
  }
  note_host_window(dir.path(), &lull).unwrap();
  let TurnAnswer::Queued(queue) = take(&params, &exclusive).unwrap() else {
    panic!("a lull of one sample is no quiet host");
  };
  assert_eq!(queue.window, Some(Cores(110)));
  let mut short = HostWindow::default();
  short.push(now - 10_000, Cores(10));
  short.push(now, Cores(10));
  note_host_window(dir.path(), &short).unwrap();
  assert!(matches!(take(&params, &exclusive).unwrap(), TurnAnswer::Queued(_)), "10 s of samples tell no quiet yet");
  quiet_for_30_s(dir.path(), Cores(10));
  assert_eq!(take(&params, &exclusive).unwrap(), TurnAnswer::Taken);
}

/// An exclusive run at the head of the queue with no holder drains the host: the new runs that are not heavy wait for
/// it, for at most `DRAIN_BAR` from the start of the drain, then they start again while it waits on; once admitted it
/// holds them back to its end. Behind a heavy holder it drains nothing
#[test]
fn a_draining_exclusive_run_bars_new_runs_for_at_most_the_bound() {
  let dir = tempfile::tempdir().unwrap();
  let params = params(dir.path());
  let command = command();
  assert_eq!(take(&params, &measured("heavy", &command, 40)).unwrap(), TurnAnswer::Taken);
  let exclusive = measurement("ab", &command);
  assert!(matches!(take(&params, &exclusive).unwrap(), TurnAnswer::Queued(_)));
  assert_eq!(bar(dir.path(), now_ms(), DRAIN_BAR), None, "behind a heavy holder it drains nothing");
  assert!(leave(&params, "heavy").unwrap());

  bench(&params, "bench");
  let before = now_ms();
  assert!(matches!(take(&params, &exclusive).unwrap(), TurnAnswer::Queued(_)));
  let Some(Bar::Drains { entry, since_ms, until_ms }) = bar(dir.path(), now_ms(), DRAIN_BAR) else {
    panic!("the exclusive run drains the host");
  };
  assert_eq!(entry.ticket.run, "ab");
  assert!(since_ms >= before && until_ms == since_ms + 300_000, "{since_ms} {until_ms}");
  assert!(matches!(take(&params, &exclusive).unwrap(), TurnAnswer::Queued(_)));
  assert!(
    matches!(bar(dir.path(), now_ms(), DRAIN_BAR), Some(Bar::Drains { since_ms: again, .. }) if again == since_ms),
    "a look keeps its start"
  );
  assert_eq!(bar(dir.path(), until_ms, DRAIN_BAR), None, "the bar lifts after the bound, and the run waits on");

  assert!(!leave(&params, "bench").unwrap());
  quiet_for_30_s(dir.path(), Cores(10));
  assert_eq!(take(&params, &exclusive).unwrap(), TurnAnswer::Taken);
  assert!(
    matches!(bar(dir.path(), until_ms + 1, DRAIN_BAR), Some(Bar::Holds(holder)) if holder.ticket.run == "ab"),
    "a hold bars to its end"
  );
  assert!(!dir.path().join(DRAIN_FILE).exists());
  assert!(leave(&params, "ab").unwrap());
  assert_eq!(bar(dir.path(), now_ms(), DRAIN_BAR), None);
}

/// Notes `run`, a run that is not heavy, in its command at 0 cores: it holds an exclusive run back, in a lull too
fn bench(params: &TurnParams<'_>, run: &str) {
  let pid = std::process::id();
  note_use(
    params,
    &UseNote {
      run,
      pid,
      process_started: sync_utils::process_scan::process_started(pid),
      repo: "/Users/u/projects/accio",
      command: &["scripts/bench".to_owned(), "ab-run".to_owned()],
      live: Cores(0),
    },
  )
  .unwrap();
}

/// Moves the start of the drain in `dir` back by `ago`, as the time that a drain takes
fn drain_began_ago(dir: &Path, ago: Duration) {
  let path = dir.join(DRAIN_FILE);
  let mut drain: Drain = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
  drain.drain_since_ms -= millis(ago);
  std::fs::write(&path, serde_json::to_vec(&drain).unwrap()).unwrap();
}

/// An exclusive run at the head of the queue that cannot start holds every heavy run back while it drains the host. Once
/// its drain has ended without a quiet host, the first heavy run behind it that fits starts past it, by its own look,
/// and the holder names it; the exclusive run keeps its place, its drain never starts again, and it starts once the host
/// is quiet. While the head can start, no run passes it
#[test]
fn a_heavy_run_that_fits_starts_past_an_exclusive_run_whose_drain_ended() {
  let command = command();
  let dir = tempfile::tempdir().unwrap();
  let params = params(dir.path());
  bench(&params, "bench");
  let exclusive = measurement("ab", &command);
  let TurnAnswer::Queued(head) = take(&params, &exclusive).unwrap() else {
    panic!("a run in its command holds it back");
  };
  assert!(matches!(head.head_drain, Some(HeadDrain::Draining { .. })), "{:?}", head.head_drain);
  let TurnAnswer::Queued(behind) = take(&params, &measured("small", &command, 40)).unwrap() else {
    panic!("no run passes it while it drains");
  };
  assert!(matches!(behind.head_drain, Some(HeadDrain::Draining { .. })), "{:?}", behind.head_drain);

  drain_began_ago(dir.path(), DRAIN_BAR + Duration::from_mins(1));
  let TurnAnswer::Passed { exclusive: passed } = look(&params, &measured("small", &command, 40)).unwrap() else {
    panic!("4 of 20 fit beside the runs that are not heavy");
  };
  assert_eq!(passed.ticket.run, "ab");
  let table = read_live(dir.path()).unwrap();
  assert_eq!(runs(&table), (Some("small"), vec!["ab"]), "the exclusive run keeps its place at the head");
  assert_eq!(table.holders[0].passed.as_deref(), Some("ab"));
  let TurnAnswer::Queued(head) = take(&params, &exclusive).unwrap() else {
    panic!("a holder holds it back");
  };
  let ended = head.head_drain;
  assert!(matches!(ended, Some(HeadDrain::Ended { .. })), "{ended:?}");

  assert!(leave(&params, "small").unwrap());
  let TurnAnswer::Queued(head) = take(&params, &exclusive).unwrap() else {
    panic!("the run in its command holds it back");
  };
  assert_eq!(head.head_drain, ended, "no holder again, and its drain does not start again");
  assert!(!leave(&params, "bench").unwrap());
  quiet_for_30_s(dir.path(), Cores(10));
  assert_eq!(
    queued(&take(&params, &measured("next", &command, 40)).unwrap()),
    (None, vec!["ab"]),
    "the head can start, and starts first"
  );
  assert_eq!(take(&params, &exclusive).unwrap(), TurnAnswer::Taken);
}

/// No run passes an exclusive run at the head of the queue behind a heavy holder, where its drain has not begun, nor
/// while it drains: the drain is its window of the host. After its drain only the first run behind it that is not
/// exclusive can pass, and only while the ordinary admission admits it: a run that does not fit beside the load waits,
/// a later smaller one waits behind it, and once the first has waited the load bound it starts past the head
#[test]
fn no_run_passes_an_exclusive_run_before_its_drain_ends_nor_one_that_does_not_fit() {
  let command = command();
  let dir = tempfile::tempdir().unwrap();
  let params = params(dir.path());
  let exclusive = measurement("ab", &command);
  assert_eq!(take(&params, &measured("heavy", &command, 40)).unwrap(), TurnAnswer::Taken);
  bench(&params, "bench");
  let TurnAnswer::Queued(head) = take(&params, &exclusive).unwrap() else {
    panic!("an exclusive run waits for every holder");
  };
  assert_eq!(head.head_drain, Some(HeadDrain::Pending));
  let TurnAnswer::Queued(behind) = take(&params, &measured("big", &command, 20)).unwrap() else {
    panic!("2 fit beside 4, and no run passes a head whose drain has not begun");
  };
  assert_eq!((behind.head_drain, behind.ahead.len()), (Some(HeadDrain::Pending), 1));
  assert!(leave(&params, "heavy").unwrap());
  assert!(matches!(take(&params, &exclusive).unwrap(), TurnAnswer::Queued(_)));
  assert!(
    matches!(take(&params, &measured("big", &command, 20)).unwrap(), TurnAnswer::Queued(_)),
    "no run passes it while it drains"
  );

  drain_began_ago(dir.path(), DRAIN_BAR);
  note_host(dir.path(), Cores(190)).unwrap();
  let TurnAnswer::Queued(behind) = look(&params, &measured("big", &command, 20)).unwrap() else {
    panic!("19 busy and 2 do not fit 20");
  };
  assert!(matches!(behind.head_drain, Some(HeadDrain::Ended { .. })), "{:?}", behind.head_drain);
  assert_eq!(
    queued(&take(&params, &measured("tiny", &command, 5)).unwrap()),
    (None, vec!["ab", "big"]),
    "it would fit, and waits behind the first run that is not exclusive"
  );
  let waited_out = TurnParams {
    load_bound: Duration::ZERO,
    ..params
  };
  assert!(
    matches!(take(&waited_out, &measured("big", &command, 20)).unwrap(), TurnAnswer::Passed { .. }),
    "past the load bound on a free turn, whatever the load"
  );
  assert_eq!(runs(&read_live(dir.path()).unwrap()), (Some("big"), vec!["ab", "tiny"]));
}

/// The table of the turn is private, as every file of the agent of a host
#[cfg(unix)]
#[test]
fn the_table_of_the_turn_is_private() {
  use std::os::unix::fs::PermissionsExt;
  let dir = tempfile::tempdir().unwrap();
  let params = params(dir.path());
  let command = command();
  assert_eq!(take(&params, &request("run-1", &command)).unwrap(), TurnAnswer::Taken);
  let mode = std::fs::metadata(dir.path().join(STATE_FILE)).unwrap().permissions().mode() & 0o777;
  assert_eq!(mode, 0o600);
}

/// The window of the samples of the host keeps the last 30 s
#[test]
fn the_window_of_the_host_keeps_the_samples_of_the_last_30_s() {
  let mut window = HostWindow::default();
  for (at, busy) in [(0, 150), (10_000, 20), (31_000, 30), (40_000, 25)] {
    window.push(at, Cores(busy));
  }
  let sample = window.sample().unwrap();
  assert_eq!(
    (sample.busy, sample.at_ms, sample.window_max, sample.window_ms),
    (Cores(25), 40_000, Some(Cores(30)), 30_000)
  );
}

/// A supervisor of a build before `exclusive` rewrites the table at each sample of its run and drops the field it does
/// not know: the exclusive holder stays in `exclusive.json`, so the runs that are not heavy still find it and `read_live`
/// still names it, the next look of the run puts the field back, and its end takes the file away
#[test]
fn an_exclusive_holder_outlives_a_rewrite_of_the_table_by_an_older_build() {
  let dir = tempfile::tempdir().unwrap();
  let params = params(dir.path());
  let command = command();
  let exclusive = measurement("ab", &command);
  assert_eq!(take(&params, &exclusive).unwrap(), TurnAnswer::Taken);
  let older = |dir: &Path| {
    let mut table: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(dir.join(STATE_FILE)).unwrap()).unwrap();
    for holder in table["holders"].as_array_mut().unwrap() {
      holder.as_object_mut().unwrap().remove("exclusive");
    }
    write_table(dir, &table);
  };
  older(dir.path());
  assert_eq!(exclusive_holder(dir.path()).map(|holder| holder.ticket.run), Some("ab".to_owned()));
  assert!(read_live(dir.path()).unwrap().holders[0].exclusive, "brd-host runs names the measurement");
  assert_eq!(take(&params, &exclusive).unwrap(), TurnAnswer::Taken);
  let table: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(dir.path().join(STATE_FILE)).unwrap()).unwrap();
  assert_eq!(table["holders"][0]["exclusive"], serde_json::json!(true), "the look of the run puts the field back");
  assert!(leave(&params, "ab").unwrap());
  assert_eq!(exclusive_holder(dir.path()), None);
  assert!(!dir.path().join(EXCLUSIVE_FILE).exists());
}

/// The live holder of `heavy.json` of the build before holds the whole host here: no run of this build joins it, and
/// `read_live` shows it first; once it is gone, the turn admits again
#[test]
fn the_holder_of_the_build_before_holds_the_whole_host() {
  let dir = tempfile::tempdir().unwrap();
  let params = params(dir.path());
  let command = command();
  let pid = std::process::id();
  std::fs::write(
    dir.path().join(LEGACY_FILE),
    serde_json::json!({ "schema": 1, "holder": { "run": "old-build", "pid": pid, "repo": "/r/idea-1", "command": "bt --lane fast", "since_ms": 1 }, "queue": [] }).to_string(),
  )
  .unwrap();
  let TurnAnswer::Queued(queue) = take(&params, &measured("new", &command, 10)).unwrap() else {
    panic!("the holder of the build before holds the whole host");
  };
  assert_eq!((queue.holders[0].ticket.run.as_str(), queue.holders[0].need(20, now_ms())), ("old-build", Need::Unmeasured));
  assert_eq!(holders(&read_live(dir.path()).unwrap()), ["old-build"]);
  std::fs::write(dir.path().join(LEGACY_FILE), serde_json::json!({ "schema": 1, "queue": [] }).to_string()).unwrap();
  assert_eq!(take(&params, &measured("new", &command, 10)).unwrap(), TurnAnswer::Taken);
}

/// A table of another schema, which an agent of another build wrote, refuses every step, and so does one without a
/// schema: no supervisor reads a turn that it does not know as free
#[test]
fn a_table_of_another_schema_refuses() {
  let dir = tempfile::tempdir().unwrap();
  let params = params(dir.path());
  let command = command();
  for table in [serde_json::json!({ "schema": 3, "queue": [] }), serde_json::json!({ "queue": [] })] {
    write_table(dir.path(), &table);
    let refused = take(&params, &of_checkout("run-1", &command)).unwrap_err().to_string();
    assert!(
      refused.contains("turn.json has the schema ") && refused.contains(" of another build of brd-host, and this one reads 2"),
      "{refused}"
    );
    leave(&params, "run-1").unwrap_err();
    read_live(dir.path()).unwrap_err();
  }
  std::fs::remove_file(dir.path().join(STATE_FILE)).unwrap();
  assert_eq!(read_live(dir.path()).unwrap(), HeavyTurn::default(), "no table is the free turn of this schema");
}

/// Writes `heavy.json` as an agent of any build would
fn write_table(dir: &Path, table: &serde_json::Value) {
  std::fs::create_dir_all(dir).unwrap();
  std::fs::write(dir.join(STATE_FILE), table.to_string()).unwrap();
}

#[test]
fn the_command_of_an_entry_is_one_line_of_at_most_200_characters() {
  assert_eq!(one_line(&command()), "cargo nextest run --workspace");
  let long = vec!["x".repeat(150), "é".repeat(100)];
  let line = one_line(&long);
  assert_eq!(line.chars().count(), 201);
  assert!(line.ends_with("é…"), "{line}");
  assert_eq!(
    one_line(&["zsh".to_owned(), "-c".to_owned(), "a\nb\r\n\tc".to_owned()]),
    "zsh -c a b   c",
    "a script of lines"
  );
}

/// On a free turn of a host that the runs that are not heavy keep busy, a run without a measure at the head of the
/// queue waits for a quiet host only until it has waited the load bound in the queue; then it takes the whole host
/// ahead of the later smaller runs, which would fit beside that load and wait behind it. An exclusive run, a
/// measurement, waits for its quiet host whatever its wait
#[test]
fn a_run_without_a_measure_takes_a_busy_free_turn_after_the_load_bound_ahead_of_later_smaller_runs() {
  let dir = tempfile::tempdir().unwrap();
  let base = params(dir.path());
  let command = command();
  note_host(dir.path(), Cores(100)).unwrap();
  let TurnAnswer::Queued(unknown) = take(&base, &request("unknown", &command)).unwrap() else {
    panic!("10 busy of 20 is no quiet host");
  };
  assert_eq!((unknown.need, unknown.holders.len()), (Need::Unmeasured, 0));
  assert_eq!(
    queued(&take(&base, &measured("small", &command, 20)).unwrap()),
    (None, vec!["unknown"]),
    "it would fit beside the load, and waits its turn"
  );
  let waited_out = TurnParams {
    load_bound: Duration::ZERO,
    ..base
  };
  assert_eq!(
    take(&waited_out, &request("unknown", &command)).unwrap(),
    TurnAnswer::Taken,
    "past the bound, whatever the load"
  );
  assert_eq!(
    queued(&take(&waited_out, &measured("small", &command, 20)).unwrap()),
    (Some("unknown"), vec![]),
    "no run joins the whole host"
  );
  assert!(leave(&base, "unknown").unwrap());
  assert!(!leave(&base, "small").unwrap());
  let exclusive = measurement("ab", &command);
  assert!(
    matches!(take(&waited_out, &exclusive).unwrap(), TurnAnswer::Queued(_)),
    "a measurement waits for a quiet host"
  );
  assert!(!leave(&base, "ab").unwrap());
}

/// A look of a run behind the head of the queue reads the table without its lock and answers its place, so a lock that
/// another process holds stops no such look; the head of the queue, and a run whose expectation changed, take the lock
#[test]
fn a_look_behind_the_head_of_the_queue_takes_no_lock() {
  let dir = tempfile::tempdir().unwrap();
  let base = params(dir.path());
  let command = command();
  three_in_line(&base, &command);
  // the lock as another process holds it: a lock of a file of its own, outside the line of the locks of this process
  let held = std::fs::OpenOptions::new()
    .create(true)
    .truncate(false)
    .write(true)
    .open(dir.path().join(LOCK_FILE))
    .unwrap();
  held.lock().unwrap();
  let short = TurnParams {
    lock_timeout: Duration::from_millis(50),
    ..base
  };
  assert_eq!(queued(&look(&short, &request("run-3", &command)).unwrap()), (Some("run-1"), vec!["run-2"]));
  let locked = |answer: Result<TurnAnswer>| answer.unwrap_err().downcast_ref::<history_rewrite::repository_lock::RepositoryLocked>().is_some();
  assert!(locked(look(&short, &request("run-2", &command))), "the head of the queue takes the lock");
  assert!(locked(look(&short, &measured("run-3", &command, 40))), "a new expectation goes into the table");
  held.unlock().unwrap();
  assert_eq!(queued(&look(&short, &measured("run-3", &command, 40)).unwrap()), (Some("run-1"), vec!["run-2"]));
  assert_eq!(read_live(dir.path()).unwrap().queue[1].expected, Some(Cores(40)));
}

/// A request as `measured` whose last passed runs took 50 s: a short run
fn short<'a>(run: &'a str, command: &'a [String], tenths: u32) -> TurnRequest<'a> {
  TurnRequest {
    passed_wall: Some(Duration::from_secs(50)),
    ..measured(run, command, tenths)
  }
}

/// How often short runs passed the run `run` of the queue in `dir`
fn passes(dir: &Path, run: &str) -> u32 {
  read_marks(dir).of(run).passes
}

/// The runs that a short run passed, as its first ask says, each with its passes
fn passed_runs(joined: &Joined) -> Vec<(&str, u32)> {
  joined.passed.iter().map(|(entry, passes)| (entry.ticket.run.as_str(), *passes)).collect()
}

/// A short run goes ahead of the runs that wait for more, and runs beside the holder at once while it fits: a lint of a
/// minute waits for no run of the whole host. The join leaves the holder as it was, its first ask names the runs it
/// passed, and `passes.json` counts a pass of each
#[test]
fn a_short_run_goes_ahead_of_the_runs_that_wait_for_more_and_runs_beside_the_holder() {
  let dir = tempfile::tempdir().unwrap();
  let params = params(dir.path());
  let command = command();
  assert_eq!(take(&params, &measured("holder", &command, 80)).unwrap(), TurnAnswer::Taken);
  assert_eq!(queued(&take(&params, &request("whole", &command)).unwrap()), (Some("holder"), vec![]));
  assert_eq!(queued(&take(&params, &measured("big", &command, 150)).unwrap()), (Some("holder"), vec!["whole"]));
  let before = read_live(dir.path()).unwrap().holders;
  let joined = join(&params, &short("lint", &command, 60)).unwrap();
  assert_eq!(joined.answer, TurnAnswer::Taken, "8 and 6 of 20, ahead of the run of the whole host");
  assert_eq!((joined.short, passed_runs(&joined)), (Some(Duration::from_secs(50)), vec![("whole", 1), ("big", 1)]));
  let table = read_live(dir.path()).unwrap();
  assert_eq!(table.holders[..1], before[..], "the join leaves the holder as it was");
  assert_eq!(runs(&table), (Some("holder"), vec!["whole", "big"]));
  assert_eq!(holders(&table), ["holder", "lint"]);
  assert_eq!((passes(dir.path(), "whole"), passes(dir.path(), "big")), (1, 1));
}

/// A run is passed at most `PASS_BUDGET` (4) times: the fifth short run goes to the end, behind it. The short runs keep
/// their order among themselves, the others too, and a short run never goes ahead of another short run
#[test]
fn a_run_is_passed_at_most_four_times_and_each_class_keeps_its_order() {
  let dir = tempfile::tempdir().unwrap();
  let params = params(dir.path());
  let command = command();
  assert_eq!(take(&params, &request("holder", &command)).unwrap(), TurnAnswer::Taken);
  assert_eq!(queued(&take(&params, &request("long-1", &command)).unwrap()), (Some("holder"), vec![]));
  assert_eq!(queued(&take(&params, &measured("long-2", &command, 120)).unwrap()), (Some("holder"), vec!["long-1"]));
  for (n, run) in ["s1", "s2", "s3", "s4"].into_iter().enumerate() {
    let joined = join(&params, &short(run, &command, 40)).unwrap();
    let pass = u32::try_from(n).unwrap() + 1;
    assert_eq!(passed_runs(&joined), [("long-1", pass), ("long-2", pass)], "{run}");
  }
  assert_eq!(
    runs(&read_live(dir.path()).unwrap()).1,
    ["s1", "s2", "s3", "s4", "long-1", "long-2"],
    "no run of the whole host joins the holder, and the short runs wait in their order"
  );
  let fifth = join(&params, &short("s5", &command, 40)).unwrap();
  assert_eq!(passed_runs(&fifth), [], "the runs behind were passed 4 times");
  assert_eq!(queued(&take(&params, &request("long-3", &command)).unwrap()).1.len(), 7);
  let sixth = join(&params, &short("s6", &command, 40)).unwrap();
  assert_eq!(passed_runs(&sixth), [("long-3", 1)], "it goes ahead of the run new to the queue only, behind s5");
  assert_eq!(runs(&read_live(dir.path()).unwrap()).1, ["s1", "s2", "s3", "s4", "long-1", "long-2", "s5", "s6", "long-3"]);
  assert_eq!((passes(dir.path(), "long-1"), passes(dir.path(), "long-2")), (4, 4));
}

/// A run that supersedes a waiting one takes its counter of passes with its place: no run is passed more than 4 times
/// through a later run of its checkout and command either
#[test]
fn the_passes_of_a_superseded_run_go_to_the_run_that_takes_its_place() {
  let dir = tempfile::tempdir().unwrap();
  let params = params(dir.path());
  let (nextest, e2e) = (command(), vec!["pnpm".to_owned(), "e2e".to_owned()]);
  assert_eq!(take(&params, &request("holder", &nextest)).unwrap(), TurnAnswer::Taken);
  assert_eq!(queued(&take(&params, &of_checkout("old", &e2e)).unwrap()), (Some("holder"), vec![]));
  for (pass, run) in [(1, "s1"), (2, "s2"), (3, "s3")] {
    assert_eq!(passed_runs(&join(&params, &short(run, &nextest, 40)).unwrap()), [("old", pass)]);
  }
  let joined = join(&params, &of_checkout("new", &e2e)).unwrap();
  assert_eq!(joined.replaced.iter().map(|entry| entry.ticket.run.as_str()).collect::<Vec<_>>(), ["old"]);
  assert_eq!(passes(dir.path(), "new"), 3, "the later run takes the passes of the superseded one");
  assert_eq!(passed_runs(&join(&params, &short("s4", &nextest, 40)).unwrap()), [("new", 4)]);
  assert_eq!(passed_runs(&join(&params, &short("s5", &nextest, 40)).unwrap()), [], "4 passes through the supersede");
  assert_eq!(runs(&read_live(dir.path()).unwrap()).1, ["s1", "s2", "s3", "s4", "new", "s5"]);
  assert_eq!(read_marks(dir.path()).runs.get("old"), None, "the superseded run leaves the marks");
}

/// A run is short only with a measure below the whole host, not exclusive, and with 3 passed runs at least that took
/// at most `SHORT_WALL` (2 min): a run without a measure, one that measured the whole host, an exclusive one and one
/// without the walls of 3 passed runs go to the end
#[test]
fn a_run_without_a_measure_an_exclusive_one_or_one_without_three_passed_walls_is_not_short() {
  let minute = Some(Duration::from_mins(1));
  assert_eq!(short_wall(false, Some(Cores(60)), minute, 20), minute);
  assert_eq!(short_wall(false, Some(Cores(60)), Some(SHORT_WALL), 20), Some(SHORT_WALL), "2 min is short");
  assert_eq!(short_wall(false, Some(Cores(60)), Some(SHORT_WALL + Duration::from_secs(1)), 20), None);
  assert_eq!(short_wall(false, Some(Cores(60)), None, 20), None, "fewer than 3 passed runs tell no wall");
  assert_eq!(short_wall(false, None, minute, 20), None, "a run without a measure takes the whole host");
  assert_eq!(short_wall(false, Some(Cores(200)), minute, 20), None, "a measure of the whole host");
  assert_eq!(short_wall(true, Some(Cores(60)), minute, 20), None, "an exclusive run");

  let dir = tempfile::tempdir().unwrap();
  let params = params(dir.path());
  let command = command();
  assert_eq!(take(&params, &request("holder", &command)).unwrap(), TurnAnswer::Taken);
  assert_eq!(queued(&take(&params, &request("long", &command)).unwrap()), (Some("holder"), vec![]));
  let unmeasured = TurnRequest {
    expected: None,
    ..short("unmeasured", &command, 40)
  };
  let exclusive = TurnRequest {
    exclusive: true,
    ..short("exclusive", &command, 40)
  };
  let unwalled = measured("unwalled", &command, 40);
  for request in [unmeasured, exclusive, unwalled] {
    let joined = join(&params, &request).unwrap();
    assert_eq!((joined.short, passed_runs(&joined)), (None, vec![]), "{}", request.run);
  }
  assert_eq!(runs(&read_live(dir.path()).unwrap()).1, ["long", "unmeasured", "exclusive", "unwalled"]);
  assert_eq!(read_marks(dir.path()), QueueMarks::default(), "no run is marked");
}

/// A short run that passed the head of the queue on a busy free turn takes the earliest join of the runs it passed: the
/// head that waited out the load bound waits no new bound behind it, the short run takes the free turn at once, and the
/// join of the head is as it was
#[test]
fn a_short_run_takes_the_earliest_join_of_the_runs_it_passed_for_the_load_bound() {
  let dir = tempfile::tempdir().unwrap();
  let params = params(dir.path());
  let command = command();
  note_host(dir.path(), Cores(100)).unwrap();
  assert!(
    matches!(take(&params, &request("head", &command)).unwrap(), TurnAnswer::Queued(_)),
    "10 busy is no quiet host"
  );
  let mut table = read_live(dir.path()).unwrap();
  let joined_ms = now_ms() - millis(LOAD_BOUND) + 60_000;
  table.queue[0].since_ms = joined_ms;
  write_table(dir.path(), &serde_json::to_value(&table).unwrap());
  let joined = join(&params, &short("lint", &command, 150)).unwrap();
  assert!(matches!(joined.answer, TurnAnswer::Queued(_)), "10 busy and 15 do not fit 20, and 14 min waited");
  let table = read_live(dir.path()).unwrap();
  assert_eq!(runs(&table).1, ["lint", "head"]);
  assert_eq!((table.queue[0].since_ms, table.queue[1].since_ms), (joined_ms, joined_ms), "the head keeps its join");
  assert!(read_marks(dir.path()).of("lint").joined_ms.is_some_and(|at| at > joined_ms), "its own join, for the lines");

  let mut table = read_live(dir.path()).unwrap();
  let waited_out = now_ms() - millis(LOAD_BOUND) - 1_000;
  table.queue[0].since_ms = waited_out;
  table.queue[1].since_ms = waited_out;
  write_table(dir.path(), &serde_json::to_value(&table).unwrap());
  assert_eq!(
    take(&params, &short("lint", &command, 150)).unwrap(),
    TurnAnswer::Taken,
    "the bound of the head, whatever the load"
  );
  let TurnAnswer::Queued(head) = take(&params, &request("head", &command)).unwrap() else {
    panic!("the head waits for the short run");
  };
  assert_eq!(head.holders.iter().map(|holder| holder.ticket.run.as_str()).collect::<Vec<_>>(), ["lint"]);
  assert!(leave(&params, "lint").unwrap());
  assert_eq!(take(&params, &request("head", &command)).unwrap(), TurnAnswer::Taken, "its bound is over: no new one");
}

/// A supervisor of an older build rewrites `turn.json` without the fields it does not know: the classes and the passes
/// are in `passes.json`, which it does not touch, so a run passed 4 times is passed no more
#[test]
fn a_table_that_an_older_build_rewrote_keeps_the_passes() {
  let dir = tempfile::tempdir().unwrap();
  let params = params(dir.path());
  let command = command();
  assert_eq!(take(&params, &request("holder", &command)).unwrap(), TurnAnswer::Taken);
  assert!(matches!(take(&params, &request("long", &command)).unwrap(), TurnAnswer::Queued(_)));
  for run in ["s1", "s2", "s3", "s4"] {
    join(&params, &short(run, &command, 40)).unwrap();
  }
  rewrite_as_an_older_build(dir.path());
  assert_eq!(passed_runs(&join(&params, &short("s5", &command, 40)).unwrap()), []);
  assert_eq!(runs(&read_live(dir.path()).unwrap()).1, ["s1", "s2", "s3", "s4", "long", "s5"]);
}

/// Rewrites `turn.json` of `dir` as a supervisor of an older build does: without the fields it does not know
fn rewrite_as_an_older_build(dir: &Path) {
  let mut table: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(dir.join(STATE_FILE)).unwrap()).unwrap();
  for list in ["holders", "queue"] {
    for entry in table[list].as_array_mut().unwrap() {
      let entry = entry.as_object_mut().unwrap();
      entry.retain(|field, _| ["run", "pid", "process_started", "repo", "command", "since_ms", "expected", "live"].contains(&field.as_str()));
    }
  }
  write_table(dir, &table);
}

/// A short run never goes ahead of an exclusive head whose drain began, the one window of the host for its measurement:
/// it waits behind it, and once that drain has ended without a quiet host, it is the first run that is not exclusive
/// behind the head and starts past it as before. An exclusive head behind a holder, whose drain has not begun, is passed
#[test]
fn a_short_run_waits_behind_an_exclusive_head_that_drains_and_passes_it_as_before_once_the_drain_ended() {
  let dir = tempfile::tempdir().unwrap();
  let params = params(dir.path());
  let command = command();
  let exclusive = measurement("ab", &command);
  bench(&params, "bench");
  assert!(matches!(take(&params, &exclusive).unwrap(), TurnAnswer::Queued(_)), "a run in its command holds it back");
  assert!(matches!(take(&params, &request("long", &command)).unwrap(), TurnAnswer::Queued(_)));
  let joined = join(&params, &short("lint", &command, 40)).unwrap();
  assert_eq!(passed_runs(&joined), [("long", 1)], "behind the exclusive head that drains");
  assert_eq!(runs(&read_live(dir.path()).unwrap()).1, ["ab", "lint", "long"]);
  drain_began_ago(dir.path(), DRAIN_BAR + Duration::from_mins(1));
  let TurnAnswer::Passed { exclusive: passed } = look(&params, &short("lint", &command, 40)).unwrap() else {
    panic!("4 of 20 fit beside the runs that are not heavy");
  };
  assert_eq!(passed.ticket.run, "ab");
  assert!(leave(&params, "lint").unwrap());
  assert!(!leave(&params, "bench").unwrap());
  assert!(!leave(&params, "long").unwrap());
  assert!(!leave(&params, "ab").unwrap());

  assert_eq!(take(&params, &measured("holder", &command, 40)).unwrap(), TurnAnswer::Taken);
  let TurnAnswer::Queued(head) = take(&params, &exclusive).unwrap() else {
    panic!("an exclusive run waits for every holder");
  };
  assert_eq!(head.head_drain, Some(HeadDrain::Pending));
  assert_eq!(join(&params, &short("next", &command, 40)).unwrap().answer, TurnAnswer::Taken, "4 and 4 of 20");
  assert_eq!(passes(dir.path(), "ab"), 1, "a head whose drain has not begun is passed");
}

/// The window of a holder whose samples of every 10 s used `tenths` over `span`, the last `ago` before now
fn steady(span: Duration, ago: Duration, tenths: u32) -> RecentUse {
  let last = now_ms() - millis(ago);
  let mut recent = RecentUse::default();
  let mut at = last - millis(span);
  while at <= last {
    recent.push(at, Cores(tenths));
    at += 10_000;
  }
  recent
}

/// Puts the admission of the holder `run` `admitted_ago` before now and gives it the window `recent`, as the samples of
/// its supervisor over that time would
fn age_holder(dir: &Path, run: &str, admitted_ago: Duration, recent: Option<RecentUse>) {
  table_file(dir)
    .update(Duration::from_secs(10), |table: &mut HeavyTurn| {
      let holder = table.holders.iter_mut().find(|holder| holder.ticket.run == run).unwrap();
      holder.since_ms = now_ms() - millis(admitted_ago);
      holder.recent = recent;
      Ok(())
    })
    .unwrap();
}

/// Notes `tenths` as the live cores of the run `run` of the tests
pub(crate) fn note_live(params: &TurnParams<'_>, run: &str, command: &[String], tenths: u32) {
  let pid = std::process::id();
  note_use(
    params,
    &UseNote {
      run,
      pid,
      process_started: sync_utils::process_scan::process_started(pid),
      repo: "/Users/u/projects/accio",
      command,
      live: Cores(tenths),
    },
  )
  .unwrap();
}

/// `gate`, a run without a measure, takes the free turn: the whole host
fn gate_takes_the_free_turn(params: &TurnParams<'_>, command: &[String]) {
  assert_eq!(take(params, &request("gate", command)).unwrap(), TurnAnswer::Taken);
}

/// A holder without a measure holds the whole host in its grace of 10 min from its admission; past it, it counts the
/// most of its live use of the last 5 min, so the head of the queue is admitted beside its tail on one core; a burst
/// raises what it holds at its next sample, and a head without a measure still waits for no holder
#[test]
fn a_holder_without_a_measure_holds_the_whole_host_in_its_grace_and_then_its_use_of_the_last_5_min() {
  let dir = tempfile::tempdir().unwrap();
  let params = params(dir.path());
  let command = command();
  gate_takes_the_free_turn(&params, &command);
  note_live(&params, "gate", &command, 10);
  assert!(read_live(dir.path()).unwrap().holders[0].recent.is_some(), "its supervisor notes its window");
  age_holder(dir.path(), "gate", Duration::from_mins(9), Some(steady(Duration::from_mins(6), Duration::from_secs(1), 10)));
  let TurnAnswer::Queued(queue) = take(&params, &measured("nextest", &command, 80)).unwrap() else {
    panic!("in its grace it holds the whole host");
  };
  assert_eq!(queue.holders[0].need(20, now_ms()), Need::Unmeasured);

  age_holder(
    dir.path(),
    "gate",
    HOLD_GRACE + Duration::from_mins(1),
    Some(steady(Duration::from_mins(6), Duration::from_secs(1), 10)),
  );
  assert_eq!(
    read_live(dir.path()).unwrap().holders[0].need(20, now_ms()),
    Need::Recent { cores: Cores(10), measured: None }
  );
  assert_eq!(take(&params, &measured("nextest", &command, 80)).unwrap(), TurnAnswer::Taken, "1.0 and 8.0 of 20");

  note_live(&params, "gate", &command, 150);
  let TurnAnswer::Queued(queue) = take(&params, &measured("e2e", &command, 40)).unwrap() else {
    panic!("15.0, 8.0 and 4.0 do not fit 20");
  };
  assert_eq!(
    queue.holders[0].need(20, now_ms()),
    Need::Recent {
      cores: Cores(150),
      measured: None
    },
    "a burst counts at once"
  );
  assert!(leave(&params, "nextest").unwrap());
  assert!(!leave(&params, "e2e").unwrap());
  let TurnAnswer::Queued(queue) = take(&params, &request("unknown", &command)).unwrap() else {
    panic!("a run without a measure waits for every holder");
  };
  assert_eq!(queue.need, Need::Unmeasured);
}

/// Past its grace, a holder without a measure holds the whole host still while its window leaves less than a fifth of
/// the host beside it, spans less than 5 min, or its last sample is more than a minute old, and when a supervisor of an
/// older build dropped its window: a new window begins at the next sample and counts once it spans 5 min
#[test]
fn a_holder_without_a_measure_holds_the_whole_host_still_without_a_window_that_leaves_room() {
  let dir = tempfile::tempdir().unwrap();
  let params = params(dir.path());
  let command = command();
  gate_takes_the_free_turn(&params, &command);
  let past_grace = HOLD_GRACE + Duration::from_mins(110);
  for (recent, why) in [
    (steady(Duration::from_mins(6), Duration::from_secs(1), 160), "16.0 leave less than a fifth of 20"),
    (steady(Duration::from_mins(4), Duration::from_secs(1), 10), "a window of 4 min"),
    (steady(Duration::from_mins(6), Duration::from_mins(2), 10), "a last sample 2 min old"),
  ] {
    age_holder(dir.path(), "gate", past_grace, Some(recent));
    let TurnAnswer::Queued(queue) = take(&params, &measured("lint", &command, 10)).unwrap() else {
      panic!("{why}: it holds the whole host");
    };
    assert_eq!(queue.holders[0].need(20, now_ms()), Need::Unmeasured, "{why}");
  }
  age_holder(dir.path(), "gate", past_grace, Some(steady(Duration::from_mins(6), Duration::from_secs(1), 150)));
  assert_eq!(
    read_live(dir.path()).unwrap().holders[0].need(20, now_ms()),
    Need::Recent {
      cores: Cores(150),
      measured: None
    },
    "15.0 leave a fifth"
  );

  let mut table: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(dir.path().join(STATE_FILE)).unwrap()).unwrap();
  table["holders"][0].as_object_mut().unwrap().remove("recent");
  write_table(dir.path(), &table);
  assert!(
    matches!(take(&params, &measured("lint", &command, 10)).unwrap(), TurnAnswer::Queued(_)),
    "a table that an older build rewrote holds the whole host"
  );
  note_live(&params, "gate", &command, 10);
  let holder = read_live(dir.path()).unwrap().holders.remove(0);
  assert_eq!(
    (holder.need(20, now_ms()), holder.need(20, now_ms() + millis(USE_WINDOW))),
    (Need::Unmeasured, Need::Unmeasured),
    "a window of one sample counts in no time"
  );
  assert!(leave(&params, "gate").unwrap());
  assert!(!leave(&params, "lint").unwrap());

  assert_eq!(take(&params, &measurement("ab", &command)).unwrap(), TurnAnswer::Taken);
  note_live(&params, "ab", &command, 30);
  assert_eq!(read_live(dir.path()).unwrap().holders[0].recent, None, "an exclusive holder keeps no window");
}

/// A holder of a measure counts the larger of its measure and its live use in its grace of 10 min, so the head waits
/// beside its build; past the grace it counts the most of its live use of the last 5 min, so the head starts beside its
/// quiet tail, and a burst raises what it holds at its next sample. A holder that measured the whole host does the same
/// past its grace
#[test]
fn a_holder_of_a_measure_counts_its_measure_in_its_grace_and_then_its_use_of_the_last_5_min() {
  let dir = tempfile::tempdir().unwrap();
  let params = params(dir.path());
  let command = command();
  assert_eq!(take(&params, &measured("gate", &command, 160)).unwrap(), TurnAnswer::Taken);
  note_live(&params, "gate", &command, 10);
  let tail = || Some(steady(Duration::from_mins(6), Duration::from_secs(1), 10));
  age_holder(dir.path(), "gate", Duration::from_mins(9), tail());
  let TurnAnswer::Queued(queue) = take(&params, &measured("next", &command, 160)).unwrap() else {
    panic!("16.0 and 16.0 do not fit 20 in its grace");
  };
  assert_eq!(queue.holders[0].need(20, now_ms()), Need::Cores { cores: Cores(160), live: false });

  age_holder(dir.path(), "gate", HOLD_GRACE + Duration::from_mins(50), tail());
  assert_eq!(
    read_live(dir.path()).unwrap().holders[0].need(20, now_ms()),
    Need::Recent {
      cores: Cores(10),
      measured: Some(Cores(160))
    }
  );
  assert_eq!(take(&params, &measured("next", &command, 160)).unwrap(), TurnAnswer::Taken, "1.0 and 16.0 of 20");
  note_live(&params, "gate", &command, 120);
  let TurnAnswer::Queued(queue) = take(&params, &measured("lint", &command, 10)).unwrap() else {
    panic!("12.0, 16.0 and 1.0 do not fit 20");
  };
  assert_eq!(
    queue.holders[0].need(20, now_ms()),
    Need::Recent {
      cores: Cores(120),
      measured: Some(Cores(160))
    },
    "a burst counts at once"
  );
  assert!(leave(&params, "next").unwrap());
  assert!(!leave(&params, "lint").unwrap());
  assert!(leave(&params, "gate").unwrap());

  assert_eq!(take(&params, &measured("whole", &command, 200)).unwrap(), TurnAnswer::Taken);
  age_holder(dir.path(), "whole", HOLD_GRACE + Duration::from_mins(1), tail());
  assert_eq!(
    take(&params, &measured("next", &command, 160)).unwrap(),
    TurnAnswer::Taken,
    "a holder that measured the whole host counts its use past its grace"
  );
}

/// A holder of a measure past its grace whose window counts as the grace still counts the most of its samples of the
/// last 5 min: a peak that leaves less than a fifth of the host beside it, a window of less than 5 min and a last sample
/// of over a minute ago count that most, not the measure below it. On 2026-11-25 a fast lane of a peak of 17.3 counted
/// its measure of 12.6, and a run of 3.0 would have started beside it
#[test]
fn a_holder_of_a_measure_past_its_grace_counts_at_least_the_most_of_its_last_5_min() {
  let dir = tempfile::tempdir().unwrap();
  let params = params(dir.path());
  let command = command();
  assert_eq!(take(&params, &measured("lane", &command, 126)).unwrap(), TurnAnswer::Taken);
  // samples of every 10 s at 10.0 over 6 min, 17.3 from 3 min to 2.5 min before the last
  let burst = |span: Duration, ago: Duration| {
    let last = now_ms() - millis(ago);
    let mut recent = RecentUse::default();
    let mut at = last - millis(span);
    while at <= last {
      let burst = (last - 180_000..last - 150_000).contains(&at);
      recent.push(at, Cores(if burst { 173 } else { 100 }));
      at += 10_000;
    }
    recent
  };
  let past_grace = HOLD_GRACE + Duration::from_mins(20);
  let recent_of = |cores| Need::Recent {
    cores: Cores(cores),
    measured: Some(Cores(126)),
  };
  for (recent, why) in [
    (burst(Duration::from_mins(6), Duration::from_secs(1)), "17.3 leave less than a fifth of 20"),
    (burst(Duration::from_mins(4), Duration::from_secs(1)), "a window of 4 min"),
    (burst(Duration::from_mins(6), Duration::from_secs(90)), "a last sample 90 s old"),
  ] {
    age_holder(dir.path(), "lane", past_grace, Some(recent));
    assert_eq!(read_live(dir.path()).unwrap().holders[0].need(20, now_ms()), recent_of(173), "{why}");
  }
  age_holder(dir.path(), "lane", past_grace, Some(burst(Duration::from_mins(6), Duration::from_secs(1))));
  let TurnAnswer::Queued(queue) = take(&params, &measured("next", &command, 30)).unwrap() else {
    panic!("17.3 and 3.0 do not fit 20");
  };
  assert_eq!(queue.holders[0].need(20, now_ms()), recent_of(173));
  assert!(!leave(&params, "next").unwrap());
  assert_eq!(take(&params, &measured("lint", &command, 20)).unwrap(), TurnAnswer::Taken, "17.3 and 2.0 fit 20");
  assert!(leave(&params, "lint").unwrap());

  age_holder(dir.path(), "lane", Duration::from_mins(9), Some(burst(Duration::from_mins(6), Duration::from_secs(1))));
  assert_eq!(
    read_live(dir.path()).unwrap().holders[0].need(20, now_ms()),
    Need::Cores { cores: Cores(126), live: false },
    "in its grace it counts its measure"
  );
  age_holder(dir.path(), "lane", past_grace, Some(steady(Duration::from_mins(4), Duration::from_secs(1), 50)));
  assert_eq!(
    read_live(dir.path()).unwrap().holders[0].need(20, now_ms()),
    Need::Cores { cores: Cores(126), live: false },
    "a most below the measure leaves the measure of a window that counts as the grace"
  );
  age_holder(dir.path(), "lane", past_grace, Some(steady(Duration::from_mins(6), Duration::from_mins(6), 173)));
  assert_eq!(
    read_live(dir.path()).unwrap().holders[0].need(20, now_ms()),
    Need::Cores { cores: Cores(126), live: false },
    "samples older than 5 min count no more"
  );
  assert!(leave(&params, "lane").unwrap());
}

/// The window of the live use keeps the most of each half minute of the last 5 min: a burst older than that counts no
/// more, a gap of over a minute between two samples begins the window again, and so does a sample before the last
#[test]
fn the_window_of_the_live_use_keeps_the_most_of_each_half_minute_of_the_last_5_min() {
  let start = 1_800_000_000_000;
  let mut recent = RecentUse::default();
  for second in (0..=600).step_by(10) {
    let burst = (60..90).contains(&second) || (400..420).contains(&second);
    recent.push(start + second * 1_000, Cores(if burst { 180 } else { 10 }));
  }
  let last = start + 600_000;
  assert_eq!(recent.since_ms, start);
  assert!(recent.steps.len() <= 11, "{} steps", recent.steps.len());
  assert_eq!(recent.peak(last), Some(Cores(180)), "the burst of 3 min ago");
  assert_eq!(recent.peak(last + 30_000), Some(Cores(180)), "a fresh window of a supervisor between its samples");
  assert_eq!(recent.peak(last + millis(USE_GAP) + 1), None, "a last sample older than a minute");
  let mut tail = recent.clone();
  for second in (610..=1_000).step_by(10) {
    tail.push(start + second * 1_000, Cores(10));
  }
  assert_eq!(tail.peak(start + 1_000_000), Some(Cores(10)), "the burst is 10 min old");
  let mut gap = recent.clone();
  gap.push(last + millis(USE_GAP) + 1_000, Cores(10));
  assert_eq!(
    (gap.since_ms, gap.steps.len(), gap.peak(last + millis(USE_GAP) + 1_000)),
    (last + millis(USE_GAP) + 1_000, 1, None)
  );
  let mut back = recent;
  back.push(last - 1_000, Cores(10));
  assert_eq!(back.since_ms, last - 1_000, "a clock set back begins the window again");
}

/// A request as `request` of the group `group` (`brd-host run --group`), from a checkout of its own
fn grouped<'a>(run: &'a str, group: &'a str, command: &'a [String]) -> TurnRequest<'a> {
  TurnRequest {
    group: Some(group),
    ..request(run, command)
  }
}

/// A request as `request` from the checkout `checkout`, without a group: the group of the run is its checkout
fn of<'a>(run: &'a str, checkout: &'a str, command: &'a [String]) -> TurnRequest<'a> {
  TurnRequest {
    affinity: checkout,
    ..request(run, command)
  }
}

/// The run `holder` of the group `session` takes the whole host of a free turn, and no run of the queue starts beside it
fn session_holds(params: &TurnParams<'_>, command: &[String]) {
  assert_eq!(take(params, &grouped("holder", "session", command)).unwrap(), TurnAnswer::Taken);
}

/// The run `holder` of the group `session` takes the whole host, and `runs` of the group wait behind it
fn session_waits(params: &TurnParams<'_>, command: &[String], runs: &[&str]) {
  session_holds(params, command);
  join_all(params, runs, |run| grouped(run, "session", command));
}

/// A table of the turn in a directory of its own where `runs` of the group `session` wait behind its `holder`, and the
/// command of the runs
fn waiting_session(runs: &[&str]) -> (tempfile::TempDir, Vec<String>) {
  let dir = tempfile::tempdir().unwrap();
  let command = command();
  session_waits(&params(dir.path()), &command, runs);
  (dir, command)
}

/// The runs of the queue in `dir`
fn queue_of(dir: &Path) -> Vec<String> {
  runs(&read_live(dir).unwrap()).1.into_iter().map(str::to_owned).collect()
}

/// Each run of `runs` joins the queue of `params` by `request` and waits
fn join_all<'a>(params: &TurnParams<'_>, runs: &[&'a str], request: impl Fn(&'a str) -> TurnRequest<'a>) {
  for run in runs {
    assert!(matches!(take(params, &request(run)).unwrap(), TurnAnswer::Queued(_)), "{run}");
  }
}

/// How often the turns of the groups passed each of `runs` of the queue in `dir`
fn turn_passes(dir: &Path, runs: &[&str]) -> Vec<u32> {
  let passes = read_groups(dir).0.passes_in(&read_live(dir).unwrap().queue);
  runs.iter().map(|run| passes.get(*run).copied().unwrap_or_default()).collect()
}

/// The heavy queue takes turns between the groups of its runs: a group of many runs (the agents of one session in
/// checkouts of their own, `--group`) no longer keeps a run of another group that comes later waiting for all of them.
/// Each row joins a run behind a holder of the whole host, and the queue after it is as the row says: first come first
/// inside a group, and a run of a group with fewer runs waiting goes ahead of the runs of a group with more. A run of
/// the turns counts its passes in `groups.json` and touches none of the short lane: no pass of `passes.json`, no join
/// of another run
#[test]
fn the_queue_takes_turns_between_groups_and_each_group_keeps_its_order() {
  let dir = tempfile::tempdir().unwrap();
  let params = params(dir.path());
  let command = command();
  session_holds(&params, &command);
  // (run, group, the queue after its join)
  let rows: [(&str, &str, &[&str]); 7] = [
    ("a1", "session", &["a1"]),
    ("a2", "session", &["a1", "a2"]),
    ("a3", "session", &["a1", "a2", "a3"]),
    ("b1", "other", &["a1", "b1", "a2", "a3"]),
    ("c1", "third", &["a1", "b1", "c1", "a2", "a3"]),
    ("b2", "other", &["a1", "b1", "c1", "a2", "b2", "a3"]),
    ("a4", "session", &["a1", "b1", "c1", "a2", "b2", "a3", "a4"]),
  ];
  for (run, group, queue) in rows {
    assert!(matches!(take(&params, &grouped(run, group, &command)).unwrap(), TurnAnswer::Queued(_)), "{run}");
    assert_eq!(queue_of(dir.path()), queue, "after {run}");
  }
  assert_eq!(turn_passes(dir.path(), &["a1", "b1", "c1", "a2", "b2", "a3", "a4"]), [0, 0, 0, 2, 0, 3, 0]);
  assert_eq!(read_marks(dir.path()).runs, BTreeMap::new(), "no pass of the short lane, no join of another run");
  let table = read_live(dir.path()).unwrap();
  assert_eq!(table.queue[1].group.as_deref(), Some("other"), "the entry names its group for the lines");
  // the runs that a run of the turns passes joined an hour ago: it keeps its own join for its load bound
  let mut old: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(dir.path().join(STATE_FILE)).unwrap()).unwrap();
  for entry in old["queue"].as_array_mut().unwrap() {
    entry["since_ms"] = serde_json::json!(now_ms() - 3_600_000);
  }
  write_table(dir.path(), &old);
  let before = now_ms();
  join_all(&params, &["d1"], |run| grouped(run, "fourth", &command));
  let table = read_live(dir.path()).unwrap();
  let d1 = table.queue.iter().find(|entry| entry.ticket.run == "d1").unwrap();
  assert!(d1.since_ms >= before, "its own join, not that of a run it passed");
  assert_eq!(table.queue.iter().position(|entry| entry.ticket.run == "d1"), Some(3), "ahead of the second run of session");
  assert_eq!(read_marks(dir.path()).runs, BTreeMap::new());

  // only the head is admitted, and the turns follow the queue
  assert!(leave(&params, "holder").unwrap());
  for run in ["a1", "b1", "c1", "d1"] {
    assert_eq!(take(&params, &grouped(run, "any", &command)).unwrap(), TurnAnswer::Taken, "{run}");
    assert!(leave(&params, run).unwrap());
  }
  assert_eq!(queue_of(dir.path()), ["a2", "b2", "a3", "a4"]);
}

/// Without `--group` the group of a run is its checkout. With at most one waiting run of each checkout the queue is
/// first come first exactly as before the groups, also for a checkout whose run took the turn from the queue and that
/// joins again, as an agent that waits for each of its runs does; a second waiting run of one checkout waits behind the
/// first runs of the others
#[test]
fn without_groups_the_runs_of_checkouts_with_one_run_waiting_each_are_first_come_first() {
  let dir = tempfile::tempdir().unwrap();
  let params = params(dir.path());
  let (nextest, e2e) = (command(), vec!["pnpm".to_owned(), "e2e".to_owned()]);
  let checkout = |run: &str| format!("checkout-{}", &run[..1]);
  let names: Vec<String> = ["x1", "a1", "b1", "c1", "a2", "d1", "b2", "d2", "e1"].iter().map(|run| checkout(run)).collect();
  let affinity = |run: &str| names.iter().find(|name| name.ends_with(&run[..1])).unwrap().as_str();
  assert_eq!(take(&params, &of("x1", affinity("x1"), &nextest)).unwrap(), TurnAnswer::Taken);
  join_all(&params, &["a1", "b1", "c1"], |run| of(run, affinity(run), &nextest));
  // a1 takes the turn from the head of the queue, and its checkout joins again, then a new checkout
  assert!(leave(&params, "x1").unwrap());
  assert_eq!(take(&params, &of("a1", affinity("a1"), &nextest)).unwrap(), TurnAnswer::Taken);
  join_all(&params, &["a2"], |run| of(run, affinity(run), &e2e));
  join_all(&params, &["d1"], |run| of(run, affinity(run), &nextest));
  assert_eq!(queue_of(dir.path()), ["b1", "c1", "a2", "d1"], "first come first, a's next run included");
  assert!(leave(&params, "a1").unwrap());
  assert_eq!(take(&params, &of("b1", affinity("b1"), &nextest)).unwrap(), TurnAnswer::Taken);
  join_all(&params, &["b2"], |run| of(run, affinity(run), &e2e));
  assert_eq!(queue_of(dir.path()), ["c1", "a2", "d1", "b2"], "the agent loop of one run at a time");
  assert_eq!(turn_passes(dir.path(), &["c1", "a2", "d1", "b2"]), [0, 0, 0, 0]);

  // a second waiting run of one checkout, then a later run of another one
  join_all(&params, &["d2"], |run| of(run, affinity(run), &e2e));
  join_all(&params, &["e1"], |run| of(run, affinity(run), &nextest));
  assert_eq!(queue_of(dir.path()), ["c1", "a2", "d1", "b2", "e1", "d2"]);
  assert_eq!(read_live(dir.path()).unwrap().queue[0].group, None, "a run without --group names none");
  assert_eq!(group_key(None, CHECKOUT, "run-1"), CHECKOUT);
  assert_eq!(group_key(Some("session"), CHECKOUT, "run-1"), "session");
  assert_eq!(group_key(Some(""), "", "run-1"), "run-1", "a run of an older client without an affinity is alone");
}

/// A short run that waits at the end of the queue, as one whose backlog used up its short passes, keeps no turn from the
/// runs of other groups: a later run of a new group still goes ahead of the later runs of a group of many, and passes
/// the short run too, which counts it in its own budget of the turns
#[test]
fn a_short_run_at_the_end_of_the_queue_keeps_the_turns_of_the_groups() {
  let (dir, command) = waiting_session(&["s1", "s2", "s3", "s4"]);
  let params = params(dir.path());
  let mut marks = read_marks(dir.path());
  for run in ["s1", "s2", "s3", "s4"] {
    marks.runs.entry(run.to_owned()).or_default().passes = PASS_BUDGET;
  }
  write_marks(dir.path(), &marks).unwrap();
  let lint = TurnRequest {
    group: Some("lint"),
    ..short("lint", &command, 60)
  };
  assert_eq!(passed_runs(&join(&params, &lint).unwrap()), [], "the backlog used up its short passes");
  assert_eq!(queue_of(dir.path()), ["s1", "s2", "s3", "s4", "lint"]);
  join_all(&params, &["o1", "o2"], |run| grouped(run, run, &command));
  assert_eq!(queue_of(dir.path()), ["s1", "o1", "o2", "s2", "s3", "s4", "lint"], "behind s1 only");
  assert_eq!(turn_passes(dir.path(), &["s2", "lint"]), [2, 2]);
}

/// The bound of the turns: a run is passed by runs of the turns at most `TURN_PASS_BUDGET` (8) times, whatever the
/// count of the groups and while the head cannot start, and only by runs of groups with fewer runs waiting; the first
/// run of a group is passed by none. Twenty new groups join behind a group of four while a holder of the whole host
/// keeps the head waiting; a run that leaves the queue without the turn gives its passes back
#[test]
fn a_run_is_passed_by_the_turns_at_most_eight_times_whatever_the_count_of_groups() {
  let dir = tempfile::tempdir().unwrap();
  let params = params(dir.path());
  let command = command();
  session_holds(&params, &command);
  let big = ["big-1", "big-2", "big-3", "big-4"];
  join_all(&params, &big, |run| grouped(run, "big", &command));
  let others: Vec<String> = (0..20).map(|n| format!("other-{n:02}")).collect();
  let names: Vec<&str> = others.iter().map(String::as_str).collect();
  join_all(&params, &names, |run| grouped(run, run, &command));
  assert_eq!(turn_passes(dir.path(), &big), [0, TURN_PASS_BUDGET, TURN_PASS_BUDGET, TURN_PASS_BUDGET]);
  let queue = queue_of(dir.path());
  let mut expected: Vec<&str> = vec!["big-1"];
  expected.extend(&names[..8]);
  expected.extend(&big[1..]);
  expected.extend(&names[8..]);
  assert_eq!(queue, expected, "the ninth new group waits behind the runs passed 8 times");
  for run in &names {
    assert_eq!(turn_passes(dir.path(), &[run]), [0], "a first run of a group is passed by none: {run}");
  }

  // a run of the turns that leaves without the turn, as a refused start of `admit_now`, gives its passes back
  assert!(!leave(&params, "other-00").unwrap());
  assert_eq!(turn_passes(dir.path(), &big[1..]), [7, 7, 7]);
  join_all(&params, &["late"], |run| grouped(run, run, &command));
  assert_eq!(queue_of(dir.path())[8], "late", "behind the first runs of the other groups, ahead of big-2");
  assert_eq!(turn_passes(dir.path(), &big[1..]), [8, 8, 8]);
}

/// The short lane keeps its rules beside the turns: the turns use none of its passes, so a short run still goes ahead
/// of a backlog that eight other groups went ahead of; no run of the turns goes ahead of the head, a short run is passed
/// by the turns as any run of its round, and a run that supersedes one of its group takes its place
#[test]
fn the_short_lane_the_head_and_the_supersede_keep_their_rules_beside_the_turns() {
  let dir = tempfile::tempdir().unwrap();
  let params = params(dir.path());
  let (nextest, e2e) = (command(), vec!["pnpm".to_owned(), "e2e".to_owned()]);
  session_holds(&params, &nextest);
  let session = ["s1", "s2", "s3"];
  join_all(&params, &session, |run| grouped(run, "session", &nextest));
  let others: Vec<String> = (0..8).map(|n| format!("o{n}")).collect();
  let names: Vec<&str> = others.iter().map(String::as_str).collect();
  join_all(&params, &names, |run| grouped(run, run, &nextest));
  assert_eq!(turn_passes(dir.path(), &session), [0, 8, 8]);
  let lint = TurnRequest {
    group: Some("lint"),
    ..short("lint", &nextest, 60)
  };
  let joined = join(&params, &lint).unwrap();
  assert_eq!(passed_runs(&joined).len(), 11, "the short run goes ahead of every run of the queue, as before");
  assert_eq!(queue_of(dir.path())[0], "lint");
  assert_eq!((passes(dir.path(), "s2"), passes(dir.path(), "s3")), (1, 1), "one pass of the short lane each");

  // a run of the turns goes behind the head; a short run of a later round is passed as any run of it
  let entry = |run: &str| TurnEntry {
    ticket: Ticket {
      run: run.to_owned(),
      ..Ticket::default()
    },
    ..TurnEntry::default()
  };
  let queue = [entry("head"), entry("short"), entry("long")];
  let groups = QueueGroups {
    runs: ["head", "short", "long"]
      .map(|run| {
        let mark = GroupMark {
          group: "session".to_owned(),
          ..GroupMark::default()
        };
        (run.to_owned(), mark)
      })
      .into(),
    ..QueueGroups::default()
  };
  let mut marks = QueueMarks::default();
  assert_eq!(place(&queue, &marks, None, false, Some((&groups, "other"))), 1, "behind the head only");
  marks.runs.insert(
    "long".to_owned(),
    RunMark {
      short_ms: Some(50_000),
      ..RunMark::default()
    },
  );
  assert_eq!(place(&queue, &marks, None, false, Some((&groups, "other"))), 1, "ahead of the short run of round 1");
  assert_eq!(place(&queue[..1], &marks, None, false, Some((&groups, "other"))), 1, "behind the head of a higher round");

  // a later run of the checkout and command of b1, of its group, takes its place and its passes
  join_all(&params, &["b1", "b2"], |run| grouped(run, "other", if run == "b1" { &nextest } else { &e2e }));
  let later = TurnRequest {
    affinity: "b1",
    ..grouped("b1-later", "other", &nextest)
  };
  let joined = join(&params, &later).unwrap();
  assert_eq!(joined.replaced.iter().map(|entry| entry.ticket.run.as_str()).collect::<Vec<_>>(), ["b1"]);
  assert_eq!(queue_of(dir.path())[12..], ["b1-later", "b2"]);
  assert_eq!(read_groups(dir.path()).0.runs.get("b1"), None, "the superseded run leaves the groups");
}

/// A later run of the checkout and command of a waiting run whose group differs takes its own turn, not the place of
/// the run it supersedes: it waits behind the earlier runs of its new group, and the passes of the superseded run go
/// back to the runs it went ahead of
#[test]
fn a_run_that_supersedes_a_run_of_another_group_takes_its_own_turn() {
  let (dir, command) = waiting_session(&["s1", "s2", "s3"]);
  let params = params(dir.path());
  join_all(&params, &["x"], |run| of(run, "checkout-x", &command));
  assert_eq!(queue_of(dir.path()), ["s1", "x", "s2", "s3"]);
  assert_eq!(turn_passes(dir.path(), &["s2", "s3"]), [1, 1]);
  let later = TurnRequest {
    affinity: "checkout-x",
    ..grouped("x-later", "session", &command)
  };
  let joined = join(&params, &later).unwrap();
  assert_eq!(joined.replaced.iter().map(|entry| entry.ticket.run.as_str()).collect::<Vec<_>>(), ["x"]);
  assert_eq!(queue_of(dir.path()), ["s1", "s2", "s3", "x-later"], "first come first inside its new group");
  assert_eq!(turn_passes(dir.path(), &["s2", "s3"]), [0, 0], "the passes of x go back");
}

/// The groups and the passes of the turns are in `groups.json`, which no older build knows: a supervisor of an older
/// build that rewrites `turn.json` without `group`, and `passes.json` as `{"runs": …}`, as builds 1.24 to 1.27 do,
/// changes no turn. Without `groups.json` each run is of the group of its entry, by its affinity, or alone once a rewrite
/// dropped that too: the order goes on, first come first, and no run is lost
#[test]
fn a_rewrite_by_an_older_build_changes_no_turn_and_without_the_groups_a_run_is_of_its_checkout() {
  let (dir, command) = waiting_session(&["a1", "a2", "a3"]);
  let params = params(dir.path());
  join_all(&params, &["b1"], |run| grouped(run, "other", &command));
  rewrite_as_an_older_build(dir.path());
  std::fs::write(dir.path().join(PASSES_FILE), r#"{"runs":{"a2":{"passes":1}}}"#).unwrap();
  join_all(&params, &["c1"], |run| grouped(run, "third", &command));
  assert_eq!(queue_of(dir.path()), ["a1", "b1", "c1", "a2", "a3"], "the turns outlive the rewrites");
  assert_eq!(turn_passes(dir.path(), &["a2", "a3"]), [2, 2]);
  assert_eq!(passes(dir.path(), "a2"), 1, "the short lane keeps its own passes");

  std::fs::remove_file(dir.path().join(GROUPS_FILE)).unwrap();
  join_all(&params, &["d1"], |run| grouped(run, "fourth", &command));
  assert_eq!(queue_of(dir.path()), ["a1", "b1", "c1", "a2", "a3", "d1"], "each run alone: first come first");
}

/// `groups.json` names its schema, and no file of any build makes the turn refuse: a file of a later schema, after a
/// downgrade, and one that does not parse, as a power loss can leave it, name no group and no pass, say so in the notes
/// of the join, and the join writes the file of this schema; every waiting run gets its whole budget again
#[test]
fn the_groups_of_the_queue_name_their_schema_and_a_file_of_a_later_build_or_that_does_not_parse_starts_anew() {
  let (dir, command) = waiting_session(&["s1", "s2"]);
  let params = params(dir.path());
  join_all(&params, &["o1"], |run| grouped(run, run, &command));
  let written = || -> serde_json::Value { serde_json::from_str(&std::fs::read_to_string(dir.path().join(GROUPS_FILE)).unwrap()).unwrap() };
  assert_eq!(written()["schema"], 1, "{}", written());
  assert_eq!(turn_passes(dir.path(), &["s2"]), [1]);

  std::fs::write(dir.path().join(GROUPS_FILE), r#"{"schema":2,"runs":{"s2":{"group":"later"}}}"#).unwrap();
  let joined = join(&params, &grouped("o2", "o2", &command)).unwrap();
  assert!(matches!(joined.answer, TurnAnswer::Queued(_)), "a later build takes no turn down");
  assert!(
    joined.notes[0].contains("has the schema 2 of a later build of brd-host, and this one reads 1"),
    "{:?}",
    joined.notes
  );
  assert_eq!(written()["schema"], 1, "the join writes the file of this schema");
  assert_eq!(queue_of(dir.path()), ["s1", "o1", "o2", "s2"], "the groups of the entries");

  std::fs::write(dir.path().join(GROUPS_FILE), "").unwrap();
  assert_eq!(turn_passes(dir.path(), &["s2"]), [0], "a file that does not parse names no pass");
  let joined = join(&params, &grouped("o3", "o3", &command)).unwrap();
  assert!(joined.notes[0].contains("does not parse"), "{:?}", joined.notes);
  assert_eq!(queue_of(dir.path()), ["s1", "o1", "o2", "o3", "s2"]);
  assert_eq!(turn_passes(dir.path(), &["s2"]), [1], "s2 may be passed 8 times again");
  assert_eq!(
    join(&params, &grouped("o4", "o4", &command)).unwrap().notes,
    Vec::<String>::new(),
    "a file of this schema says nothing"
  );
}

/// The `groups.json` of the first build of the groups, which a host that ran it has and whose supervisors write it
/// again after an upgrade (no `schema`, a number of `passes` per run), takes no turn down: it keeps its groups and the
/// runs each went ahead of, and the passes of the runs that took the turn before the upgrade count no more
#[test]
fn the_groups_of_the_first_build_of_the_groups_keep_their_turns_after_an_upgrade() {
  let (dir, command) = waiting_session(&["s1", "s2", "s3"]);
  let params = params(dir.path());
  join_all(&params, &["o1"], |run| grouped(run, run, &command));
  // the shape that 68569e7e5 writes, as on the Studio: the group, a number of passes, the runs it went ahead of
  let first_build = r#"{"runs":{"holder":{"group":"session","passes":0},"s1":{"group":"session","passes":0},"s2":{"group":"session","passes":1},"s3":{"group":"session","passes":1},"o1":{"group":"o1","passes":0,"passed":["s2","s3"]},"gone":{"group":"gone","passes":0,"passed":["s3"]}}}"#;
  std::fs::write(dir.path().join(GROUPS_FILE), first_build).unwrap();
  let (groups, note) = read_groups(dir.path());
  assert_eq!((groups.schema, note), (1, None));
  assert_eq!(turn_passes(dir.path(), &["s2", "s3"]), [1, 1], "the passes of runs that wait count, of gone ones not");
  let sight = crate::host_estimate::Sight::of(dir.path(), 20, now_ms(), DRAIN_BAR);
  assert!(sight.is_ok(), "the estimate reads it");
  let joined = join(&params, &grouped("o2", "o2", &command)).unwrap();
  assert!(matches!(joined.answer, TurnAnswer::Queued(_)));
  assert_eq!(joined.notes, Vec::<String>::new());
  assert_eq!(queue_of(dir.path()), ["s1", "o1", "o2", "s2", "s3"], "the groups of the first build order the queue");
  assert_eq!(turn_passes(dir.path(), &["s2", "s3"]), [2, 2]);
  let written: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(dir.path().join(GROUPS_FILE)).unwrap()).unwrap();
  assert_eq!(written["schema"], 1, "the join writes the file of this schema");
  std::fs::write(dir.path().join(GROUPS_FILE), first_build).unwrap();
  assert!(leave(&params, "o2").is_ok(), "a supervisor of the first build may write it again, and it reads so again");
  assert_eq!(read_groups(dir.path()).1, None);
}

/// A run that takes the place of a run of its group that it supersedes takes the passes that runs of the turns made of
/// that place too: an agent that starts its gate again on each new commit does not move back by the budget each time
#[test]
fn a_run_that_supersedes_a_run_of_its_group_keeps_the_passes_of_its_place() {
  let dir = tempfile::tempdir().unwrap();
  let params = params(dir.path());
  let command = command();
  session_holds(&params, &command);
  join_all(&params, &["s1"], |run| grouped(run, "session", &command));
  fn again<'a>(run: &'a str, command: &'a [String]) -> TurnRequest<'a> {
    TurnRequest {
      affinity: "checkout-s2",
      ..grouped(run, "session", command)
    }
  }
  join_all(&params, &["s2"], |run| again(run, &command));
  let others: Vec<String> = (0..40).map(|n| format!("o{n:02}")).collect();
  let names: Vec<&str> = others.iter().map(String::as_str).collect();
  join_all(&params, &names[..8], |run| grouped(run, run, &command));
  assert_eq!(queue_of(dir.path())[9], "s2");
  for (round, run) in ["s2-a", "s2-b", "s2-c"].into_iter().enumerate() {
    let joined = join(&params, &again(run, &command)).unwrap();
    assert_eq!(joined.replaced.len(), 1, "{run}");
    assert_eq!(turn_passes(dir.path(), &[run]), [TURN_PASS_BUDGET], "{run} keeps the passes of its place");
    join_all(&params, &names[8 * (round + 1)..8 * (round + 2)], |run| grouped(run, run, &command));
    assert_eq!(queue_of(dir.path())[9], run, "{run} keeps its place: no run of the turns passes it");
  }
}

/// A run passes every run from its place to the end, whatever their rounds: a first waiting run of a group (round 0)
/// behind a run of a later round is passed too, within its budget; a first waiting run with no run of a later round
/// ahead of it is passed by none
#[test]
fn a_first_waiting_run_behind_a_run_of_a_later_round_is_passed_within_its_budget() {
  let (dir, command) = waiting_session(&["s1", "s2", "s3"]);
  let params = params(dir.path());
  join_all(&params, &["o1", "o2", "o3", "o4", "o5", "o6", "o7", "o8"], |run| grouped(run, run, &command));
  join_all(&params, &["f1"], |run| grouped(run, "f", &command));
  assert_eq!(queue_of(dir.path()).last().map(String::as_str), Some("f1"), "s2 and s3 are spent");
  assert!(!leave(&params, "o1").unwrap());
  join_all(&params, &["p1"], |run| grouped(run, "p", &command));
  let queue = queue_of(dir.path());
  assert_eq!(queue[8..], ["p1", "s2", "s3", "f1"]);
  assert_eq!(turn_passes(dir.path(), &["f1", "o2"]), [1, 0], "f1 of round 0 behind s2 of round 1 is passed; o2 not");
}

/// The passes of a run are counted from `groups.json` and the queue: a run of the turns whose process is gone, and one
/// whose join wrote its mark while the write of the table after it failed, count no pass
#[test]
fn a_run_of_the_turns_that_is_gone_or_whose_table_write_failed_counts_no_pass() {
  let (dir, command) = waiting_session(&["s1", "s2"]);
  let params = params(dir.path());
  // the pid of this process with another start: the process of the run is gone
  let gone = TurnRequest {
    process_started: Some(1),
    ..grouped("gone", "gone", &command)
  };
  join(&params, &gone).unwrap();
  assert_eq!(read_groups(dir.path()).0.runs["gone"].passed, ["s2"], "it went ahead of s2");
  assert_eq!(turn_passes(dir.path(), &["s2"]), [0], "a gone run counts no pass");
  // a mark of a run that is not in the table, as a failed write of the table after the write of groups.json leaves it
  let mut groups = read_groups(dir.path()).0;
  groups.runs.insert(
    "ghost".to_owned(),
    GroupMark {
      commit: None,
      measure_key: None,
      priority: None,
      group: "ghost".to_owned(),
      passed: vec!["s2".to_owned()],
      took: false,
    },
  );
  write_groups(dir.path(), &groups).unwrap();
  assert_eq!(turn_passes(dir.path(), &["s2"]), [0], "a run not in the table counts no pass");
  join_all(&params, &["x"], |run| grouped(run, "x", &command));
  assert_eq!(turn_passes(dir.path(), &["s2"]), [1], "only x, which waits");
  assert_eq!(read_groups(dir.path()).0.runs.get("ghost"), None, "the next join drops the marks that count nothing");
}

/// A mark of `groups.json` of this schema with a field that this build does not know, as a later build of the same
/// schema adds one, or a value of a field it knows that it does not read, is read with its marks kept: no field of a mark
/// makes the file refuse or start anew
#[test]
fn a_mark_of_the_groups_with_a_field_this_build_does_not_know_is_read_and_kept() {
  let dir = tempfile::tempdir().unwrap();
  std::fs::write(
    dir.path().join(GROUPS_FILE),
    r#"{"schema":1,"runs":{"s2":{"group":"session","priority":3,"commit":"1a2b"},"o1":{"group":"o1","passed":["s2"],"measure_key":"k"}}}"#,
  )
  .unwrap();
  let (groups, note) = read_groups(dir.path());
  assert_eq!(note, None);
  assert_eq!(groups.runs["s2"].group, "session");
  assert_eq!(groups.runs["o1"].passed, ["s2"]);
  // a field this build knows with a value it does not read is none, and the rest of the mark stays
  assert_eq!((groups.runs["s2"].priority, groups.runs["s2"].commit.as_deref()), (None, Some("1a2b")));
}

/// The runs `s1` and `s2` of the group `session` wait behind its holder, and the run `x` of a checkout of its own joins
/// between them by its turn
fn session_and_x_wait(params: &TurnParams<'_>, command: &[String]) {
  session_waits(params, command, &["s1", "s2"]);
  join_all(params, &["x"], |run| of(run, "checkout-x", command));
}

/// The runs of `entries` by their ids
fn ids(entries: &[TurnEntry]) -> Vec<&str> {
  entries.iter().map(|entry| entry.ticket.run.as_str()).collect()
}

/// A run of a higher priority goes ahead of the runs of its group that wait with a lower one, over the places of its
/// group only: the run of another group keeps its turn, and the head its place, as the turns never pass it
#[test]
fn a_high_run_goes_ahead_of_the_normal_runs_of_its_group_never_of_another_group_nor_the_head() {
  let dir = tempfile::tempdir().unwrap();
  let params = params(dir.path());
  let command = command();
  session_and_x_wait(&params, &command);
  assert_eq!(queue_of(dir.path()), ["s1", "x", "s2"]);
  let high = TurnRequest {
    priority: Some(Priority::High),
    ..grouped("s3", "session", &command)
  };
  let joined = join(&params, &high).unwrap();
  assert_eq!(ids(&joined.prioritized), ["s2"]);
  assert_eq!(queue_of(dir.path()), ["s1", "x", "s3", "s2"], "x keeps its turn and s1 its place");
  assert_eq!(read_groups(dir.path()).0.priority("s3"), Priority::High);
  assert_eq!(turn_passes(dir.path(), &["s2"]), [1], "the priority counts no pass of the turns");
}

/// The priority orders the runs of one class only: a long run of a high priority never takes the place of a short run
/// of its group, which went ahead in the short lane
#[test]
fn the_priority_never_moves_a_long_run_into_the_place_of_a_short_one() {
  let (dir, command) = waiting_session(&[]);
  let params = params(dir.path());
  join_all(&params, &["long-1", "long-2"], |run| grouped(run, "a", &command));
  join_all(&params, &["lint", "fmt"], |run| TurnRequest {
    group: Some("a"),
    ..short(run, &command, 40)
  });
  assert_eq!(queue_of(dir.path()), ["lint", "fmt", "long-1", "long-2"]);
  let high = TurnRequest {
    priority: Some(Priority::High),
    ..grouped("long-3", "a", &command)
  };
  take(&params, &high).unwrap();
  assert_eq!(queue_of(dir.path()), ["lint", "fmt", "long-3", "long-1", "long-2"]);
}

/// `prioritize` orders a queued run again inside its group; a later look of a run keeps the order, the head keeps its
/// place, and a holder and a run that the turn does not name change nothing
#[test]
fn prioritize_moves_a_queued_run_inside_its_group_and_leaves_a_holder_as_it_is() {
  let dir = tempfile::tempdir().unwrap();
  let params = params(dir.path());
  let command = command();
  session_and_x_wait(&params, &command);
  join_all(&params, &["s3"], |run| grouped(run, "session", &command));
  assert_eq!(queue_of(dir.path()), ["s1", "x", "s2", "s3"]);
  match prioritize(&params, "s3", Priority::High).unwrap() {
    Prioritized::Queued { from, to, ahead_of } => assert_eq!((from, to, ids(&ahead_of)), (3, 2, vec!["s2"])),
    other => panic!("{other:?}"),
  }
  assert_eq!(queue_of(dir.path()), ["s1", "x", "s3", "s2"]);
  assert_eq!(
    queued(&take(&params, &grouped("s2", "session", &command)).unwrap()).1,
    ["s1", "x", "s3"],
    "a look keeps the order"
  );
  match prioritize(&params, "s3", Priority::Low).unwrap() {
    Prioritized::Queued { from, to, ahead_of } => assert_eq!((from, to, ahead_of), (2, 3, vec![])),
    other => panic!("{other:?}"),
  }
  assert_eq!(queue_of(dir.path()), ["s1", "x", "s2", "s3"]);
  match prioritize(&params, "s1", Priority::Low).unwrap() {
    Prioritized::Queued { from, to, .. } => assert_eq!((from, to), (0, 0), "the head keeps its place"),
    other => panic!("{other:?}"),
  }
  assert_eq!(prioritize(&params, "holder", Priority::High).unwrap(), Prioritized::Holds);
  assert_eq!(prioritize(&params, "nobody", Priority::High).unwrap(), Prioritized::Unknown);
  assert_eq!(queue_of(dir.path()), ["s1", "x", "s2", "s3"]);
}

/// A request as `of_checkout` that runs `commit` and measures by the key `measure_key`
fn of_commit<'a>(run: &'a str, command: &'a [String], commit: &'a str, measure_key: &'a str) -> TurnRequest<'a> {
  TurnRequest {
    commit,
    measure_key,
    ..of_checkout(run, command)
  }
}

/// A later run of the checkout with another commit supersedes a waiting run of the same key of its measures, whatever
/// its command: the gate of an amended head, with another `--base` too. The same commit with another command of the
/// class (an A/B of two bases) and another checkout supersede nothing, and a waiting run that names no commit only by
/// its command; the later run keeps the priority of the superseded one unless it names one
#[test]
fn a_run_of_another_commit_supersedes_a_waiting_run_of_its_measures_and_keeps_its_priority() {
  let dir = tempfile::tempdir().unwrap();
  let params = params(dir.path());
  let (nextest, e2e) = (command(), vec!["pnpm".to_owned(), "e2e".to_owned()]);
  session_holds(&params, &nextest);
  let old = TurnRequest {
    priority: Some(Priority::High),
    ..of_commit("old", &e2e, "c1", "class-gate")
  };
  take(&params, &old).unwrap();
  let same_commit = join(&params, &of_commit("same-commit", &nextest, "c1", "class-gate")).unwrap();
  assert_eq!(ids(&same_commit.replaced), Vec::<&str>::new(), "one commit, two commands of the class: both run");
  let elsewhere = TurnRequest {
    affinity: "checkout-ba9876543210",
    ..of_commit("elsewhere", &nextest, "c2", "class-gate")
  };
  assert_eq!(ids(&join(&params, &elsewhere).unwrap().replaced), Vec::<&str>::new());
  let unnamed = TurnRequest {
    command_key: "key-of-another-command",
    ..of_commit("unnamed", &nextest, "", "class-gate")
  };
  assert_eq!(ids(&join(&params, &unnamed).unwrap().replaced), Vec::<&str>::new());

  let amended = join(&params, &of_commit("amended", &nextest, "c2", "class-gate")).unwrap();
  assert_eq!(ids(&amended.replaced), ["old", "same-commit"], "another commit of the class replaces both");
  assert_eq!(superseded(&read_live(dir.path()).unwrap()), [("old", "amended"), ("same-commit", "amended")]);
  assert_eq!(read_groups(dir.path()).0.priority("amended"), Priority::High, "the priority of the superseded run");
  assert_eq!(queue_of(dir.path()), ["amended", "elsewhere", "unnamed"], "the place of the first superseded run");

  let low = TurnRequest {
    priority: Some(Priority::Low),
    ..of_commit("again", &nextest, "c3", "class-gate")
  };
  assert_eq!(ids(&join(&params, &low).unwrap().replaced), ["amended"]);
  assert_eq!(read_groups(dir.path()).0.priority("again"), Priority::Low, "a named priority wins");
}

/// A request as `request` of a run without a measure, which the host assumes `tenths` of cores of
fn assumed<'a>(run: &'a str, command: &'a [String], tenths: u32) -> TurnRequest<'a> {
  TurnRequest {
    expected: Some(Cores(tenths)),
    expectation: Some(Expectation::Assumed),
    ..request(run, command)
  }
}

/// A run without a measure takes the cores the host assumes of one, not the whole host: it starts beside a measured
/// holder when they fit, and on a free turn without a quiet host. As a holder it holds the most it used since its start
/// when that is more, so a run that the default misjudged keeps the next runs out while it runs; the holder of a build
/// before the assumed cores, without a measure, still holds the whole host
#[test]
fn a_run_without_a_measure_takes_the_cores_the_host_assumes_of_one() {
  let dir = tempfile::tempdir().unwrap();
  let params = params(dir.path());
  let command = command();
  assert_eq!(take(&params, &measured("gate", &command, 80)).unwrap(), TurnAnswer::Taken);
  let new = assumed("new", &command, 100);
  assert_eq!(take(&params, &new).unwrap(), TurnAnswer::Taken, "8 and the assumed 10 fit 20");
  let table = read_live(dir.path()).unwrap();
  let holder = table.holders.iter().find(|holder| holder.ticket.run == "new").unwrap();
  assert_eq!(holder.need(20, now_ms()), Need::Assumed { cores: Cores(100) });
  assert_eq!(holder.expectation, Some(Expectation::Assumed));

  let pid = std::process::id();
  note_use(
    &params,
    &UseNote {
      run: "new",
      pid,
      process_started: sync_utils::process_scan::process_started(pid),
      repo: "/Users/u/projects/branch-deck",
      command: &command,
      live: Cores(170),
    },
  )
  .unwrap();
  assert!(leave(&params, "gate").unwrap());
  let TurnAnswer::Queued(queue) = take(&params, &measured("next", &command, 40)).unwrap() else {
    panic!("the 17 that the new run showed and 4 do not fit 20");
  };
  assert_eq!(queue.holders[0].need(20, now_ms()), Need::Assumed { cores: Cores(170) });

  assert!(leave(&params, "new").unwrap());
  let before = TurnEntry::default();
  assert_eq!(before.need(20, now_ms()), Need::Unmeasured, "an entry of a build before the assumed cores, no expectation");
  let rewritten = TurnEntry {
    expected: Some(Cores(100)),
    ..TurnEntry::default()
  };
  assert_eq!(
    rewritten.need(20, now_ms()),
    Need::Cores { cores: Cores(100), live: false },
    "an older build that dropped the expectation counts the assumed cores as a measure"
  );
}
