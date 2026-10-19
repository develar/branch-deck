//! The lease pool of a repository (`bd lease`, spec/cli/lease.spec.md, "Lease" in docs/design/git-operations.md): linked
//! worktrees that agent sessions lease, one session per slot. The state in `$GIT_COMMON_DIR/branch-deck/leases.json` is
//! the authority: a free slot is pool property, and a get replaces its content, so no ancestry proves anything. The
//! pool has a lock of its own (`leases.lock`), held only while the state is read and written, never across a git that
//! writes a slot: two gets never wait for each other's checkouts, and no get waits for an update.

use crate::process_scan::{ProcessInfo, ProcessScan, is_running, process_started};
use crate::repository_lock::{RepositoryLock, lock_file};
use crate::resolve_copies::display_path;
use anyhow::{Context, Result, anyhow, bail};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::lease_slot::{
  UnlandedCommit, add_slot, checkout_slot, detach_slot_head, discard_slot_changes, ignored_paths, is_valid_branch_name, lease_pool_config, linked_worktree, slot_changes,
  unlanded_commits,
};
use git_ops::revs::{RepositoryDirs, read_head, ref_exists, resolve_commit};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fmt::Write;
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tracing::{debug, instrument, warn};

const STATE_DIR: &str = "branch-deck";
const STATE_FILE: &str = "leases.json";
const LOCK_FILE: &str = "leases.lock";
/// The file at the root of the source that names the ignored files a get copies into the slot
pub const WORKTREE_INCLUDE: &str = ".worktreeinclude";
/// How many unlanded commits a refused return names
const UNLANDED_LIMIT: usize = 20;

/// The state of the pool: every slot, free or leased
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LeaseStore {
  #[serde(default)]
  pub slots: Vec<Slot>,
}

/// A linked worktree of the pool
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Slot {
  /// The worktree id (its directory under `$GIT_COMMON_DIR/worktrees`); empty while a new slot is being added
  #[serde(default)]
  pub name: String,
  /// The top level, canonical
  pub path: String,
  /// A get found local changes in the free slot and skipped it; the next gets try it last
  #[serde(default, skip_serializing_if = "std::ops::Not::not")]
  pub dirty: bool,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub lease: Option<Lease>,
}

/// The lease of a slot
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lease {
  /// A UUID
  pub id: String,
  pub holder: String,
  /// Unix seconds
  pub leased_at: u64,
  pub state: LeaseState,
  /// The process of the get, and when it started (`process_started`): a `preparing` lease whose process is gone is free
  pub pid: u32,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub process_started: Option<u64>,
  /// The commit of the source that the slot was moved to
  pub source_head: String,
  /// The branch the get created at `source_head`
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub branch: Option<String>,
  /// The paths the get copied from the source (`.worktreeinclude`), relative, `/`-separated: the return removes them
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub seeded: Vec<String>,
  /// Whether the get moved a free slot rather than adding a new one
  #[serde(default)]
  pub reused: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LeaseState {
  /// The get recorded the lease and moves the slot
  Preparing,
  /// The slot is at `source_head`, seeded, the holder's
  Ready,
}

impl Lease {
  /// Whether the lease is `preparing` and its process is gone: the next get takes the slot as free
  pub fn is_abandoned(&self) -> bool {
    self.state == LeaseState::Preparing && !is_running(self.pid, self.process_started)
  }
}

/// What the pool refuses; `bd` gives each its error code
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LeaseRefusal {
  #[error("{0} holds no lease of this repository's pool")]
  NoLease(String),
  #[error("The lease of {path} is being prepared by the process {pid}")]
  Preparing { path: String, pid: u32 },
  #[error("The lease of {path} is {actual}, not {expected}")]
  LeaseMismatch { path: String, expected: String, actual: String },
  #[error("Processes work in {path}: {}; stop them, or wait until they are done", describe_processes(.processes))]
  SlotBusy { path: String, processes: Vec<(u32, String)> },
  #[error(
    "HEAD of {path} has {} commit(s) that no branch, tag or remote branch holds; land them first (`bd pick --from {path} --follow` in the shared checkout), or pass --confirm-preserved to give them up",
    .commits.len()
  )]
  UnlandedCommits { path: String, commits: Vec<UnlandedCommit> },
  #[error("{path} has local changes; commit or land them, or pass --confirm-preserved to discard them")]
  DirtyWorktree { path: String, paths: Vec<String> },
  #[error("{path} has local changes; a slot of the pool must be clean")]
  DirtyCandidate { path: String, paths: Vec<String> },
  #[error("The branch {0} exists")]
  BranchExists(String),
  #[error("{0:?} is not a valid branch name")]
  InvalidBranch(String),
  #[error("{path} is not a slot: {reason}")]
  NotASlot { path: String, reason: String },
  #[error("The pool holds {0} already")]
  InPool(String),
  #[error("The repository at {0} has no commits yet")]
  NoCommits(String),
}

