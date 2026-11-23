//! The one place Branch Deck writes the user's working tree (docs/design/git-operations.md, the exception to
//! invariant 1): a move of the checked out branch from tree `old` to tree `new`, and the files follow with a two-way
//! `read-tree -m -u`. It updates only the paths whose content differs between the two trees, keeps local changes on
//! every other path, and refuses as a whole, before writing anything, if a path it would update has uncommitted or
//! staged changes or is untracked.
//!
//! `read-tree -m` frees the cache tree of the index it merges, so on the user's index it walks every entry and builds
//! every tree again, whatever the move changes (6 s of CPU on the 1.4M entries of the IntelliJ monorepo). So the merge
//! runs on an index of the paths the move changes (`SmallIndex`, "cut, merge, put back"):
//! 1. the two trees of the changed paths alone, written into a temporary object directory;
//! 2. the cut: the entries of the user's index on those paths (and the `.gitattributes` above them), in a temporary
//!    index, its stat data refreshed;
//! 3. the same `read-tree -m -u` on that index and those trees: git checks every path, then writes the files;
//! 4. the put back: the entries the merge changed go into a copy of the user's index (`update-index --index-info`),
//!    its stat data is refreshed (where a `.gitattributes` of the working tree is not its entry, the entries the merge
//!    wrote once more by the attributes of the merge, as `read-tree -u` records them), and the copy takes the place of
//!    the index.
//!
//! This process holds the index lock of the worktree from the cut to the put back (`IndexLock::hold`), so no git of
//! the user commits the old index over the new files. The whole index merges as before (`read-tree -m -u` of the two
//! trees on the user's index, after a refresh of its stat data) where the small one costs more than it saves, an
//! index file below `SMALL_INDEX_FROM` (its ten processes against two), and where it does not model the index: a
//! sparse checkout, a skip-worktree or unmerged entry, an assume-unchanged entry or a submodule on a path of the
//! move, or a small index that could not be built. The exact dry run of a move (`dry_run_worktree_update`) merges the
//! same small index with `-n`, without the lock and the put back.
//!
//! `crates/git-executor/src/worktree_safety_test.rs` allows `-u` in this file only, and counts the occurrences;
//! `git_usage_scan_test.rs` allows `update-index --index-info`, the second writer of the user's index, here only. The
//! functions are `pub(crate)`: only `update_branch.rs` calls them, for Update Project (the app and `brd update`) and
//! its undo (`brd undo update:`), and through its moves for the finish of a resolve session, `brd pick` and the unapply
//! of a virtual branch (`MOVE_CALLERS` of the scan).

use crate::commit_utils::{blob_id, empty_tree};
use crate::index_lock::{HeldIndexLock, INDEX_LOCK_WAIT, IndexLock, IndexLocked};
use crate::paths::{ChangedPath, changed_paths};
use crate::rewrite_guard::IndexCopy;
use anyhow::{Result, anyhow, bail};
use bstr::ByteSlice;
use git_executor::git_command_executor::GitCommandExecutor;
use std::collections::{HashMap, HashSet};
use std::ops::ControlFlow;
use std::path::Path;
use sync_utils::paths::without_verbatim;
use tracing::{debug, instrument, warn};

/// Mode of a submodule (gitlink): the small index leaves a move of one to the whole index
const GITLINK_MODE: &str = "160000";

/// The size of the index file from which the merge runs on the small index. On the whole index `read-tree -m` costs
/// about 50 ms per megabyte (7 s for the 140 MB of the IntelliJ monorepo); the small index costs eight processes more,
/// 0.3 s on a Mac under load and more on Windows, whatever the index (a repository of 4 300 files, 170 paths: 0.5–0.7 s
/// against 0.2–0.3 s). So a small repository keeps the two processes it had
pub(crate) const SMALL_INDEX_FROM: u64 = 16 * 1024 * 1024;

/// What a small index is built for
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Purpose {
  /// The files of a move: the index lock is held from the cut on
  Move,
  /// The files of a move that was interrupted after HEAD moved, whatever the size of the index: a changed path whose
  /// file has the new content already goes in with its new entry
  Interrupted,
  /// A dry run of the merge: no lock, nothing of the user's is written
  DryRun,
}

/// The directories of the copies of the user's index in its git dir (`SmallIndex::put_back`): a copy must be a file
/// of the file system of the index to take its place by a rename
const INDEX_COPY_PREFIX: &str = "branch-deck-index-";

/// Check that `read-tree -m -u <old_tree> <new_tree>` would succeed, so neither the index nor the working tree is
/// written: on the small index of the move where the index is large (`SMALL_INDEX_FROM`) and it models it, else on a
/// copy of the whole index. Returns git's message if the update is refused.
/// A file that was only touched (same content, new stat data) must not count as changed: the small index is always
/// refreshed; with `refresh_copy` the copy of the whole index is refreshed first; without it the caller refreshed the
/// user's index right before (`prepare_worktree_update`), and the copy has its stat data, so one stat pass serves
/// the dry run and the move
#[instrument(skip(git_executor))]
pub(crate) fn dry_run_worktree_update(git_executor: &GitCommandExecutor, repo_path: &str, old_tree: &str, new_tree: &str, refresh_copy: bool) -> Result<Option<String>> {
  dry_run_from(git_executor, repo_path, old_tree, new_tree, refresh_copy, SMALL_INDEX_FROM)
}

