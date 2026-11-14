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
  let first = take(&home, "run-1", "/r/branch-deck", &command, Duration::from_mins(1), &never, &mut quiet).unwrap();
  assert_eq!(first, Some(TurnAnswer::Taken));

  let mut said = Vec::new();
  let looks = AtomicU32::new(0);
  // the wait asks `stop` once before each look: two looks of 2 s, then the stop
  let stop = || looks.fetch_add(1, Ordering::SeqCst) >= 2;
  let before = watches_started();
  let waited = take(&home, "run-2", "/r/idea-1", &command, Duration::from_mins(1), &stop, &mut |line| said.push(line.to_owned())).unwrap();
  assert_eq!(waited, None, "the stop ended the wait");
  assert_eq!(watches_started() - before, 1, "one watch of the table for the whole wait");
  assert_eq!(said.len(), 1, "{said:?}");
  assert!(
    said[0].starts_with("run run-2 waits 0 s for the heavy turn of the host, 60 s left: run run-1 of branch-deck holds it 0 s (pnpm e2e), no run ahead"),
    "{said:?}"
  );
  let table = live(&home).unwrap();
  assert_eq!((table["holder"]["run"].as_str(), table["queue"][0]["run"].as_str()), (Some("run-1"), Some("run-2")));

  assert_eq!(
    take(&home, "run-1", "/r/branch-deck", &command, Duration::ZERO, &never, &mut quiet).unwrap(),
    Some(TurnAnswer::Taken)
  );
  assert!(!leave(&home, "run-2").unwrap(), "a ticket is no turn");
  assert!(leave(&home, "run-1").unwrap());
  assert_eq!(lines(&live(&home).unwrap()).unwrap(), ["heavy turn: free"]);
}
