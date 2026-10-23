//! The answers of the CLIs: `--version`, `auth status --json` of each, the `version:` of a skill and `teamcity skill
//! list`. A field the app doesn't know is ignored; an answer it can't read is an error with the CLI's words.

use domain_types::cli_tools::{CliToolServer, CliToolSignInState};
use serde::Deserialize;
use std::cmp::Ordering;

/// The version in `teamcity version 1.6.0`, `youtrack version v0.7.0` or `0.7.0`: the first word that starts with a
/// digit (after a `v`) and has a dot
pub fn parse_version(stdout: &str) -> Option<String> {
  stdout
    .split_whitespace()
    .map(|word| word.strip_prefix('v').unwrap_or(word).trim_end_matches([',', ')']))
    .find(|word| word.starts_with(|c: char| c.is_ascii_digit()) && word.contains('.'))
    .map(str::to_string)
}

/// Orders two versions by their numeric parts (`1.10.0` after `1.9.2`); a part that is not a number orders as 0, so a
/// pre-release `1.7.0-rc1` equals `1.7.0`
pub fn compare_versions(a: &str, b: &str) -> Ordering {
  let parts = |version: &str| -> Vec<u64> { version.split(['.', '-', '+']).take(3).map(|part| part.parse().unwrap_or(0)).collect() };
  let (a, b) = (parts(a), parts(b));
  (0..3)
    .map(|i| a.get(i).unwrap_or(&0).cmp(b.get(i).unwrap_or(&0)))
    .find(|order| order.is_ne())
    .unwrap_or(Ordering::Equal)
}

/// `teamcity auth status --json`: one entry per server
#[derive(Deserialize)]
struct TeamcityStatusDto {
  #[serde(default)]
  server: String,
  #[serde(default)]
  status: String,
  #[serde(default)]
  user: Option<TeamcityUserDto>,
  #[serde(default)]
  token_expiry: Option<String>,
  #[serde(default)]
  error: Option<String>,
}

#[derive(Deserialize)]
struct TeamcityUserDto {
  #[serde(default)]
  username: Option<String>,
}

/// The servers of `teamcity auth status --json`: an array, or one object; the entry of no server (not logged in to any)
/// is no server
pub fn parse_teamcity_auth(stdout: &str) -> Result<Vec<CliToolServer>, String> {
  let entries = one_or_many::<TeamcityStatusDto>(stdout).map_err(|e| format!("teamcity auth status answered what Branch Deck can't read: {e}"))?;
  Ok(
    entries
      .into_iter()
      .filter(|entry| !entry.server.is_empty())
      .map(|entry| {
        let state = match entry.status.as_str() {
          "authenticated" => CliToolSignInState::Authenticated,
          "guest" => CliToolSignInState::Guest,
          _ => CliToolSignInState::Failed,
        };
        CliToolServer {
          url: entry.server.trim_end_matches('/').to_string(),
          user: entry.user.and_then(|user| user.username),
          error: (state == CliToolSignInState::Failed).then(|| entry.error.unwrap_or(entry.status)),
          expires_at: entry.token_expiry.as_deref().and_then(parse_timestamp),
          state,
        }
      })
      .collect(),
  )
}

/// `youtrack auth status --json`: the active instance as an object, or every instance as an array when several are saved
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct YoutrackStatusDto {
  #[serde(default)]
  url: String,
  #[serde(default)]
  login: Option<String>,
  #[serde(default)]
  status: String,
}

/// The instances of `youtrack auth status --json`; "not configured" is no instance. It always exits with 0
pub fn parse_youtrack_auth(stdout: &str) -> Result<Vec<CliToolServer>, String> {
  let entries = one_or_many::<YoutrackStatusDto>(stdout).map_err(|e| format!("youtrack auth status answered what Branch Deck can't read: {e}"))?;
  Ok(
    entries
      .into_iter()
      .filter(|entry| !entry.url.is_empty() && entry.status != "not configured")
      .map(|entry| {
        let authenticated = entry.status == "authenticated";
        CliToolServer {
          url: entry.url.trim_end_matches('/').to_string(),
          user: entry.login.filter(|login| !login.is_empty()),
          state: if authenticated { CliToolSignInState::Authenticated } else { CliToolSignInState::Failed },
          error: (!authenticated).then_some(entry.status),
          // a permanent token has no expiry
          expires_at: None,
        }
      })
      .collect(),
  )
}

fn one_or_many<T: for<'de> Deserialize<'de>>(stdout: &str) -> Result<Vec<T>, serde_json::Error> {
  serde_json::from_str::<Vec<T>>(stdout).or_else(|_| serde_json::from_str::<T>(stdout).map(|entry| vec![entry]))
}

/// Unix seconds of an RFC 3339 time (`2026-11-21T16:58:19.041Z`)
fn parse_timestamp(text: &str) -> Option<u32> {
  let timestamp: jiff::Timestamp = text.parse().ok()?;
  u32::try_from(timestamp.as_second()).ok()
}

/// The `version:` of the front matter of a `SKILL.md`
pub fn parse_skill_version(skill_md: &str) -> Option<String> {
  let front_matter = skill_md.strip_prefix("---")?.split("\n---").next()?;
  front_matter
    .lines()
    .find_map(|line| line.trim().strip_prefix("version:"))
    .map(|version| version.trim().trim_matches(['"', '\'']).to_string())
    .filter(|version| !version.is_empty())
}

/// The version of `name` in `teamcity skill list`: a table of name, version and description
pub fn parse_skill_list(stdout: &str, name: &str) -> Option<String> {
  stdout.lines().find_map(|line| {
    let mut words = line.split_whitespace();
    (words.next() == Some(name)).then(|| words.next()).flatten().map(str::to_string)
  })
}
