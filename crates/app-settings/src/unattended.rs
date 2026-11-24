//! Unattended Mode (openspec/specs/repository/unattended-mode/spec.md), as the app and `brd unattended` switch it and
//! as the hook records what it denied: the setting `appSettings.unattended`, written only through the writer of the
//! settings (`set_unattended`), and the journal of denials, `unattended-denials.jsonl` beside `settings.json` in the app
//! data directory (`denials_file`), one JSON line per denied request (`UnattendedDenial`).
//!
//! The journal is the list of one run of the mode: turning the mode on (from off) removes the list of the run before,
//! turning it off keeps it, so the morning reads it until the next night. Many hook processes append at once, each a
//! line of one `write` to a file opened for appending, which the system puts at the end whole; a reader takes the lines
//! that parse and skips the rest (a line cut by a full disk), so one bad line never costs the others.

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
}

/// Turns the mode on or off through the writer of the settings, `origin` the window or `brd` that asks. Turning it on
/// from off first removes the journal of the run before, so its list starts empty; turning it off keeps the journal.
/// The change is on the disk once this answers (`SettingsService::flush`): the hook reads the file, so after the switch
/// off of the app or of `brd` no request is denied. A switch to the state the mode is in changes nothing
///
/// # Errors
/// The journal of the run before cannot be removed (the mode stays off), the writer refused the change, or it could not
/// save it (the writer saves it again later)
pub fn set_unattended(settings: &SettingsService, journal: &Path, on: bool, origin: &str) -> Result<UnattendedSwitch, UnattendedError> {
  if settings.app().unattended == on {
    return Ok(UnattendedSwitch { changed: false, change: None });
  }
  if on {
    match fs_retry::remove_file(journal) {
      Ok(()) => {}
      Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
      Err(e) => return Err(UnattendedError::Journal(e)),
    }
  }
  let fields = Map::from_iter([(UNATTENDED_FIELD.to_owned(), Value::Bool(on))]);
  let change = settings.set(
    SettingsPatch::Fields {
      key: APP_SETTINGS_KEY.to_owned(),
      fields,
    },
    origin,
  )?;
  settings.flush()?;
  Ok(UnattendedSwitch {
    changed: true,
    change: Some(change),
  })
}

/// Why `set_unattended` changed nothing
#[derive(Debug, thiserror::Error)]
pub enum UnattendedError {
  #[error("cannot remove the denied requests of the last run: {0}")]
  Journal(#[source] std::io::Error),
  #[error(transparent)]
  Settings(#[from] SettingsError),
}
