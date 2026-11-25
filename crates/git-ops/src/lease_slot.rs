//! The git of a lease slot (`brd lease`, openspec/specs/cli/lease/spec.md): a linked worktree of the repository that a pool owns.
//! A slot is pool property, not the user's checkout, so its working tree is written here, and only here, by name:
//! `reset --merge` moves a clean slot to the commit of a get or a run by the diff of the two commits, `checkout
//! --force` writes every file of a new slot (or of a slot whose diff git refused), `clean -fd` clears the untracked
//! files a cloned slot got from its template, `reset --hard` and `clean -fd` discard the changes of a return the agent
//! confirmed, of a give-back, or of an idle lease whose changes are proven patches in its notes
//! (`preserve_slot_changes`, which reads the slot only, on private copies of its index). The worktree safety scan exempts exactly these argument lists in this file
//! (`worktree_safety_test.rs`, "Lease" in docs/design/git-operations.md). Every write of a slot's index waits for a
//! lock of another process and removes one that no process owns (`IndexLock`): a git killed in a slot leaves its
//! `index.lock`, and the next write there would fail on it.

use crate::index_lock::{INDEX_LOCK_WAIT, IndexLock};
use crate::lfs::{LFS_KEYS, LfsConfig, LfsKeys};
use crate::ref_transaction::RefTransaction;
use crate::revs::{is_object_id, read_head};
use anyhow::{Result, anyhow, bail};
use domain_types::landing_route::LandingSetting;
use git_executor::git_command_executor::GitCommandExecutor;
use std::path::{Component, Path, PathBuf};
use sync_utils::fs_retry;
use sync_utils::git_bool::parse_git_bool;

/// A linked worktree, as its `.git` file and its admin directory name it
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkedWorktree {
  /// The worktree id: the name of its directory under `$GIT_COMMON_DIR/worktrees`
  pub id: String,
  /// `$GIT_COMMON_DIR`, canonical
  pub git_common_dir: PathBuf,
  /// `$GIT_DIR` of the worktree, its admin directory, canonical: its `index` lives there
  pub git_dir: PathBuf,
}

/// The git directory of the worktree whose top level is `dir`, as `git rev-parse --git-dir` names it, without a git
/// process: `dir/.git` itself when it is a directory (a main worktree), else what the file `dir/.git` names
/// (`gitdir: <dir>`, relative to `dir`), canonical: the admin directory of a linked worktree, `.git/modules/<name>` of
/// a submodule, the directory of `git init --separate-git-dir`. `None` without either
pub fn git_dir_of(dir: &Path) -> Option<PathBuf> {
  let dot_git = dir.join(".git");
  if dot_git.is_dir() {
    return Some(dot_git);
  }
  named_git_dir(dir)
}

/// The git directory the file `dir/.git` names (`gitdir: <dir>`), canonical
fn named_git_dir(dir: &Path) -> Option<PathBuf> {
  let text = std::fs::read_to_string(dir.join(".git")).ok()?;
  let named = text.lines().next()?.strip_prefix("gitdir:")?.trim();
  std::fs::canonicalize(dir.join(named)).ok()
}

/// The linked worktree whose top level is `dir`, without a git process: `dir/.git` is a file `gitdir: <admin dir>`, the
/// admin dir names the common dir in `commondir` and points back at `dir/.git` in `gitdir`. `None` for anything else: a
/// missing directory, the main worktree, a submodule, a broken link
pub fn linked_worktree(dir: &Path) -> Option<LinkedWorktree> {
  let dot_git = dir.join(".git");
  let admin = named_git_dir(dir)?;
  let common = std::fs::read_to_string(admin.join("commondir")).ok()?;
  let git_common_dir = std::fs::canonicalize(admin.join(common.trim())).ok()?;
  if admin.parent()? != git_common_dir.join("worktrees") {
    return None;
  }
  let back = std::fs::read_to_string(admin.join("gitdir")).ok()?;
  let back = std::fs::canonicalize(admin.join(back.trim())).ok()?;
  if back != std::fs::canonicalize(&dot_git).ok()? {
    return None;
  }
  Some(LinkedWorktree {
    id: admin.file_name()?.to_string_lossy().into_owned(),
    git_common_dir,
    git_dir: admin,
  })
}

/// How a `status` of the pool lists untracked files: in the mode of the repository's `status.showUntrackedFiles` when it
/// is `all`, else `normal`, never `no`, which would hide them. git's untracked cache answers only in the mode it was
/// built for, and git builds it only in the configured mode: in idea-1 (1.4M files, `all` configured) a `status` with
/// `normal` walked every directory in 41 s, with `all` it took 2 s
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum UntrackedFiles {
  #[default]
  Normal,
  All,
}

impl UntrackedFiles {
  /// The option of a `status`
  pub const fn arg(self) -> &'static str {
    match self {
      Self::Normal => "--untracked-files=normal",
      Self::All => "--untracked-files=all",
    }
  }
}

/// The notes of a lease in its slot (`lease_pool::lease_notes`): a link to a directory outside the worktree, which no
/// status of the pool counts as a change, also where `info/exclude` lost the line that ignores it
pub const NOTES_LINK: &str = "notes.local";

/// The directory of the state of Branch Deck in `$GIT_COMMON_DIR`: the pool, its locks, the notes of the leases of a brd
/// before the notes in the pool directory, the object cache, the operations registry; and in the git directory of a
/// worktree, the files of its lease and the copies of its resolve sessions. The one name of it: every path of the state
/// is built from this
pub const STATE_DIR: &str = "branch-deck";

/// The directory of the object cache in `STATE_DIR` of `$GIT_COMMON_DIR`, shared by every worktree of the repository
const OBJECT_CACHE_DIR: &str = "cache";

/// `STATE_DIR` in `git_dir`: `$GIT_COMMON_DIR/branch-deck`, or the git directory of a worktree for its own files
pub fn state_dir(git_dir: &Path) -> PathBuf {
  git_dir.join(STATE_DIR)
}

/// The directory of the files of the remote runs of brd-host in `STATE_DIR` of the git directory of a checkout: its run
/// records and its packs (`host_client`)
pub const HOST_DIR: &str = "host";

/// The run records of the remote runs of a checkout, `<git dir>/branch-deck/host/runs`: `host_client` writes them, and
/// the end of a lease reads which runs of its slot have no end (`lease_pool::host_cancel`)
pub fn host_runs_dir(git_dir: &Path) -> PathBuf {
  state_dir(git_dir).join(HOST_DIR).join("runs")
}

/// The object cache of the repository of `git_common_dir`: `$GIT_COMMON_DIR/branch-deck/cache`
pub fn object_cache_dir(git_common_dir: &Path) -> PathBuf {
  state_dir(git_common_dir).join(OBJECT_CACHE_DIR)
}

/// The directory of the notes of the leases (`lease_pool::lease_notes`), one directory per lease named by its id, with
/// its record `<lease id>.json` beside it: in the pool directory, `<pool dir>/notes`; of a brd before that, in
/// `STATE_DIR` (`legacy_notes_root`)
pub const LEASE_NOTES_DIR: &str = "notes";

