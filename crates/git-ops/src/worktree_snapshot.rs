//! The tree of the files of a worktree as `git add --all` stages them (the tracked and the untracked files that git does
//! not ignore), built in a copy of its index, so the index, the files and the refs of the worktree stay as they are: the
//! scans of a host slot before and after the command of a run (`bd lease host snapshot`, `changes`), and the commit of
//! the working tree of a client (`bd host snapshot --working-tree`, `host_client`). spec/cli/lease.spec.md, "Host
//! runs". The patch between two trees streams into a file, never whole in memory

use crate::lease_slot::UntrackedFiles;
use anyhow::{Context, Result, anyhow, bail};
use git_executor::git_command_executor::GitCommandExecutor;
use std::io::Write as _;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};

/// The most paths of a `status` that the add takes by their paths: a pathspec add matches each path against each entry
/// of the index, so above this count one `add --all` of every path is faster
pub const PATHSPEC_ADD_MAX: usize = 256;

/// A copy of the index of a worktree in a private directory of its git dir (mode 0700, a random name), removed with the
/// lock file git makes beside it when the copy goes: never a name of the shared temporary directory, which a link
/// planted there would redirect
pub struct IndexCopy {
  _dir: tempfile::TempDir,
  path: PathBuf,
}

impl IndexCopy {
  /// A copy of `index` (an APFS clone where it can be) in `git_dir`; `None` when the worktree has no index yet
  pub fn of(git_dir: &Path, index: &Path) -> Result<Option<Self>> {
    if !index.exists() {
      return Ok(None);
    }
    let dir = tempfile::Builder::new()
      .prefix("branch-deck-index-")
      .tempdir_in(git_dir)
      .with_context(|| format!("Cannot make a directory in {}", git_dir.display()))?;
    let path = dir.path().join("index");
    std::fs::copy(index, &path).with_context(|| format!("Cannot copy the index {} to {}", index.display(), path.display()))?;
    Ok(Some(Self { _dir: dir, path }))
  }

  pub fn path(&self) -> &Path {
    &self.path
  }
}

/// One entry of `status --porcelain=v1 -z --no-renames`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusEntry {
  /// `XY`: the index against HEAD, the files against the index; `??` for an untracked file
  pub code: String,
  pub path: String,
}

impl StatusEntry {
  /// `XY path`, as `git status --porcelain` prints it
  pub fn line(&self) -> String {
    format!("{} {}", self.code, self.path)
  }

  /// A staged deletion with no file at the path: the copy of the index has it, and an add of its path would fail
  fn is_staged_deletion(&self) -> bool {
    self.code == "D "
  }
}

/// The entries of `status --porcelain=v1 -z --no-renames`: `XY <path>` each, NUL-terminated; without renames no entry
/// has a second path
pub fn status_entries(output: &[u8]) -> Vec<StatusEntry> {
  output
    .split(|&byte| byte == 0)
    .filter(|entry| entry.len() > 3)
    .map(|entry| StatusEntry {
      code: String::from_utf8_lossy(&entry[..2]).into_owned(),
      path: String::from_utf8_lossy(&entry[3..]).into_owned(),
    })
    .collect()
}

/// Where the status of `worktree_tree` reads from
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusOf {
  /// The copy, which the status may write (`GIT_OPTIONAL_LOCKS=1`): the stat data it refreshes saves the add a second
  /// look at each file
  Copy,
  /// The index of the worktree, read only; the add runs in the copy after it
  Index,
}

/// The tree of a worktree and the entries of its status
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeTree {
  pub tree: String,
  pub entries: Vec<StatusEntry>,
}