fn describe_processes(processes: &[(u32, String)]) -> String {
  processes.iter().map(|(pid, name)| format!("{name} ({pid})")).collect::<Vec<_>>().join(", ")
}

/// The pool state of one repository: its file and its lock in `$GIT_COMMON_DIR/branch-deck/`
pub struct Pool {
  dir: PathBuf,
}

impl Pool {
  /// The pool of the repository whose `$GIT_COMMON_DIR` is `git_common_dir`
  pub fn new(git_common_dir: &Path) -> Self {
    Self {
      dir: git_common_dir.join(STATE_DIR),
    }
  }

  /// The state, without the lock: the file is replaced by a rename, so a read sees one whole state. A missing file is an
  /// empty pool
  pub fn read(&self) -> Result<LeaseStore> {
    let path = self.dir.join(STATE_FILE);
    match std::fs::read(&path) {
      Ok(bytes) => serde_json::from_slice(&bytes).with_context(|| format!("Cannot read the lease state {}", path.display())),
      Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(LeaseStore::default()),
      Err(error) => Err(anyhow!(error).context(format!("Cannot read the lease state {}", path.display()))),
    }
  }

  /// Run `change` on the state under the pool lock and write the state it leaves; the lock is released at once
  pub fn update<T>(&self, timeout: Duration, change: impl FnOnce(&mut LeaseStore) -> Result<T>) -> Result<T> {
    let _lock = self.lock(timeout)?;
    let mut store = self.read()?;
    let before = store.clone();
    let result = change(&mut store)?;
    if store != before {
      self.write(&store)?;
    }
    Ok(result)
  }

  fn lock(&self, timeout: Duration) -> Result<RepositoryLock> {
    std::fs::create_dir_all(&self.dir).with_context(|| format!("Cannot create {}", self.dir.display()))?;
    lock_file(self.dir.join(LOCK_FILE), timeout, &|| debug!("waiting for the lease pool lock"))
  }

  /// A temporary file and a rename: a reader sees the old state or the new one
  fn write(&self, store: &LeaseStore) -> Result<()> {
    let path = self.dir.join(STATE_FILE);
    let temp = self.dir.join(format!("{STATE_FILE}.{}.tmp", std::process::id()));
    std::fs::write(&temp, serde_json::to_vec_pretty(store)?).with_context(|| format!("Cannot write {}", temp.display()))?;
    std::fs::rename(&temp, &path)
      .inspect_err(|_| {
        let _ = std::fs::remove_file(&temp);
      })
      .with_context(|| format!("Cannot write {}", path.display()))
  }
}

/// `path` canonical, as the state names a slot (`display_path`: no `\\?\` on Windows); `path` itself when it is gone
pub fn canonical_path(path: &Path) -> String {
  std::fs::canonicalize(path).map_or_else(|_| display_path(path), |path| display_path(&path))
}

/// The name of the repository: the directory of the main worktree (`idea-1` of `idea-1/.git`), or of a bare one without
/// `.git`. A slot's leaf directory keeps it, since tools derive the identity of a checkout from it
pub fn repository_name(git_common_dir: &Path) -> String {
  let leaf = git_common_dir.file_name().map_or_default(|name| name.to_string_lossy().into_owned());
  if leaf == ".git" {
    return git_common_dir
      .parent()
      .and_then(Path::file_name)
      .map_or_else(|| "repository".to_string(), |name| name.to_string_lossy().into_owned());
  }
  leaf.strip_suffix(".git").unwrap_or(&leaf).to_string()
}

