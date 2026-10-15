//! The Space token Patronus accepts: the one the Safe Push CLI (`./safePush.cmd`) keeps, else `$SPACE_TOKEN`.
//! Branch Deck never logs in itself; the CLI renews its 14-day token when it runs.

use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use sync_utils::timestamp::parse_rfc3339_timestamp;
use tracing::warn;

/// Relative to the home directory; written by the Safe Push CLI after its Space login
pub const TOKEN_FILE: &str = ".patronus/code.jetbrains.team-identity-token.json";

pub const TOKEN_ENV: &str = "SPACE_TOKEN";

/// A token and where it came from
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Token {
  pub value: String,
  pub stamp: TokenStamp,
}

/// Identifies one version of the token: a rejected token is not sent again until the stamp changes
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TokenStamp {
  /// The modification time of the token file
  File(Option<SystemTime>),
  Env,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TokenLookup {
  Found(Token),
  /// Neither the file nor the variable
  Missing,
  /// The file says the token expired; the stamp tells when the CLI renews it
  Expired(TokenStamp),
}

/// `~/.patronus/code.jetbrains.team-identity-token.json` = `{"name", "token", "expires"}`
#[derive(Deserialize)]
struct TokenFile {
  token: String,
  expires: Option<String>,
}

pub fn token_file_path(home: &Path) -> PathBuf {
  home.join(TOKEN_FILE)
}

/// The token of the Safe Push CLI in `home`, else `env_token`. `now` is Unix seconds: an expired token is never sent.
pub fn find_token(home: Option<&Path>, env_token: Option<String>, now: u32) -> TokenLookup {
  if let Some(home) = home {
    let path = token_file_path(home);
    if let Ok(content) = std::fs::read(&path) {
      let stamp = TokenStamp::File(std::fs::metadata(&path).and_then(|metadata| metadata.modified()).ok());
      match serde_json::from_slice::<TokenFile>(&content) {
        Ok(file) if !file.token.is_empty() => {
          let expired = file.expires.as_deref().and_then(parse_rfc3339_timestamp).is_some_and(|expires| expires <= now);
          if expired {
            return TokenLookup::Expired(stamp);
          }
          return TokenLookup::Found(Token { value: file.token, stamp });
        }
        Ok(_) => {}
        Err(e) => warn!(path = %path.display(), error = %e, "unreadable Safe Push CLI token file"),
      }
    }
  }
  match env_token.filter(|token| !token.trim().is_empty()) {
    Some(value) => TokenLookup::Found(Token {
      value: value.trim().to_string(),
      stamp: TokenStamp::Env,
    }),
    None => TokenLookup::Missing,
  }
}

/// `find_token` for the current user
pub fn find_user_token(now: u32) -> TokenLookup {
  let home = std::env::home_dir();
  find_token(home.as_deref(), std::env::var(TOKEN_ENV).ok(), now)
}