/// `dry_run_worktree_update` that merges the small index from `small_index_from` bytes of the index file on
fn dry_run_from(git_executor: &GitCommandExecutor, repo_path: &str, old_tree: &str, new_tree: &str, refresh_copy: bool, small_index_from: u64) -> Result<Option<String>> {
  let lock = IndexLock::known(git_executor, repo_path);
  match SmallIndex::prepare(git_executor, repo_path, old_tree, new_tree, &lock, Purpose::DryRun, small_index_from) {
    Ok(Some(small)) => return small.dry_run(git_executor, repo_path),
    Ok(None) => {}
    Err(e) => warn!(error = %format!("{e:#}"), "cannot build the index of the changed paths: the dry run merges the whole index"),
  }
  let copy = IndexCopy::of(&lock.git_dir()?.join("index"))?;
  let env = [("GIT_INDEX_FILE", copy.path())];

  if refresh_copy {
    refresh_index(git_executor, repo_path, &env).map_err(|e| anyhow!("Failed to refresh a copy of the index: {e:#}"))?;
  }
  read_tree_dry_run(git_executor, repo_path, &env, old_tree, new_tree)
}

/// `read-tree -m -u -n <old> <new>` on the index of `env`: git's message if it refuses, and nothing is written
fn read_tree_dry_run(git_executor: &GitCommandExecutor, repo_path: &str, env: &[(&str, &str)], old: &str, new: &str) -> Result<Option<String>> {
  let dry_run = git_executor
    .cmd(&["read-tree", "-m", "-u", "-n", "--no-recurse-submodules", old, new], repo_path)
    .env(env)
    .status()?;
  if dry_run.code == 0 {
    return Ok(None);
  }
  debug!(output = %dry_run.stderr, "working tree update dry run refused");
  Ok(Some(dry_run.stderr))
}

/// Refresh the stat data of the user's index before a move whose files follow on the whole index, so `read-tree -m -u`
/// takes a file that was only touched as unchanged: before the branch moves, so a wait for the index lock of another
/// process (`IndexLock::retry`) waits while nothing has moved
#[instrument(skip(git_executor, lock))]
pub(crate) fn prepare_worktree_update(git_executor: &GitCommandExecutor, repo_path: &str, lock: &IndexLock<'_>) -> Result<()> {
  lock.retry(INDEX_LOCK_WAIT, || refresh_index(git_executor, repo_path, &[]))
}

/// The files followed a move by the small index, its index did not, and the files could not be taken back: the mark of
/// the move stays, and the next update, pick or undo finishes it (`update_branch::finish_pending_update`)
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("The files followed, the index did not ({index}), and the files could not be taken back ({files}). The next update or pick of this worktree finishes the move")]
pub struct IndexLeftBehind {
  /// Why the index could not follow
  pub index: String,
  /// Why the files could not be taken back
  pub files: String,
}

/// The files of a move, prepared before the branch moves (`prepare`) and written after it (`apply`): by the small
/// index, which holds the index lock of the worktree from here on, or on the whole index
pub(crate) struct WorktreeUpdate<'a> {
  git_executor: &'a GitCommandExecutor,
  repo_path: &'a str,
  old_tree: &'a str,
  new_tree: &'a str,
  lock: &'a IndexLock<'a>,
  small: Option<SmallIndex>,
}

impl<'a> WorktreeUpdate<'a> {
  /// Before the branch moves from `old_tree` to `new_tree` (commits or trees): the small index of the move with the
  /// index lock held, or, where it does not apply (an index file below `SMALL_INDEX_FROM`, an index it does not
  /// model), the wait for the lock of another process and the refresh of the stat data of the user's index (not when
  /// the check of the move `refreshed` it under the same lock). Nothing of the user's is written. `IndexLocked` when
  /// the lock of another process stays for the wait
  #[instrument(skip(git_executor, lock))]
  pub(crate) fn prepare(git_executor: &'a GitCommandExecutor, repo_path: &'a str, old_tree: &'a str, new_tree: &'a str, lock: &'a IndexLock<'a>, refreshed: bool) -> Result<Self> {
    Self::prepare_from(git_executor, repo_path, old_tree, new_tree, lock, refreshed, SMALL_INDEX_FROM)
  }

