//! Unattended Mode (openspec/specs/repository/unattended-mode/spec.md), as the app and `brd unattended` switch it and
//! as the hook records what it denied: the setting `appSettings.unattended`, written only through the writer of the
//! settings, with the hook in the settings of Claude Code only while it is on (`set_unattended`), and the journal of
//! denials, `unattended-denials.jsonl` beside `settings.json` in the app data directory (`denials_file`), one JSON line
//! per denied request (`UnattendedDenial`).
//!
//! A crash or a quit while the mode is on leaves the hook and the mode on: the night goes on, and the hook keeps
//! denying until the user turns the mode off.
//!
//! The journal is the list of one run of the mode: turning the mode on (from off) removes the list of the run before,
//! turning it off keeps it, so the morning reads it until the next night. Many hook processes append at once, each a
//! line of one `write` to a file opened for appending, which the system puts at the end whole; a reader takes the lines
//! that parse and skips the rest (a line cut by a full disk), so one bad line never costs the others.

use crate::claude_hook::{HookFileError, HookTarget, add_hook, remove_hook};
use crate::service::{SettingsError, SettingsService};
use domain_types::settings::{APP_SETTINGS_KEY, SettingsChange, SettingsPatch};
use domain_types::unattended::UnattendedDenial;
use serde_json::{Map, Value};
use std::io::Write;
use std::path::{Path, PathBuf};
use sync_utils::fs_retry;

/// The field of `appSettings` that holds the mode
pub const UNATTENDED_FIELD: &str = "unattended";

/// The journal of denials in the app data directory
pub const DENIALS_FILE_NAME: &str = "unattended-denials.jsonl";

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

/// Turns the mode on or off, `origin` the window or `brd` that asks, `hook` where the hook of the host goes (`None`:
/// the host cannot tell, no home directory or no brd).
///
/// On: the hook goes into the settings of Claude Code first (`claude_hook::add_hook`, a hook of an earlier brd
/// replaced), so the mode is never on without it; then, from off, the journal of the run before is removed, so its list
/// starts empty, and the setting is switched. Off: the setting is switched first, then the hook removed; the journal
/// stays. Either way the setting is on the disk once this answers (`SettingsService::flush`): the hook reads the file.
/// A switch to the state the mode is in changes no setting, and still adds or removes the hook
///
/// # Errors
/// On: no place for the hook, a settings file of Claude Code that cannot take it, or a journal of the run before that
/// cannot be removed (the mode stays off). Either way: the writer refused the change, or could not save it (the writer
/// saves it again later)
pub fn set_unattended(settings: &SettingsService, journal: &Path, on: bool, origin: &str, hook: Option<&HookTarget>) -> Result<UnattendedSwitch, UnattendedError> {
  if on {
    let hook = hook.ok_or(UnattendedError::NoHookFile)?;
    add_hook(&hook.file, &hook.command).map_err(UnattendedError::Hook)?;
  }
  let change = if settings.app().unattended == on {
    None
  } else {
    if on {
      match fs_retry::remove_file(journal) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(UnattendedError::Journal(e)),
      }
    }
    let fields = Map::from_iter([(UNATTENDED_FIELD.to_owned(), Value::Bool(on))]);
    let patch = SettingsPatch::Fields {
      key: APP_SETTINGS_KEY.to_owned(),
      fields,
    };
    match settings.set(patch, origin) {
      Ok(change) => Some(change),
      Err(e) => {
        // the mode stays as it was: a hook just added for it goes again (it would deny nothing while off)
        if let (true, Some(hook)) = (on, hook) {
          let _ = remove_hook(&hook.file);
        }
        return Err(e.into());
      }
    }
  };
  settings.flush()?;
  let warning = if on {
    None
  } else {
    hook
      .and_then(|hook| remove_hook(&hook.file).err())
      .map(|e| format!("Cannot remove the hook of Unattended Mode from {e}"))
  };
  Ok(UnattendedSwitch {
    changed: change.is_some(),
    change,
    warning,
  })
}

/// Why `set_unattended` changed nothing
#[derive(Debug, thiserror::Error)]
pub enum UnattendedError {
  #[error("no home directory of the user or no brd binary to run, so no hook for the settings of Claude Code")]
  NoHookFile,
  #[error("cannot add the hook of Unattended Mode to {0}. Fix the file, then turn the mode on again")]
  Hook(#[source] HookFileError),
  #[error("cannot remove the denied requests of the last run: {0}")]
  Journal(#[source] std::io::Error),
  #[error(transparent)]
  Settings(#[from] SettingsError),
}
