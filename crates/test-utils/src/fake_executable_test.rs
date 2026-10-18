use crate::fake_executable::fake_executable;
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

/// What the fakes of the tests are given: an empty argument (`--tools ""`), JSON (`--json-schema`), text with quotes,
/// a backtick and a `$` (`--system-prompt`), Windows paths, and what a shell would glob or split
const ARGUMENTS: [&str; 8] = [
  "",
  "two words",
  r#"{"type":"object","required":["names"],"properties":{"names":{"type":"array"}}}"#,
  "it's `git show --format=%s%n%b` of $HOME",
  r"C:\Users\me\answer.json",
  "*.rs",
  "{a,b}",
  "line\nbreak",
];

#[test]
fn the_script_gets_the_arguments_stdin_and_environment_and_gives_its_output_and_exit_code() {
  let dir = tempfile::tempdir().unwrap();
  let fake = fake_executable(
    dir.path(),
    "tool",
    "printf '%s\\n' \"$#\" \"$@\"\ncat\necho \"$FAKE_TEST_VALUE\"\necho to-stderr >&2\nexit 7",
  );
  let mut child = Command::new(&fake)
    .args(ARGUMENTS)
    .env("FAKE_TEST_VALUE", "from the environment")
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .unwrap();
  child.stdin.take().unwrap().write_all(b"from stdin\n").unwrap();
  let output = child.wait_with_output().unwrap();

  let mut expected = format!("{}\n", ARGUMENTS.len());
  for argument in ARGUMENTS {
    expected.push_str(argument);
    expected.push('\n');
  }
  expected.push_str("from stdin\nfrom the environment\n");
  assert_eq!(String::from_utf8_lossy(&output.stdout), expected);
  assert_eq!(String::from_utf8_lossy(&output.stderr).trim(), "to-stderr");
  assert_eq!(output.status.code(), Some(7));

  // a fake written again is the new script
  let fake = fake_executable(dir.path(), "tool", "echo second");
  let output = Command::new(&fake).output().unwrap();
  assert_eq!(String::from_utf8_lossy(&output.stdout), "second\n");
  assert!(output.status.success());
}

/// A caller that kills a hanging fake after a timeout gets the end of its output at once: no process of the fake
/// (on Windows its sh and the `sleep`) lives on with the pipe
#[test]
fn a_killed_fake_leaves_no_process_that_holds_its_output() {
  let dir = tempfile::tempdir().unwrap();
  let fake = fake_executable(dir.path(), "hang", "echo started\nexec sleep 30");
  let mut child = Command::new(&fake).stdin(Stdio::null()).stdout(Stdio::piped()).spawn().unwrap();
  let mut stdout = child.stdout.take().unwrap();
  let mut started = [0; 8];
  stdout.read_exact(&mut started).unwrap();
  assert_eq!(&started, b"started\n");

  child.kill().unwrap();
  child.wait().unwrap();
  let (sender, receiver) = mpsc::channel();
  std::thread::spawn(move || {
    let mut rest = Vec::new();
    let _ = stdout.read_to_end(&mut rest);
    let _ = sender.send(rest);
  });
  let rest = receiver.recv_timeout(Duration::from_secs(10)).expect("a process of the killed fake still holds its stdout");
  assert_eq!(rest, b"");
}
