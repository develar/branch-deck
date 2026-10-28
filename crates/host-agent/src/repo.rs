//! The repositories of the host and the files of a slot. Branch Deck owns their git, in process: the commits that a
//! repository has (`lease_pool::host_refs::have`), the ingest of a pack (`lease_pool::host_refs::ingest`), the files
//! of a commit (`lease_pool::host_refs::show`), and the tree and the changes of a slot (`snapshot_run`, `changes_run`).
//! No `bd` child runs. Each of these calls is a git user of the host, so it takes its turn behind an upgrade of git
//!
//! A pack lands as `refs/branch-deck/host/run/<id>` or `refs/branch-deck/host/base/<name>`. The give-back of a run
//! deletes its ref, and the keeper deletes a run ref without a lend after an hour and a base ref that no pack moved for
//! 14 days

use crate::home::Home;
use git_executor::git_command_executor::GitCommandExecutor;
use git_executor::git_info::GitInfo;
use git_ops::host_objects::{ShowRefusal, ShownCommit};
use git_ops::revs::{RepositoryDirs, is_object_id, repository_dirs};
use history_rewrite::repository_lock::DEFAULT_LOCK_TIMEOUT;
use host_link::proto::PackTarget;
use host_link::refusal::{Code, Refusal};
use lease_pool::host_refs::{HostRef, HostRefRefusal, IngestedPack, have, ingest, show};
use lease_pool::host_slots::{HostSlotRefusal, HostSlotsParams, changes_run, snapshot_run};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// How long a get or a return of a slot waits for the Bazel servers it asked to exit, as `bd` does
const SERVER_TIMEOUT: Duration = Duration::from_secs(30);

/// A repository of the host, with its git and its directories, read once
#[derive(Debug)]
pub(crate) struct Repository {
  /// The path that a client names, with `~/` expanded
  pub(crate) path: PathBuf,
  pub(crate) git: GitCommandExecutor,
  pub(crate) dirs: RepositoryDirs,
  /// The home directory of the pool of the host slots
  home: PathBuf,
}

/// The git of the agent: the one of `PATH`, else of Homebrew, at the floor of Branch Deck. A git below that floor refuses
/// with `old_git` and the hint of Branch Deck
pub(crate) fn git() -> Result<GitCommandExecutor, Refusal> {
  GitInfo::discover_for_cli(None)
    .map(GitCommandExecutor::with_info)
    .map_err(|hint| Refusal::new(Code::OldGit, hint))
}

impl Repository {
  /// The repository at `path`. A path that is no git repository refuses with `repo_missing`; a stat comes first, so a
  /// loaded host answers a wrong path without a git
  pub(crate) fn open(home: &Home, path: PathBuf) -> Result<Self, Refusal> {
    let missing = |why: &str| Refusal::new(Code::RepoMissing, format!("{} is not a git repository on this host: {why}", path.display()));
    if !path.is_dir() {
      return Err(missing("it is no directory"));
    }
    if !(path.join(".git").exists() || (path.join("HEAD").is_file() && path.join("objects").is_dir())) {
      return Err(missing("it has no .git, and no HEAD and objects of a bare repository"));
    }
    let git = git()?;
    let dirs = repository_dirs(&git, &path.to_string_lossy())
      .map_err(|error| Refusal::new(Code::AgentError, format!("cannot read the directories of {}: {error:#}", path.display())))?
      .ok_or_else(|| missing("git finds no repository there"))?;
    Ok(Self {
      path,
      git,
      dirs,
      home: home.dir().to_path_buf(),
    })
  }

