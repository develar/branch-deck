//! Update Project: bring the checked out branch onto its upstream, as `git pull --rebase` would, without touching
//! unrelated local changes (docs/design/git-operations.md, "Update project"). The local commits are replayed in memory
//! with `git replay`, a dry run on a copy of the index proves the working tree can follow, and only then does the branch
//! move; the files follow through `worktree_update`, the one exemption from "never write the working tree".

use crate::ref_transaction::RefTransaction;
use crate::rewrite_guard::{ensure_no_merges, ensure_no_operation_in_progress, get_current_branch};
use crate::rewrite_plan::replay_range_with_env;
use crate::worktree_update::{apply_worktree_update, dry_run_worktree_update};
use anyhow::{Result, anyhow, bail};
use git_executor::git_command_executor::GitCommandExecutor;
use serde::Serialize;
use std::collections::HashSet;
use std::path::Path;
use tracing::{debug, info, instrument};

/// How the branch reaches the upstream
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum UpdateKind {
  /// No local commits: the branch moves to the upstream
  FastForward,
  /// The local commits are replayed onto the upstream
  Rebase,
}

/// A local commit that would be replayed
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct UpdateCommit {
  pub id: String,
  pub subject: String,
}

/// Whether the branch can be updated, relative to the last fetch of the upstream
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum UpdateState {
  /// The upstream has nothing the branch lacks
  UpToDate,
  /// The update applies without conflicts and without touching local changes
  Clean { kind: UpdateKind },
  /// Replaying the local commits onto the upstream conflicts; nothing was changed
  Conflict {
    /// The files that likely conflict
    files: Vec<String>,
    /// The local commits, oldest first
    commits: Vec<UpdateCommit>,
  },
  /// Uncommitted or staged changes, or untracked files, sit on paths the update would write; nothing was changed
  Blocked { paths: Vec<String> },
}

/// The result of checking the branch against its upstream
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheck {
  /// The checked out branch, short name
  pub branch: String,
  /// The upstream as the user knows it, e.g. `origin/master`
  pub upstream: String,
  pub head: String,
  pub upstream_head: String,
  /// The merge base of HEAD and the upstream
  pub base: String,
  pub state: UpdateState,
  /// When the upstream was last fetched (Unix seconds), if known
  pub fetched_at: Option<u32>,
}

/// What an applied update did
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct AppliedUpdate {
  pub kind: UpdateKind,
  pub new_head: String,
  /// Local commits replayed onto the upstream
  pub replayed: u32,
  /// Local commits whose changes are already upstream: `git replay` drops a commit that becomes empty
  pub dropped_already_upstream: u32,
}

/// The upstream of the checked out branch
#[derive(Debug, Clone, Copy)]
pub struct UpstreamRef<'a> {
  /// Full name of the remote-tracking ref, e.g. `refs/remotes/origin/master`
  pub tracking_ref: &'a str,
  /// Short name, e.g. `origin/master`
  pub name: &'a str,
  /// Where the tracking ref pointed before the fetch that just moved it, if that was a forced update
  pub previous: Option<&'a str>,
}

/// What a check is for
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckPurpose {
  /// Only the state is shown (the check inside every sync). The local commits are replayed with a fixed committer
  /// date, so repeated checks of the same state write the same objects instead of new ones each time; such a plan
  /// can't be applied.
  Preview,
  /// The plan is applied next
  Apply,
}

/// A fixed committer date for preview replays
const PREVIEW_COMMITTER_DATE: [(&str, &str); 1] = [("GIT_COMMITTER_DATE", "@0 +0000")];

/// A checked update; `apply_update` applies a clean one
#[derive(Debug)]
pub struct UpdatePlan {
  pub check: UpdateCheck,
  purpose: CheckPurpose,
  /// HEAD after the update, for a clean one
  new_head: Option<String>,
  old_tree: String,
  new_tree: String,
  /// The revisions excluded from the replayed range besides the upstream (previous upstream tips)
  excluded: Vec<String>,
}

