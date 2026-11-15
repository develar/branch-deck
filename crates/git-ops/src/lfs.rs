//! Git LFS around the moves of a working tree (docs/design/git-operations.md, "Git LFS"): what its config says
//! (`LfsConfig`: whether its smudge filter downloads, where it keeps its objects), and the fetch, before the repository
//! lock, of the objects that a move of a working tree needs and the store lacks (`prefetch_lfs_objects`), so the smudge
//! filter of the move under the lock reads the store only. A repository without objects of LFS costs no process.

use crate::attributes::read_attributes;
use crate::lease_slot::linked_worktree;
use crate::object_texts::read_blob_bytes;
use crate::paths::changed_path_names;
use crate::remote_command::{LFS_IDLE, execute_remote_command, execute_remote_command_while_progressing};
use crate::revs::{git_common_dir, known_dirs};
use anyhow::{Context, Result};
use git_executor::git_command_executor::GitCommandExecutor;
use std::path::{Path, PathBuf};
use std::time::Duration;
use sync_utils::git_bool::parse_git_bool;
use tracing::{debug, instrument, warn};

/// The largest pointer file of Git LFS (its specification): a larger blob is content
const POINTER_MAX_BYTES: usize = 1024;

/// The first line of a pointer file of Git LFS
const POINTER_VERSION: &str = "version https://git-lfs.github.com/spec/v1";

/// The longest `--include` of one fetch: a longer list is fetched in several, each of at most this many characters, so
/// the command line stays far from the 32K characters of Windows
const MAX_INCLUDE_CHARS: usize = 8 * 1024;

/// Git LFS as the config tells it
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LfsConfig {
  /// The smudge filter downloads: `filter.lfs.process`, else `filter.lfs.smudge`, names a command without `--skip`
  /// (`git lfs install --skip-smudge`), and the environment that git inherits has no `GIT_LFS_SKIP_SMUDGE` that is true.
  /// A new slot of a lease pool is checked out with the pointers then (`lease_slot::LfsFiles::Pointers`), and the moves of
  /// a working tree fetch what they need before the repository lock
  pub smudge: bool,
  /// `lfs.storage`, `~` expanded: where Git LFS keeps its objects instead of `$GIT_COMMON_DIR/lfs`
  pub storage: Option<String>,
}

impl LfsConfig {
  /// The objects of Git LFS of the repository whose `$GIT_COMMON_DIR` is `git_common_dir`: `objects` of `lfs.storage`
  /// (relative to `git_common_dir`), else of `$GIT_COMMON_DIR/lfs`. Git LFS creates it with the first object it stores,
  /// so a repository without files of LFS has none, also where the filter is configured for every repository (the
  /// system config of Git for Windows)
  pub fn objects(&self, git_common_dir: &Path) -> PathBuf {
    self
      .storage
      .as_deref()
      .map_or_else(|| git_common_dir.join("lfs"), |storage| git_common_dir.join(storage))
      .join("objects")
  }
}

/// The keys of `LfsConfig` as one `config --get-regexp` lists them, the last value of each winning: `LFS_KEYS` in the
/// regular expression of the read
#[derive(Debug, Default)]
pub(crate) struct LfsKeys {
  process: Option<String>,
  smudge: String,
  storage: Option<String>,
}

/// The alternatives of the keys of `LfsKeys` in the regular expression of a `config --get-regexp`
pub(crate) const LFS_KEYS: &str = r"filter\.lfs\.process|filter\.lfs\.smudge|lfs\.storage";

impl LfsKeys {
  /// Take `key` (as `config --get-regexp` prints it, lowercase) with its value; `false` for another key
  pub(crate) fn read(&mut self, key: &str, value: &str) -> bool {
    match key {
      "filter.lfs.process" => self.process = Some(value.to_string()),
      "filter.lfs.smudge" => value.clone_into(&mut self.smudge),
      "lfs.storage" => self.storage = Some(value.to_string()).filter(|storage| !storage.is_empty()),
      _ => return false,
    }
    true
  }

