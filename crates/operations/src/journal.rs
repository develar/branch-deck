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
//! Activity reads both (`read_activity`): the journals of the recent projects in parallel on the blocking pool, one
//! read of each journal at a time; a read of Activity asked while one runs waits for the next, which begins once that
//! one answered, as the one that runs may have read the file before the change the read was asked for (`JournalReads`).
//! A read waits for each within `ACTIVITY_READ_TIMEOUT`, then shows what that journal answered last: a journal of a
//! removed disk, or of one that never answered, is listed unavailable; a read that answers later with other entries
//! tells the windows (`late`), so a slow disk shows once it answered and the wall clock never decides what shows. Clear
//! Log forgets the log of a repository in both (`clear`).

use crate::registry_file::{ProcessRegistry, Record};
use domain_types::activity::{ActivityLog, PageActivity, PageEntry, RepositoryJournal};
use domain_types::error::ApiError;
use domain_types::operation::{Intent, JournalEntry, JournalTransition, OperationResult, OperationStats, Outcome, Resource};
use domain_types::review_request::ReviewRange;
use history_rewrite::repository_lock::lock_file;
use lease_pool::lease::{Pool, canonical_path};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex, MutexGuard, PoisonError};
use std::time::Duration;
use sync_utils::atomic_file::{AtomicWrite, write_atomically};
use sync_utils::operation_id::OperationId as Ulid;
use sync_utils::operation_tally::OperationTally;
use sync_utils::span::in_current_span;
use sync_utils::timestamp::unix_now;
use tokio::sync::watch;
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
/// How long Activity waits for the journal of a repository before it shows what the journal answered last (none yet:
/// unavailable); a read that answers later with other entries tells the windows
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

/// The journal of the repository of `git_common_dir`, as `subscribe_view` remembers it for the projects of the
/// repository
#[must_use]
pub fn journal_path(git_common_dir: &Path) -> PathBuf {
  path(ProcessRegistry::of(git_common_dir).dir())
}

