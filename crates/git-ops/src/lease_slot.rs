//! The git of a lease slot (`bd lease`, spec/cli/lease.spec.md): a linked worktree of the repository that a pool owns.
//! A slot is pool property, not the user's checkout, so its working tree is written here, and only here, by name:
//! `reset --merge` moves a clean slot to the commit of a get or a run by the diff of the two commits, `checkout
//! --force` writes every file of a new slot (or of a slot whose diff git refused), `clean -fd` clears the untracked
//! files a cloned slot got from its template, `reset --hard` and `clean -fd` discard the changes of a return the agent
//! confirmed or of a give-back. The worktree safety scan exempts exactly these argument lists in this file
//! (`worktree_safety_test.rs`, "Lease" in docs/design/git-operations.md). Every write of a slot's index waits for a
//! lock of another process and removes one that no process owns (`IndexLock`): a git killed in a slot leaves its
//! `index.lock`, and the next write there would fail on it.

use crate::index_lock::{INDEX_LOCK_WAIT, IndexLock};
use crate::ref_transaction::RefTransaction;
use crate::revs::{is_object_id, read_head};
use anyhow::{Result, anyhow, bail};
use git_executor::git_command_executor::GitCommandExecutor;
use std::path::{Path, PathBuf};
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

/// The linked worktree whose top level is `dir`, without a git process: `dir/.git` is a file `gitdir: <admin dir>`, the
/// admin dir names the common dir in `commondir` and points back at `dir/.git` in `gitdir`. `None` for anything else: a
/// missing directory, the main worktree, a submodule, a broken link
pub fn linked_worktree(dir: &Path) -> Option<LinkedWorktree> {
  let dot_git = dir.join(".git");
  let text = std::fs::read_to_string(&dot_git).ok()?;
  let admin = text.lines().next()?.strip_prefix("gitdir:")?.trim();
  let admin = std::fs::canonicalize(dir.join(admin)).ok()?;
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

/// The paths of the local changes of `slot`: staged, unstaged and untracked files, not the ignored ones. One
/// `status --porcelain=v1 -z` with the `untracked` mode of the repository; its own `core.fsmonitor` and
/// `core.untrackedCache` apply. The one `status` that may write the index (`GIT_OPTIONAL_LOCKS=1`, against the
/// executor's default): the refreshed stat data, the fsmonitor token and the untracked cache stay in the slot's index,
/// so the next `status` and the checkout start warm. Without the write, a slot of idea-1 (1.4M files) idle for days
/// walked its whole tree on every `status`, 2-5 min each; after one write a `status` took 2 s and a checkout without
/// changes 1-3 s instead of 4-10 s. Only a slot of the pool, with no other process in it, is asked
pub fn slot_changes(git_executor: &GitCommandExecutor, slot: &str, untracked: UntrackedFiles) -> Result<Vec<String>> {
  let output = git_executor
    .cmd(&["status", "--porcelain=v1", "-z", untracked.arg()], slot)
    .env(&[("GIT_OPTIONAL_LOCKS", "1")])
    .bytes()?;
  let mut paths = Vec::new();
  let mut entries = output.split(|&byte| byte == 0).filter(|entry| !entry.is_empty());
  while let Some(entry) = entries.next() {
    let Some(path) = entry.get(3..) else { continue };
    paths.push(String::from_utf8_lossy(path).into_owned());
    // a rename or a copy names its source in the next entry
    if matches!(entry.first(), Some(b'R' | b'C')) {
      entries.next();
    }
  }
  Ok(paths)
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

/// Write every file of `slot` at `commit`, detached, or on the new `branch` created there: the checkout of a new slot,
/// which `register_slot` left without an index or a clone brought with the files and the changes of its template, and
/// the fallback of a move that git refused (`move_clean_slot`). It writes only the paths that differ, but it stats every
/// file that `core.fsmonitor` does not vouch for: all of them in a slot without the daemon, 11-25 s in the IntelliJ
/// monorepo (1.4M files). `--force` discards the local changes, which is what a clone needs, and is safe after
/// `slot_changes` found nothing: a case-insensitive file system hides a case-only rename from `status`, and a checkout
/// without it then refuses. A `branch` that exists fails
pub fn checkout_slot(git_executor: &GitCommandExecutor, slot: &str, commit: &str, branch: Option<&str>, workers: CheckoutWorkers) -> Result<()> {
  let args = match branch {
    Some(branch) => [workers.config(), &["checkout", "--quiet", "--force", "-b", branch, commit]].concat(),
    None => [workers.config(), &["checkout", "--quiet", "--force", "--detach", commit]].concat(),
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

/// Bring the stat data of the index that `slot` got from its template up to the cloned files: `update-index -q
/// --refresh` with `core.checkStat=minimal`, `core.trustctime=false` and `core.fsmonitor=false`. A clone keeps the mtime
/// and the size of each file, but its inodes and ctimes are new: the default check would hash every file again. The
/// refresh writes the new stat data, so later commands run with the default check; without `core.fsmonitor` the index
/// also loses the fsmonitor bits of the template
pub fn refresh_cloned_index(git_executor: &GitCommandExecutor, slot: &str) -> Result<()> {
  git_executor
    .cmd(
      &[
        "-c",
        "core.checkStat=minimal",
        "-c",
        "core.trustctime=false",
        "-c",
        "core.fsmonitor=false",
        "update-index",
        "-q",
        "--refresh",
      ],
      slot,
    )
    .accept(&[1])
    .output()?;
  Ok(())
}

/// Remove the untracked files of `slot` that a clone brought from its template, not the ignored ones (the warm build
/// outputs): `clean -fd`
pub fn clean_cloned_slot(git_executor: &GitCommandExecutor, slot: &str) -> Result<()> {
  git_executor.execute_command(&["clean", "-fd", "--quiet"], slot)?;
  Ok(())
}

/// Move the linked worktree `from` to `to` with `worktree move`, which renames the directory and fixes the links of its
/// `.git` file and of its admin directory; never a plain rename. `to` must not exist. It rewrites the admin directory
/// and reads every other one, so it runs alone too (`register_slot`)
pub fn move_slot(git_executor: &GitCommandExecutor, repository: &str, from: &str, to: &Path) -> Result<()> {
  let to = to.to_str().ok_or_else(|| anyhow!("The slot path is not UTF-8: {}", to.display()))?;
  git_executor.execute_command(&["worktree", "move", from, to], repository)?;
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
  let max_count = format!("--max-count={limit}");
  let mut args = vec!["log", "-z", "--format=%H %s", &max_count, "HEAD", "--not", "--branches", "--tags", "--remotes"];
  // an id from the state file, never an option
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

/// Discard the local changes of `slot` that its holder confirmed: `reset --hard HEAD`, then `clean -fd`, which removes
/// the untracked files and keeps the ignored ones (build outputs keep the next lease warm)
pub fn discard_slot_changes(git_executor: &GitCommandExecutor, slot: &str) -> Result<()> {
  IndexLock::of(git_executor, slot).retry(INDEX_LOCK_WAIT, || git_executor.execute_command(&["reset", "--quiet", "--hard", "HEAD"], slot).map(drop))?;
  git_executor.execute_command(&["clean", "-fd", "--quiet"], slot)?;
  Ok(())
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
  /// `branchdeck.hostSlots`, as written: how many slots of the pool `bd-host` keeps for its runs (`host_slots`)
  pub host_slots: Option<String>,
  /// `branchdeck.hostSlotMaxSize`, as written (`60G`): what the clean rules may free in an idle host slot before it is
  /// cleaned
  pub host_slot_max_size: Option<String>,
  /// `core.fileMode` is false: git keeps the modes of the index, not the executable bits of the files
  pub no_file_mode: bool,
  /// `core.symlinks` is false: git keeps a link as a file of its target
  pub no_symlinks: bool,
}

/// `branchdeck.leasePool`, whether `checkout.workers` is set, `status.showUntrackedFiles`, `branchdeck.leaseMinFree`,
/// `branchdeck.leaseMaintenance`, `branchdeck.hostSlots`, `branchdeck.hostSlotMaxSize`, `core.fileMode` and
/// `core.symlinks`: one `config --type=path --get-regexp`, which exits with 1 when no key is set
pub fn lease_config(git_executor: &GitCommandExecutor, repository: &str) -> Result<LeaseConfig> {
  let output = git_executor
    .cmd(
      &[
        "config",
        "--type=path",
        "--get-regexp",
        r"^(branchdeck\.leasepool|branchdeck\.leaseminfree|branchdeck\.leasemaintenance|branchdeck\.hostslots|branchdeck\.hostslotmaxsize|checkout\.workers|status\.showuntrackedfiles|core\.filemode|core\.symlinks)$",
      ],
      repository,
    )
    .accept(&[1])
    .output()?;
  let mut config = LeaseConfig::default();
  for line in output.into_text().lines() {
    let (key, value) = line.split_once(' ').unwrap_or((line, ""));
    match key {
      "branchdeck.leasepool" if !value.is_empty() => config.pool = Some(value.to_string()),
      "branchdeck.leaseminfree" if !value.is_empty() => config.min_free = Some(value.to_string()),
      "branchdeck.hostslots" if !value.is_empty() => config.host_slots = Some(value.to_string()),
      "branchdeck.hostslotmaxsize" if !value.is_empty() => config.host_slot_max_size = Some(value.to_string()),
      // the last value wins; a key without a value is true
      "branchdeck.leasemaintenance" => config.return_maintenance = parse_git_bool(line.split_once(' ').map(|(_, value)| value)) == Some(true),
      "checkout.workers" => config.workers.configured = true,
      // the last value wins, as in git
      "status.showuntrackedfiles" => config.untracked = if value == "all" { UntrackedFiles::All } else { UntrackedFiles::Normal },
      "core.filemode" => config.no_file_mode = parse_git_bool(line.split_once(' ').map(|(_, value)| value)) == Some(false),
      "core.symlinks" => config.no_symlinks = parse_git_bool(line.split_once(' ').map(|(_, value)| value)) == Some(false),
      _ => {}
    }
  }
  Ok(config)
}

/// Whether `name` is a valid new branch name: one `check-ref-format --branch`, which exits with 128 for an invalid one
pub fn is_valid_branch_name(git_executor: &GitCommandExecutor, repository: &str, name: &str) -> Result<bool> {
  if name.starts_with('-') {
    return Ok(false);
  }
  Ok(git_executor.cmd(&["check-ref-format", "--branch", name], repository).status()?.code == 0)
}