/// Check, without the network and without writing anything, whether the checked out branch can be updated to
/// `upstream`. Refuses (an error) while a rebase, merge, cherry-pick or revert is in progress, on a detached HEAD, and
/// if the local commits contain a merge.
///
/// The local commits are `HEAD ^upstream ^<previous tips>`: the previous tips are the tip before a forced fetch and the
/// fork point from the tracking ref's reflog, so commits that a force-push removed upstream are not replayed back, as
/// with `git pull --rebase`.
#[instrument(skip(git_executor))]
pub fn check_update(git_executor: &GitCommandExecutor, repo_path: &str, upstream: UpstreamRef<'_>, purpose: CheckPurpose) -> Result<UpdatePlan> {
  ensure_no_operation_in_progress(git_executor, repo_path)?;
  let branch = get_current_branch(git_executor, repo_path)?;
  let ids = git_executor.execute_command_lines(&["rev-parse", "HEAD^{commit}", &format!("{}^{{commit}}", upstream.tracking_ref)], repo_path)?;
  let [head, upstream_head] = <[String; 2]>::try_from(ids).map_err(|ids| anyhow!("Unexpected rev-parse output: {ids:?}"))?;
  let (base, exit_code) = git_executor.execute_command_with_status(&["merge-base", &head, &upstream_head], repo_path)?;
  if exit_code != 0 {
    bail!("{branch} and {} have no common history", upstream.name);
  }

  let mut plan = UpdatePlan {
    check: UpdateCheck {
      branch,
      upstream: upstream.name.to_string(),
      head: head.clone(),
      upstream_head: upstream_head.clone(),
      base: base.clone(),
      state: UpdateState::UpToDate,
      fetched_at: None,
    },
    purpose,
    new_head: None,
    old_tree: String::new(),
    new_tree: String::new(),
    excluded: Vec::new(),
  };
  if base == upstream_head {
    return Ok(plan);
  }

  let kind = if base == head { UpdateKind::FastForward } else { UpdateKind::Rebase };
  let new_head = match kind {
    UpdateKind::FastForward => upstream_head.clone(),
    UpdateKind::Rebase => {
      plan.excluded = previous_upstream_tips(git_executor, repo_path, upstream, &head, &base)?;
      let mut exclude = vec![upstream_head.as_str()];
      exclude.extend(plan.excluded.iter().map(String::as_str));
      ensure_no_merges(git_executor, repo_path, &head, &exclude)?;

      let negated: Vec<String> = exclude.iter().map(|rev| format!("^{rev}")).collect();
      let mut revs = vec![head.as_str()];
      revs.extend(negated.iter().map(String::as_str));
      let env: &[(&str, &str)] = match purpose {
        CheckPurpose::Preview => &PREVIEW_COMMITTER_DATE,
        CheckPurpose::Apply => &[],
      };
      if let Some(new_head) = replay_range_with_env(git_executor, repo_path, &upstream_head, &revs, env)? {
        new_head
      } else {
        let files = conflicting_files(git_executor, repo_path, &base, &upstream_head, &head)?;
        let commits = local_commits(git_executor, repo_path, &revs)?;
        debug!(files = files.len(), commits = commits.len(), "update conflicts");
        plan.check.state = UpdateState::Conflict { files, commits };
        return Ok(plan);
      }
    }
  };

  let trees = git_executor.execute_command_lines(&["rev-parse", &format!("{head}^{{tree}}"), &format!("{new_head}^{{tree}}")], repo_path)?;
  let [old_tree, new_tree] = <[String; 2]>::try_from(trees).map_err(|trees| anyhow!("Unexpected rev-parse output: {trees:?}"))?;
  if old_tree != new_tree {
    let blocked = blocking_paths(git_executor, repo_path, &old_tree, &new_tree)?;
    if !blocked.is_empty() {
      plan.check.state = UpdateState::Blocked { paths: blocked };
      return Ok(plan);
    }
  }

  plan.check.state = UpdateState::Clean { kind };
  plan.new_head = Some(new_head);
  plan.old_tree = old_tree;
  plan.new_tree = new_tree;
  Ok(plan)
}

