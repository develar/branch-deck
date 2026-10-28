//! A lock file that serializes history rewrites between processes: the app and parallel `bd` calls. The in-process
//! mutexes (ref renames, notes) don't cover another process. Every rewrite of the checked out branch takes it in its
//! core, through the `RewriteOptions` it is called with (AGENTS.md, "Repository lock"): after it read and computed on
//! the oids it read once, for the witness of those refs and its compare-and-swap write only (`lock_prepared`).
//!
//! Before the file lock stands the line of this process (`Place`): a fair queue per lock file, in which an operation
//! waits for the operations of this process that came before it, as long as they take, so two operations of one
//! process never refuse each other. Only the one whose turn it is takes the file lock, which only another process can
//! hold then: a bounded wait (`bd`) and a refusal (`RepositoryLocked`) are for another process alone.

use crate::commit_ids::MIN_ID_LENGTH;
use anyhow::{Context, Result};
use domain_types::error::{HolderTool, LockHolderInfo};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::revs::{checked_out_branch, git_common_dir, head_and_tips, with_dirs_of};
use git_ops::reword_commits::PreparedReword;
use git_ops::rewrite_guard::HeadMoved;
use git_ops::rewrite_plan::PreparedRewrite;
use git_ops::squash_commits::CheckedRestore;
use git_ops::unapply::MoveBlocked;
use git_ops::update_branch::{FilesBlocked, UndoBlocked, UndoCheck, finish_pending_update, pending_mark, pending_update};
use serde::{Deserialize, Serialize};
use std::cell::Cell;
use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::fs::{File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, LazyLock, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use sync_utils::atomic_file::{AtomicWrite, write_atomically};
use sync_utils::process_scan::{is_running, process_started};
use tracing::{debug, instrument, warn};

/// How long an operation waits for another one to finish
pub const DEFAULT_LOCK_TIMEOUT: Duration = Duration::from_secs(10);
const POLL_INTERVAL: Duration = Duration::from_millis(50);
const LOCK_FILE_NAME: &str = "branch-deck.lock";

/// Another process held the lock for the whole timeout (the operations of this process wait for each other in its line)
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(
  "{} holds {} (waited {:.1} s); retry when it is done",
  .holder.as_ref().map_or_else(|| "Another Branch Deck operation".to_string(), ToString::to_string),
  .path.display(),
  .waited.as_secs_f64()
)]
pub struct RepositoryLocked {
  pub path: PathBuf,
  pub waited: Duration,
  /// Who holds it, as the holder wrote it next to the lock (`LockHolder`); `None` when unknown
  pub holder: Option<LockHolder>,
}

/// The wait for a live holder (`lock_repository_while_held`) ended without the lock: the user canceled it
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("The wait for {} was canceled", .path.display())]
pub struct LockWaitCanceled {
  pub path: PathBuf,
}

/// Who holds a lock: written next to the lock file (`<lock>.holder`) once the lock is taken, deleted before it is
/// released. The lock file itself can't carry it: on Windows another process can't read a locked file
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LockHolder {
  /// The command line of the holder, its program by its file name: `bd check --fix`
  pub command: String,
  pub pid: u32,
  /// When the process started (`process_started`): a pid that the system gave to another process tells apart
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub process_started: Option<u64>,
  /// When the lock was taken, in seconds since the epoch
  pub since: u64,
  /// What the holder works on, for a lock whose waiter may take the holder's result instead of doing the work again
  /// (`lock_file_while_held_for`): the tracking ref a fetch of the baseline writes (`fetch.lock`,
  /// `sync_core::fetch_lock`); none for the other locks
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub subject: Option<String>,
}

impl fmt::Display for LockHolder {
  /// "`bd check --fix` (pid 4242, for 12 min 3 s)"
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let held = now_seconds().saturating_sub(self.since);
    let held = if held >= 60 {
      format!("{} min {} s", held / 60, held % 60)
    } else {
      format!("{held} s")
    };
    write!(f, "`{}` (pid {}, for {held})", self.command, self.pid)
  }
}

/// The longest command line a holder record keeps
const MAX_HOLDER_COMMAND: usize = 200;

fn now_seconds() -> u64 {
  SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| since.as_secs())
}

impl LockHolder {
  /// This process, taking a lock now
  fn of_this_process() -> Self {
    let mut args = std::env::args_os();
    let program = args.next().map_or_else(String::new, |program| {
      Path::new(&program)
        .file_name()
        .map_or_else(|| program.to_string_lossy().into_owned(), |name| name.to_string_lossy().into_owned())
    });
    let mut command = std::iter::once(program)
      .chain(args.map(|arg| arg.to_string_lossy().into_owned()))
      .collect::<Vec<_>>()
      .join(" ");
    if command.len() > MAX_HOLDER_COMMAND {
      let cut = (0..=MAX_HOLDER_COMMAND).rev().find(|index| command.is_char_boundary(*index)).unwrap_or(0);
      command.truncate(cut);
      command.push('…');
    }
    let pid = std::process::id();
    Self {
      command,
      pid,
      process_started: process_started(pid),
      since: now_seconds(),
      subject: None,
    }
  }

  /// The holder as the page names it (`LockHolderInfo`): `bd` when its program is `bd` or the app binary runs the CLI
  /// (`branch-deck cli`), else the app; `held_for` in whole seconds at `now` (seconds since the epoch), 0 for a record
  /// ahead of that clock
  #[must_use]
  pub fn info_at(&self, now: u64) -> LockHolderInfo {
    let mut words = self.command.split(' ');
    let program = words.next().unwrap_or_default();
    let is_bd = program.strip_suffix(".exe").unwrap_or(program) == "bd" || words.next() == Some("cli");
    LockHolderInfo {
      tool: if is_bd { HolderTool::Bd } else { HolderTool::App },
      command: self.command.clone(),
      pid: self.pid,
      held_for: u32::try_from(now.saturating_sub(self.since)).unwrap_or(u32::MAX),
    }
  }

