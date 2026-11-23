use super::*;
use lease_pool::host_slots::watches_started;
use pretty_assertions::assert_eq;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc;
use test_utils::time_shape::{is_shape, starts_with_shape};

/// The lines of `brd-host runs` of an agent of the build before, which names one holder and no cores, say who holds the
/// turn and for how long, then each run that waits; a free turn says so
#[test]
fn the_lines_of_a_heavy_turn_of_the_build_before_name_the_holder_and_each_waiting_run() {
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
  assert_eq!(
    lines_at(&serde_json::json!({ "schema": 2 }), now).unwrap(),
    ["heavy turn: free"],
    "a turn that no run counted cores in"
  );
  let before = serde_json::json!({
    "schema": 2,
    "holders": [{ "run": "1788-12", "pid": 1, "repo": "/Users/u/projects/idea-1", "command": "bt --lane fast", "since_ms": now - 60_000 }],
  });
  assert_eq!(
    lines_at(&before, now).unwrap(),
    [
      "heavy turn: 1 run(s) hold the whole host; no run of this build counted its cores yet",
      "  run 1788-12 of idea-1 holds the whole host for 60 s (no measure): bt --lane fast",
    ],
    "the holder of the build before, ahead of the first run of this build"
  );
  assert_eq!(lines_at(&serde_json::json!({ "queue": 1 }), now).unwrap_err().code, Code::Link);
}

/// The lines of `brd-host runs` say the cores held of the cores of the host, each holder with what it holds, why and
/// for how long, each waiting run with what it waits for, and each run that is not heavy with its live cores
#[test]
fn the_lines_of_the_heavy_turn_name_the_cores_of_each_holder_waiting_run_and_light_run() {
  let now = 1_000_000_000;
  let table = serde_json::json!({
    "schema": 2,
    "cores": 20,
    "holders": [
      { "run": "1788-12", "pid": 1, "repo": "/Users/u/projects/branch-deck", "command": "pnpm lint:backend", "since_ms": now - 250_000, "expected": 6.5 },
      { "run": "1789-40", "pid": 1, "repo": "/Users/u/projects/accio", "command": "scripts/check.sh --smoke", "since_ms": now - 30_000, "expected": 4.0, "live": 7.5 },
    ],
    "queue": [
      { "run": "1790-03", "pid": 2, "repo": "/Users/u/projects/idea-1", "command": "./community/tools/bt.cmd --lane fast", "since_ms": now - 60_000 },
      { "run": "1790-04", "pid": 2, "repo": "/Users/u/projects/idea-1", "command": "./community/tools/bt.cmd --lane slow", "since_ms": now - 10_000, "expected": 20.0 },
      { "run": "1790-05", "pid": 2, "repo": "/Users/u/projects/branch-deck", "command": "pnpm e2e", "since_ms": now - 5_000, "expected": 9.0 },
    ],
    "light": [{ "run": "1791-01", "pid": 3, "repo": "/Users/u/projects/idea-1", "command": "./community/tools/bt.cmd --module x", "live": 2.0 }],
  });
  assert_eq!(
    lines_at(&table, now).unwrap(),
    [
      "heavy turn: 14.0 of 20 cores held",
      "  run 1788-12 of branch-deck holds 6.5 for 4 min (measured): pnpm lint:backend",
      "  run 1789-40 of accio holds 7.5 for 30 s (live, above its measure): scripts/check.sh --smoke",
      "  run 1790-03 of idea-1 waits 60 s for the whole host (no measure): ./community/tools/bt.cmd --lane fast",
      "  run 1790-04 of idea-1 waits 10 s for the whole host (measured 20.0): ./community/tools/bt.cmd --lane slow",
      "  run 1790-05 of branch-deck waits 5 s for 9.0 (measured): pnpm e2e",
      "  run 1791-01 of idea-1, not heavy, uses 2.0: ./community/tools/bt.cmd --module x",
    ]
  );
  assert_eq!(lines_at(&serde_json::json!({ "schema": 2, "cores": 20 }), now).unwrap(), ["heavy turn: free, 20 cores"]);
}

