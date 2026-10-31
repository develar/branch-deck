//! The LMDB environment and the typed batch reads and writes over it.
//!
//! - **Keys are object ids**, so an entry never becomes stale: it is only evicted. The cap is the map size; a write that
//!   hits `MDB_MAP_FULL` clears every database of its environment and is retried once.
//! - **Four environments**, so that what is bulky never evicts what is small and costly: the diffs and merges of
//!   `file_diff` and `file_merge` in `patches/` (`CacheConfig::patch_map_size`); the merges of `merge_result`, the trees of
//!   `commit_tree` and the resolves of `word_resolution`, small entries a check costs most to learn again, in `merges/`
//!   (`CacheConfig::merge_map_size`); the conflict views of
//!   `conflict_info`, a few that may each hold megabytes of file contents, and the chains of `conflict_chain`, in
//!   `conflicts/` (`CacheConfig::conflict_map_size`); the other stores (the diffs, lines and summaries of commits and
//!   blobs) in the cache directory itself. A store moved to another environment is dropped from the old one on open,
//!   and a retired store (`RETIRED_STORES`) from every environment.
//! - **The exceptions are `review_answer`, `patronus_robots`, `patronus_history` and `build_result`**, keyed by a hash of
//!   what they decide on. An answer of Patronus (the changed paths, the target, the reviews of a pull request; the user's
//!   robots of a repository; a week of everyone's robots and the runs of the user's finished ones), which the target's ownership rules or a robot that starts or finishes can outdate: its value carries
//!   when it was fetched, and the reader decides how long it holds (`remote_facts::review_readiness`, `bd`'s
//!   `robot_list`, `remote_facts::robot_stats`). A passed build of `bd check --build` (the command, the branch, the ids of what it
//!   extracts), which what the build reads outside the tree (a JDK, a toolchain) can outdate: `--rebuild` runs it
//!   anyway. `session_frame` is keyed by a hash of a worktree: its value is the last frame of the worktree's session,
//!   which the next save replaces and whose reader checks its key. `remote_status` is keyed by a hash too, of commit ids and the user's email: its value is what walks
//!   between those commits find, so it holds as long as the commit graph does (a `git replace` of a commit in that
//!   range would outdate it). `stack_branch` is keyed by a hash of the base and the original commits of a stack
//!   branch: its value is what their build gave (the tip, or the conflict), a function of those commits.
//! - **Several processes** (the app and parallel `bd` calls) share the environment: LMDB serializes writers through its
//!   lock file and readers never block. Within a process there is one `Env` per directory (its slot in `ENVS`, whose
//!   guard alone is held across the open and the close), because LMDB must not open the same environment twice in one
//!   process. Across processes the opens and closes of a directory run one
//!   at a time under its `open.lock`, and on macOS each lock file gets a semaphore set of its own (`semaphore_key`).
//! - **Each environment opens on its own**: one that cannot (a file in the way, a corrupt map, a semaphore set another
//!   process removed) misses its stores, logged as a warning, and the others work.
//! - **The last handle closes the environment** (`EnvHandle`): on macOS and the BSDs LMDB locks with a System V
//!   semaphore set per lock file, which only `mdb_env_close` removes, and not an exit. An environment kept open to the
//!   end of every process left one set per directory, a new one per temporary directory of a test, until the system
//!   ran out of them (`ENOSPC`, every cache disabled).
//! - **Each store has its own version, part of its database name** (`commit_diff.v3`). A changed value type bumps the
//!   version of its store only, so the other stores keep their entries (the merges of `merge_result` are the costly
//!   ones). Opening drops a store's databases of older versions; newer ones belong to a newer binary that may still
//!   run, and are kept.
//! - **A batch per transaction**: each commit syncs the file (`F_FULLFSYNC` on macOS) and takes the writer lock that
//!   other processes wait for, so a caller writes what a phase learned at once, never one entry per transaction in a
//!   loop (`sync_core::cache_memo::CacheMemo`). Only a test turns the sync off (`CacheConfig::sync_commits`).
//! - **Every failure is a miss**: it is logged at debug level (an environment that cannot open at warning level) and
//!   never fails the caller.