  /// `info_at` now: whom a wait for the lock is for (`OperationStep::WaitingForLock`)
  #[must_use]
  pub fn info(&self) -> LockHolderInfo {
    self.info_at(now_seconds())
  }

  /// The holder record of the lock `lock_path`; `None` when there is none, it doesn't parse, or its process is gone
  /// (killed before it deleted the record)
  fn read(lock_path: &Path) -> Option<Self> {
    let text = std::fs::read_to_string(holder_path(lock_path)).ok()?;
    let holder: Self = serde_json::from_str(&text).ok()?;
    is_running(holder.pid, holder.process_started).then_some(holder)
  }

  /// Write the record of the lock `lock_path` through a temporary file and a rename (`write_atomically`), so a reader
  /// never sees half of it
  fn write(&self, lock_path: &Path) -> Result<PathBuf> {
    let path = holder_path(lock_path);
    write_atomically(&path, &serde_json::to_vec(self)?, AtomicWrite::default()).with_context(|| format!("Cannot write {}", path.display()))?;
    Ok(path)
  }
}

/// `<lock>.holder`
fn holder_path(lock_path: &Path) -> PathBuf {
  let mut path = lock_path.as_os_str().to_owned();
  path.push(".holder");
  PathBuf::from(path)
}

/// An exclusive lock on `$GIT_COMMON_DIR/branch-deck.lock`, released when dropped
#[derive(Debug)]
pub struct RepositoryLock {
  _file: File,
  /// The holder record this lock wrote (`LockHolder`), deleted before the lock is released
  holder: Option<PathBuf>,
  /// The turn of the line of this process it was taken in, passed on once the file is unlocked; none for a lock taken
  /// as another process (`lock_as_another_process`)
  _turn: Option<Place>,
}

impl Drop for RepositoryLock {
  /// The record goes first: once the lock is free, the next holder writes its own, which a later delete would take. The
  /// fields go in their order: the file unlocked before the turn passes, so the next of this process takes it at once
  fn drop(&mut self) {
    if let Some(holder) = &self.holder
      && let Err(e) = sync_utils::fs_retry::remove_file(holder)
    {
      warn!(path = %holder.display(), error = %e, "cannot delete the holder record of the lock");
    }
  }
}

/// Lock the repository, after the operations of this process before it in its line, waiting up to `timeout` for another
/// process that holds it. All worktrees of a repository share the lock, as they share their branches. A timeout is a
/// `RepositoryLocked` error (`downcast_ref` it to tell it from others)
pub fn lock_repository(git_executor: &GitCommandExecutor, repository_path: &str, timeout: Duration) -> Result<RepositoryLock> {
  lock_repository_with(git_executor, repository_path, timeout, &|| {})
}

/// `lock_repository`; `on_wait` is called once, when another holds the lock and the wait starts (the update says so)
#[instrument(skip(git_executor, on_wait))]
pub fn lock_repository_with(git_executor: &GitCommandExecutor, repository_path: &str, timeout: Duration, on_wait: &dyn Fn()) -> Result<RepositoryLock> {
  let common_dir = git_common_dir(git_executor, repository_path)?;
  lock_repository_in(Path::new(&common_dir), timeout, on_wait)
}

/// `lock_repository_with` for a caller that knows `$GIT_COMMON_DIR` (absolute, `bd`'s context): no git process
#[instrument(skip(on_wait))]
pub fn lock_repository_in(git_common_dir: &Path, timeout: Duration, on_wait: &dyn Fn()) -> Result<RepositoryLock> {
  lock_file(git_common_dir.join(LOCK_FILE_NAME), timeout, on_wait)
}

/// The lock of the repository at `repository_path` when nobody holds it or waits for it, this process included; `None`
/// at once otherwise: a best-effort write that the next sync tries again (`drop_store_lines`)
pub fn try_lock_repository(git_executor: &GitCommandExecutor, repository_path: &str) -> Result<Option<RepositoryLock>> {
  let common_dir = git_common_dir(git_executor, repository_path)?;
  try_lock_repository_in(Path::new(&common_dir))
}

/// `try_lock_repository` for a caller that knows `$GIT_COMMON_DIR`: no git process
pub fn try_lock_repository_in(git_common_dir: &Path) -> Result<Option<RepositoryLock>> {
  try_lock_file(&git_common_dir.join(LOCK_FILE_NAME))
}

/// The lock of `$GIT_COMMON_DIR` as another process holds it (`lock_as_another_process`)
pub fn lock_repository_as_another_process(git_common_dir: &Path, command: &str) -> Result<RepositoryLock> {
  lock_as_another_process(git_common_dir.join(LOCK_FILE_NAME), command)
}