  /// `prepare` that merges the small index from `small_index_from` bytes of the index file on
  fn prepare_from(
    git_executor: &'a GitCommandExecutor,
    repo_path: &'a str,
    old_tree: &'a str,
    new_tree: &'a str,
    lock: &'a IndexLock<'a>,
    refreshed: bool,
    small_index_from: u64,
  ) -> Result<Self> {
    let small = match SmallIndex::prepare(git_executor, repo_path, old_tree, new_tree, lock, Purpose::Move, small_index_from) {
      Ok(small) => small,
      Err(e) if e.downcast_ref::<IndexLocked>().is_some() => return Err(e),
      Err(e) => {
        warn!(error = %format!("{e:#}"), "cannot build the index of the changed paths: the whole index merges");
        None
      }
    };
    if small.is_none() {
      lock.wait_while_locked(INDEX_LOCK_WAIT)?;
      if !refreshed {
        prepare_worktree_update(git_executor, repo_path, lock)?;
      }
    }
    Ok(Self {
      git_executor,
      repo_path,
      old_tree,
      new_tree,
      lock,
      small,
    })
  }

  /// `prepare` on the whole index, whatever the move: after the exact check of an interrupted move that the small
  /// index could not finish (`update_branch::files_follow`), and what a test compares the small index with
  pub(crate) fn prepare_whole(git_executor: &'a GitCommandExecutor, repo_path: &'a str, old_tree: &'a str, new_tree: &'a str, lock: &'a IndexLock<'a>) -> Result<Self> {
    prepare_worktree_update(git_executor, repo_path, lock)?;
    Ok(Self {
      git_executor,
      repo_path,
      old_tree,
      new_tree,
      lock,
      small: None,
    })
  }

  /// The files of a move from `old_tree` to `new_tree` that was interrupted after HEAD moved: some files may have the
  /// new content already while the index has the old entries (a crash between the files and the index of the small
  /// index, or in the middle of the files). The small index takes such a path as followed, so the merge writes the
  /// rest and the index follows. `None` where the small index does not apply: the whole index decides
  #[instrument(skip(git_executor, lock))]
  pub(crate) fn prepare_interrupted(
    git_executor: &'a GitCommandExecutor,
    repo_path: &'a str,
    old_tree: &'a str,
    new_tree: &'a str,
    lock: &'a IndexLock<'a>,
  ) -> Result<Option<Self>> {
    Ok(
      SmallIndex::prepare(git_executor, repo_path, old_tree, new_tree, lock, Purpose::Interrupted, 0)?.map(|small| Self {
        git_executor,
        repo_path,
        old_tree,
        new_tree,
        lock,
        small: Some(small),
      }),
    )
  }

  /// Whether the files follow by the small index
  #[cfg(test)]
  pub(crate) const fn is_small(&self) -> bool {
    self.small.is_some()
  }

  /// Update the index and the working tree from `old_tree` (the tree of the commit HEAD pointed at) to `new_tree`
  /// (HEAD's tree now; for an undo, the tree before the update). Must follow a passed check of the files (the exact
  /// `dry_run_worktree_update`, or for a pick and a reapply the check by the paths, `blocking_paths_by_preview`, with
  /// the dry run only where that finds a path) and the branch move: git checks every path again before it writes any
  /// file, so a file that the check missed or that changed since fails the update with nothing written, and the
  /// caller moves the branch back (`FilesRefused`). `read-tree -u` records the stat data of every file it writes; the
  /// small index refreshes the copy of the user's index it puts back. On the whole index a lock of another process
  /// waits (`IndexLock::retry`). Where the fetch of the objects of Git LFS before the lock failed and the repository
  /// skips download errors (`lfs::downloads_held_back`), with `-c lfs.fetchexclude=*`: its smudge filter downloads
  /// nothing under the lock. `IndexLeftBehind` when the files followed and the index could not: the branch stays
  #[instrument(skip(self), fields(repo_path = self.repo_path, old_tree = self.old_tree, new_tree = self.new_tree, small = self.small.is_some()))]
  pub(crate) fn apply(self) -> Result<()> {
    let hold_back = crate::lfs::downloads_held_back(self.repo_path, self.old_tree);
    match self.small {
      Some(small) => small.apply(self.git_executor, self.repo_path, hold_back),
      None => self.lock.retry(INDEX_LOCK_WAIT, || {
        read_tree_update(self.git_executor, self.repo_path, &[], hold_back, self.old_tree, self.new_tree)
      }),
    }
  }
}

/// `read-tree -m -u <old> <new>` on the index of `env` (the user's without one): the one command that writes the
/// working tree. A refusal is an error with git's message, and nothing was written
fn read_tree_update(git_executor: &GitCommandExecutor, repo_path: &str, env: &[(&str, &str)], hold_back_lfs: bool, old: &str, new: &str) -> Result<()> {
  let read_tree = ["read-tree", "-m", "-u", "--no-recurse-submodules", old, new];
  let args: Vec<&str> = if hold_back_lfs {
    ["-c", "lfs.fetchexclude=*"].into_iter().chain(read_tree).collect()
  } else {
    read_tree.to_vec()
  };
  let output = git_executor.cmd(&args, repo_path).env(env).status()?;
  if output.code != 0 {
    bail!("{}{}", String::from_utf8_lossy(&output.stdout), output.stderr);
  }
  Ok(())
}