  /// The parameters of the lend table of this repository
  pub(crate) fn params(&self) -> HostSlotsParams<'_> {
    HostSlotsParams {
      repository: &self.dirs,
      home: Some(&self.home),
      lock_timeout: DEFAULT_LOCK_TIMEOUT,
      server_timeout: SERVER_TIMEOUT,
    }
  }

  /// The common dir, where the lend table lives
  pub(crate) fn common_dir(&self) -> &Path {
    Path::new(&self.dirs.git_common_dir)
  }

  /// The commits of `commits` that the repository has as commit objects, in their order
  pub(crate) fn have(&self, commits: &[String]) -> Result<Vec<String>, Refusal> {
    if let Some(bad) = commits.iter().find(|commit| !is_object_id(commit)) {
      return Err(Refusal::new(Code::BadRequest, format!("{bad:?} is not a full commit id")));
    }
    if commits.is_empty() {
      return Ok(Vec::new());
    }
    have(&self.git, &self.dirs, commits, DEFAULT_LOCK_TIMEOUT).map_err(|error| Refusal::new(Code::AgentError, format!("{error:#}")))
  }

  /// Whether the repository has the commit `commit`
  pub(crate) fn has_commit(&self, commit: &str) -> Result<bool, Refusal> {
    self.have(&[commit.to_owned()]).map(|found| !found.is_empty())
  }

  /// Writes the pack that `input` streams into the repository and points the ref of `target` at `commit`. A refused
  /// target or commit is `bad_request` after the pack was read to its end; a pack that git refuses is `pack_failed`
  pub(crate) fn ingest(&self, target: &PackTarget, commit: &str, input: &mut dyn Read) -> Result<IngestedPack, Refusal> {
    let target = match target {
      PackTarget::Run(run) => HostRef::Run(run.clone()),
      PackTarget::Base(name) => HostRef::Base(name.clone()),
    };
    ingest(&self.git, &self.dirs, &target, commit, input, DEFAULT_LOCK_TIMEOUT).map_err(|error| {
      let code = if error.downcast_ref::<HostRefRefusal>().is_some() {
        Code::BadRequest
      } else {
        Code::PackFailed
      };
      Refusal::new(code, format!("{error:#}"))
    })
  }

  /// The text of the file `blob` and the object ids of `paths` in `commit`. A directory has its tree id, which covers its
  /// whole content
  pub(crate) fn show(&self, commit: &str, blob: Option<&str>, paths: &[String]) -> Result<ShownCommit, Refusal> {
    let trimmed: Vec<String> = paths.iter().map(|path| path.trim_end_matches('/').to_owned()).collect();
    show(&self.git, &self.dirs, commit, blob, &trimmed, DEFAULT_LOCK_TIMEOUT).map_err(|error| {
      let code = if error.downcast_ref::<ShowRefusal>().is_some() {
        Code::BadRequest
      } else {
        Code::AgentError
      };
      Refusal::new(code, format!("{error:#}"))
    })
  }

  /// The tree of the files of the slot lent to `run`, as `git add --all` stages them, through an index copy
  pub(crate) fn snapshot(&self, run: &str) -> Result<String, Refusal> {
    snapshot_run(&self.git, &self.params(), run)
      .map(|snapshot| snapshot.tree)
      .map_err(|error| Refusal::new(Code::AgentError, format!("{error:#}")))
  }

  /// Writes the patch from the tree `base` to the files of the slot lent to `run` into `patch`, for `git apply`. Answers
  /// whether the files changed. A slot whose files have the tree `base` gets no file
  pub(crate) fn changes(&self, run: &str, base: &str, patch: &Path) -> Result<bool, Refusal> {
    changes_run(&self.git, &self.params(), run, base, patch)
      .map(|changes| changes.patch_bytes.is_some())
      .map_err(|error| Refusal::new(Code::AgentError, format!("{error:#}")))
  }
}

/// The refusal of a failed call of the lend table: `old_git` with the hint of Branch Deck for a git below the floor of a
/// host, else `code`
pub(crate) fn pool_refusal(code: Code, error: &anyhow::Error) -> Refusal {
  if let Some(HostSlotRefusal::OldGit(hint)) = error.downcast_ref::<HostSlotRefusal>() {
    return Refusal::new(Code::OldGit, hint.clone());
  }
  Refusal::new(code, format!("{error:#}"))
}
