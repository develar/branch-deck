//! The lease pool of a repository (`bd lease`, spec/cli/lease.spec.md, "Lease" in docs/design/git-operations.md): linked
//! worktrees that agent sessions lease, one session per slot. The state in `$GIT_COMMON_DIR/branch-deck/leases.json` is
//! the authority: a free slot is pool property, and a get replaces its content, so no ancestry proves anything. The
//! pool has a lock of its own (`leases.lock`), held only while the state is read and written, never across a git that
//! writes a slot: two gets never wait for each other's checkouts, and no get waits for an update.

use crate::clone_dir::clone_directory;
use crate::lease_clean::{CleanOverrides, Cleaned, apply_clean_rules, read_clean_rules};
use anyhow::{Context, Result, anyhow, bail};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::lease_slot::{
  CheckoutWorkers, LeaseConfig, UnlandedCommit, UntrackedFiles, add_slot, checkout_slot, clean_cloned_slot, detach_slot_head, discard_slot_changes, ignored_paths,
  is_valid_branch_name, lease_config, linked_worktree, move_slot, refresh_cloned_index, register_slot, slot_changes, unlanded_commits, unlock_slot,
};
use git_ops::pick::commits_to_pick;
use git_ops::revs::{RepositoryDirs, read_head, ref_exists};
use history_rewrite::repository_lock::{RepositoryLock, lock_file};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fmt::Write;
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use sync_core::baseline::baseline_tracking_ref;
use sync_utils::paths::display_path;
use sync_utils::process_scan::{ProcessInfo, ProcessScan, is_running, process_started, terminate};
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
  /// The number of its last return in the pool, higher for a later one: a get takes the free slot returned last first
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub returned: Option<u64>,
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
  /// The process of the get, and when it started (`process_started`); of the return while `returning`, of the clean
  /// while `cleaning`: a lease that is not `ready` and whose process is gone is free
  pub pid: u32,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub process_started: Option<u64>,
  /// The commit of the source that the slot was moved to
  pub source_head: String,
  /// The source checkout of the get, its top level
  #[serde(default, skip_serializing_if = "String::is_empty")]
  pub source: String,
  /// The branch checked out in the source at the get; `None` for a detached HEAD. The return asks whether this branch
  /// has every change of the slot (`commits_to_pick`)
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub source_branch: Option<String>,
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
  /// A free slot that a get clones into a new slot: no get takes it until the clone ends
  Templating,
  /// The return of the lease runs (`lease_return_core`): no get takes the slot and no other return acts on it
  Returning,
  /// A free slot that `bd lease clean` cleans: no get takes it until the clean ends
  Cleaning,
}

impl Lease {
  /// Whether the lease is not `ready` (`preparing`, `templating`, `returning`, `cleaning`) and its process is gone: the
  /// next get takes the slot as free
  pub fn is_abandoned(&self) -> bool {
    self.state != LeaseState::Ready && !is_running(self.pid, self.process_started)
  }
}

/// What the pool refuses; `bd` gives each its error code
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LeaseRefusal {
  #[error("{0} holds no lease of this repository's pool")]
  NoLease(String),
  #[error("The slot {path} is being prepared or cloned by the process {pid}")]
  Preparing { path: String, pid: u32 },
  #[error("The lease of {path} is being returned by the process {pid}")]
  Returning { path: String, pid: u32 },
  #[error("The lease of {path} is {actual}, not {expected}")]
  LeaseMismatch { path: String, expected: String, actual: String },
  #[error("Processes work in {path}: {}; stop them, or wait until they are done", describe_processes(.processes))]
  SlotBusy { path: String, processes: Vec<(u32, String)> },
  #[error(
    "HEAD of {path} has {} commit(s) that {}; land them first (`bd pick --from {path} --follow` in the shared checkout), or pass --confirm-preserved to give them up",
    .commits.len(),
    .check.describe()
  )]
  UnlandedCommits { path: String, commits: Vec<UnlandedCommit>, check: LandedCheck },
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
  #[error("{path} is on another volume than the pool {pool}: a move would copy every file")]
  OtherVolume { path: String, pool: String },
  #[error("Cannot move {path} into the pool: {reason}")]
  CannotMove { path: String, reason: String },
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