/// The lines of `brd-host runs` name the exclusive run that a holder started past, and the drain of an exclusive head of
/// the queue: whether a run can pass it yet. A reply of an agent before the pass names neither, and says as before
#[test]
fn the_lines_of_the_heavy_turn_name_the_drain_of_an_exclusive_head_and_who_passed_it() {
  let now = 1_000_000_000;
  let table = |head_drain: serde_json::Value| {
    serde_json::json!({
      "schema": 2,
      "cores": 20,
      "holders": [{ "run": "1789-40", "pid": 1, "repo": "/Users/u/projects/branch-deck", "command": "pnpm lint:backend", "since_ms": now - 120_000, "expected": 6.5, "passed": "1788-12" }],
      "queue": [
        { "run": "1788-12", "pid": 2, "repo": "/Users/u/projects/idea-1", "command": "zsh ./measure-ab.sh", "since_ms": now - 2_400_000, "exclusive": true },
        { "run": "1790-05", "pid": 2, "repo": "/Users/u/projects/branch-deck", "command": "pnpm e2e", "since_ms": now - 600_000, "expected": 9.0 },
      ],
      "head_drain": head_drain,
    })
  };
  assert_eq!(
    lines_at(&table(serde_json::json!({ "state": "ended", "ended_ms": now - 300_000 })), now).unwrap(),
    [
      "heavy turn: 6.5 of 20 cores held",
      "  run 1789-40 of branch-deck holds 6.5 for 2 min (measured, past the exclusive run 1788-12): pnpm lint:backend",
      "  run 1788-12 of idea-1 waits 40 min for a quiet host (exclusive; its drain ended 5 min ago without one, and the heavy runs behind it start past it while they fit): zsh ./measure-ab.sh",
      "  run 1790-05 of branch-deck waits 10 min for 9.0 (measured): pnpm e2e",
    ]
  );
  assert_eq!(
    lines_at(&table(serde_json::json!({ "state": "draining", "until_ms": now + 540_000 })), now).unwrap()[2],
    "  run 1788-12 of idea-1 waits 40 min for a quiet host (exclusive; it drains the host for 9 min more, and no run starts past it before then): zsh ./measure-ab.sh"
  );
  assert_eq!(
    lines_at(&table(serde_json::json!({ "state": "pending" })), now).unwrap()[2],
    "  run 1788-12 of idea-1 waits 40 min for a quiet host (exclusive; it drains the host once no heavy run holds cores, and no run starts past it before then): zsh ./measure-ab.sh"
  );
  assert_eq!(
    lines_at(&table(serde_json::Value::Null), now).unwrap()[2],
    "  run 1788-12 of idea-1 waits 40 min for a quiet host (exclusive): zsh ./measure-ab.sh",
    "an agent before the pass"
  );
}