/// The directory of the new slots: `branchdeck.leasePool` (a relative one under the repository), else
/// `~/.branch-deck/pool/<repository name>-<8 hex digits of the SHA-256 of the canonical common dir>`
pub fn pool_dir(git_executor: &GitCommandExecutor, repository: &str, git_common_dir: &Path, home: Option<&Path>) -> Result<PathBuf> {
  if let Some(configured) = lease_pool_config(git_executor, repository)? {
    return Ok(Path::new(repository).join(configured));
  }
  let home = home.ok_or_else(|| anyhow!("No home directory for the lease pool; set `git config branchdeck.leasePool <dir>`"))?;
  let digest = Sha256::digest(canonical_path(git_common_dir).as_bytes());
  let hash = digest.iter().take(4).fold(String::new(), |mut hash, byte| {
    let _ = write!(hash, "{byte:02x}");
    hash
  });
  Ok(home.join(".branch-deck").join("pool").join(format!("{}-{hash}", repository_name(git_common_dir))))
}

/// A slot a get passed over
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkippedSlot {
  pub name: String,
  pub path: String,
  pub reason: SkipReason,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkipReason {
  /// Local changes: the slot stays free and marked dirty, and the next gets try it last
  Dirty { paths: Vec<String> },
  /// Processes have their working directory in it
  Busy { processes: Vec<(u32, String)> },
}

pub struct LeaseGetParams<'a> {
  /// The checkout whose HEAD the slot is moved to; a leased slot itself may be the source
  pub source: &'a RepositoryDirs,
  /// The home directory of the default pool directory
  pub home: Option<&'a Path>,
  /// Default: `agent-<uuid>`
  pub holder: Option<String>,
  /// A new branch at the source HEAD, checked out in the slot
  pub branch: Option<String>,
  pub lock_timeout: Duration,
}

/// What a get leased
#[derive(Debug, Clone)]
pub struct LeaseGot {
  /// The slot with its `ready` lease
  pub slot: Slot,
  pub pool_dir: PathBuf,
  pub skipped: Vec<SkippedSlot>,
  /// Whether the processes were checked (`ProcessScan::Unavailable` on Windows)
  pub process_scan: bool,
}

/// The slot a get reserved under the pool lock
enum Reserved {
  Free { path: String, name: String },
  New { path: PathBuf },
}