/// `update-index -q --refresh` of the index of `env` (the user's without one): with `-q`, modified files are not an
/// error, only a failure to write the index is
fn refresh_index(git_executor: &GitCommandExecutor, repo_path: &str, env: &[(&str, &str)]) -> Result<()> {
  let output = git_executor.cmd(&["update-index", "-q", "--refresh"], repo_path).env(env).status()?;
  if output.code != 0 {
    bail!("Failed to refresh the index: {}{}", String::from_utf8_lossy(&output.stdout), output.stderr);
  }
  Ok(())
}

/// `update-index -z --index-info` of the index of `env`, never the user's index itself: the temporary indexes of the
/// small index and the copy of the user's index that takes its place. `input` is `<mode> <id> <stage>\t<path>\0` per
/// entry, `0 <null id>\t<path>\0` for a path to remove
fn write_entries(git_executor: &GitCommandExecutor, repo_path: &str, env: &[(&str, &str)], input: &[u8]) -> Result<()> {
  git_executor.cmd(&["update-index", "-z", "--index-info"], repo_path).env(env).stdin(input).bytes().map(drop)
}

/// An entry of an index: `ls-files -s` shows it, `update-index --index-info` takes it
#[derive(Debug, Clone, PartialEq, Eq)]
struct Entry {
  mode: String,
  id: String,
  /// As git stores it, not necessarily UTF-8
  path: Vec<u8>,
}

/// `<mode> <id> 0\t<prefix><path>\0`
fn write_entry(mode: &str, id: &str, prefix: &[u8], path: &[u8], input: &mut Vec<u8>) {
  input.extend_from_slice(mode.as_bytes());
  input.push(b' ');
  input.extend_from_slice(id.as_bytes());
  input.extend_from_slice(b" 0\t");
  input.extend_from_slice(prefix);
  input.extend_from_slice(path);
  input.push(0);
}

/// `0 <null id>\t<path>\0`: the removal of `path`
fn write_removal(null_id: &str, path: &[u8], input: &mut Vec<u8>) {
  input.extend_from_slice(b"0 ");
  input.extend_from_slice(null_id.as_bytes());
  input.push(b'\t');
  input.extend_from_slice(path);
  input.push(0);
}

/// Why a path of the user's index is in the cut: the move changes it
const CHANGED: u8 = 1;
/// A directory above a changed path: an entry of that name is a file where the move needs a directory
const ABOVE: u8 = 2;
/// The `.gitattributes` of a directory above a changed path: git reads the attributes of a file it checks out from the
/// index before the working tree
const ATTRIBUTES: u8 = 4;

/// The paths of the user's index the cut takes, by the paths the move changes
struct Wanted<'a> {
  /// `CHANGED`, `ABOVE` and `ATTRIBUTES` by path
  paths: HashMap<Vec<u8>, u8>,
  changed: HashSet<&'a [u8]>,
}

impl<'a> Wanted<'a> {
  fn of(changed: &'a [ChangedPath]) -> Self {
    let mut paths: HashMap<Vec<u8>, u8> = HashMap::new();
    *paths.entry(b".gitattributes".to_vec()).or_default() |= ATTRIBUTES;
    for c in changed {
      *paths.entry(c.path.clone()).or_default() |= CHANGED;
      let mut rest = c.path.as_slice();
      while let Some(slash) = rest.rfind_byte(b'/') {
        let dir = &rest[..slash];
        let kind = paths.entry(dir.to_vec()).or_default();
        if *kind & ABOVE != 0 {
          break;
        }
        *kind |= ABOVE;
        *paths.entry([dir, b"/.gitattributes"].concat()).or_default() |= ATTRIBUTES;
        rest = dir;
      }
    }
    Self {
      paths,
      changed: changed.iter().map(|c| c.path.as_slice()).collect(),
    }
  }

  /// Whether `dir` or a directory above it is a changed path: an entry in `dir` sits where the move puts a file
  fn is_under_changed(&self, dir: &[u8]) -> bool {
    let mut rest = dir;
    loop {
      if self.changed.contains(rest) {
        return true;
      }
      let Some(slash) = rest.rfind_byte(b'/') else { return false };
      rest = &rest[..slash];
    }
  }
}

/// The entries of the user's index the move may read or write, read from `ls-files -s -v -z` as it streams by: every
/// entry is looked at once, only the wanted ones are kept
struct Cut<'a> {
  wanted: &'a Wanted<'a>,
  entries: Vec<Entry>,
  /// The bytes of a record that the chunks so far did not end
  rest: Vec<u8>,
  /// The directory of the entry before, and whether a changed path is that directory or one above it
  dir: Vec<u8>,
  under_changed: bool,
  /// Why the small index does not model this index
  unmodelled: Option<&'static str>,
}

impl<'a> Cut<'a> {
  const fn new(wanted: &'a Wanted<'a>) -> Self {
    Self {
      wanted,
      entries: Vec::new(),
      rest: Vec::new(),
      dir: Vec::new(),
      under_changed: false,
      unmodelled: None,
    }
  }