  pub(crate) fn finish(self) -> LfsConfig {
    // a `process`, even an empty one, is what git runs instead of the command of each file (`apply_filter` of convert.c)
    let command = self.process.unwrap_or(self.smudge);
    LfsConfig {
      smudge: !command.is_empty() && !command.split_whitespace().any(|word| word == "--skip") && !true_in_environment("GIT_LFS_SKIP_SMUDGE"),
      storage: self.storage,
    }
  }
}

/// `lfs.storage` as an operation read it with the rest of its config (`sync_core::config_snapshot::ConfigSnapshot`), a
/// fact of its executor (`GitCommandExecutor::with_fact`): the check of the store before a move takes the objects there,
/// with no process. Without the fact the check takes `$GIT_COMMON_DIR/lfs/objects`, and a repository whose store is
/// elsewhere is fetched for by no move
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LfsStorage(pub Option<String>);

/// A move of a working tree that its files follow under the repository lock (`read-tree -m -u`): `worktree` from the
/// commit `from` to the commit `to`, on `branch` (`None` detached)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeMove {
  pub worktree: String,
  pub from: String,
  pub to: String,
  pub branch: Option<String>,
}

/// Fetch, before the repository lock, the objects of Git LFS that `moves` need and the store lacks, so the smudge
/// filter of their moves under the lock reads the store only. Nothing without `$GIT_COMMON_DIR/lfs/objects` (or the
/// `objects` of `LfsStorage`, a fact of the executor), no process then; else one `diff-tree` of each move, one `check-attr` of its changed paths at `to` and one `cat-file` of the
/// pointers of those that the filter `lfs` converts. Only when an object is missing: one `config` (whether the smudge
/// downloads, `lfs.storage`, the remote) and `git lfs fetch --include <the paths> <remote> <to>` under the policy of a
/// remote command, one per `MAX_INCLUDE_CHARS` of paths (`include_lists`), each killed after `LFS_IDLE` without a line
/// of progress or when `canceled` says so (the Stop Waiting of the app's operation). A failed fetch is the error of the
/// move, before its lock, nothing written, but where the repository skips download errors (`skips_download_errors`):
/// there the move under the lock downloads nothing (`downloads_held_back`), its smudge writes the pointers of what the
/// store lacks
///
/// # Errors
/// A read of git, the fetch
#[instrument(skip(git_executor, moves, canceled))]
pub fn prefetch_lfs_objects(git_executor: &GitCommandExecutor, moves: &[WorktreeMove], canceled: &dyn Fn() -> bool) -> Result<()> {
  for worktree_move in moves {
    prefetch_move(git_executor, worktree_move, canceled)?;
  }
  Ok(())
}

fn prefetch_move(git_executor: &GitCommandExecutor, worktree_move: &WorktreeMove, canceled: &dyn Fn() -> bool) -> Result<()> {
  let WorktreeMove { worktree, from, to, branch } = worktree_move;
  forget_held_back(worktree_move);
  let common_dir = common_dir_of(git_executor, worktree)?;
  let known = LfsConfig {
    storage: git_executor.fact::<LfsStorage>().and_then(|storage| storage.0.clone()),
    ..LfsConfig::default()
  };
  let known_objects = known.objects(&common_dir);
  if !known_objects.is_dir() {
    return Ok(());
  }
  let pointers = changed_pointers(git_executor, worktree, from, to)?;
  if missing(&pointers, &known_objects).is_empty() {
    return Ok(());
  }
  let config = fetch_config(git_executor, worktree, branch.as_deref())?;
  if !config.lfs.smudge {
    debug!("the smudge filter of Git LFS does not download: no fetch before the lock");
    return Ok(());
  }
  let objects = config.lfs.objects(&common_dir);
  let missing = missing(&pointers, &objects);
  if missing.is_empty() {
    return Ok(());
  }
  debug!(objects = missing.len(), remote = %config.remote, "fetching the objects of Git LFS of the move before the lock");
  let mut failed = false;
  for include in include_lists(&missing) {
    let args = ["lfs", "fetch", "--include", include.as_str(), config.remote.as_str(), to.as_str()];
    match execute_remote_command_while_progressing(git_executor, &args, worktree, idle(), canceled) {
      Ok(()) => {}
      // the smudge writes the pointers of what it can not download, as the repository asks
      Err(error) if skips_download_errors(git_executor, worktree) => {
        warn!(error = %format!("{error:#}"), "cannot fetch the files of Git LFS before the move; lfs.skipdownloaderrors: the move goes on");
        failed = true;
      }
      Err(error) => return Err(error.context(format!("Cannot fetch the files of Git LFS of {to} before the move"))),
    }
  }
  if failed {
    hold_back_downloads(worktree_move);
  }
  Ok(())
}