/// Where a brd before the notes in the pool directory kept the notes of the leases of the repository of
/// `git_common_dir`: `$GIT_COMMON_DIR/branch-deck/notes`, read and expired there, never written
pub fn legacy_notes_root(git_common_dir: &Path) -> PathBuf {
  state_dir(git_common_dir).join(LEASE_NOTES_DIR)
}

/// Whether `link`, `NOTES_LINK` at the root of a checkout, links to the notes of the lease of that very checkout: its
/// target is absolute, has no `.` or `..`, and names a directory, no link itself, in a `LEASE_NOTES_DIR` directory,
/// whose record beside it (`<name>.json`) names the checkout as its slot. A fetch of brd-host writes through such a
/// link, and through no other one. `git_common_dir`: the repository of the checkout, whose old place of the notes
/// (`legacy_notes_root`) counts too
pub fn links_to_lease_notes(link: &Path, git_common_dir: &Path) -> bool {
  let Ok(target) = std::fs::read_link(link) else {
    return false;
  };
  let plain = target.is_absolute()
    && target
      .components()
      .all(|part| matches!(part, Component::RootDir | Component::Prefix(_) | Component::Normal(_)));
  if !plain || !std::fs::symlink_metadata(&target).is_ok_and(|metadata| metadata.is_dir()) {
    return false;
  }
  let (Some(parent), Some(name), Some(root)) = (target.parent(), target.file_name(), link.parent()) else {
    return false;
  };
  let in_notes = parent.file_name().is_some_and(|dir| dir == LEASE_NOTES_DIR) || parent == legacy_notes_root(git_common_dir);
  let record = parent.join(format!("{}.json", name.to_string_lossy()));
  let slot = std::fs::read(record)
    .ok()
    .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
    .and_then(|record| record.get("slot")?.as_str().map(PathBuf::from));
  in_notes && slot.is_some_and(|slot| matches!((std::fs::canonicalize(slot), std::fs::canonicalize(root)), (Ok(slot), Ok(root)) if slot == root))
}

/// Whether `path` of a `status` is the notes of a lease: the link, or the directory where the system makes no link, a
/// file in it with `--untracked-files=all`
fn is_notes(path: &str) -> bool {
  path.strip_prefix(NOTES_LINK).is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
}

/// The paths of the local changes of `slot`: staged, unstaged and untracked files, not the ignored ones nor the notes
/// of the lease (`NOTES_LINK`, what is in it). One
/// `status --porcelain=v1 -z` with the `untracked` mode of the repository; its own `core.fsmonitor` and
/// `core.untrackedCache` apply. It may write the index (`slot_status`). Only a slot of the pool, with no other process
/// in it, is asked
pub fn slot_changes(git_executor: &GitCommandExecutor, slot: &str, untracked: UntrackedFiles) -> Result<Vec<String>> {
  Ok(changed_paths(slot_status(git_executor, slot, &[untracked.arg()])?))
}

/// The entries of one `status --porcelain=v1 -z` of `slot` with `options` (the `untracked` mode of the repository), a
/// `status` that may write the index (`GIT_OPTIONAL_LOCKS=1`, against the executor's default): the refreshed stat data, the
/// fsmonitor token and the untracked cache stay in the slot's index, so the next `status` and the checkout start warm.
/// Without the write, a slot of idea-1 (1.4M files) idle for days walked its whole tree on every `status`, 2-5 min
/// each; after one write a `status` took 2 s and a checkout without changes 1-3 s instead of 4-10 s. Every `status` of
/// this file runs here
fn slot_status(git_executor: &GitCommandExecutor, slot: &str, options: &[&str]) -> Result<Vec<([u8; 2], String)>> {
  let args = [&["status", "--porcelain=v1", "-z"][..], options].concat();
  let output = git_executor.cmd(&args, slot).env(&[("GIT_OPTIONAL_LOCKS", "1")]).bytes()?;
  Ok(status_entries(&output))
}

/// The paths of the entries of a `status`, without the notes of the lease (`NOTES_LINK`, what is in it)
fn changed_paths(entries: Vec<([u8; 2], String)>) -> Vec<String> {
  entries
    .into_iter()
    .filter(|(code, path)| !(code == b"??" && is_notes(path)))
    .map(|(_, path)| path)
    .collect()
}

/// The entries of a `status --porcelain=v1 -z`: the two status letters and the path (of a rename or a copy, the new one)
fn status_entries(output: &[u8]) -> Vec<([u8; 2], String)> {
  let mut found = Vec::new();
  let mut entries = output.split(|&byte| byte == 0).filter(|entry| !entry.is_empty());
  while let Some(entry) = entries.next() {
    let (Some(&[x, y]), Some(path)) = (entry.get(..2), entry.get(3..)) else { continue };
    found.push(([x, y], String::from_utf8_lossy(path).into_owned()));
    // a rename or a copy names its source in the next entry
    if matches!(x, b'R' | b'C') {
      entries.next();
    }
  }
  found
}

/// What a checkout or a move of the pool runs with (`checkout_slot`, `move_clean_slot`): `checkout.workers=0`, one
/// worker per core, unless the repository sets `checkout.workers` itself. git writes the files with one worker by
/// default
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CheckoutWorkers {
  /// The repository's config sets `checkout.workers`: its value applies
  pub configured: bool,
}

impl CheckoutWorkers {
  /// The `-c` option before the subcommand; none when the repository sets the key
  const fn config(self) -> &'static [&'static str] {
    if self.configured { &[] } else { &["-c", "checkout.workers=0"] }
  }
}

/// What a checkout of the pool writes for a file that the filter of Git LFS (`filter=lfs`) converts (`checkout_slot`)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LfsFiles {
  /// Its content, as git checks it out: the smudge filter of LFS downloads what the store of the repository lacks
  Content,
  /// Its pointer, the blob itself: no filter of LFS runs, so the checkout never waits for the network. The checkout of a
  /// new slot under the clones lock of the pool writes these, and `git lfs pull` writes the content after its release
  /// where the repository keeps objects of LFS (`lfs::pull_lfs_files`); a filter that is `required` would
  /// fail without its command, so it is not then
  Pointers,
}

impl LfsFiles {
  /// The `-c` options before the subcommand
  const fn config(self) -> &'static [&'static str] {
    match self {
      Self::Content => &[],
      Self::Pointers => &["-c", "filter.lfs.process=", "-c", "filter.lfs.smudge=", "-c", "filter.lfs.required=false"],
    }
  }
}

