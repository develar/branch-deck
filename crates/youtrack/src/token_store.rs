//! The token of the app's own YouTrack sign-in: a JSON file in the app data directory, readable by the user only
//! (`sync_utils::private_file`), next to `github-token.json`.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use sync_utils::private_file::read_private_json;

pub const TOKEN_FILE_NAME: &str = "youtrack-token.json";

pub fn token_file(app_data_dir: &Path) -> PathBuf {
  app_data_dir.join(TOKEN_FILE_NAME)
}

/// The refresh grant of a browser sign-in: a new access token from the Hub, without the user
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefreshGrant {
  pub hub_url: String,
  pub service_id: String,
  pub refresh_token: String,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredToken {
  /// The YouTrack the token is for: it is sent nowhere else
  pub url: String,
  pub login: String,
  pub full_name: String,
  pub access_token: String,
  /// Unix seconds; None for a pasted token, which does not expire
  #[serde(default)]
  pub access_expires_at: Option<u32>,
  /// None for a pasted token
  #[serde(default)]
  pub refresh: Option<RefreshGrant>,
  /// Unix seconds
  #[serde(default)]
  pub created_at: u32,
}

impl std::fmt::Debug for StoredToken {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    // neither token
    write!(f, "StoredToken {{ url: {}, login: {}, renewable: {} }}", self.url, self.login, self.refresh.is_some())
  }
}

/// The stored token; None without one, and for a file others may read
pub fn read_token(path: &Path) -> Option<StoredToken> {
  read_private_json(path, "the YouTrack token file")
}
