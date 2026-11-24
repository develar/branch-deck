//! The kill of the backend without a window by the system (openspec/specs/repository/ide-embedding/spec.md, "Backend:
//! The system's stop of the app without a window is remembered"; docs/design/ide-embedding.md, "The system stops the
//! app in the background"). On macOS, Background Task Management quits an app that runs with no window about 30 s
//! after it went into the background when Login Items & Extensions → Allow in the Background is off for it, a bridge
//! connected or not. The app records such an end in its data directory (`backend-killed.json`, `record`), written only
//! by the app; the bridge reads it before it starts the app without a window (`starts_app`): a kill of its build within
//! `MEMORY` serves the IDE with its own backend instead, so the IDE does not start the bridge again and again. The first
//! bridge that tells the user about a record claims it (`claim_notice`, `backend-killed.told`, written only by bridges).

use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Duration;
use sync_utils::atomic_file::{AtomicWrite, replace_if_unchanged, write_atomically};

/// The record of the last kill, in the data directory of the app
pub const RECORD_FILE: &str = "backend-killed.json";

/// The `at_ms` of the record a bridge told the user about, beside the record
pub const TOLD_FILE: &str = "backend-killed.told";

/// How long a kill keeps the bridges from starting the app without a window: a day, after which a bridge tries again
pub const MEMORY: Duration = Duration::from_hours(24);

/// A kill of the app by the system while it had no window
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackgroundKill {
  /// When, in milliseconds since the epoch
  pub at_ms: u64,
  /// The build of the app (`identity::build_id`)
  pub build: String,
}

/// Records `kill` in `data_dir`
///
/// # Errors
/// The file cannot be written
pub fn record(data_dir: &Path, kill: &BackgroundKill) -> std::io::Result<()> {
  let text = serde_json::to_vec(kill).map_err(std::io::Error::other)?;
  write_atomically(&data_dir.join(RECORD_FILE), &text, AtomicWrite { private: false, durable: true })
}

/// The last kill recorded in `data_dir`; none without a readable record
pub fn read(data_dir: &Path) -> Option<BackgroundKill> {
  serde_json::from_slice(&std::fs::read(data_dir.join(RECORD_FILE)).ok()?).ok()
}

/// Whether a bridge of `build` starts the app without a window at `now_ms` after `kill`: not within `MEMORY` of a kill
/// of the same build (another build may be allowed in the background, and is tried)
pub fn starts_app(kill: Option<&BackgroundKill>, now_ms: u64, build: &str) -> bool {
  let memory = u64::try_from(MEMORY.as_millis()).unwrap_or(u64::MAX);
  !kill.is_some_and(|kill| kill.build == build && now_ms.saturating_sub(kill.at_ms) < memory)
}

/// Claims the telling of `kill` for this bridge: true for the first claim of that record of `data_dir`, false for
/// every later one, of this process or another; a claim that cannot be written is not told
pub fn claim_notice(data_dir: &Path, kill: &BackgroundKill) -> bool {
  let path = data_dir.join(TOLD_FILE);
  let told = std::fs::read(&path).ok();
  let at = kill.at_ms.to_string();
  if told.as_deref() == Some(at.as_bytes()) {
    return false;
  }
  replace_if_unchanged(&path, at.as_bytes(), told.as_deref(), AtomicWrite::default()).unwrap_or(false)
}

/// The time now in milliseconds since the epoch
pub fn now_ms() -> u64 {
  std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .map_or(0, |since| u64::try_from(since.as_millis()).unwrap_or(u64::MAX))
}

#[cfg(test)]
#[path = "background_kill_test.rs"]
mod background_kill_test;
