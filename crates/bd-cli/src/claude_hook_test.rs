use crate::claude_hook::{HookExec, edit, hook_state, install_hook, our_entry, uninstall_hook, with_our_hook};
use bd_core::bd_command::BdCommand;
use domain_types::error::ErrorCode;
use domain_types::unattended::UnattendedHook;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::path::Path;

const PROGRAM: &str = "/Users/me/.local/bin/brd";

/// The hook of `brd install` of `~/.local/bin/brd`
fn command() -> HookExec {
  HookExec {
    program: PROGRAM.to_owned(),
    args: vec!["hook".to_owned(), "permission-request".to_owned()],
  }
}

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

/// The hook of Branch Deck as the first installs wrote it: a command line of the shell form
fn shell_entry(command: &str) -> Value {
  json!({ "matcher": "*", "hooks": [{ "type": "command", "command": command }] })
}

/// The entry of `brd install` for `program` run with `args`
fn exec_entry(program: &str, args: &[&str]) -> Value {
  json!({ "matcher": "*", "hooks": [{ "type": "command", "command": program, "args": args, "onFailure": "continue" }] })
}

/// The hook runs brd with no shell on every platform: the path as it is, spaces and quotes included, the arguments
/// apart, and a failure leaves the request to the prompt
#[test]
fn the_hook_runs_brd_with_no_shell_and_never_blocks_when_it_fails() {
  let app = BdCommand {
    program: "/Applications/Branch Deck.app/Contents/MacOS/branch-deck".into(),
    subcommand: Some("cli"),
  };
  assert_eq!(
    our_entry(&HookExec::of(&app)),
    exec_entry("/Applications/Branch Deck.app/Contents/MacOS/branch-deck", &["cli", "hook", "permission-request"])
  );
  let windows = BdCommand {
    program: r"C:\Users\O'Brien\AppData\Local\Branch Deck\brd.exe".into(),
    subcommand: None,
  };
  assert_eq!(
    our_entry(&HookExec::of(&windows)),
    exec_entry(r"C:\Users\O'Brien\AppData\Local\Branch Deck\brd.exe", &["hook", "permission-request"])
  );
}

#[test]
fn an_install_adds_the_hook_once_and_keeps_every_other_key_and_hook_as_written() {
  let home = tempfile::tempdir().unwrap();
  let file = home.path().join(".claude/settings.json");
  std::fs::create_dir_all(file.parent().unwrap()).unwrap();
  std::fs::write(&file, USER_SETTINGS).unwrap();
  assert_eq!(hook_state(&file, &command()), UnattendedHook::Missing);

  assert!(install_hook(&file, &command()).unwrap(), "written");
  let text = std::fs::read_to_string(&file).unwrap();
  // the text of the user as it was: the order of the keys, the escape, the other hooks
  let (before, after) = USER_SETTINGS
    .split_once("            \"command\": \"notify.sh\"\n          }\n        ]\n      }\n")
    .unwrap();
  assert!(text.starts_with(before), "{text}");
  assert!(
    text.ends_with(&format!(
      ",\n      {{\n        \"hooks\": [\n          {{\n            \"args\": [\n              \"hook\",\n              \"permission-request\"\n            ],\n            \"command\": \"{PROGRAM}\",\n            \"onFailure\": \"continue\",\n            \"type\": \"command\"\n          }}\n        ],\n        \"matcher\": \"*\"\n      }}\n{after}"
    )),
    "{text}"
  );
  assert_eq!(read(&file)["hooks"]["PermissionRequest"][1], exec_entry(PROGRAM, &["hook", "permission-request"]));
  assert_eq!(hook_state(&file, &command()), UnattendedHook::Installed);

  assert!(!install_hook(&file, &command()).unwrap(), "an install again writes nothing");
  assert_eq!(std::fs::read_to_string(&file).unwrap(), text);
}