/// The directory of the new slots: `branchdeck.leasePool` of `config` (a relative one under the repository), else
/// `~/.branch-deck/pool/<repository name>-<8 hex digits of the SHA-256 of the canonical common dir>`
pub fn pool_dir(config: &LeaseConfig, repository: &str, git_common_dir: &Path, home: Option<&Path>) -> Result<PathBuf> {
  if let Some(configured) = &config.pool {
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

/// How a get added a new slot
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreatedBy {
  /// An APFS clone of a template (`clone_dir`), then the checkout of the diff
  Clone,
  /// `git worktree add`: a full checkout
  Checkout,
}

/// What a get leased
#[derive(Debug, Clone)]
pub struct LeaseGot {
  /// The slot with its `ready` lease
  pub slot: Slot,
  /// How a new slot was added; `None` for a reused one
  pub created_by: Option<CreatedBy>,
  /// The free slot or the source checkout a new slot was cloned from
  pub cloned_from: Option<String>,
  pub pool_dir: PathBuf,
  pub skipped: Vec<SkippedSlot>,
  /// Whether the processes were checked (`ProcessScan::Unavailable` on Windows)
  pub process_scan: bool,
}

/// The slot a get reserved under the pool lock
enum Reserved {
  Free {
    path: String,
    name: String,
  },
  /// A new slot at `path`; `template` is a free slot marked `templating` for its clone
  New {
    path: PathBuf,
    template: Option<String>,
  },
}

/// Lease a slot at the HEAD of the source. Under the pool lock: a free slot of this repository (a linked worktree of the
/// same common dir, no process in it, clean slots before dirty ones), its lease recorded as `preparing`. Then, without
/// the lock: one `status` (a dirty slot is marked, skipped, and the next one tried), `checkout --force` to the source
/// HEAD (only the diff is written), the seed. No free slot: a clone of a template (`add_cloned_slot`), else
/// `worktree add`, the only full checkout. A failure after the lease was recorded gives the slot back
#[instrument(skip_all, fields(source = %params.source.top_level))]
pub fn lease_get_core(git_executor: &GitCommandExecutor, params: &LeaseGetParams<'_>) -> Result<LeaseGot> {
  let source = &params.source.top_level;
  let git_common_dir = PathBuf::from(canonical_path(Path::new(&params.source.git_common_dir)));
  let Some((source_head, source_branch)) = read_head(git_executor, source)? else {
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
  let config = lease_config(git_executor, source)?;
  let pool_dir = pool_dir(&config, source, &git_common_dir, params.home)?;
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
    source: source.clone(),
    source_branch,
    branch: params.branch.clone(),
    seeded: Vec::new(),
    reused: false,
  };

  let mut skipped = Vec::new();
  let mut tried = HashSet::new();
  let mut created_by = None;
  let mut cloned_from = None;
  loop {
    let reserved = pool.update(params.lock_timeout, |store| {
      Ok(reserve(store, &git_common_dir, &pool_dir, &scan, &mut tried, &mut skipped, &lease))
    })?;
    let (path, reused) = match reserved {
      Reserved::Free { path, name } => {
        let changes = slot_changes(git_executor, &path, config.untracked).map_err(|error| give_back(&pool, params.lock_timeout, &path, &lease.id, false, error))?;
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
        checkout_slot(git_executor, &path, &source_head, params.branch.as_deref(), config.workers)
          .map_err(|error| give_back(&pool, params.lock_timeout, &path, &lease.id, false, error))?;
        (path, true)
      }
      Reserved::New { path, template } => {
        let text = display_path(&path);
        let template_dir = template.clone().unwrap_or_else(|| source.clone());
        let release_template = || {
          if let Some(template) = &template {
            release_marker(&pool, params.lock_timeout, template, &template_marker(&lease.id));
          }
        };
        let request = NewSlot {
          path: &path,
          source: params.source,
          template: &template_dir,
          commit: &source_head,
          branch: params.branch.as_deref(),
          workers: config.workers,
          untracked: config.untracked,
        };
        let added = path
          .parent()
          .map_or(Ok(()), std::fs::create_dir_all)
          .map_err(|error| Added::NotRegistered(error.into()))
          .and_then(|()| add_new_slot(git_executor, &request, &release_template));
        // a clone that failed early releases the template itself; this one is for any later error
        release_template();
        match added {
          Ok(how) => {
            created_by = Some(how);
            if how == CreatedBy::Clone {
              cloned_from = Some(template_dir);
            }
          }
          Err(Added::NotRegistered(error)) => {
            // the number directory, when the add left it empty
            if let Some(parent) = path.parent() {
              let _ = std::fs::remove_dir(parent);
            }
            return Err(give_back(&pool, params.lock_timeout, &text, &lease.id, true, error));
          }
          Err(Added::Registered(error)) => return Err(give_back(&pool, params.lock_timeout, &text, &lease.id, false, error)),
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
      created_by,
      cloned_from,
      pool_dir,
      skipped,
      process_scan: scan.is_available(),
    });
  }
}

/// Under the pool lock: free the leases whose get died while preparing, drop the free slots whose directory is gone, and
/// reserve a free slot (clean ones first, the one returned last first, then in the order they joined the pool) or a new
/// one. The slot returned last has the warmest index, fsmonitor daemon and build caches and the commit closest to the
/// source; the order they joined left the other slots idle for days, cold and far behind. A free slot with a process
/// in it is skipped as busy
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
    let mut order: Vec<usize> = (0..store.slots.len())
      .filter(|&index| store.slots[index].lease.is_none() && store.slots[index].dirty == dirty)
      .collect();
    // stable: the slots never returned come last, in the order they joined
    order.sort_by_key(|&index| std::cmp::Reverse(store.slots[index].returned));
    for index in order {
      let slot = &mut store.slots[index];
      if tried.contains(&slot.path) || !is_worktree_of(Path::new(&slot.path), git_common_dir) {
        continue;
      }
      tried.insert(slot.path.clone());
      // git's background maintenance writes nothing in the slot: no reason to skip it
      let maintenance = scan.git_maintenance();
      let inside = scan.inside(Path::new(&slot.path));
      let processes = processes_of(&inside.into_iter().filter(|process| !maintenance.contains(&process.pid)).collect::<Vec<_>>());
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
  let template = CLONE_AVAILABLE.then(|| mark_template(store, git_common_dir, scan, lease)).flatten();
  let path = next_slot_path(store, pool_dir, git_common_dir);
  store.slots.push(Slot {
    name: String::new(),
    path: display_path(&path),
    dirty: false,
    returned: None,
    lease: Some(lease.clone()),
  });
  Reserved::New { path, template }
}

/// The path of the next new slot: `<pool>/<n>/<repository name>`, the smallest `n` whose directory does not exist and
/// holds no slot of the state
fn next_slot_path(store: &LeaseStore, pool_dir: &Path, git_common_dir: &Path) -> PathBuf {
  let leaf = repository_name(git_common_dir);
  (1..=u32::MAX)
    .map(|number| pool_dir.join(number.to_string()))
    .find(|dir| !dir.exists() && !store.slots.iter().any(|slot| Path::new(&slot.path).starts_with(dir)))
    .map_or_default(|dir| dir.join(&leaf))
}

/// Whether a new slot can be a clone: APFS `clonefile(2)` is macOS only
const CLONE_AVAILABLE: bool = cfg!(target_os = "macos");

/// The id of the `templating` marker of the free slot that the get of `lease_id` clones
fn template_marker(lease_id: &str) -> String {
  format!("{lease_id}-template")
}

/// Under the pool lock: mark a free slot of this repository `templating` for the clone of a new slot, one without a
/// process in it first; `None` without one, and the source checkout is the template. The free slots left here are the
/// ones the get passed over (dirty or busy): the checkout and the clean of the clone make them right
fn mark_template(store: &mut LeaseStore, git_common_dir: &Path, scan: &ProcessScan, lease: &Lease) -> Option<String> {
  let candidates: Vec<usize> = (0..store.slots.len())
    .filter(|&index| store.slots[index].lease.is_none() && is_worktree_of(Path::new(&store.slots[index].path), git_common_dir))
    .collect();
  let index = candidates
    .iter()
    .copied()
    .find(|&index| scan.inside(Path::new(&store.slots[index].path)).is_empty())
    .or_else(|| candidates.first().copied())?;
  let slot = &mut store.slots[index];
  slot.lease = Some(Lease {
    id: template_marker(&lease.id),
    holder: format!("template of {}", lease.id),
    state: LeaseState::Templating,
    branch: None,
    seeded: Vec::new(),
    ..lease.clone()
  });
  Some(slot.path.clone())
}

/// Free the template `path` of its marker `marker_id`, best effort: an abandoned marker is freed by the next get
fn release_marker(pool: &Pool, timeout: Duration, path: &str, marker_id: &str) {
  let released = pool.update(timeout, |store| {
    if let Some(slot) = leased_slot(store, path, marker_id) {
      slot.lease = None;
    }
    Ok(())
  });
  if let Err(error) = released {
    warn!(template = %path, error = %error, "cannot release the template");
  }
}

/// A new slot to add
struct NewSlot<'a> {
  path: &'a Path,
  source: &'a RepositoryDirs,
  /// A free slot or the source checkout
  template: &'a str,
  commit: &'a str,
  branch: Option<&'a str>,
  workers: CheckoutWorkers,
  untracked: UntrackedFiles,
}

/// Why a new slot failed
enum Added {
  /// Nothing was registered: the slot leaves the state
  NotRegistered(anyhow::Error),
  /// git knows the worktree: the slot stays as a free one, and the next get checks it
  Registered(anyhow::Error),
}

/// Add the new slot of `request`: a clone of its template when the volume clones (`add_cloned_slot`), else `worktree
/// add` with the parallel checkout
fn add_new_slot(git_executor: &GitCommandExecutor, request: &NewSlot<'_>, release_template: &dyn Fn()) -> Result<CreatedBy, Added> {
  if CLONE_AVAILABLE {
    match add_cloned_slot(git_executor, request, release_template) {
      Ok(()) => return Ok(CreatedBy::Clone),
      Err(Cloned::Unavailable(error)) => debug!(template = %request.template, error = %error, "no clone, checking out instead"),
      Err(Cloned::Failed(error)) => return Err(error),
    }
  }
  add_slot(git_executor, &request.source.top_level, request.path, request.commit, request.branch, request.workers).map_err(Added::NotRegistered)?;
  Ok(CreatedBy::Checkout)
}

/// Why a clone failed
enum Cloned {
  /// `clonefile(2)` refused (another volume, no APFS, an unreadable file): check out instead
  Unavailable(std::io::Error),
  Failed(Added),
}

/// A new slot as an APFS clone of `request.template`, then the diff to `request.commit`:
/// 1. `clonefile(2)` of the template to `<path>.cloning`, then the template is released;
/// 2. `worktree add --no-checkout`, its `.git` file moved over the cloned `.git` (a directory when the template is a
///    main checkout), `<path>.cloning` renamed to `<path>`;
/// 3. the template's index copied in and refreshed (`refresh_cloned_index`);
/// 4. `checkout --force` to the commit, `clean -fd`, one `status`: the changes and the untracked files of the
///    template never reach the lease, and its ignored files stay as the warm cache
fn add_cloned_slot(git_executor: &GitCommandExecutor, request: &NewSlot<'_>, release_template: &dyn Fn()) -> Result<(), Cloned> {
  let path = request.path;
  let cloning = PathBuf::from(format!("{}.cloning", path.display()));
  let started = std::time::Instant::now();
  let cloned = clone_directory(Path::new(request.template), &cloning);
  release_template();
  if let Err(error) = cloned {
    let _ = std::fs::remove_dir_all(&cloning);
    return Err(Cloned::Unavailable(error));
  }
  debug!(template = %request.template, elapsed_ms = started.elapsed().as_millis(), "cloned the template");
  let failed = |error: anyhow::Error| {
    let _ = std::fs::remove_dir_all(&cloning);
    Cloned::Failed(Added::NotRegistered(error))
  };
  register_slot(git_executor, &request.source.top_level, path, request.commit).map_err(failed)?;
  let registered = |error: anyhow::Error| Cloned::Failed(Added::Registered(error));
  let swapped = swap_git_link(path, &cloning);
  // the lock of the registration kept the `worktree prune` of a get beside this one from deleting the admin directory
  // while `path` was away; a slot left as a free one must be prunable again once its directory is gone
  let unlocked = unlock_slot(git_executor, &request.source.top_level, path);
  swapped.map_err(|error| {
    let _ = std::fs::remove_dir_all(&cloning);
    registered(error)
  })?;
  unlocked.map_err(registered)?;
  let slot = display_path(path);
  let worktree = linked_worktree(path).ok_or_else(|| registered(anyhow!("{slot} is not a linked worktree after its clone")))?;
  let template_git_dir = if request.template == request.source.top_level {
    PathBuf::from(&request.source.git_dir)
  } else {
    linked_worktree(Path::new(request.template)).map_or_else(|| PathBuf::from(&request.source.git_dir), |template| template.git_dir)
  };
  // without the index of the template the checkout writes every file: slow, still right
  match std::fs::copy(template_git_dir.join("index"), worktree.git_dir.join("index")) {
    Ok(_) => refresh_cloned_index(git_executor, &slot).map_err(registered)?,
    Err(error) => warn!(template = %request.template, error = %error, "no index of the template to copy"),
  }
  debug!(elapsed_ms = started.elapsed().as_millis(), "refreshed the index of the clone");
  checkout_slot(git_executor, &slot, request.commit, request.branch, request.workers).map_err(registered)?;
  clean_cloned_slot(git_executor, &slot).map_err(registered)?;
  let changes = slot_changes(git_executor, &slot, request.untracked).map_err(registered)?;
  if !changes.is_empty() {
    return Err(registered(anyhow!(
      "The clone of {} kept {} local change(s), e.g. {}",
      request.template,
      changes.len(),
      changes[0]
    )));
  }
  debug!(elapsed_ms = started.elapsed().as_millis(), "checked out the clone");
  Ok(())
}

/// Give the clone at `cloning` the `.git` file that `worktree add --no-checkout` wrote at `path`, then move the clone to
/// `path`. The cloned `.git` of the template goes first: a file of a linked worktree, or the whole git directory of a
/// main checkout (its packs are clones too, so the removal is cheap: 0.2 s for the 2,079 files of idea-1)
fn swap_git_link(path: &Path, cloning: &Path) -> Result<()> {
  let cloned_git = cloning.join(".git");
  match cloned_git.symlink_metadata() {
    Ok(metadata) if metadata.is_dir() => std::fs::remove_dir_all(&cloned_git)?,
    Ok(_) => std::fs::remove_file(&cloned_git)?,
    Err(_) => {}
  }
  std::fs::rename(path.join(".git"), &cloned_git).with_context(|| format!("Cannot move the .git file of {}", path.display()))?;
  std::fs::remove_dir(path).with_context(|| format!("Cannot remove the empty {}", path.display()))?;
  std::fs::rename(cloning, path).with_context(|| format!("Cannot move the clone to {}", path.display()))?;
  Ok(())
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
  /// Refuse while a Bazel server of the slot runs, instead of stopping it
  pub keep_servers: bool,
  /// How long the return waits for the Bazel servers it asked to exit
  pub server_timeout: Duration,
  /// How long the return waits for git's background maintenance (`git maintenance`, `git gc --auto`) to leave the slot
  pub maintenance_timeout: Duration,
  /// Apply the clean rules of `.branchdeck.toml`, with these overrides; `None` (`--no-clean`) applies none
  pub clean: Option<CleanOverrides>,
  pub lock_timeout: Duration,
}

/// Which check of a return decided what is unlanded
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LandedCheck {
  /// No branch, tag or remote branch lacks a commit of HEAD (beyond the leased commit); or, with `reason`, the check by
  /// content could not run
  Reachability { reason: Option<String> },
  /// The dry run of a pick onto the branch of the source (`commits_to_pick`): what it would pick is unlanded
  Content { branch: String },
}

impl LandedCheck {
  /// The words of the refusal
  fn describe(&self) -> String {
    match self {
      Self::Reachability { reason: None } => "no branch, tag or remote branch holds".to_string(),
      Self::Reachability { reason: Some(reason) } => format!("no branch, tag or remote branch holds (no check by content: {reason})"),
      Self::Content { branch } => format!("{branch} lacks by content, as `bd pick --dry-run` would pick them"),
    }
  }
}

/// The unlanded commits of the slot at `path`. The fast path: `unlanded_commits`, reachability; nothing found is
/// landed. Else, when the lease recorded the branch of its source and the branch is there, the dry run of a pick onto
/// it decides: a rewrite of the branch (`bd update`, a fold) leaves the commits of a follow unreachable, yet present by
/// content. Without the branch, or when the pick can't tell (`range_unknown`, a merge), reachability decides
fn unlanded_work(git_executor: &GitCommandExecutor, path: &str, lease: &Lease) -> Result<(Vec<UnlandedCommit>, LandedCheck)> {
  let reachable = unlanded_commits(git_executor, path, Some(&lease.source_head), UNLANDED_LIMIT)?;
  let reachability = |reason: Option<String>| LandedCheck::Reachability { reason };
  if reachable.is_empty() {
    return Ok((reachable, reachability(None)));
  }
  let Some(branch) = &lease.source_branch else {
    return Ok((reachable, reachability(Some("the source had a detached HEAD".to_string()))));
  };
  if !ref_exists(git_executor, path, &format!("refs/heads/{branch}")) {
    return Ok((reachable, reachability(Some(format!("the branch {branch} is gone")))));
  }
  let upstream = baseline_tracking_ref(git_executor, path).ok().and_then(|(_, upstream)| upstream);
  match commits_to_pick(git_executor, path, branch, path, upstream.as_deref()) {
    Ok(commits) => {
      let mut unlanded: Vec<UnlandedCommit> = commits
        .into_iter()
        .rev()
        .map(|commit| UnlandedCommit {
          id: commit.id,
          subject: commit.subject,
        })
        .collect();
      unlanded.truncate(UNLANDED_LIMIT);
      Ok((unlanded, LandedCheck::Content { branch: branch.clone() }))
    }
    Err(error) => {
      debug!(slot = %path, error = %format!("{error:#}"), "no check by content");
      Ok((reachable, reachability(Some(format!("{error:#}")))))
    }
  }
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
  /// Which check decided what is unlanded; none for a return that joined another before it checked anything
  pub landed_check: Option<LandedCheck>,
  /// Another return of the same lease (a retry) returned it: this one changed nothing, and its lists are empty
  pub joined: bool,
  /// The Bazel servers of the slot that exited on the SIGTERM of the return
  pub stopped_servers: Vec<StoppedServer>,
  /// The seeded paths removed
  pub removed: Vec<String>,
  /// The branch the slot was on; it stays
  pub detached_branch: Option<String>,
  /// What each clean rule of `.branchdeck.toml` did
  pub cleaned: Vec<Cleaned>,
  /// Why the rules could not be read; the return succeeded anyway
  pub clean_error: Option<String>,
  pub process_scan: bool,
}

/// Give the lease of a slot back. Refused while a process other than `bd` and its ancestors is in the slot, while HEAD
/// has commits that no branch, tag or remote branch holds (not counting those of the commit it was leased at), and
/// while the tree has local changes; `confirm_preserved`
/// gives up the commits and discards the changes (`reset --hard`, `clean -fd`: the ignored files stay). Then the seeded
/// paths go, a branch is detached (it stays) and the slot is free; it is not moved, the next get moves it
#[instrument(skip_all, fields(workspace = %params.workspace.top_level))]
///
/// A repeat joins: while another return of the same lease runs (a retry after a timeout), this one waits for it and
/// answers `joined` once it freed the slot, or returns the lease itself once it failed; a return that another return
/// of its lease overtook answers `joined` too and changes nothing
pub fn lease_return_core(git_executor: &GitCommandExecutor, params: &LeaseReturnParams<'_>) -> Result<LeaseReturned> {
  let path = canonical_path(Path::new(&params.workspace.top_level));
  let pool = Pool::new(Path::new(&canonical_path(Path::new(&params.workspace.git_common_dir))));
  // the slot with the lease of the return this one waited for
  let mut waited_for: Option<Slot> = None;
  loop {
    let mut checked = None;
    let step = match return_once(git_executor, params, &pool, &path, waited_for.as_ref(), &mut checked) {
      Ok(step) => step,
      // a check refused the slot after another return of the lease gave it back (a get may have leased it again):
      // the refusal is of the next lease, the lease of this return was returned
      Err(error) => match checked.filter(|slot| !holds_lease(&pool, slot).unwrap_or(true)) {
        Some(slot) => return Ok(joined_return(slot, None, false)),
        None => return Err(error),
      },
    };
    match step {
      ReturnStep::Done(returned) => return Ok(*returned),
      ReturnStep::WaitFor(slot) => {
        if let Some(lease) = &slot.lease {
          wait_while_returning(&pool, &path, &lease.id)?;
        }
        waited_for = Some(*slot);
      }
    }
  }
}

/// What one pass of a return ends with
enum ReturnStep {
  Done(Box<LeaseReturned>),
  /// Another return of the lease runs: wait for it, then look again
  WaitFor(Box<Slot>),
}

/// The answer of a return whose lease another return of it gave back: nothing changed
const fn joined_return(slot: Slot, landed_check: Option<LandedCheck>, process_scan: bool) -> LeaseReturned {
  LeaseReturned {
    slot,
    discarded: Vec::new(),
    given_up: Vec::new(),
    landed_check,
    joined: true,
    stopped_servers: Vec::new(),
    removed: Vec::new(),
    detached_branch: None,
    cleaned: Vec::new(),
    clean_error: None,
    process_scan,
  }
}

/// Whether the slot still has the lease of `slot`
fn holds_lease(pool: &Pool, slot: &Slot) -> Result<bool> {
  let store = pool.read()?;
  Ok(
    store
      .slots
      .iter()
      .any(|current| current.path == slot.path && current.lease.as_ref().map(|lease| &lease.id) == slot.lease.as_ref().map(|lease| &lease.id)),
  )
}

/// Until the return of `lease_id` in `path` ends: it freed the slot, it failed (`ready` again) or its process died.
/// No lock is held: the pool is only read
fn wait_while_returning(pool: &Pool, path: &str, lease_id: &str) -> Result<()> {
  loop {
    let store = pool.read()?;
    let returning = store
      .slots
      .iter()
      .filter(|slot| slot.path == path)
      .filter_map(|slot| slot.lease.as_ref())
      .any(|lease| lease.id == lease_id && lease.state == LeaseState::Returning && !lease.is_abandoned());
    if !returning {
      return Ok(());
    }
    std::thread::sleep(Duration::from_millis(20));
  }
}

/// One pass of a return; `checked` is the slot with the lease it checks, once no other return of it runs
fn return_once(
  git_executor: &GitCommandExecutor,
  params: &LeaseReturnParams<'_>,
  pool: &Pool,
  path: &str,
  waited_for: Option<&Slot>,
  checked: &mut Option<Slot>,
) -> Result<ReturnStep> {
  let path = path.to_string();
  let store = pool.read()?;
  let current = store
    .slots
    .iter()
    .find_map(|slot| slot.lease.clone().filter(|_| slot.path == path).map(|lease| (slot.clone(), lease)));
  // the return it waited for freed the slot (a get may have leased it again): that return was this one's
  if let Some(waited) = waited_for
    && let Some(waited_lease) = &waited.lease
    && current.as_ref().is_none_or(|(_, lease)| lease.id != waited_lease.id)
  {
    return Ok(ReturnStep::Done(Box::new(joined_return(waited.clone(), None, false))));
  }
  let (slot, lease) = current.ok_or_else(|| LeaseRefusal::NoLease(path.clone()))?;
  if let Some(expected) = params.lease_id
    && expected != lease.id
  {
    bail!(LeaseRefusal::LeaseMismatch {
      path,
      expected: expected.to_string(),
      actual: lease.id,
    });
  }
  match refusal_to_return(&path, &lease) {
    Some(LeaseRefusal::Returning { .. }) => return Ok(ReturnStep::WaitFor(Box::new(slot))),
    Some(refusal) => bail!(refusal),
    None => {}
  }
  *checked = Some(slot.clone());
  let scan = wait_for_git_maintenance(Path::new(&path), params.maintenance_timeout);
  let inside = scan.inside(Path::new(&path));
  // the Bazel servers of the slot, which serve only it; every other process keeps the slot busy
  let (servers, others): (Vec<&ProcessInfo>, Vec<&ProcessInfo>) = inside.iter().partition(|process| !params.keep_servers && process.bazel_workspace().is_some());
  if !others.is_empty() {
    bail!(LeaseRefusal::SlotBusy {
      path,
      processes: processes_of(&inside),
    });
  }
  let (unlanded, landed_check) = unlanded_work(git_executor, &path, &lease)?;
  if !unlanded.is_empty() && !params.confirm_preserved {
    bail!(LeaseRefusal::UnlandedCommits {
      path,
      commits: unlanded,
      check: landed_check,
    });
  }
  let changes = slot_changes(git_executor, &path, lease_config(git_executor, &path)?.untracked)?;
  if !changes.is_empty() && !params.confirm_preserved {
    bail!(LeaseRefusal::DirtyWorktree { path, paths: changes });
  }
  // only now, when nothing else refuses: a server that does not exit keeps the slot busy
  let stopped_servers = stop_servers(&servers, params.server_timeout);
  let running: Vec<(u32, String)> = servers
    .iter()
    .filter(|server| !stopped_servers.iter().any(|stopped| stopped.pid == server.pid))
    .map(|server| (server.pid, server.name.clone()))
    .collect();
  if !running.is_empty() {
    bail!(LeaseRefusal::SlotBusy { path, processes: running });
  }
  // from here on the return acts on the slot: reserved under the pool lock, while the lease is still the one it
  // checked, so a retried return beside it, or one that another return overtook and whose slot a get leased meanwhile,
  // changes nothing
  match reserve_return(pool, params.lock_timeout, &path, &lease.id)? {
    Reservation::Reserved => {}
    Reservation::Returning => return Ok(ReturnStep::WaitFor(Box::new(slot))),
    Reservation::Returned => return Ok(ReturnStep::Done(Box::new(joined_return(slot, Some(landed_check), scan.is_available())))),
  }
  let returned = finish_return(git_executor, &path, &lease, &changes, params.clean);
  let FinishedReturn {
    removed,
    detached_branch,
    cleaned,
    clean_error,
  } = match returned {
    Ok(finished) => finished,
    Err(error) => {
      // the lease stays the holder's, `ready` again, for a return once the error is fixed
      let restored = pool.update(params.lock_timeout, |store| {
        if let Some(lease) = leased_slot(store, &path, &lease.id).and_then(|slot| slot.lease.as_mut()) {
          lease.state = LeaseState::Ready;
        }
        Ok(())
      });
      if let Err(restore_error) = restored {
        warn!(slot = %path, error = %format!("{restore_error:#}"), "cannot give the lease back after a failed return");
      }
      return Err(error);
    }
  };
  pool.update(params.lock_timeout, |store| {
    let last_return = store.slots.iter().filter_map(|slot| slot.returned).max().unwrap_or(0);
    let slot = leased_slot(store, &path, &lease.id).ok_or_else(|| anyhow!("The lease of {path} changed while it was returned"))?;
    slot.lease = None;
    slot.dirty = false;
    slot.returned = Some(last_return + 1);
    Ok(())
  })?;
  Ok(ReturnStep::Done(Box::new(LeaseReturned {
    slot,
    discarded: changes,
    given_up: unlanded,
    landed_check: Some(landed_check),
    joined: false,
    stopped_servers,
    removed,
    detached_branch,
    cleaned,
    clean_error,
    process_scan: scan.is_available(),
  })))
}

/// Why `lease` of the slot `path` can't be returned now: its get or another return of it runs
fn refusal_to_return(path: &str, lease: &Lease) -> Option<LeaseRefusal> {
  if lease.state == LeaseState::Ready || lease.is_abandoned() {
    return None;
  }
  let (path, pid) = (path.to_string(), lease.pid);
  Some(match lease.state {
    LeaseState::Returning => LeaseRefusal::Returning { path, pid },
    _ => LeaseRefusal::Preparing { path, pid },
  })
}

/// What the reservation of a return found
enum Reservation {
  /// The lease of `path` is `returning`, of this process
  Reserved,
  /// Another return of the lease runs
  Returning,
  /// The slot has another lease or none: another return of the lease freed it (a get may have leased it again)
  Returned,
}

/// Under the pool lock, with CAS on the lease id: mark the lease of `path` `returning`, of this process, unless another
/// return of it runs or freed it
fn reserve_return(pool: &Pool, timeout: Duration, path: &str, lease_id: &str) -> Result<Reservation> {
  let pid = std::process::id();
  pool.update(timeout, |store| {
    let current = store.slots.iter_mut().find(|slot| slot.path == path).and_then(|slot| slot.lease.as_mut());
    let Some(current) = current.filter(|current| current.id == lease_id) else {
      return Ok(Reservation::Returned);
    };
    match refusal_to_return(path, current) {
      Some(LeaseRefusal::Returning { .. }) => return Ok(Reservation::Returning),
      Some(refusal) => bail!(refusal),
      None => {}
    }
    current.state = LeaseState::Returning;
    current.pid = pid;
    current.process_started = process_started(pid);
    Ok(Reservation::Reserved)
  })
}

/// What a reserved return did to the slot
struct FinishedReturn {
  removed: Vec<String>,
  detached_branch: Option<String>,
  cleaned: Vec<Cleaned>,
  clean_error: Option<String>,
}

/// What a reserved return does to the slot: discard the `changes` it may give up, remove the seeded paths, detach HEAD
/// and apply the clean rules
fn finish_return(git_executor: &GitCommandExecutor, path: &str, lease: &Lease, changes: &[String], clean: Option<CleanOverrides>) -> Result<FinishedReturn> {
  if !changes.is_empty() {
    discard_slot_changes(git_executor, path)?;
  }
  let removed = remove_seeded(Path::new(path), &lease.seeded)?;
  let detached_branch = read_head(git_executor, path)?.and_then(|(_, branch)| branch);
  if detached_branch.is_some() {
    detach_slot_head(git_executor, path)?;
  }
  // the rules of the source, as the seed is read; the slot's own when the source is gone. They never fail the return
  let rules_from = Some(Path::new(&lease.source))
    .filter(|source| !lease.source.is_empty() && source.is_dir())
    .unwrap_or_else(|| Path::new(path));
  let (cleaned, clean_error) = match clean.map(|overrides| read_clean_rules(rules_from).map(|rules| overrides.apply(rules))) {
    None => (Vec::new(), None),
    Some(Ok(rules)) => (apply_clean_rules(git_executor, path, &rules, true), None),
    Some(Err(error)) => (Vec::new(), Some(format!("{error:#}"))),
  };
  Ok(FinishedReturn {
    removed,
    detached_branch,
    cleaned,
    clean_error,
  })
}

/// A fresh scan once git's background maintenance (`ProcessScan::git_maintenance`) has left `slot`, or after `timeout`:
/// a commit in the slot starts `git maintenance run --auto --detach`, whose repack keeps the slot as its working
/// directory for half a minute and writes only the shared object store. One still there after the wait stays in the
/// scan, so the return refuses as `slot_busy` and names it
fn wait_for_git_maintenance(slot: &Path, timeout: Duration) -> ProcessScan {
  let deadline = std::time::Instant::now() + timeout;
  loop {
    let scan = ProcessScan::scan();
    let maintenance = scan.git_maintenance();
    let waiting = scan.inside(slot).iter().filter(|process| maintenance.contains(&process.pid)).count();
    if waiting == 0 || std::time::Instant::now() >= deadline {
      return scan;
    }
    debug!(slot = %slot.display(), processes = waiting, "waiting for git's maintenance to leave the slot");
    std::thread::sleep(Duration::from_millis(250));
  }
}

/// A Bazel server of a slot that the return stopped
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoppedServer {
  pub pid: u32,
  /// Its `--workspace_directory`: the slot or a directory in it (`community/`)
  pub workspace: String,
  /// Its `--output_base`, which stays on disk: the next lease of the slot gets the same one, with its disk cache
  pub output_base: Option<String>,
}

/// Ask each Bazel server to exit with SIGTERM and wait up to `timeout` for them; the ones that are gone. No other
/// process is ever signalled: the caller passes only processes with `--workspace_directory` in the slot, and each is
/// signalled only while its pid still has the start time of the scan, checked right before its `kill`. Between the
/// scan and the signal a server can exit and the system can give its pid to an unrelated process. A server without a
/// known start time is never signalled and stays running, so the slot stays busy
fn stop_servers(servers: &[&ProcessInfo], timeout: Duration) -> Vec<StoppedServer> {
  if servers.is_empty() {
    return Vec::new();
  }
  let running = |server: &ProcessInfo| server.started.is_none_or(|started| is_running(server.pid, Some(started)));
  for server in servers {
    let Some(started) = server.started else {
      debug!(pid = server.pid, "a Bazel server without a start time is not signalled");
      continue;
    };
    if is_running(server.pid, Some(started)) {
      debug!(pid = server.pid, argv = ?server.argv, "stopping a Bazel server of the slot");
      terminate(server.pid);
    }
  }
  let deadline = std::time::Instant::now() + timeout;
  while servers.iter().any(|server| running(server)) && std::time::Instant::now() < deadline {
    std::thread::sleep(Duration::from_millis(50));
  }
  servers
    .iter()
    .filter(|server| !running(server))
    .map(|server| StoppedServer {
      pid: server.pid,
      workspace: server.bazel_workspace().map_or_default(|workspace| display_path(&workspace)),
      output_base: server.output_base().map(str::to_string),
    })
    .collect()
}

/// How `bd lease status` sees a slot, from the state alone
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotState {
  Free,
  Preparing,
  Ready,
  /// A free slot that a get clones into a new one
  Templating,
  /// Its return runs
  Returning,
  /// A free slot that `bd lease clean` cleans
  Cleaning,
  /// A lease that is not `ready` and whose process is gone: the next get takes the slot as free
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
        Some(lease) if lease.state == LeaseState::Templating => SlotState::Templating,
        Some(lease) if lease.state == LeaseState::Returning => SlotState::Returning,
        Some(lease) if lease.state == LeaseState::Cleaning => SlotState::Cleaning,
        Some(_) => SlotState::Ready,
      };
      let missing = !Path::new(&slot.path).is_dir();
      let processes = processes.then(|| processes_of(&scan.inside(Path::new(&slot.path))));
      SlotStatus { slot, state, missing, processes }
    })
    .collect();
  Ok((slots, scan.is_available()))
}