  /// A chunk of the output: `<tag> <mode> <id> <stage>\t<path>\0` per entry
  fn read(&mut self, chunk: &[u8]) -> Result<ControlFlow<()>> {
    let mut chunk = chunk;
    if !self.rest.is_empty() {
      let Some(end) = chunk.find_byte(0) else {
        self.rest.extend_from_slice(chunk);
        return Ok(ControlFlow::Continue(()));
      };
      let mut record = std::mem::take(&mut self.rest);
      record.extend_from_slice(&chunk[..end]);
      chunk = &chunk[end + 1..];
      if self.entry(&record)?.is_break() {
        return Ok(ControlFlow::Break(()));
      }
    }
    while let Some(end) = chunk.find_byte(0) {
      if self.entry(&chunk[..end])?.is_break() {
        return Ok(ControlFlow::Break(()));
      }
      chunk = &chunk[end + 1..];
    }
    self.rest.extend_from_slice(chunk);
    Ok(ControlFlow::Continue(()))
  }

  fn entry(&mut self, record: &[u8]) -> Result<ControlFlow<()>> {
    let unexpected = || anyhow!("Unexpected ls-files output: {:?}", record.as_bstr());
    let (meta, path) = record.split_once_str("\t").ok_or_else(unexpected)?;
    let mut fields = meta.split_str(" ");
    let (Some(tag), Some(mode), Some(id), Some(stage)) = (fields.next(), fields.next(), fields.next(), fields.next()) else {
      return Err(unexpected());
    };
    if stage != b"0" {
      return Ok(self.stop("an unmerged entry"));
    }
    // `S`, and `s` with assume-unchanged
    if tag.eq_ignore_ascii_case(b"S") {
      return Ok(self.stop("a skip-worktree entry"));
    }
    let dir = path.rfind_byte(b'/').map_or(&b""[..], |slash| &path[..slash]);
    if dir != self.dir.as_slice() {
      self.dir.clear();
      self.dir.extend_from_slice(dir);
      self.under_changed = !dir.is_empty() && self.wanted.is_under_changed(dir);
    }
    let kind = self.wanted.paths.get(path).copied().unwrap_or_default();
    let merged = self.under_changed || kind & (CHANGED | ABOVE) != 0;
    if !merged && kind & ATTRIBUTES == 0 {
      return Ok(ControlFlow::Continue(()));
    }
    // lowercase: assume-unchanged, which the put back would drop
    if merged && tag.first().is_some_and(u8::is_ascii_lowercase) {
      return Ok(self.stop("an assume-unchanged entry on a path of the move"));
    }
    self.entries.push(Entry {
      mode: mode.to_str()?.to_string(),
      id: id.to_str()?.to_string(),
      path: path.to_vec(),
    });
    Ok(ControlFlow::Continue(()))
  }

  const fn stop(&mut self, reason: &'static str) -> ControlFlow<()> {
    self.unmodelled = Some(reason);
    ControlFlow::Break(())
  }
}

/// The move of the files by an index of the paths it changes; see the header of this file
struct SmallIndex {
  /// The temporary indexes and the objects of the trees; removed on drop
  dir: tempfile::TempDir,
  /// The index lock of the worktree, held from the cut to the put back; none for a dry run
  held: Option<HeldIndexLock>,
  /// The entries of the user's index in the small one, as the cut read them
  cut: Vec<Entry>,
  /// The trees of the changed paths: the old side and the new side
  old_tree: String,
  new_tree: String,
  /// The null id, of the length of the ids of the repository
  null_id: String,
}

impl SmallIndex {
  /// The trees, the lock (not for a dry run), the cut and the small index with fresh stat data; `None` where the
  /// index file is smaller than `small_index_from` bytes (whatever its size for an interrupted move), or where the
  /// small index does not model the move or the index (the header of this file)
  fn prepare(git_executor: &GitCommandExecutor, repo_path: &str, old: &str, new: &str, lock: &IndexLock<'_>, purpose: Purpose, small_index_from: u64) -> Result<Option<Self>> {
    let git_dir = lock.git_dir()?;
    if purpose != Purpose::Interrupted && std::fs::metadata(git_dir.join("index")).map_or(0, |index| index.len()) < small_index_from {
      return Ok(None);
    }
    // a sparse checkout reads its patterns from this file: none without it
    if git_dir.join("info").join("sparse-checkout").exists() {
      debug!("a sparse checkout: the whole index merges");
      return Ok(None);
    }
    let changed = changed_paths(git_executor, repo_path, old, new)?;
    let Some(first) = changed.first() else { return Ok(None) };
    if changed.iter().any(|c| c.old_mode == GITLINK_MODE || c.new_mode == GITLINK_MODE) {
      debug!("a submodule among the changed paths: the whole index merges");
      return Ok(None);
    }
    let null_id = "0".repeat(first.old_id.len());
    let dir = tempfile::Builder::new().prefix("branchdeck-small-index-").tempdir()?;
    let (old_tree, new_tree) = Self::write_trees(git_executor, repo_path, dir.path(), &changed)?;

    let held = if purpose == Purpose::DryRun { None } else { Some(lock.hold(INDEX_LOCK_WAIT)?) };
    let wanted = Wanted::of(&changed);
    let mut cut = Cut::new(&wanted);
    git_executor.cmd(&["ls-files", "-s", "-v", "-z"], repo_path).stream(|chunk| cut.read(chunk))?;
    if let Some(reason) = cut.unmodelled {
      debug!(reason, "the small index does not model the index: the whole index merges");
      return Ok(None);
    }
    let small = Self {
      dir,
      held,
      cut: cut.entries,
      old_tree,
      new_tree,
      null_id,
    };
    let followed = if purpose == Purpose::Interrupted {
      small.followed(git_executor, repo_path, &changed)?
    } else {
      HashSet::new()
    };
    small.write_index(git_executor, repo_path, &changed, &followed)?;
    Ok(Some(small))
  }

