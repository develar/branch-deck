//! The process test of `brd mcp` (openspec/specs/repository/ide-embedding/spec.md): the binary on its real stdin and
//! stdout, as the IDE runs it, in a temporary home. `process_test.rs` of mcp-host serves on pipes in memory, so it does
//! not see what only the stdout of the process does
#![cfg(unix)]
#![expect(clippy::tests_outside_test_module, reason = "a Cargo integration test has no #[cfg(test)] module")]

use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

const BRD: &str = env!("CARGO_BIN_EXE_brd");

/// The most a test waits for an answer or the exit: a regression fails the test, it does not hang it
const TIMEOUT: Duration = Duration::from_secs(10);

/// Ends the process of `pid` that did not answer in time
fn kill(pid: u32) {
  let _ = Command::new("kill").args(["-KILL", &pid.to_string()]).status();
}

/// The first answer of `brd mcp` is on its stdout, and the end of its stdin ends it with 0. The app data directory is in
/// the temporary home; neither the start nor `initialize` reads a secret, so the keychain stays untouched
#[test]
fn brd_mcp_answers_initialize_on_its_stdout_and_ends_with_its_stdin() {
  let home = tempfile::tempdir().expect("a temporary home");
  let mut child = Command::new(BRD)
    .arg("mcp")
    .current_dir(home.path())
    .env("HOME", home.path())
    .env("XDG_DATA_HOME", home.path().join(".local/share"))
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::inherit())
    .spawn()
    .expect("brd mcp starts");
  let pid = child.id();
  let mut stdin = child.stdin.take().expect("the stdin of brd mcp");
  let stdout = child.stdout.take().expect("the stdout of brd mcp");

  let (lines, first_line) = mpsc::channel();
  thread::spawn(move || {
    let mut line = String::new();
    let read = BufReader::new(stdout).read_line(&mut line).map(|_| line);
    let _ = lines.send(read);
  });
  let initialize = json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {} });
  stdin.write_all(format!("{initialize}\n").as_bytes()).expect("initialize reaches brd mcp");
  stdin.flush().expect("initialize reaches brd mcp");
  let Ok(line) = first_line.recv_timeout(TIMEOUT) else {
    kill(pid);
    panic!("brd mcp wrote no line on its stdout in {TIMEOUT:?}");
  };
  let answer: Value = serde_json::from_str(&line.expect("the stdout of brd mcp")).expect("the answer is JSON");
  assert_eq!(answer["id"], 1, "{answer:#}");
  assert_eq!(answer["result"]["serverInfo"]["name"], "branch-deck", "{answer:#}");
  assert_eq!(answer["result"]["capabilities"]["experimental"]["x-air-app"]["version"], "1.0.0", "{answer:#}");

  drop(stdin);
  let (exits, exit) = mpsc::channel::<std::io::Result<ExitStatus>>();
  thread::spawn(move || {
    let _ = exits.send(child.wait());
  });
  let Ok(status) = exit.recv_timeout(TIMEOUT) else {
    kill(pid);
    panic!("brd mcp did not exit in {TIMEOUT:?} after the end of its stdin");
  };
  assert_eq!(status.expect("brd mcp ended").code(), Some(0));
}