/// Write every file of `slot` at `commit`, detached, or on the new `branch` created there: the checkout of a new slot,
/// which `register_slot` left without an index or a clone brought with the files and the changes of its template, and
/// the fallback of a move that git refused (`move_clean_slot`). It writes only the paths that differ, but it stats every
/// file that `core.fsmonitor` does not vouch for: all of them in a slot without the daemon, 11-25 s in the IntelliJ
/// monorepo (1.4M files). `--force` discards the local changes, which is what a clone needs, and is safe after
/// `slot_changes` found nothing: a case-insensitive file system hides a case-only rename from `status`, and a checkout
/// without it then refuses. `lfs` says what a file of Git LFS gets. A `branch` that exists fails
pub fn checkout_slot(git_executor: &GitCommandExecutor, slot: &str, commit: &str, branch: Option<&str>, workers: CheckoutWorkers, lfs: LfsFiles) -> Result<()> {
  let args = match branch {
    Some(branch) => [workers.config(), lfs.config(), &["checkout", "--quiet", "--force", "-b", branch, commit]].concat(),
    None => [workers.config(), lfs.config(), &["checkout", "--quiet", "--force", "--detach", commit]].concat(),
  };
  IndexLock::of(git_executor, slot).retry(INDEX_LOCK_WAIT, || git_executor.execute_command(&args, slot).map(drop))
}

/// How `move_clean_slot` brought a slot to its commit
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlotMove {
  /// HEAD was at the commit already (a HEAD on a branch is detached there): no file and no index entry written
  Kept,
  /// `reset --merge` moved the detached HEAD: only the paths that differ between the two commits were read and written
  Switched,
  /// git refused the move by the diff, in its words: an untracked file or a local change in a path of the diff. Nothing
  /// moved; the caller checks out every file instead (`checkout_slot`) where it may
  Refused(String),
}

/// A `reset --merge` that git refused (exit 128), as against one that a signal ended or a lock blocked
#[derive(Debug)]
struct MoveRefused(String);

impl std::fmt::Display for MoveRefused {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str(&self.0)
  }
}

impl std::error::Error for MoveRefused {}

/// Move the clean `slot` (its index, HEAD and files agree: a get after its `status`, a host slot after its give-back)
/// to `commit`, detached, by the diff of the two commits. A HEAD on a branch is detached first (the branch survives).
/// A slot at `commit` already costs the `rev-parse` of its HEAD and writes nothing; else `reset --merge`, a two-way
/// merge from the index that reads and writes only the paths of the diff, and stats no other file, with or without
/// fsmonitor: 1.5 s for one file and 6 s for 737 in the IntelliJ monorepo, where a `checkout --force` takes 14-25 s
/// without the daemon. Not `reset --keep`, which then resets the index once more and reads every tree of the commit
/// (`prime_cache_tree`, 7.5 s there). `--merge` keeps a local change outside the diff: the slot must be clean. The
/// index keeps the token of the repository's `core.fsmonitor`, never overridden here: a git that writes the index
/// without it drops the token, and the next `status` walks every file
pub fn move_clean_slot(git_executor: &GitCommandExecutor, slot: &str, commit: &str, workers: CheckoutWorkers) -> Result<SlotMove> {
  let Some((head, branch)) = read_head(git_executor, slot)? else {
    bail!("The slot {slot} has no commit");
  };
  if branch.is_some() {
    detach_head_at(git_executor, slot, &head, "lease move")?;
  }
  if head == commit {
    return Ok(SlotMove::Kept);
  }
  let args = [workers.config(), &["reset", "--quiet", "--merge", commit]].concat();
  let moved = IndexLock::of(git_executor, slot).retry(INDEX_LOCK_WAIT, || {
    let output = git_executor.cmd(&args, slot).status()?;
    match output.code {
      0 => Ok(()),
      128 => Err(MoveRefused(output.stderr).into()),
      code => bail!("git reset --merge {commit} in {slot} exited with {code}: {}", output.stderr),
    }
  });
  match moved {
    Ok(()) => Ok(SlotMove::Switched),
    Err(error) => match error.downcast::<MoveRefused>() {
      Ok(MoveRefused(reason)) => Ok(SlotMove::Refused(reason)),
      Err(error) => Err(error),
    },
  }
}

/// Create the branch `branch` at `commit` and attach the detached HEAD of `slot` there, in one `RefTransaction`: a
/// branch that exists fails it, and HEAD stays detached. Unlike `checkout -b`, it reads no index and no file
pub fn attach_new_branch(git_executor: &GitCommandExecutor, slot: &str, branch: &str, commit: &str) -> Result<()> {
  let full_ref = format!("refs/heads/{branch}");
  RefTransaction::new()
    .create(&full_ref, commit)
    .symref_update_from_id("HEAD", &full_ref, commit)
    .commit(git_executor, slot, "lease get")
}

/// A new slot registered at `path`, detached at `commit`, without its files: `worktree add --force --no-checkout
/// --detach`, which writes only `path/.git` and the admin directory; the files come from `checkout_slot` or from a
/// clone of a template (`lease`). git writes the admin directory in several files, and every `worktree` command reads
/// the admin directories of all worktrees, so the caller runs no other `worktree` command of the pool beside it.
/// `--force` drops a registration of `path` whose directory is gone (a slot deleted by hand), so the path is free
/// again, and overrides nothing else here, `commit` being an id; never a `worktree prune`, which deletes the admin
/// directory that the `worktree add` of a get beside this one has just made, before git locks it.
///
/// `locked` adds `--lock`, until `unlock_slot`: a clone moves to `path` after the `.git` file of the registration, so
/// `path` is away for a moment, and a `worktree prune` beside it (the user's, a gc's) would delete the admin directory
/// of a worktree whose directory is gone
pub fn register_slot(git_executor: &GitCommandExecutor, repository: &str, path: &Path, commit: &str, locked: bool) -> Result<()> {
  let path = path.to_str().ok_or_else(|| anyhow!("The slot path is not UTF-8: {}", path.display()))?;
  let lock: &[&str] = if locked { &["--lock"] } else { &[] };
  let args = [&["worktree", "add", "--quiet", "--force", "--no-checkout"][..], lock, &["--detach", path, commit]].concat();
  git_executor.execute_command(&args, repository)?;
  Ok(())
}

/// The lock of `register_slot` off the slot at `path`, once its files are there; it reads every admin directory, so it
/// runs alone too (`register_slot`)
pub fn unlock_slot(git_executor: &GitCommandExecutor, repository: &str, path: &Path) -> Result<()> {
  let path = path.to_str().ok_or_else(|| anyhow!("The slot path is not UTF-8: {}", path.display()))?;
  git_executor.execute_command(&["worktree", "unlock", path], repository)?;
  Ok(())
}

/// Bring the stat data of the index that `slot` got from its template up to the cloned files. A clone keeps the mtime and
/// the size of each file, but its inodes and ctimes are new. First `cloned_index::patch_cloned_index` writes the stat
/// data of each cloned file whose mtime, size and mode the clone kept into its entry, without a hash (Unix, and only
/// with `template_written`, the mtime of the index of the template before its copy into `git_dir`). Then `update-index
/// -q --refresh` with the default check and `core.fsmonitor=false` checks the entries that the patch left: it hashes
/// each such file and writes its stat data when its content is unchanged. Every later git, the `checkout --force` of the
/// get first, then sees the entries unchanged by their stat data and neither hashes nor writes their files. Without
/// `core.fsmonitor` the index also loses the fsmonitor bits of the template. `object_id`: a full id of the repository,
/// which tells the hash of the index checksum
pub fn refresh_cloned_index(git_executor: &GitCommandExecutor, slot: &str, git_dir: &Path, template_written: Option<std::time::SystemTime>, object_id: &str) -> Result<()> {
  #[cfg(unix)]
  if let Some(template_written) = template_written {
    match crate::cloned_index::patch_cloned_index(git_dir, Path::new(slot), template_written, object_id) {
      Ok(patch) => tracing::debug!(slot, ?patch, "the stat data of the cloned index"),
      Err(error) => tracing::warn!(slot, error = %format!("{error:#}"), "cannot write the stat data of the cloned files into the index: the refresh hashes them"),
    }
  }
  #[cfg(not(unix))]
  let _ = (git_dir, template_written, object_id);
  git_executor
    .cmd(&["-c", "core.fsmonitor=false", "update-index", "-q", "--refresh"], slot)
    .accept(&[1])
    .output()?;
  Ok(())
}

