//! The git of a client of a remote run (`bd host snapshot`, `pack`, `apply`, spec/cli/lease.spec.md, "Host runs"; and
//! `bd-host run`, spec/cli/host.spec.md, "run"): the commit a run ships (HEAD, or a commit of the working tree built in
//! a copy of the index), the commits the host may have, the base of its later packs, the thin pack of the commit into a
//! private file, and the patch of the changes of the command applied to the working tree. The apply is the one write of
//! the user's working tree outside `worktree_update`: what the user asked a run for (`--apply`), all or nothing, the
//! index left alone (`worktree_safety_test.rs` counts it)

use crate::lease_slot::{LeaseConfig, lease_config};
use crate::revs::{is_object_id, merge_base};
use crate::worktree_snapshot::{CommitText, Head, StatusEntry, WorktreeCommit, local_changes, read_head_commit, worktree_commit};
use anyhow::{Context, Result, anyhow, bail};
use git_executor::git_command_executor::{GitCommandExecutor, ProgressStop, StoppedCommand};
use std::io::Write as _;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// Who ships a commit of the working tree: its author, its committer and the first word of its message. The client of
/// yonder and `bd-host` write two commits of one tree, so each of them sends its own pack once
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shipper {
  /// `yonder <yonder@yonder.invalid>`: the commits of `bd host snapshot --working-tree`
  Yonder,
  /// `bd-host <bd-host@bd-host.invalid>`: the commits of `bd-host run`
  BdHost,
}

impl Shipper {
  /// The name and the email of the author and the committer
  const fn ident(self) -> (&'static str, &'static str) {
    match self {
      Self::Yonder => ("yonder", "yonder@yonder.invalid"),
      Self::BdHost => ("bd-host", "bd-host@bd-host.invalid"),
    }
  }
}

/// The refs whose merge bases with HEAD the host may have, and whose tip is the base of the later packs, first found
/// first
const BASE_REFS: [&str; 2] = ["refs/remotes/origin/master", "refs/heads/master"];
/// The farthest first-parent distance from HEAD of a candidate by default: the powers of two up to it, so a host some
/// thousand commits behind still has one of them
pub const FARTHEST: usize = 65_536;
/// How long a pack file of `pack_commit` stays in its directory when its caller did not remove it
const PACK_KEEP: Duration = Duration::from_hours(24);
/// A minute without a progress line of `pack-objects` is a stuck git: it writes one at least every second while it works
const PACK_QUIET: Duration = Duration::from_mins(1);

/// The commit of the working tree of `repo` (git dir `git_dir`): HEAD when the status finds no change, else a commit on
/// HEAD whose tree is what `git add --all` would stage (`worktree_commit`), so the index, the files and the refs stay.
/// Author and committer are fixed and dated as the committer of HEAD, so the same tree on the same HEAD is the same
/// commit, and a second run of it sends no pack. Only the objects of the changed files are new, and no ref names them;
/// a `gc` prunes them
pub fn working_tree_commit(git: &GitCommandExecutor, repo: &str, git_dir: &Path, head: &Head, config: &LeaseConfig) -> Result<WorktreeCommit> {
  working_tree_commit_of(git, repo, git_dir, head, config, Shipper::Yonder)
}

/// `working_tree_commit` of `shipper`: its author and committer, and the message `<name>: the working tree on <HEAD>`
pub fn working_tree_commit_of(git: &GitCommandExecutor, repo: &str, git_dir: &Path, head: &Head, config: &LeaseConfig, shipper: Shipper) -> Result<WorktreeCommit> {
  let (name, email) = shipper.ident();
  let message = format!("{name}: the working tree on {}", head.commit.get(..12).unwrap_or(&head.commit));
  worktree_commit(git, repo, git_dir, head, config, None, CommitText { name, email, message: &message })
}

/// What a snapshot of a checkout reads beside HEAD and the candidates
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scan {
  /// No status: the fast first read of a run of the committed HEAD
  Head,
  /// The status of the working tree: its entries are the local changes
  Status,
  /// The commit of the working tree (`working_tree_commit_of`)
  WorkingTree,
}

/// The commit that a run ships, and what its client asks the host about
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
  /// HEAD, or the commit of the working tree
  pub commit: String,
  pub head: Head,
  /// The entries of the status beyond HEAD; `None` for `Scan::Head`
  pub changed: Option<Vec<StatusEntry>>,
  /// The candidates of HEAD up to the farthest distance, and the base
  pub candidates: Candidates,
}