/// The line after the wait names the exclusive run that the run started past, at once too, and the sample of the host
/// that its admission read; a turn taken at once says nothing
#[test]
fn the_line_after_the_wait_names_the_exclusive_run_that_the_run_passed() {
  let exclusive = TurnEntry {
    ticket: lease_pool::host_slots::Ticket {
      run: "1788-12".to_owned(),
      pid: 1,
      process_started: None,
    },
    repo: "/Users/u/projects/idea-1".to_owned(),
    command: "zsh ./measure-ab.sh".to_owned(),
    since_ms: 1,
    affinity: None,
    command_key: None,
    expected: None,
    live: None,
    exclusive: true,
    passed: None,
  };
  assert_eq!(took_line(Duration::from_millis(300), Some(Cores(50)), 20, None), None);
  assert_eq!(
    took_line(Duration::from_secs(75), Some(Cores(50)), 20, None).as_deref(),
    Some("took the heavy turn of the host after 75 s, the host busy with 5.0 of its 20 cores")
  );
  assert_eq!(
    took_line(Duration::from_millis(300), None, 20, Some(&exclusive)).as_deref(),
    Some("took the heavy turn of the host after 0 s, past the exclusive run 1788-12 of idea-1 at the head of the queue, whose drain ended without a quiet host")
  );
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
    cores: 20,
    exclusive: false,
    fetch: &[],
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
  // the first line has waited nothing; how long run-1 holds is measured
  assert!(
    starts_with_shape(
      &said[0],
      "run run-2 waits 0 s for the heavy turn of the host, all its 20 cores, as the command has no measure yet, 60 s left: run run-1 of branch-deck holds the whole host for <time> (pnpm e2e, no measure), no run ahead; a wait, not a hang: do not cancel it and do not run it on the client; `brd-host runs <host>` shows the heavy turn"
    ),
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
  assert_eq!(lines(&live(&home).unwrap()).unwrap(), ["heavy turn: free, 20 cores"]);
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
    cores: 20,
    exclusive: false,
    fetch: &[],
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
  // how long old and other waited is measured
  assert!(
    is_shape(
      &said[0],
      "replaces run old, which waited <time> for the heavy turn with the same checkout and command: it ends with `superseded` and runs nothing, and this run takes its place in the queue"
    ),
    "{said:?}"
  );
  assert!(
    is_shape(
      &said[1],
      "run other of this checkout also waits for the heavy turn, <time>: pnpm lint. Both run, one after the other; if this run replaces it, cancel it: `brd-host cancel <host> other`"
    ),
    "{said:?}"
  );
  assert!(
    said[2].starts_with(
      "run new waits 0 s for the heavy turn of the host, all its 20 cores, as the command has no measure yet, 60 s left: run holder of branch-deck holds the whole host for "
    ),
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

/// A run that is not heavy waits while an exclusive run holds the turn, says what it waits for at once, ends its wait
/// when its time is up or a stop comes, and goes on at once when no exclusive run holds
#[test]
fn a_run_that_is_not_heavy_waits_for_the_end_of_an_exclusive_run() {
  let dir = tempfile::tempdir().unwrap();
  let home = Home::new(dir.path().to_path_buf());
  let command = vec!["zsh".to_owned(), "./measure-ab.sh".to_owned()];
  let never = || false;
  let mut quiet = |_: &str| {};
  let exclusive = TurnRun {
    run: "ab",
    repo: "/r/idea-1",
    affinity: "checkout-0123456789ab",
    command: &command,
    cores: 20,
    exclusive: true,
    fetch: &[],
  };
  assert_eq!(take(&home, &exclusive, Duration::ZERO, &never, &mut quiet).unwrap(), Some(TurnAnswer::Taken));
  let shown = lines(&live(&home).unwrap()).unwrap();
  assert!(
    shown
      .iter()
      .any(|line| line.starts_with("  run ab of idea-1 holds the whole host for a measurement for ") && line.ends_with(" (exclusive): zsh ./measure-ab.sh")),
    "{shown:?}"
  );
  assert_eq!(wait_for_measurement(&home, "light", Duration::ZERO, &never, &mut quiet), Some(false), "the time is up");
  let mut said = Vec::new();
  let at_once = || true;
  assert_eq!(
    wait_for_measurement(&home, "light", Duration::from_mins(1), &at_once, &mut |line| said.push(line.to_owned())),
    None
  );
  assert!(said.is_empty(), "a stop before the first line: {said:?}");
  let mut said = Vec::new();
  let looks = AtomicU32::new(0);
  let second_look = || looks.fetch_add(1, Ordering::Relaxed) >= 1;
  assert_eq!(
    wait_for_measurement(&home, "light", Duration::from_mins(1), &second_look, &mut |line| said.push(line.to_owned())),
    None
  );
  assert_eq!(said.len(), 1, "{said:?}");
  assert!(
    starts_with_shape(&said[0], "run light waits <time> for a measurement: run ab of idea-1 holds the host for ")
      && said[0].ends_with(" (zsh ./measure-ab.sh); a measurement holds the host; this run starts when it ends"),
    "{said:?}"
  );
  assert!(leave(&home, "ab").unwrap());
  assert_eq!(wait_for_measurement(&home, "light", Duration::from_mins(1), &never, &mut quiet), Some(true));
}

/// A look that finds the lock of the table held by another process costs the look, not the run: a run that asks for its
/// first place asks again, a run of the queue keeps its place ahead of a later run, each says so at once, and the run
/// takes the turn once the lock is free and the holder leaves
#[test]
fn a_held_lock_of_the_table_retries_and_the_run_keeps_its_place_in_the_queue() {
  let dir = tempfile::tempdir().unwrap();
  let home = Home::new(dir.path().to_path_buf());
  let command = vec!["pnpm".to_owned(), "e2e".to_owned()];
  let run = |run| TurnRun {
    run,
    repo: "/r/branch-deck",
    affinity: run,
    command: &command,
    cores: 20,
    exclusive: false,
    fetch: &[],
  };
  let never = || false;
  let mut quiet = |_: &str| {};
  assert_eq!(take(&home, &run("holder"), Duration::ZERO, &never, &mut quiet).unwrap(), Some(TurnAnswer::Taken));
  // the lock as another process holds it: a lock of a file of its own, outside the line of the locks of this process
  let dir = home.heavy_dir();
  let lock = std::fs::OpenOptions::new().write(true).open(dir.join(lease_pool::host_turn::LOCK_FILE)).unwrap();
  lock.lock().unwrap();
  let (said, lines) = mpsc::channel::<String>();
  std::thread::scope(|scope| {
    let waiter = scope.spawn(|| {
      let mut say = |line: &str| said.send(line.to_owned()).unwrap();
      take_with(&home, &run("first"), Duration::from_mins(1), Duration::from_millis(100), &never, &mut say).unwrap()
    });
    let held = lines.recv().unwrap();
    assert!(
      starts_with_shape(&held, "the heavy turn of the host is locked by another process of the host for <time> (")
        && held.ends_with("turn.lock); this run asks again for its place in the queue"),
      "{held}"
    );
    lock.unlock().unwrap();
    let queued = lines.recv().unwrap();
    assert!(queued.starts_with("run first waits 0 s for the heavy turn of the host"), "{queued}");
    let TurnAnswer::Queued(later) = take(&home, &run("later"), Duration::ZERO, &never, &mut quiet).unwrap().unwrap() else {
      panic!("the holder holds the turn");
    };
    assert_eq!(later.ahead.iter().map(|entry| entry.ticket.run.as_str()).collect::<Vec<_>>(), ["first"]);

    lock.lock().unwrap();
    // a change of the table under the lock wakes the wait of the run, whose look finds the lock held
    let table = std::fs::read(dir.join("turn.json")).unwrap();
    sync_utils::atomic_file::write_atomically(&dir.join("turn.json"), &table, sync_utils::atomic_file::AtomicWrite::default()).unwrap();
    let held = lines.recv().unwrap();
    assert!(held.ends_with("turn.lock); this run keeps its place in the queue and looks again"), "{held}");
    lock.unlock().unwrap();
    assert!(leave(&home, "holder").unwrap());
    assert_eq!(waiter.join().unwrap(), Some(TurnAnswer::Taken), "no refusal: the lock cost looks only");
  });
  let table = live(&home).unwrap();
  assert_eq!((table["holder"]["run"].as_str(), table["queue"][0]["run"].as_str()), (Some("first"), Some("later")));
}

/// A measure of the command that comes while a run waits counts at its next look, also the measure of a run that wrote
/// its outputs to another directory that it fetched
#[test]
fn a_measure_that_comes_while_a_run_waits_counts_at_its_next_look() {
  let dir = tempfile::tempdir().unwrap();
  let home = Home::new(dir.path().to_path_buf());
  let lint = vec!["pnpm".to_owned(), "lint".to_owned()];
  let lane = |out: &str| {
    vec![
      "zsh".to_owned(),
      "-lc".to_owned(),
      format!("mkdir -p {out} && ./community/tools/vm.cmd --stream run --lane ui-light > {out}/run.json"),
    ]
  };
  let (light1, light5) = (lane("out/air/light1"), lane("out/air/light5"));
  let (fetch1, fetch5) = (vec!["out/air/light1".to_owned()], vec!["out/air/light5".to_owned()]);
  let run = |run, command, fetch| TurnRun {
    run,
    repo: "/r/idea-1",
    affinity: run,
    command,
    cores: 20,
    exclusive: false,
    fetch,
  };
  let never = || false;
  let mut quiet = |_: &str| {};
  assert_eq!(
    take(&home, &run("holder", &lint, &[]), Duration::ZERO, &never, &mut quiet).unwrap(),
    Some(TurnAnswer::Taken)
  );
  let mut said = Vec::new();
  let looks = AtomicU32::new(0);
  // before its first look the other run of the command ends with its measure, and the second look stops
  let stop = || match looks.fetch_add(1, Ordering::SeqCst) {
    0 => {
      record(&home, &run("earlier", &light1, &fetch1), Cores(15), Cores(136), Duration::from_mins(1)).unwrap();
      false
    }
    _ => true,
  };
  let waited = take(&home, &run("waiting", &light5, &fetch5), Duration::from_mins(1), &stop, &mut |line| {
    said.push(line.to_owned());
  })
  .unwrap();
  assert_eq!(waited, None, "the stop ended the wait");
  assert!(said[0].contains(", all its 20 cores, as the command has no measure yet, "), "{said:?}");
  let table = live(&home).unwrap();
  assert_eq!(
    (table["queue"][0]["run"].as_str(), table["queue"][0]["expected"].as_f64()),
    (Some("waiting"), Some(1.5)),
    "the look after the measure expects it"
  );
}