use crate::semaphore_key;
use heed::types::{Bytes, DecodeIgnore};
use heed::{CommitOrAbort, CommitToken, Database, DatabaseOpener, Env, EnvFlags, EnvOpenOptions, OnCommit, RoTxn, RwTxn, WithTls};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::convert::Infallible;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError, Weak};
use tracing::{debug, warn};

/// The key of the entry that names the version of a store whose keys hold a version (`keep_only_version`); no key of
/// an entry is this short
pub(crate) const VERSION_KEY: &[u8] = b"version";
/// The default map size, which caps the size of the cache
pub const DEFAULT_MAP_SIZE: usize = 1 << 30;
/// The default map size of the patch environment
pub const DEFAULT_PATCH_MAP_SIZE: usize = 512 << 20;
/// The default map size of the merge environment: an entry of `merge_result` takes about 250 bytes (a key of three tree
/// ids, a tree id or a few paths, the B-tree's overhead), one of `commit_tree` about 130, so it holds the merges and
/// trees of some 700,000 commit copies; a check of 200 commits writes at most 200 of each
pub const DEFAULT_MERGE_MAP_SIZE: usize = 256 << 20;
/// The default map size of the conflict environment: the conflict views (a few kilobytes each) and the chains of the
/// conflicting branches. It was sized for the contents of the views, which are no longer kept
pub const DEFAULT_CONFLICT_MAP_SIZE: usize = 512 << 20;

/// An LMDB environment of the cache: a directory with its own map size
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Environment {
  /// The cache directory itself
  Main,
  /// `patches/` in it
  Patches,
  /// `merges/` in it
  Merges,
  /// `conflicts/` in it
  Conflicts,
}

const ENVIRONMENTS: [Environment; 4] = [Environment::Main, Environment::Patches, Environment::Merges, Environment::Conflicts];

impl Environment {
  fn dir(self, cache_dir: &Path) -> PathBuf {
    match self {
      Self::Main => cache_dir.to_path_buf(),
      Self::Patches => cache_dir.join("patches"),
      Self::Merges => cache_dir.join("merges"),
      Self::Conflicts => cache_dir.join("conflicts"),
    }
  }

  const fn map_size(self, config: &CacheConfig) -> usize {
    match self {
      Self::Main => config.map_size,
      Self::Patches => config.patch_map_size,
      Self::Merges => config.merge_map_size,
      Self::Conflicts => config.conflict_map_size,
    }
  }
}

/// The databases, in the order of `Inner::databases`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Store {
  CommitDiff,
  CommitFiles,
  CommitTree,
  MergeResult,
  CommitLines,
  BlobSummary,
  ReviewAnswer,
  ConflictInfo,
  RemoteStatus,
  BuildResult,
  ConflictChain,
  StackBranch,
  PatronusRobots,
  PatronusHistory,
  FileDiff,
  FileMerge,
  WordResolution,
  SessionFrame,
}

/// How many stores there are
const STORE_COUNT: usize = 18;

/// The stores that no binary writes any more: `commit_patch`, the patches of the old diff views (now `file_diff`)
const RETIRED_STORES: [&str; 1] = ["commit_patch"];

/// The version each store was checked for by this process (`keep_only_version`), by `Store::index`; 0 before a check
static CHECKED_VERSIONS: [AtomicU64; STORE_COUNT] = [const { AtomicU64::new(0) }; STORE_COUNT];

pub(crate) const STORES: [Store; STORE_COUNT] = [
  Store::CommitDiff,
  Store::CommitFiles,
  Store::CommitTree,
  Store::MergeResult,
  Store::CommitLines,
  Store::BlobSummary,
  Store::ReviewAnswer,
  Store::ConflictInfo,
  Store::RemoteStatus,
  Store::BuildResult,
  Store::ConflictChain,
  Store::StackBranch,
  Store::PatronusRobots,
  Store::PatronusHistory,
  Store::FileDiff,
  Store::FileMerge,
  Store::WordResolution,
  Store::SessionFrame,
];

