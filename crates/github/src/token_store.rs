//! The token of the app's own sign-in (`device_flow.rs`): a JSON file in the app data directory, readable by the user
//! only (0600, `sync_utils::private_file`), shared by the app and `bd`.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use sync_utils::private_file::{read_private_json, remove_private, write_private_json};

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
  read_private_json(path, "the GitHub token file")
}

/// Writes the token: a temporary file created 0600 in the same directory, then renamed over the old one
pub fn write_token(path: &Path, token: &StoredToken) -> std::io::Result<()> {
  write_private_json(path, token)
}

/// Removes the token; none is fine
pub fn remove_token(path: &Path) -> std::io::Result<()> {
  remove_private(path)
}
