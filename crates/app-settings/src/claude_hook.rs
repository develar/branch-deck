//! The hook of Unattended Mode in the user settings of Claude Code, `~/.claude/settings.json`
//! (openspec/specs/repository/unattended-mode/spec.md): a `PermissionRequest` hook that runs `brd hook
//! permission-request` for every tool (`"matcher": "*"`), in the file only while the mode is on. Turning the mode on
//! adds it before the setting (`add_hook`), turning it off removes it after (`remove_hook`), so the file of a user who
//! never turns the mode on is never written; Claude Code reads a hook written into the file while a session runs.
//!
//! The hook is a command hook of the exec form (`HookExec`): `command` is the path of brd and `args` its arguments,
//! which Claude Code starts with no shell on every platform. A command line of the shell form would run in `sh` on macOS
//! and Linux but in PowerShell on Windows without Git Bash, which cannot run a quoted path without `&`; and there a hook
//! that fails blocks the tool call. `"onFailure": "continue"` says the same on every platform: a hook that cannot start
//! or fails leaves the request to the prompt of the user, as the mode never blocks anything.
//!
//! The file is the user's: an edit changes only the hook of Branch Deck. Every other key, hook and value stays as it was
//! written, in its order, its text kept as it is (`RawValue`); only the objects on the way to the hook (the file,
//! `hooks`) are read as objects, and only the entries of `PermissionRequest` that held a hook of Branch Deck are written
//! anew; a `PermissionRequest` and a `hooks` the removal empties go, so the file comes back to what it was. A hook of
//! Branch Deck is a command hook whose `args` end with `hook permission-request`, or one of the shell form whose command
//! ends with it (as the first installs wrote it), so an edit finds the hook of an earlier one wherever brd was then, and
//! keeps one: the command of this brd, in one entry of its own. A file that is no JSON object, or whose `hooks` is no
//! object, is refused and stays as it is.
//!
//! The file is written whole and renamed over the old one, so Claude Code never reads a cut file; a settings file that
//! is a symlink (stow, chezmoi) is written where it points and stays a link. Claude Code writes the file too: just
//! before the rename the file is read again (`replace_if_unchanged`), and when it changed since the read the edit reads
//! and merges again, so a write of Claude Code during the edit is kept.

use indexmap::IndexMap;
use serde::Serialize;
use serde_json::value::RawValue;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use sync_utils::atomic_file::{AtomicWrite, replace_if_unchanged};

/// The user settings of Claude Code, under the home directory
pub const CLAUDE_SETTINGS: &str = ".claude/settings.json";
/// The event of the hook: Claude Code is about to ask the user for a permission
pub const HOOK_EVENT: &str = "PermissionRequest";
/// The arguments of brd that the hook runs, which tell a hook of Branch Deck
pub const HOOK_ARGUMENTS: [&str; 2] = ["hook", "permission-request"];
/// What Claude Code does when the hook fails: the request goes on to the prompt, never blocked
pub const ON_FAILURE: &str = "continue";
/// How many times an edit reads, merges and writes the file while Claude Code keeps writing it
const EDIT_ATTEMPTS: usize = 5;
/// How many symlinks the settings file may go through
const MAX_LINKS: usize = 40;

/// The settings file of Claude Code under `home`
#[must_use]
pub fn claude_settings_file(home: &Path) -> PathBuf {
  home.join(CLAUDE_SETTINGS)
}

/// The command of the hook, of the exec form: the program that Claude Code starts with no shell, and its arguments
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookExec {
  /// The path of brd, as it is: `/Users/me/.local/bin/brd`, `C:\…\brd.exe`
  pub program: String,
  /// `hook permission-request`, after `subcommand` (`cli` when the program is the app binary)
  pub args: Vec<String>,
}

impl HookExec {
  /// The hook of the brd `program`, run with `subcommand` before the arguments of the hook
  #[must_use]
  pub fn new(program: String, subcommand: Option<&str>) -> Self {
    Self {
      program,
      args: subcommand.into_iter().chain(HOOK_ARGUMENTS).map(str::to_owned).collect(),
    }
  }
}

/// Where the hook of a host goes and what it runs: the settings file of Claude Code of the user and the brd of the host
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookTarget {
  pub file: PathBuf,
  pub command: HookExec,
}

/// Why the settings file could not take the edit: it names the file and says why
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{}: {why}", file.display())]
pub struct HookFileError {
  pub file: PathBuf,
  pub why: String,
}

impl HookFileError {
  fn new(file: &Path, why: impl Into<String>) -> Self {
    Self {
      file: file.to_path_buf(),
      why: why.into(),
    }
  }
}

/// An object of the file with its keys in their order, each value as it was written
pub(crate) type RawObject = IndexMap<String, Box<RawValue>>;

