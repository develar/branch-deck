//! The hook of Unattended Mode in the user settings of Claude Code, `~/.claude/settings.json`
//! (openspec/specs/repository/unattended-mode/spec.md): `brd install` adds a `PermissionRequest` hook that runs
//! `brd hook permission-request` for every tool (`"matcher": "*"`), `brd install --uninstall` removes it, Settings →
//! Tools reads whether it is there (`hook_state`).
//!
//! The file is the user's: an edit changes only the hook of Branch Deck. Every other key, hook and value stays as it was
//! written, in its order, its text kept as it is (`RawValue`); only the objects on the way to the hook (the file,
//! `hooks`) are read as objects, and only the entries of `PermissionRequest` that held a hook of Branch Deck are written
//! anew. A hook of Branch Deck is a command hook whose command ends with `hook permission-request`, so an install
//! finds the hook of an earlier one wherever brd was then, and keeps one: the command of this brd, in one entry of its
//! own. A file that is no JSON object, or whose `hooks` is no object, is refused and stays as it is. The file is
//! written whole and renamed over the old one (`write_atomically`), so Claude Code never reads a cut file.

use bd_core::bd_command::{BdCommand, display, io_error, shell_word};
use bd_core::error::CliError;
use domain_types::error::ErrorCode;
use domain_types::unattended::UnattendedHook;
use indexmap::IndexMap;
use serde::Serialize;
use serde_json::value::RawValue;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use sync_utils::atomic_file::{AtomicWrite, write_atomically};

/// The user settings of Claude Code, under the home directory
pub const CLAUDE_SETTINGS: &str = ".claude/settings.json";
/// The event of the hook: Claude Code is about to ask the user for a permission
pub const HOOK_EVENT: &str = "PermissionRequest";
/// The arguments of brd that the hook runs, which tell a hook of Branch Deck
pub const HOOK_ARGUMENTS: &str = "hook permission-request";

/// The settings file of Claude Code under `home`
pub fn claude_settings_file(home: &Path) -> PathBuf {
  home.join(CLAUDE_SETTINGS)
}

/// The command of the hook for `brd`, quoted for the shell Claude Code runs it with: `/Users/me/.local/bin/brd hook
/// permission-request`; never PowerShell's `&`, as Claude Code runs a hook with a POSIX shell on every platform
pub fn hook_command(brd: &BdCommand) -> String {
  let program = shell_word(&display(&brd.program));
  match brd.subcommand {
    Some(subcommand) => format!("{program} {subcommand} {HOOK_ARGUMENTS}"),
    None => format!("{program} {HOOK_ARGUMENTS}"),
  }
}

/// An object of the file with its keys in their order, each value as it was written
type RawObject = IndexMap<String, Box<RawValue>>;

/// A value of the file as it is written again: kept as it was, or an object or array with some values written anew
#[derive(Serialize)]
#[serde(untagged)]
enum Node {
  Raw(Box<RawValue>),
  Object(IndexMap<String, Self>),
  Array(Vec<Self>),
  Value(Value),
}

/// The file `file` as an object: empty when it is missing or blank; an error that names it when it is no object
fn read_object(file: &Path) -> Result<RawObject, CliError> {
  let text = match std::fs::read_to_string(file) {
    Ok(text) => text,
    Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(RawObject::new()),
    Err(e) => return Err(io_error(file, &e)),
  };
  if text.trim().is_empty() {
    return Ok(RawObject::new());
  }
  serde_json::from_str(&text).map_err(|e| refused(file, &format!("it is no JSON object ({e})")))
}

fn refused(file: &Path, why: &str) -> CliError {
  CliError::new(
    ErrorCode::InstallRefused,
    format!("Cannot add the hook of Unattended Mode to {}: {why}. Fix the file, then install again", file.display()),
  )
}

/// Whether `hook`, an item of `hooks` of an entry, is a hook of Branch Deck
fn is_ours(hook: &Value) -> bool {
  hook.get("type").and_then(Value::as_str) == Some("command")
    && hook
      .get("command")
      .and_then(Value::as_str)
      .is_some_and(|command| command.trim_end().ends_with(&format!(" {HOOK_ARGUMENTS}")))
}