#[test]
fn an_install_of_a_moved_brd_replaces_the_command_of_the_old_one() {
  let home = tempfile::tempdir().unwrap();
  let file = home.path().join(".claude/settings.json");
  let old = HookExec::of(&BdCommand {
    program: "/Applications/Branch Deck.app/Contents/MacOS/branch-deck".into(),
    subcommand: Some("cli"),
  });
  assert!(install_hook(&file, &old).unwrap(), "a missing file is created");
  assert_eq!(read(&file), json!({ "hooks": { "PermissionRequest": [our_entry(&old)] } }));
  assert_eq!(hook_state(&file, &command()), UnattendedHook::Stale);

  // the user put a hook of their own beside it, and a timeout on ours
  let mut settings = read(&file);
  settings["hooks"]["PermissionRequest"][0]["hooks"][0]["timeout"] = json!(5);
  settings["hooks"]["PermissionRequest"][0]["hooks"]
    .as_array_mut()
    .unwrap()
    .push(json!({ "type": "command", "command": "log.sh" }));
  std::fs::write(&file, serde_json::to_string_pretty(&settings).unwrap()).unwrap();

  assert!(install_hook(&file, &command()).unwrap());
  assert_eq!(
    read(&file),
    json!({ "hooks": { "PermissionRequest": [
      { "matcher": "*", "hooks": [{ "type": "command", "command": "log.sh" }] },
      our_entry(&command()),
    ] } }),
    "one hook of Branch Deck, of this brd; the hook of the user stays"
  );
  assert_eq!(hook_state(&file, &command()), UnattendedHook::Installed);

  // what the user changed of the installed hook stays
  let mut settings = read(&file);
  settings["hooks"]["PermissionRequest"][1]["hooks"][0]["timeout"] = json!(5);
  settings["hooks"]["PermissionRequest"][1]["hooks"][0]["onFailure"] = json!("block");
  let text = serde_json::to_string_pretty(&settings).unwrap();
  std::fs::write(&file, &text).unwrap();
  assert!(!install_hook(&file, &command()).unwrap());
  assert_eq!(std::fs::read_to_string(&file).unwrap(), text);
}

/// The hook of the first installs, a command line of the shell form, which PowerShell cannot run: stale even of this
/// brd, replaced by an install, removed by an uninstall
#[test]
fn a_hook_of_the_shell_form_is_replaced_by_the_exec_form() {
  let home = tempfile::tempdir().unwrap();
  let file = home.path().join("settings.json");
  let shell = json!({ "hooks": { "PermissionRequest": [shell_entry(&format!("{PROGRAM} hook permission-request"))] } });
  std::fs::write(&file, shell.to_string()).unwrap();
  assert_eq!(hook_state(&file, &command()), UnattendedHook::Stale);
  assert!(install_hook(&file, &command()).unwrap());
  assert_eq!(read(&file), json!({ "hooks": { "PermissionRequest": [our_entry(&command())] } }));

  std::fs::write(&file, shell.to_string()).unwrap();
  assert!(uninstall_hook(&file).unwrap());
  assert_eq!(read(&file), json!({}));
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

  install_hook(&file, &command()).unwrap();
  assert!(uninstall_hook(&file).unwrap());
  assert_eq!(read(&file), serde_json::from_str::<Value>(USER_SETTINGS).unwrap());
  assert_eq!(hook_state(&file, &command()), UnattendedHook::Missing);
}

#[test]
fn a_file_that_is_no_object_is_refused_and_stays() {
  let home = tempfile::tempdir().unwrap();
  let file = home.path().join("settings.json");
  for text in ["[1, 2]", "{ \"model\": ", "{ \"hooks\": [] }", "{ \"hooks\": { \"PermissionRequest\": {} } }"] {
    std::fs::write(&file, text).unwrap();
    let error = install_hook(&file, &command()).unwrap_err();
    assert_eq!(error.code, ErrorCode::InstallRefused, "{text}");
    assert!(
      error.message.starts_with(&format!("Cannot add the hook of Unattended Mode to {}: ", file.display())),
      "{}",
      error.message
    );
    assert_eq!(std::fs::read_to_string(&file).unwrap(), text, "the file stays as it was");
    assert_eq!(hook_state(&file, &command()), UnattendedHook::Invalid, "{text}");
  }
  std::fs::write(&file, "\n").unwrap();
  assert!(install_hook(&file, &command()).unwrap(), "a blank file is an empty one");
  assert_eq!(read(&file), json!({ "hooks": { "PermissionRequest": [our_entry(&command())] } }));
}