impl Store {
  /// The version of the store's values: bump it when their type changes, or how git computes them, and only this store
  /// is dropped
  pub(crate) const fn version(self) -> u32 {
    match self {
      // 2: the blob ids of `CommitFile`; 3: its submodule flag
      Self::CommitFiles => 3,
      // `commit_lines` 2: the paths of the raw entries, so a file whose name holds a space or a quote is there; 3: the
      // lines and their order in each hunk, with the algorithm, the inter-hunk context and the indent heuristic pinned;
      // 4: the lines of the blocks of the diff views (`file_diffs::patch`).
      // `commit_diff` 2: the blob ids of `FileChange`; 3: the hunks pinned as `commit_lines`; 4: the hunks and the
      // counts from the blocks of the diff views (`file_diffs::blocks`)
      Self::CommitLines | Self::CommitDiff => 4,
      // a new extractor needs no bump: its version is part of each key (`blob_summary`, `conflict_info`,
      // `remote_status`, `build_result`, `conflict_chain`, `stack_branch`, `word_resolution`, the schema of
      // `session_frame`)
      Self::CommitTree
      | Self::MergeResult
      | Self::BlobSummary
      | Self::ReviewAnswer
      | Self::ConflictInfo
      | Self::RemoteStatus
      | Self::BuildResult
      | Self::ConflictChain
      | Self::StackBranch
      | Self::PatronusRobots
      | Self::PatronusHistory
      | Self::FileDiff
      | Self::FileMerge
      | Self::WordResolution
      | Self::SessionFrame => 1,
    }
  }

  pub(crate) const fn environment(self) -> Environment {
    match self {
      Self::FileDiff | Self::FileMerge => Environment::Patches,
      Self::MergeResult | Self::CommitTree | Self::WordResolution => Environment::Merges,
      Self::ConflictInfo | Self::ConflictChain => Environment::Conflicts,
      Self::CommitDiff
      | Self::CommitFiles
      | Self::CommitLines
      | Self::BlobSummary
      | Self::ReviewAnswer
      | Self::RemoteStatus
      | Self::BuildResult
      | Self::StackBranch
      | Self::PatronusRobots
      | Self::PatronusHistory
      | Self::SessionFrame => Environment::Main,
    }
  }

  /// The index of the store in `STORES` and in `Inner::databases`
  pub(crate) const fn index(self) -> usize {
    self as usize
  }

  const fn base_name(self) -> &'static str {
    match self {
      Self::CommitDiff => "commit_diff",
      Self::CommitFiles => "commit_files",
      Self::CommitTree => "commit_tree",
      Self::MergeResult => "merge_result",
      Self::CommitLines => "commit_lines",
      Self::BlobSummary => "blob_summary",
      Self::ReviewAnswer => "review_answer",
      Self::ConflictInfo => "conflict_info",
      Self::RemoteStatus => "remote_status",
      Self::BuildResult => "build_result",
      Self::ConflictChain => "conflict_chain",
      Self::StackBranch => "stack_branch",
      Self::PatronusRobots => "patronus_robots",
      Self::PatronusHistory => "patronus_history",
      Self::FileDiff => "file_diff",
      Self::FileMerge => "file_merge",
      Self::WordResolution => "word_resolution",
      Self::SessionFrame => "session_frame",
    }
  }
}

#[derive(Debug, Clone)]
pub struct CacheConfig {
  /// The LMDB map size in bytes, rounded up to the page size: the cap of the cache
  pub map_size: usize,
  /// The map size of the patch environment (`file_diff`, `file_merge`)
  pub patch_map_size: usize,
  /// The map size of the merge environment (`merge_result`, `commit_tree`, `word_resolution`)
  pub merge_map_size: usize,
  /// The map size of the conflict environment (`conflict_info`)
  pub conflict_map_size: usize,
  /// The version of each store, by `Store::index`: `Store::version`, or a newer one in a test
  pub(crate) versions: [u32; STORE_COUNT],
  /// Whether a commit returns only once its pages are on the drive (`F_FULLFSYNC` on macOS, `fdatasync` on Linux,
  /// `FlushFileBuffers` on Windows): always in the app and in `bd`, where a power loss must not leave a wrong but readable
  /// entry. A test turns it off (`bd_core::context::Environment::flush_to_drive`): thousands of commits in 20 processes
  /// at once queue at the drive, and a test survives no power loss. The first open of an environment in a process
  /// decides for every later handle of it (`ENVS`)
  pub sync_commits: bool,
}