/// The lock of `$GIT_COMMON_DIR` for the app: after the operations of this process before it in its line, as long as
/// they take, it waits, without a bound, while the record of the holder names a live process (`LockHolder`: `bd pick
/// --follow` of an agent), calling `on_holder` when the holder changes and about once a second (its time grows), in the
/// line too (the record of the one before it, which names this process or the other one it waits for), and refuses only
/// when the user canceled (`canceled`, polled in the line too, `LockWaitCanceled`) or when no record names a live holder
/// of the file lock for `unknown_timeout` (`RepositoryLocked`): a holder that writes none, or one whose record is not
/// written yet. A killed holder frees the lock with its process. `bd` keeps the bounded wait (`lock_repository_in`)
#[instrument(skip(on_holder, canceled))]
pub fn lock_repository_while_held(git_common_dir: &Path, unknown_timeout: Duration, on_holder: &mut dyn FnMut(&LockHolder), canceled: &dyn Fn() -> bool) -> Result<RepositoryLock> {
  lock_file_while_held(git_common_dir.join(LOCK_FILE_NAME), unknown_timeout, on_holder, canceled)
}

/// `lock_repository_while_held` of the lock file `path`, created when missing: the lease pool's `clones.lock`, which a
/// get that adds a slot holds for the whole clone
pub fn lock_file_while_held(path: PathBuf, unknown_timeout: Duration, on_holder: &mut dyn FnMut(&LockHolder), canceled: &dyn Fn() -> bool) -> Result<RepositoryLock> {
  wait_while_held(path, None, unknown_timeout, on_holder, canceled)
}

/// `lock_file_while_held`, its record naming `subject` (`LockHolder::subject`), what the holder works on: a waiter that
/// `on_holder` tells of a holder of the same subject may take its result once the lock is its own, instead of doing the
/// work again (the fetch of a tracking ref, `sync_core::fetch_lock`)
pub fn lock_file_while_held_for(
  path: PathBuf,
  subject: &str,
  unknown_timeout: Duration,
  on_holder: &mut dyn FnMut(&LockHolder),
  canceled: &dyn Fn() -> bool,
) -> Result<RepositoryLock> {
  wait_while_held(path, Some(subject), unknown_timeout, on_holder, canceled)
}

/// The wait of `lock_file_while_held` and `lock_file_while_held_for`: the record of the lock names `subject`, none for a
/// lock that nobody joins
fn wait_while_held(path: PathBuf, subject: Option<&str>, unknown_timeout: Duration, on_holder: &mut dyn FnMut(&LockHolder), canceled: &dyn Fn() -> bool) -> Result<RepositoryLock> {
  let file = open_lock_file(&path)?;
  let started = Instant::now();
  let mut teller = Teller::default();
  let Some(turn) = take_turn(
    &path,
    &mut || {
      teller.tell(&path, on_holder);
    },
    canceled,
  ) else {
    return Err(anyhow::Error::new(LockWaitCanceled { path }));
  };
  // the file lock, which only another process holds now
  let mut known = Instant::now();
  loop {
    match file.try_lock() {
      Ok(()) => {
        let holder = LockHolder {
          subject: subject.map(ToString::to_string),
          ..LockHolder::of_this_process()
        };
        return Ok(locked(file, &path, &holder, Some(turn)));
      }
      Err(TryLockError::WouldBlock) => {
        if canceled() {
          return Err(anyhow::Error::new(LockWaitCanceled { path }));
        }
        if teller.tell(&path, on_holder) {
          known = Instant::now();
        } else if known.elapsed() >= unknown_timeout {
          return Err(anyhow::Error::new(RepositoryLocked {
            path,
            waited: started.elapsed(),
            holder: None,
          }));
        }
        std::thread::sleep(POLL_INTERVAL);
      }
      Err(TryLockError::Error(e)) => return Err(anyhow::Error::new(e).context(format!("Cannot lock {}", path.display()))),
    }
  }
}

/// How often the wait for a live holder tells it again: the time of its hold grows
const HOLDER_TICK: Duration = Duration::from_secs(1);

/// Whom a wait is for, as the record of the lock names it: told when the holder changes and about once a second
#[derive(Default)]
struct Teller {
  /// The pid told last, and when
  told: Option<(u32, Instant)>,
}

impl Teller {
  /// Reads the record of the lock `path` and tells its holder to `on_holder` when due; whether it names a live holder
  fn tell(&mut self, path: &Path, on_holder: &mut dyn FnMut(&LockHolder)) -> bool {
    let Some(holder) = LockHolder::read(path) else {
      return false;
    };
    if self.told.is_none_or(|(pid, at)| pid != holder.pid || at.elapsed() >= HOLDER_TICK) {
      on_holder(&holder);
      self.told = Some((holder.pid, Instant::now()));
    }
    true
  }
}

/// The app's side of a wait for a live holder of the lock (`LockWait::WhileHeld`): it hears whom the wait is for and
/// ends it when the user stops waiting (`operations::rewrite::StepWaiter`, the step `WaitingForLock` of an operation of
/// the engine, whose Stop Waiting is `cancel_operation`)
pub trait LockWaiter: Sync {
  /// The holder the wait is for, when it changes and about once a second: another process, or this one while an
  /// operation of it before this one in its line holds the lock; `None` once the wait ended, with the lock or without
  /// it. Not called when the lock is free at once
  fn holder(&self, holder: Option<&LockHolder>);
  /// Whether the user stopped waiting: the wait ends with `LockWaitCanceled`, before the rewrite read or wrote anything
  fn canceled(&self) -> bool;
  /// How long the wait goes on while no record names a live holder (one that writes none), then `RepositoryLocked`
  fn unknown_timeout(&self) -> Duration {
    DEFAULT_LOCK_TIMEOUT
  }
}

/// How a rewrite waits for another process that holds the repository lock; for the operations of its own process it
/// waits in their line, as long as they take, either way
#[derive(Clone, Copy)]
pub enum LockWait<'a> {
  /// At most this long, then `RepositoryLocked`: `bd` (`locked`, exit 3), and a rewrite nobody waits on
  Bounded(Duration),
  /// The app (`lock_repository_while_held`): as long as the record of the holder names a live process; only a holder no
  /// record names refuses, after `LockWaiter::unknown_timeout`
  WhileHeld(&'a dyn LockWaiter),
}

