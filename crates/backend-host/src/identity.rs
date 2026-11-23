//! The build of a binary and the rule of another build (docs/design/backend-protocol.md, "The socket", "Another
//! build"): the build id is the version and the commit the binary was built from (`build.rs`: `BRANCH_DECK_COMMIT` of
//! the build, `-dirty` for a tree with changes, else `GITHUB_SHA` of CI, else `dev`), compared byte for byte; the stale side yields, stale being a
//! process whose executable on disk is no longer the file it started from (the app updated in place), told by its
//! identity: the device, the inode, the size and the modification time (on Windows the size, the modification and the
//! creation time).

use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// The build id of this binary: its version and the commit its build named (`build.rs`: `BRANCH_DECK_COMMIT`, else
/// `GITHUB_SHA`, else `dev`)
pub fn build_id() -> String {
  format!("{}+{}", env!("CARGO_PKG_VERSION"), env!("BACKEND_BUILD_COMMIT"))
}

/// What tells one file of an executable from another at the same path
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileIdentity {
  device: u64,
  inode: u64,
  size: u64,
  modified: Option<SystemTime>,
  created: Option<SystemTime>,
}

impl FileIdentity {
  /// The identity of the file at `path` now; none when it cannot be read (it was removed)
  pub fn of(path: &Path) -> Option<Self> {
    let metadata = std::fs::metadata(path).ok()?;
    #[cfg(unix)]
    let (device, inode, created) = {
      use std::os::unix::fs::MetadataExt;
      (metadata.dev(), metadata.ino(), None)
    };
    #[cfg(not(unix))]
    let (device, inode, created) = (0, 0, metadata.created().ok());
    Some(Self {
      device,
      inode,
      size: metadata.len(),
      modified: metadata.modified().ok(),
      created,
    })
  }
}

/// The executable of this process as it started, which tells whether it became stale
#[derive(Debug, Clone)]
pub struct Executable {
  path: PathBuf,
  at_start: Option<FileIdentity>,
}

impl Executable {
  /// The executable at `path`, its identity read now (at the start of the process)
  pub fn at(path: PathBuf) -> Self {
    let at_start = FileIdentity::of(&path);
    Self { path, at_start }
  }

  /// The executable of this process; none where the system does not tell it
  pub fn current() -> Option<Self> {
    std::env::current_exe().ok().map(Self::at)
  }

  /// The file on disk is no longer the one the process started from: another build replaced it (an update in place),
  /// or it went. An executable whose identity was unknown at the start is never stale
  pub fn is_stale(&self) -> bool {
    self.at_start.as_ref().is_some_and(|at_start| FileIdentity::of(&self.path).as_ref() != Some(at_start))
  }
}

#[cfg(test)]
#[path = "identity_test.rs"]
mod identity_test;