  fn path_of(&self, name: &str) -> Result<String> {
    path_text(&self.dir.path().join(name))
  }

  /// The trees of the changed paths alone, the old side and the new side: one temporary index with the old entries
  /// under `o/` and the new ones under `n/`, one `write-tree` into the temporary object directory, and the two
  /// subtrees of its tree by name; the empty tree for a side without a path (a move that only adds, or only deletes)
  fn write_trees(git_executor: &GitCommandExecutor, repo_path: &str, dir: &Path, changed: &[ChangedPath]) -> Result<(String, String)> {
    let objects = dir.join("objects");
    std::fs::create_dir_all(&objects)?;
    let (index, objects) = (path_text(&dir.join("trees"))?, path_text(&objects)?);
    let mut input = Vec::new();
    let (mut has_old, mut has_new) = (false, false);
    for c in changed {
      if c.status != 'A' {
        has_old = true;
        write_entry(&c.old_mode, &c.old_id, b"o/", &c.path, &mut input);
      }
      if c.status != 'D' {
        has_new = true;
        write_entry(&c.new_mode, &c.new_id, b"n/", &c.path, &mut input);
      }
    }
    write_entries(git_executor, repo_path, &[("GIT_INDEX_FILE", index.as_str())], &input)?;
    // the blobs are in the object directory of the repository, which this one does not know
    let env = [("GIT_INDEX_FILE", index.as_str()), ("GIT_OBJECT_DIRECTORY", objects.as_str())];
    let tree = git_executor.cmd(&["write-tree", "--missing-ok"], repo_path).env(&env).text()?;
    let side = |has: bool, name: &str| if has { format!("{tree}:{name}") } else { empty_tree(&tree).to_string() };
    Ok((side(has_old, "o"), side(has_new, "n")))
  }