impl Default for CacheConfig {
  fn default() -> Self {
    Self {
      map_size: DEFAULT_MAP_SIZE,
      patch_map_size: DEFAULT_PATCH_MAP_SIZE,
      merge_map_size: DEFAULT_MERGE_MAP_SIZE,
      conflict_map_size: DEFAULT_CONFLICT_MAP_SIZE,
      versions: STORES.map(Store::version),
      sync_commits: true,
    }
  }
}

/// The key of an entry of a store keyed by a hash the caller makes of what the value is a function of
/// (`remote_status`, `build_result`, `conflict_chain`, `stack_branch`): the version that wrote it, big-endian, then the
/// hash
pub(crate) fn hashed_key(version: u32, hash: &[u8; 32]) -> Vec<u8> {
  [version.to_be_bytes().as_slice(), hash].concat()
}

/// `$GIT_COMMON_DIR/branch-deck/cache`: shared by all worktrees of a repository
pub fn cache_dir(git_common_dir: &Path) -> PathBuf {
  git_common_dir.join("branch-deck").join("cache")
}

/// The slot of the environment of one directory: its `Env` while a handle of it lives. Its guard is held across the open
/// and the close of that environment, so an open of the same directory waits until it is closed
type EnvSlot = Arc<Mutex<Weak<Env>>>;

/// The slots of the environments of this process, by canonical directory. Locked only to find or add a slot, never
/// across an open, a close or the wait for `open.lock` of another process: the four environments of a cache open side by
/// side, and an open of another repository's cache never waits for this one
static ENVS: OnceLock<Mutex<HashMap<PathBuf, EnvSlot>>> = OnceLock::new();

/// The slot of `dir`; the slots that no handle holds and no open fills go
fn env_slot(dir: &Path) -> EnvSlot {
  let mut envs = ENVS.get_or_init(Mutex::default).lock().unwrap_or_else(PoisonError::into_inner);
  // a slot another thread holds (`try_lock` fails) may be opening: it stays
  envs.retain(|_, slot| Arc::strong_count(slot) > 1 || slot.try_lock().map_or(true, |env| env.strong_count() > 0));
  Arc::clone(envs.entry(dir.to_path_buf()).or_default())
}

fn lock_slot(slot: &EnvSlot) -> MutexGuard<'_, Weak<Env>> {
  slot.lock().unwrap_or_else(PoisonError::into_inner)
}

/// An open environment of its slot (`ENVS`). The last handle closes it (`mdb_env_close`, which removes its semaphores
/// when no other process has it open) while its slot is locked, so an open of the same directory waits until it is
/// closed, and under `open.lock`, so the open of another process does not read the semaphores the close removes
pub(crate) struct EnvHandle {
  env: Option<Arc<Env>>,
  slot: EnvSlot,
}

impl Deref for EnvHandle {
  type Target = Env;

  fn deref(&self) -> &Env {
    self.env.as_deref().expect("taken only by drop")
  }
}

impl Drop for EnvHandle {
  fn drop(&mut self) {
    let slot = lock_slot(&self.slot);
    let env = self.env.take();
    let closing = env.as_ref().filter(|env| Arc::strong_count(env) == 1).and_then(|env| lock_opens(env.path()).ok());
    drop(env);
    drop(closing);
    drop(slot);
  }
}

/// Holds `open.lock` of the environment directory `dir` until dropped: the opens and closes of the environment by the
/// processes of this build, one at a time
fn lock_opens(dir: &Path) -> std::io::Result<std::fs::File> {
  let file = std::fs::OpenOptions::new().write(true).create(true).truncate(false).open(dir.join("open.lock"))?;
  file.lock()?;
  Ok(file)
}

/// A database committed by the transaction that opened it, usable in any later one (heed's `OnCommit`)
type RawDatabase = Database<'static, Bytes, Bytes>;

/// The databases of one environment while the transaction that opens them runs; its commit makes them `RawDatabase`s
struct OpenedDatabases<'e>(Vec<Database<'e, Bytes, Bytes>>);