/// Remove the untracked files of `slot` that a clone brought from its template, not the ignored ones (the warm build
/// outputs): `remove_untracked`. The local changes that are left, as `slot_changes` names them, come from its last
/// `status`, so a clone needs no other
pub fn clean_cloned_slot(git_executor: &GitCommandExecutor, slot: &str, untracked: UntrackedFiles) -> Result<Vec<String>> {
  remove_untracked(git_executor, slot, untracked)
}

/// How many passes `remove_untracked` makes at most: one more for each untracked ignore file that hid another
const UNTRACKED_PASSES: usize = 4;

/// Remove the untracked files of `slot` and keep the ignored ones (the warm build outputs), so the slot holds its
/// commit's tree and what the commit's own ignore rules ignore. A `status` in the `untracked` mode of the repository
/// lists them (its untracked cache serves that mode alone: 2 s in a slot of idea-1, where `clean -fd` walks every
/// directory, 49 s); with none untracked nothing more runs. Else the untracked `.gitignore` files it lists go first,
/// then one `clean -fd`, and one more `status` checks that nothing is left: a `clean` judges what is ignored by the
/// ignore files as it finds them when it starts, so it deleted an untracked `.gitignore` and kept the file that
/// `.gitignore` hid, untracked from then on. A `.gitignore` inside an untracked directory (one entry in the `normal`
/// mode) is found so, by the pass after. Each `status` may write the index (`slot_status`). The answer is the local
/// changes of the last pass, without the notes of the lease (`slot_changes`): the tracked files that differ
fn remove_untracked(git_executor: &GitCommandExecutor, slot: &str, untracked: UntrackedFiles) -> Result<Vec<String>> {
  for _ in 0..UNTRACKED_PASSES {
    let entries = slot_status(git_executor, slot, &[untracked.arg()])?;
    let found: Vec<&str> = entries.iter().filter(|(code, _)| code == b"??").map(|(_, path)| path.as_str()).collect();
    if found.is_empty() {
      return Ok(changed_paths(entries));
    }
    for path in found.iter().filter(|path| Path::new(path).file_name().is_some_and(|name| name == ".gitignore")) {
      fs_retry::remove_file(Path::new(slot).join(path))?;
    }
    git_executor.execute_command(&["clean", "-fd", "--quiet"], slot)?;
  }
  bail!("{slot} still has untracked files after {UNTRACKED_PASSES} passes of `clean -fd`")
}

/// Move the linked worktree `from` to `to` with `worktree move`, which renames the directory and fixes the links of its
/// `.git` file and of its admin directory; never a plain rename. `to` must not exist. It rewrites the admin directory
/// and reads every other one, so it runs alone too (`register_slot`)
pub fn move_slot(git_executor: &GitCommandExecutor, repository: &str, from: &str, to: &Path) -> Result<()> {
  let to = to.to_str().ok_or_else(|| anyhow!("The slot path is not UTF-8: {}", to.display()))?;
  git_executor.execute_command(&["worktree", "move", from, to], repository)?;
  Ok(())
}

/// Drop the linked worktree of the slot whose directory was at `path` from the repository of `git_common_dir`: one
/// `worktree remove --force --force`, which deletes its admin directory. Only after the directory moved away (the trim
/// of the pool moves it into the trash first, a rename): git would delete the files of a directory that is still there,
/// so a `path` that exists is refused. The second `--force` drops a lock too: a get killed in its clone left the lock of
/// `register_slot` on the slot, and a slot whose add never finished is removed so (`lease::is_unfinished`). It reads
/// every admin directory, so it runs alone too (`register_slot`)
pub fn forget_slot(git_executor: &GitCommandExecutor, git_common_dir: &str, path: &Path) -> Result<()> {
  if path.symlink_metadata().is_ok() {
    bail!("{} still exists: a slot is forgotten only once its directory moved away", path.display());
  }
  let path = path.to_str().ok_or_else(|| anyhow!("The slot path is not UTF-8: {}", path.display()))?;
  git_executor.execute_command(&["worktree", "remove", "--force", "--force", path], git_common_dir)?;
  Ok(())
}

/// The paths of `paths` (relative, `/`-separated) under which git tracks a file in `slot`, as the file it tracks: one
/// `ls-files -z` of all of them, literal pathspecs
pub fn tracked_under(git_executor: &GitCommandExecutor, slot: &str, paths: &[&str]) -> Result<Vec<String>> {
  if paths.is_empty() {
    return Ok(Vec::new());
  }
  let mut args = vec!["--literal-pathspecs", "ls-files", "-z", "--"];
  args.extend_from_slice(paths);
  let output = git_executor.cmd(&args, slot).bytes()?;
  Ok(nul_separated(&output))
}

/// The paths of `-z` output: NUL-separated, the empty ones left out
fn nul_separated(output: &[u8]) -> Vec<String> {
  output
    .split(|&byte| byte == 0)
    .filter(|path| !path.is_empty())
    .map(|path| String::from_utf8_lossy(path).into_owned())
    .collect()
}

/// A commit of HEAD that no branch, tag or remote-tracking ref holds, with its subject
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnlandedCommit {
  pub id: String,
  pub subject: String,
}

/// Up to `limit` commits of the HEAD of `slot` that no branch, tag or remote-tracking ref holds and that `base` (the
/// commit the slot was leased at) does not have, newest first: one `log HEAD --not --branches --tags --remotes <base>`.
/// Not `--all`, which holds the HEAD of every worktree, this one too. `base` keeps out the commits the slot started
/// with, which a rewrite of master may have left unreachable
pub fn unlanded_commits(git_executor: &GitCommandExecutor, slot: &str, base: Option<&str>, limit: usize) -> Result<Vec<UnlandedCommit>> {
  head_commits_without(git_executor, slot, &["--branches", "--tags", "--remotes"], base, limit)
}

/// Up to `limit` commits of the HEAD of `slot` that `branch` and `base` (the commit the slot was leased at) do not have,
/// newest first: one `log HEAD --not refs/heads/<branch> <base>`. The commits of a lease that its home branch lacks,
/// whatever other branch holds them (its own, made by the get)
pub fn commits_beyond_branch(git_executor: &GitCommandExecutor, slot: &str, branch: &str, base: Option<&str>, limit: usize) -> Result<Vec<UnlandedCommit>> {
  let branch = format!("refs/heads/{branch}");
  head_commits_without(git_executor, slot, &[&branch], base, limit)
}

