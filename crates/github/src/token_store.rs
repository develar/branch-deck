//! The token of the app's own sign-in (`device_flow.rs`): a JSON file in the app data directory, readable by the user
//! only (0600), shared by the app and `bd`. Not the keychain: the release is not signed by Apple, so macOS would ask
//! for the keychain item again after every update, and `bd` in the background could not answer.

use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};
use tracing::warn;

pub const TOKEN_FILE_NAME: &str = "github-token.json";

/// The token file in the app data directory (`~/Library/Application Support/<identifier>` on macOS)
pub fn token_file(app_data_dir: &Path) -> PathBuf {
  app_data_dir.join(TOKEN_FILE_NAME)
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredToken {
  pub token: String,
  /// The GitHub login the token belongs to
  pub login: String,
  /// The scopes GitHub granted, e.g. `read:org,repo`
  #[serde(default)]
  pub scopes: String,
  /// Unix seconds
  #[serde(default)]
  pub created_at: u32,
}

impl std::fmt::Debug for StoredToken {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("StoredToken")
      .field("login", &self.login)
      .field("scopes", &self.scopes)
      .finish_non_exhaustive()
  }
}

/// The stored token; None without one, and for a file others may read (it is not used, a warning says why)
pub fn read_token(path: &Path) -> Option<StoredToken> {
  let text = match std::fs::read_to_string(path) {
    Ok(text) => text,
    Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
    Err(e) => {
      warn!(path = %path.display(), error = %e, "cannot read the GitHub token file");
      return None;
    }
  };
  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    let mode = std::fs::metadata(path).map_or(0, |metadata| metadata.permissions().mode());
    if mode & 0o077 != 0 {
      warn!(path = %path.display(), mode = format!("{mode:o}"), "the GitHub token file is readable by others: not used");
      return None;
    }
  }
  match serde_json::from_str(&text) {
    Ok(token) => Some(token),
    Err(e) => {
      warn!(path = %path.display(), error = %e, "the GitHub token file is not valid");
      None
    }
  }
}

/// Writes the token: a temporary file created 0600 in the same directory, then renamed over the old one
pub fn write_token(path: &Path, token: &StoredToken) -> std::io::Result<()> {
  let dir = path.parent().ok_or_else(|| std::io::Error::other("the token file has no directory"))?;
  std::fs::create_dir_all(dir)?;
  let temporary = dir.join(format!(".{TOKEN_FILE_NAME}.{}", std::process::id()));
  let mut options = std::fs::OpenOptions::new();
  options.write(true).create(true).truncate(true);
  #[cfg(unix)]
  {
    use std::os::unix::fs::OpenOptionsExt;
    options.mode(0o600);
  }
  let written = options.open(&temporary).and_then(|mut file| {
    file.write_all(serde_json::to_string_pretty(token).map_err(std::io::Error::other)?.as_bytes())?;
    file.sync_all()
  });
  if let Err(e) = written.and_then(|()| std::fs::rename(&temporary, path)) {
    let _ = std::fs::remove_file(&temporary);
    return Err(e);
  }
  Ok(())
}

/// Removes the token; none is fine
pub fn remove_token(path: &Path) -> std::io::Result<()> {
  match std::fs::remove_file(path) {
    Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
    _ => Ok(()),
  }
}