impl OnCommit for OpenedDatabases<'_> {
  type Committed = Vec<RawDatabase>;

  fn on_commit(self, token: &CommitToken) -> Self::Committed {
    self.0.into_iter().map(|database| database.on_commit(token)).collect()
  }
}

struct Inner {
  /// By `Environment`, in the order of `ENVIRONMENTS`; None for one that could not be opened, whose stores miss
  envs: [Option<EnvHandle>; ENVIRONMENTS.len()],
  /// None for the stores of such an environment
  databases: [Option<RawDatabase>; STORE_COUNT],
}

/// A handle to the cache of one repository; cheap to clone. A disabled cache misses every read and drops every write
#[derive(Clone, Default)]
pub struct ObjectCache {
  inner: Option<Arc<Inner>>,
}

impl ObjectCache {
  /// A cache that is never read or written (`--no-cache`)
  pub fn disabled() -> Self {
    Self::default()
  }

  /// The cache of the repository whose common git directory is `git_common_dir`, with the default config
  pub fn open(git_common_dir: &Path) -> Self {
    Self::open_dir(&cache_dir(git_common_dir), &CacheConfig::default())
  }

  /// The cache in `dir`; disabled if it can't be opened
  pub fn open_dir(dir: &Path, config: &CacheConfig) -> Self {
    match open_inner(dir, config) {
      Ok(inner) => Self { inner: Some(Arc::new(inner)) },
      Err(e) => {
        debug!(error = %e, dir = %dir.display(), "object cache disabled");
        Self::disabled()
      }
    }
  }

  pub const fn is_enabled(&self) -> bool {
    self.inner.is_some()
  }

  /// Clear `store` unless it holds the entries of `version` (its `VERSION_KEY` entry), then mark it so: the entries of
  /// another version go at once instead of waiting for a full map. Once per process, store and version
  /// (`CHECKED_VERSIONS`)
  pub(crate) fn keep_only_version(&self, store: Store, version: u32) {
    let Some(inner) = &self.inner else { return };
    let checked = &CHECKED_VERSIONS[store.index()];
    // the high bit tells a checked version from the initial 0
    let stamp = u64::from(version) | 1 << 63;
    if checked.load(Ordering::Relaxed) == stamp {
      return;
    }
    let meta = version.to_be_bytes();
    let Some((env, database)) = inner.store(store) else { return };
    let result = (|| -> heed::Result<()> {
      let txn = env.read_txn()?;
      let current = database.get(&txn, VERSION_KEY)?.map(<[u8]>::to_vec);
      drop(txn);
      if current.as_deref() != Some(meta.as_slice()) {
        debug!(store = store.base_name(), version, "clearing the entries of another version");
        let mut txn = env.write_txn()?;
        database.clear(&mut txn)?;
        database.put(&mut txn, VERSION_KEY, &meta)?;
        txn.commit()?;
      }
      Ok(())
    })();
    match result {
      Ok(()) => checked.store(stamp, Ordering::Relaxed),
      Err(e) => debug!(error = %e, store = store.base_name(), "cache version check failed"),
    }
  }

  /// The values of `keys`, in their order; a missing or unreadable value is `None`
  pub(crate) fn get_many<V: DeserializeOwned>(&self, store: Store, keys: &[Vec<u8>]) -> Vec<Option<V>> {
    let Some((env, database)) = self.inner.as_ref().and_then(|inner| inner.store(store)) else {
      return keys.iter().map(|_| None).collect();
    };
    let result = (|| -> heed::Result<Vec<Option<V>>> {
      let txn = env.read_txn()?;
      keys
        .iter()
        .map(|key| {
          Ok(database.get(&txn, key)?.and_then(|bytes| match postcard::from_bytes(bytes) {
            Ok(value) => Some(value),
            Err(e) => {
              debug!(error = %e, store = store.base_name(), "unreadable cache value");
              None
            }
          }))
        })
        .collect()
    })();
    result.unwrap_or_else(|e| {
      debug!(error = %e, store = store.base_name(), "cache read failed");
      keys.iter().map(|_| None).collect()
    })
  }

