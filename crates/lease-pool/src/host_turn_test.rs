use super::*;
use pretty_assertions::assert_eq;
use sync_utils::cores::Cores;

fn params(dir: &Path) -> TurnParams<'_> {
  TurnParams {
    dir,
    lock_timeout: Duration::from_secs(10),
    recheck: RECHECK,
    cores: 20,
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
    queue.holders.iter().map(|holder| holder.need(20)).collect::<Vec<_>>(),
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
  assert_eq!(queue.holders[0].need(20), Need::Cores { cores: Cores(140), live: true });
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
  let bench = vec!["scripts/bench".to_owned(), "ab-run".to_owned()];
  let pid = std::process::id();
  note_use(
    &params,
    &UseNote {
      run: "bench",
      pid,
      process_started: sync_utils::process_scan::process_started(pid),
      repo: "/Users/u/projects/accio",
      command: &bench,
      live: Cores(0),
    },
  )
  .unwrap();
  quiet_for_30_s(dir.path(), Cores(10));
  let exclusive = TurnRequest {
    exclusive: true,
    ..request("ab", &command)
  };
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
  let exclusive = TurnRequest {
    exclusive: true,
    ..request("ab", &command)
  };
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
  assert_eq!((queue.holders[0].ticket.run.as_str(), queue.holders[0].need(20)), ("old-build", Need::Unmeasured));
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