/// Claude Code writes the file between the read of an install and its rename: the write stays, and the install merges
/// the hook into it
#[test]
fn a_write_of_claude_code_during_the_install_is_kept() {
  let home = tempfile::tempdir().unwrap();
  let file = home.path().join("settings.json");
  std::fs::write(&file, "{\n  \"model\": \"opus\"\n}\n").unwrap();
  let mut reads = 0;
  let written = edit(&file, |root| {
    reads += 1;
    if reads == 1 {
      std::fs::write(&file, "{\n  \"model\": \"opus\",\n  \"theme\": \"dark\"\n}\n").unwrap();
    }
    with_our_hook(&file, root, &command())
  })
  .unwrap();
  assert!(written);
  assert_eq!(reads, 2, "read again after the file changed");
  assert_eq!(
    read(&file),
    json!({ "model": "opus", "theme": "dark", "hooks": { "PermissionRequest": [our_entry(&command())] } })
  );
  let names: Vec<_> = std::fs::read_dir(home.path()).unwrap().map(|entry| entry.unwrap().file_name()).collect();
  assert_eq!(names, ["settings.json"], "no temporary file is left");

  // a file that never stops changing is refused, as Claude Code wrote it last
  std::fs::write(&file, "{}").unwrap();
  let mut writes = 0;
  let error = edit(&file, |root| {
    writes += 1;
    std::fs::write(&file, format!("{{\"n\": {writes}}}")).unwrap();
    with_our_hook(&file, root, &command())
  })
  .unwrap_err();
  assert_eq!(error.code, ErrorCode::InstallRefused, "{}", error.message);
  assert_eq!(read(&file), json!({ "n": writes }));
}

/// A settings file that is a symlink, as dotfile managers keep it, is written where it points and stays a link
#[cfg(unix)]
#[test]
fn a_settings_file_that_is_a_symlink_stays_one() {
  let home = tempfile::tempdir().unwrap();
  let dotfiles = home.path().join("dotfiles/claude");
  std::fs::create_dir_all(&dotfiles).unwrap();
  std::fs::write(dotfiles.join("settings.json"), "{\"model\": \"opus\"}").unwrap();
  let file = home.path().join(".claude/settings.json");
  std::fs::create_dir_all(file.parent().unwrap()).unwrap();
  std::os::unix::fs::symlink("../dotfiles/claude/settings.json", &file).unwrap();

  assert!(install_hook(&file, &command()).unwrap());
  assert!(std::fs::symlink_metadata(&file).unwrap().file_type().is_symlink(), "still a link");
  assert_eq!(
    read(&dotfiles.join("settings.json")),
    json!({ "model": "opus", "hooks": { "PermissionRequest": [our_entry(&command())] } })
  );
  assert_eq!(hook_state(&file, &command()), UnattendedHook::Installed);

  assert!(uninstall_hook(&file).unwrap());
  assert!(std::fs::symlink_metadata(&file).unwrap().file_type().is_symlink());
  assert_eq!(read(&dotfiles.join("settings.json")), json!({ "model": "opus" }));
}

#[cfg(unix)]
#[test]
fn a_private_file_stays_private() {
  use std::os::unix::fs::PermissionsExt;
  let home = tempfile::tempdir().unwrap();
  let file = home.path().join("settings.json");
  std::fs::write(&file, "{}").unwrap();
  std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
  install_hook(&file, &command()).unwrap();
  assert_eq!(std::fs::metadata(&file).unwrap().permissions().mode() & 0o777, 0o600);
}
