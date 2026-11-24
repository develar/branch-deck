use crate::fixture_test::{environment, run_json};
use crate::unattended::{DENY_MESSAGE, permission_request, run_permission_request, summary_of};
use app_settings::unattended::{denials_file, read_denials};
use domain_types::unattended::{SUMMARY_LIMIT, UnattendedDenial};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::path::Path;

/// The input Claude Code gives the hook of a subagent's Bash command
fn bash_input(command: &str) -> String {
  json!({
    "hook_event_name": "PermissionRequest",
    "session_id": "4f1c",
    "transcript_path": "/Users/me/.claude/projects/x/4f1c.jsonl",
    "cwd": "/Users/me/projects/app",
    "permission_mode": "default",
    "tool_name": "Bash",
    "tool_input": { "command": command, "description": "Read the hosts" },
    "agent_id": "a7",
    "agent_type": "general-purpose",
    "permission_suggestions": []
  })
  .to_string()
}

fn write_settings(app_data: &Path, settings: &str) {
  std::fs::write(app_data.join("settings.json"), settings).unwrap();
}

fn turned_on(app_data: &Path) {
  write_settings(app_data, r#"{ "appSettings": { "unattended": true } }"#);
}

#[test]
fn the_hook_denies_a_request_in_unattended_mode_and_records_it() {
  let app_data = tempfile::tempdir().unwrap();
  turned_on(app_data.path());

  let answer = permission_request(&bash_input("cat /etc/hosts"), app_data.path(), 1_795_500_000).expect("a denial");
  assert_eq!(
    serde_json::from_str::<Value>(&answer).unwrap(),
    json!({ "hookSpecificOutput": { "hookEventName": "PermissionRequest", "decision": { "behavior": "deny", "message": DENY_MESSAGE } } })
  );
  assert!(!answer.contains('\n'), "one line: {answer}");

  // the main session's write of a file: its path, never its content
  let write = json!({
    "hook_event_name": "PermissionRequest",
    "session_id": "4f1c",
    "cwd": "/Users/me/projects/app",
    "tool_name": "Write",
    "tool_input": { "file_path": "/Users/me/.zshrc", "content": "export SECRET=1" }
  });
  assert!(permission_request(&write.to_string(), app_data.path(), 1_795_500_060).is_some());

  assert_eq!(
    read_denials(&denials_file(app_data.path())),
    vec![
      UnattendedDenial {
        time: 1_795_500_000,
        session_id: "4f1c".to_owned(),
        agent_id: Some("a7".to_owned()),
        agent_type: Some("general-purpose".to_owned()),
        cwd: "/Users/me/projects/app".to_owned(),
        tool_name: "Bash".to_owned(),
        summary: "cat /etc/hosts".to_owned(),
      },
      UnattendedDenial {
        time: 1_795_500_060,
        session_id: "4f1c".to_owned(),
        agent_id: None,
        agent_type: None,
        cwd: "/Users/me/projects/app".to_owned(),
        tool_name: "Write".to_owned(),
        summary: "/Users/me/.zshrc".to_owned(),
      },
    ]
  );
  let journal = std::fs::read_to_string(denials_file(app_data.path())).unwrap();
  assert!(!journal.contains("SECRET"), "{journal}");
}

#[test]
fn the_hook_answers_nothing_while_the_mode_is_off_or_its_setting_cannot_be_read() {
  let app_data = tempfile::tempdir().unwrap();
  assert_eq!(permission_request(&bash_input("ls"), app_data.path(), 1), None, "no settings file");
  for settings in [
    r#"{ "appSettings": { "unattended": false } }"#,
    r#"{ "appSettings": { "zoom": 2 } }"#,
    "{ \"appSettings\": ",
    r#"{ "appSettings": { "unattended": "yes" } }"#,
  ] {
    write_settings(app_data.path(), settings);
    assert_eq!(permission_request(&bash_input("ls"), app_data.path(), 1), None, "{settings}");
  }
  assert!(!denials_file(app_data.path()).exists(), "nothing recorded");

  // the process: no output, exit code 0
  let mut out = Vec::new();
  assert_eq!(run_permission_request(Some(app_data.path()), &mut bash_input("ls").as_bytes(), &mut out), 0);
  assert_eq!(out, b"");
  assert_eq!(run_permission_request(None, &mut bash_input("ls").as_bytes(), &mut out), 0, "no app data directory");
  assert_eq!(out, b"");
}

#[test]
fn the_hook_answers_nothing_to_an_input_that_is_no_permission_request() {
  let app_data = tempfile::tempdir().unwrap();
  turned_on(app_data.path());
  let other_event = json!({ "hook_event_name": "PreToolUse", "tool_name": "Bash", "tool_input": { "command": "ls" } }).to_string();
  for input in ["", "garbage", "[1]", "{\"tool_input\": {}}", "{\"tool_name\": 7}", other_event.as_str()] {
    assert_eq!(permission_request(input, app_data.path(), 1), None, "{input:?}");
    let mut out = Vec::new();
    assert_eq!(run_permission_request(Some(app_data.path()), &mut input.as_bytes(), &mut out), 0);
    assert_eq!(out, b"", "{input:?}");
  }
  assert!(!denials_file(app_data.path()).exists(), "nothing recorded");

  let mut out = Vec::new();
  assert_eq!(run_permission_request(Some(app_data.path()), &mut bash_input("ls").as_bytes(), &mut out), 0);
  assert_eq!(String::from_utf8(out).unwrap().lines().count(), 1, "the denial on one line");
}

#[test]
fn a_summary_names_what_the_tool_was_asked_for_and_is_cut() {
  assert_eq!(summary_of(&json!({ "command": "git push", "description": "Push" })), "git push");
  assert_eq!(summary_of(&json!({ "file_path": "/a.rs", "old_string": "a", "new_string": "b" })), "/a.rs");
  assert_eq!(summary_of(&json!({ "url": "https://example.com", "prompt": "Read it" })), "https://example.com");
  assert_eq!(summary_of(&json!({ "pattern": "TODO", "path": "src" })), "TODO");
  assert_eq!(summary_of(&json!({ "channel": "C1", "text": "hello" })), "channel, text", "the names of the arguments");
  assert_eq!(summary_of(&json!("text")), "");
  let long = summary_of(&json!({ "command": "é".repeat(300) }));
  assert_eq!(long.chars().count(), SUMMARY_LIMIT);
  assert!(long.ends_with('…'));
}

#[test_log::test]
fn unattended_turns_the_mode_on_and_off_and_lists_what_the_hook_denied_since_it_was_turned_on() {
  let directory = tempfile::tempdir().unwrap();
  let app_data = directory.path().join("app-data");
  let mut environment = environment(directory.path());
  environment.app_data = Some(app_data.clone());
  environment.home = Some(directory.path().to_path_buf());
  let journal = denials_file(&app_data);

  let (exit_code, status) = run_json(&environment, &["unattended", "status"]);
  assert_eq!(exit_code, 0, "{status}");
  assert_eq!(
    status,
    json!({ "schema": 2, "on": false, "changed": false, "hook": "missing", "denied": 0, "denials": [], "journal": journal.to_str().unwrap(), "next": ["brd install"] })
  );

  let (exit_code, on) = run_json(&environment, &["unattended", "on"]);
  assert_eq!(exit_code, 0, "{on}");
  assert_eq!((&on["on"], &on["changed"]), (&json!(true), &json!(true)), "{on}");
  // the hook reads the file the command saved
  assert!(permission_request(&bash_input("cat /etc/hosts"), &app_data, 7).is_some());
  let (_, again) = run_json(&environment, &["unattended", "on"]);
  assert_eq!((&again["changed"], &again["denied"]), (&json!(false), &json!(1)), "on again keeps the list: {again}");

  let (_, off) = run_json(&environment, &["unattended", "off"]);
  assert_eq!((&off["on"], &off["changed"], &off["denied"]), (&json!(false), &json!(true), &json!(1)), "{off}");
  assert_eq!(off["denials"][0]["summary"], json!("cat /etc/hosts"));
  assert_eq!(permission_request(&bash_input("ls"), &app_data, 8), None, "off: the hook lets Claude Code ask");
  let (_, status) = run_json(&environment, &["unattended", "status"]);
  assert_eq!((&status["on"], &status["denied"]), (&json!(false), &json!(1)), "the morning reads the night: {status}");

  let (_, on) = run_json(&environment, &["unattended", "on"]);
  assert_eq!(
    (&on["on"], &on["denied"], &on["denials"]),
    (&json!(true), &json!(0), &json!([])),
    "a new night, a new list: {on}"
  );

  let (_, schema) = run_json(&environment, &["schema", "unattended"]);
  crate::fixture_test::assert_covered(&schema["outputs"]["unattended"], &schema["outputs"]["unattended"], &off, "unattended");
}

#[test_log::test]
fn unattended_without_a_data_directory_is_refused() {
  let directory = tempfile::tempdir().unwrap();
  let (exit_code, output) = run_json(&environment(directory.path()), &["unattended", "on"]);
  assert_ne!(exit_code, 0);
  assert_eq!(output["error"]["code"], json!("invalid_arguments"), "{output}");
}