/// Lease a slot at the HEAD of the source. Under the pool lock: a free slot of this repository (a linked worktree of the
/// same common dir, no process in it, clean slots before dirty ones), its lease recorded as `preparing`. Then, without
/// the lock: one `status` (a dirty slot is marked, skipped, and the next one tried), `checkout --force` to the source
/// HEAD (only the diff is written), the seed. No free slot: `worktree add`, the only full checkout. A failure after the
/// lease was recorded gives the slot back
#[instrument(skip_all, fields(source = %params.source.top_level))]
pub fn lease_get_core(git_executor: &GitCommandExecutor, params: &LeaseGetParams<'_>) -> Result<LeaseGot> {
  let source = &params.source.top_level;
  let git_common_dir = PathBuf::from(canonical_path(Path::new(&params.source.git_common_dir)));
  let Some(source_head) = resolve_commit(git_executor, source, "HEAD")? else {
    bail!(LeaseRefusal::NoCommits(source.clone()));
  };
  if let Some(branch) = &params.branch {
    if !is_valid_branch_name(git_executor, source, branch)? {
      bail!(LeaseRefusal::InvalidBranch(branch.clone()));
    }
    if ref_exists(git_executor, source, &format!("refs/heads/{branch}")) {
      bail!(LeaseRefusal::BranchExists(branch.clone()));
    }
  }
  let pool_dir = pool_dir(git_executor, source, &git_common_dir, params.home)?;
  std::fs::create_dir_all(&pool_dir).with_context(|| format!("Cannot create the pool directory {}", pool_dir.display()))?;
  let pool_dir = PathBuf::from(canonical_path(&pool_dir));
  let seeds = seed_paths(git_executor, source)?;
  let scan = ProcessScan::scan();
  let pool = Pool::new(&git_common_dir);
  let pid = std::process::id();
  let lease = Lease {
    id: uuid::Uuid::new_v4().to_string(),
    holder: params.holder.clone().unwrap_or_else(|| format!("agent-{}", uuid::Uuid::new_v4())),
    leased_at: SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| since.as_secs()),
    state: LeaseState::Preparing,
    pid,
    process_started: process_started(pid),
    source_head: source_head.clone(),
    branch: params.branch.clone(),
    seeded: Vec::new(),
    reused: false,
  };

  let mut skipped = Vec::new();
  let mut tried = HashSet::new();
  loop {
    let reserved = pool.update(params.lock_timeout, |store| {
      Ok(reserve(store, &git_common_dir, &pool_dir, &scan, &mut tried, &mut skipped, &lease))
    })?;
    let (path, reused) = match reserved {
      Reserved::Free { path, name } => {
        let changes = slot_changes(git_executor, &path).map_err(|error| give_back(&pool, params.lock_timeout, &path, &lease.id, false, error))?;
        if !changes.is_empty() {
          debug!(slot = %path, changes = changes.len(), "skipping a dirty slot");
          pool.update(params.lock_timeout, |store| {
            if let Some(slot) = leased_slot(store, &path, &lease.id) {
              slot.lease = None;
              slot.dirty = true;
            }
            Ok(())
          })?;
          skipped.push(SkippedSlot {
            name,
            path,
            reason: SkipReason::Dirty { paths: changes },
          });
          continue;
        }
        checkout_slot(git_executor, &path, &source_head, params.branch.as_deref()).map_err(|error| give_back(&pool, params.lock_timeout, &path, &lease.id, false, error))?;
        (path, true)
      }
      Reserved::New { path } => {
        let text = display_path(&path);
        let added = path
          .parent()
          .map_or(Ok(()), std::fs::create_dir_all)
          .map_err(anyhow::Error::from)
          .and_then(|()| add_slot(git_executor, source, &path, &source_head, params.branch.as_deref()));
        if let Err(error) = added {
          // the number directory, when the add left it empty
          if let Some(parent) = path.parent() {
            let _ = std::fs::remove_dir(parent);
          }
          return Err(give_back(&pool, params.lock_timeout, &text, &lease.id, true, error));
        }
        (text, false)
      }
    };
    let seeded = seed(Path::new(source), Path::new(&path), &seeds).map_err(|error| give_back(&pool, params.lock_timeout, &path, &lease.id, false, error))?;
    let name = linked_worktree(Path::new(&path)).map_or_default(|worktree| worktree.id);
    let slot = pool.update(params.lock_timeout, |store| {
      let slot = leased_slot(store, &path, &lease.id).ok_or_else(|| anyhow!("The lease of {path} was taken back while it was being prepared"))?;
      slot.name = name;
      slot.dirty = false;
      if let Some(lease) = slot.lease.as_mut() {
        lease.state = LeaseState::Ready;
        lease.seeded = seeded;
        lease.reused = reused;
      }
      Ok(slot.clone())
    })?;
    return Ok(LeaseGot {
      slot,
      pool_dir,
      skipped,
      process_scan: scan.is_available(),
    });
  }
}

/// Under the pool lock: free the leases whose get died while preparing, drop the free slots whose directory is gone, and
/// reserve a free slot (clean ones first, in the order they joined the pool) or a new one. A free slot with a process in
/// it is skipped as busy
fn reserve(
  store: &mut LeaseStore,
  git_common_dir: &Path,
  pool_dir: &Path,
  scan: &ProcessScan,
  tried: &mut HashSet<String>,
  skipped: &mut Vec<SkippedSlot>,
  lease: &Lease,
) -> Reserved {
  for slot in &mut store.slots {
    if slot.lease.as_ref().is_some_and(Lease::is_abandoned) {
      warn!(slot = %slot.path, "freeing a lease whose get is gone");
      slot.lease = None;
    }
  }
  store.slots.retain(|slot| slot.lease.is_some() || Path::new(&slot.path).is_dir());
  for dirty in [false, true] {
    for slot in store.slots.iter_mut().filter(|slot| slot.lease.is_none() && slot.dirty == dirty) {
      if tried.contains(&slot.path) || !is_worktree_of(Path::new(&slot.path), git_common_dir) {
        continue;
      }
      tried.insert(slot.path.clone());
      let processes = processes_of(&scan.inside(Path::new(&slot.path)));
      if !processes.is_empty() {
        skipped.push(SkippedSlot {
          name: slot.name.clone(),
          path: slot.path.clone(),
          reason: SkipReason::Busy { processes },
        });
        continue;
      }
      slot.lease = Some(lease.clone());
      return Reserved::Free {
        path: slot.path.clone(),
        name: slot.name.clone(),
      };
    }
  }
  let leaf = repository_name(git_common_dir);
  let path = (1..=u32::MAX)
    .map(|number| pool_dir.join(number.to_string()))
    .find(|dir| !dir.exists() && !store.slots.iter().any(|slot| Path::new(&slot.path).starts_with(dir)))
    .map_or_default(|dir| dir.join(&leaf));
  store.slots.push(Slot {
    name: String::new(),
    path: display_path(&path),
    dirty: false,
    lease: Some(lease.clone()),
  });
  Reserved::New { path }
}

