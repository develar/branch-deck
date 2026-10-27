//! The journal of the operations of the remote of a repository (docs/design/git-operations.md, "Journal";
//! docs/design/rust-domain-authority.md, "Наблюдаемость"): `$GIT_COMMON_DIR/branch-deck/journal.jsonl`, one line of JSON
//! per operation that ended (`JournalEntry`), of the app and of every `bd` alike, and one per start whose answer was
//! lost once a read of Patronus settled it or the user forgot it (`JournalTransition`). This module is its one writer:
//!
//! - an entry is written by the operations registry (`registry_file`) under `operations.lock`, in the change that takes
//!   the record of the operation out of `operations.json`: the end of an operation (`Entry::finish`, the app's
//!   `RunningGuard::finish` and the run of `bd`; a run dropped without an answer as `internal`), a settle and a forget;
//! - each write keeps the newest `LIMIT` entries of the last `MAX_AGE` seconds (by `endedAt`) and replaces the file by a
//!   rename, so a read without the lock sees whole lines; a line this version can't read stays while it is new enough,
//!   and the reader skips it;
//! - an entry holds no token and no secret: the intent, the effects with their statuses, what they answered, the error,
//!   what it cost (`OperationStats`: its git processes by subcommand, its HTTP requests and its time from its admission,
//!   `sync_utils::operation_tally`), and whether its worktree is a slot of the lease pool (an agent's).

use crate::registry_file::{ProcessRegistry, Record};
use domain_types::error::ApiError;
use domain_types::operation::{JournalEntry, JournalTransition, OperationResult, OperationStats, Outcome};
use lease_pool::lease::{Pool, canonical_path};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use sync_utils::atomic_file::{AtomicWrite, write_atomically};
use sync_utils::operation_tally::OperationTally;
use sync_utils::timestamp::unix_now;

const FILE: &str = "journal.jsonl";
/// How many entries the journal keeps
pub const LIMIT: usize = 200;
/// How old, in seconds, the oldest entry the journal keeps is: 30 days
pub const MAX_AGE: u32 = 30 * 24 * 60 * 60;

/// The journal file in the directory of the registry (`$GIT_COMMON_DIR/branch-deck`)
fn path(dir: &Path) -> PathBuf {
  dir.join(FILE)
}

/// The entries of the journal of the repository of `git_common_dir`, oldest first, read without the lock (the file is
/// replaced by a rename); a line this version can't read is skipped, and a missing file is an empty journal
///
/// # Errors
/// The file can't be read
pub fn read(git_common_dir: &Path) -> std::io::Result<Vec<JournalEntry>> {
  let text = match std::fs::read_to_string(path(ProcessRegistry::of(git_common_dir).dir())) {
    Ok(text) => text,
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
    Err(error) => return Err(error),
  };
  Ok(text.lines().filter_map(|line| serde_json::from_str(line).ok()).collect())
}

/// Appends `entry` to the journal in `dir`, keeping what `kept` keeps at `now`; the caller holds `operations.lock`
pub(crate) fn append_locked(dir: &Path, entry: &JournalEntry, now: u32) -> std::io::Result<()> {
  let path = path(dir);
  let text = match std::fs::read_to_string(&path) {
    Ok(text) => text,
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
    Err(error) => return Err(error),
  };
  let line = serde_json::to_string(entry).map_err(std::io::Error::other)?;
  let lines = kept(text.lines().chain(std::iter::once(line.as_str())), now);
  let mut bytes = lines.join("\n").into_bytes();
  bytes.push(b'\n');
  write_atomically(&path, &bytes, AtomicWrite::default())
}

/// The part of a line the rules of the journal read
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Dated {
  ended_at: u32,
}

/// The lines of a journal it keeps at `now`, in their order: the newest `LIMIT` of the ones that ended within `MAX_AGE`;
/// a line without its end is no entry
pub(crate) fn kept<'a>(lines: impl Iterator<Item = &'a str>, now: u32) -> Vec<&'a str> {
  let recent: Vec<&str> = lines
    .filter(|line| serde_json::from_str::<Dated>(line).is_ok_and(|dated| now.saturating_sub(dated.ended_at) <= MAX_AGE))
    .collect();
  recent[recent.len().saturating_sub(LIMIT)..].to_vec()
}

/// What an operation cost so far
#[must_use]
pub fn stats_of(tally: &OperationTally) -> OperationStats {
  OperationStats {
    git_processes: tally.git_processes(),
    http_requests: tally.http_requests(),
    millis: tally.millis(),
  }
}

/// The entry of the operation of `record` in `registry` that ended as `ended` and cost `stats`
pub(crate) fn ended(registry: &ProcessRegistry, record: &Record, ended: Result<&Outcome, &ApiError>, stats: OperationStats) -> JournalEntry {
  let (effects, result, error) = match ended {
    Ok(outcome) => (outcome.effects.clone(), outcome.result.clone(), None),
    Err(error) => (Vec::new(), OperationResult::None, Some(error.clone())),
  };
  JournalEntry {
    effects,
    result,
    error,
    stats,
    ..entry_of(registry, record, None)
  }
}

/// The entry of what became of the start of `record` whose answer was lost, now
pub(crate) fn transition(registry: &ProcessRegistry, record: &Record, transition: JournalTransition) -> JournalEntry {
  entry_of(registry, record, Some(transition))
}

fn entry_of(registry: &ProcessRegistry, record: &Record, transition: Option<JournalTransition>) -> JournalEntry {
  JournalEntry {
    id: record.id.clone(),
    tool: record.tool,
    pid: record.pid,
    worktree: record.session.worktree.clone(),
    prefix: record.session.prefix.clone(),
    lease_slot: is_lease_slot(registry, &record.session.worktree),
    intent: record.intent.clone(),
    subject: record.intent.subject(),
    started_at: record.started_at,
    ended_at: unix_now(),
    effects: Vec::new(),
    result: OperationResult::None,
    error: None,
    stats: OperationStats::default(),
    transition,
  }
}

/// Whether `worktree` is a slot of the lease pool of the repository of `registry`, by its state (`leases.json`, read
/// without its lock); a state that can't be read tells no
fn is_lease_slot(registry: &ProcessRegistry, worktree: &str) -> bool {
  let Some(common_dir) = registry.dir().parent() else {
    return false;
  };
  let top_level = canonical_path(Path::new(worktree));
  Pool::new(common_dir).read().is_ok_and(|store| store.slots.iter().any(|slot| slot.path == top_level))
}

#[cfg(test)]
#[path = "journal_test.rs"]
pub(crate) mod journal_test;
