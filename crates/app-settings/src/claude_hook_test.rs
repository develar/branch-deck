use crate::claude_hook::{HookExec, add_hook, edit, has_hook, our_entry, remove_hook, with_our_hook};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::path::Path;

const PROGRAM: &str = "/Users/me/.local/bin/brd";

/// The hook of `~/.local/bin/brd`
fn command() -> HookExec {
  HookExec::new(PROGRAM.to_owned(), None)
}

/// The hook of the app binary of another place, run with `cli`
fn moved_app() -> HookExec {
  HookExec::new("/Applications/Branch Deck.app/Contents/MacOS/branch-deck".to_owned(), Some("cli"))
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

/// The entry of the hook of `program` run with `args`
fn exec_entry(program: &str, args: &[&str]) -> Value {
  json!({ "matcher": "*", "hooks": [{ "type": "command", "command": program, "args": args, "onFailure": "continue" }] })
}

/// The hook runs brd with no shell on every platform: the path as it is, spaces and quotes included, the arguments
/// apart, and a failure leaves the request to the prompt
#[test]
fn the_hook_runs_brd_with_no_shell_and_never_blocks_when_it_fails() {
  assert_eq!(
    our_entry(&moved_app()),
    exec_entry("/Applications/Branch Deck.app/Contents/MacOS/branch-deck", &["cli", "hook", "permission-request"])
  );
  let windows = HookExec::new(r"C:\Users\O'Brien\AppData\Local\Branch Deck\brd.exe".to_owned(), None);
  assert_eq!(
    our_entry(&windows),
    exec_entry(r"C:\Users\O'Brien\AppData\Local\Branch Deck\brd.exe", &["hook", "permission-request"])
  );
}

#[test]
fn an_add_writes_the_hook_once_and_keeps_every_other_key_and_hook_as_written() {
  let home = tempfile::tempdir().unwrap();
  let file = home.path().join(".claude/settings.json");
  std::fs::create_dir_all(file.parent().unwrap()).unwrap();
  std::fs::write(&file, USER_SETTINGS).unwrap();
  assert!(!has_hook(&file));

  assert!(add_hook(&file, &command()).unwrap(), "written");
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
  assert!(has_hook(&file));

  assert!(!add_hook(&file, &command()).unwrap(), "an add again writes nothing");
  assert_eq!(std::fs::read_to_string(&file).unwrap(), text);
}

#[test]
fn an_add_of_a_moved_brd_replaces_the_command_of_the_old_one() {
  let home = tempfile::tempdir().unwrap();
  let file = home.path().join(".claude/settings.json");
  let old = moved_app();
  assert!(add_hook(&file, &old).unwrap(), "a missing file is created");
  assert_eq!(read(&file), json!({ "hooks": { "PermissionRequest": [our_entry(&old)] } }));
  assert!(has_hook(&file), "a hook of Branch Deck all the same");

  // the user put a hook of their own beside it, and a timeout on ours
  let mut settings = read(&file);
  settings["hooks"]["PermissionRequest"][0]["hooks"][0]["timeout"] = json!(5);
  settings["hooks"]["PermissionRequest"][0]["hooks"]
    .as_array_mut()
    .unwrap()
    .push(json!({ "type": "command", "command": "log.sh" }));
  std::fs::write(&file, serde_json::to_string_pretty(&settings).unwrap()).unwrap();

  assert!(add_hook(&file, &command()).unwrap());
  assert_eq!(
    read(&file),
    json!({ "hooks": { "PermissionRequest": [
      { "matcher": "*", "hooks": [{ "type": "command", "command": "log.sh" }] },
      our_entry(&command()),
    ] } }),
    "one hook of Branch Deck, of this brd; the hook of the user stays"
  );
  assert!(has_hook(&file));

  // what the user changed of the installed hook stays
  let mut settings = read(&file);
  settings["hooks"]["PermissionRequest"][1]["hooks"][0]["timeout"] = json!(5);
  settings["hooks"]["PermissionRequest"][1]["hooks"][0]["onFailure"] = json!("block");
  let text = serde_json::to_string_pretty(&settings).unwrap();
  std::fs::write(&file, &text).unwrap();
  assert!(!add_hook(&file, &command()).unwrap());
  assert_eq!(std::fs::read_to_string(&file).unwrap(), text);
}

/// The hook of the first installs, a command line of the shell form, which PowerShell cannot run: Branch Deck's even of
/// this brd, replaced by an add, removed by a removal
#[test]
fn a_hook_of_the_shell_form_is_replaced_by_the_exec_form() {
  let home = tempfile::tempdir().unwrap();
  let file = home.path().join("settings.json");
  let shell = json!({ "hooks": { "PermissionRequest": [shell_entry(&format!("{PROGRAM} hook permission-request"))] } });
  std::fs::write(&file, shell.to_string()).unwrap();
  assert!(has_hook(&file), "a hook of Branch Deck all the same");
  assert!(add_hook(&file, &command()).unwrap());
  assert_eq!(read(&file), json!({ "hooks": { "PermissionRequest": [our_entry(&command())] } }));

  std::fs::write(&file, shell.to_string()).unwrap();
  assert!(remove_hook(&file).unwrap());
  assert_eq!(read(&file), json!({}));
}

#[test]
fn a_removal_takes_only_the_hook_of_branch_deck_and_gives_the_file_back() {
  let home = tempfile::tempdir().unwrap();
  let file = home.path().join(".claude/settings.json");
  assert!(!remove_hook(&file).unwrap(), "no file, nothing to remove");
  std::fs::create_dir_all(file.parent().unwrap()).unwrap();
  std::fs::write(&file, USER_SETTINGS).unwrap();
  assert!(!remove_hook(&file).unwrap(), "no hook of Branch Deck");
  assert_eq!(std::fs::read_to_string(&file).unwrap(), USER_SETTINGS, "and the file stays as it was");

  add_hook(&file, &command()).unwrap();
  assert!(remove_hook(&file).unwrap());
  assert_eq!(std::fs::read_to_string(&file).unwrap(), USER_SETTINGS, "the text of the user, byte for byte");
  assert!(!has_hook(&file));

  // a file without `hooks`: the `hooks` the add made goes with the hook
  let plain = "{\n  \"model\": \"opus\"\n}\n";
  std::fs::write(&file, plain).unwrap();
  add_hook(&file, &command()).unwrap();
  assert!(remove_hook(&file).unwrap());
  assert_eq!(std::fs::read_to_string(&file).unwrap(), plain);
}

#[test]
fn a_file_that_is_no_object_is_refused_and_stays() {
  let home = tempfile::tempdir().unwrap();
  let file = home.path().join("settings.json");
  for text in ["[1, 2]", "{ \"model\": ", "{ \"hooks\": [] }", "{ \"hooks\": { \"PermissionRequest\": {} } }"] {
    std::fs::write(&file, text).unwrap();
    let error = add_hook(&file, &command()).unwrap_err();
    assert_eq!(error.file, file, "{text}");
    assert!(error.to_string().starts_with(&format!("{}: ", file.display())), "{error}");
    assert_eq!(remove_hook(&file).unwrap_err(), error, "a removal is refused alike");
    assert_eq!(std::fs::read_to_string(&file).unwrap(), text, "the file stays as it was");
    assert!(!has_hook(&file), "{text}");
  }
  std::fs::write(&file, "\n").unwrap();
  assert!(add_hook(&file, &command()).unwrap(), "a blank file is an empty one");
  assert_eq!(read(&file), json!({ "hooks": { "PermissionRequest": [our_entry(&command())] } }));
}

/// Claude Code writes the file between the read of an edit and its rename: the write stays, and the edit merges the hook
/// into it
#[test]
fn a_write_of_claude_code_during_an_edit_is_kept() {
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
  assert_eq!(error.why, "another program kept writing it");
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

  assert!(add_hook(&file, &command()).unwrap());
  assert!(std::fs::symlink_metadata(&file).unwrap().file_type().is_symlink(), "still a link");
  assert_eq!(
    read(&dotfiles.join("settings.json")),
    json!({ "model": "opus", "hooks": { "PermissionRequest": [our_entry(&command())] } })
  );
  assert!(has_hook(&file));

  assert!(remove_hook(&file).unwrap());
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
  add_hook(&file, &command()).unwrap();
  assert_eq!(std::fs::metadata(&file).unwrap().permissions().mode() & 0o777, 0o600);
}