/// Up to `limit` commits of the HEAD of `slot` that `base` (the commit the slot was leased at) does not have, newest
/// first: one `log HEAD --not <base>`. Every commit of a lease, whatever branch holds it
pub fn commits_since(git_executor: &GitCommandExecutor, slot: &str, base: Option<&str>, limit: usize) -> Result<Vec<UnlandedCommit>> {
  head_commits_without(git_executor, slot, &[], base, limit)
}

/// The commits of HEAD of `slot` without those of `not` and `base` (an id from the state file, never an option), newest
/// first: one `log -z`
fn head_commits_without(git_executor: &GitCommandExecutor, slot: &str, not: &[&str], base: Option<&str>, limit: usize) -> Result<Vec<UnlandedCommit>> {
  let max_count = format!("--max-count={limit}");
  let mut args = vec!["log", "-z", "--format=%H %s", &max_count, "HEAD", "--not"];
  args.extend_from_slice(not);
  args.extend(base.filter(|base| is_object_id(base)));
  let output = git_executor.cmd(&args, slot).bytes()?;
  Ok(
    output
      .split(|&byte| byte == 0)
      .filter(|record| !record.is_empty())
      .map(|record| {
        let record = String::from_utf8_lossy(record);
        let (id, subject) = record.split_once(' ').unwrap_or((&record, ""));
        UnlandedCommit {
          id: id.to_string(),
          subject: subject.to_string(),
        }
      })
      .collect(),
  )
}

/// Discard the local changes of `slot` that its holder confirmed: `reset --hard HEAD`, which also restores a tracked
/// ignore file, then `remove_untracked`, which removes the untracked files and keeps the ignored ones (build outputs
/// keep the next lease warm)
pub fn discard_slot_changes(git_executor: &GitCommandExecutor, slot: &str, untracked: UntrackedFiles) -> Result<()> {
  IndexLock::of(git_executor, slot).retry(INDEX_LOCK_WAIT, || git_executor.execute_command(&["reset", "--quiet", "--hard", "HEAD"], slot).map(drop))?;
  remove_untracked(git_executor, slot, untracked).map(drop)
}

/// A local change of a slot as `slot_change_entries` lists it: the two letters of `status --porcelain=v1` and the path
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotChange {
  pub code: [u8; 2],
  pub path: String,
}

impl SlotChange {
  /// An unmerged entry of the index (`DD`, `AU`, `UD`, `UA`, `DU`, `AA`, `UU`)
  pub const fn is_unmerged(&self) -> bool {
    matches!(&self.code, b"DD" | b"AU" | b"UD" | b"UA" | b"DU" | b"AA" | b"UU")
  }

  /// The one rule of what the discard of a slot would take away unseen: a deletion or a type change (`D` or `T` in
  /// either column) whose path is still there in another form in the slot at `root` (a file that left the index by `git
  /// rm --cached`, a directory in place of a file), or under a parent that is no directory now (a file in place of a
  /// directory of HEAD). `reset --hard` writes the version of HEAD there and removes what stands in its place, which
  /// may be ignored and so in no patch
  pub fn is_replaced(&self, root: &Path) -> bool {
    if !self.code.iter().any(|code| matches!(code, b'D' | b'T')) {
      return false;
    }
    root.join(&self.path).symlink_metadata().is_ok()
      || Path::new(&self.path)
        .ancestors()
        .skip(1)
        .filter(|parent| !parent.as_os_str().is_empty())
        .any(|parent| root.join(parent).symlink_metadata().is_ok_and(|metadata| !metadata.is_dir()))
  }
}

/// Every local change of `slot`, each untracked file by itself (`--untracked-files=all`) and a rename as a deletion and
/// an addition (`--no-renames`), without the notes of the lease: the `status` of the preserve of the changes of an idle
/// lease, before its patches and after them. It changes no file and no entry of the index; it may refresh the stat data
/// of the index, as every `status` of the pool may (`slot_status`). In a repository that configures another mode, git's
/// untracked cache serves no such `status` and stays as it was
pub fn slot_change_entries(git_executor: &GitCommandExecutor, slot: &str) -> Result<Vec<SlotChange>> {
  Ok(
    slot_status(git_executor, slot, &[UntrackedFiles::All.arg(), "--no-renames"])?
      .into_iter()
      .filter(|(code, path)| !(code == b"??" && is_notes(path)))
      .map(|(code, path)| SlotChange { code, path })
      .collect(),
  )
}

/// Why the local changes of a slot are not preserved as a patch (`preserve_refusal`, `preserve_slot_changes`)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreserveRefusal {
  /// An unmerged entry of the index: a patch carries no conflict
  Unmerged(String),
  /// A gitlink or a nested repository: a patch carries its commit, not its files
  Repository(String),
  /// A path that a filter of the attributes converts (Git LFS): a patch would carry what the filter made of it
  Filtered(String),
  /// An entry of the index with the bit assume-unchanged or skip-worktree: no `status` tells what its file holds
  Hidden(String),
  /// A changed ignore file (`.gitignore` at any depth): the discard puts the ignore rules of HEAD back first, and its
  /// `clean` would delete a file that only the changed rules kept, which no patch holds
  IgnoreRules(String),
  /// A deletion or a type change whose path stands in another form (`SlotChange::is_replaced`): no patch holds what
  /// stands there, which the discard would overwrite or delete
  Replaced(String),
  /// The changes have more bytes than the bound; `largest` names their largest file where the stat told it
  TooLarge { max: u64, largest: Option<String> },
}

impl std::fmt::Display for PreserveRefusal {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Unmerged(path) => write!(f, "an unmerged entry: {path}"),
      Self::Repository(path) => write!(f, "a gitlink or a nested repository: {path}"),
      Self::Filtered(path) => write!(f, "a path with a filter: {path}"),
      Self::Hidden(path) => write!(f, "an entry of the index that git does not check (assume-unchanged or skip-worktree): {path}"),
      Self::IgnoreRules(path) => write!(f, "a changed ignore file, whose ignored files no patch holds: {path}"),
      Self::Replaced(path) => write!(f, "a deleted or retyped path that something else stands in for: {path}"),
      Self::TooLarge { max, largest: None } => write!(f, "the changes have more than {max} bytes (branchdeck.leasePreserveMax)"),
      Self::TooLarge { max, largest: Some(path) } => write!(f, "the changes have more than {max} bytes (branchdeck.leasePreserveMax), the largest: {path}"),
    }
  }
}

impl PreserveRefusal {
  /// The path that refuses, if one does
  pub fn path(&self) -> Option<&str> {
    match self {
      Self::Unmerged(path) | Self::Repository(path) | Self::Filtered(path) | Self::Hidden(path) | Self::IgnoreRules(path) | Self::Replaced(path) => Some(path),
      Self::TooLarge { largest, .. } => largest.as_deref(),
    }
  }
}