/// The snapshot of the worktree `repo` (git dir `git_dir`): HEAD (one `log`); by `scan` nothing, the status, or the
/// commit of the working tree of `shipper` (`lease_config` and one `status`, then `hash-object` and `fast-import` for a
/// changed tree); then the candidates of HEAD up to `farthest`. No ref, index or file of the worktree changes
pub fn snapshot(git: &GitCommandExecutor, repo: &str, git_dir: &Path, scan: Scan, farthest: usize, shipper: Shipper) -> Result<Snapshot> {
  let head = read_head_commit(git, repo)?;
  let (commit, changed) = match scan {
    Scan::Head => (head.commit.clone(), None),
    Scan::Status => (head.commit.clone(), Some(local_changes(git, repo, &lease_config(git, repo)?)?)),
    Scan::WorkingTree => {
      let shipped = working_tree_commit_of(git, repo, git_dir, &head, &lease_config(git, repo)?, shipper)?;
      (shipped.commit, Some(shipped.entries))
    }
  };
  let candidates = candidates(git, repo, &head.commit, farthest)?;
  Ok(Snapshot {
    commit,
    head,
    changed,
    candidates,
  })
}

/// The commits a host may have and the base of the later packs
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidates {
  /// HEAD, its first-parent ancestors at the distances 1, 2, 4 … `farthest`, then the merge bases of HEAD with
  /// `origin/master` and `master`; each once
  pub commits: Vec<String>,
  /// The tip of `origin/master`, else of `master`
  pub base: Option<String>,
}

/// The candidates of `head`: one `rev-list --first-parent`, one `for-each-ref` of the base refs and one `merge-base`
/// per base ref the repository has
pub fn candidates(git: &GitCommandExecutor, repo: &str, head: &str, farthest: usize) -> Result<Candidates> {
  let mut commits: Vec<String> = Vec::new();
  let mut add = |commit: &str| {
    if is_object_id(commit) && !commits.iter().any(|known| known == commit) {
      commits.push(commit.to_string());
    }
  };
  let max_count = format!("--max-count={}", farthest.saturating_add(1));
  let ancestors = git.cmd(&["rev-list", "--first-parent", &max_count, head], repo).lines()?;
  let powers = (0..=farthest.max(1).ilog2()).map(|power| 1usize << power).filter(|distance| *distance <= farthest);
  for distance in std::iter::once(0).chain(powers) {
    if let Some(ancestor) = ancestors.get(distance) {
      add(ancestor);
    }
  }
  let mut args = vec!["for-each-ref", "--format=%(refname) %(objectname)"];
  args.extend_from_slice(&BASE_REFS);
  let tips: Vec<(String, String)> = git
    .cmd(&args, repo)
    .lines()?
    .iter()
    .filter_map(|line| line.split_once(' ').map(|(reference, tip)| (reference.to_string(), tip.to_string())))
    .collect();
  let tip_of = |reference: &str| tips.iter().find(|(name, _)| name == reference).map(|(_, tip)| tip.clone());
  let mut base = None;
  for reference in BASE_REFS {
    let Some(tip) = tip_of(reference) else { continue };
    if let Some(merged) = merge_base(git, repo, head, &tip)? {
      add(&merged);
    }
    base.get_or_insert(tip);
  }
  Ok(Candidates { commits, base })
}

/// The name of the repository: the directory of its main worktree, without `.git` (`idea` for a linked worktree of
/// `~/projects/idea`), from its common dir, without a process
pub fn repository_name(git_common_dir: &Path) -> Option<String> {
  let name = if git_common_dir.file_name().is_some_and(|name| name == ".git") {
    git_common_dir.parent()?.file_name()?
  } else {
    git_common_dir.file_name()?
  };
  let name = name.to_str()?;
  let name = name.strip_suffix(".git").unwrap_or(name);
  (!name.is_empty()).then(|| name.to_string())
}

/// A thin pack in its file
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackFile {
  pub path: PathBuf,
  pub bytes: u64,
}

/// The thin pack of `commit` against the commits of `have` (`pack-objects --revs --thin --stdout --progress`) streamed
/// into a new file of `dir` (made private, mode 0700; the file 0600 and of a random name): the caller sends the file and
/// removes it, and a pack file older than a day goes at the next pack. `on_line` gets each progress line of git; a
/// minute without one kills git
pub fn pack_commit(git: &GitCommandExecutor, repo: &str, commit: &str, have: &[String], dir: &Path, on_line: &mut dyn FnMut(&str)) -> Result<PackFile> {
  pack_commit_until(git, repo, commit, have, dir, on_line, &|| false)
}