  /// Whether `store` has a value for each of `keys`, in their order, without decoding one; a failed read is false
  pub(crate) fn contains_many(&self, store: Store, keys: &[Vec<u8>]) -> Vec<bool> {
    let Some((env, database)) = self.inner.as_ref().and_then(|inner| inner.store(store)) else {
      return vec![false; keys.len()];
    };
    let result = (|| -> heed::Result<Vec<bool>> {
      let txn = env.read_txn()?;
      let database = database.remap_data_type::<DecodeIgnore>();
      keys.iter().map(|key| Ok(database.get(&txn, key)?.is_some())).collect()
    })();
    result.unwrap_or_else(|e| {
      debug!(error = %e, store = store.base_name(), "cache read failed");
      vec![false; keys.len()]
    })
  }

  /// The value of `hash` in a store keyed by a hash the caller makes (`hashed_key`), written by `version`
  pub(crate) fn get_hashed<V: DeserializeOwned>(&self, store: Store, version: u32, hash: &[u8; 32]) -> Option<V> {
    self.get_many(store, &[hashed_key(version, hash)]).pop().flatten()
  }

  /// Write `entries` of `version` into a store keyed by a hash in one transaction; the first write of a process clears
  /// the entries of another version (`keep_only_version`)
  pub(crate) fn put_hashed<V: Serialize>(&self, store: Store, version: u32, entries: &[([u8; 32], V)]) {
    if !self.is_enabled() || entries.is_empty() {
      return;
    }
    self.keep_only_version(store, version);
    let entries: Vec<(Vec<u8>, &V)> = entries.iter().map(|(hash, value)| (hashed_key(version, hash), value)).collect();
    self.put_many(store, &entries);
  }

  /// Write `entries` in one transaction. When the map is full, every database is cleared and the write retried once
  pub(crate) fn put_many<V: Serialize>(&self, store: Store, entries: &[(Vec<u8>, &V)]) {
    if self.inner.is_none() || entries.is_empty() {
      return;
    }
    match entries
      .iter()
      .map(|(key, value)| postcard::to_allocvec(value).map(|bytes| (key.as_slice(), bytes)))
      .collect::<Result<Vec<_>, _>>()
    {
      Ok(encoded) => self.put_encoded(store, &encoded),
      Err(e) => debug!(error = %e, store = store.base_name(), "cache value not encodable"),
    }
  }

  /// `put_many` of values encoded already (`postcard`)
  pub(crate) fn put_encoded(&self, store: Store, encoded: &[(&[u8], Vec<u8>)]) {
    let Some(inner) = &self.inner else { return };
    let Some((env, database)) = inner.store(store) else { return };
    if encoded.is_empty() {
      return;
    }
    let write = || -> heed::Result<()> {
      let mut txn = env.write_txn()?;
      for (key, value) in encoded {
        database.put(&mut txn, key, value)?;
      }
      txn.commit()
    };
    match write() {
      Ok(()) => {}
      Err(heed::Error::Mdb(heed::MdbError::MapFull)) => {
        debug!(store = store.base_name(), "object cache is full, clearing it");
        if let Err(e) = inner.clear_all(store.environment()).and_then(|()| write()) {
          debug!(error = %e, store = store.base_name(), "cache write failed after clearing");
        }
      }
      Err(e) => debug!(error = %e, store = store.base_name(), "cache write failed"),
    }
  }
}

impl Inner {
  /// The environment and the database of `store`; None when its environment could not be opened
  fn store(&self, store: Store) -> Option<(&Env, &RawDatabase)> {
    Some((self.envs[store.environment() as usize].as_deref()?, self.databases[store.index()].as_ref()?))
  }

  /// Clear the databases of `environment`
  fn clear_all(&self, environment: Environment) -> heed::Result<()> {
    let Some(env) = self.envs[environment as usize].as_deref() else { return Ok(()) };
    let mut txn = env.write_txn()?;
    for store in STORES.iter().filter(|store| store.environment() == environment) {
      if let Some(database) = &self.databases[store.index()] {
        database.clear(&mut txn)?;
      }
    }
    txn.commit()
  }
}