/// A move whose smudge under the lock downloads nothing: of a worktree (canonical, as the move names it otherwise), from
/// the commit `from`. Not by the commit it moves to: the finish of a resolve session moves to the commit it replays,
/// which its fetch before the lock could not know, and read the upstream tip instead
#[derive(PartialEq, Eq)]
struct HeldBack {
  worktree: PathBuf,
  from: String,
}

impl HeldBack {
  fn of(worktree: &str, from: &str) -> Self {
    Self {
      worktree: std::fs::canonicalize(worktree).unwrap_or_else(|_| PathBuf::from(worktree)),
      from: from.to_string(),
    }
  }
}

/// The moves whose fetch before the lock failed where the repository skips download errors: written by the fetch
/// (`prefetch_move`), taken by the move under the lock (`downloads_held_back`), kept in the process between the two; at
/// most `HELD_BACK_LIMIT`, the oldest dropped, for a move that never came
static HELD_BACK: std::sync::Mutex<Vec<HeldBack>> = std::sync::Mutex::new(Vec::new());

const HELD_BACK_LIMIT: usize = 64;

fn held_back() -> std::sync::MutexGuard<'static, Vec<HeldBack>> {
  HELD_BACK.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// What an earlier fetch of the same move held back, forgotten: this fetch decides again
fn forget_held_back(worktree_move: &WorktreeMove) {
  let key = HeldBack::of(&worktree_move.worktree, &worktree_move.from);
  held_back().retain(|held| *held != key);
}

fn hold_back_downloads(worktree_move: &WorktreeMove) {
  let key = HeldBack::of(&worktree_move.worktree, &worktree_move.from);
  let mut held = held_back();
  held.retain(|held| *held != key);
  if held.len() >= HELD_BACK_LIMIT {
    held.remove(0);
  }
  held.push(key);
}

/// Whether the move of `worktree` from the commit `from` downloads nothing under the lock, taken: its fetch before the
/// lock failed where the repository skips download errors. Its `read-tree -m -u` then runs with `-c lfs.fetchexclude=*`:
/// the smudge reads every object the store has, the fetch put there all it could get, and writes the pointer of the
/// rest, as a failed download would, with no network
pub(crate) fn downloads_held_back(worktree: &str, from: &str) -> bool {
  let mut held = held_back();
  if held.is_empty() {
    return false;
  }
  let key = HeldBack::of(worktree, from);
  let before = held.len();
  held.retain(|held| *held != key);
  held.len() != before
}

/// Whether `name` is set in the environment to a value git reads as true
fn true_in_environment(name: &str) -> bool {
  std::env::var(name).is_ok_and(|value| parse_git_bool(Some(&value)) == Some(true))
}

/// Whether the repository lives on the pointers of the objects Git LFS can not download: `lfs.skipdownloaderrors`, or
/// `GIT_LFS_SKIP_DOWNLOAD_ERRORS` true, so its smudge writes a pointer rather than fail. Read only after a fetch or a pull
/// failed, then the operation goes on without the object: one `config --type=bool`, which reads a key without a value
/// as true
pub fn skips_download_errors(git_executor: &GitCommandExecutor, worktree: &str) -> bool {
  true_in_environment("GIT_LFS_SKIP_DOWNLOAD_ERRORS")
    || git_executor
      .cmd(&["config", "--type=bool", "--get", "lfs.skipdownloaderrors"], worktree)
      .accept(&[1])
      .text()
      .is_ok_and(|value| value == "true")
}

/// `LFS_IDLE`, or the milliseconds of `BD_TEST_LFS_IDLE_MS`, which a test of the idle deadline sets
fn idle() -> Duration {
  std::env::var("BD_TEST_LFS_IDLE_MS")
    .ok()
    .and_then(|millis| millis.parse().ok())
    .map_or(LFS_IDLE, Duration::from_millis)
}

