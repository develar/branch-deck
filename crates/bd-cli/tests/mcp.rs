//! The process test of `brd mcp` (openspec/specs/repository/ide-embedding/spec.md): the binary on its real stdin and
//! stdout, as the IDE runs it, in a temporary home. `process_test.rs` of ide-host serves on pipes in memory, so it does
//! not see what only the stdout of the process does
#![cfg(unix)]
#![expect(clippy::tests_outside_test_module, reason = "a Cargo integration test has no #[cfg(test)] module")]

use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

const BRD: &str = env!("CARGO_BIN_EXE_brd");

/// The first answer of `brd mcp` is on its stdout, and the end of its stdin ends it with 0. The app data directory is in
/// the temporary home; neither the start nor `initialize` reads a secret, so the keychain stays untouched
#[test]
fn brd_mcp_answers_initialize_on_its_stdout_and_ends_with_its_stdin() {
  let home = tempfile::tempdir().expect("a temporary home");
  let mut child = Command::new(BRD)
    .arg("mcp")
    // the backend of the bridge's process: a test starts no app
    .env("BRANCH_DECK_IDE_BACKEND", "in-process")
    .current_dir(home.path())
    .env("HOME", home.path())
    .env("XDG_DATA_HOME", home.path().join(".local/share"))
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::inherit())
    .spawn()
    .expect("brd mcp starts");
  let mut stdin = child.stdin.take().expect("the stdin of brd mcp");
  let mut stdout = BufReader::new(child.stdout.take().expect("the stdout of brd mcp"));

  // No clock: the start of a process is seconds on a loaded host (6.6 s on the Studio), so the test waits for the line
  // and the exit themselves. A brd that never answers or never exits is ended by the terminate-after of
  // `.config/nextest.toml`, which kills the process group of the test, brd included
  let initialize = json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {} });
  stdin.write_all(format!("{initialize}\n").as_bytes()).expect("initialize reaches brd mcp");
  stdin.flush().expect("initialize reaches brd mcp");
  let mut line = String::new();
  stdout.read_line(&mut line).expect("the stdout of brd mcp");
  assert!(!line.is_empty(), "brd mcp closed its stdout before it answered initialize");
  let answer: Value = serde_json::from_str(&line).expect("the answer is JSON");
  assert_eq!(answer["id"], 1, "{answer:#}");
  assert_eq!(answer["result"]["serverInfo"]["name"], "branch-deck", "{answer:#}");
  assert_eq!(answer["result"]["capabilities"]["experimental"]["x-air-app"]["version"], "1.0.0", "{answer:#}");

  drop(stdin);
  let status = child.wait();
  assert_eq!(status.expect("brd mcp ended").code(), Some(0));
}