/// Whether `dir` is the top level of a linked worktree whose common dir is `git_common_dir` (canonical)
fn is_worktree_of(dir: &Path, git_common_dir: &Path) -> bool {
  linked_worktree(dir).is_some_and(|worktree| display_path(&worktree.git_common_dir) == display_path(git_common_dir))
}

/// The slot at `path` whose lease is `lease_id`
fn leased_slot<'a>(store: &'a mut LeaseStore, path: &str, lease_id: &str) -> Option<&'a mut Slot> {
  store
    .slots
    .iter_mut()
    .find(|slot| slot.path == path && slot.lease.as_ref().is_some_and(|lease| lease.id == lease_id))
}

/// Give the slot of a failed get back: free it, or drop it from the state when it was never added (`remove`). The
/// error says so
fn give_back(pool: &Pool, timeout: Duration, path: &str, lease_id: &str, remove: bool, error: anyhow::Error) -> anyhow::Error {
  let released = pool.update(timeout, |store| {
    if remove {
      store
        .slots
        .retain(|slot| !(slot.path == path && slot.lease.as_ref().is_some_and(|lease| lease.id == lease_id)));
    } else if let Some(slot) = leased_slot(store, path, lease_id) {
      slot.lease = None;
    }
    Ok(())
  });
  match released {
    Ok(()) => error.context(format!("The lease was rolled back: {path} is free again")),
    Err(release_error) => error.context(format!("The lease of {path} could not be rolled back ({release_error:#}): the next get frees it")),
  }
}

/// `(pid, name)` of each process
fn processes_of(processes: &[&ProcessInfo]) -> Vec<(u32, String)> {
  processes.iter().map(|process| (process.pid, process.name.clone())).collect()
}

/// The ignored paths that `.worktreeinclude` at the root of `source` names (`seed_candidates`), one `check-ignore`: a
/// tracked or not ignored file is never seeded
fn seed_paths(git_executor: &GitCommandExecutor, source: &str) -> Result<Vec<String>> {
  let Ok(text) = std::fs::read_to_string(Path::new(source).join(WORKTREE_INCLUDE)) else {
    return Ok(Vec::new());
  };
  let candidates = seed_candidates(Path::new(source), &text);
  ignored_paths(git_executor, source, &candidates)
}

/// The paths under `root` that the lines of a `.worktreeinclude` name, without a walk of the tree: a literal path
/// (`/.claude/settings.local.json`, a file or a directory), or a glob of `*`, `?` and `[…]` in its last component
/// (`/config/*.local.json`), listed from that one directory. A pattern is always relative to the root, also without a
/// leading `/`: unlike in `.gitignore` it never matches at every depth. `#` comments, `!` negations, `**` and a glob in
/// a directory component are left out
pub fn seed_candidates(root: &Path, text: &str) -> Vec<String> {
  let mut paths = Vec::new();
  for line in text.lines() {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') || line.starts_with('!') {
      continue;
    }
    let pattern = line.trim_start_matches('/').trim_end_matches('/');
    if !is_relative_path(pattern) || pattern.contains("**") {
      continue;
    }
    let (dir, leaf) = pattern.rsplit_once('/').unwrap_or(("", pattern));
    if has_glob(dir) {
      continue;
    }
    if !has_glob(leaf) {
      if root.join(pattern).symlink_metadata().is_ok() {
        paths.push(pattern.to_string());
      }
      continue;
    }
    let Ok(entries) = std::fs::read_dir(root.join(dir)) else { continue };
    let mut names: Vec<String> = entries
      .flatten()
      .filter_map(|entry| entry.file_name().to_str().map(str::to_string))
      .filter(|name| name != ".git" && glob_matches(leaf, name))
      .collect();
    names.sort();
    paths.extend(names.into_iter().map(|name| if dir.is_empty() { name } else { format!("{dir}/{name}") }));
  }
  let mut seen = HashSet::new();
  paths.retain(|path| seen.insert(path.clone()));
  paths
}

