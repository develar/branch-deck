//! The watch of the app data directory by a `SettingsService` (`notify`: `FSEvents`, inotify, `ReadDirectoryChangesW`,
//! non-recursive): a change of `settings.json` or of `repository-state.json`, which the writer of another process (the
//! app, `brd mcp` of an IDE) renamed over the file, asks the writer to read the file again. A watcher that lost events
//! (an event that names no file, `need_rescan`) or failed asks for both files; its own writes are read again too, and
//! find nothing new. A directory the system refuses to watch is only logged: the files are then read again by each
//! save, which merges into them anyway.

use crate::file::{REPOSITORY_STATE_FILE_NAME, SETTINGS_FILE_NAME};
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use tracing::{debug, warn};

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

  /// The files of `event`: both when the watcher lost events or failed
  fn of(event: notify::Result<Event>) -> Self {
    let event = match event {
      Ok(event) if !event.need_rescan() => event,
      Ok(_) => return Self::BOTH,
      Err(error) => {
        debug!("the watch of the settings failed: {error}");
        return Self::BOTH;
      }
    };
    let mut changed = Self::default();
    for name in event.paths.iter().filter_map(|path| path.file_name()) {
      changed.settings |= name == SETTINGS_FILE_NAME;
      changed.repository_state |= name == REPOSITORY_STATE_FILE_NAME;
    }
    changed
  }
}

/// The watcher of `directory` (created when missing), which calls `wake` with the settings files each event names;
/// `None` when the system refused it
pub fn watch(directory: &Path, wake: impl Fn(Changed) + Send + 'static) -> Option<RecommendedWatcher> {
  let watched = std::fs::create_dir_all(directory).map_err(notify::Error::io).and_then(|()| {
    let mut watcher = notify::recommended_watcher(move |event: notify::Result<Event>| {
      let changed = Changed::of(event);
      if changed != Changed::default() {
        wake(changed);
      }
    })?;
    watcher.watch(directory, RecursiveMode::NonRecursive)?;
    Ok(watcher)
  });
  watched
    .inspect_err(|error| warn!("cannot watch {}: {error}; a change of another process shows with the next save", directory.display()))
    .ok()
}
