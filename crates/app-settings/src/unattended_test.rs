use crate::claude_hook::{HookExec, HookTarget, has_hook};
use crate::service::SettingsService;
use crate::unattended::{UNATTENDED_FIELD, UnattendedError, append_denial, denials_file, hook_memo_file, read_denials, refresh_hook, set_unattended};
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
    claude: None,
  }
}

/// The journal, the hook of a home and the writer of the settings, all in `directory`, the writer saving only on a flush
fn switch_in(directory: &Path) -> (PathBuf, HookTarget, SettingsService) {
  let service = SettingsService::open_with_debounce(directory.to_path_buf(), NEVER).unwrap();
  (denials_file(directory), hook_in(directory), service)
}

/// `switch_in`, with the settings of Claude Code of a user (`CLAUDE_SETTINGS`)
fn switch_with_claude_settings(directory: &Path) -> (PathBuf, HookTarget, SettingsService) {
  let switch = switch_in(directory);
  std::fs::create_dir_all(switch.1.file.parent().unwrap()).unwrap();
  std::fs::write(&switch.1.file, CLAUDE_SETTINGS).unwrap();
  switch
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
  let (journal, hook, service) = switch_with_claude_settings(directory.path());

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

/// A save that fails after the switch puts the setting back and takes away the hook a switch on added, so the mode,
/// the file of Claude Code and the windows agree; the next switch tries again, and switches once the save succeeds
#[test]
fn a_switch_whose_save_fails_puts_the_mode_back() {
  let directory = tempfile::tempdir().unwrap();
  let (journal, hook, service) = switch_with_claude_settings(directory.path());
  // a directory in the place of the settings file: no save renames over it
  let settings_file = directory.path().join("settings.json");
  std::fs::create_dir_all(&settings_file).unwrap();

  for _ in 0..2 {
    let error = set_unattended(&service, &journal, true, "rust", Some(&hook)).unwrap_err();
    assert!(matches!(error, UnattendedError::Settings(_)), "{error}");
    assert!(!service.app().unattended, "the mode is off as before");
    assert_eq!(std::fs::read_to_string(&hook.file).unwrap(), CLAUDE_SETTINGS, "and the hook is gone again");
  }

  std::fs::remove_dir(&settings_file).unwrap();
  let on = set_unattended(&service, &journal, true, "rust", Some(&hook)).unwrap();
  assert!(on.changed && on.change.is_some(), "the switch the windows hear");
  assert_eq!(saved(directory.path())[APP_SETTINGS_KEY][UNATTENDED_FIELD], json!(true));

  // off whose save fails: the mode stays on with its hook
  std::fs::remove_file(&settings_file).unwrap();
  std::fs::create_dir_all(&settings_file).unwrap();
  let error = set_unattended(&service, &journal, false, "rust", Some(&hook)).unwrap_err();
  assert!(matches!(error, UnattendedError::Settings(_)), "{error}");
  assert!(service.app().unattended);
  assert!(has_hook(&hook.file), "the hook stays while the mode is on");
}

/// An on whose journal of the run before cannot be removed stays off, without the hook it added
#[test]
fn a_switch_on_whose_journal_cannot_be_removed_takes_the_hook_away() {
  let directory = tempfile::tempdir().unwrap();
  let (journal, hook, service) = switch_with_claude_settings(directory.path());
  // a directory with a file in it in the place of the journal: no removal of a file takes it
  std::fs::create_dir_all(journal.join("inside")).unwrap();

  let error = set_unattended(&service, &journal, true, "rust", Some(&hook)).unwrap_err();
  assert!(matches!(error, UnattendedError::Journal(_)), "{error}");
  assert!(!service.app().unattended);
  assert_eq!(std::fs::read_to_string(&hook.file).unwrap(), CLAUDE_SETTINGS, "the hook is gone again");
}

/// Two processes, each with its writer of the settings: a switch decides by the mode another one saved a moment ago,
/// not by its copy, and switches at the same time take turns, so the mode in the file always has its hook
#[test]
fn switches_of_two_processes_take_turns_and_decide_by_the_file() {
  let directory = tempfile::tempdir().unwrap();
  let (journal, hook, first) = switch_in(directory.path());
  let second = SettingsService::open_with_debounce(directory.path().to_path_buf(), NEVER).unwrap();

  assert!(set_unattended(&first, &journal, true, "app", Some(&hook)).unwrap().changed);
  // the copy of the second writer was loaded before: it learns of the switch under the lock
  let off = set_unattended(&second, &journal, false, "brd", Some(&hook)).unwrap();
  assert!(off.changed, "off finds the mode on");
  assert!(!has_hook(&hook.file));

  std::thread::scope(|scope| {
    for (writer, origin) in [(&first, "app"), (&second, "brd")] {
      let (journal, hook) = (&journal, &hook);
      scope.spawn(move || {
        for round in 0..10 {
          set_unattended(writer, journal, (round % 2 == 0) == (origin == "app"), origin, Some(hook)).unwrap();
        }
      });
    }
  });
  let on = saved(directory.path())[APP_SETTINGS_KEY][UNATTENDED_FIELD] == json!(true);
  assert_eq!(has_hook(&hook.file), on, "the hook is in the file exactly while the mode is on there");

  // `brd install` decides by the file under the same lock
  let refresh = refresh_hook(Some(directory.path()), &hook).unwrap();
  assert_eq!((refresh.on, has_hook(&hook.file)), (on, on));
}

/// The containers of the hook the settings of Claude Code had empty stay after the mode, as the switch keeps them until
/// the switch off
#[test]
fn the_containers_the_settings_had_empty_stay_after_the_mode() {
  let directory = tempfile::tempdir().unwrap();
  let (journal, hook, service) = switch_in(directory.path());
  std::fs::create_dir_all(hook.file.parent().unwrap()).unwrap();
  let text = "{\n    \"model\": \"opus\",\n    \"hooks\": {\n        \"PermissionRequest\": []\n    }\n}";
  std::fs::write(&hook.file, text).unwrap();

  set_unattended(&service, &journal, true, "rust", Some(&hook)).unwrap();
  assert!(has_hook(&hook.file));
  // an on again, of a brd that moved, keeps what the first add found
  let moved = HookTarget {
    command: HookExec::new("/Applications/Branch Deck.app/Contents/MacOS/branch-deck".to_owned(), Some("cli")),
    ..hook.clone()
  };
  set_unattended(&service, &journal, true, "rust", Some(&moved)).unwrap();
  set_unattended(&service, &journal, false, "rust", Some(&hook)).unwrap();
  assert_eq!(std::fs::read_to_string(&hook.file).unwrap(), text, "byte for byte");
  assert!(!hook_memo_file(directory.path()).exists(), "the memo goes with the hook");
}

/// A Claude Code older than the hook needs is refused: it would run brd without its arguments and deny nothing
#[cfg(unix)]
#[test]
fn a_switch_on_with_an_older_claude_code_stays_off() {
  use std::os::unix::fs::PermissionsExt;
  let directory = tempfile::tempdir().unwrap();
  let (journal, mut hook, service) = switch_in(directory.path());
  let claude = directory.path().join("claude");
  hook.claude = Some(claude.clone());
  for (version, refused) in [("2.1.294", true), ("2.1.295", false)] {
    std::fs::write(&claude, format!("#!/bin/sh\necho '{version} (Claude Code)'\n")).unwrap();
    std::fs::set_permissions(&claude, std::fs::Permissions::from_mode(0o755)).unwrap();
    let switch = set_unattended(&service, &journal, true, "rust", Some(&hook));
    if refused {
      let error = switch.unwrap_err();
      assert!(matches!(&error, UnattendedError::OldClaudeCode(found) if found == version), "{error}");
      assert!(error.to_string().contains("Update Claude Code to 2.1.295 or later"), "{error}");
      assert!(!service.app().unattended && !hook.file.exists(), "off, and the settings of Claude Code are not written");
    } else {
      assert!(switch.unwrap().changed);
    }
  }
}
