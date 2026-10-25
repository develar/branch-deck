//! What a sync reads, as cheap inputs: a focus syncs only when they changed since the last sync, as the scheduler of a
//! session (`repository_model::sync_scheduler`) compares them (docs/design/git-operations.md, "Sync inputs check"). The
//! remote-tracking refs of the prefix are listed apart from the hash of the rest, so a session replaces those its own
//! push moved without a git process: a focus after the push compares equal.

use crate::issue_navigation::issue_navigation_config_path;
use anyhow::{Result, anyhow};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::rewrite_guard::OPERATION_MARKERS;
use history_rewrite::issue_groups::ISSUE_GROUPS_REF;
use history_rewrite::parked::PARKED_REF;
use history_rewrite::stack_parents::STACK_PARENTS_REF;
use std::collections::BTreeMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::Path;
use std::time::UNIX_EPOCH;
use tracing::instrument;

/// What a sync of a branch prefix reads, equal while none of it changed
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SyncInputs {
  /// The hash of everything else: the prefix, the other refs, the upstream of HEAD's branch, the stamps of the files
  pub rest: u64,
  /// The remote-tracking refs of the prefix (`refs/remotes/<remote>/<prefix>/…`) by full name, each with its commit
  pub remote_refs: BTreeMap<String, String>,
}

impl SyncInputs {
  /// The remote-tracking ref `name` is at `tip` now, or gone (`None`): what a push of this clone left
  pub fn set_remote_ref(&mut self, name: String, tip: Option<String>) {
    match tip {
      Some(tip) => self.remote_refs.insert(name, tip),
      None => self.remote_refs.remove(&name),
    };
  }
}

/// The inputs of a sync of `branch_prefix`; two git processes and the stats of nine files, no object read, and a third
/// process when HEAD's branch tracks another remote branch:
/// - the local branches with the one HEAD is on (`%(HEAD)`): a commit, an amend, a switch, a rebase finished, but not
///   the virtual branches of the prefix, which the sync writes itself;
/// - the `master` and `main` of every remote and the upstream of HEAD's branch (the baseline, moved by a fetch
///   elsewhere), and every remote branch of the prefix (a push from elsewhere), listed apart: the remote status of the
///   virtual branches, the stack branches (`bd safe-push` of a whole stack) and the unapplied ones (`bd push --unapplied`);
/// - the issue groups, the stack parents and the parked branches;
/// - the size and the time of `config` and `config.worktree` (descriptions, `branchdeck.stacks`) and of `.idea/vcs.xml`
///   (issue links);
/// - which of the files of an operation in progress exist (`OPERATION_MARKERS`): a sync git deferred while a rebase or
///   a merge was in progress syncs once it ended, also by an abort, which moves no ref.
///
/// The working tree is not in it: the blocking paths of an update are read by the next sync.
#[instrument(skip(git_executor))]
pub fn read_sync_inputs(git_executor: &GitCommandExecutor, repository_path: &str, branch_prefix: &str) -> Result<SyncInputs> {
  let dirs = git_executor.execute_command_lines(&["rev-parse", "--path-format=absolute", "--git-dir", "--git-common-dir"], repository_path)?;
  let [git_dir, common_dir] = <[String; 2]>::try_from(dirs).map_err(|dirs| anyhow!("Unexpected rev-parse output: {dirs:?}"))?;
  let own_branches = format!("--exclude=refs/heads/{branch_prefix}/");
  // a glob matches the whole name, and `*` no slash: `**` takes `virtual/`, `stack/`, `unapplied/` and the stacked names
  let remote_branches = format!("refs/remotes/*/{branch_prefix}/**");
  let refs = git_executor.execute_command(
    &[
      "for-each-ref",
      // the upstream of HEAD's branch only, which `@{u}` makes the baseline
      "--format=%(HEAD)%(objectname) %(refname)%(if)%(HEAD)%(then) %(upstream)%(end)",
      &own_branches,
      "refs/heads/",
      "refs/remotes/*/master",
      "refs/remotes/*/main",
      &remote_branches,
      ISSUE_GROUPS_REF,
      STACK_PARENTS_REF,
      PARKED_REF,
    ],
    repository_path,
  )?;

  let mut hasher = DefaultHasher::new();
  branch_prefix.hash(&mut hasher);
  let mut remote_refs = BTreeMap::new();
  for line in refs.lines() {
    match remote_ref_of(line, branch_prefix) {
      Some((tip, name)) => {
        remote_refs.insert(name.to_owned(), tip.to_owned());
      }
      None => line.hash(&mut hasher),
    }
  }
  if let Some(upstream) = unlisted_upstream(&refs) {
    #[expect(clippy::collection_is_never_read, reason = "the output is hashed whole, which the lint does not count as a read")]
    let tip = git_executor.execute_command(&["for-each-ref", "--format=%(objectname)", upstream], repository_path)?;
    tip.hash(&mut hasher);
  }
  for file in [
    Path::new(&common_dir).join("config"),
    Path::new(&git_dir).join("config.worktree"),
    issue_navigation_config_path(repository_path),
  ] {
    file_stamp(&file).hash(&mut hasher);
  }
  for (marker, _) in OPERATION_MARKERS {
    Path::new(&git_dir).join(marker).exists().hash(&mut hasher);
  }
  Ok(SyncInputs {
    rest: hasher.finish(),
    remote_refs,
  })
}

/// The commit and the name of a line of the listing that is a remote-tracking ref of the prefix
/// (`refs/remotes/<remote>/<prefix>/…`); `None` for any other line
fn remote_ref_of<'a>(line: &'a str, branch_prefix: &str) -> Option<(&'a str, &'a str)> {
  // `%(HEAD)` is a space for every ref but the branch HEAD is on
  let (tip, name) = line.strip_prefix(' ')?.split_once(' ')?;
  let (_, branch) = name.strip_prefix("refs/remotes/")?.split_once('/')?;
  branch.strip_prefix(branch_prefix)?.starts_with('/').then_some((tip, name))
}

/// The upstream of HEAD's branch in the listing, unless the listing has its tip already: a local branch, or the `master`
/// or `main` of a remote
fn unlisted_upstream(refs: &str) -> Option<&str> {
  let upstream = refs.lines().find(|line| line.starts_with('*'))?.split(' ').nth(2).filter(|upstream| !upstream.is_empty())?;
  let listed = upstream.starts_with("refs/heads/")
    || upstream
      .strip_prefix("refs/remotes/")
      .and_then(|name| name.split_once('/'))
      .is_some_and(|(_, branch)| branch == "master" || branch == "main");
  (!listed).then_some(upstream)
}

/// The size and the modification time of a file, nanoseconds; none for a missing one
pub fn file_stamp(path: &Path) -> Option<(u64, u128)> {
  let metadata = std::fs::metadata(path).ok()?;
  let modified = metadata.modified().ok()?.duration_since(UNIX_EPOCH).ok()?.as_nanos();
  Some((metadata.len(), modified))
}