  /// The changed paths whose file has the content of the new tree already, or is gone where the new tree has none: an
  /// index of the new entries, refreshed, and the paths it calls modified; a deleted path by a look at the file
  fn followed<'c>(&self, git_executor: &GitCommandExecutor, repo_path: &str, changed: &'c [ChangedPath]) -> Result<HashSet<&'c [u8]>> {
    let index = self.path_of("followed")?;
    let env = [("GIT_INDEX_FILE", index.as_str())];
    let mut input = Vec::new();
    for c in changed.iter().filter(|c| c.status != 'D') {
      write_entry(&c.new_mode, &c.new_id, b"", &c.path, &mut input);
    }
    let modified = if input.is_empty() {
      Vec::new()
    } else {
      write_entries(git_executor, repo_path, &env, &input)?;
      refresh_index(git_executor, repo_path, &env)?;
      git_executor.cmd(&["ls-files", "-m", "-z"], repo_path).env(&env).bytes()?
    };
    let modified: HashSet<&[u8]> = modified.split(|&b| b == 0).filter(|path| !path.is_empty()).collect();
    let root = Path::new(repo_path);
    Ok(
      changed
        .iter()
        .filter(|c| {
          if c.status == 'D' {
            // a path the file system can't name counts as not followed: the merge decides
            c.path.to_path().is_ok_and(|file| root.join(file).symlink_metadata().is_err())
          } else {
            !modified.contains(c.path.as_slice())
          }
        })
        .map(|c| c.path.as_slice())
        .collect(),
    )
  }

  /// The small index: the entries of the cut, a `followed` path that the cut has as the old tree has it with its new
  /// entry, and the stat data of the files
  fn write_index(&self, git_executor: &GitCommandExecutor, repo_path: &str, changed: &[ChangedPath], followed: &HashSet<&[u8]>) -> Result<()> {
    let index = self.path_of("index")?;
    let env = [("GIT_INDEX_FILE", index.as_str())];
    let cut: HashMap<&[u8], &Entry> = self.cut.iter().map(|entry| (entry.path.as_slice(), entry)).collect();
    // by path: the changed path whose new entry the index takes, `None` for one the new tree deletes
    let mut new_entries: HashMap<&[u8], Option<&ChangedPath>> = HashMap::new();
    for c in changed.iter().filter(|c| followed.contains(c.path.as_slice())) {
      let as_old = match cut.get(c.path.as_slice()) {
        Some(entry) => c.status != 'A' && entry.mode == c.old_mode && entry.id == c.old_id,
        None => c.status == 'A',
      };
      if as_old {
        new_entries.insert(c.path.as_slice(), (c.status != 'D').then_some(c));
      }
    }
    let mut input = Vec::new();
    for entry in &self.cut {
      match new_entries.remove(entry.path.as_slice()) {
        None => write_entry(&entry.mode, &entry.id, b"", &entry.path, &mut input),
        Some(Some(c)) => write_entry(&c.new_mode, &c.new_id, b"", &c.path, &mut input),
        Some(None) => {}
      }
    }
    // the followed paths the new tree adds, in the order of the move
    for c in changed {
      if let Some(Some(c)) = new_entries.remove(c.path.as_slice()) {
        write_entry(&c.new_mode, &c.new_id, b"", &c.path, &mut input);
      }
    }
    if input.is_empty() {
      // an index without a file is unborn, and a two-way merge into an unborn index takes every path of the new tree
      return git_executor.cmd(&["read-tree", "--empty"], repo_path).env(&env).bytes().map(drop);
    }
    write_entries(git_executor, repo_path, &env, &input)?;
    refresh_index(git_executor, repo_path, &env)
  }

  /// The merge on the small index without a write: git's message if it refuses
  fn dry_run(&self, git_executor: &GitCommandExecutor, repo_path: &str) -> Result<Option<String>> {
    let (index, objects) = (self.path_of("index")?, self.path_of("objects")?);
    let env = [("GIT_INDEX_FILE", index.as_str()), ("GIT_ALTERNATE_OBJECT_DIRECTORIES", objects.as_str())];
    read_tree_dry_run(git_executor, repo_path, &env, &self.old_tree, &self.new_tree)
  }

  /// The merge on the small index, which writes the files, then the put back. A refusal of the merge wrote nothing. A
  /// put back that fails takes the files back by the merge the other way, so the caller moves the branch back and
  /// nothing changed; `IndexLeftBehind` when that fails too
  fn apply(self, git_executor: &GitCommandExecutor, repo_path: &str, hold_back_lfs: bool) -> Result<()> {
    let (index, objects) = (self.path_of("index")?, self.path_of("objects")?);
    let env = [("GIT_INDEX_FILE", index.as_str()), ("GIT_ALTERNATE_OBJECT_DIRECTORIES", objects.as_str())];
    read_tree_update(git_executor, repo_path, &env, hold_back_lfs, &self.old_tree, &self.new_tree)?;
    let Err(index_error) = self.put_back(git_executor, repo_path, &index) else {
      return Ok(());
    };
    warn!(error = %format!("{index_error:#}"), "the index could not follow the files of a move: the files go back");
    match read_tree_update(git_executor, repo_path, &env, hold_back_lfs, &self.new_tree, &self.old_tree) {
      Ok(()) => Err(index_error),
      Err(files_error) => Err(anyhow::Error::new(IndexLeftBehind {
        index: format!("{index_error:#}"),
        files: format!("{files_error:#}"),
      })),
    }
  }

  /// Put the entries the merge changed into a copy of the user's index, refresh its stat data (the entries
  /// `update-index --index-info` writes have none, and a `git status` would hash their files each time), and put the
  /// copy in the place of the index. The refresh hashes a file by the attributes of the working tree, the merge wrote
  /// it by those of its index: where a `.gitattributes` of the merge is not its file, the entries the merge wrote get
  /// their stat data by the attributes of the merge (`refresh_by_merge_attributes`), as `read-tree -u` records the stat
  /// data of every file it writes. The copy lives in the git dir: a rename needs one file system. The lock is released
  /// when the small index is dropped
  fn put_back(&self, git_executor: &GitCommandExecutor, repo_path: &str, small_index: &str) -> Result<()> {
    let merged = git_executor.cmd(&["ls-files", "-s", "-z"], repo_path).env(&[("GIT_INDEX_FILE", small_index)]).bytes()?;
    // `<mode> <id> <stage>\t<path>` per entry: by path, its mode and id
    let mut merged_entries: HashMap<&[u8], (&[u8], &[u8])> = HashMap::new();
    for record in merged.split(|&b| b == 0).filter(|record| !record.is_empty()) {
      let unexpected = || anyhow!("Unexpected ls-files output: {:?}", record.as_bstr());
      let (meta, path) = record.split_once_str("\t").ok_or_else(unexpected)?;
      let mut fields = meta.split_str(" ");
      let (Some(mode), Some(id)) = (fields.next(), fields.next()) else {
        return Err(unexpected());
      };
      merged_entries.insert(path, (mode, id));
    }
    let attributes_differ = merged_entries
      .iter()
      .any(|(path, (_, id))| (*path == b".gitattributes" || path.ends_with(b"/.gitattributes")) && attributes_file_differs(repo_path, path, id));
    let mut input = Vec::new();
    // the paths whose files the merge wrote
    let mut written: Vec<&[u8]> = Vec::new();
    for entry in &self.cut {
      match merged_entries.remove(entry.path.as_slice()) {
        Some((mode, id)) if mode == entry.mode.as_bytes() && id == entry.id.as_bytes() => {}
        Some((mode, id)) => {
          write_entry(mode.to_str()?, id.to_str()?, b"", &entry.path, &mut input);
          written.push(&entry.path);
        }
        None => write_removal(&self.null_id, &entry.path, &mut input),
      }
    }
    // what the merge added, sorted: the same input for the same move
    let mut added: Vec<_> = merged_entries.into_iter().collect();
    added.sort_unstable();
    for (path, (mode, id)) in added {
      write_entry(mode.to_str()?, id.to_str()?, b"", path, &mut input);
      written.push(path);
    }
    if input.is_empty() {
      // the index had every changed path as the new tree has it
      return Ok(());
    }

    let held = self.held.as_ref().ok_or_else(|| anyhow!("The index lock is not held: a dry run puts nothing back"))?;
    let git_dir = held.git_dir();
    remove_stale_copies(git_dir);
    let copy_dir = tempfile::Builder::new().prefix(INDEX_COPY_PREFIX).tempdir_in(git_dir)?;
    let copy = copy_dir.path().join("index");
    let index_path = held.index_path();
    if index_path.exists() {
      std::fs::copy(&index_path, &copy).map_err(|e| anyhow!("Failed to copy the index: {e}"))?;
    }
    let copy_text = path_text(&copy)?;
    let env = [("GIT_INDEX_FILE", copy_text.as_str())];
    write_entries(git_executor, repo_path, &env, &input)?;
    refresh_index(git_executor, repo_path, &env)?;
    if attributes_differ && !written.is_empty() {
      self.refresh_by_merge_attributes(git_executor, repo_path, small_index, &copy_text, &written)?;
    }
    held.replace_index(&copy)
  }

  /// The stat data of the `written` entries of the index at `copy` by the attributes the merge wrote their files with:
  /// those of the small index, whose tree goes into the temporary object directory (`--attr-source`), on those paths
  /// alone. `add --refresh` matches its paths against every entry of the index (0.9 s for 50 paths in an index of
  /// 1.4M entries), so it runs only where a `.gitattributes` of the working tree is not the one of the merge
  fn refresh_by_merge_attributes(&self, git_executor: &GitCommandExecutor, repo_path: &str, small_index: &str, copy: &str, written: &[&[u8]]) -> Result<()> {
    let objects = self.path_of("objects")?;
    let tree = git_executor
      .cmd(&["write-tree", "--missing-ok"], repo_path)
      .env(&[("GIT_INDEX_FILE", small_index), ("GIT_OBJECT_DIRECTORY", objects.as_str())])
      .text()?;
    let mut paths = Vec::new();
    for path in written {
      paths.extend_from_slice(path);
      paths.push(0);
    }
    git_executor
      .cmd(
        &[
          "--literal-pathspecs",
          "--attr-source",
          &tree,
          "add",
          "--refresh",
          "--pathspec-from-file=-",
          "--pathspec-file-nul",
        ],
        repo_path,
      )
      .env(&[("GIT_INDEX_FILE", copy), ("GIT_ALTERNATE_OBJECT_DIRECTORIES", objects.as_str())])
      .stdin(&paths)
      .bytes()
      .map(drop)
  }
}