/// Each environment on its own: one that cannot be opened misses its stores; none, and the cache is disabled
fn open_inner(dir: &Path, config: &CacheConfig) -> Result<Inner, Box<dyn std::error::Error>> {
  let mut envs: [Option<EnvHandle>; ENVIRONMENTS.len()] = std::array::from_fn(|_| None);
  let mut databases: [Option<RawDatabase>; STORE_COUNT] = [None; STORE_COUNT];
  for environment in ENVIRONMENTS {
    let env_dir = environment.dir(dir);
    let stores: Vec<Store> = STORES.iter().copied().filter(|store| store.environment() == environment).collect();
    let opened = open_env(&env_dir, environment.map_size(config), config.sync_commits).and_then(|env| {
      let opened = open_databases(&env, environment, &stores, &config.versions)?;
      Ok((env, opened))
    });
    match opened {
      Ok((env, opened)) => {
        for (store, database) in stores.iter().zip(opened) {
          databases[store.index()] = Some(database);
        }
        envs[environment as usize] = Some(env);
      }
      Err(e) => warn!(error = %e, dir = %env_dir.display(), "an environment of the object cache cannot be opened: its stores miss"),
    }
  }
  if envs.iter().all(Option::is_none) {
    return Err("no environment of the object cache can be opened".into());
  }
  Ok(Inner { envs, databases })
}

#[cfg(test)]
impl ObjectCache {
  /// The flags of each open environment (`mdb_env_get_flags`), in the order of `ENVIRONMENTS`
  pub(crate) fn env_flags(&self) -> Vec<u32> {
    let Some(inner) = &self.inner else { return Vec::new() };
    inner.envs.iter().flatten().map(|env| env.get_flags().expect("the flags of an open environment")).collect()
  }
}

/// Whether this process has the environment of `dir` open
#[cfg(test)]
pub(crate) fn is_env_open(dir: &Path) -> bool {
  let dir = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
  lock_slot(&env_slot(&dir)).strong_count() > 0
}

/// Whether a thread of this process opens or closes the environment of `dir` now: it holds its slot
#[cfg(test)]
pub(crate) fn is_env_opening(dir: &Path) -> bool {
  let dir = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
  env_slot(&dir).try_lock().is_err()
}

/// The environment of `dir`, the one of this process if it is open (`ENVS`); a new one syncs each commit unless
/// `sync_commits` is off (`CacheConfig::sync_commits`)
pub(crate) fn open_env(dir: &Path, map_size: usize, sync_commits: bool) -> Result<EnvHandle, Box<dyn std::error::Error>> {
  std::fs::create_dir_all(dir)?;
  let dir = dir.canonicalize()?;
  let slot = env_slot(&dir);
  let mut opened = lock_slot(&slot);
  if let Some(env) = opened.upgrade() {
    drop(opened);
    return Ok(EnvHandle { env: Some(env), slot });
  }
  // a multiple of the 4 KiB and 16 KiB pages of the supported platforms
  let page_size = 16 * 1024;
  let map_size = map_size.div_ceil(page_size).max(1) * page_size;
  let _opening = lock_opens(&dir)?;
  let open = || {
    semaphore_key::claim(&dir);
    let mut options = EnvOpenOptions::new();
    options.map_size(map_size).max_dbs(32);
    if !sync_commits {
      // SAFETY: the flags are set before the open. Without `MDB_WRITEMAP` LMDB writes its pages with `write`, so a crash
      // of the process loses no commit (the pages are the kernel's) and a commit stays atomic; only a crash of the system
      // may lose the last commits or, on a file system that reorders writes, leave a wrong entry, and only a test, which
      // outlives no such crash, opens an environment so (`sync_commits`). On Windows `MDB_NOSYNC` skips
      // `FlushFileBuffers` the same way
      #[expect(unsafe_code, reason = "heed sets the flags of an LMDB environment through an unsafe call")]
      unsafe {
        options.flags(EnvFlags::NO_SYNC | EnvFlags::NO_META_SYNC);
      }
    }
    // SAFETY: the slot of this directory in `ENVS` holds its only `Env` in the process, and the files are only written by
    // LMDB
    #[expect(unsafe_code, reason = "heed opens an LMDB environment through an unsafe call")]
    unsafe {
      options.open(&dir)
    }
  };
  let env = Arc::new(match open() {
    // LMDB's own race with a process of an older build: the open waited for the close of the last other user, which
    // removed the semaphores the open then reads; the next open is the first
    Err(heed::Error::Io(e)) if e.kind() == std::io::ErrorKind::InvalidInput => open()?,
    result => result?,
  });
  *opened = Arc::downgrade(&env);
  drop(opened);
  Ok(EnvHandle { env: Some(env), slot })
}

