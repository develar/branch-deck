use crate::claude_hook::{check_settings, hook_state, install_hook, uninstall_hook};
use domain_types::error::ErrorCode;
use domain_types::unattended::UnattendedHook;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::path::Path;

const COMMAND: &str = "/Users/me/.local/bin/brd hook permission-request";

/// The settings of a user of Claude Code: keys out of alphabetical order, an escape, a hook of another event and one
/// of `PermissionRequest`, as Claude Code writes them
const USER_SETTINGS: &str = r#"{
  "model": "opus",
  "apiKeyHelper": "proxy start & key",
  "permissions": {
    "allow": [
      "Bash(ls:*)"
    ],
    "additionalDirectories": [
      "/Users/me/pool"
    ]
  },
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
    ],
    "PermissionRequest": [
      {
        "matcher": "WebFetch",
        "hooks": [
          {
            "type": "command",
            "command": "notify.sh"
          }
        ]
      }
    ]
  },
  "alwaysThinkingEnabled": true
}
"#;

fn read(file: &Path) -> Value {
  serde_json::from_str(&std::fs::read_to_string(file).unwrap()).unwrap()
}

fn our_entry(command: &str) -> Value {
  json!({ "matcher": "*", "hooks": [{ "type": "command", "command": command }] })
}

#[test]
fn an_install_adds_the_hook_once_and_keeps_every_other_key_and_hook_as_written() {
  let home = tempfile::tempdir().unwrap();
  let file = home.path().join(".claude/settings.json");
  std::fs::create_dir_all(file.parent().unwrap()).unwrap();
  std::fs::write(&file, USER_SETTINGS).unwrap();
  assert_eq!(hook_state(&file, COMMAND), UnattendedHook::Missing);

  assert!(install_hook(&file, COMMAND).unwrap(), "written");
  let text = std::fs::read_to_string(&file).unwrap();
  // the text of the user as it was: the order of the keys, the escape, the other hooks
  let (before, after) = USER_SETTINGS
    .split_once("            \"command\": \"notify.sh\"\n          }\n        ]\n      }\n")
    .unwrap();
  assert!(text.starts_with(before), "{text}");
  assert!(
    text.ends_with(&format!(
      ",\n      {{\n        \"hooks\": [\n          {{\n            \"command\": \"{COMMAND}\",\n            \"type\": \"command\"\n          }}\n        ],\n        \"matcher\": \"*\"\n      }}\n{after}"
    )),
    "{text}"
  );
  assert_eq!(read(&file)["hooks"]["PermissionRequest"][1], our_entry(COMMAND));
  assert_eq!(hook_state(&file, COMMAND), UnattendedHook::Installed);

  assert!(!install_hook(&file, COMMAND).unwrap(), "an install again writes nothing");
  assert_eq!(std::fs::read_to_string(&file).unwrap(), text);
}

#[test]
fn an_install_of_a_moved_brd_replaces_the_command_of_the_old_one() {
  let home = tempfile::tempdir().unwrap();
  let file = home.path().join(".claude/settings.json");
  let old = "'/Applications/Branch Deck.app/Contents/MacOS/branch-deck' cli hook permission-request";
  assert!(install_hook(&file, old).unwrap(), "a missing file is created");
  assert_eq!(read(&file), json!({ "hooks": { "PermissionRequest": [our_entry(old)] } }));
  assert_eq!(hook_state(&file, COMMAND), UnattendedHook::Stale);

  // the user put a hook of their own beside it, and a timeout on ours
  let mut settings = read(&file);
  settings["hooks"]["PermissionRequest"][0]["hooks"][0]["timeout"] = json!(5);
  settings["hooks"]["PermissionRequest"][0]["hooks"]
    .as_array_mut()
    .unwrap()
    .push(json!({ "type": "command", "command": "log.sh" }));
  std::fs::write(&file, serde_json::to_string_pretty(&settings).unwrap()).unwrap();

  assert!(install_hook(&file, COMMAND).unwrap());
  assert_eq!(
    read(&file),
    json!({ "hooks": { "PermissionRequest": [
      { "matcher": "*", "hooks": [{ "type": "command", "command": "log.sh" }] },
      our_entry(COMMAND),
    ] } }),
    "one hook of Branch Deck, of this brd; the hook of the user stays"
  );
  assert_eq!(hook_state(&file, COMMAND), UnattendedHook::Installed);

  // what the user changed of the installed hook stays
  let mut settings = read(&file);
  settings["hooks"]["PermissionRequest"][1]["hooks"][0]["timeout"] = json!(5);
  let text = serde_json::to_string_pretty(&settings).unwrap();
  std::fs::write(&file, &text).unwrap();
  assert!(!install_hook(&file, COMMAND).unwrap());
  assert_eq!(std::fs::read_to_string(&file).unwrap(), text);
}

#[test]
fn an_uninstall_removes_only_the_hook_of_branch_deck() {
  let home = tempfile::tempdir().unwrap();
  let file = home.path().join(".claude/settings.json");
  assert!(!uninstall_hook(&file).unwrap(), "no file, nothing to remove");
  std::fs::create_dir_all(file.parent().unwrap()).unwrap();
  std::fs::write(&file, USER_SETTINGS).unwrap();
  assert!(!uninstall_hook(&file).unwrap(), "no hook of Branch Deck");
  assert_eq!(std::fs::read_to_string(&file).unwrap(), USER_SETTINGS, "and the file stays as it was");

  install_hook(&file, COMMAND).unwrap();
  assert!(uninstall_hook(&file).unwrap());
  assert_eq!(read(&file), serde_json::from_str::<Value>(USER_SETTINGS).unwrap());
  assert_eq!(hook_state(&file, COMMAND), UnattendedHook::Missing);
}

#[test]
fn a_file_that_is_no_object_is_refused_and_stays() {
  let home = tempfile::tempdir().unwrap();
  let file = home.path().join("settings.json");
  for text in ["[1, 2]", "{ \"model\": ", "{ \"hooks\": [] }", "{ \"hooks\": { \"PermissionRequest\": {} } }"] {
    std::fs::write(&file, text).unwrap();
    let error = install_hook(&file, COMMAND).unwrap_err();
    assert_eq!(error.code, ErrorCode::InstallRefused, "{text}");
    assert!(
      error.message.starts_with(&format!("Cannot add the hook of Unattended Mode to {}: ", file.display())),
      "{}",
      error.message
    );
    assert_eq!(check_settings(&file).unwrap_err().message, error.message);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), text, "the file stays as it was");
    assert_eq!(hook_state(&file, COMMAND), UnattendedHook::Missing);
  }
  std::fs::write(&file, "\n").unwrap();
  assert!(install_hook(&file, COMMAND).unwrap(), "a blank file is an empty one");
  assert_eq!(read(&file), json!({ "hooks": { "PermissionRequest": [our_entry(COMMAND)] } }));
}

#[cfg(unix)]
#[test]
fn a_private_file_stays_private() {
  use std::os::unix::fs::PermissionsExt;
  let home = tempfile::tempdir().unwrap();
  let file = home.path().join("settings.json");
  std::fs::write(&file, "{}").unwrap();
  std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
  install_hook(&file, COMMAND).unwrap();
  assert_eq!(std::fs::metadata(&file).unwrap().permissions().mode() & 0o777, 0o600);
}