impl Default for LockWait<'_> {
  fn default() -> Self {
    Self::Bounded(DEFAULT_LOCK_TIMEOUT)
  }
}

impl fmt::Debug for LockWait<'_> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Bounded(timeout) => f.debug_tuple("Bounded").field(timeout).finish(),
      Self::WhileHeld(_) => f.write_str("WhileHeld"),
    }
  }
}

impl LockWait<'_> {
  /// The lock of the repository at `repository_path`, waited for as this wait says
  pub fn lock(&self, git_executor: &GitCommandExecutor, repository_path: &str) -> Result<RepositoryLock> {
    self.lock_with(git_executor, repository_path, &|| {})
  }

  /// `lock`; `on_wait` is called once, when another holds the lock and the wait starts (the update says so)
  pub fn lock_with(&self, git_executor: &GitCommandExecutor, repository_path: &str, on_wait: &dyn Fn()) -> Result<RepositoryLock> {
    let common_dir = git_common_dir(git_executor, repository_path)?;
    self.lock_in(Path::new(&common_dir), on_wait)
  }

  /// `lock_with` for a caller that knows `$GIT_COMMON_DIR` (absolute): no git process
  pub fn lock_in(&self, git_common_dir: &Path, on_wait: &dyn Fn()) -> Result<RepositoryLock> {
    match self {
      Self::Bounded(timeout) => lock_repository_in(git_common_dir, *timeout, on_wait),
      Self::WhileHeld(waiter) => {
        let mut waited = false;
        let mut on_holder = |holder: &LockHolder| {
          if !waited {
            waited = true;
            on_wait();
          }
          waiter.holder(Some(holder));
        };
        let locked = lock_repository_while_held(git_common_dir, waiter.unknown_timeout(), &mut on_holder, &|| waiter.canceled());
        if waited {
          waiter.holder(None);
        }
        locked
      }
    }
  }
}

/// The lock file at `path`, created when missing
fn open_lock_file(path: &Path) -> Result<File> {
  OpenOptions::new()
    .create(true)
    .truncate(false)
    .write(true)
    .open(path)
    .with_context(|| format!("Cannot open {}", path.display()))
}

/// The lock just taken on `file` in `turn`, with the record of `holder` next to it
fn locked(file: File, path: &Path, holder: &LockHolder, turn: Option<Place>) -> RepositoryLock {
  debug!(path = %path.display(), "locked");
  // a lock without its record still locks: the record only names the holder to the ones that wait
  let record = holder
    .write(path)
    .inspect_err(|e| warn!(error = %format!("{e:#}"), "cannot write the holder record of the lock"))
    .ok();
  RepositoryLock {
    _file: file,
    holder: record,
    _turn: turn,
  }
}

/// An exclusive lock on the file `path`, created when missing, as `lock_repository_in` takes it: the lease pool has a
/// lock of its own (`lease`), so a lease never waits for a long update. It waits in the line of this process as long as
/// the ones before it take, `timeout` counts only the wait for another process at the file lock; `on_wait` is called
/// once, when the wait starts
pub fn lock_file(path: PathBuf, timeout: Duration, on_wait: &dyn Fn()) -> Result<RepositoryLock> {
  let file = open_lock_file(&path)?;

  let started = Instant::now();
  let waiting = Cell::new(false);
  let wait_starts = || {
    if !waiting.replace(true) {
      on_wait();
    }
  };
  let Some(turn) = take_turn(&path, &mut || wait_starts(), &|| false) else {
    return Err(anyhow::Error::new(LockWaitCanceled { path }));
  };
  let turned = Instant::now();
  loop {
    match file.try_lock() {
      Ok(()) => return Ok(locked(file, &path, &LockHolder::of_this_process(), Some(turn))),
      Err(TryLockError::WouldBlock) if turned.elapsed() < timeout => {
        wait_starts();
        std::thread::sleep(POLL_INTERVAL.min(timeout));
      }
      Err(TryLockError::WouldBlock) => {
        let holder = LockHolder::read(&path);
        return Err(anyhow::Error::new(RepositoryLocked {
          path,
          waited: started.elapsed(),
          holder,
        }));
      }
      Err(TryLockError::Error(e)) => return Err(anyhow::Error::new(e).context(format!("Cannot lock {}", path.display()))),
    }
  }
}

/// The lock of the file `path` when nobody holds it or waits for it, this process included; `None` at once otherwise
fn try_lock_file(path: &Path) -> Result<Option<RepositoryLock>> {
  let file = open_lock_file(path)?;
  let Some(turn) = Place::turn_if_free(path) else {
    return Ok(None);
  };
  match file.try_lock() {
    Ok(()) => Ok(Some(locked(file, path, &LockHolder::of_this_process(), Some(turn)))),
    Err(TryLockError::WouldBlock) => Ok(None),
    Err(TryLockError::Error(e)) => Err(anyhow::Error::new(e).context(format!("Cannot lock {}", path.display()))),
  }
}

/// The lock of the file `path` as another process holds it (an agent's `bd pick --follow`), its record naming `command`
/// and this process: taken on a descriptor of its own, outside the line of this process, so the operations of this
/// process wait for it as they wait for another process, name it by its record, and refuse after their bound. For the
/// tests and the E2E server, whose app runs in the process that holds it; a held lock refuses it at once
pub fn lock_as_another_process(path: PathBuf, command: &str) -> Result<RepositoryLock> {
  lock_as_another_process_for(path, command, None)
}

