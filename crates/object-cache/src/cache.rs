//! The LMDB environment and the typed batch reads and writes over it.
//!
//! - **The environment is psi-syntax's `lmdb-env`** (decision 2026-11-09-lmdb-env-in-psi-syntax of accio), shared with
//!   accio's forward store: one per directory in a process, opened and closed under `open.lock`, a semaphore set of its
//!   own on macOS, reader slots held per transaction, the readers of dead processes cleared, the last handle closing
//!   it. Each rule is a doc comment and a test there; this module holds the stores, their environments and versions.
//! - **Keys are object ids**, so an entry never becomes stale: it is only evicted. The cap is the map size; a write that
//!   would fill it, or hits `MDB_MAP_FULL`, clears every database of its environment and is retried once.
//! - **Four environments**, so that what is bulky never evicts what is small and costly: the diffs and merges of
//!   `file_diff` and `file_merge`, and what a diff leaves to the other files of its subject (`file_leftovers`), in
//!   `patches/` (`CacheConfig::patch_map_size`); the merges of `merge_result`, the trees of
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
//!   `robot_list`, `remote_facts::robot_stats`). A passed build of `brd check --build` (the command, the branch, the ids of what it
//!   extracts), which what the build reads outside the tree (a JDK, a toolchain) can outdate: `--rebuild` runs it
//!   anyway. `session_frame` is keyed by a hash of a worktree: its value is the last frame of the worktree's session,
//!   which the next save replaces and whose reader checks its key. `remote_status` is keyed by a hash too, of commit ids and the user's email: its value is what walks
//!   between those commits find, so it holds as long as the commit graph does (a `git replace` of a commit in that
//!   range would outdate it). `stack_branch` is keyed by a hash of the base and the original commits of a stack
//!   branch: its value is what their build gave (the tip, or the conflict), a function of those commits.
//! - **Several processes** (the app and parallel `bd` calls) share the environment: LMDB serializes writers through its
//!   lock file and readers never block.
//! - **Each environment opens on its own**: one that cannot (a file in the way, a corrupt map, a semaphore set another
//!   process removed) misses its stores, logged as a warning, and the others work.
//! - **Each store has its own version, part of its database name** (`commit_diff.v3`). A changed value type bumps the
//!   version of its store only, so the other stores keep their entries (the merges of `merge_result` are the costly
//!   ones). Opening drops a store's databases of older versions; newer ones belong to a newer binary that may still
//!   run, and are kept. A store moved to another environment, or retired, is dropped there by its name (`retired_in`).
//! - **A batch per transaction**: each commit syncs the file (`F_FULLFSYNC` on macOS) and takes the writer lock that
//!   other processes wait for, so a caller writes what a phase learned at once, never one entry per transaction in a
//!   loop (`sync_core::cache_memo::CacheMemo`). Only a test turns the sync off (`CacheConfig::sync_commits`).
//! - **Every failure is a miss**: it is logged at debug level (an environment that cannot open at warning level) and
//!   never fails the caller.

use serde::Serialize;
use serde::de::DeserializeOwned;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
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
  FileLeftovers,
}

/// How many stores there are
const STORE_COUNT: usize = 19;

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
  Store::FileLeftovers,
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
      // 4: the lines of the blocks of the diff views (`file_diffs::patch`); 5: those blocks with the empty last line of a
      // text paired only with its like (`line_diff::provider::compare_for_view`); 6: those blocks without a deadline,
      // never cut short by the time of a view (`file_diffs::blocks`)
      Self::CommitLines => 6,
      // `commit_diff` 2: the blob ids of `FileChange`; 3: the hunks pinned as `commit_lines`; 4: the hunks and the
      // counts from the blocks of the diff views (`file_diffs::blocks`); 5: git's counts beside them (`--numstat`); 6: the
      // blocks with the empty last line of a text paired only with its like; 7: the blocks without a deadline
      Self::CommitDiff => 7,
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
      | Self::SessionFrame
      | Self::FileLeftovers => 1,
    }
  }

  pub(crate) const fn environment(self) -> Environment {
    match self {
      Self::FileDiff | Self::FileMerge | Self::FileLeftovers => Environment::Patches,
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
      Self::FileLeftovers => "file_leftovers",
    }
  }
}

#[derive(Debug, Clone)]
pub struct CacheConfig {
  /// The LMDB map size in bytes, rounded up to the page size: the cap of the cache
  pub map_size: usize,
  /// The map size of the patch environment (`file_diff`, `file_merge`, `file_leftovers`)
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

/// The environments of a cache and the table of each store in its environment
struct Inner {
  /// By `Environment`, in the order of `ENVIRONMENTS`; None for one that could not be opened, whose stores miss
  envs: [Option<lmdb_env::Store>; ENVIRONMENTS.len()],
  /// The table of each store, by `Store::index`; None for the stores of such an environment
  tables: [Option<lmdb_env::Table>; STORE_COUNT],
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
    let checked = &CHECKED_VERSIONS[store.index()];
    // the high bit tells a checked version from the initial 0
    let stamp = u64::from(version) | 1 << 63;
    if checked.load(Ordering::Relaxed) == stamp {
      return;
    }
    let Some((env, table)) = self.inner.as_ref().and_then(|inner| inner.store(store)) else {
      return;
    };
    let meta = version.to_be_bytes();
    let result = if env.get(table, VERSION_KEY).as_deref() == Some(meta.as_slice()) {
      Ok(())
    } else {
      debug!(store = store.base_name(), version, "clearing the entries of another version");
      env.clear_table(table).and_then(|()| env.put(table, &[(VERSION_KEY.to_vec(), meta.to_vec())]))
    };
    match result {
      Ok(()) => checked.store(stamp, Ordering::Relaxed),
      Err(e) => debug!(error = %e, store = store.base_name(), "cache version check failed"),
    }
  }