/// A value of the file as it is written again: kept as it was, or an object or array with some values written anew
#[derive(Serialize)]
#[serde(untagged)]
pub(crate) enum Node {
  Raw(Box<RawValue>),
  Object(IndexMap<String, Self>),
  Array(Vec<Self>),
  Value(Value),
}

/// The bytes of `file`, `None` when it is missing
fn read_bytes(file: &Path) -> Result<Option<Vec<u8>>, HookFileError> {
  match std::fs::read(file) {
    Ok(bytes) => Ok(Some(bytes)),
    Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
    Err(e) => Err(HookFileError::new(file, e.to_string())),
  }
}

/// The settings file `file` of the bytes `read` as an object: empty when it is missing or blank; an error that names it
/// when it is no object
fn parse(file: &Path, read: Option<&[u8]>) -> Result<RawObject, HookFileError> {
  let Some(bytes) = read else {
    return Ok(RawObject::new());
  };
  let text = std::str::from_utf8(bytes).map_err(|e| HookFileError::new(file, format!("it is no UTF-8 text ({e})")))?;
  if text.trim().is_empty() {
    return Ok(RawObject::new());
  }
  serde_json::from_str(text).map_err(|e| HookFileError::new(file, format!("it is no JSON object ({e})")))
}

/// Whether `hook`, an item of `hooks` of an entry, is a hook of Branch Deck: of the exec form whose `args` end with
/// `hook permission-request`, or of the shell form whose command ends with it
fn is_ours(hook: &Value) -> bool {
  if hook.get("type").and_then(Value::as_str) != Some("command") {
    return false;
  }
  match hook.get("args") {
    Some(args) => args_of(args).is_some_and(|args| args.ends_with(&HOOK_ARGUMENTS)),
    None => hook
      .get("command")
      .and_then(Value::as_str)
      .is_some_and(|command| command.trim_end().ends_with(&format!(" {}", HOOK_ARGUMENTS.join(" ")))),
  }
}

/// The `args` of a hook, when they are strings
fn args_of(args: &Value) -> Option<Vec<&str>> {
  args.as_array()?.iter().map(Value::as_str).collect()
}

/// Whether `hook` runs `command`: its program, with its arguments
fn runs(hook: &Value, command: &HookExec) -> bool {
  hook.get("command").and_then(Value::as_str) == Some(command.program.as_str()) && hook.get("args").and_then(args_of).is_some_and(|args| args == command.args)
}

/// The hooks of Branch Deck in the entries of `PermissionRequest`
fn our_hooks(entries: &[Value]) -> Vec<&Value> {
  entries
    .iter()
    .filter_map(|entry| entry.get("hooks").and_then(Value::as_array))
    .flatten()
    .filter(|hook| is_ours(hook))
    .collect()
}

/// An entry of `PermissionRequest` as written, and as a value
type Entry = (Box<RawValue>, Value);