/// `lock_as_another_process`, its record naming `subject`, what the holder works on (`lock_file_while_held_for`): the
/// tracking ref the fetch of an agent writes (`sync_core::fetch_lock::hold_fetch_as_another_process`)
pub fn lock_as_another_process_for(path: PathBuf, command: &str, subject: Option<&str>) -> Result<RepositoryLock> {
  let file = open_lock_file(&path)?;
  match file.try_lock() {
    Ok(()) => {
      let holder = LockHolder {
        command: command.to_string(),
        subject: subject.map(str::to_owned),
        ..LockHolder::of_this_process()
      };
      Ok(locked(file, &path, &holder, None))
    }
    Err(TryLockError::WouldBlock) => {
      let holder = LockHolder::read(&path);
      Err(anyhow::Error::new(RepositoryLocked {
        path,
        waited: Duration::ZERO,
        holder,
      }))
    }
    Err(TryLockError::Error(e)) => Err(anyhow::Error::new(e).context(format!("Cannot lock {}", path.display()))),
  }
}

/// The line of this process for one lock file: the operations of this process that wait for it, first come first, and
/// whether one has its turn (it waits for the file lock, which only another process holds then, or holds it)
#[derive(Default)]
struct Line {
  state: Mutex<LineState>,
  /// Told when the turn is passed on or a place leaves
  changed: Condvar,
}

#[derive(Default)]
struct LineState {
  /// The number of the next place
  next: u64,
  /// The places that wait for the turn, in the order they came
  waiting: VecDeque<u64>,
  /// One place has the turn
  taken: bool,
}

impl LineState {
  /// Whether the turn is free and `ticket` is the first in the line
  fn turn_of(&self, ticket: u64) -> bool {
    !self.taken && self.waiting.front() == Some(&ticket)
  }
}

/// The lines of this process by the canonical path of their lock file; a line goes once no place has its turn or waits
static LINES: LazyLock<Mutex<HashMap<PathBuf, Arc<Line>>>> = LazyLock::new(Mutex::default);

fn lines() -> MutexGuard<'static, HashMap<PathBuf, Arc<Line>>> {
  LINES.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Line {
  fn state(&self) -> MutexGuard<'_, LineState> {
    self.state.lock().unwrap_or_else(PoisonError::into_inner)
  }
}

/// The key of the line of the lock file `path`: one file, however its path is spelled (`/var` and `/private/var` on
/// macOS, a relative path); the file exists, the lock opened it
fn line_key(path: &Path) -> PathBuf {
  std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// A place in the line of a lock file: it waits for the turn (`wait_turn`), then has it until dropped; dropped before
/// the turn came, it leaves the line
struct Place {
  key: PathBuf,
  line: Arc<Line>,
  ticket: u64,
  has_turn: bool,
}

impl fmt::Debug for Place {
  /// The line itself is left out: its state changes under another lock
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("Place")
      .field("key", &self.key)
      .field("ticket", &self.ticket)
      .field("has_turn", &self.has_turn)
      .finish_non_exhaustive()
  }
}

impl Place {
  /// A place at the end of the line of the lock file `path`
  fn enter(path: &Path) -> Self {
    let key = line_key(path);
    let mut lines = lines();
    let line = Arc::clone(lines.entry(key.clone()).or_default());
    let mut state = line.state();
    let ticket = state.next;
    state.next += 1;
    state.waiting.push_back(ticket);
    drop(state);
    drop(lines);
    Self {
      key,
      line,
      ticket,
      has_turn: false,
    }
  }

  /// The turn of the line of the lock file `path` when no place has it or waits for it; `None` otherwise, the place
  /// left at once
  fn turn_if_free(path: &Path) -> Option<Self> {
    let mut place = Self::enter(path);
    place.wait_turn(Duration::ZERO).then_some(place)
  }

  /// Takes the turn once it came, waiting for it at most `wait`; whether it has it
  fn wait_turn(&mut self, wait: Duration) -> bool {
    let mut state = self.line.state();
    if !state.turn_of(self.ticket) {
      state = self.line.changed.wait_timeout(state, wait).unwrap_or_else(PoisonError::into_inner).0;
      if !state.turn_of(self.ticket) {
        return false;
      }
    }
    state.waiting.pop_front();
    state.taken = true;
    self.has_turn = true;
    true
  }
}

impl Drop for Place {
  /// Passes the turn on, or leaves the line; the line goes when nobody else is in it
  fn drop(&mut self) {
    let mut lines = lines();
    let mut state = self.line.state();
    if self.has_turn {
      state.taken = false;
    } else {
      state.waiting.retain(|ticket| *ticket != self.ticket);
    }
    if !state.taken && state.waiting.is_empty() && lines.get(&self.key).is_some_and(|line| Arc::ptr_eq(line, &self.line)) {
      lines.remove(&self.key);
    }
    drop(state);
    drop(lines);
    self.line.changed.notify_all();
  }
}

/// The turn of the line of this process for the lock file `path`: it waits for the operations of this process before
/// it, as long as they take (they never refuse each other), calling `meanwhile` about every `POLL_INTERVAL` while it
/// waits; `None` once `canceled` (polled as often), its place left
fn take_turn(path: &Path, meanwhile: &mut dyn FnMut(), canceled: &dyn Fn() -> bool) -> Option<Place> {
  let mut place = Place::enter(path);
  let mut wait = Duration::ZERO;
  loop {
    if place.wait_turn(wait) {
      return Some(place);
    }
    if canceled() {
      return None;
    }
    meanwhile();
    wait = POLL_INTERVAL;
  }
}

