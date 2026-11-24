use crate::claude_hook::{HookExec, HookTarget, has_hook};
use crate::service::SettingsService;
use crate::unattended::{UNATTENDED_FIELD, UnattendedError, append_denial, denials_file, read_denials, set_unattended};
use domain_types::settings::APP_SETTINGS_KEY;
use domain_types::unattended::UnattendedDenial;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Longer than any test: a save happens only by a flush or a drop
const NEVER: Duration = Duration::from_hours(1);

/// The settings of Claude Code of a user, as Claude Code writes them, with a hook of their own
const CLAUDE_SETTINGS: &str = r#"{
  "model": "opus",
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [
          {
            "type": "command",
            "command": "audit.sh"
          }
        ]
      }
    ]
  }
}
"#;

/// The hook of `~/.local/bin/brd` in the settings of Claude Code under `home`
fn hook_in(home: &Path) -> HookTarget {
  HookTarget {
    file: home.join(".claude/settings.json"),
    command: HookExec::new("/Users/me/.local/bin/brd".to_owned(), None),
  }
}

/// The journal, the hook of a home and the writer of the settings, all in `directory`, the writer saving only on a flush
fn switch_in(directory: &Path) -> (PathBuf, HookTarget, SettingsService) {
  let service = SettingsService::open_with_debounce(directory.to_path_buf(), NEVER).unwrap();
  (denials_file(directory), hook_in(directory), service)
}

/// The settings file of the app data directory `directory` as it is on the disk
fn saved(directory: &Path) -> Value {
  serde_json::from_slice(&std::fs::read(directory.join("settings.json")).unwrap()).unwrap()
}

fn denial(time: u32, tool_name: &str) -> UnattendedDenial {
  UnattendedDenial {
    time,
    session_id: "session".to_owned(),
    agent_id: None,
    agent_type: None,
    cwd: "/work".to_owned(),
    tool_name: tool_name.to_owned(),
    summary: "cat /etc/hosts".to_owned(),
  }
}

#[test]
fn turning_the_mode_on_starts_a_new_list_and_turning_it_off_keeps_it() {
  let directory = tempfile::tempdir().unwrap();
  let (journal, hook, service) = switch_in(directory.path());
  append_denial(&journal, &denial(1, "Bash")).unwrap();

  let on = set_unattended(&service, &journal, true, "rust", Some(&hook)).unwrap();
  assert!(on.changed);
  // saved once the switch answers, with no save of the writer due before an hour: the hook reads the file
  assert_eq!(saved(directory.path())[APP_SETTINGS_KEY][UNATTENDED_FIELD], json!(true));
  let change = on.change.expect("the change the windows get");
  assert_eq!(change.value["unattended"], json!(true));
  assert!(service.app().unattended);
  assert_eq!(read_denials(&journal), Vec::new(), "the list of the night before is gone");

  append_denial(&journal, &denial(2, "Read")).unwrap();
  let again = set_unattended(&service, &journal, true, "rust", Some(&hook)).unwrap();
  assert!(!again.changed && again.change.is_none(), "on again changes nothing");
  assert_eq!(read_denials(&journal), vec![denial(2, "Read")], "and keeps the list");

  let off = set_unattended(&service, &journal, false, "rust", Some(&hook)).unwrap();
  assert!(off.changed);
  assert_eq!(off.warning, None);
  assert!(!service.app().unattended);
  assert_eq!(read_denials(&journal), vec![denial(2, "Read")], "the morning reads the list of the night");

  assert_eq!(saved(directory.path()), json!({}), "saved at once; off is the default: the file keeps no value");
}