/// The tree of the files of `worktree` (whose git dir is `git_dir` and index `index`) as `git add --all` stages them, in
/// a copy of its index: one `status --porcelain=v1 -z --no-renames` with the untracked mode of the repository (its
/// `core.fsmonitor` and `core.untrackedCache` apply, and the copy keeps their token), one add of the paths it named (up
/// to `PATHSPEC_ADD_MAX`, else of every path; again of every path when one went away meanwhile), and one `write-tree`.
/// Without changes the add does not run. A worktree without an index starts from `head` (`read-tree`)
pub fn worktree_tree(git: &GitCommandExecutor, worktree: &str, git_dir: &Path, index: &Path, head: &str, untracked: UntrackedFiles, status_of: StatusOf) -> Result<WorktreeTree> {
  let copy = IndexCopy::of(git_dir, index)?;
  // without an index, a new one in a directory of its own, from the tree of HEAD
  let mut fresh = None;
  let copy_path = if let Some(copy) = &copy {
    copy.path().to_path_buf()
  } else {
    let dir = fresh.insert(tempfile::Builder::new().prefix("branch-deck-index-").tempdir_in(git_dir)?);
    let path = dir.path().join("index");
    let index_env = path.to_string_lossy().into_owned();
    git.cmd(&["read-tree", head], worktree).env(&[("GIT_INDEX_FILE", &index_env)]).output()?;
    path
  };
  let index_env = copy_path.to_str().ok_or_else(|| anyhow!("The index copy is no UTF-8 path: {}", copy_path.display()))?;
  let in_copy = [("GIT_INDEX_FILE", index_env)];
  let status_env: &[(&str, &str)] = match status_of {
    StatusOf::Copy => &[("GIT_INDEX_FILE", index_env), ("GIT_OPTIONAL_LOCKS", "1")],
    StatusOf::Index => &[],
  };
  let status = git
    .cmd(&["status", "--porcelain=v1", "-z", "--no-renames", untracked.arg()], worktree)
    .env(status_env)
    .bytes()?;
  let entries = status_entries(&status);
  let paths: Vec<&str> = entries.iter().filter(|entry| !entry.is_staged_deletion()).map(|entry| entry.path.as_str()).collect();
  if !paths.is_empty() {
    // a path that went away since the status fails the add: then every path once more
    let added = paths.len() <= PATHSPEC_ADD_MAX && {
      let mut pathspec = Vec::new();
      for path in &paths {
        pathspec.extend_from_slice(path.as_bytes());
        pathspec.push(0);
      }
      git
        .cmd(&["--literal-pathspecs", "add", "--all", "--pathspec-from-file=-", "--pathspec-file-nul"], worktree)
        .env(&in_copy)
        .stdin(&pathspec)
        .status()?
        .code
        == 0
    };
    if !added {
      git.cmd(&["add", "--all"], worktree).env(&in_copy).output()?;
    }
  }
  let tree = git.cmd(&["write-tree"], worktree).env(&in_copy).text()?;
  Ok(WorktreeTree { tree, entries })
}

/// The size of the patch `write_tree_patch` wrote
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WrittenPatch {
  pub bytes: u64,
}

/// The patch from the tree `before` to the tree `after` for `git apply` (`diff-tree -r -p --binary --full-index
/// --no-renames`), streamed into a new file beside `target` (created anew, mode 0600, never through a link), then
/// renamed onto `target`
pub fn write_tree_patch(git: &GitCommandExecutor, repo: &str, before: &str, after: &str, target: &Path) -> Result<WrittenPatch> {
  let dir = target.parent().filter(|dir| !dir.as_os_str().is_empty()).unwrap_or_else(|| Path::new("."));
  let mut file = tempfile::Builder::new()
    .prefix(".branch-deck-patch-")
    .tempfile_in(dir)
    .with_context(|| format!("Cannot make a file in {}", dir.display()))?;
  let mut bytes = 0u64;
  git
    .cmd(&["diff-tree", "-r", "-p", "--binary", "--full-index", "--no-renames", before, after], repo)
    .stream(|chunk| {
      file.write_all(chunk).with_context(|| format!("Cannot write the patch {}", file.path().display()))?;
      bytes += chunk.len() as u64;
      Ok(ControlFlow::Continue(()))
    })?;
  file.flush()?;
  let (_, path) = file.keep().map_err(|error| anyhow!("Cannot keep the patch: {error}"))?;
  if let Err(error) = sync_utils::fs_retry::rename(&path, target) {
    let _ = sync_utils::fs_retry::remove_file(&path);
    bail!("Cannot move the patch to {}: {error}", target.display());
  }
  Ok(WrittenPatch { bytes })
}
