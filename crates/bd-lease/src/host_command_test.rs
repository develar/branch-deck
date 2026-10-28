//! `bd lease host`: the commands of the remote-run agent of a host, end to end through the Cli. A lend gives a slot or
//! a place in the queue, a give-back frees the slot or leaves the queue, fill, reclaim, shrink, limit and status answer
//! the table, every output as `bd schema` names it, and the refusals by their codes

use crate::host_slots_test::{gone_process, host_fixture};
use crate::lease_test::Fixture;
use bd_cli::fixture_test::{assert_covered, assert_error};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

/// `bd lease host <args>`, which must succeed, its output checked against `bd schema`
pub(crate) fn host(fixture: &Fixture, output: &str, args: &[&str]) -> Value {
  let (exit_code, value) = fixture.run(&[&["lease", "host"][..], args].concat());
  assert_eq!(exit_code, 0, "{value}");
  let (_, schema) = fixture.run(&["schema", "--all"]);
  let schema = &schema["outputs"][output];
  assert_covered(schema, schema, &value, output);
  value
}

/// The lend of `run` to this process, which lives as long as the test
fn lend(fixture: &Fixture, run: &str, affinity: &str) -> Value {
  host(
    fixture,
    "lease_host_lend",
    &["lend", "--run", run, "--pid", &std::process::id().to_string(), "--affinity", affinity],
  )
}

/// fill, lend, give-back and status: the slot of a run, the queue behind it, the slot again for the run that waited
#[test_log::test]
fn a_run_lends_a_slot_waits_in_line_and_gives_it_back() {
  let fixture = host_fixture(1);
  let filled = host(&fixture, "lease_host_fill", &["fill"]);
  assert_eq!(filled["filled"].as_array().map(Vec::len), Some(1), "{filled}");

  let first = lend(&fixture, "run-1", "checkout-a");
  assert_eq!((&first["state"], &first["slot"]["n"]), (&json!("lent"), &json!(1)), "{first}");
  assert_eq!(lend(&fixture, "run-1", "checkout-a")["slot"]["n"], json!(1), "a repeat gets the same slot");
  let waiting = lend(&fixture, "run-2", "checkout-b");
  assert_eq!(
    (&waiting["state"], &waiting["ahead"], &waiting["lent"]),
    (&json!("queued"), &json!(0), &json!([{ "n": 1, "run": "run-1" }])),
    "{waiting}"
  );
  let status = host(&fixture, "lease_host_status", &["status"]);
  assert_eq!(
    (&status["slots"][0]["state"], &status["slots"][0]["run"], &status["queue"]),
    (&json!("lent"), &json!("run-1"), &json!(["run-2"])),
    "{status}"
  );

  assert_eq!(first["slot"]["generation"], json!(0));
  let path = first["slot"]["path"].as_str().unwrap().to_string();
  std::fs::write(std::path::Path::new(&path).join("left.txt"), "a run left it\n").unwrap();
  let gave_back = host(&fixture, "lease_host_give_back", &["give-back", "--run", "run-1"]);
  assert_eq!(
    (&gave_back["n"], &gave_back["left_queue"], &gave_back["discarded"]),
    (&json!(1), &json!(false), &json!(1)),
    "{gave_back}"
  );
  let second = lend(&fixture, "run-2", "checkout-b");
  assert_eq!((&second["state"], &second["slot"]["generation"]), (&json!("lent"), &json!(1)), "{second}");
  let left = host(&fixture, "lease_host_give_back", &["give-back", "--run", "run-3"]);
  assert_eq!(
    (&left["n"], &left["left_queue"]),
    (&Value::Null, &json!(true)),
    "a run with no slot leaves the queue: {left}"
  );
}

