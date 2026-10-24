//! The index lock of a worktree (`<git dir>/index.lock`) in the way of a write of its index
//! (docs/design/git-operations.md, "Index lock"). Another git holds it: an IDE's `git status` refreshes the index for
//! the whole of its scan, 11–20 s on the IntelliJ monorepo, and starts again at every ref change, so at the move of a
//! pick too. Or a git that was killed (SIGKILL) left it: git removes its lock on exit, SIGTERM and SIGINT, never after
//! SIGKILL. Git has no wait for the index lock (`core.filesRefLockTimeout` is only for refs), so a write of the user's
//! index (the files of a move, the index of a rewrite) runs through `IndexLock::retry`: a step that the lock blocked
//! runs again once the lock is gone, a lock that no process owns is removed, and after the wait the answer is
//! `IndexLocked`. The happy path costs no process: the lock file is looked at only after a step failed.

use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use std::cell::OnceCell;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};
use sync_utils::process_scan::{ProcessScan, processes_with_open_file};
use tracing::{debug, warn};

/// How long one write of the index waits for the lock of another process
pub const INDEX_LOCK_WAIT: Duration = Duration::from_secs(30);

/// How often a wait looks at the lock file
const POLL: Duration = Duration::from_millis(50);

/// How often a wait asks who owns the lock; two answers "nobody" this far apart remove it
const OWNER_CHECK: Duration = Duration::from_secs(2);

/// A git that started up to this long after the lock appeared may still have taken it: Linux knows the start of a
/// process to a second
const START_TOLERANCE: Duration = Duration::from_secs(1);

/// How much younger than the start of a step a lock may look and still count as the one that blocked it: Linux stamps a
/// file with a coarse clock (a tick behind), so a lock taken before the step may look younger. A lock taken that soon
/// after a step that refused on its own makes the step run once more, which refuses again: no harm
const TIMESTAMP_SLACK: Duration = Duration::from_millis(100);

/// The processes `IndexLocked` names at most
const MAX_HOLDERS: usize = 5;

/// The index lock of the worktree of `worktree`
pub struct IndexLock<'a> {
  git_executor: &'a GitCommandExecutor,
  /// A directory of the worktree, its top level for the callers: where its git processes work, and what a message names
  worktree: &'a str,
  /// Known up front, or read once (`rev-parse --git-path index.lock`) when a step failed
  path: OnceCell<PathBuf>,
  owner_check: Duration,
}

/// A process that may own the lock: it has the file open, or it is a git of the worktree that started before the lock
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockHolder {
  pub pid: u32,
  /// The command name, as `ps -o comm` shows it
  pub name: String,
  /// The arguments as one line, at most 200 characters
  pub command: String,
}

/// The index lock of a worktree stayed for the whole wait
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{}", self.message())]
pub struct IndexLocked {
  pub worktree: String,
  pub path: String,
  /// Since the lock appeared
  pub held_for: Duration,
  /// Empty when no process could be named: no process scan on this platform
  pub holders: Vec<LockHolder>,
}

impl IndexLocked {
  fn message(&self) -> String {
    let held = format!("{} has been held for {} s", self.path, self.held_for.as_secs());
    if self.holders.is_empty() {
      return format!(
        "Cannot write the index of {}: {held}, and no process could be named. Retry; remove it once no git runs in the worktree",
        self.worktree
      );
    }
    let holders: Vec<String> = self.holders.iter().map(|holder| format!("{} ({})", holder.command, holder.pid)).collect();
    format!("Cannot write the index of {}: {held} by {}. Retry once it is gone", self.worktree, holders.join(", "))
  }
}

/// What tells one lock file from another that took its place
#[derive(Debug, Clone, PartialEq, Eq)]
struct Identity {
  #[cfg(unix)]
  inode: (u64, u64),
  modified: Option<SystemTime>,
  /// When it appeared (`born`)
  born: Option<SystemTime>,
}

impl Identity {
  fn of(path: &Path) -> Option<Self> {
    let metadata = std::fs::symlink_metadata(path).ok()?;
    #[cfg(unix)]
    let inode = {
      use std::os::unix::fs::MetadataExt;
      (metadata.dev(), metadata.ino())
    };
    Some(Self {
      #[cfg(unix)]
      inode,
      modified: metadata.modified().ok(),
      born: born(&metadata),
    })
  }
}

impl<'a> IndexLock<'a> {
  /// The lock of the worktree of `worktree`; its path is read only when a step fails
  pub const fn of(git_executor: &'a GitCommandExecutor, worktree: &'a str) -> Self {
    Self {
      git_executor,
      worktree,
      path: OnceCell::new(),
      owner_check: OWNER_CHECK,
    }
  }

  /// The lock of the worktree whose git dir is `git_dir`
  pub fn at(git_executor: &'a GitCommandExecutor, worktree: &'a str, git_dir: &str) -> Self {
    let lock = Self::of(git_executor, worktree);
    let _ = lock.path.set(Path::new(git_dir).join("index.lock"));
    lock
  }

  /// Ask who owns the lock this often (`OWNER_CHECK`); for tests
  #[must_use]
  pub const fn with_owner_check(mut self, owner_check: Duration) -> Self {
    self.owner_check = owner_check;
    self
  }

  fn path(&self) -> Result<&Path> {
    if let Some(path) = self.path.get() {
      return Ok(path);
    }
    let path = self
      .git_executor
      .execute_command(&["rev-parse", "--path-format=absolute", "--git-path", "index.lock"], self.worktree)?;
    Ok(self.path.get_or_init(|| PathBuf::from(path)))
  }

