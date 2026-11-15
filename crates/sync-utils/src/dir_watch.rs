//! The watch of one directory by the file events of the system (`notify`, non-recursive: `FSEvents`, inotify,
//! `ReadDirectoryChangesW`), under the two rules of every watch of Branch Deck (docs/design/async-patterns.md, "A watch
//! is a hint"):
//!
//! - **An event is a hint, not a list.** `FSEvents` coalesces and drops events on a loaded machine and tells it by an
//!   event that names the directory or no file at all (`need_rescan`: `MUST_SCAN_SUBDIRS`), and a rename over a file may
//!   come as an event of its temporary name only (`write_atomically`). Such an event, an event of any name of the
//!   directory that the waiter does not look at, and an error of the watcher are `Wake::Any`: any of its files may have
//!   changed, and the waiter looks at each (`Fingerprints`).
//! - **A look of its own is the guarantee.** A waiter looks again at least every `RECHECK` without an event: with
//!   fseventsd at 100 % CPU a watch told no change of a file for 28 s and more, and a wait for its event hung.
//!
//! A look is a stat of each file (`Fingerprints::look`), so a waiter acts only on the files that changed since its last
//! look, whatever woke it. Its first look comes before its watcher (`watch_from`): `FSEvents` registers a stream after
//! `FSEventStreamStart` returned and from "now", so it drops a change of its first moments without telling it, and a
//! first look after the watcher could take that change as where it starts.

