//! The token of the app's own sign-in (`device_flow.rs`): a JSON file in the app data directory, readable by the user
//! only (0600), or the keychain on macOS (`sync_utils::secret_store`), shared by the app and `bd`.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use sync_utils::secret_store::{read_secret_json, remove_secret, write_secret_json};

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
  read_secret_json(path, "the GitHub token file")
}

/// Keeps the token: in the keychain on macOS, else a 0600 file (`sync_utils::secret_store`)
pub fn write_token(path: &Path, token: &StoredToken) -> std::io::Result<()> {
  write_secret_json(path, token)
}

/// Removes the token; none is fine
pub fn remove_token(path: &Path) -> std::io::Result<()> {
  remove_secret(path)
}