  /// The values of `keys`, in their order; a missing or unreadable value is `None`
  pub(crate) fn get_many<V: DeserializeOwned>(&self, store: Store, keys: &[Vec<u8>]) -> Vec<Option<V>> {
    let Some((env, table)) = self.inner.as_ref().and_then(|inner| inner.store(store)) else {
      return keys.iter().map(|_| None).collect();
    };
    let keys: Vec<&[u8]> = keys.iter().map(Vec::as_slice).collect();
    env
      .get_all(table, &keys)
      .into_iter()
      .map(|bytes| {
        bytes.and_then(|bytes| match postcard::from_bytes(&bytes) {
          Ok(value) => Some(value),
          Err(e) => {
            debug!(error = %e, store = store.base_name(), "unreadable cache value");
            None
          }
        })
      })
      .collect()
  }

  /// Whether `store` has a value for each of `keys`, in their order, without decoding one; a failed read is false
  pub(crate) fn contains_many(&self, store: Store, keys: &[Vec<u8>]) -> Vec<bool> {
    let Some((env, table)) = self.inner.as_ref().and_then(|inner| inner.store(store)) else {
      return vec![false; keys.len()];
    };
    let keys: Vec<&[u8]> = keys.iter().map(Vec::as_slice).collect();
    env.contains_all(table, &keys)
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

  /// Write `entries` in one transaction (`put_encoded`)
  pub(crate) fn put_many<V: Serialize>(&self, store: Store, entries: &[(Vec<u8>, &V)]) {
    if self.inner.is_none() || entries.is_empty() {
      return;
    }
    match entries
      .iter()
      .map(|(key, value)| postcard::to_allocvec(value).map(|bytes| (key.clone(), bytes)))
      .collect::<Result<Vec<_>, _>>()
    {
      Ok(encoded) => self.put_encoded(store, &encoded),
      Err(e) => debug!(error = %e, store = store.base_name(), "cache value not encodable"),
    }
  }

  /// `put_many` of values encoded already (`postcard`), in one transaction: a batch that would fill the map, or meets
  /// `MDB_MAP_FULL`, clears every store of the environment first and is written once more (`lmdb_env::Store::put`)
  pub(crate) fn put_encoded(&self, store: Store, encoded: &[(Vec<u8>, Vec<u8>)]) {
    let Some((env, table)) = self.inner.as_ref().and_then(|inner| inner.store(store)) else {
      return;
    };
    if encoded.is_empty() {
      return;
    }
    if let Err(e) = env.put(table, encoded) {
      debug!(error = %e, store = store.base_name(), "cache write failed");
    }
  }
}

impl Inner {
  /// The environment and the table of `store`; None when its environment could not be opened
  fn store(&self, store: Store) -> Option<(&lmdb_env::Store, lmdb_env::Table)> {
    Some((self.envs[store.environment() as usize].as_ref()?, self.tables[store.index()]?))
  }
}

/// The names dropped from `environment` on open: the retired stores (`RETIRED_STORES`) and the stores of another
/// environment, which a binary of another layout kept here
fn retired_in(environment: Environment) -> Vec<&'static str> {
  let moved = STORES.iter().filter(|store| store.environment() != environment).map(|store| store.base_name());
  RETIRED_STORES.iter().copied().chain(moved).collect()
}

/// Each environment on its own: one that cannot be opened misses its stores; none, and the cache is disabled
fn open_inner(dir: &Path, config: &CacheConfig) -> Result<Inner, String> {
  let mut envs: [Option<lmdb_env::Store>; ENVIRONMENTS.len()] = std::array::from_fn(|_| None);
  let mut tables: [Option<lmdb_env::Table>; STORE_COUNT] = [None; STORE_COUNT];
  for environment in ENVIRONMENTS {
    let env_dir = environment.dir(dir);
    let stores: Vec<Store> = STORES.iter().copied().filter(|store| store.environment() == environment).collect();
    let names: Vec<(&str, u32)> = stores.iter().map(|store| (store.base_name(), config.versions[store.index()])).collect();
    let env_config = lmdb_env::Config {
      map_size: environment.map_size(config),
      sync: config.sync_commits,
      ..lmdb_env::Config::default()
    };
    match lmdb_env::Store::open(&env_dir, &env_config, &names, &retired_in(environment)) {
      Ok(env) => {
        for (index, store) in stores.iter().enumerate() {
          tables[store.index()] = env.table(index);
        }
        envs[environment as usize] = Some(env);
      }
      Err(e) => warn!(error = %e, dir = %env_dir.display(), "an environment of the object cache cannot be opened: its stores miss"),
    }
  }
  if envs.iter().all(Option::is_none) {
    return Err("no environment of the object cache can be opened".to_string());
  }
  Ok(Inner { envs, tables })
}