/// A path of normal components only, none of them `.git`
fn is_relative_path(path: &str) -> bool {
  !path.is_empty() && !path.contains('\\') && Path::new(path).components().all(|component| matches!(component, Component::Normal(name) if name != ".git"))
}

fn has_glob(text: &str) -> bool {
  text.contains(['*', '?', '['])
}

/// Whether `name` matches the glob `pattern`: `*` any run of characters, `?` one, `[abc]`, `[a-z]` and `[!a]` (or
/// `[^a]`) one of a set; a `[` without its `]` is itself
pub fn glob_matches(pattern: &str, name: &str) -> bool {
  let pattern: Vec<char> = pattern.chars().collect();
  let name: Vec<char> = name.chars().collect();
  matches_from(&pattern, &name)
}

fn matches_from(pattern: &[char], name: &[char]) -> bool {
  match pattern.first() {
    None => name.is_empty(),
    Some('*') => (0..=name.len()).any(|skip| matches_from(&pattern[1..], &name[skip..])),
    Some('?') => !name.is_empty() && matches_from(&pattern[1..], &name[1..]),
    Some('[') => match (class_end(pattern), name.first()) {
      (Some(end), Some(&c)) => class_matches(&pattern[1..end], c) && matches_from(&pattern[end + 1..], &name[1..]),
      (Some(_), None) => false,
      (None, _) => name.first() == Some(&'[') && matches_from(&pattern[1..], &name[1..]),
    },
    Some(&c) => name.first() == Some(&c) && matches_from(&pattern[1..], &name[1..]),
  }
}

/// The index of the `]` that closes the class `pattern` starts with; a `]` right after `[` or `[!` belongs to the set
fn class_end(pattern: &[char]) -> Option<usize> {
  let mut index = 1;
  if matches!(pattern.get(index), Some('!' | '^')) {
    index += 1;
  }
  if pattern.get(index) == Some(&']') {
    index += 1;
  }
  pattern[index..].iter().position(|&c| c == ']').map(|offset| index + offset)
}

fn class_matches(class: &[char], c: char) -> bool {
  let (negated, class) = match class.first() {
    Some('!' | '^') => (true, &class[1..]),
    _ => (false, class),
  };
  let mut found = false;
  let mut index = 0;
  while index < class.len() {
    if class.get(index + 1) == Some(&'-') && index + 2 < class.len() {
      found |= (class[index]..=class[index + 2]).contains(&c);
      index += 3;
    } else {
      found |= class[index] == c;
      index += 1;
    }
  }
  found != negated
}

/// Copy each of `paths` from `source` into `slot`: a file, or a directory with its files and directories (symlinks are
/// left out); an existing file is replaced. Returns the paths copied
fn seed(source: &Path, slot: &Path, paths: &[String]) -> Result<Vec<String>> {
  let mut seeded = Vec::new();
  for path in paths {
    let from = source.join(path);
    let Ok(metadata) = from.symlink_metadata() else { continue };
    let to = slot.join(path);
    if metadata.is_dir() {
      copy_dir(&from, &to)?;
    } else if metadata.is_file() {
      if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent)?;
      }
      std::fs::copy(&from, &to).with_context(|| format!("Cannot copy {} to {}", from.display(), to.display()))?;
    } else {
      continue;
    }
    seeded.push(path.clone());
  }
  Ok(seeded)
}

fn copy_dir(from: &Path, to: &Path) -> Result<()> {
  std::fs::create_dir_all(to).with_context(|| format!("Cannot create {}", to.display()))?;
  for entry in std::fs::read_dir(from)?.flatten() {
    let file_type = entry.file_type()?;
    let target = to.join(entry.file_name());
    if file_type.is_dir() {
      copy_dir(&entry.path(), &target)?;
    } else if entry.metadata()?.is_file() {
      std::fs::copy(entry.path(), &target).with_context(|| format!("Cannot copy {} to {}", entry.path().display(), target.display()))?;
    }
  }
  Ok(())
}

/// Remove the seeded `paths` from `slot`, a directory with what is in it; a path that is gone already is no error.
/// Returns the paths removed
fn remove_seeded(slot: &Path, paths: &[String]) -> Result<Vec<String>> {
  let mut removed = Vec::new();
  for path in paths.iter().filter(|path| is_relative_path(path)) {
    let target = slot.join(path);
    let Ok(metadata) = target.symlink_metadata() else { continue };
    if metadata.is_dir() {
      std::fs::remove_dir_all(&target)
    } else {
      std::fs::remove_file(&target)
    }
    .with_context(|| format!("Cannot remove the seeded {}", target.display()))?;
    removed.push(path.clone());
  }
  Ok(removed)
}

