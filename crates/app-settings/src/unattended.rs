//! Unattended Mode (openspec/specs/repository/unattended-mode/spec.md), as the app and `brd unattended` switch it and
//! as the hook records what it denied: the setting `appSettings.unattended`, written only through the writer of the
//! settings, with the hook in the settings of Claude Code only while it is on (`set_unattended`), and the journal of
//! denials, `unattended-denials.jsonl` beside `settings.json` in the app data directory (`denials_file`), one JSON line
//! per denied request (`UnattendedDenial`).
//!
//! A crash or a quit while the mode is on leaves the hook and the mode on: the night goes on, and the hook keeps
//! denying until the user turns the mode off.
//!
//! The app, `brd unattended` and `brd install` each read the mode and then write the hook and the setting, each in a
//! process of its own: one switch at a time, under the lock `unattended.lock` of the app data directory
//! (`lock_switch`), held by the read, the decision and the writes of the pair, never across the network. Under it the
//! writer of the switch first takes what another process saved into memory (`SettingsService::flush_and_reread`), so
//! it decides by the mode as the file has it, not as its copy had it. What an add of the hook found empty in the
//! settings of Claude Code (`claude_hook::EmptyBefore`) is kept in `unattended-hook.json` beside the settings until the
//! removal, which keeps those containers.
//!
//! The journal is the list of one run of the mode: turning the mode on (from off) removes the list of the run before,
//! turning it off keeps it, so the morning reads it until the next night. Many hook processes append at once, each a
//! line of one `write` to a file opened for appending, which the system puts at the end whole; a reader takes the lines
//! that parse and skips the rest (a line cut by a full disk), so one bad line never costs the others.

use crate::claude_hook::{EmptyBefore, HookFileError, HookTarget, MIN_CLAUDE_CODE, add_hook, claude_code_version, remove_hook};
use crate::file::{SETTINGS_FILE_NAME, SettingsFile};
use crate::service::{SettingsError, SettingsService};
use domain_types::settings::{APP_SETTINGS_KEY, SettingsChange, SettingsPatch};
use domain_types::unattended::UnattendedDenial;
use serde_json::{Map, Value};
use std::io::Write;
use std::path::{Path, PathBuf};
use sync_utils::atomic_file::{AtomicWrite, write_atomically};
use sync_utils::fs_retry;

/// The field of `appSettings` that holds the mode
pub const UNATTENDED_FIELD: &str = "unattended";

/// The journal of denials in the app data directory
pub const DENIALS_FILE_NAME: &str = "unattended-denials.jsonl";

/// The lock of a switch in the app data directory: one read, decision and write of the hook and the setting at a time,
/// across processes
pub const LOCK_FILE_NAME: &str = "unattended.lock";

/// What the add of the hook found empty in the settings of Claude Code, in the app data directory, until the removal
pub const HOOK_MEMO_FILE_NAME: &str = "unattended-hook.json";

/// The journal of denials of the app data directory `app_data`
#[must_use]
pub fn denials_file(app_data: &Path) -> PathBuf {
  app_data.join(DENIALS_FILE_NAME)
}

/// Appends `denial` to the journal `file` as one line, creating the file; the line goes out in one write, so the lines
/// of hooks that deny at the same moment never mix
///
/// # Errors
/// The file cannot be opened or written
pub fn append_denial(file: &Path, denial: &UnattendedDenial) -> std::io::Result<()> {
  let mut line = serde_json::to_vec(denial).map_err(std::io::Error::other)?;
  line.push(b'\n');
  let mut journal = std::fs::OpenOptions::new().create(true).append(true).open(file)?;
  journal.write_all(&line)
}

/// The denials of the journal `file`, oldest first: none without the file, a line that does not parse skipped
#[must_use]
pub fn read_denials(file: &Path) -> Vec<UnattendedDenial> {
  let Ok(text) = std::fs::read_to_string(file) else {
    return Vec::new();
  };
  text.lines().filter_map(|line| serde_json::from_str(line).ok()).collect()
}

/// What `set_unattended` did
#[derive(Debug)]
pub struct UnattendedSwitch {
  /// The mode was the other way before
  pub changed: bool,
  /// The change of the settings the windows get; none when nothing changed
  pub change: Option<SettingsChange>,
  /// Why the hook stayed in the settings of Claude Code after a switch off, which is off all the same: the hook reads
  /// the mode and denies nothing while it is off
  pub warning: Option<String>,
}