/// Move the branch to the checked update, then let the index and the working tree follow. If the files can't follow
/// (a file changed since the check), the branch moves back and nothing is changed.
#[instrument(skip(git_executor, plan), fields(branch = %plan.check.branch))]
pub fn apply_update(git_executor: &GitCommandExecutor, repo_path: &str, plan: &UpdatePlan) -> Result<AppliedUpdate> {
  let (UpdateState::Clean { kind }, Some(new_head)) = (&plan.check.state, &plan.new_head) else {
    bail!("Nothing to apply: the update is not clean");
  };
  if plan.purpose == CheckPurpose::Preview {
    bail!("A preview check can't be applied");
  }
  let check = &plan.check;
  let full_ref = format!("refs/heads/{}", check.branch);
  let mut transaction = RefTransaction::new();
  transaction.update(&full_ref, new_head, Some(&check.head));
  transaction
    .commit(git_executor, repo_path, "update project")
    .map_err(|e| anyhow!("{} changed during the update, nothing was changed: {e:#}", check.branch))?;

  if plan.old_tree != plan.new_tree
    && let Err(e) = apply_worktree_update(git_executor, repo_path, &plan.old_tree, &plan.new_tree)
  {
    let mut rollback = RefTransaction::new();
    rollback.update(&full_ref, &check.head, Some(new_head));
    if let Err(rollback_error) = rollback.commit(git_executor, repo_path, "update project rolled back") {
      bail!(
        "Files changed during the update, and {} could not be moved back to {}: {rollback_error:#}; {e:#}",
        check.branch,
        check.head
      );
    }
    bail!("Files changed during the update; nothing was changed: {e:#}");
  }

  let (replayed, dropped_already_upstream) = match kind {
    UpdateKind::FastForward => (0, 0),
    UpdateKind::Rebase => {
      let replayed = count(git_executor, repo_path, &[new_head, &format!("^{}", check.upstream_head)])?;
      let mut revs = vec![check.head.clone(), format!("^{}", check.upstream_head)];
      revs.extend(plan.excluded.iter().map(|rev| format!("^{rev}")));
      let local = count(git_executor, repo_path, &revs.iter().map(String::as_str).collect::<Vec<_>>())?;
      (replayed, local.saturating_sub(replayed))
    }
  };
  info!(?kind, replayed, dropped_already_upstream, %new_head, "updated the branch");
  Ok(AppliedUpdate {
    kind: *kind,
    new_head: new_head.clone(),
    replayed,
    dropped_already_upstream,
  })
}

/// The previous upstream tips to exclude from the local commits: the tip before a forced fetch, and the fork point of
/// HEAD in the tracking ref's reflog. Tips that are the merge base already are left out.
fn previous_upstream_tips(git_executor: &GitCommandExecutor, repo_path: &str, upstream: UpstreamRef<'_>, head: &str, base: &str) -> Result<Vec<String>> {
  let mut tips = Vec::new();
  if let Some(previous) = upstream.previous
    && previous != base
    && git_executor.commit_exists(repo_path, previous)
  {
    tips.push(previous.to_string());
  }
  // exits with 1 without a reflog or a fork point
  let (fork_point, exit_code) = git_executor.execute_command_with_status(&["merge-base", "--fork-point", upstream.tracking_ref, head], repo_path)?;
  if exit_code == 0 && fork_point != base && !tips.contains(&fork_point) {
    tips.push(fork_point);
  }
  Ok(tips)
}

/// The files that likely make the replay conflict: those that a merge of HEAD and the upstream conflicts on, or, if
/// only the replay of single commits conflicts, those changed on both sides of the merge base
fn conflicting_files(git_executor: &GitCommandExecutor, repo_path: &str, base: &str, upstream_head: &str, head: &str) -> Result<Vec<String>> {
  // `<tree>\0<file>\0...`, exit code 1 on conflicts
  let output = git_executor.execute_command_accepting(&["merge-tree", "--write-tree", "--name-only", "-z", "--no-messages", upstream_head, head], repo_path, &[1])?;
  let mut files: Vec<String> = output.split('\0').skip(1).filter(|file| !file.is_empty()).map(str::to_string).collect();
  if files.is_empty() {
    let upstream_files: HashSet<String> = changed_paths(git_executor, repo_path, base, upstream_head)?.into_iter().map(|(_, path)| path).collect();
    files = changed_paths(git_executor, repo_path, base, head)?
      .into_iter()
      .map(|(_, path)| path)
      .filter(|path| upstream_files.contains(path))
      .collect();
  }
  files.dedup();
  Ok(files)
}

/// The commits of `revs`, oldest first
fn local_commits(git_executor: &GitCommandExecutor, repo_path: &str, revs: &[&str]) -> Result<Vec<UpdateCommit>> {
  let mut args = vec!["log", "--reverse", "--format=%H %s"];
  args.extend_from_slice(revs);
  args.push("--");
  let lines = git_executor.execute_command_lines(&args, repo_path)?;
  Ok(
    lines
      .into_iter()
      .map(|line| {
        let (id, subject) = line.split_once(' ').unwrap_or((&line, ""));
        UpdateCommit {
          id: id.to_string(),
          subject: subject.to_string(),
        }
      })
      .collect(),
  )
}