/// What makes the local changes `changes` of `slot` (`slot_change_entries`) no patch, by reads only: an unmerged
/// entry, a nested repository or a gitlink (a directory with `.git`), a changed `.gitignore`, a deletion or a type
/// change that something else stands in for (`SlotChange::is_replaced`), a path with a `filter` attribute, an entry of the index that git does not check (one `ls-files -v`,
/// read until the first such entry). `None`: a patch can carry them
pub fn preserve_refusal(git_executor: &GitCommandExecutor, slot: &str, changes: &[SlotChange]) -> Result<Option<PreserveRefusal>> {
  if let Some(change) = changes.iter().find(|change| change.is_unmerged()) {
    return Ok(Some(PreserveRefusal::Unmerged(change.path.clone())));
  }
  let root = Path::new(slot);
  if let Some(change) = changes
    .iter()
    .find(|change| change.path.ends_with('/') || root.join(&change.path).join(".git").symlink_metadata().is_ok())
  {
    return Ok(Some(PreserveRefusal::Repository(change.path.clone())));
  }
  if let Some(change) = changes.iter().find(|change| Path::new(&change.path).file_name().is_some_and(|name| name == ".gitignore")) {
    return Ok(Some(PreserveRefusal::IgnoreRules(change.path.clone())));
  }
  if let Some(change) = changes.iter().find(|change| change.is_replaced(root)) {
    return Ok(Some(PreserveRefusal::Replaced(change.path.clone())));
  }
  let attributes = crate::attributes::read_attributes(git_executor, slot, None, &["filter"], changes.iter().map(|change| change.path.as_str()))?;
  if let Some(filtered) = attributes.into_iter().find(|attribute| !matches!(attribute.value.as_str(), "unspecified" | "unset")) {
    return Ok(Some(PreserveRefusal::Filtered(filtered.path)));
  }
  Ok(hidden_entry(git_executor, slot)?.map(PreserveRefusal::Hidden))
}

/// The first entry of the index of `slot` that git does not check: `ls-files -v -z` tags it with a lowercase letter
/// (assume-unchanged) or `S` (skip-worktree). git is stopped there
fn hidden_entry(git_executor: &GitCommandExecutor, slot: &str) -> Result<Option<String>> {
  let mut at_tag = true;
  let mut found: Option<Vec<u8>> = None;
  git_executor.cmd(&["ls-files", "-v", "-z"], slot).stream(|chunk| {
    for &byte in chunk {
      match (&mut found, byte) {
        (Some(path), 0) if !path.is_empty() => return Ok(std::ops::ControlFlow::Break(())),
        (Some(path), _) => path.push(byte),
        (None, 0) => at_tag = true,
        (None, tag) if at_tag => {
          at_tag = false;
          if tag.is_ascii_lowercase() || tag == b'S' {
            found = Some(Vec::new());
          }
        }
        (None, _) => {}
      }
    }
    Ok(std::ops::ControlFlow::Continue(()))
  })?;
  // the path follows the tag and its space
  Ok(found.map(|path| String::from_utf8_lossy(path.strip_prefix(b" ").unwrap_or(&path)).into_owned()))
}

/// The patch of the local changes of a slot (`preserve_slot_changes`): the tree of HEAD to the tree of `add --all`
pub const CHANGES_PATCH: &str = "changes.patch";
/// The patch of the index of a slot where it differs from its files: the tree of HEAD to the tree of the index
pub const STAGED_PATCH: &str = "staged.patch";

/// The patches `preserve_slot_changes` wrote and proved
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreservedPatches {
  /// HEAD of the slot, which both patches apply to
  pub base: String,
  /// The tree of the slot as `add --all` stages it: `CHANGES_PATCH` on `base` gives it
  pub tree: String,
  /// `STAGED_PATCH` was written: the index held a version of a file that its file no longer has
  pub staged: bool,
  /// The bytes of both patches
  pub bytes: u64,
}

/// The local changes of `slot` as patches in `dir`, no file of the slot and no entry of its index changed: a copy of its
/// index in its git dir (`IndexCopy`) gives the tree of the index (`write-tree`, no hash of a file) and, after `add
/// --all` with `core.safecrlf=true` (a conversion that a checkout would not undo fails it), the tree of the slot. Every
/// git of a private index runs with `core.splitIndex=false`, so no shared index of it lands in the git dir of the slot.
/// Then `CHANGES_PATCH`, HEAD to the tree of the slot (the unstaged, staged and untracked files, links, modes,
/// deletions), and `STAGED_PATCH` where the tree of the index is neither, both by `diff-tree --binary --full-index`, at
/// most `max` bytes together. The proof of each: a new index read from HEAD, its `apply --cached`, and its `write-tree`
/// is the tree of the slot or of its index (`prove_patch`). The caller bounds the bytes of the changed files first, as
/// `add --all` writes an object of each. A gitlink in either tree refuses, as a patch carries no files of it. The ignored
/// files are no change and stay out
pub fn preserve_slot_changes(git_executor: &GitCommandExecutor, slot: &str, dir: &Path, max: u64) -> Result<Result<PreservedPatches, PreserveRefusal>> {
  use crate::worktree_snapshot::{IndexCopy, read_head_commit, write_tree_patch_within};
  let worktree = linked_worktree(Path::new(slot)).ok_or_else(|| anyhow!("{slot} is no linked worktree"))?;
  let head = read_head_commit(git_executor, slot)?;
  let copy = IndexCopy::of(&worktree.git_dir, &worktree.git_dir.join("index"))?;
  let copy_path = copy.env_path()?;
  let in_copy = [("GIT_INDEX_FILE", copy_path.as_str())];
  // every write of a private index is whole: a split one would write its shared index into the git dir of the slot
  let staged = git_executor
    .cmd(&["-c", "core.splitIndex=false", "write-tree"], slot)
    .env(&in_copy)
    .text()?
    .trim()
    .to_string();
  git_executor
    .cmd(&["-c", "core.splitIndex=false", "-c", "core.safecrlf=true", "add", "--all"], slot)
    .env(&in_copy)
    .output()?;
  let tree = git_executor
    .cmd(&["-c", "core.splitIndex=false", "write-tree"], slot)
    .env(&in_copy)
    .text()?
    .trim()
    .to_string();
  for changed in [&tree, &staged] {
    if let Some(path) = gitlink_between(git_executor, slot, &head.tree, changed)? {
      return Ok(Err(PreserveRefusal::Repository(path)));
    }
  }
  std::fs::create_dir_all(dir).map_err(|error| anyhow!("Cannot make {}: {error}", dir.display()))?;
  let too_large = || Ok(Err(PreserveRefusal::TooLarge { max, largest: None }));
  let Some(changes) = write_tree_patch_within(git_executor, slot, (&head.tree, &tree), &dir.join(CHANGES_PATCH), max)? else {
    return too_large();
  };
  let staged_patch = dir.join(STAGED_PATCH);
  let mut bytes = changes.bytes;
  let has_staged = staged != head.tree && staged != tree;
  if has_staged {
    let Some(written) = write_tree_patch_within(git_executor, slot, (&head.tree, &staged), &staged_patch, max - bytes)? else {
      return too_large();
    };
    bytes += written.bytes;
  } else if staged_patch.symlink_metadata().is_ok() {
    // of an earlier preserve of the same lease
    fs_retry::remove_file(&staged_patch)?;
  }
  if tree != head.tree {
    prove_patch(git_executor, (slot, &worktree.git_dir), &head, &dir.join(CHANGES_PATCH), &tree)?;
  }
  if has_staged {
    prove_patch(git_executor, (slot, &worktree.git_dir), &head, &staged_patch, &staged)?;
  }
  Ok(Ok(PreservedPatches {
    base: head.commit,
    tree,
    staged: has_staged,
    bytes,
  }))
}

