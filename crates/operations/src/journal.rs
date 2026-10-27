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
//!
//! The page has a journal of the same form in the app data directory (`PageEntry`, spec/repository/activity.spec.md):
//! what only the page knows (an error it told, an update, what landed, a rewrite, the news of a review request, an
//! approval or a robot), recorded through `record_page` under `journal.lock` beside it, within the same bounds.
//! Activity reads both (`read_activity`): the journals of the recent projects in parallel on the blocking pool, each
//! within `ACTIVITY_READ_TIMEOUT` (a journal of a removed disk is listed unavailable), and Clear Log forgets the log of
//! a repository in both (`clear`).

use crate::registry_file::{ProcessRegistry, Record};
use domain_types::activity::{ActivityLog, PageActivity, PageEntry, RepositoryJournal};
use domain_types::error::ApiError;
use domain_types::operation::{JournalEntry, JournalTransition, OperationResult, OperationStats, Outcome};
use history_rewrite::repository_lock::lock_file;
use lease_pool::lease::{Pool, canonical_path};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::time::Duration;
use sync_utils::atomic_file::{AtomicWrite, write_atomically};
use sync_utils::operation_id::OperationId as Ulid;
use sync_utils::operation_tally::OperationTally;
use sync_utils::span::in_current_span;
use sync_utils::timestamp::unix_now;
use tracing::debug;

const FILE: &str = "journal.jsonl";
/// How many entries the journal keeps
pub const LIMIT: usize = 200;
/// How old, in seconds, the oldest entry the journal keeps is: 30 days
pub const MAX_AGE: u32 = 30 * 24 * 60 * 60;
/// The lock of the journal of the page, beside it in the app data directory
const PAGE_LOCK: &str = "journal.lock";
/// The bound of the wait for the lock of the journal of the page: a write holds it for a file operation only
const PAGE_LOCK_TIMEOUT: Duration = Duration::from_secs(2);
/// How long Activity waits for the journal of a repository: one of a removed disk, or a disk that does not answer, is
/// listed unavailable after it
pub const ACTIVITY_READ_TIMEOUT: Duration = Duration::from_millis(200);

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
  read_lines(&path(ProcessRegistry::of(git_common_dir).dir()))
}

/// The journal in the directory `dir` of a registry, as `subscribe_view` remembers it for the projects of its repository
pub(crate) fn journal_path_in(dir: &Path) -> PathBuf {
  path(dir)
}

/// Whether `name` is the name of a journal: a watch of the directory of a registry tells Activity of its change
pub(crate) fn is_file_name(name: &std::ffi::OsStr) -> bool {
  name == FILE
}

/// The lines of the file at `path` that `read` parses, oldest first: a missing file is an empty journal, a line this
/// version can't read is skipped
fn read_lines<T: serde::de::DeserializeOwned>(path: &Path) -> std::io::Result<Vec<T>> {
  match std::fs::read_to_string(path) {
    Ok(text) => Ok(text.lines().filter_map(|line| serde_json::from_str(line).ok()).collect()),
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
    Err(error) => Err(error),
  }
}

/// Appends `entry` to the journal in `dir`, keeping what `kept` keeps at `now`; the caller holds `operations.lock`
pub(crate) fn append_locked(dir: &Path, entry: &JournalEntry, now: u32) -> std::io::Result<()> {
  append_line(&path(dir), entry, now)
}

/// Appends the line of `entry` to the journal at `path`, keeping what `kept` keeps at `now`, in one rename; the caller
/// holds the lock of the journal
fn append_line(path: &Path, entry: &impl serde::Serialize, now: u32) -> std::io::Result<()> {
  let line = serde_json::to_string(entry).map_err(std::io::Error::other)?;
  rewrite(path, |text| kept(text.lines().chain(std::iter::once(line.as_str())), now).join("\n"))
}