/// `$GIT_COMMON_DIR` of `worktree` without a process when the executor knows it (`with_dirs_of`) or the files tell it:
/// the `.git` directory of a main worktree, the `.git` file of a linked one; else `rev-parse`
fn common_dir_of(git_executor: &GitCommandExecutor, worktree: &str) -> Result<PathBuf> {
  if let Some(dirs) = known_dirs(git_executor, worktree) {
    return Ok(PathBuf::from(&dirs.git_common_dir));
  }
  let dot_git = Path::new(worktree).join(".git");
  if dot_git.is_dir() {
    return Ok(dot_git);
  }
  if let Some(linked) = linked_worktree(Path::new(worktree)) {
    return Ok(linked.git_common_dir);
  }
  git_common_dir(git_executor, worktree).map(PathBuf::from)
}

/// A file of Git LFS that a move writes: its path and the object id of its pointer at the target
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChangedPointer {
  pub(crate) path: String,
  pub(crate) oid: String,
}

/// The files that differ between `from` and `to`, that the filter `lfs` converts at `to`, with the pointer `to` has
fn changed_pointers(git_executor: &GitCommandExecutor, worktree: &str, from: &str, to: &str) -> Result<Vec<ChangedPointer>> {
  let Some(paths) = changed_path_names(git_executor, worktree, from, to)? else {
    return Ok(Vec::new());
  };
  let converted: Vec<String> = read_attributes(git_executor, worktree, Some(to), &["filter"], paths.iter().map(String::as_str))?
    .into_iter()
    .filter(|attribute| attribute.value == "lfs")
    .map(|attribute| attribute.path)
    .collect();
  if converted.is_empty() {
    return Ok(Vec::new());
  }
  let names: Vec<String> = converted.iter().map(|path| format!("{to}:{path}")).collect();
  let blobs = read_blob_bytes(git_executor, worktree, &names, POINTER_MAX_BYTES)?;
  Ok(
    converted
      .into_iter()
      .zip(blobs)
      .filter_map(|(path, blob)| pointer_oid(&blob?).map(|oid| ChangedPointer { path, oid }))
      .collect(),
  )
}

/// The object id (sha256, 64 hex digits) of the pointer file `content`; `None` for content that is no pointer
pub(crate) fn pointer_oid(content: &[u8]) -> Option<String> {
  let text = std::str::from_utf8(content).ok()?;
  let mut lines = text.lines();
  if lines.next()? != POINTER_VERSION {
    return None;
  }
  lines
    .find_map(|line| line.strip_prefix("oid sha256:"))
    .filter(|oid| oid.len() == 64 && oid.bytes().all(|byte| byte.is_ascii_hexdigit()))
    .map(str::to_ascii_lowercase)
}

/// The pointers whose object `objects` lacks: `<objects>/<oid[0..2]>/<oid[2..4]>/<oid>`, as Git LFS stores it
fn missing<'a>(pointers: &'a [ChangedPointer], objects: &Path) -> Vec<&'a ChangedPointer> {
  pointers
    .iter()
    .filter(|pointer| !objects.join(&pointer.oid[..2]).join(&pointer.oid[2..4]).join(&pointer.oid).is_file())
    .collect()
}

/// The `--include` lists of the fetch of `missing`: their paths as patterns of Git LFS (`include_pattern`), comma
/// separated, each list of at most `MAX_INCLUDE_CHARS` characters (a longer path is a list of its own)
pub(crate) fn include_lists(missing: &[&ChangedPointer]) -> Vec<String> {
  let mut lists: Vec<String> = Vec::new();
  for pattern in missing.iter().map(|pointer| include_pattern(&pointer.path)) {
    match lists.last_mut() {
      Some(list) if list.len() + 1 + pattern.len() <= MAX_INCLUDE_CHARS => {
        list.push(',');
        list.push_str(&pattern);
      }
      _ => lists.push(pattern),
    }
  }
  lists
}