/// The proof of the patch at `patch`: on a new index of the tree of `head` in the git dir `git_dir` of `slot`, its
/// `apply --cached` gives the tree `expected`
fn prove_patch(git_executor: &GitCommandExecutor, (slot, git_dir): (&str, &Path), head: &crate::worktree_snapshot::Head, patch: &Path, expected: &str) -> Result<()> {
  let proof = crate::worktree_snapshot::IndexCopy::empty(git_dir)?;
  let proof_path = proof.env_path()?;
  let in_proof = [("GIT_INDEX_FILE", proof_path.as_str())];
  let patch = sync_utils::paths::without_verbatim(patch);
  let patch = patch.to_str().ok_or_else(|| anyhow!("The patch is no UTF-8 path: {}", patch.display()))?;
  git_executor.cmd(&["-c", "core.splitIndex=false", "read-tree", &head.tree], slot).env(&in_proof).output()?;
  git_executor
    .cmd(&["-c", "core.splitIndex=false", "apply", "--cached", patch], slot)
    .env(&in_proof)
    .output()?;
  let applied = git_executor.cmd(&["-c", "core.splitIndex=false", "write-tree"], slot).env(&in_proof).text()?;
  if applied.trim() != expected {
    bail!("The patch {patch} gives the tree {} on {}, not {expected}", applied.trim(), head.commit);
  }
  Ok(())
}

/// The first path that is a gitlink in `before` or in `after`, of the paths that differ: one `diff-tree -r --raw -z`
fn gitlink_between(git_executor: &GitCommandExecutor, slot: &str, before: &str, after: &str) -> Result<Option<String>> {
  if before == after {
    return Ok(None);
  }
  let output = git_executor.cmd(&["diff-tree", "-r", "-z", "--raw", "--no-renames", before, after], slot).bytes()?;
  // `:<old mode> <new mode> <old id> <new id> <status>` NUL `<path>` NUL
  let fields: Vec<&[u8]> = output.split(|&byte| byte == 0).collect();
  Ok(fields.as_chunks::<2>().0.iter().find_map(|[meta, path]| {
    let meta = String::from_utf8_lossy(meta);
    let mut modes = meta.trim_start_matches(':').split(' ');
    let gitlink = modes.next() == Some("160000") || modes.next() == Some("160000");
    gitlink.then(|| String::from_utf8_lossy(path).into_owned())
  }))
}

/// Start git's maintenance as a commit would, once a return freed its slot: `maintenance run --auto --detach` in the
/// common dir `git_common_dir`, so the detached run keeps no slot as its working directory. `--auto` runs only the tasks
/// whose thresholds passed (a geometric repack, loose objects), usually none; the run returns once it detached
pub fn start_deferred_maintenance(git_executor: &GitCommandExecutor, git_common_dir: &str) -> Result<()> {
  git_executor.execute_command(&["maintenance", "run", "--auto", "--detach"], git_common_dir)?;
  Ok(())
}

/// Detach the HEAD of `slot` at its commit when it is on a branch, which survives; no file changes. One `rev-parse`,
/// and a `RefTransaction` with `option no-deref` when attached
pub fn detach_slot_head(git_executor: &GitCommandExecutor, slot: &str) -> Result<()> {
  let Some((head, branch)) = read_head(git_executor, slot)? else {
    bail!("The slot {slot} has no commit");
  };
  if branch.is_some() {
    detach_head_at(git_executor, slot, &head, "lease return")?;
  }
  Ok(())
}

/// Detach the HEAD of `slot` at `head`, the commit of the branch it is on: `option no-deref`, so the branch stays
fn detach_head_at(git_executor: &GitCommandExecutor, slot: &str, head: &str, operation: &str) -> Result<()> {
  RefTransaction::new().update_no_deref("HEAD", head, Some(head)).commit(git_executor, slot, operation)
}

/// The paths of `paths` (relative to `repository`, `/`-separated) that git ignores there and does not track: one
/// `check-ignore -z --stdin`, which exits with 1 when it ignores none
pub fn ignored_paths(git_executor: &GitCommandExecutor, repository: &str, paths: &[String]) -> Result<Vec<String>> {
  if paths.is_empty() {
    return Ok(Vec::new());
  }
  let mut input = Vec::new();
  for path in paths {
    input.extend_from_slice(path.as_bytes());
    input.push(0);
  }
  let output = git_executor.cmd(&["check-ignore", "-z", "--stdin"], repository).stdin(&input).accept(&[1]).bytes()?;
  Ok(nul_separated(&output))
}

/// The git config of the pool, from one read
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LeaseConfig {
  /// `branchdeck.leasePool`, `~` expanded
  pub pool: Option<String>,
  pub workers: CheckoutWorkers,
  /// From `status.showUntrackedFiles`
  pub untracked: UntrackedFiles,
  /// `branchdeck.leaseMinFree`, as written (`200G`): the free space a get needs on the volume of the pool
  pub min_free: Option<String>,
  /// `branchdeck.leaseMaintenance`: a return starts git's maintenance once the slot is free
  /// (`start_deferred_maintenance`), for a repository whose `maintenance.auto` is false, where no commit or fetch in a
  /// slot starts it
  pub return_maintenance: bool,
  /// `branchdeck.leaseMaxSlots`, as written: how many slots the pool keeps once the volume has less free space than
  /// `min_free`; the free slots past it are deleted then, the ones returned longest ago first (`lease_pool::trim`)
  pub max_slots: Option<String>,
  /// `branchdeck.hostSlots`, as written: how many slots of the pool `bd-host` keeps for its runs (`host_slots`)
  pub host_slots: Option<String>,
  /// `branchdeck.hostSlotMaxSize`, as written (`60G`): what the clean rules may free in an idle host slot before it is
  /// cleaned
  pub host_slot_max_size: Option<String>,
  /// `branchdeck.hostSlotIdleClean`, as written (`6h`, `0` for never): how long a host slot waits idle after its last run
  /// before the keeper applies its clean rules (`host_slots::clean_idle_slots`)
  pub host_slot_idle_clean: Option<String>,
  /// `branchdeck.hostCores`, as written: the cores that the heavy runs of `brd-host` share on this host, in place of the
  /// cores the system counts (`host_turn`)
  pub host_cores: Option<String>,
  /// `branchdeck.hostJobs`, as written: the jobs that a command of `brd-host` gets on this host
  /// (`CARGO_BUILD_JOBS`, `NEXTEST_TEST_THREADS`, `BRD_HOST_JOBS`), so it uses fewer cores than the host counts
  pub host_jobs: Option<String>,
  /// `branchdeck.leaseNotesKeep`, as written (`14d`, `0` for ever): how long the notes of a returned lease stay before
  /// `brd lease trim` and the keeper of a host delete them (`lease_pool::lease_notes`)
  pub notes_keep: Option<String>,
  /// `branchdeck.leaseIdleChangesAfter`, as written (`12h`, `0` for never): how long the local changes of an idle lease
  /// stay untouched before a get or a reclaim preserves them as a patch in its notes and takes its slot
  pub idle_changes_after: Option<String>,
  /// `branchdeck.leasePreserveMax`, as written (`64M`): the most bytes of the patches of that preserve
  pub preserve_max: Option<String>,
  /// `core.fileMode` is false: git keeps the modes of the index, not the executable bits of the files
  pub no_file_mode: bool,
  /// `core.symlinks` is false: git keeps a link as a file of its target
  pub no_symlinks: bool,
  /// `branchdeck.agentPullRequest` under the route of `branchdeck.landing`: how far the landing of an agent's commit
  /// goes, which the `next` of a get names
  pub agent_landing: AgentLanding,
  /// Git LFS: whether its smudge filter downloads, and where it keeps its objects
  pub lfs: LfsConfig,
}

