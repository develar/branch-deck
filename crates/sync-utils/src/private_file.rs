//! A file readable by the user only (0600) in the app data directory: the tokens of the app's own sign-ins
//! (`github-token.json`, `youtrack-token.json`) where the keychain does not keep them (`secret_store`).

use crate::atomic_file::{AtomicWrite, write_atomically};
use std::path::Path;
use tracing::warn;

/// The text of the file; None without one, and for a file others may read (it is not used, a warning says why).
/// `what` names the file in the warnings ("the GitHub token file").
pub fn read_private(path: &Path, what: &str) -> Option<String> {
  let text = match std::fs::read_to_string(path) {
    Ok(text) => text,
    Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
    Err(e) => {
      warn!(path = %path.display(), error = %e, "cannot read {what}");
      return None;
    }
  };
  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    let mode = std::fs::metadata(path).map_or(0, |metadata| metadata.permissions().mode());
    if mode & 0o077 != 0 {
      warn!(path = %path.display(), mode = format!("{mode:o}"), "{what} is readable by others: not used");
      return None;
    }
  }
  Some(text)
}

/// Writes the file: a temporary file created 0600 in the same directory, on the disk, then renamed over the old one
/// (`write_atomically`)
pub fn write_private(path: &Path, content: &[u8]) -> std::io::Result<()> {
  let dir = path.parent().ok_or_else(|| std::io::Error::other("the file has no directory"))?;
  std::fs::create_dir_all(dir)?;
  write_atomically(path, content, AtomicWrite { private: true, durable: true })
}

/// Removes the file; none is fine
pub fn remove_private(path: &Path) -> std::io::Result<()> {
  match crate::fs_retry::remove_file(path) {
    Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
    _ => Ok(()),
  }
}