/// What `bd lease clean` cleans
pub struct LeaseCleanParams<'a> {
  /// The checkout bd runs in: its `.branchdeck.toml` holds the rules, and without `free` and `dirs` it is the target
  pub repository: &'a RepositoryDirs,
  /// The free slots of the pool
  pub free: bool,
  /// Worktrees to clean
  pub dirs: &'a [PathBuf],
  /// Delete, the default; false (`--dry-run`) only measures
  pub apply: bool,
  /// The parameters of this call that change the rules of the file
  pub overrides: CleanOverrides,
  /// The wait for the pool lock, for the `cleaning` marker of a slot
  pub lock_timeout: Duration,
}

/// A checkout `bd lease clean` cleaned, or skipped
#[derive(Debug, Clone)]
pub struct CleanTarget {
  pub path: String,
  /// Why it was not cleaned: a lease, a process in it
  pub skipped: Option<String>,
  pub cleaned: Vec<Cleaned>,
}

/// Apply the rules of `.branchdeck.toml` of the repository, read once, to the free slots, to `dirs`, or to the
/// repository itself. A slot of the pool with a lease or with a process in it is skipped, as a get would skip it; a
/// checkout that is no slot is cleaned as asked. The rules that can't be read are the error, and nothing is cleaned
pub fn lease_clean_core(git_executor: &GitCommandExecutor, params: &LeaseCleanParams<'_>) -> Result<(Vec<CleanTarget>, bool)> {
  let rules = params.overrides.apply(read_clean_rules(Path::new(&params.repository.top_level))?);
  let pool = Pool::new(Path::new(&canonical_path(Path::new(&params.repository.git_common_dir))));
  let store = pool.read()?;
  let mut paths: Vec<String> = Vec::new();
  if params.free {
    paths.extend(
      store
        .slots
        .iter()
        .filter(|slot| slot.lease.is_none() && Path::new(&slot.path).is_dir())
        .map(|slot| slot.path.clone()),
    );
  }
  paths.extend(params.dirs.iter().map(|dir| canonical_path(dir)));
  if !params.free && params.dirs.is_empty() {
    paths.push(canonical_path(Path::new(&params.repository.top_level)));
  }
  let mut seen = HashSet::new();
  paths.retain(|path| seen.insert(path.clone()));
  let scan = ProcessScan::scan();
  let marker = Lease {
    id: format!("clean-{}", uuid::Uuid::new_v4()),
    holder: "bd lease clean".to_string(),
    leased_at: SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| since.as_secs()),
    state: LeaseState::Cleaning,
    pid: std::process::id(),
    process_started: process_started(std::process::id()),
    source_head: String::new(),
    source: String::new(),
    source_branch: None,
    branch: None,
    seeded: Vec::new(),
    reused: false,
  };
  let targets = paths
    .into_iter()
    .map(|path| {
      let is_slot = store.slots.iter().any(|slot| slot.path == path);
      let processes = processes_of(&scan.inside(Path::new(&path)));
      // a slot is cleaned only under the `cleaning` marker, set while it is free, so no get leases it meanwhile; a dry
      // run deletes nothing and only reads the state again
      let leased = if !is_slot {
        Ok(None)
      } else if params.apply && processes.is_empty() {
        mark_for_clean(&pool, params.lock_timeout, &path, &marker)
      } else {
        Ok(
          pool
            .read()
            .ok()
            .and_then(|store| store.slots.into_iter().find(|slot| slot.path == path))
            .and_then(|slot| slot.lease)
            .map(|lease| lease.holder),
        )
      };
      let skipped = match leased {
        Ok(Some(holder)) => Some(format!("leased by {holder}")),
        Ok(None) if is_slot && !processes.is_empty() => Some(format!("processes work in it: {}", describe_processes(&processes))),
        Ok(None) => None,
        Err(error) => Some(format!("{error:#}")),
      };
      let cleaned = if skipped.is_some() {
        Vec::new()
      } else {
        apply_clean_rules(git_executor, &path, &rules, params.apply)
      };
      if is_slot && params.apply && skipped.is_none() {
        release_marker(&pool, params.lock_timeout, &path, &marker.id);
      }
      CleanTarget { path, skipped, cleaned }
    })
    .collect();
  Ok((targets, scan.is_available()))
}

