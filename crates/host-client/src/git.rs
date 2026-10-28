//! The git of the checkout of a run, in process (spec/cli/host.spec.md, "run"): the snapshot of HEAD or of the working
//! tree with the candidates that the host may have (`git_ops::host_client::snapshot`), `[host]` of the shipped commit
//! (`git_ops::host_objects::show_commit`), the thin pack into a private file (`pack_commit_until`), and the apply of the
//! changes of a command to the working tree (`apply_patch`). A git that fails refuses with `git_failed` and its words,
//! and a patch that does not apply with `apply_failed`

use git_executor::git_command_executor::{GitCommandExecutor, StoppedCommand};
use git_ops::host_client::{Scan, Shipper, apply_patch, commit_count, pack_commit_until, repository_name, snapshot};
use git_ops::host_objects::show_commit;
use git_ops::revs::{repository_dirs, with_repository_dirs};
use host_link::config::{ReadError, RepoConfig, read_commit_config};
use host_link::refusal::{Code, Refusal};
use std::path::{Path, PathBuf};

/// The directory of the run records and the patches, in the git directory of a checkout
const RUNS_DIR: &str = "branch-deck/host/runs";
/// The directory of the pack files, in the git directory of a checkout
const PACKS_DIR: &str = "branch-deck/host/packs";

/// The commit that a run ships, and what the client asks the host about
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Snapshot {
  pub(crate) commit: String,
  pub(crate) head: String,
  /// The lines of the status beyond HEAD (`XY path`); empty for `Scan::Head`
  pub(crate) changed: Vec<String>,
  /// The base of the later packs: `origin/master`, else `master`
  pub(crate) base: Option<String>,
  /// HEAD, its first-parent ancestors at the distances 1, 2, 4 … up to the farthest, and the merge bases with `master`
  /// and `origin/master`
  pub(crate) candidates: Vec<String>,
}

/// A thin pack in a private file, and the count of its commits. The file goes on every way out
pub(crate) struct Pack {
  pub(crate) path: PathBuf,
  pub(crate) bytes: u64,
  pub(crate) commits: u64,
}

impl Drop for Pack {
  fn drop(&mut self) {
    let _ = sync_utils::fs_retry::remove_file(&self.path);
  }
}

/// The local checkout of a run: its git with the directories it read once
#[derive(Clone)]
pub(crate) struct Checkout {
  git: GitCommandExecutor,
  top_level: String,
  git_dir: PathBuf,
  git_common_dir: PathBuf,
}

impl Checkout {
  /// The checkout that holds `dir`: one `rev-parse`. A directory in no checkout refuses with `usage`
  pub(crate) fn open(git: &GitCommandExecutor, dir: &Path) -> Result<Self, Refusal> {
    let path = dir.to_string_lossy();
    let dirs = repository_dirs(git, &path)
      .map_err(|error| git_failed("cannot read the git directory", &error))?
      .ok_or_else(|| Refusal::new(Code::Usage, format!("{} is in no git checkout", dir.display())))?;
    let checkout = Self {
      git: with_repository_dirs(git, &dirs.top_level, dirs.clone()),
      top_level: dirs.top_level,
      git_dir: PathBuf::from(dirs.git_dir),
      git_common_dir: PathBuf::from(dirs.git_common_dir),
    };
    Ok(checkout)
  }

  /// The root of the working tree
  pub(crate) fn root(&self) -> &Path {
    Path::new(&self.top_level)
  }

  /// `branch-deck/host/runs` of the git directory: the run records and the patches
  pub(crate) fn runs_dir(&self) -> PathBuf {
    self.git_dir.join(RUNS_DIR)
  }

  /// The name of the repository: the directory of its main checkout
  pub(crate) fn repository_name(&self) -> Option<String> {
    repository_name(&self.git_common_dir)
  }

  /// The snapshot of the checkout with the candidates up to `farthest`
  pub(crate) fn snapshot(&self, scan: Scan, farthest: usize) -> Result<Snapshot, Refusal> {
    let read =
      snapshot(&self.git, &self.top_level, &self.git_dir, scan, farthest, Shipper::BdHost).map_err(|error| git_failed("cannot read the snapshot of the checkout", &error))?;
    Ok(Snapshot {
      commit: read.commit,
      head: read.head.commit,
      changed: read.changed.unwrap_or_default().iter().map(git_ops::worktree_snapshot::StatusEntry::line).collect(),
      base: read.candidates.base,
      candidates: read.candidates.commits,
    })
  }

  /// `[host]` of `.branchdeck.toml` of `commit`, else its `.yonder.toml`, else the default, with the parser of the agent.
  /// A file that does not parse refuses with `bad_config`
  pub(crate) fn host_config(&self, commit: &str) -> Result<RepoConfig, Refusal> {
    read_commit_config(|path| {
      show_commit(&self.git, &self.top_level, commit, Some(path), &[])
        .map(|shown| shown.blob.map(|blob| blob.text))
        .map_err(|error| git_failed(&format!("cannot read {path} of the commit {commit}"), &error))
    })
    .map_err(|error| match error {
      ReadError::Read(refusal) => refusal,
      ReadError::Config(error) => Refusal::new(Code::BadConfig, format!("{error}, in the commit {commit}")),
    })
  }

  /// The thin pack of `commit` against the commits of `have`, into a private file of `branch-deck/host/packs`. `stop`
  /// kills git, and it sends the keepalives of the connection of the pack too. A stop answers `interrupted`
  pub(crate) fn pack(&self, commit: &str, have: &[String], stop: &dyn Fn() -> bool) -> Result<Pack, Refusal> {
    let dir = self.git_dir.join(PACKS_DIR);
    let packed = pack_commit_until(&self.git, &self.top_level, commit, have, &dir, &mut |_| {}, stop).map_err(|error| {
      if error.downcast_ref::<StoppedCommand>() == Some(&StoppedCommand::Cancelled) {
        Refusal::new(Code::Interrupted, "Ctrl+C stopped the build of the pack")
      } else {
        git_failed("cannot build the pack", &error)
      }
    })?;
    let mut pack = Pack {
      path: packed.path,
      bytes: packed.bytes,
      commits: 0,
    };
    pack.commits = commit_count(&self.git, &self.top_level, commit, have).map_err(|error| git_failed("cannot count the commits of the pack", &error))?;
    Ok(pack)
  }

  /// The count of the commits of the history of `commit`, for the refusal `behind`; `None` when git cannot count them
  pub(crate) fn history(&self, commit: &str) -> Option<u64> {
    commit_count(&self.git, &self.top_level, commit, &[]).ok()
  }

  /// Applies the patch to the files of the working tree, not to its index, all or nothing. Answers the count of the
  /// files. A patch that does not apply refuses with `apply_failed` and the words of git
  pub(crate) fn apply(&self, patch: &Path) -> Result<usize, Refusal> {
    apply_patch(&self.git, &self.top_level, patch).map_err(|error| Refusal::new(Code::ApplyFailed, format!("{error:#}")))
  }
}

/// The refusal of a git that failed: `git_failed` with what the client did and the words of git
fn git_failed(what: &str, error: &anyhow::Error) -> Refusal {
  Refusal::new(Code::GitFailed, format!("{what}: {error:#}"))
}
