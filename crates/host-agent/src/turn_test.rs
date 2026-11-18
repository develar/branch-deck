use super::*;
use lease_pool::host_slots::watches_started;
use pretty_assertions::assert_eq;
use std::sync::atomic::{AtomicU32, Ordering};

/// The lines of `brd-host runs` name who holds the turn and for how long, then each run that waits and for how long; a
/// free turn says so
#[test]
fn the_lines_of_the_heavy_turn_name_the_holder_and_each_waiting_run_with_their_time() {
  let now = 1_000_000_000;
  let table = serde_json::json!({
    "holder": { "run": "1788-12", "pid": 1, "repo": "/Users/u/projects/branch-deck", "command": "cargo nextest run --workspace", "since_ms": now - 250_000 },
    "queue": [
      { "run": "1790-03", "pid": 2, "repo": "/Users/u/projects/idea-1", "command": "./community/tools/bt.cmd --lane fast", "since_ms": now - 60_000 },
    ],
  });
  assert_eq!(
    lines_at(&table, now).unwrap(),
    [
      "heavy turn: run 1788-12 of branch-deck holds it 4 min: cargo nextest run --workspace",
      "  run 1790-03 of idea-1 waits 60 s: ./community/tools/bt.cmd --lane fast",
    ]
  );
  assert_eq!(lines_at(&serde_json::json!({}), now).unwrap(), ["heavy turn: free"]);
  assert_eq!(lines_at(&serde_json::json!({ "queue": 1 }), now).unwrap_err().code, Code::Link);
}

/// A heavy run whose turn another run holds says the line of its wait at once, keeps one watch of the table for all the
/// looks of its wait, and a stop ends the wait, its ticket kept for the end of the run to take out; the run that holds
/// the turn takes it again at once
#[test]
fn a_heavy_run_says_its_wait_at_once_and_a_stop_ends_it() {
  let dir = tempfile::tempdir().unwrap();
  let home = Home::new(dir.path().to_path_buf());
  let command = vec!["pnpm".to_owned(), "e2e".to_owned()];
  let never = || false;
  let mut quiet = |_: &str| {};
  let run = |run, repo| TurnRun {
    run,
    repo,
    affinity: "checkout-0123456789ab",
    command: &command,
  };
  let first = take(&home, &run("run-1", "/r/branch-deck"), Duration::from_mins(1), &never, &mut quiet).unwrap();
  assert_eq!(first, Some(TurnAnswer::Taken));

  let mut said = Vec::new();
  let looks = AtomicU32::new(0);
  // the wait asks `stop` once before each look: two looks of 2 s, then the stop
  let stop = || looks.fetch_add(1, Ordering::SeqCst) >= 2;
  let before = watches_started();
  let waited = take(&home, &run("run-2", "/r/idea-1"), Duration::from_mins(1), &stop, &mut |line| said.push(line.to_owned())).unwrap();
  assert_eq!(waited, None, "the stop ended the wait");
  assert_eq!(watches_started() - before, 1, "one watch of the table for the whole wait");
  assert_eq!(said.len(), 1, "{said:?}");
  assert!(
    said[0].starts_with("run run-2 waits 0 s for the heavy turn of the host, 60 s left: run run-1 of branch-deck holds it 0 s (pnpm e2e), no run ahead; a wait, not a hang: do not cancel it and do not run it on the client; `brd-host runs <host>` shows the heavy turn"),
    "{said:?}"
  );
  let table = live(&home).unwrap();
  assert_eq!((table["holder"]["run"].as_str(), table["queue"][0]["run"].as_str()), (Some("run-1"), Some("run-2")));

  assert_eq!(
    take(&home, &run("run-1", "/r/branch-deck"), Duration::ZERO, &never, &mut quiet).unwrap(),
    Some(TurnAnswer::Taken)
  );
  assert!(!leave(&home, "run-2").unwrap(), "a ticket is no turn");
  assert!(leave(&home, "run-1").unwrap());
  assert_eq!(lines(&live(&home).unwrap()).unwrap(), ["heavy turn: free"]);
}

/// A later run of the same checkout and command says that it replaces the waiting one and for how long that waited,
/// and names a run of the checkout that waits with another command; the superseded run learns the later one at its next
/// look, and its message says that nothing ran and how to follow the later run
#[test]
fn a_later_run_says_whom_it_replaces_and_who_else_of_its_checkout_waits() {
  let dir = tempfile::tempdir().unwrap();
  let home = Home::new(dir.path().to_path_buf());
  let (e2e, lint) = (vec!["pnpm".to_owned(), "e2e".to_owned()], vec!["pnpm".to_owned(), "lint".to_owned()]);
  let run = |run, command| TurnRun {
    run,
    repo: "/r/branch-deck",
    affinity: "checkout-0123456789ab",
    command,
  };
  let never = || false;
  let at_once = || true;
  let mut quiet = |_: &str| {};
  assert_eq!(take(&home, &run("holder", &lint), Duration::ZERO, &never, &mut quiet).unwrap(), Some(TurnAnswer::Taken));
  for waiting in [run("old", &e2e), run("other", &lint)] {
    assert_eq!(take(&home, &waiting, Duration::from_mins(1), &at_once, &mut quiet).unwrap(), None, "it waits");
  }
  let mut said = Vec::new();
  assert_eq!(
    take(&home, &run("new", &e2e), Duration::from_mins(1), &at_once, &mut |line| said.push(line.to_owned())).unwrap(),
    None
  );
  assert_eq!(said.len(), 3, "{said:?}");
  assert_eq!(
    said[0],
    "replaces run old, which waited 0 s for the heavy turn with the same checkout and command: it ends with `superseded` and runs nothing, and this run takes its place in the queue"
  );
  assert_eq!(
    said[1],
    "run other of this checkout also waits for the heavy turn, 0 s: pnpm lint. Both run, one after the other; if this run replaces it, cancel it: `brd-host cancel <host> other`"
  );
  assert!(
    said[2].starts_with("run new waits 0 s for the heavy turn of the host, 60 s left: run holder of branch-deck holds it "),
    "{said:?}"
  );
  assert_eq!(
    take(&home, &run("old", &e2e), Duration::from_mins(1), &never, &mut quiet).unwrap(),
    Some(TurnAnswer::Superseded { by: "new".to_owned() })
  );
  assert_eq!(
    superseded_message("old", "new"),
    "run old was superseded by run new of the same checkout and command before it started: nothing ran; do not run it again, here or on the client; follow new: `brd-host follow <host> new`"
  );
}