/// What every rewrite of the checked out branch takes besides its request (`*_core(…, options)`): the HEAD the request
/// was made for and the repository lock. The core reads and computes before the lock, on the oids it read once, and
/// holds the lock only for the witness of what it read and its compare-and-swap write (`lock_prepared`), so two
/// rewrites never interleave, also when one runs in the app and the other in `bd`, and a long computation never makes
/// another writer wait
#[derive(Debug, Clone, Default)]
pub struct RewriteOptions<'a> {
  /// The HEAD the request was made for; abbreviated ids are fine. Another HEAD is `HeadMoved`, when the core reads it
  /// and when it reads it again under the lock after a witness that missed
  pub expect_head: Option<String>,
  /// How the rewrite waits for another holder of the lock: `bd` at most its timeout, then `RepositoryLocked`; the app
  /// as long as the holder lives
  pub lock_wait: LockWait<'a>,
  /// The repository lock the caller holds, so that it can write more under it (`bd group` with descriptions); the core
  /// then reads and computes under it too, without a witness. `None`: the core takes the lock after it computed
  pub held_lock: Option<&'a RepositoryLock>,
}

impl<'a> RewriteOptions<'a> {
  /// The options of a rewrite of the app: no expected HEAD, the lock waited for as `lock_wait` says
  pub fn waiting(lock_wait: LockWait<'a>) -> Self {
    Self { lock_wait, ..Self::default() }
  }

  /// The lock of the rewrite, held until the guard is dropped: none when the caller holds it, else the lock waited for
  /// as `lock_wait` says
  pub fn lock(&self, git_executor: &GitCommandExecutor, repository_path: &str) -> Result<Option<RepositoryLock>> {
    match self.held_lock {
      Some(_) => Ok(None),
      None => self.lock_wait.lock(git_executor, repository_path).map(Some),
    }
  }

  /// `lock_prepared` with the lock of these options
  ///
  /// # Errors
  /// The lock not taken (`RepositoryLocked`, `LockWaitCanceled`), the witness not read, the error of `again`
  pub fn lock_prepared<P: Prepared, E: From<anyhow::Error>>(
    &self,
    git_executor: &GitCommandExecutor,
    repository_path: &str,
    prepared: P,
    again: impl FnOnce() -> Result<P, E>,
  ) -> Result<(P, Option<RepositoryLock>), E> {
    lock_prepared((self.lock_wait, self.held_lock), git_executor, repository_path, prepared, again)
  }

  /// `lock_prepared_move` with the lock of these options
  ///
  /// # Errors
  /// As `lock_prepared_move`
  pub fn lock_prepared_move<P: Prepared>(
    &self,
    git_executor: &GitCommandExecutor,
    repository_path: &str,
    prepared: Result<P>,
    again: impl FnOnce() -> Result<P>,
  ) -> Result<(P, Option<RepositoryLock>)> {
    lock_prepared_move((self.lock_wait, self.held_lock), git_executor, repository_path, prepared, again)
  }

  /// `ensure_expected_head` of `expect_head`, the branch read only for the error
  pub fn ensure_head(&self, git_executor: &GitCommandExecutor, repository_path: &str, head: &str) -> Result<(), HeadMoved> {
    ensure_expected_head(self.expect_head.as_deref(), head, || {
      checked_out_branch(git_executor, repository_path).unwrap_or_else(|_| "HEAD".to_string())
    })
  }

  /// HEAD as a full id when `expect_head` is set and HEAD is still that commit, for a git-ops rewrite that compares the
  /// full id; `None`, without a process, when no HEAD is expected
  pub fn expected_full_head(&self, git_executor: &GitCommandExecutor, repository_path: &str) -> Result<Option<String>> {
    if self.expect_head.is_none() {
      return Ok(None);
    }
    let head = git_executor.execute_command(&["rev-parse", "--verify", "HEAD^{commit}"], repository_path)?;
    self.ensure_head(git_executor, repository_path, &head)?;
    Ok(Some(head))
  }
}

/// The refs a rewrite computed on, as it read them before the repository lock: under the lock one read of them
/// (`holds`) tells whether what it computed may be written
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Witness {
  /// HEAD and the branch it was on (`main`), `HEAD` for a detached one (git refuses a branch of that name); `None` for
  /// a rewrite that reads no HEAD
  pub head: Option<(String, String)>,
  /// `(full ref, its commit then)`, `None` for a ref that did not exist: the stores and the upstream the rewrite read,
  /// the mark of an interrupted move it assumed absent
  pub refs: Vec<(String, Option<String>)>,
  /// `(top level, its witness)` of another worktree the rewrite moves too (the source of a pick that followed), read in
  /// its own directory: HEAD is a worktree's
  pub elsewhere: Vec<(String, Self)>,
}

impl Witness {
  /// HEAD at `head` on `branch` (`HEAD`: detached), and nothing else
  pub fn of_head(head: &str, branch: &str) -> Self {
    Self {
      head: Some((head.to_string(), branch.to_string())),
      ..Self::default()
    }
  }

  /// This witness and `name` at `tip` (`None`: absent)
  #[must_use]
  pub fn with_ref(mut self, name: &str, tip: Option<&str>) -> Self {
    self.refs.push((name.to_string(), tip.map(str::to_string)));
    self
  }

