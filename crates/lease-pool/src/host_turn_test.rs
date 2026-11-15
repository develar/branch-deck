use super::*;
use pretty_assertions::assert_eq;

fn params(dir: &Path) -> TurnParams<'_> {
  TurnParams {
    dir,
    lock_timeout: Duration::from_secs(10),
    recheck: RECHECK,
  }
}

/// The command of each request of the tests
fn command() -> Vec<String> {
  vec!["cargo".to_owned(), "nextest".to_owned(), "run".to_owned(), "--workspace".to_owned()]
}

/// A request of this process, whose pid lives as long as the test
fn request<'a>(run: &'a str, command: &'a [String]) -> TurnRequest<'a> {
  let pid = std::process::id();
  TurnRequest {
    run,
    pid,
    process_started: sync_utils::process_scan::process_started(pid),
    repo: "/Users/u/projects/branch-deck",
    command,
  }
}

/// The holder and the runs of the queue
fn runs(table: &HeavyTurn) -> (Option<&str>, Vec<&str>) {
  (
    table.holder.as_ref().map(|holder| holder.ticket.run.as_str()),
    table.queue.iter().map(|entry| entry.ticket.run.as_str()).collect(),
  )
}

/// The holder and the runs ahead of a queued answer
fn queued(answer: &TurnAnswer) -> (Option<&str>, Vec<&str>) {
  match answer {
    TurnAnswer::Queued(queue) => (
      queue.holder.as_ref().map(|holder| holder.ticket.run.as_str()),
      queue.ahead.iter().map(|entry| entry.ticket.run.as_str()).collect(),
    ),
    TurnAnswer::Taken => panic!("the run took the turn"),
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
  let holder = table.holder.unwrap();
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
  };
  write_table(dir.path(), &serde_json::json!({ "schema": 1, "queue": [joined] }));
  let before = now_ms();
  assert_eq!(take(&params, &waited_long).unwrap(), TurnAnswer::Taken);
  let holder = read_live(dir.path()).unwrap().holder.unwrap();
  assert!(holder.since_ms >= before, "{} is the time it joined the queue", holder.since_ms);
}

/// A table of another schema, which an agent of another build wrote, refuses every step, and so does one without a
/// schema: no supervisor reads a turn that it does not know as free
#[test]
fn a_table_of_another_schema_refuses() {
  let dir = tempfile::tempdir().unwrap();
  let params = params(dir.path());
  let command = command();
  for table in [serde_json::json!({ "schema": 2, "queue": [] }), serde_json::json!({ "queue": [] })] {
    write_table(dir.path(), &table);
    let refused = take(&params, &request("run-1", &command)).unwrap_err().to_string();
    assert!(
      refused.contains("heavy.json has the schema ") && refused.contains(" of another build of brd-host, and this one reads 1"),
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
}