/// Takes the lock of a switch in the app data directory `app_data`, waiting for the switch of another process; it goes
/// with the file
///
/// # Errors
/// The lock file cannot be created or locked
pub fn lock_switch(app_data: &Path) -> std::io::Result<std::fs::File> {
  std::fs::create_dir_all(app_data)?;
  let file = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(app_data.join(LOCK_FILE_NAME))?;
  file.lock()?;
  Ok(file)
}

/// Turns the mode on or off, `origin` the window or `brd` that asks, `hook` where the hook of the host goes (`None`:
/// the host cannot tell, no home directory or no brd).
///
/// On: a Claude Code older than `MIN_CLAUDE_CODE` (`claude --version`, when the host finds `claude`) is refused, as it
/// would run the hook without its arguments; then, under the lock of a switch (`lock_switch`) and with what another
/// process saved read (`flush_and_reread`), the hook goes into the settings of Claude Code first (`add_hook`, a hook of
/// an earlier brd replaced), so the mode is never on without it; then, from off, the journal of the run before is
/// removed, so its list starts empty, and the setting is switched. Off: the setting is switched first, then the hook
/// removed; the journal stays. Either way the setting is on the disk once this answers (`SettingsService::flush`): the
/// hook reads the file; a save that fails puts the setting back as it was, and takes away a hook the switch on added,
/// so the mode, the file and the windows agree. A switch to the state the mode is in changes no setting, and still adds
/// or removes the hook
///
/// # Errors
/// On: no place for the hook, an older Claude Code, a settings file of Claude Code that cannot take it, or a journal of
/// the run before that cannot be removed (the mode stays off). Either way: the lock cannot be taken, the writer refused
/// the change or could not save it (the mode stays as it was)
pub fn set_unattended(settings: &SettingsService, journal: &Path, on: bool, origin: &str, hook: Option<&HookTarget>) -> Result<UnattendedSwitch, UnattendedError> {
  let app_data = settings.directory();
  let target = if on {
    let hook = hook.ok_or(UnattendedError::NoHookFile)?;
    refuse_old_claude_code(hook)?;
    Some(hook)
  } else {
    None
  };
  let _lock = lock_switch(app_data).map_err(UnattendedError::Lock)?;
  // another process may have switched the mode a moment ago: the decision reads the file as it is now
  settings.flush_and_reread()?;
  let was_on = settings.app().unattended;
  if let Some(hook) = target {
    add_our_hook(app_data, hook).map_err(UnattendedError::Hook)?;
  }
  // the mode stays as it was: a hook just added for it goes again (it would deny nothing while off)
  let take_back_hook = || {
    if let Some(hook) = target {
      let _ = remove_our_hook(app_data, &hook.file);
    }
  };
  let change = if was_on == on {
    None
  } else {
    if on {
      match fs_retry::remove_file(journal) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => {
          take_back_hook();
          return Err(UnattendedError::Journal(e));
        }
      }
    }
    match settings.set(mode_patch(on), origin) {
      Ok(change) => Some(change),
      Err(e) => {
        take_back_hook();
        return Err(e.into());
      }
    }
  };
  if let Err(error) = settings.flush() {
    if change.is_some() {
      // the windows never heard the switch: the setting goes back in memory before a later save puts it into the file
      let _ = settings.set(mode_patch(was_on), origin);
      let _ = settings.flush();
      take_back_hook();
    }
    return Err(error.into());
  }
  let warning = if on {
    None
  } else {
    hook
      .and_then(|hook| remove_our_hook(app_data, &hook.file).err())
      .map(|e| format!("Cannot remove the hook of Unattended Mode from {e}"))
  };
  Ok(UnattendedSwitch {
    changed: change.is_some(),
    change,
    warning,
  })
}

/// The patch of the mode
fn mode_patch(on: bool) -> SettingsPatch {
  SettingsPatch::Fields {
    key: APP_SETTINGS_KEY.to_owned(),
    fields: Map::from_iter([(UNATTENDED_FIELD.to_owned(), Value::Bool(on))]),
  }
}

