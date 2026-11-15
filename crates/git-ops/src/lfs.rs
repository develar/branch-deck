//! Git LFS around the moves of a working tree (docs/design/git-operations.md, "Git LFS"): what its config says
//! (`LfsConfig`: whether its smudge filter downloads, where it keeps its objects), and the fetch, before the repository
//! lock, of the objects that a move of a working tree needs and the store lacks (`prefetch_lfs_objects`), so the smudge
//! filter of the move under the lock reads the store only. A repository without objects of LFS costs no process.

use crate::attributes::read_attributes;
use crate::lease_slot::linked_worktree;
use crate::object_texts::read_blob_bytes;
use crate::paths::changed_path_names;
use crate::remote_command::execute_remote_command;
use crate::revs::{git_common_dir, known_dirs};
use anyhow::{Context, Result};
use git_executor::git_command_executor::GitCommandExecutor;
use std::path::{Path, PathBuf};
use sync_utils::git_bool::parse_git_bool;
use tracing::{debug, instrument};

/// The largest pointer file of Git LFS (its specification): a larger blob is content
const POINTER_MAX_BYTES: usize = 1024;

/// The first line of a pointer file of Git LFS
const POINTER_VERSION: &str = "version https://git-lfs.github.com/spec/v1";

/// The longest `--include` of a fetch: a longer list fetches every object of the commit the store lacks, so the command
/// line stays far from the 32K characters of Windows
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
    let skipped = std::env::var("GIT_LFS_SKIP_SMUDGE").is_ok_and(|value| parse_git_bool(Some(&value)) == Some(true));
    LfsConfig {
      smudge: !command.is_empty() && !command.split_whitespace().any(|word| word == "--skip") && !skipped,
      storage: self.storage,
    }
  }
}

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
/// filter of their moves under the lock reads the store only. Nothing without `$GIT_COMMON_DIR/lfs/objects`, no process
/// then; else one `diff-tree` of each move, one `check-attr` of its changed paths at `to` and one `cat-file` of the
/// pointers of those that the filter `lfs` converts. Only when an object is missing: one `config` (whether the smudge
/// downloads, `lfs.storage`, the remote) and one `git lfs fetch --include=<the paths> <remote> <to>` under the policy of a
/// remote command. A failed fetch is the error of the move, before its lock, nothing written
///
/// # Errors
/// A read of git, the fetch
#[instrument(skip(git_executor, moves))]
pub fn prefetch_lfs_objects(git_executor: &GitCommandExecutor, moves: &[WorktreeMove]) -> Result<()> {
  for worktree_move in moves {
    prefetch_move(git_executor, worktree_move)?;
  }
  Ok(())
}

fn prefetch_move(git_executor: &GitCommandExecutor, worktree_move: &WorktreeMove) -> Result<()> {
  let WorktreeMove { worktree, from, to, branch } = worktree_move;
  let common_dir = common_dir_of(git_executor, worktree)?;
  let default_objects = LfsConfig::default().objects(&common_dir);
  if !default_objects.is_dir() {
    return Ok(());
  }
  let pointers = changed_pointers(git_executor, worktree, from, to)?;
  if missing(&pointers, &default_objects).is_empty() {
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
  let include = include_of(&missing);
  let mut args = vec!["lfs", "fetch"];
  if let Some(include) = &include {
    args.extend(["--include", include.as_str()]);
  }
  args.extend([config.remote.as_str(), to.as_str()]);
  debug!(objects = missing.len(), remote = %config.remote, "fetching the objects of Git LFS of the move before the lock");
  execute_remote_command(git_executor, &args, worktree).with_context(|| format!("Cannot fetch the files of Git LFS of {to} before the move"))?;
  Ok(())
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

/// The `--include` of the fetch of `missing`: their paths, comma separated; `None` (every object of the commit the store
/// lacks) when a path holds a comma or a character of a pattern, which Git LFS would read as one, or the list is long.
/// A path without a slash matches that name in every directory: more, never fewer
pub(crate) fn include_of(missing: &[&ChangedPointer]) -> Option<String> {
  let plain = |path: &str| !path.contains(|c: char| ",*?[]\\!#".contains(c));
  if !missing.iter().all(|pointer| plain(&pointer.path)) {
    return None;
  }
  let include = missing.iter().map(|pointer| pointer.path.as_str()).collect::<Vec<_>>().join(",");
  (include.len() <= MAX_INCLUDE_CHARS).then_some(include)
}

/// `git lfs pull` in the worktree `worktree`, under the policy of a remote command: the objects that the store lacks
/// downloaded, and every pointer of HEAD's files replaced by its content, the index refreshed. A new slot of a lease
/// pool, whose checkout under the clones lock wrote the pointers (`lease_slot::LfsFiles::Pointers`), after that lock
///
/// # Errors
/// The pull
pub fn pull_lfs_files(git_executor: &GitCommandExecutor, worktree: &str) -> Result<()> {
  execute_remote_command(git_executor, &["lfs", "pull"], worktree).with_context(|| format!("Cannot pull the files of Git LFS into {worktree}"))?;
  Ok(())
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