/// reclaim gives back the lend of a gone process; shrink returns the slots and keeps the count as the limit of the
/// fills, which `limit --unset` lifts
#[test_log::test]
fn reclaim_shrink_and_limit_answer_the_table() {
  let fixture = host_fixture(1);
  host(&fixture, "lease_host_fill", &["fill"]);
  let (pid, _) = gone_process();
  let (exit_code, refused) = fixture.run(&["lease", "host", "lend", "--run", "gone", "--pid", &pid.to_string()]);
  assert_error(&refused, exit_code, (2, "invalid_arguments"));
  let reclaimed = host(&fixture, "lease_host_reclaim", &["reclaim"]);
  assert!(reclaimed["reclaimed"].is_array(), "{reclaimed}");

  let shrunk = host(&fixture, "lease_host_shrink", &["shrink", "0"]);
  assert_eq!(
    shrunk["shrunk"],
    json!([{ "n": 1, "path": shrunk["shrunk"][0]["path"], "outcome": "returned" }]),
    "{shrunk}"
  );
  assert_eq!(host(&fixture, "lease_host_fill", &["fill"])["filled"], json!([]), "the limit of the shrink holds");
  assert_eq!(host(&fixture, "lease_host_limit", &["limit", "--unset"])["limit"], Value::Null);
  assert_eq!(host(&fixture, "lease_host_limit", &["limit", "2"])["limit"], json!(2));
  assert_eq!(
    host(&fixture, "lease_host_fill", &["fill"])["filled"].as_array().map(Vec::len),
    Some(1),
    "the config keeps one slot"
  );
}

/// The refusals of the table by their codes: a give-back of a slot lent to another run is `no_lease`, a bad
/// `branchdeck.hostSlots` is `invalid_arguments`
#[test_log::test]
fn the_refusals_of_the_table_have_their_codes() {
  let fixture = host_fixture(1);
  bd_cli::fixture_test::git(&fixture.repo, &["config", "branchdeck.hostSlots", "many"]);
  let (exit_code, refused) = fixture.run(&["lease", "host", "fill"]);
  assert_error(&refused, exit_code, (2, "invalid_arguments"));
  let (exit_code, refused) = fixture.run(&["lease", "host", "limit"]);
  assert_error(&refused, exit_code, (2, "invalid_arguments"));
}

/// checkout: the slot of a run moves to the commit of the run by the diff, stays at it on a repeat, and checks out every
/// file with git's reason when an untracked file is in a path of the diff; a run without a slot is `no_lease`, a commit
/// that is not a full id `invalid_arguments`
#[test_log::test]
fn a_run_checks_out_its_commit_in_its_slot_by_the_diff() {
  let fixture = host_fixture(1);
  host(&fixture, "lease_host_fill", &["fill"]);
  let lent = lend(&fixture, "run-1", "checkout-a");
  let path = std::path::PathBuf::from(lent["slot"]["path"].as_str().unwrap());
  let checkout = |commit: &str| host(&fixture, "lease_host_checkout", &["checkout", "--run", "run-1", "--commit", commit]);

  let first = fixture.repo.create_commit("Add c", "c.txt", "c\n");
  let moved = checkout(&first);
  assert_eq!((&moved["n"], &moved["moved"], &moved["commit"]), (&json!(1), &json!("switched"), &json!(first)), "{moved}");
  assert_eq!(std::fs::read_to_string(path.join("c.txt")).unwrap(), "c\n");
  assert_eq!(checkout(&first)["moved"], json!("kept"));

  let second = fixture.repo.create_commit("Add d", "d.txt", "d\n");
  std::fs::write(path.join("d.txt"), "in the way\n").unwrap();
  let forced = checkout(&second);
  assert_eq!(forced["moved"], json!("forced"), "{forced}");
  assert!(forced["reason"].as_str().unwrap().contains("d.txt"), "{forced}");
  assert_eq!(std::fs::read_to_string(path.join("d.txt")).unwrap(), "d\n");

  let (exit_code, refused) = fixture.run(&["lease", "host", "checkout", "--run", "run-2", "--commit", &second]);
  assert_error(&refused, exit_code, (2, "no_lease"));
  let (exit_code, refused) = fixture.run(&["lease", "host", "checkout", "--run", "run-1", "--commit", &second[..8]]);
  assert_error(&refused, exit_code, (2, "invalid_arguments"));
}

