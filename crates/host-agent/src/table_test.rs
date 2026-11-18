use pretty_assertions::assert_eq;
use serde_json::json;

use super::*;

/// A table of the agent reads back with the type of the pool, and its lines name the limit, the queue and each slot
#[test]
fn the_lines_of_a_table_name_the_limit_the_queue_and_each_slot() {
  let table = parse(&json!({
    "slots": [
      { "n": 1, "lease_id": "l1", "path": "/pool/1/repo", "affinity": "checkout-0123456789ab", "state": { "kind": "idle" } },
      { "n": 2, "lease_id": "l2", "path": "/pool/2/repo", "state": { "kind": "lent", "run": "r-1", "pid": 4242, "since_ms": 0 } }
    ],
    "queue": [{ "run": "r-2", "pid": 4343 }],
    "limit": 2
  }))
  .expect("a table");
  let lines = lines("~/projects/repo", &table, &[]);
  assert_eq!(lines[0], "~/projects/repo: 2 slots, limit 2, queue: r-2");
  assert_eq!(lines[1], "  slot 1: idle, affinity checkout-0123456789ab  /pool/1/repo");
  assert!(lines[2].starts_with("  slot 2: lent to the run r-1 (pid 4242)"), "{}", lines[2]);
  assert!(lines[2].ends_with("/pool/2/repo"), "{}", lines[2]);
}

/// A slot is abandoned by the processes of the machine of its table: the lines mark the slots that the agent named, and
/// never judge a pid of the host by the processes of the client; the table of this machine names its own
#[test]
fn the_lines_mark_the_slots_that_the_machine_of_the_table_found_abandoned() {
  let own = std::process::id();
  let table = parse(&json!({
    "slots": [
      { "n": 1, "lease_id": "l1", "path": "/pool/1/repo", "state": { "kind": "lent", "run": "r-1", "pid": own, "since_ms": 0 } },
      { "n": 2, "lease_id": "l2", "path": "/pool/2/repo", "state": { "kind": "lent", "run": "r-2", "pid": own, "process_started": 1, "since_ms": 0 } }
    ]
  }))
  .expect("a table");
  let marked = |abandoned: &[u32]| -> Vec<bool> { lines("~/projects/repo", &table, abandoned)[1..].iter().map(|line| line.contains(" (abandoned)")).collect() };
  assert_eq!(marked(&[]), [false, false], "an older agent names none, whatever the processes of this machine");
  assert_eq!(marked(&[1]), [true, false], "the agent named slot 1, whose pid lives here");
  assert_eq!(abandoned(&table), [2], "on this machine the pid of slot 2 started at another time");
}

/// A table of another shape is a protocol fault of the agent
#[test]
fn a_table_of_another_shape_refuses_with_link() {
  let refusal = parse(&json!({ "slots": "none" })).expect_err("refused");
  assert_eq!(refusal.code, Code::Link);
}

/// The sizes are the bytes of the directories that the clean rules of each slot name
#[test]
fn the_sizes_measure_the_directories_of_the_clean_rules_of_each_slot() {
  let dir = tempfile::tempdir().expect("a temporary directory");
  let slot = dir.path().join("slot");
  std::fs::create_dir_all(slot.join("tests/results")).expect("the directory");
  std::fs::write(slot.join("tests/results/log.txt"), vec![b'x'; 10_000]).expect("a file");
  std::fs::write(slot.join(".branchdeck.toml"), "[lease]\nclean = [{ delete = \"tests/results\" }]\n").expect("the rules");
  let table = parse(&json!({
    "slots": [{ "n": 1, "lease_id": "l1", "path": slot.to_string_lossy(), "state": { "kind": "idle" } }]
  }))
  .expect("a table");
  let sizes = sizes(&table);
  assert_eq!(sizes.len(), 1, "{sizes:?}");
  assert_eq!((sizes[0].n, sizes[0].rule.as_str()), (1, "delete tests/results"));
  assert!(sizes[0].bytes >= 10_000, "{sizes:?}");
}