/// Whether the `.gitattributes` of the working tree at `path` says other than its entry `id` in the merged index: a
/// file whose bytes are not the blob, with LF or CRLF line ends (git reads a line the same with either, and
/// `core.autocrlf` checks the file out with CRLF), or what git would not read as a file. None at `path`: git reads the
/// entry, as the merge did
fn attributes_file_differs(repo_path: &str, path: &[u8], id: &[u8]) -> bool {
  let (Ok(path), Ok(id)) = (path.to_path(), id.to_str()) else { return true };
  let file = Path::new(repo_path).join(path);
  match std::fs::symlink_metadata(&file) {
    Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
    Ok(metadata) if metadata.is_file() => std::fs::read(&file).map_or(true, |content| {
      blob_id(id, &content) != id && (!content.contains_str("\r\n") || blob_id(id, &content.replace("\r\n", "\n")) != id)
    }),
    _ => true,
  }
}

/// The copies of the index that a killed move left in `git_dir`. The caller holds the index lock, and only its holder
/// makes a copy, so every one there is stale
fn remove_stale_copies(git_dir: &Path) {
  let Ok(entries) = std::fs::read_dir(git_dir) else { return };
  for entry in entries.flatten() {
    if entry.file_name().to_str().is_some_and(|name| name.starts_with(INDEX_COPY_PREFIX))
      && let Err(e) = sync_utils::fs_retry::remove_dir_all(entry.path())
    {
      warn!(path = %entry.path().display(), error = %e, "cannot remove a stale copy of the index");
    }
  }
}

/// `path` as git takes it in a variable
fn path_text(path: &Path) -> Result<String> {
  let path = without_verbatim(path);
  path
    .to_str()
    .map(str::to_string)
    .ok_or_else(|| anyhow!("A temporary path is not valid UTF-8: {}", path.display()))
}

#[cfg(test)]
#[path = "worktree_update_test.rs"]
mod worktree_update_test;