/// How far the landing of a coding agent's commit goes (`branchdeck.agentPullRequest`, a git boolean, off when unset or
/// invalid): the pick onto the local branch, or the pick, the push of the branch and its pull request. Under the Push
/// route of the repository itself the pick alone (`LandingSetting::agent_pull_request_off`): a `push` of the global or
/// the system config is unset, as `sync_core::repository_config` reads the same keys with their sources for
/// `brd workspace` and Settings → Agents
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AgentLanding {
  #[default]
  Pick,
  PullRequest,
}

/// `branchdeck.leasePool`, whether `checkout.workers` is set, `status.showUntrackedFiles`, `branchdeck.leaseMinFree`,
/// `branchdeck.leaseMaxSlots`, `branchdeck.leaseMaintenance`, `branchdeck.hostSlots`, `branchdeck.hostSlotMaxSize`,
/// `branchdeck.hostSlotIdleClean`, `branchdeck.hostCores`, `branchdeck.hostJobs`, `branchdeck.leaseNotesKeep`,
/// `branchdeck.leaseIdleChangesAfter`, `branchdeck.leasePreserveMax`, `branchdeck.agentPullRequest`, `branchdeck.landing`, `core.fileMode`, `core.symlinks`, the smudge commands of Git LFS
/// (`filter.lfs.process`, `filter.lfs.smudge`) and `lfs.storage`: one
/// `config --type=path --show-scope --get-regexp`, which exits with 1 when no key is set
pub fn lease_config(git_executor: &GitCommandExecutor, repository: &str) -> Result<LeaseConfig> {
  let output = git_executor
    .cmd(
      &[
        "config",
        "--type=path",
        "--show-scope",
        "--get-regexp",
        &format!(
          r"^(branchdeck\.leasepool|branchdeck\.leaseminfree|branchdeck\.leasemaxslots|branchdeck\.leasemaintenance|branchdeck\.hostslots|branchdeck\.hostslotmaxsize|branchdeck\.hostslotidleclean|branchdeck\.hostcores|branchdeck\.hostjobs|branchdeck\.leasenoteskeep|branchdeck\.leaseidlechangesafter|branchdeck\.leasepreservemax|branchdeck\.agentpullrequest|branchdeck\.landing|checkout\.workers|status\.showuntrackedfiles|core\.filemode|core\.symlinks|{LFS_KEYS})$"
        ),
      ],
      repository,
    )
    .accept(&[1])
    .output()?;
  let mut config = LeaseConfig::default();
  let mut pull_request = false;
  let mut route = None;
  let mut lfs = LfsKeys::default();
  for line in output.into_text().lines() {
    // `<scope>\t<key> <value>`, `<scope>\t<key>` for a key without a value
    let (scope, line) = line.split_once('\t').unwrap_or(("", line));
    let (key, value) = line.split_once(' ').unwrap_or((line, ""));
    match key {
      "branchdeck.leasepool" if !value.is_empty() => config.pool = Some(value.to_string()),
      "branchdeck.leaseminfree" if !value.is_empty() => config.min_free = Some(value.to_string()),
      "branchdeck.leasemaxslots" if !value.is_empty() => config.max_slots = Some(value.to_string()),
      "branchdeck.hostslots" if !value.is_empty() => config.host_slots = Some(value.to_string()),
      "branchdeck.hostslotmaxsize" if !value.is_empty() => config.host_slot_max_size = Some(value.to_string()),
      "branchdeck.hostslotidleclean" if !value.is_empty() => config.host_slot_idle_clean = Some(value.to_string()),
      "branchdeck.leasenoteskeep" if !value.is_empty() => config.notes_keep = Some(value.to_string()),
      "branchdeck.leaseidlechangesafter" if !value.is_empty() => config.idle_changes_after = Some(value.to_string()),
      "branchdeck.leasepreservemax" if !value.is_empty() => config.preserve_max = Some(value.to_string()),
      "branchdeck.hostcores" if !value.is_empty() => config.host_cores = Some(value.to_string()),
      "branchdeck.hostjobs" if !value.is_empty() => config.host_jobs = Some(value.to_string()),
      // the last value wins; a key without a value is true
      "branchdeck.leasemaintenance" => config.return_maintenance = parse_git_bool(line.split_once(' ').map(|(_, value)| value)) == Some(true),
      "branchdeck.agentpullrequest" => pull_request = parse_git_bool(line.split_once(' ').map(|(_, value)| value)) == Some(true),
      // the last entry wins; a `push` only the repository's own config names, as `sync_core::repository_config` takes it
      "branchdeck.landing" => route = LandingSetting::parse(value).filter(|setting| *setting != LandingSetting::Push || !matches!(scope, "system" | "global")),
      "checkout.workers" => config.workers.configured = true,
      // the last value wins, as in git
      "status.showuntrackedfiles" => config.untracked = if value == "all" { UntrackedFiles::All } else { UntrackedFiles::Normal },
      "core.filemode" => config.no_file_mode = parse_git_bool(line.split_once(' ').map(|(_, value)| value)) == Some(false),
      "core.symlinks" => config.no_symlinks = parse_git_bool(line.split_once(' ').map(|(_, value)| value)) == Some(false),
      _ => {
        lfs.read(key, value);
      }
    }
  }
  config.lfs = lfs.finish();
  config.agent_landing = if pull_request && route.and_then(LandingSetting::agent_pull_request_off).is_none() {
    AgentLanding::PullRequest
  } else {
    AgentLanding::Pick
  };
  Ok(config)
}

/// Whether `name` is a valid new branch name: one `check-ref-format --branch`, which exits with 128 for an invalid one
pub fn is_valid_branch_name(git_executor: &GitCommandExecutor, repository: &str, name: &str) -> Result<bool> {
  if name.starts_with('-') {
    return Ok(false);
  }
  Ok(git_executor.cmd(&["check-ref-format", "--branch", name], repository).status()?.code == 0)
}