/// Replaces the journal at `path` with what `change` makes of its text, in one rename; the caller holds its lock
fn rewrite(path: &Path, change: impl FnOnce(&str) -> String) -> std::io::Result<()> {
  let text = match std::fs::read_to_string(path) {
    Ok(text) => text,
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
    Err(error) => return Err(error),
  };
  let mut bytes = change(&text).into_bytes();
  if !bytes.is_empty() {
    bytes.push(b'\n');
  }
  write_atomically(path, &bytes, AtomicWrite::default())
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

/// The journal of the page in the app data directory `app_data`
fn page_path(app_data: &Path) -> PathBuf {
  app_data.join(FILE)
}

/// `run` under the lock of the journal of the page in `app_data`
fn page_locked<R>(app_data: &Path, run: impl FnOnce(&Path) -> std::io::Result<R>) -> Result<R, ApiError> {
  let io = |error: std::io::Error| ApiError::internal(format!("Cannot write the journal of Activity in {}: {error}", app_data.display()));
  std::fs::create_dir_all(app_data).map_err(io)?;
  let _lock = lock_file(app_data.join(PAGE_LOCK), PAGE_LOCK_TIMEOUT, &|| debug!("waiting for the lock of the journal of Activity"))
    .map_err(|error| crate::land_execute::lock_failure(&error))?;
  run(&page_path(app_data)).map_err(io)
}

/// Records `activity` in the journal of the page in `app_data`, now, and answers its entry
///
/// # Errors
/// `locked` while another process keeps the lock of the journal, `internal_error` when it can't be written
pub fn record_page(app_data: &Path, activity: PageActivity) -> Result<PageEntry, ApiError> {
  let now = unix_now();
  let entry = PageEntry {
    id: Ulid::new().to_string(),
    ended_at: now,
    activity,
  };
  page_locked(app_data, |path| append_line(path, &entry, now))?;
  Ok(entry)
}

/// The entries of the journal of the page in `app_data`, oldest first
///
/// # Errors
/// The file can't be read
pub fn read_page(app_data: &Path) -> std::io::Result<Vec<PageEntry>> {
  read_lines(&page_path(app_data))
}

/// The part of a line of the journal of the page that `clear` reads
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OfRepository {
  activity: RepositoryOf,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RepositoryOf {
  repository_path: String,
}

/// Forgets the log of the recent project `repository` (Clear Log): its repository's journal at `journal` empties under
/// `operations.lock`, and the entries of the page of `repository` leave the journal of the page in `app_data`
///
/// # Errors
/// `locked` while another process keeps a lock, an error of a file that can't be written
pub fn clear(journal: Option<&Path>, app_data: &Path, repository: &str) -> Result<(), ApiError> {
  if let Some(common_dir) = journal.and_then(Path::parent).and_then(Path::parent) {
    ProcessRegistry::of(common_dir).locked(|dir| rewrite(&path(dir), |_| String::new()))?;
  }
  page_locked(app_data, |path| {
    rewrite(path, |text| {
      let others = |line: &&str| serde_json::from_str::<OfRepository>(line).is_ok_and(|entry| entry.activity.repository_path != repository);
      text.lines().filter(others).collect::<Vec<_>>().join("\n")
    })
  })
}

/// What Activity shows (`read_activity`): the journals of `projects` (each recent project with the journal
/// `subscribe_view` remembered for it; one never opened since has none), each read once for the projects that share
/// it, in parallel on the blocking pool, each within `timeout` (one that is not read by then is unavailable), and the
/// journal of the page in `app_data`
pub async fn read_activity(projects: &[(String, Option<String>)], app_data: &Path, timeout: Duration) -> ActivityLog {
  let mut journals: Vec<RepositoryJournal> = Vec::new();
  for (repository, journal) in projects {
    let Some(journal) = journal else { continue };
    match journals.iter_mut().find(|known| &known.path == journal) {
      Some(known) => known.repositories.push(repository.clone()),
      None => journals.push(RepositoryJournal {
        path: journal.clone(),
        repositories: vec![repository.clone()],
        available: false,
        entries: Vec::new(),
      }),
    }
  }
  let reads: Vec<_> = journals
    .iter()
    .map(|journal| {
      let path = PathBuf::from(&journal.path);
      tokio::task::spawn_blocking(in_current_span(move || read_lines::<JournalEntry>(&path)))
    })
    .collect();
  let app_data = app_data.to_path_buf();
  let page = tokio::task::spawn_blocking(in_current_span(move || read_page(&app_data)));
  let deadline = tokio::time::Instant::now() + timeout;
  for (journal, read) in journals.iter_mut().zip(reads) {
    match tokio::time::timeout_at(deadline, read).await {
      Ok(Ok(Ok(entries))) => {
        journal.available = true;
        journal.entries = entries;
      }
      Ok(Ok(Err(error))) => debug!(journal = journal.path, "cannot read the journal: {error}"),
      Ok(Err(error)) => debug!(journal = journal.path, "the read of the journal failed: {error}"),
      Err(_) => debug!(journal = journal.path, "the journal did not answer within {timeout:?}"),
    }
  }
  let page = match page.await {
    Ok(Ok(page)) => page,
    Ok(Err(error)) => {
      debug!("cannot read the journal of Activity: {error}");
      Vec::new()
    }
    Err(error) => {
      debug!("the read of the journal of Activity failed: {error}");
      Vec::new()
    }
  };
  ActivityLog { journals, page }
}

#[cfg(test)]
#[path = "journal_test.rs"]
pub(crate) mod journal_test;