/// Under the pool lock: mark the slot `path` with the `cleaning` `marker` while it is free (a lease whose process is gone
/// counts as free); the holder of its lease when it has one (a get leased it since the read, or a return runs)
fn mark_for_clean(pool: &Pool, timeout: Duration, path: &str, marker: &Lease) -> Result<Option<String>> {
  pool.update(timeout, |store| {
    // a slot the pool dropped meanwhile (its directory was gone) is a checkout that is no slot
    let Some(slot) = store.slots.iter_mut().find(|slot| slot.path == path) else {
      return Ok(None);
    };
    if let Some(lease) = slot.lease.as_ref().filter(|lease| !lease.is_abandoned()) {
      return Ok(Some(lease.holder.clone()));
    }
    slot.lease = Some(marker.clone());
    Ok(None)
  })
}

/// A directory `bd lease adopt` added, or refused
#[derive(Debug, Clone)]
pub enum Adopted {
  /// With `moved_from` when `--move` moved it into the pool directory
  Added {
    slot: Slot,
    moved_from: Option<String>,
  },
  Refused {
    path: String,
    refusal: LeaseRefusal,
  },
}

/// Add existing linked worktrees of this repository to the pool as free slots, where they are. Each is refused when it
/// is not a linked worktree of the same common dir, the pool holds it, a process is in it, or it has local changes (one
/// `status`)
/// What `bd lease adopt` adds
pub struct LeaseAdoptParams<'a> {
  /// A worktree of the repository, where `git worktree move` runs
  pub repository: &'a str,
  pub git_common_dir: &'a Path,
  pub dirs: &'a [PathBuf],
  /// `--move`: the pool directory each worktree moves into, as `<pool>/<n>/<repository name>`
  pub move_into: Option<&'a Path>,
  pub lock_timeout: Duration,
}