/// The hook is in the settings of Claude Code only while the mode is on: on adds it before the setting, off removes it
/// after, and the file of the user comes back byte for byte, its own hook as written
#[test]
fn the_hook_is_in_the_settings_of_claude_code_only_while_the_mode_is_on() {
  let directory = tempfile::tempdir().unwrap();
  let (journal, hook, service) = switch_in(directory.path());
  std::fs::create_dir_all(hook.file.parent().unwrap()).unwrap();
  std::fs::write(&hook.file, CLAUDE_SETTINGS).unwrap();

  set_unattended(&service, &journal, false, "rust", Some(&hook)).unwrap();
  assert_eq!(std::fs::read_to_string(&hook.file).unwrap(), CLAUDE_SETTINGS, "off: the file is not written");

  set_unattended(&service, &journal, true, "rust", Some(&hook)).unwrap();
  assert!(has_hook(&hook.file));
  let text = std::fs::read_to_string(&hook.file).unwrap();
  assert!(text.contains("\"command\": \"audit.sh\""), "{text}");

  set_unattended(&service, &journal, false, "rust", Some(&hook)).unwrap();
  assert_eq!(std::fs::read_to_string(&hook.file).unwrap(), CLAUDE_SETTINGS, "the file of the user, byte for byte");

  // a user without settings of Claude Code: on creates the file, off leaves it without the hook
  let other = hook_in(&directory.path().join("other"));
  set_unattended(&service, &journal, true, "rust", Some(&other)).unwrap();
  assert!(has_hook(&other.file));
  set_unattended(&service, &journal, false, "rust", Some(&other)).unwrap();
  assert_eq!(std::fs::read_to_string(&other.file).unwrap(), "{}\n");
}

/// On without a place for the hook, or with a settings file of Claude Code that cannot take it, stays off and says why;
/// off with such a file is off all the same, with a warning
#[test]
fn a_switch_on_whose_hook_cannot_be_written_stays_off() {
  let directory = tempfile::tempdir().unwrap();
  let (journal, hook, service) = switch_in(directory.path());

  let error = set_unattended(&service, &journal, true, "rust", None).unwrap_err();
  assert!(matches!(error, UnattendedError::NoHookFile), "{error}");
  assert!(!service.app().unattended);

  std::fs::create_dir_all(hook.file.parent().unwrap()).unwrap();
  std::fs::write(&hook.file, "{ \"model\": ").unwrap();
  let error = set_unattended(&service, &journal, true, "rust", Some(&hook)).unwrap_err();
  assert!(
    error
      .to_string()
      .starts_with(&format!("cannot add the hook of Unattended Mode to {}: it is no JSON object", hook.file.display())),
    "{error}"
  );
  assert!(!service.app().unattended, "the mode stays off");
  assert!(!directory.path().join("settings.json").exists(), "and nothing was saved");
  assert_eq!(std::fs::read_to_string(&hook.file).unwrap(), "{ \"model\": ", "the file stays as it was");

  // on, then the user breaks the file: off is off, and says the hook stayed
  std::fs::write(&hook.file, "{}").unwrap();
  set_unattended(&service, &journal, true, "rust", Some(&hook)).unwrap();
  std::fs::write(&hook.file, "[]").unwrap();
  let off = set_unattended(&service, &journal, false, "rust", Some(&hook)).unwrap();
  assert!(off.changed && !service.app().unattended);
  let warning = off.warning.unwrap_or_default();
  assert!(
    warning.starts_with(&format!("Cannot remove the hook of Unattended Mode from {}: it is no JSON object", hook.file.display())),
    "{warning}"
  );
}

#[test]
fn the_journal_keeps_the_lines_that_parse() {
  let directory = tempfile::tempdir().unwrap();
  let journal = denials_file(directory.path());
  assert_eq!(read_denials(&journal), Vec::new(), "no file, no denial");
  append_denial(&journal, &denial(1, "Bash")).unwrap();
  // a line cut by a full disk, and a line of another shape
  let mut text = std::fs::read_to_string(&journal).unwrap();
  text.push_str("{\"time\":2,\"tool_na\n[1,2]\n");
  std::fs::write(&journal, text).unwrap();
  append_denial(&journal, &denial(3, "WebFetch")).unwrap();
  assert_eq!(read_denials(&journal), vec![denial(1, "Bash"), denial(3, "WebFetch")]);
}

#[test]
fn hooks_that_deny_at_once_append_whole_lines() {
  let directory = tempfile::tempdir().unwrap();
  let journal = denials_file(directory.path());
  std::thread::scope(|scope| {
    for thread in 0..8 {
      let journal = &journal;
      scope.spawn(move || {
        for index in 0..50 {
          let mut denial = denial(thread * 100 + index, "Bash");
          denial.summary = "x".repeat(150);
          append_denial(journal, &denial).unwrap();
        }
      });
    }
  });
  let text = std::fs::read_to_string(&journal).unwrap();
  assert_eq!(text.lines().count(), 400);
  assert_eq!(read_denials(&journal).len(), 400, "every line parses: none mixed with another");
}