  /// Whether HEAD, its branch and every ref are still as read: one `for-each-ref` (`head_and_tips`) per worktree, none
  /// for an empty witness
  ///
  /// # Errors
  /// The refs not read
  pub fn holds(&self, git_executor: &GitCommandExecutor, repository_path: &str) -> Result<bool> {
    for (top_level, witness) in &self.elsewhere {
      if !witness.holds(git_executor, top_level)? {
        return Ok(false);
      }
    }
    if self.head.is_none() && self.refs.is_empty() {
      return Ok(true);
    }
    let names: Vec<&str> = self.refs.iter().map(|(name, _)| name.as_str()).collect();
    let now = head_and_tips(git_executor, repository_path, &names)?;
    let head_holds = self.head.as_ref().is_none_or(|(head, branch)| {
      let on = now.head_ref.as_deref().map_or(Some("HEAD"), |name| name.strip_prefix("refs/heads/"));
      now.head.as_deref() == Some(head.as_str()) && on == Some(branch.as_str())
    });
    Ok(head_holds && self.refs.iter().all(|(name, tip)| now.tips.get(name) == tip.as_ref()))
  }
}

/// What a rewrite core computed before the repository lock, on the oids it read once: its new objects written (on
/// temporary refs), no ref moved
pub trait Prepared {
  /// The refs it computed on
  fn witness(&self) -> Witness;

  /// Delete the temporary refs of the objects it wrote: it is abandoned, or computed once more
  fn discard(&self, git_executor: &GitCommandExecutor, repository_path: &str);
}

/// `Prepared` of a rewrite of git-ops whose new commits sit on temporary refs: its witness is HEAD at the tip they were
/// computed on (`old_head`), on its branch
macro_rules! prepared_on_head {
  ($($prepared:ty),+) => {$(
    impl Prepared for $prepared {
      fn witness(&self) -> Witness {
        Witness::of_head(self.old_head(), self.branch())
      }

      fn discard(&self, git_executor: &GitCommandExecutor, repository_path: &str) {
        Self::discard(self, git_executor, repository_path);
      }
    }
  )+};
}

prepared_on_head!(PreparedRewrite, PreparedReword);

impl Prepared for CheckedRestore {
  /// HEAD at the head the restore moves back from, on its branch
  fn witness(&self) -> Witness {
    Witness::of_head(self.head(), self.branch())
  }

  /// A restore writes no object
  fn discard(&self, _git_executor: &GitCommandExecutor, _repository_path: &str) {}
}