/// Add existing linked worktrees of this repository to the pool as free slots, where they are, or with `move_into`
/// moved into the pool directory by `git worktree move` (same volume only). Each is refused when it is not a linked
/// worktree of the same common dir, the pool holds it, a process is in it, or it has local changes (one `status`)
pub fn lease_adopt_core(git_executor: &GitCommandExecutor, params: &LeaseAdoptParams<'_>) -> Result<(Vec<Adopted>, bool)> {
  let git_common_dir = PathBuf::from(canonical_path(params.git_common_dir));
  let pool = Pool::new(&git_common_dir);
  let scan = ProcessScan::scan();
  let untracked = lease_config(git_executor, params.repository)?.untracked;
  let move_into = match params.move_into {
    Some(dir) => {
      std::fs::create_dir_all(dir).with_context(|| format!("Cannot create the pool directory {}", dir.display()))?;
      Some(PathBuf::from(canonical_path(dir)))
    }
    None => None,
  };
  let mut outcomes = Vec::new();
  for dir in params.dirs {
    let path = canonical_path(dir);
    let adopted = match adopt_one(git_executor, &pool, &git_common_dir, &path, &scan, untracked)? {
      Ok(()) => match &move_into {
        Some(pool_dir) => move_in(git_executor, &pool, params, &git_common_dir, pool_dir, &path)?,
        None => add_slot_entry(&pool, params.lock_timeout, &path, None)?,
      },
      Err(refusal) => Err(refusal),
    };
    outcomes.push(match adopted {
      Ok((slot, moved_from)) => Adopted::Added { slot, moved_from },
      Err(refusal) => Adopted::Refused { path, refusal },
    });
  }
  Ok((outcomes, scan.is_available()))
}

