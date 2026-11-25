use super::*;
use pretty_assertions::assert_eq;
use sync_utils::cores::Cores;

fn params(dir: &Path) -> TurnParams<'_> {
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
fn command() -> Vec<String> {
  vec!["cargo".to_owned(), "nextest".to_owned(), "run".to_owned(), "--workspace".to_owned()]
}

/// The checkout of the requests of the tests
const CHECKOUT: &str = "checkout-0123456789ab";

/// A request of this process, whose pid lives as long as the test, from a checkout of its own (the run id), so no run
/// supersedes another
fn request<'a>(run: &'a str, command: &'a [String]) -> TurnRequest<'a> {
  let pid = std::process::id();
  TurnRequest {
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
  assert!(since_ms >= before && until_ms == since_ms + 900_000, "{since_ms} {until_ms}");
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
  let mut table: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(dir.path().join(STATE_FILE)).unwrap()).unwrap();
  for list in ["holders", "queue"] {
    for entry in table[list].as_array_mut().unwrap() {
      let entry = entry.as_object_mut().unwrap();
      entry.retain(|field, _| ["run", "pid", "process_started", "repo", "command", "since_ms", "expected", "live"].contains(&field.as_str()));
    }
  }
  write_table(dir.path(), &table);
  assert_eq!(passed_runs(&join(&params, &short("s5", &command, 40)).unwrap()), []);
  assert_eq!(runs(&read_live(dir.path()).unwrap()).1, ["s1", "s2", "s3", "s4", "long", "s5"]);
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
fn note_live(params: &TurnParams<'_>, run: &str, command: &[String], tenths: u32) {
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