/// Refuses a Claude Code older than `MIN_CLAUDE_CODE`; a `claude` the host cannot find, or whose version it cannot
/// read, refuses nothing: the hook is written for the version the documentation names
fn refuse_old_claude_code(hook: &HookTarget) -> Result<(), UnattendedError> {
  match hook.claude.as_deref().and_then(claude_code_version) {
    Some(version) if version < MIN_CLAUDE_CODE => Err(UnattendedError::OldClaudeCode(version.map(|part| part.to_string()).join("."))),
    _ => Ok(()),
  }
}

/// What `refresh_hook` did
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HookRefresh {
  /// The mode, as the settings file of the app has it
  pub on: bool,
  /// The settings file of Claude Code was written
  pub changed: bool,
}

/// The hook in the settings of Claude Code as the mode is (`brd install`): written for `hook` while it is on, removed
/// while it is off, deciding by the settings file of the app data directory `app_data` under the lock of a switch, so a
/// switch of another process meanwhile never leaves the mode on without its hook; none `app_data` is off
///
/// # Errors
/// The lock cannot be taken; the settings file of Claude Code cannot take the edit
pub fn refresh_hook(app_data: Option<&Path>, hook: &HookTarget) -> Result<HookRefresh, UnattendedError> {
  let Some(app_data) = app_data else {
    let changed = remove_hook(&hook.file, EmptyBefore::default()).map_err(UnattendedError::Unhook)?;
    return Ok(HookRefresh { on: false, changed });
  };
  let _lock = lock_switch(app_data).map_err(UnattendedError::Lock)?;
  let on = SettingsFile::load(&app_data.join(SETTINGS_FILE_NAME)).app.unattended;
  let changed = if on {
    add_our_hook(app_data, hook).map_err(UnattendedError::Hook)?
  } else {
    remove_our_hook(app_data, &hook.file).map_err(UnattendedError::Unhook)?
  };
  Ok(HookRefresh { on, changed })
}

/// The memo of the containers the add of the hook found empty, in the app data directory `app_data`
#[must_use]
pub fn hook_memo_file(app_data: &Path) -> PathBuf {
  app_data.join(HOOK_MEMO_FILE_NAME)
}

/// Adds the hook, and keeps what the first add of a run found empty for the removal (a memo that cannot be written
/// costs only those containers). Answers whether the settings file of Claude Code was written
fn add_our_hook(app_data: &Path, hook: &HookTarget) -> Result<bool, HookFileError> {
  let added = add_hook(&hook.file, &hook.command)?;
  if let Some(first) = added.first.filter(|_| added.written)
    && let Ok(memo) = serde_json::to_vec(&first)
  {
    let _ = write_atomically(&hook_memo_file(app_data), &memo, AtomicWrite::default());
  }
  Ok(added.written)
}

/// Removes the hook, keeping the containers its add found empty, and forgets them once the hook is gone. Answers
/// whether the hook was there
fn remove_our_hook(app_data: &Path, file: &Path) -> Result<bool, HookFileError> {
  let memo = hook_memo_file(app_data);
  let keep = std::fs::read(&memo).ok().and_then(|bytes| serde_json::from_slice(&bytes).ok()).unwrap_or_default();
  let removed = remove_hook(file, keep)?;
  match fs_retry::remove_file(&memo) {
    Ok(()) => {}
    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
    Err(e) => tracing::debug!(memo = %memo.display(), error = %e, "the memo of the hook stays"),
  }
  Ok(removed)
}

/// Why `set_unattended` or `refresh_hook` changed nothing
#[derive(Debug, thiserror::Error)]
pub enum UnattendedError {
  #[error("no home directory of the user or no brd binary to run, so no hook for the settings of Claude Code")]
  NoHookFile,
  #[error(
    "Claude Code {0} runs the hook of Unattended Mode without its arguments, so it would deny nothing. Update Claude Code to {min} or later, then turn the mode on again",
    min = MIN_CLAUDE_CODE.map(|part| part.to_string()).join(".")
  )]
  OldClaudeCode(String),
  #[error("cannot add the hook of Unattended Mode to {0}. Fix the file, then turn the mode on again")]
  Hook(#[source] HookFileError),
  #[error("cannot remove the hook of Unattended Mode from {0}")]
  Unhook(#[source] HookFileError),
  #[error("cannot remove the denied requests of the last run: {0}")]
  Journal(#[source] std::io::Error),
  #[error("cannot lock {LOCK_FILE_NAME}: {0}")]
  Lock(#[source] std::io::Error),
  #[error(transparent)]
  Settings(#[from] SettingsError),
}