/// The commands of the hooks of Branch Deck in the entries of `PermissionRequest`
fn our_commands(entries: &[Value]) -> Vec<String> {
  entries
    .iter()
    .filter_map(|entry| entry.get("hooks").and_then(Value::as_array))
    .flatten()
    .filter(|hook| is_ours(hook))
    .filter_map(|hook| hook.get("command").and_then(Value::as_str).map(str::to_owned))
    .collect()
}

/// An entry of `PermissionRequest` as written, and as a value
type Entry = (Box<RawValue>, Value);

/// The hooks of the file, and the entries of `PermissionRequest`; refused when `hooks` is no object or
/// `PermissionRequest` no array
fn read_hooks(file: &Path, root: &RawObject) -> Result<(RawObject, Vec<Entry>), CliError> {
  let hooks: RawObject = match root.get("hooks") {
    Some(raw) => serde_json::from_str(raw.get()).map_err(|e| refused(file, &format!("its `hooks` is no object ({e})")))?,
    None => RawObject::new(),
  };
  let entries: Vec<Box<RawValue>> = match hooks.get(HOOK_EVENT) {
    Some(raw) => serde_json::from_str(raw.get()).map_err(|e| refused(file, &format!("its `hooks.{HOOK_EVENT}` is no array ({e})")))?,
    None => Vec::new(),
  };
  let entries = entries
    .into_iter()
    .map(|raw| {
      let value = serde_json::from_str(raw.get()).unwrap_or(Value::Null);
      (raw, value)
    })
    .collect();
  Ok((hooks, entries))
}

/// Refuses a settings file that `install_hook` and `uninstall_hook` would refuse, before an install writes anything
///
/// # Errors
/// As `install_hook`
pub fn check_settings(file: &Path) -> Result<(), CliError> {
  let root = read_object(file)?;
  read_hooks(file, &root).map(drop)
}

/// Whether the file has the hook of `command`, a hook of Branch Deck that runs another brd, or none; a file that cannot
/// be read has none
pub fn hook_state(file: &Path, command: &str) -> UnattendedHook {
  let Ok(root) = read_object(file) else {
    return UnattendedHook::Missing;
  };
  let Ok((_, entries)) = read_hooks(file, &root) else {
    return UnattendedHook::Missing;
  };
  let values: Vec<Value> = entries.into_iter().map(|(_, value)| value).collect();
  let ours = our_commands(&values);
  if ours.iter().any(|ours| ours == command) {
    UnattendedHook::Installed
  } else if ours.is_empty() {
    UnattendedHook::Missing
  } else {
    UnattendedHook::Stale
  }
}

/// The entries of `PermissionRequest` without the hooks of Branch Deck: an entry that held none as it was written, one
/// that held some without them, one left with no hook gone. Answers whether a hook of Branch Deck was there
fn without_ours(entries: Vec<Entry>) -> (Vec<Node>, bool) {
  let mut found = false;
  let mut kept = Vec::new();
  for (raw, mut value) in entries {
    let Some(hooks) = value.get_mut("hooks").and_then(Value::as_array_mut) else {
      kept.push(Node::Raw(raw));
      continue;
    };
    let before = hooks.len();
    hooks.retain(|hook| !is_ours(hook));
    if hooks.len() == before {
      kept.push(Node::Raw(raw));
      continue;
    }
    found = true;
    if !hooks.is_empty() {
      kept.push(Node::Value(value));
    }
  }
  (kept, found)
}

/// The file with `hooks` set to `hooks`, or without the key when `hooks` is empty and the file had a `hooks` that the
/// edit emptied, every other key as it was, in its order
fn with_hooks(root: RawObject, hooks: IndexMap<String, Node>) -> IndexMap<String, Node> {
  let mut result: IndexMap<String, Node> = IndexMap::new();
  let mut hooks = Some(hooks);
  for (key, raw) in root {
    if key == "hooks" {
      if let Some(hooks) = hooks.take().filter(|hooks| !hooks.is_empty()) {
        result.insert(key, Node::Object(hooks));
      }
    } else {
      result.insert(key, Node::Raw(raw));
    }
  }
  if let Some(hooks) = hooks.filter(|hooks| !hooks.is_empty()) {
    result.insert("hooks".to_owned(), Node::Object(hooks));
  }
  result
}