/// `path` as a pattern of the `--include` of Git LFS, which splits its list at the commas, trims each item and matches
/// it as a wildmatch: a character of a pattern (`*?[]\`) and a leading `!` or `#` escaped by a backslash, a comma and a
/// blank at an end, which no escape keeps, by `?`, the one character there. A path without a slash matches its name in
/// every directory: more, never fewer
pub(crate) fn include_pattern(path: &str) -> String {
  let last = path.chars().count().saturating_sub(1);
  let mut pattern = String::with_capacity(path.len());
  for (index, c) in path.chars().enumerate() {
    match c {
      ',' => pattern.push('?'),
      ' ' | '\t' if index == 0 || index == last => pattern.push('?'),
      '*' | '?' | '[' | ']' | '\\' => {
        pattern.push('\\');
        pattern.push(c);
      }
      '!' | '#' if index == 0 => {
        pattern.push('\\');
        pattern.push(c);
      }
      _ => pattern.push(c),
    }
  }
  pattern
}

/// `git lfs pull` in the worktree `worktree`, under the policy of a remote command: the objects that the store lacks
/// downloaded, and every pointer of HEAD's files replaced by its content, the index refreshed. A new slot of a lease
/// pool, whose checkout under the clones lock wrote the pointers (`lease_slot::LfsFiles::Pointers`), after that lock.
/// Killed after `LFS_IDLE` without a line of progress
///
/// # Errors
/// The pull
pub fn pull_lfs_files(git_executor: &GitCommandExecutor, worktree: &str) -> Result<()> {
  execute_remote_command_while_progressing(git_executor, &["lfs", "pull"], worktree, idle(), &|| false).with_context(|| format!("Cannot pull the files of Git LFS into {worktree}"))
}

/// `git lfs checkout` in the worktree `worktree`: every pointer of HEAD's files whose object the store has replaced by
/// its content, no network. After a `git lfs pull` that failed in part, where the repository skips download errors: what
/// it got, or the store had, is content, the rest stays pointers
///
/// # Errors
/// The checkout
pub fn checkout_lfs_files(git_executor: &GitCommandExecutor, worktree: &str) -> Result<()> {
  // under the policy of a remote command as every command of git-lfs (`network_commands_run_through_remote_command`),
  // though this one asks no remote: its prompts off, its locale C
  execute_remote_command(git_executor, &["lfs", "checkout"], worktree)
    .map(drop)
    .with_context(|| format!("Cannot check out the files of Git LFS in {worktree}"))
}

/// What a fetch of Git LFS reads of the config: `LfsConfig` and the remote Git LFS takes by default
pub(crate) struct FetchConfig {
  pub(crate) lfs: LfsConfig,
  pub(crate) remote: String,
}

/// One `config --get-regexp`: the keys of `LfsConfig`, and the remote as Git LFS picks it by default
/// (`Configuration::Remote` of git-lfs): the remote of `branch`, else `remote.lfsdefault`, else the one remote, else
/// `origin`
pub(crate) fn fetch_config(git_executor: &GitCommandExecutor, worktree: &str, branch: Option<&str>) -> Result<FetchConfig> {
  let pattern = format!(r"^({LFS_KEYS}|remote\.lfsdefault|branch\..*\.remote|remote\..*\.url)$");
  let output = git_executor.cmd(&["config", "--type=path", "--get-regexp", &pattern], worktree).accept(&[1]).output()?;
  let branch_key = branch.map(|branch| format!("branch.{branch}.remote"));
  let mut keys = LfsKeys::default();
  let (mut tracked, mut lfs_default, mut remotes) = (None, None, Vec::new());
  for line in output.into_text().lines() {
    let (key, value) = line.split_once(' ').unwrap_or((line, ""));
    if keys.read(key, value) {
      continue;
    }
    if branch_key.as_deref() == Some(key) && !value.is_empty() && value != "." {
      tracked = Some(value.to_string());
    } else if key == "remote.lfsdefault" && !value.is_empty() {
      lfs_default = Some(value.to_string());
    } else if let Some(name) = key.strip_prefix("remote.").and_then(|rest| rest.strip_suffix(".url"))
      && !remotes.iter().any(|remote| remote == name)
    {
      remotes.push(name.to_string());
    }
  }
  let single = (remotes.len() == 1).then(|| remotes.remove(0));
  Ok(FetchConfig {
    lfs: keys.finish(),
    remote: tracked.or(lfs_default).or(single).unwrap_or_else(|| "origin".to_string()),
  })
}
