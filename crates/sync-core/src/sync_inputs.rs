//! What a sync reads, as one cheap fingerprint: the window syncs on focus only when it changed since the last sync
//! (docs/design/git-operations.md, "Sync inputs check").

use crate::issue_navigation::issue_navigation_config_path;
use anyhow::{Result, anyhow};
use git_executor::git_command_executor::GitCommandExecutor;
use history_rewrite::issue_groups::ISSUE_GROUPS_REF;
use history_rewrite::parked::PARKED_REF;
use history_rewrite::stack_parents::STACK_PARENTS_REF;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::Path;
use std::time::UNIX_EPOCH;
use tracing::instrument;

/// A fingerprint of the inputs of a sync of `branch_prefix`, equal while none changed; two git processes and three
/// file stats, no object read, and a third process when HEAD's branch tracks another remote branch:
/// - the local branches with the one HEAD is on (`%(HEAD)`): a commit, an amend, a switch, a rebase finished, but not
///   the virtual branches of the prefix, which the sync writes itself;
/// - the `master` and `main` of every remote and the upstream of HEAD's branch (the baseline, moved by a fetch
///   elsewhere), and every remote branch of the prefix (a push from elsewhere): the remote status of the virtual
///   branches, the stack branches (`bd safe-push` of a whole stack) and the unapplied ones (`bd push --unapplied`);
/// - the issue groups, the stack parents and the parked branches;
/// - the size and the time of `config` and `config.worktree` (descriptions, `branchdeck.stacks`) and of `.idea/vcs.xml`
///   (issue links).
///
/// The working tree is not in it: the blocking paths of an update are read by the next sync.
#[instrument(skip(git_executor))]
pub fn sync_inputs_fingerprint(git_executor: &GitCommandExecutor, repository_path: &str, branch_prefix: &str) -> Result<String> {
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
  refs.hash(&mut hasher);
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
  Ok(format!("{:016x}", hasher.finish()))
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