/// `pack_commit` that `is_cancelled` stops. git polls it while it runs, so the keepalives of the connection of the pack
/// go out from it too. A stop fails with `StoppedCommand::Cancelled`, and the file goes
pub fn pack_commit_until(
  git: &GitCommandExecutor,
  repo: &str,
  commit: &str,
  have: &[String],
  dir: &Path,
  on_line: &mut dyn FnMut(&str),
  is_cancelled: &dyn Fn() -> bool,
) -> Result<PackFile> {
  if let Some(bad) = std::iter::once(commit).chain(have.iter().map(String::as_str)).find(|id| !is_object_id(id)) {
    bail!("{bad:?} is no full commit id");
  }
  std::fs::create_dir_all(dir).with_context(|| format!("Cannot make {}", dir.display()))?;
  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700)).with_context(|| format!("Cannot make {} private", dir.display()))?;
  }
  remove_old_packs(dir);
  let mut file = tempfile::Builder::new()
    .prefix("pack-")
    .suffix(".pack")
    .tempfile_in(dir)
    .with_context(|| format!("Cannot make a file in {}", dir.display()))?;
  let mut input = format!("{commit}\n");
  for known in have {
    input.push('^');
    input.push_str(known);
    input.push('\n');
  }
  let mut bytes = 0u64;
  git
    .cmd(&["pack-objects", "--revs", "--thin", "--stdout", "--progress"], repo)
    .stdin(input.as_bytes())
    .stream_progress(
      |chunk| {
        file.write_all(chunk).context("Cannot write the pack")?;
        bytes += chunk.len() as u64;
        Ok(ControlFlow::Continue(()))
      },
      on_line,
      &ProgressStop {
        idle_timeout: Some(PACK_QUIET),
        is_cancelled,
      },
    )?;
  // a git that ended before its first poll saw the stop
  if is_cancelled() {
    bail!(StoppedCommand::Cancelled);
  }
  file.flush()?;
  let (_, path) = file.keep().map_err(|error| anyhow!("Cannot keep the pack: {error}"))?;
  Ok(PackFile { path, bytes })
}

/// The pack files of `dir` older than `PACK_KEEP`: a caller that died before it removed its file
fn remove_old_packs(dir: &Path) {
  let Ok(entries) = std::fs::read_dir(dir) else { return };
  let now = SystemTime::now();
  for entry in entries.flatten() {
    let old = entry
      .metadata()
      .and_then(|metadata| metadata.modified())
      .ok()
      .and_then(|modified| now.duration_since(modified).ok())
      .is_some_and(|age| age >= PACK_KEEP);
    if old && entry.file_name().to_string_lossy().ends_with(".pack") {
      let _ = sync_utils::fs_retry::remove_file(entry.path());
    }
  }
}

/// The commits of `commit` that the commits of `have` lack: one `rev-list --count`
pub fn commit_count(git: &GitCommandExecutor, repo: &str, commit: &str, have: &[String]) -> Result<u64> {
  let excluded: Vec<String> = have.iter().map(|known| format!("^{known}")).collect();
  let mut args = vec!["rev-list", "--count", commit];
  args.extend(excluded.iter().map(String::as_str));
  let count = git.cmd(&args, repo).text()?;
  count.trim().parse().map_err(|error| anyhow!("git rev-list --count answered {count:?}: {error}"))
}

/// Apply the patch `patch` (an absolute path) to the files of the worktree `repo`, not its index: `apply --numstat`
/// counts its files, then `apply --whitespace=nowarn` applies it, all or nothing. The count of its files
pub fn apply_patch(git: &GitCommandExecutor, repo: &str, patch: &Path) -> Result<usize> {
  let patch = patch.to_str().ok_or_else(|| anyhow!("The patch is no UTF-8 path: {}", patch.display()))?;
  if !Path::new(patch).is_absolute() {
    bail!("The patch {patch} is no absolute path");
  }
  let files = git.cmd(&["apply", "--numstat", patch], repo).lines()?.len();
  git.cmd(&["apply", "--whitespace=nowarn", patch], repo).output()?;
  Ok(files)
}