/// The entries of the journal of a repository at `path` for Activity: one the repository has not written yet is empty,
/// but the journal of a repository that is gone (its `$GIT_COMMON_DIR` is not there: a removed disk, a deleted
/// repository) can't be read
fn read_journal(path: &Path) -> std::io::Result<Vec<JournalEntry>> {
  let entries = read_lines(path)?;
  if entries.is_empty() && !path.parent().and_then(Path::parent).is_some_and(Path::is_dir) {
    return Err(std::io::Error::new(std::io::ErrorKind::NotFound, "the repository of the journal is gone"));
  }
  Ok(entries)
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

/// What the journal keeps of what an operation answered: a review range without its commits, which git has
fn journal_result(result: &OperationResult) -> OperationResult {
  match result {
    OperationResult::ReviewRange(range) => OperationResult::ReviewRange(Box::new(ReviewRange {
      commits: Vec::new(),
      ..(**range).clone()
    })),
    result => result.clone(),
  }
}

/// The entry of the operation of `record` in `registry` that ended as `ended` and cost `stats`
pub(crate) fn ended(registry: &ProcessRegistry, record: &Record, ended: Result<&Outcome, &ApiError>, stats: OperationStats) -> JournalEntry {
  let (effects, result, error) = match ended {
    Ok(outcome) => (outcome.effects.clone(), journal_result(&outcome.result), None),
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
    into: pushed_into(record),
    started_at: record.started_at,
    ended_at: unix_now(),
    effects: Vec::new(),
    result: OperationResult::None,
    error: None,
    stats: OperationStats::default(),
    transition,
  }
}

/// The target the start of `record` pushes the copy into: the branch of the remote-tracking ref it holds
/// (`refs/remotes/origin/master`, `engine::reserved`), "master"; `None` for a start of a robot and any other operation
fn pushed_into(record: &Record) -> Option<String> {
  if !matches!(record.intent, Intent::Land(_)) {
    return None;
  }
  record.resources.iter().find_map(|resource| match resource {
    Resource::Ref { name } => name.strip_prefix("refs/remotes/")?.split_once('/').map(|(_, target)| target.to_owned()),
    _ => None,
  })
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
  #[serde(default)]
  operation_id: Option<String>,
}

/// The part of a line of a journal that `clear` reads: the id of its operation
#[derive(Deserialize)]
struct OperationOf {
  id: String,
}

/// Forgets the log of the recent project `repository` (Clear Log): its repository's journal at `journal` empties under
/// `operations.lock`, and the entries of the page of `repository` leave the journal of the page in `app_data`, with the
/// errors the page told of the operations of that journal in another repository (an entry of the page that names an
/// operation shows where the operation ran)
///
/// # Errors
/// `locked` while another process keeps a lock, an error of a file that can't be written
pub fn clear(journal: Option<&Path>, app_data: &Path, repository: &str) -> Result<(), ApiError> {
  let mut operations = HashSet::new();
  if let Some(common_dir) = journal.and_then(Path::parent).and_then(Path::parent) {
    ProcessRegistry::of(common_dir).locked(|dir| {
      rewrite(&path(dir), |text| {
        operations.extend(text.lines().filter_map(|line| serde_json::from_str::<OperationOf>(line).ok()).map(|entry| entry.id));
        String::new()
      })
    })?;
  }
  page_locked(app_data, |path| {
    rewrite(path, |text| {
      let kept = |line: &&str| {
        serde_json::from_str::<OfRepository>(line)
          .is_ok_and(|entry| entry.activity.repository_path != repository && entry.activity.operation_id.is_none_or(|op| !operations.contains(&op)))
      };
      text.lines().filter(kept).collect::<Vec<_>>().join("\n")
    })
  })
}

/// What hears that a journal answered after Activity stopped waiting for it, with entries other than it showed: the
/// app tells its windows `activity-changed`
pub type LateListener = Arc<dyn Fn() + Send + Sync>;

/// The answer of a read of a journal: its entries, or why it can't be read
type Answer = Result<Vec<JournalEntry>, String>;

/// A read of a journal: its answer, once it came
type Reading = watch::Receiver<Option<Answer>>;

/// The reads of a journal in this process: one runs at a time, so a disk that hangs keeps one blocked thread, never one
/// per read. A read of Activity asked while one runs never takes its answer, which may be of the file before the change
/// it was asked for (`activity-changed`): it waits for the next read, which the same thread runs once the one that runs
/// answered, and which the reads of Activity asked meanwhile share
#[derive(Default)]
struct JournalReads {
  /// The read that runs
  running: Option<Reading>,
  /// The read that runs next, once one was asked while one runs
  next: Option<(watch::Sender<Option<Answer>>, Reading)>,
  /// The entries of the last read that answered; none after one that failed
  last: Option<Vec<JournalEntry>>,
  /// Set once a read of Activity stopped waiting for its read: told by the first answer that differs from `last`
  late: Option<LateListener>,
}

/// The reads of the journals of this process, by path
fn journal_reads() -> MutexGuard<'static, HashMap<PathBuf, JournalReads>> {
  static READS: LazyLock<Mutex<HashMap<PathBuf, JournalReads>>> = LazyLock::new(Mutex::default);
  READS.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The answer of a read of the journal at `path` that begins after this call: one started now on the blocking pool, or,
/// while one runs, the next one
fn begin_read(path: &Path) -> Reading {
  let mut reads = journal_reads();
  let state = reads.entry(path.to_path_buf()).or_default();
  // a read whose thread is gone without an answer (a panic) is no read that runs
  if state.running.as_ref().is_some_and(|running| running.has_changed().is_ok()) {
    return state.next.get_or_insert_with(|| watch::channel(None)).1.clone();
  }
  let (sender, receiver) = state.next.take().unwrap_or_else(|| watch::channel(None));
  state.running = Some(receiver.clone());
  let path = path.to_path_buf();
  drop(tokio::task::spawn_blocking(in_current_span(move || read_in_turn(&path, sender))));
  receiver
}

/// Reads the journal at `path` for `sender`, then the next read asked meanwhile, until none is asked
fn read_in_turn(path: &Path, sender: watch::Sender<Option<Answer>>) {
  let mut sender = sender;
  loop {
    let answer = read_journal(path).map_err(|error| error.to_string());
    let (late, next) = {
      let mut reads = journal_reads();
      let state = reads.entry(path.to_path_buf()).or_default();
      let changed = answer.as_ref().ok() != state.last.as_ref();
      state.last = answer.as_ref().ok().cloned();
      let late = if changed { state.late.take() } else { None };
      // under the lock: a read of Activity that stops waiting sees either this answer or `late` taken after it
      let _ = sender.send(Some(answer));
      let next = state.next.take();
      state.running = next.as_ref().map(|(_, receiver)| receiver.clone());
      (late, next)
    };
    if let Some(late) = late {
      late();
    }
    let Some((next, _)) = next else {
      return;
    };
    sender = next;
  }
}

/// The entries of the journal at `path` as Activity shows them: the answer of its read (`begin_read`) by `deadline`, else
/// what it answered last, with `late` told once its read answers other entries; none when it can't be read, or never
/// answered
async fn shown(path: &Path, mut answer: Reading, deadline: tokio::time::Instant, late: &LateListener) -> Option<Vec<JournalEntry>> {
  if let Ok(waited) = tokio::time::timeout_at(deadline, answer.wait_for(Option::is_some)).await {
    return match waited.as_deref() {
      Ok(Some(Ok(entries))) => Some(entries.clone()),
      Ok(Some(Err(error))) => {
        debug!(journal = %path.display(), "cannot read the journal: {error}");
        None
      }
      Ok(None) | Err(_) => {
        debug!(journal = %path.display(), "the read of the journal ended without an answer");
        None
      }
    };
  }
  let mut reads = journal_reads();
  // it may have answered since the wait ended
  if let Some(answered) = answer.borrow().as_ref() {
    return answered.as_ref().ok().cloned();
  }
  let state = reads.entry(path.to_path_buf()).or_default();
  state.late = Some(Arc::clone(late));
  debug!(journal = %path.display(), "the journal did not answer within the bound: its last answer shows until it does");
  state.last.clone()
}

/// What Activity shows (`read_activity`): the journals of `projects` (each recent project with the journal
/// `subscribe_view` remembered for it; one never opened since has none), each read once for the projects that share
/// it, in parallel on the blocking pool, each waited for within `timeout`, then shown as it answered last (`shown`;
/// one that can't be read, whose repository is gone, or that never answered is unavailable, and `late` hears when one
/// answers other entries later), and the journal of the page in `app_data`
pub async fn read_activity(projects: &[(String, Option<String>)], app_data: &Path, timeout: Duration, late: &LateListener) -> ActivityLog {
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
  let app_data = app_data.to_path_buf();
  let page = tokio::task::spawn_blocking(in_current_span(move || read_page(&app_data)));
  let deadline = tokio::time::Instant::now() + timeout;
  // every read runs before the first wait
  let reads: Vec<_> = journals.iter().map(|journal| begin_read(Path::new(&journal.path))).collect();
  for (journal, answer) in journals.iter_mut().zip(reads) {
    if let Some(entries) = shown(Path::new(&journal.path), answer, deadline, late).await {
      journal.available = true;
      journal.entries = entries;
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