/// `(status letter, path)` of every path that differs between two trees, renames as delete and add
fn changed_paths(git_executor: &GitCommandExecutor, repo_path: &str, from: &str, to: &str) -> Result<Vec<(char, String)>> {
  let output = git_executor.execute_command_raw(&["diff-tree", "-r", "-z", "--no-renames", "--name-status", from, to], repo_path)?;
  let mut fields = output.split('\0').filter(|field| !field.is_empty());
  let mut paths = Vec::new();
  while let (Some(status), Some(path)) = (fields.next(), fields.next()) {
    paths.push((status.chars().next().unwrap_or('M'), path.to_string()));
  }
  Ok(paths)
}

/// The local paths that stop the working tree from following HEAD from `old_tree` to `new_tree`, empty if it can
/// follow: the paths of a refused dry run, or files that are ignored by git but sit where the update adds a file
/// (`read-tree -u` would silently overwrite them, as `git checkout` does)
fn blocking_paths(git_executor: &GitCommandExecutor, repo_path: &str, old_tree: &str, new_tree: &str) -> Result<Vec<String>> {
  let changed = changed_paths(git_executor, repo_path, old_tree, new_tree)?;
  if let Some(refusal) = dry_run_worktree_update(git_executor, repo_path, old_tree, new_tree)? {
    let paths = overlapping_local_changes(git_executor, repo_path, &changed)?;
    if paths.is_empty() {
      bail!("The files can't be updated: {refusal}");
    }
    return Ok(paths);
  }

  let root = Path::new(repo_path);
  Ok(
    changed
      .into_iter()
      .filter(|(status, path)| *status == 'A' && is_in_the_way(root, path))
      .map(|(_, path)| path)
      .collect(),
  )
}

/// Whether something exists at `path` or a file at one of its parent directories
fn is_in_the_way(root: &Path, path: &str) -> bool {
  if root.join(path).symlink_metadata().is_ok() {
    return true;
  }
  let mut parent = Path::new(path).parent();
  while let Some(dir) = parent.filter(|dir| !dir.as_os_str().is_empty()) {
    if root.join(dir).symlink_metadata().is_ok_and(|metadata| !metadata.is_dir()) {
      return true;
    }
    parent = dir.parent();
  }
  false
}

/// The uncommitted, staged and untracked paths (`git status`) on, inside or in place of the changed paths
fn overlapping_local_changes(git_executor: &GitCommandExecutor, repo_path: &str, changed: &[(char, String)]) -> Result<Vec<String>> {
  let output = git_executor.execute_command_raw(&["status", "--porcelain=v1", "-z", "--no-renames", "--untracked-files=all"], repo_path)?;
  let changed_paths: HashSet<&str> = changed.iter().map(|(_, path)| path.as_str()).collect();
  // the directories of the changed paths: an untracked file there blocks a directory the update adds
  let mut changed_dirs: HashSet<&str> = HashSet::new();
  for (_, path) in changed {
    let mut rest = path.as_str();
    while let Some((dir, _)) = rest.rsplit_once('/') {
      if !changed_dirs.insert(dir) {
        break;
      }
      rest = dir;
    }
  }

  let mut paths = Vec::new();
  // `XY <path>`
  for entry in output.split('\0').filter(|entry| entry.len() > 3) {
    let path = &entry[3..];
    let mut overlaps = changed_paths.contains(path) || changed_dirs.contains(path);
    // a changed file where a local directory holds this path
    let mut rest = path;
    while !overlaps && let Some((dir, _)) = rest.rsplit_once('/') {
      overlaps = changed_paths.contains(dir);
      rest = dir;
    }
    if overlaps {
      paths.push(path.to_string());
    }
  }
  Ok(paths)
}

fn count(git_executor: &GitCommandExecutor, repo_path: &str, revs: &[&str]) -> Result<u32> {
  let mut args = vec!["rev-list", "--count"];
  args.extend_from_slice(revs);
  args.push("--");
  let output = git_executor.execute_command(&args, repo_path)?;
  output.parse().map_err(|e| anyhow!("Unexpected rev-list count {output:?}: {e}"))
}