type AdoptResult = Result<Result<(Slot, Option<String>), LeaseRefusal>>;

/// Record the worktree at `path` (moved from `moved_from`) as a free slot, under the pool lock
fn add_slot_entry(pool: &Pool, lock_timeout: Duration, path: &str, moved_from: Option<String>) -> AdoptResult {
  let name = linked_worktree(Path::new(path)).map_or_default(|worktree| worktree.id);
  pool.update(lock_timeout, |store| {
    if store.slots.iter().any(|slot| slot.path == path) {
      return Ok(Err(LeaseRefusal::InPool(path.to_string())));
    }
    let slot = Slot {
      name,
      path: path.to_string(),
      dirty: false,
      returned: None,
      lease: None,
    };
    store.slots.push(slot.clone());
    Ok(Ok((slot, moved_from)))
  })
}

/// `--move`: under the pool lock, the next slot path, then `git worktree move` there (a rename on one volume, so the
/// lock is held for a moment) and the free slot recorded. The parent of the old directory goes when it is left empty
/// (`<treehouse pool>/<n>/`); the state file of Treehouse is not touched, and Treehouse sees the old path as missing
fn move_in(git_executor: &GitCommandExecutor, pool: &Pool, params: &LeaseAdoptParams<'_>, git_common_dir: &Path, pool_dir: &Path, path: &str) -> AdoptResult {
  if !same_volume(Path::new(path), pool_dir) {
    return Ok(Err(LeaseRefusal::OtherVolume {
      path: path.to_string(),
      pool: display_path(pool_dir),
    }));
  }
  let moved = pool.update(params.lock_timeout, |store| {
    if store.slots.iter().any(|slot| slot.path == path) {
      return Ok(Err(LeaseRefusal::InPool(path.to_string())));
    }
    let target = next_slot_path(store, pool_dir, git_common_dir);
    let cannot_move = |reason: String| LeaseRefusal::CannotMove { path: path.to_string(), reason };
    if let Some(parent) = target.parent()
      && let Err(error) = std::fs::create_dir_all(parent)
    {
      return Ok(Err(cannot_move(error.to_string())));
    }
    if let Err(error) = move_slot(git_executor, params.repository, path, &target) {
      if let Some(parent) = target.parent() {
        let _ = std::fs::remove_dir(parent);
      }
      return Ok(Err(cannot_move(format!("{error:#}"))));
    }
    let target = display_path(&target);
    let slot = Slot {
      name: linked_worktree(Path::new(&target)).map_or_default(|worktree| worktree.id),
      path: target,
      dirty: false,
      returned: None,
      lease: None,
    };
    store.slots.push(slot.clone());
    Ok(Ok(slot))
  })?;
  let slot = match moved {
    Ok(slot) => slot,
    Err(refusal) => return Ok(Err(refusal)),
  };
  if let Some(parent) = Path::new(path).parent() {
    // only an empty one goes
    let _ = std::fs::remove_dir(parent);
  }
  Ok(Ok((slot, Some(path.to_string()))))
}

/// Whether `a` and `b` are on one volume (`st_dev`)
#[cfg(unix)]
fn same_volume(a: &Path, b: &Path) -> bool {
  use std::os::unix::fs::MetadataExt;
  matches!((std::fs::metadata(a), std::fs::metadata(b)), (Ok(a), Ok(b)) if a.dev() == b.dev())
}

/// Elsewhere than on Unix the move is left to git
#[cfg(not(unix))]
const fn same_volume(_a: &Path, _b: &Path) -> bool {
  true
}

/// The checks of an adopt: a linked worktree of this repository, not in the pool, no process in it, clean
fn adopt_one(git_executor: &GitCommandExecutor, pool: &Pool, git_common_dir: &Path, path: &str, scan: &ProcessScan, untracked: UntrackedFiles) -> Result<Result<(), LeaseRefusal>> {
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
  let changes = slot_changes(git_executor, path, untracked)?;
  if !changes.is_empty() {
    return Ok(Err(LeaseRefusal::DirtyCandidate {
      path: path.to_string(),
      paths: changes,
    }));
  }
  Ok(Ok(()))
}

#[cfg(test)]
#[path = "lease_test.rs"]
mod lease_test;