/// lend --wait: a run that waits behind a lent slot gets it once the slot is given back, without a second call; a wait
/// whose time is up answers its place in the queue with the time it waited; a process that ends while it waits is
/// `invalid_arguments`, and its ticket leaves the queue
#[test_log::test]
fn a_lend_that_waits_gets_the_slot_once_it_is_given_back() {
  let fixture = host_fixture(1);
  host(&fixture, "lease_host_fill", &["fill"]);
  lend(&fixture, "run-1", "checkout-a");
  let pid = std::process::id().to_string();
  let queue = || host(&fixture, "lease_host_status", &["status"])["queue"].clone();

  let timed_out = host(&fixture, "lease_host_lend", &["lend", "--run", "run-2", "--pid", &pid, "--wait", "1"]);
  assert_eq!((&timed_out["state"], &timed_out["ahead"]), (&json!("queued"), &json!(0)), "{timed_out}");
  assert!(timed_out["waited_ms"].as_u64().is_some_and(|waited| waited >= 1000), "{timed_out}");
  assert_eq!(queue(), json!(["run-2"]), "the ticket stays while its process lives");

  let waited = std::thread::scope(|scope| {
    let waiting = scope.spawn(|| host(&fixture, "lease_host_lend", &["lend", "--run", "run-3", "--pid", &pid, "--wait", "120"]));
    while queue() != json!(["run-2", "run-3"]) {
      std::thread::yield_now();
    }
    // run-2 leaves the queue, then the slot comes free
    host(&fixture, "lease_host_give_back", &["give-back", "--run", "run-2"]);
    host(&fixture, "lease_host_give_back", &["give-back", "--run", "run-1"]);
    waiting.join().unwrap()
  });
  assert_eq!((&waited["state"], &waited["slot"]["n"]), (&json!("lent"), &json!(1)), "{waited}");
  assert!(waited["waited_ms"].is_u64(), "{waited}");

  let mut child = crate::lease_test::Killed(sleeping());
  let child_pid = child.0.id().to_string();
  let (exit_code, refused) = std::thread::scope(|scope| {
    let waiting = scope.spawn(|| fixture.run(&["lease", "host", "lend", "--run", "run-4", "--pid", &child_pid, "--wait", "120"]));
    while queue() != json!(["run-4"]) {
      std::thread::yield_now();
    }
    let _ = child.0.kill();
    let _ = child.0.wait();
    waiting.join().unwrap()
  });
  assert_error(&refused, exit_code, (2, "invalid_arguments"));
  assert_eq!(queue(), json!([]), "the ticket of a gone process left the queue");
}

/// With `branchdeck.leaseMaintenance` a give-back starts git's maintenance, also for a run without a slot; without it
/// none
#[test_log::test]
fn a_give_back_starts_the_maintenance_that_branchdeck_lease_maintenance_asks_for() {
  let fixture = host_fixture(1);
  host(&fixture, "lease_host_fill", &["fill"]);
  lend(&fixture, "run-1", "checkout-a");
  let gave_back = host(&fixture, "lease_host_give_back", &["give-back", "--run", "run-1"]);
  assert_eq!(gave_back.get("maintenance_started"), None, "{gave_back}");

  bd_cli::fixture_test::git(&fixture.repo, &["config", "branchdeck.leaseMaintenance", "true"]);
  lend(&fixture, "run-2", "checkout-a");
  let gave_back = host(&fixture, "lease_host_give_back", &["give-back", "--run", "run-2"]);
  assert_eq!((&gave_back["n"], &gave_back["maintenance_started"]), (&json!(1), &json!(true)), "{gave_back}");
  let left = host(&fixture, "lease_host_give_back", &["give-back", "--run", "run-3"]);
  assert_eq!((&left["left_queue"], &left["maintenance_started"]), (&json!(true), &json!(true)), "{left}");
}

/// A process that sleeps for 30 s
fn sleeping() -> std::process::Child {
  let mut command = if cfg!(windows) {
    let mut command = std::process::Command::new("powershell");
    command.args(["-NoProfile", "-Command", "Start-Sleep 30"]);
    command
  } else {
    let mut command = std::process::Command::new("sleep");
    command.arg("30");
    command
  };
  command.spawn().unwrap()
}