pub struct LeaseReturnParams<'a> {
  /// The slot, as git names its worktree
  pub workspace: &'a RepositoryDirs,
  /// The lease the caller holds; another one refuses
  pub lease_id: Option<&'a str>,
  /// Give up the unlanded commits and discard the local changes
  pub confirm_preserved: bool,
  pub lock_timeout: Duration,
}

/// What a return did
#[derive(Debug, Clone)]
pub struct LeaseReturned {
  /// The slot with the lease it had
  pub slot: Slot,
  /// The local changes `--confirm-preserved` discarded
  pub discarded: Vec<String>,
  /// The unlanded commits `--confirm-preserved` gave up; they stay in the reflog of the slot's HEAD
  pub given_up: Vec<UnlandedCommit>,
  /// The seeded paths removed
  pub removed: Vec<String>,
  /// The branch the slot was on; it stays
  pub detached_branch: Option<String>,
  pub process_scan: bool,
}

/// Give the lease of a slot back. Refused while a process other than `bd` and its ancestors is in the slot, while HEAD
/// has commits that no branch, tag or remote branch holds (not counting those of the commit it was leased at), and
/// while the tree has local changes; `confirm_preserved`
/// gives up the commits and discards the changes (`reset --hard`, `clean -fd`: the ignored files stay). Then the seeded
/// paths go, a branch is detached (it stays) and the slot is free; it is not moved, the next get moves it
#[instrument(skip_all, fields(workspace = %params.workspace.top_level))]
pub fn lease_return_core(git_executor: &GitCommandExecutor, params: &LeaseReturnParams<'_>) -> Result<LeaseReturned> {
  let path = canonical_path(Path::new(&params.workspace.top_level));
  let pool = Pool::new(Path::new(&canonical_path(Path::new(&params.workspace.git_common_dir))));
  let store = pool.read()?;
  let (slot, lease) = store
    .slots
    .iter()
    .find_map(|slot| slot.lease.clone().filter(|_| slot.path == path).map(|lease| (slot.clone(), lease)))
    .ok_or_else(|| LeaseRefusal::NoLease(path.clone()))?;
  if let Some(expected) = params.lease_id
    && expected != lease.id
  {
    bail!(LeaseRefusal::LeaseMismatch {
      path,
      expected: expected.to_string(),
      actual: lease.id,
    });
  }
  if lease.state == LeaseState::Preparing && !lease.is_abandoned() {
    bail!(LeaseRefusal::Preparing { path, pid: lease.pid });
  }
  let scan = ProcessScan::scan();
  let processes = processes_of(&scan.inside(Path::new(&path)));
  if !processes.is_empty() {
    bail!(LeaseRefusal::SlotBusy { path, processes });
  }
  let unlanded = unlanded_commits(git_executor, &path, Some(&lease.source_head), UNLANDED_LIMIT)?;
  if !unlanded.is_empty() && !params.confirm_preserved {
    bail!(LeaseRefusal::UnlandedCommits { path, commits: unlanded });
  }
  let changes = slot_changes(git_executor, &path)?;
  if !changes.is_empty() {
    if !params.confirm_preserved {
      bail!(LeaseRefusal::DirtyWorktree { path, paths: changes });
    }
    discard_slot_changes(git_executor, &path)?;
  }
  let removed = remove_seeded(Path::new(&path), &lease.seeded)?;
  let detached_branch = read_head(git_executor, &path)?.and_then(|(_, branch)| branch);
  if detached_branch.is_some() {
    detach_slot_head(git_executor, &path)?;
  }
  pool.update(params.lock_timeout, |store| {
    let slot = leased_slot(store, &path, &lease.id).ok_or_else(|| anyhow!("The lease of {path} changed while it was returned"))?;
    slot.lease = None;
    slot.dirty = false;
    Ok(())
  })?;
  Ok(LeaseReturned {
    slot,
    discarded: changes,
    given_up: unlanded,
    removed,
    detached_branch,
    process_scan: scan.is_available(),
  })
}

/// How `bd lease status` sees a slot, from the state alone
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotState {
  Free,
  Preparing,
  Ready,
  /// A `preparing` lease whose get is gone: the next get takes the slot as free
  Abandoned,
}