use notify::{Event, EventHandler, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// How often a waiter on a watch looks again without an event. A look of the operations registry with its stamp and
/// journal (a read of `operations.json` with three records, two stats) took 0.14 ms at a load of 64 (2026-11-15), less
/// than 0.01 % of a core every 2 s, while an operation of another process or a stamp of `brd` shows within 2 s even when
/// the watch tells nothing
pub const RECHECK: Duration = Duration::from_secs(2);

/// The most files a waiter looks at in one directory: a bit each in `Wake::Named` and `Fingerprints::look`
pub const MAX_FILES: usize = 64;

/// How a waiter on a watch learns of a change: the events of the system, and a look every `recheck` without one
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WatchConfig {
  /// How often the waiter looks without an event; `None` only in a test of the events alone
  pub recheck: Option<Duration>,
  /// Whether the events of the watcher reach the waiter; `false` only in a test of the looks alone, as a watch that
  /// tells nothing under load
  pub events: bool,
}

impl Default for WatchConfig {
  fn default() -> Self {
    Self {
      recheck: Some(RECHECK),
      events: true,
    }
  }
}

/// What an event of a watch tells of the files a waiter looks at
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wake {
  /// Nothing of them: an event of a path below or outside the directory
  None,
  /// These of them, a bit each by their index in the names of the waiter
  Named(u64),
  /// Any of them may have changed
  Any,
}

impl Wake {
  /// Whether the file of `index` in the names of the waiter may have changed
  #[must_use]
  pub const fn names(self, index: usize) -> bool {
    match self {
      Self::None => false,
      Self::Named(bits) => bits & (1 << index) != 0,
      Self::Any => true,
    }
  }

  /// Whether the event wakes the waiter at all
  #[must_use]
  pub const fn wakes(self) -> bool {
    !matches!(self, Self::None)
  }
}

/// The directory of a watch as its events name it: as given and as the real path, which `FSEvents` names
/// (`/private/var/…` for `/var/…`)
#[derive(Debug, Clone)]
pub struct WatchedDir {
  given: PathBuf,
  real: Option<PathBuf>,
}

impl WatchedDir {
  #[must_use]
  pub fn new(dir: &Path) -> Self {
    Self {
      given: dir.to_path_buf(),
      real: std::fs::canonicalize(dir).ok(),
    }
  }

  /// The directory as given
  #[must_use]
  pub fn path(&self) -> &Path {
    &self.given
  }

  fn is(&self, path: &Path) -> bool {
    path == self.given || self.real.as_deref() == Some(path)
  }

  /// What `event` tells of the files `names` of the directory (at most `MAX_FILES`): those it names; any of them for an
  /// error, a lost event (`need_rescan`), an event without a path or of the directory itself, and an event of another
  /// name of the directory (a temporary file renamed over one of them, a file the waiter does not know)
  #[must_use]
  pub fn wake_of(&self, event: &notify::Result<Event>, names: &[&str]) -> Wake {
    debug_assert!(names.len() <= MAX_FILES, "{} names", names.len());
    let Ok(event) = event else { return Wake::Any };
    if event.need_rescan() || event.paths.is_empty() {
      return Wake::Any;
    }
    let mut bits = 0_u64;
    for path in &event.paths {
      if self.is(path) {
        return Wake::Any;
      }
      if !path.parent().is_some_and(|parent| self.is(parent)) {
        continue;
      }
      match path.file_name().and_then(|name| names.iter().position(|known| name == *known)) {
        Some(index) => bits |= 1 << index,
        None => return Wake::Any,
      }
    }
    if bits == 0 { Wake::None } else { Wake::Named(bits) }
  }
}

/// The non-recursive watch of `dir` (made when missing), which gives each event to `handler`; an error when the system
/// refused one, which the looks of the waiter every `RECHECK` stand in for
///
/// # Errors
/// The directory cannot be made, or the system refused the watch
pub fn watch(dir: &Path, handler: impl EventHandler) -> notify::Result<RecommendedWatcher> {
  std::fs::create_dir_all(dir).map_err(notify::Error::io)?;
  let mut watcher = notify::recommended_watcher(handler)?;
  watcher.watch(dir, RecursiveMode::NonRecursive)?;
  Ok(watcher)
}

/// The first look at `paths`, then the watch of `dir` (`watch`): a change after the look is told by an event or found by
/// the next look, also one in the first moments of the stream, which `FSEvents` drops without telling it. A waiter that
/// looks at files starts its watch this way, never with a first look after the watcher
pub fn watch_from(dir: &Path, paths: Vec<PathBuf>, handler: impl EventHandler) -> (Fingerprints, notify::Result<RecommendedWatcher>) {
  watch_from_with(dir, paths, handler, watch)
}

/// `watch_from` with `start` as the start of the watcher, the seam of the test of the order
pub(crate) fn watch_from_with<H, W>(dir: &Path, paths: Vec<PathBuf>, handler: H, start: impl FnOnce(&Path, H) -> notify::Result<W>) -> (Fingerprints, notify::Result<W>) {
  let fingerprints = Fingerprints::new(paths);
  let watcher = start(dir, handler);
  (fingerprints, watcher)
}

/// What a stat tells of a file that changes by a rename over it or by a write in place: its size, its modification time
/// and its file id, which a rename over it changes whatever its size and time (two writes of one size within the tick
/// of the clock of the file times, about 15 ms on Windows, have one time)
#[derive(Debug, Clone, PartialEq, Eq)]
struct Fingerprint {
  len: u64,
  modified: Option<SystemTime>,
  id: u64,
}

fn fingerprint(path: &Path) -> Option<Fingerprint> {
  let metadata = std::fs::metadata(path).ok()?;
  Some(Fingerprint {
    len: metadata.len(),
    modified: metadata.modified().ok(),
    id: file_id(path, &metadata),
  })
}

/// The inode
#[cfg(unix)]
fn file_id(_path: &Path, metadata: &std::fs::Metadata) -> u64 {
  std::os::unix::fs::MetadataExt::ino(metadata)
}

/// The file index of NTFS (`GetFileInformationByHandle`), which std gives only behind a feature of nightly; 0 for a file
/// that cannot be opened, so its size and time tell alone
#[cfg(windows)]
#[expect(unsafe_code, reason = "Win32: the file index of an open file")]
fn file_id(path: &Path, _metadata: &std::fs::Metadata) -> u64 {
  use std::os::windows::io::AsRawHandle;
  use windows_sys::Win32::Storage::FileSystem::{BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle};
  // std opens with every share mode: a rename over the file or its removal goes on while it is open
  let Ok(file) = std::fs::File::open(path) else { return 0 };
  // SAFETY: a structure of plain integers, for which all zeros is a value
  let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
  // SAFETY: the handle of a file this function keeps open, and a structure of the size the call writes
  if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &raw mut info) } == 0 {
    return 0;
  }
  (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow)
}

#[cfg(not(any(unix, windows)))]
fn file_id(_path: &Path, _metadata: &std::fs::Metadata) -> u64 {
  0
}

/// The fingerprints of some files at the last look of a waiter
#[derive(Debug)]
pub struct Fingerprints {
  paths: Vec<PathBuf>,
  last: Vec<Option<Fingerprint>>,
}

impl Fingerprints {
  /// The first look at `paths` (at most `MAX_FILES`)
  #[must_use]
  pub fn new(paths: Vec<PathBuf>) -> Self {
    debug_assert!(paths.len() <= MAX_FILES, "{} paths", paths.len());
    let last = paths.iter().map(|path| fingerprint(path)).collect();
    Self { paths, last }
  }

  /// The files that changed since the last look (made, removed, written or renamed over), a bit each by index: this look
  /// is the last one from now
  pub fn look(&mut self) -> u64 {
    let mut changed = 0_u64;
    for (index, (path, last)) in self.paths.iter().zip(&mut self.last).enumerate() {
      let now = fingerprint(path);
      if now != *last {
        changed |= 1 << index;
        *last = now;
      }
    }
    changed
  }
}
