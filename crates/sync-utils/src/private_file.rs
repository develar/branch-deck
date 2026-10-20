//! A file readable by the user only (0600) in the app data directory: the tokens of the app's own sign-ins
//! (`github-token.json`, `youtrack-token.json`). Not the keychain: the release is not signed by Apple, so macOS would
//! ask for the keychain item again after every update, and `bd` in the background could not answer.

use serde::Serialize;
use serde::de::DeserializeOwned;
use std::io::Write;
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

/// Writes the file: a temporary file created 0600 in the same directory, then renamed over the old one
pub fn write_private(path: &Path, content: &[u8]) -> std::io::Result<()> {
  let dir = path.parent().ok_or_else(|| std::io::Error::other("the file has no directory"))?;
  let name = path.file_name().ok_or_else(|| std::io::Error::other("the file has no name"))?;
  std::fs::create_dir_all(dir)?;
  let temporary = dir.join(format!(".{}.{}", name.to_string_lossy(), std::process::id()));
  let mut options = std::fs::OpenOptions::new();
  options.write(true).create(true).truncate(true);
  #[cfg(unix)]
  {
    use std::os::unix::fs::OpenOptionsExt;
    options.mode(0o600);
  }
  let written = options.open(&temporary).and_then(|mut file| {
    file.write_all(content)?;
    file.sync_all()
  });
  if let Err(e) = written.and_then(|()| std::fs::rename(&temporary, path)) {
    let _ = std::fs::remove_file(&temporary);
    return Err(e);
  }
  Ok(())
}

/// Removes the file; none is fine
pub fn remove_private(path: &Path) -> std::io::Result<()> {
  match std::fs::remove_file(path) {
    Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
    _ => Ok(()),
  }
}

/// The JSON of the file (`read_private`); None also for a file that does not parse, with a warning
pub fn read_private_json<T: DeserializeOwned>(path: &Path, what: &str) -> Option<T> {
  let text = read_private(path, what)?;
  match serde_json::from_str(&text) {
    Ok(value) => Some(value),
    Err(e) => {
      warn!(path = %path.display(), error = %e, "{what} is not valid");
      None
    }
  }
}

/// Writes `value` as pretty JSON (`write_private`)
pub fn write_private_json<T: Serialize>(path: &Path, value: &T) -> std::io::Result<()> {
  write_private(path, serde_json::to_string_pretty(value).map_err(std::io::Error::other)?.as_bytes())
}