/// A slot of `bd lease status`
#[derive(Debug, Clone)]
pub struct SlotStatus {
  pub slot: Slot,
  pub state: SlotState,
  /// The directory is gone
  pub missing: bool,
  /// With `processes`: the processes in it, except `bd` and its ancestors
  pub processes: Option<Vec<(u32, String)>>,
}

/// Every slot of the pool from the state, no git: with `processes`, the process scan of each slot too
pub fn lease_status_core(git_common_dir: &Path, processes: bool) -> Result<(Vec<SlotStatus>, bool)> {
  let store = Pool::new(Path::new(&canonical_path(git_common_dir))).read()?;
  let scan = if processes { ProcessScan::scan() } else { ProcessScan::Unavailable };
  let slots = store
    .slots
    .into_iter()
    .map(|slot| {
      let state = match &slot.lease {
        None => SlotState::Free,
        Some(lease) if lease.is_abandoned() => SlotState::Abandoned,
        Some(lease) if lease.state == LeaseState::Preparing => SlotState::Preparing,
        Some(_) => SlotState::Ready,
      };
      let missing = !Path::new(&slot.path).is_dir();
      let processes = processes.then(|| processes_of(&scan.inside(Path::new(&slot.path))));
      SlotStatus { slot, state, missing, processes }
    })
    .collect();
  Ok((slots, scan.is_available()))
}

/// A directory `bd lease adopt` added, or refused
#[derive(Debug, Clone)]
pub enum Adopted {
  Added(Slot),
  Refused { path: String, refusal: LeaseRefusal },
}

/// Add existing linked worktrees of this repository to the pool as free slots, where they are. Each is refused when it
/// is not a linked worktree of the same common dir, the pool holds it, a process is in it, or it has local changes (one
/// `status`)
pub fn lease_adopt_core(git_executor: &GitCommandExecutor, git_common_dir: &Path, dirs: &[PathBuf], lock_timeout: Duration) -> Result<(Vec<Adopted>, bool)> {
  let git_common_dir = PathBuf::from(canonical_path(git_common_dir));
  let pool = Pool::new(&git_common_dir);
  let scan = ProcessScan::scan();
  let mut outcomes = Vec::new();
  for dir in dirs {
    let path = canonical_path(dir);
    match adopt_one(git_executor, &pool, &git_common_dir, &path, &scan, lock_timeout)? {
      Ok(slot) => outcomes.push(Adopted::Added(slot)),
      Err(refusal) => outcomes.push(Adopted::Refused { path, refusal }),
    }
  }
  Ok((outcomes, scan.is_available()))
}

fn adopt_one(git_executor: &GitCommandExecutor, pool: &Pool, git_common_dir: &Path, path: &str, scan: &ProcessScan, lock_timeout: Duration) -> Result<Result<Slot, LeaseRefusal>> {
  let not_a_slot = |reason: &str| {
    Ok(Err(LeaseRefusal::NotASlot {
      path: path.to_string(),
      reason: reason.to_string(),
    }))
  };
  let Some(worktree) = linked_worktree(Path::new(path)) else {
    return not_a_slot("not the top level of a linked worktree");
  };
  if display_path(&worktree.git_common_dir) != display_path(git_common_dir) {
    return not_a_slot("a worktree of another repository");
  }
  if pool.read()?.slots.iter().any(|slot| slot.path == path) {
    return Ok(Err(LeaseRefusal::InPool(path.to_string())));
  }
  let processes = processes_of(&scan.inside(Path::new(path)));
  if !processes.is_empty() {
    return Ok(Err(LeaseRefusal::SlotBusy {
      path: path.to_string(),
      processes,
    }));
  }
  let changes = slot_changes(git_executor, path)?;
  if !changes.is_empty() {
    return Ok(Err(LeaseRefusal::DirtyCandidate {
      path: path.to_string(),
      paths: changes,
    }));
  }
  pool.update(lock_timeout, |store| {
    if store.slots.iter().any(|slot| slot.path == path) {
      return Ok(Err(LeaseRefusal::InPool(path.to_string())));
    }
    let slot = Slot {
      name: worktree.id,
      path: path.to_string(),
      dirty: false,
      lease: None,
    };
    store.slots.push(slot.clone());
    Ok(Ok(slot))
  })
}

#[cfg(test)]
#[path = "lease_test.rs"]
mod lease_test;
