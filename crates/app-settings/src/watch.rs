//! The watch of the app data directory by a `SettingsService` (`sync_utils::dir_watch`, non-recursive): a change of
//! `settings.json` or of `repository-state.json`, which the writer of another process (the app, `brd mcp` of an IDE)
//! renamed over the file, asks the writer to read the file again. An event is a hint: an event of another name of the
//! directory (the temporary file of a rename over one of them), of the directory itself, a lost event (`need_rescan`)
//! and an error ask for both files, and the writer looks at both files every `dir_watch::RECHECK` without an event
//! (`write_loop`), as a watch may tell nothing for long under load. Its own writes are read again too, and find nothing
//! new. The watcher starts and stops on a thread of its own (`dir_watch::Watch`), so neither the open of a writer nor its
//! drop waits for fseventsd; a directory the system refuses to watch is only logged there: the looks stand in for it.

use crate::file::{REPOSITORY_STATE_FILE_NAME, SETTINGS_FILE_NAME};
use notify::Event;
use std::path::Path;
use sync_utils::dir_watch::{self, Fingerprints, Wake, Watch, WatchedDir};
use tracing::debug;

/// The settings files of the directory, in the order of the bits of a `Wake` and of a look
pub(crate) const NAMES: [&str; 2] = [SETTINGS_FILE_NAME, REPOSITORY_STATE_FILE_NAME];

/// Which settings files an event of the watch names
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Changed {
  pub settings: bool,
  pub repository_state: bool,
}

impl Changed {
  /// Both files
  pub const BOTH: Self = Self {
    settings: true,
    repository_state: true,
  };

  /// The files that changed by the bits of a look or a wake, in the order of `NAMES`
  pub(crate) const fn of_bits(bits: u64) -> Self {
    Self {
      settings: bits & 0b01 != 0,
      repository_state: bits & 0b10 != 0,
    }
  }

  /// The files `event` of the watch of `dir` may have changed (`WatchedDir::wake_of`)
  pub(crate) fn of(dir: &WatchedDir, event: &notify::Result<Event>) -> Self {
    if let Err(error) = event {
      debug!("the watch of the settings failed: {error}");
    }
    match dir.wake_of(event, &NAMES) {
      Wake::None => Self::default(),
      Wake::Named(bits) => Self::of_bits(bits),
      Wake::Any => Self::BOTH,
    }
  }
}

/// The first look at the settings files of `directory`, then its watch (`dir_watch::watch_from`; the directory is
/// created when missing), which calls `wake` with the settings files each event may have changed once its watcher
/// started on its thread. Without `events` (a test of the looks alone) its events reach nothing
pub fn watch(directory: &Path, events: bool, wake: impl Fn(Changed) + Send + 'static) -> (Fingerprints, Watch) {
  let dir = WatchedDir::new(directory);
  let paths = NAMES.iter().map(|name| directory.join(name)).collect();
  dir_watch::watch_from(directory, paths, move |event: notify::Result<Event>| {
    let changed = if events { Changed::of(&dir, &event) } else { Changed::default() };
    if changed != Changed::default() {
      wake(changed);
    }
  })
}

#[cfg(test)]
#[path = "watch_test.rs"]
mod watch_test;
