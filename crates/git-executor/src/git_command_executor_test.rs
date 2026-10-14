use crate::git_command_executor::{GitCommandExecutor, read_progress_lines};

fn read(input: &str) -> (Vec<String>, String) {
  let mut lines = Vec::new();
  let messages = read_progress_lines(input.as_bytes(), &mut |line| lines.push(line.to_string()));
  (lines, messages)
}

#[test]
fn progress_updates_are_lines_but_not_messages() {
  let (lines, messages) = read("remote: Counting objects:  50% (1/2)   \rremote: Counting objects: 100% (2/2), done.   \nReceiving objects:  50% (1/2)\rfatal: early EOF\n");
  assert_eq!(
    lines,
    [
      "remote: Counting objects:  50% (1/2)",
      "remote: Counting objects: 100% (2/2), done.",
      "Receiving objects:  50% (1/2)",
      "fatal: early EOF"
    ]
  );
  assert_eq!(messages, "remote: Counting objects: 100% (2/2), done.\nfatal: early EOF\n");
}

#[test]
fn crlf_lines_are_messages() {
  let (lines, messages) = read("error: one\r\nfatal: two");
  assert_eq!(lines, ["error: one", "fatal: two"]);
  assert_eq!(messages, "error: one\nfatal: two");
}

#[test]
fn failed_command_reports_messages_without_progress_updates() {
  let dir = std::env::temp_dir();
  let mut lines = Vec::new();
  let error = GitCommandExecutor::new()
    .execute_command_with_progress(
      &["-c", "alias.fail=!printf 'step 1\\rstep 2\\rfatal: broken\\n' >&2; exit 3", "fail"],
      dir.to_str().unwrap(),
      &[],
      &mut |line| lines.push(line.to_string()),
    )
    .unwrap_err();
  assert_eq!(lines, ["step 1", "step 2", "fatal: broken"]);
  let error = format!("{error:#}");
  assert!(error.ends_with("Error: fatal: broken"), "{error}");
}