  /// Run `step`, a git that writes the index of the worktree, and run it again while the lock of another process blocks
  /// it: the step failed, and the lock exists now and appeared before the step started (`TIMESTAMP_SLACK`; a step that
  /// refused for its own reasons held the lock itself, and a lock that exists now appeared after it started). Such a step wrote nothing: git
  /// takes the lock before it writes. The wait (`wait_while_locked`) lasts at most `wait` in all
  pub fn retry<T>(&self, wait: Duration, mut step: impl FnMut() -> Result<T>) -> Result<T> {
    let deadline = Instant::now() + wait;
    loop {
      let started = SystemTime::now();
      let error = match step() {
        Ok(value) => return Ok(value),
        Err(error) => error,
      };
      let Ok(path) = self.path() else { return Err(error) };
      let blocked = Identity::of(path).and_then(|identity| identity.born).is_some_and(|born| born <= started + TIMESTAMP_SLACK);
      if !blocked {
        return Err(error);
      }
      debug!(path = %path.display(), error = %format!("{error:#}"), "the index lock of another process blocked a step: it waits");
      self.wait_until(deadline)?;
    }
  }

  /// Wait until no lock of another process is there, at most `wait`: before a move whose files follow, so that a
  /// collision waits while nothing has moved. Needs the path (`at`): a lock of unknown path is not looked for
  pub fn wait_while_locked(&self, wait: Duration) -> Result<()> {
    if self.path.get().is_none() {
      return Ok(());
    }
    self.wait_until(Instant::now() + wait)
  }

  /// Poll the lock until it is gone, asking every `owner_check` who owns it: a lock that no process owns at two answers
  /// that far apart, the same file at both, is removed. `IndexLocked` when it is still there at `deadline`
  fn wait_until(&self, deadline: Instant) -> Result<()> {
    let path = self.path()?;
    let mut ownerless: Option<(Identity, Instant)> = None;
    let mut next_check = Instant::now();
    loop {
      let Some(identity) = Identity::of(path) else { return Ok(()) };
      let now = Instant::now();
      if now >= next_check {
        next_check = now + self.owner_check;
        match self.holders(path, &identity) {
          Some(holders) if holders.is_empty() => match &ownerless {
            Some((seen, since)) if *seen == identity && now.duration_since(*since) >= self.owner_check => {
              if remove_if_same(path, &identity) {
                warn!(path = %path.display(), age_s = age(&identity).as_secs(), "removed an index lock that no process owns");
                return Ok(());
              }
              ownerless = None;
            }
            Some((seen, _)) if *seen == identity => {}
            _ => ownerless = Some((identity.clone(), now)),
          },
          _ => ownerless = None,
        }
      }
      if now >= deadline {
        let holders = self.holders(path, &identity).unwrap_or_default();
        return Err(anyhow::Error::new(IndexLocked {
          worktree: self.worktree.to_string(),
          path: path.display().to_string(),
          held_for: age(&identity),
          holders,
        }));
      }
      std::thread::sleep(POLL);
    }
  }

  /// The processes that may own the lock `identity` at `path`: those that have it open, and the git processes in the
  /// worktree that started before it appeared (a git between the write of its lock and its commit closes the file, while
  /// a hook runs). `None` when that can't be told: no process scan, no answer of who has the file open
  fn holders(&self, path: &Path, identity: &Identity) -> Option<Vec<LockHolder>> {
    let open = processes_with_open_file(path)?;
    let scan = ProcessScan::scan();
    let ProcessScan::Done { processes, .. } = &scan else { return None };
    let latest_start = identity.born.and_then(|born| born.checked_add(START_TOLERANCE));
    let inside: Vec<u32> = scan
      .inside(Path::new(self.worktree))
      .into_iter()
      .filter(|process| process.is_git())
      .filter(|process| match (process.started_at(), latest_start) {
        (Some(started), Some(latest)) => started <= latest,
        _ => true,
      })
      .map(|process| process.pid)
      .collect();
    let mut holders: Vec<LockHolder> = open
      .iter()
      .chain(inside.iter().filter(|pid| !open.contains(pid)))
      .map(|&pid| {
        let process = processes.iter().find(|process| process.pid == pid);
        LockHolder {
          pid,
          name: process.map_or_else(|| "?".to_string(), |process| process.name.clone()),
          command: process.map_or_else(|| "?".to_string(), |process| process.command_line(200)),
        }
      })
      .collect();
    holders.truncate(MAX_HOLDERS);
    Some(holders)
  }
}

/// When the lock file appeared: its birth, else its last change. Windows gives a file created under the name of one
/// deleted or renamed within 15 s the old one's creation time (NTFS tunneling), and git renames `index.lock` away at
/// every write, so there the last change tells: an empty lock is written once, when it is created
fn born(metadata: &std::fs::Metadata) -> Option<SystemTime> {
  if cfg!(windows) {
    return metadata.modified().ok();
  }
  metadata.created().or_else(|_| metadata.modified()).ok()
}

/// Since the lock appeared, as far as its file tells
fn age(identity: &Identity) -> Duration {
  identity.born.and_then(|born| born.elapsed().ok()).unwrap_or_default()
}

/// Remove the lock at `path` when it is still the file `identity`; whether it was removed
fn remove_if_same(path: &Path, identity: &Identity) -> bool {
  if Identity::of(path).as_ref() != Some(identity) {
    return false;
  }
  match std::fs::remove_file(path) {
    Ok(()) => true,
    Err(e) => {
      warn!(path = %path.display(), error = %e, "cannot remove an index lock that no process owns");
      false
    }
  }
}

#[cfg(test)]
#[path = "index_lock_test.rs"]
mod index_lock_test;