/// `hooks` with `entries` as `PermissionRequest` (gone when empty), every other event as it was
fn with_entries(hooks: RawObject, entries: Vec<Node>) -> IndexMap<String, Node> {
  let mut result: IndexMap<String, Node> = IndexMap::new();
  let mut entries = Some(entries);
  for (event, raw) in hooks {
    if event == HOOK_EVENT {
      if let Some(entries) = entries.take().filter(|entries| !entries.is_empty()) {
        result.insert(event, Node::Array(entries));
      }
    } else {
      result.insert(event, Node::Raw(raw));
    }
  }
  if let Some(entries) = entries.filter(|entries| !entries.is_empty()) {
    result.insert(HOOK_EVENT.to_owned(), Node::Array(entries));
  }
  result
}

/// Adds the hook of `command` to the settings file `file`, creating it, unless it is there (`is_installed`): one hook
/// of Branch Deck, of `command`, in an entry for every tool. Else every hook of Branch Deck goes, and the hook of
/// `command` comes in an entry of its own at the end. Answers whether the file was written
///
/// # Errors
/// The file is no JSON object, its `hooks` no object or `hooks.PermissionRequest` no array; it cannot be read or written
pub fn install_hook(file: &Path, command: &str) -> Result<bool, CliError> {
  let root = read_object(file)?;
  let (hooks, entries) = read_hooks(file, &root)?;
  if is_installed(entries.iter().map(|(_, entry)| entry), command) {
    return Ok(false);
  }
  let (mut kept, _) = without_ours(entries);
  kept.push(Node::Value(our_entry(command)));
  write(file, &with_hooks(root, with_entries(hooks, kept)))?;
  Ok(true)
}

/// Removes the hooks of Branch Deck from the settings file `file`, and an entry or a `hooks` that it leaves empty.
/// Answers whether there was one; a file without one is not written
///
/// # Errors
/// As `install_hook`
pub fn uninstall_hook(file: &Path) -> Result<bool, CliError> {
  if !file.exists() {
    return Ok(false);
  }
  let root = read_object(file)?;
  let (hooks, entries) = read_hooks(file, &root)?;
  let (kept, found) = without_ours(entries);
  if !found {
    return Ok(false);
  }
  write(file, &with_hooks(root, with_entries(hooks, kept)))?;
  Ok(true)
}

/// Whether `entries` hold one hook of Branch Deck, of `command`, in an entry for every tool; what else the user put
/// into its entry or its hook (a `timeout`) stays
fn is_installed<'a>(entries: impl Iterator<Item = &'a Value>, command: &str) -> bool {
  let ours: Vec<(&Value, &Value)> = entries
    .flat_map(|entry| {
      let hooks = entry.get("hooks").and_then(Value::as_array).map_or(&[][..], Vec::as_slice);
      hooks.iter().filter(|hook| is_ours(hook)).map(move |hook| (entry, hook))
    })
    .collect();
  matches!(ours.as_slice(), [(entry, hook)] if hook.get("command").and_then(Value::as_str) == Some(command) && entry.get("matcher").and_then(Value::as_str) == Some("*"))
}

/// The entry of the hook of `command`: every tool, one command hook
fn our_entry(command: &str) -> Value {
  json!({ "matcher": "*", "hooks": [{ "type": "command", "command": command }] })
}

/// Writes `root` into `file` as Claude Code writes it, two spaces of indent; the file keeps whether only its owner
/// reads it
fn write(file: &Path, root: &IndexMap<String, Node>) -> Result<(), CliError> {
  let mut text = serde_json::to_string_pretty(root).map_err(|e| CliError::new(ErrorCode::InstallRefused, e.to_string()))?;
  text.push('\n');
  if let Some(dir) = file.parent() {
    std::fs::create_dir_all(dir).map_err(|e| io_error(dir, &e))?;
  }
  let options = AtomicWrite {
    private: is_private(file),
    durable: true,
  };
  write_atomically(file, text.as_bytes(), options).map_err(|e| io_error(file, &e))
}

/// Whether only the owner may read `file` (0600 and the like), which the new file keeps
#[cfg(unix)]
fn is_private(file: &Path) -> bool {
  use std::os::unix::fs::PermissionsExt;
  std::fs::metadata(file).is_ok_and(|metadata| metadata.permissions().mode().trailing_zeros() >= 6)
}

/// Windows has no mode of a file: the new file has the permissions of its directory
#[cfg(not(unix))]
const fn is_private(_file: &Path) -> bool {
  false
}