/// The lock of a rewrite that computed `prepared` before it (AGENTS.md, "Repository lock"): `(lock_wait, held_lock)`.
/// Under the lock only the witness is read, one process; when it holds, `prepared` is written as it is. When a ref it
/// computed on moved meanwhile (another process rewrote HEAD, a sync dropped the lines of a store), `prepared` is
/// discarded and `again` computes it once more, under the lock now, so nothing it reads moves again: the HEAD it
/// expects is checked there, and another HEAD is `HeadMoved`. The caller writes under the lock it gets back: the
/// compare-and-swap ref transaction, then the move of the index. A lock not taken discards `prepared`. A caller that
/// holds the lock (`held_lock`) computed under it: no witness then
///
/// # Errors
/// The lock not taken (`RepositoryLocked`, `LockWaitCanceled`), the witness not read, the error of `again`
pub fn lock_prepared<P: Prepared, E: From<anyhow::Error>>(
  (lock_wait, held_lock): (LockWait<'_>, Option<&RepositoryLock>),
  git_executor: &GitCommandExecutor,
  repository_path: &str,
  prepared: P,
  again: impl FnOnce() -> Result<P, E>,
) -> Result<(P, Option<RepositoryLock>), E> {
  if held_lock.is_some() {
    return Ok((prepared, None));
  }
  let lock = match lock_wait.lock(git_executor, repository_path) {
    Ok(lock) => lock,
    Err(error) => {
      prepared.discard(git_executor, repository_path);
      return Err(error.into());
    }
  };
  match prepared.witness().holds(git_executor, repository_path) {
    Ok(true) => Ok((prepared, Some(lock))),
    Ok(false) => {
      prepared.discard(git_executor, repository_path);
      debug!("a ref moved since the rewrite read it: computed once more under the lock");
      Ok((again()?, Some(lock)))
    }
    Err(error) => {
      prepared.discard(git_executor, repository_path);
      Err(error.into())
    }
  }
}

/// `lock_prepared` of a move that the files follow (an unapply, a reapply, the undo of a move; AGENTS.md, "Repository
/// lock"): `prepared` was read and computed before the lock, the dry run of the files on a copy of the index included,
/// on a worktree without the mark of an interrupted move (`git_ops::update_branch::pending_mark`). The witness reads
/// that mark absent with HEAD and the refs it computed on, in its one `for-each-ref`; a mark there lets the files of
/// that move follow (`finish_pending_update`), under the lock, before `again` computes once more there, as after any
/// ref that moved. A refusal by local changes before the lock may be the files of such a move: with the mark they
/// follow under the lock and `again` runs; without it the refusal is the answer, and no lock is waited for. Under a
/// lock the caller holds, the files of an interrupted move follow, and one that did computes it again
///
/// # Errors
/// The lock not taken (`RepositoryLocked`, `LockWaitCanceled`), the witness or the mark not read, the files of an
/// interrupted move that can't follow (`PendingUpdate`), the error of `prepared` or `again`
pub fn lock_prepared_move<P: Prepared>(
  (lock_wait, held_lock): (LockWait<'_>, Option<&RepositoryLock>),
  git_executor: &GitCommandExecutor,
  repository_path: &str,
  prepared: Result<P>,
  again: impl FnOnce() -> Result<P>,
) -> Result<(P, Option<RepositoryLock>)> {
  let discard = |prepared: &Result<P>| {
    if let Ok(prepared) = prepared {
      prepared.discard(git_executor, repository_path);
    }
  };
  if held_lock.is_some() {
    return match finish_pending_update(git_executor, repository_path) {
      Ok(None) => Ok((prepared?, None)),
      Ok(Some(_)) => {
        discard(&prepared);
        Ok((again()?, None))
      }
      Err(error) => {
        discard(&prepared);
        Err(error)
      }
    };
  }
  let prepared = match prepared {
    Ok(prepared) => prepared,
    Err(error) if refused_by_local_changes(&error) && pending_update(git_executor, repository_path)?.is_some() => {
      let lock = lock_wait.lock(git_executor, repository_path)?;
      finish_pending_update(git_executor, repository_path)?;
      return Ok((again()?, Some(lock)));
    }
    Err(error) => return Err(error),
  };
  let mark = match pending_mark(git_executor, repository_path) {
    Ok(mark) => mark,
    Err(error) => {
      prepared.discard(git_executor, repository_path);
      return Err(error);
    }
  };
  let marked = Marked { prepared, mark: &mark };
  let again = || -> Result<Marked<'_, P>> {
    finish_pending_update(git_executor, repository_path)?;
    Ok(Marked { prepared: again()?, mark: &mark })
  };
  let (marked, lock) = lock_prepared((lock_wait, None), git_executor, repository_path, marked, again)?;
  Ok((marked.prepared, lock))
}

/// The undo of a move that the files follow (an update, a pick, an unapply, a reapply): `check` before the repository
/// lock, waited for as `lock_wait` says, then under it the witness of HEAD and of the mark of an interrupted move
/// (`lock_prepared_move`), the check again when one moved. The undo to apply, with the lock to apply it under; one done
/// before (`UndoCheck::AlreadyUndone`) takes no lock, unless the mark of an interrupted move says its files have not
/// followed yet: they follow under the lock; local changes on a path it writes are `UndoBlocked`. `check` reads with
/// the directories of `repository_path` known
///
/// # Errors
/// As `lock_prepared_move`, and the refusals of `check` (`HeadMoved`, `NotAnUpdate`)
pub fn lock_checked_undo(
  lock_wait: LockWait<'_>,
  git_executor: &GitCommandExecutor,
  repository_path: &str,
  check: impl Fn(&GitCommandExecutor) -> Result<UndoCheck>,
) -> Result<(UndoCheck, Option<RepositoryLock>)> {
  // the guards, the lock and the mark take the directories read here
  let git_executor = &with_dirs_of(git_executor, repository_path)?;
  let ready = || match check(git_executor)? {
    UndoCheck::Ready(undo) if !undo.blocked().is_empty() => Err(anyhow::Error::new(UndoBlocked { paths: undo.blocked().to_vec() })),
    checked => Ok(checked),
  };
  let checked = ready();
  if matches!(checked, Ok(UndoCheck::AlreadyUndone { .. })) {
    // undone before: nothing to write and no lock, unless a crash stopped that undo before its files followed
    if pending_update(git_executor, repository_path)?.is_none() {
      return Ok((checked?, None));
    }
    let lock = lock_wait.lock(git_executor, repository_path)?;
    finish_pending_update(git_executor, repository_path)?;
    return Ok((ready()?, Some(lock)));
  }
  lock_prepared_move((lock_wait, None), git_executor, repository_path, checked, ready)
}

impl Prepared for UndoCheck {
  /// HEAD at the commit the undo moves back from; none for an undo done before, which writes nothing
  fn witness(&self) -> Witness {
    match self {
      Self::Ready(undo) => Witness::of_head(undo.head(), undo.branch()),
      Self::AlreadyUndone { .. } => Witness::default(),
    }
  }

  /// A checked undo writes no object
  fn discard(&self, _git_executor: &GitCommandExecutor, _repository_path: &str) {}
}

/// A prepared move and the mark of pending files its witness reads absent
struct Marked<'a, P> {
  prepared: P,
  mark: &'a str,
}

impl<P: Prepared> Prepared for Marked<'_, P> {
  fn witness(&self) -> Witness {
    self.prepared.witness().with_ref(self.mark, None)
  }

  fn discard(&self, git_executor: &GitCommandExecutor, repository_path: &str) {
    self.prepared.discard(git_executor, repository_path);
  }
}

/// A refusal of a move because local changes sit on paths it writes: of an unapply or a reapply (`MoveBlocked`), of an
/// undo (`UndoBlocked`), of a pick or an update (`FilesBlocked`)
fn refused_by_local_changes(error: &anyhow::Error) -> bool {
  error.downcast_ref::<MoveBlocked>().is_some() || error.downcast_ref::<UndoBlocked>().is_some() || error.downcast_ref::<FilesBlocked>().is_some()
}

/// Refuse with `HeadMoved` unless `head` (a full id) is `expected`, or starts with it when `expected` is an abbreviated
/// id of at least `MIN_ID_LENGTH` characters; `branch` names the checked out branch for the error. No `expected` is no
/// check
pub fn ensure_expected_head(expected: Option<&str>, head: &str, branch: impl FnOnce() -> String) -> Result<(), HeadMoved> {
  match expected {
    Some(expected) if !(expected.len() >= MIN_ID_LENGTH && head.starts_with(expected)) => Err(HeadMoved {
      branch: branch(),
      expected: expected.to_string(),
      actual: head.to_string(),
    }),
    _ => Ok(()),
  }
}

#[cfg(test)]
#[path = "repository_lock_test.rs"]
mod repository_lock_test;
