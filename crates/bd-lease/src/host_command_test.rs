//! `bd lease host`: the commands of the remote-run agent of a host, end to end through the Cli. A lend gives a slot or
//! a place in the queue, a give-back frees the slot or leaves the queue, fill, reclaim, shrink, limit and status answer
//! the table, every output as `bd schema` names it, and the refusals by their codes

use crate::host_slots_test::{gone_process, host_fixture};
use crate::lease_test::Fixture;
use bd_cli::fixture_test::{assert_covered, assert_error};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

/// `bd lease host <args>`, which must succeed, its output checked against `bd schema`
fn host(fixture: &Fixture, output: &str, args: &[&str]) -> Value {
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
  if cfg!(any(target_os = "macos", target_os = "linux")) {
    assert_error(&refused, exit_code, (2, "invalid_arguments"));
  } else {
    assert_eq!(exit_code, 0, "{refused}");
  }
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