/// The databases of `stores` in `env`, the environment `environment`, in their order, created if missing; the databases
/// of older versions, and those of stores that moved to another environment, are dropped
fn open_databases(env: &Env, environment: Environment, stores: &[Store], versions: &[u32; STORE_COUNT]) -> heed::Result<Vec<RawDatabase>> {
  let names: Vec<String> = stores.iter().map(|store| format!("{}.v{}", store.base_name(), versions[store.index()])).collect();
  // the common case, with every database there and none to drop, needs no write transaction; the handles stay valid
  // only because the read transaction commits (`open_databases_and_commit`), the abort of a miss drops them
  let existing = env.open_databases_and_commit(env.read_txn()?, |opener, txn| -> heed::Result<_> {
    let stale = stale_database_names(opener, environment, txn, versions)?;
    let databases: Option<Vec<_>> = names
      .iter()
      .map(|name| opener.open_database::<Bytes, Bytes>(txn, Some(name)))
      .collect::<heed::Result<Vec<_>>>()?
      .into_iter()
      .collect();
    Ok(match databases.filter(|_| stale.is_empty()) {
      Some(databases) => CommitOrAbort::Commit(OpenedDatabases(databases)),
      None => CommitOrAbort::Abort(()),
    })
  })?;
  if let CommitOrAbort::Commit(databases) = existing {
    return Ok(databases);
  }
  let databases = env
    .create_databases_and_commit(env.write_txn()?, |opener, txn| -> heed::Result<_> {
      drop_stale_databases(opener, environment, txn, versions)?;
      let databases = names
        .iter()
        .map(|name| opener.create_database::<Bytes, Bytes>(txn, Some(name)))
        .collect::<heed::Result<Vec<_>>>()?;
      Ok(CommitOrAbort::<_, Infallible>::Commit(OpenedDatabases(databases)))
    })?
    .unwrap_commit();
  Ok(databases)
}

/// The names of our databases in the environment `environment` of `opener`, with an older version than their store's in
/// `versions`, of a store of another environment, or of a retired store, from the unnamed database that lists all of
/// them
fn stale_database_names<'e>(opener: &DatabaseOpener<'e, WithTls>, environment: Environment, txn: &RoTxn<'e>, versions: &[u32; STORE_COUNT]) -> heed::Result<Vec<String>> {
  let Some(main) = opener.open_database::<Bytes, DecodeIgnore>(txn, None)? else {
    return Ok(Vec::new());
  };
  let mut stale = Vec::new();
  for item in main.iter(txn)? {
    let (name, ()) = item?;
    let Ok(name) = std::str::from_utf8(name) else { continue };
    let dropped = name.rsplit_once(".v").is_some_and(|(base, version)| {
      RETIRED_STORES.contains(&base)
        || STORES
          .iter()
          .find(|store| store.base_name() == base)
          .is_some_and(|store| store.environment() != environment || version.parse::<u32>().is_ok_and(|version| version < versions[store.index()]))
    });
    if dropped {
      stale.push(name.to_string());
    }
  }
  Ok(stale)
}

fn drop_stale_databases<'e>(opener: &DatabaseOpener<'e, WithTls>, environment: Environment, txn: &mut RwTxn<'e>, versions: &[u32; STORE_COUNT]) -> heed::Result<()> {
  for name in stale_database_names(opener, environment, txn, versions)? {
    if let Some(database) = opener.open_database::<Bytes, Bytes>(txn, Some(&name))? {
      debug!(name, "dropping an object cache database of an older schema");
      // SAFETY: the handle is used for nothing else; a process of an older version that still uses it gets errors,
      // which it treats as misses
      #[expect(unsafe_code, reason = "heed removes an LMDB database through an unsafe call")]
      unsafe {
        database.remove(txn)?;
      }
    }
  }
  Ok(())
}