/// The hooks of the file, and the entries of `PermissionRequest`; refused when `hooks` is no object or
/// `PermissionRequest` no array
fn read_hooks(file: &Path, root: &RawObject) -> Result<(RawObject, Vec<Entry>), HookFileError> {
  let hooks: RawObject = match root.get("hooks") {
    Some(raw) => serde_json::from_str(raw.get()).map_err(|e| HookFileError::new(file, format!("its `hooks` is no object ({e})")))?,
    None => RawObject::new(),
  };
  let entries: Vec<Box<RawValue>> = match hooks.get(HOOK_EVENT) {
    Some(raw) => serde_json::from_str(raw.get()).map_err(|e| HookFileError::new(file, format!("its `hooks.{HOOK_EVENT}` is no array ({e})")))?,
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

/// Whether the file holds a hook of Branch Deck, of any brd and form; a file that cannot be read holds none
#[must_use]
pub fn has_hook(file: &Path) -> bool {
  let entries = read_bytes(file)
    .and_then(|read| parse(file, read.as_deref()))
    .and_then(|root| read_hooks(file, &root).map(|(_, entries)| entries));
  entries.is_ok_and(|entries| {
    let values: Vec<Value> = entries.into_iter().map(|(_, value)| value).collect();
    !our_hooks(&values).is_empty()
  })
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

/// Reads the settings file `file`, a symlink where it points, and writes what `change` makes of it, unless it answers
/// `None`; when the file changed between the read and the rename it reads and changes it again. Answers whether the
/// file was written
///
/// # Errors
/// What `change` answers; the file is no JSON object, cannot be read or written, or kept changing
pub(crate) fn edit(file: &Path, mut change: impl FnMut(RawObject) -> Result<Option<IndexMap<String, Node>>, HookFileError>) -> Result<bool, HookFileError> {
  let target = link_target(file).map_err(|e| HookFileError::new(file, e.to_string()))?;
  for _ in 0..EDIT_ATTEMPTS {
    let read = read_bytes(&target)?;
    let Some(root) = change(parse(file, read.as_deref())?)? else {
      return Ok(false);
    };
    if write(&target, &root, read.as_deref())? {
      return Ok(true);
    }
  }
  Err(HookFileError::new(file, "another program kept writing it"))
}

/// The file a symlink `file` points at, through every link; `file` itself when it is no link or is missing
fn link_target(file: &Path) -> std::io::Result<PathBuf> {
  let mut path = file.to_path_buf();
  for _ in 0..MAX_LINKS {
    match std::fs::symlink_metadata(&path) {
      Ok(metadata) if metadata.file_type().is_symlink() => {
        let points_to = std::fs::read_link(&path)?;
        // a relative link is relative to its directory; an absolute one replaces the path
        path = path.parent().map_or_else(|| points_to.clone(), |dir| dir.join(&points_to));
      }
      Ok(_) => return Ok(path),
      Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(path),
      Err(e) => return Err(e),
    }
  }
  Err(std::io::Error::other(format!("more than {MAX_LINKS} symbolic links")))
}

/// Adds the hook of `command` to the settings file `file`, creating it, unless it is there (`is_installed`): one hook
/// of Branch Deck, of `command`, in an entry for every tool. Else every hook of Branch Deck goes, and the hook of
/// `command` comes in an entry of its own at the end. Answers whether the file was written
///
/// # Errors
/// The file is no JSON object, its `hooks` no object or `hooks.PermissionRequest` no array; it cannot be read or
/// written, or kept changing
pub fn add_hook(file: &Path, command: &HookExec) -> Result<bool, HookFileError> {
  edit(file, |root| with_our_hook(file, root, command))
}

/// The file `root` with the hook of `command`, `None` when it is there
///
/// # Errors
/// As `add_hook`
pub(crate) fn with_our_hook(file: &Path, root: RawObject, command: &HookExec) -> Result<Option<IndexMap<String, Node>>, HookFileError> {
  let (hooks, entries) = read_hooks(file, &root)?;
  if is_installed(entries.iter().map(|(_, entry)| entry), command) {
    return Ok(None);
  }
  let (mut kept, _) = without_ours(entries);
  kept.push(Node::Value(our_entry(command)));
  Ok(Some(with_hooks(root, with_entries(hooks, kept))))
}

/// Removes the hooks of Branch Deck from the settings file `file`, and an entry or a `hooks` that it leaves empty.
/// Answers whether there was one; a file without one is not written
///
/// # Errors
/// As `add_hook`
pub fn remove_hook(file: &Path) -> Result<bool, HookFileError> {
  edit(file, |root| {
    let (hooks, entries) = read_hooks(file, &root)?;
    let (kept, found) = without_ours(entries);
    Ok(found.then(|| with_hooks(root, with_entries(hooks, kept))))
  })
}

/// Whether `entries` hold one hook of Branch Deck, of `command`, in an entry for every tool; what else the user put
/// into its entry or its hook (a `timeout`, another `onFailure`) stays
fn is_installed<'a>(entries: impl Iterator<Item = &'a Value>, command: &HookExec) -> bool {
  let ours: Vec<(&Value, &Value)> = entries
    .flat_map(|entry| {
      let hooks = entry.get("hooks").and_then(Value::as_array).map_or(&[][..], Vec::as_slice);
      hooks.iter().filter(|hook| is_ours(hook)).map(move |hook| (entry, hook))
    })
    .collect();
  matches!(ours.as_slice(), [(entry, hook)] if runs(hook, command) && entry.get("matcher").and_then(Value::as_str) == Some("*"))
}

/// The entry of the hook of `command`: every tool, one command hook of the exec form, which never blocks when it fails
pub(crate) fn our_entry(command: &HookExec) -> Value {
  json!({
    "matcher": "*",
    "hooks": [{ "type": "command", "command": command.program, "args": command.args, "onFailure": ON_FAILURE }],
  })
}

/// Writes `root` into `target` as Claude Code writes it, two spaces of indent, while it still holds `read`; the file
/// keeps whether only its owner reads it. Answers whether it was written
fn write(target: &Path, root: &IndexMap<String, Node>, read: Option<&[u8]>) -> Result<bool, HookFileError> {
  let mut text = serde_json::to_string_pretty(root).map_err(|e| HookFileError::new(target, e.to_string()))?;
  text.push('\n');
  if let Some(dir) = target.parent() {
    std::fs::create_dir_all(dir).map_err(|e| HookFileError::new(dir, e.to_string()))?;
  }
  let options = AtomicWrite {
    private: is_private(target),
    durable: true,
  };
  replace_if_unchanged(target, text.as_bytes(), read, options).map_err(|e| HookFileError::new(target, e.to_string()))
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
