//! The commit map of a Patronus robot: which commits of a branch the robot checked, by the hashes of the checked-out
//! branch (`spec/branch/safe-push-failure-prompt.spec.md`, docs/design/git-operations.md, "Robot commit map").
//!
//! The robot ran on `source_revision`, the tip of the virtual branch when it started. Since then the branch may have
//! been amended, extended, rebuilt on a newer baseline, or a commit of it may have landed in the baseline. The virtual
//! branch is compared with the source revision by content, and its commits are translated to the commits of the
//! checked-out branch they are copies of: an agent edits those, no hash the CI reports is local.

use crate::commit_key::CommitKey;
use crate::commit_walk::{WalkedCommit, walk_commits};
use crate::remote_status::same_changes;
use crate::review_fetch::present_commits;
use crate::snapshot::read_grouping_inputs;
use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::commit_list::{Commit, get_commit_list};
use git_ops::model::to_final_branch_name;
use git_ops::revs::{current_branch, is_object_id};
use std::collections::{BTreeSet, HashMap, HashSet};
use sync_types::robot_failure::{RobotCommit, RobotCommitMap, RobotCommitState, RobotFailure, RobotFailureReport};
use tracing::{debug, instrument};

/// The commits of a `rev-list` walk, newest first, each with its mark
fn walk(git_executor: &GitCommandExecutor, repository_path: &str, options: &[&str], revs: &[&str]) -> Result<Vec<WalkedCommit>> {
  walk_commits(git_executor, repository_path, &[&["--topo-order"], options].concat(), revs)
}

/// A commit of the checked-out branch with the virtual branch it is grouped into
struct HeadCommit {
  commit: Commit,
  branch: String,
}

/// The commits of `branches` on HEAD, oldest first, grouped as the sync groups them, with their mapping notes. Three
/// or four processes: the stack setting and one `cat-file --batch` for the issue groups and the stack parents, one `log`
/// of the commits ahead of the baseline (plus a `rev-parse` on a local baseline)
fn head_commits(git_executor: &GitCommandExecutor, repository_path: &str, baseline_branch: &str, branches: &[&str]) -> Result<Vec<HeadCommit>> {
  let grouping = read_grouping_inputs(git_executor, repository_path)?;
  Ok(
    get_commit_list(git_executor, repository_path, baseline_branch)?
      .into_iter()
      .filter_map(|commit| {
        let branch = grouping.branch_of(&commit.subject).filter(|branch| branches.contains(&branch.as_str()))?;
        Some(HeadCommit { commit, branch })
      })
      .collect(),
  )
}

/// The virtual commit a HEAD commit's mapping note names, unless the note is stale: one copied onto a commit with
/// another tree (`notes.rewriteRef` on an amend) records the source it was copied from
fn mapped_virtual(commit: &Commit) -> Option<&str> {
  let mapped = commit.mapped_commit_id.as_deref()?;
  match &commit.mapped_source {
    Some(source) if source.tree != commit.tree_id => None,
    _ => Some(mapped),
  }
}

/// The identities of the baseline commits since `source_revision` by the authors of `commits`: one `rev-list`, the
/// authors as fixed strings, so a monorepo baseline stays small (as `landed` reads the upstream)
fn landed_keys(git_executor: &GitCommandExecutor, repository_path: &str, baseline_branch: &str, source_revision: &str, commits: &[&WalkedCommit]) -> Result<HashSet<CommitKey>> {
  let authors: BTreeSet<String> = commits
    .iter()
    .filter(|commit| !commit.key.email.is_empty())
    .map(|commit| format!("--author=<{}>", commit.key.email))
    .collect();
  if authors.is_empty() {
    return Ok(HashSet::new());
  }
  let exclusion = format!("^{source_revision}");
  let mut options = vec!["--fixed-strings"];
  options.extend(authors.iter().map(String::as_str));
  Ok(
    walk(git_executor, repository_path, &options, &[baseline_branch, exclusion.as_str()])?
      .into_iter()
      .map(|commit| commit.key)
      .collect(),
  )
}

/// The commits of the branch against those its robot checked, by the hashes of the checked-out branch.
///
/// `branch_name` is the virtual branch without the `{branch_prefix}/virtual/` namespace, `source_revision` the commit
/// the robot started on (`PatronusRobot::source_revision`), `ancestors` the branches a stacked branch is built on (root
/// first). The robot of a stacked branch builds the whole stack, so the map has the ancestors' commits too, each with
/// its branch: a failure may come from a commit of a parent.
///
/// One process per phase:
/// 1. one `cat-file --batch-check` for the source revision and the virtual branch; a missing source revision lists the
///    commits of the checked-out branch as `unknown`;
/// 2. the commits of the branch and its ancestors on HEAD with their mapping notes, grouped as the sync groups them
///    (`head_commits`);
/// 3. one `rev-list` of the virtual branch and one `rev-list --left-right --cherry-mark` of
///    `<source>...<virtual branch>`, both without the baseline: a commit both reach, or a patch-equivalent copy (`=`), is
///    `tested`;
/// 4. a new commit and a robot commit with one identity (`CommitKey`) are compared without context (`same_changes`):
///    `rebased` when the changes are the same, else `changed`. `--cherry-mark` compares patch ids with context, which a
///    rebuild on a newer baseline moves;
/// 5. the other robot commits: `landed` when a baseline commit since the source revision has the identity (one
///    `rev-list` by their authors), else `removed`; the other new commits are `added`.
///
/// A virtual commit is shown by the HEAD commit whose mapping note names it, else by the HEAD commit of its identity.
/// A HEAD commit no note maps into the virtual branch was made or changed after the last sync: `virtual_behind_head`,
/// and a commit shown by it is `changed`, not `tested`.
#[instrument(skip(git_executor))]
pub fn robot_commit_map(
  git_executor: &GitCommandExecutor,
  repository_path: &str,
  branch_prefix: &str,
  branch_name: &str,
  source_revision: &str,
  baseline_branch: &str,
  ancestors: &[String],
) -> Result<RobotCommitMap> {
  let virtual_ref = format!("refs/heads/{}", to_final_branch_name(branch_prefix, branch_name)?);
  let [source_present, virtual_present] = if is_object_id(source_revision) {
    present_commits(git_executor, repository_path, [source_revision, virtual_ref.as_str()])?
  } else {
    [false, false]
  };

  let mut stack: Vec<&str> = ancestors.iter().map(String::as_str).collect();
  stack.push(branch_name);
  let head = head_commits(git_executor, repository_path, baseline_branch, &stack)?;
  if !source_present {
    debug!("the source revision is not in the repository");
    return Ok(RobotCommitMap {
      commits: head
        .iter()
        .rev()
        .map(|head| RobotCommit {
          local_id: Some(head.commit.id.clone()),
          subject: head.commit.subject.clone(),
          author_time: head.commit.author_timestamp,
          branch: Some(head.branch.clone()),
          state: RobotCommitState::Unknown,
        })
        .collect(),
      source_missing: true,
      virtual_behind_head: false,
    });
  }

  let baseline_exclusion = format!("^{baseline_branch}");

  // the commits of the virtual branch, and the marks of those that differ from the source revision (`>`), and of the
  // robot's own (`<`). Without a virtual branch (archived, deleted) every robot commit is the robot's own
  let (virtual_commits, marked) = if virtual_present {
    let virtual_commits = walk(git_executor, repository_path, &[], &[&baseline_exclusion, &virtual_ref])?;
    let range = format!("{source_revision}...{virtual_ref}");
    let marked = walk(git_executor, repository_path, &["--left-right", "--cherry-mark"], &[&baseline_exclusion, &range])?;
    (virtual_commits, marked)
  } else {
    let source_commits = walk(git_executor, repository_path, &[], &[&baseline_exclusion, source_revision])?;
    (Vec::new(), source_commits.into_iter().map(|commit| WalkedCommit { mark: '<', ..commit }).collect())
  };
  let virtual_marks: HashMap<&str, char> = marked.iter().map(|commit| (commit.id.as_str(), commit.mark)).collect();
  let robot_only: Vec<&WalkedCommit> = marked.iter().filter(|commit| commit.mark == '<').collect();

  // a new commit with the identity of a robot commit is a version of it: rebased, or changed
  let robot_by_key: HashMap<&CommitKey, &WalkedCommit> = robot_only.iter().map(|commit| (&commit.key, *commit)).collect();
  let versions: Vec<(&str, &str)> = virtual_commits
    .iter()
    .filter(|commit| virtual_marks.get(commit.id.as_str()) == Some(&'>'))
    .filter_map(|commit| robot_by_key.get(&commit.key).map(|robot| (robot.id.as_str(), commit.id.as_str())))
    .collect();
  let rebased: HashMap<&str, bool> = versions
    .iter()
    .map(|(_, newer)| *newer)
    .zip(same_changes(git_executor, repository_path, &versions)?)
    .collect();
  let paired: HashSet<&str> = versions.iter().map(|(older, _)| *older).collect();

  let remaining: Vec<&WalkedCommit> = robot_only.into_iter().filter(|commit| !paired.contains(commit.id.as_str())).collect();
  let landed = if remaining.is_empty() {
    HashSet::new()
  } else {
    landed_keys(git_executor, repository_path, baseline_branch, source_revision, &remaining)?
  };

  // the HEAD commit of each virtual commit: by its mapping note, else by its identity
  let by_note: HashMap<&str, &HeadCommit> = head.iter().filter_map(|head| mapped_virtual(&head.commit).map(|mapped| (mapped, head))).collect();
  let by_key: HashMap<CommitKey, &HeadCommit> = head
    .iter()
    .map(|head| (CommitKey::from_subject(&head.commit.author_email, head.commit.author_timestamp, &head.commit.subject), head))
    .collect();
  let mut shown: HashSet<&str> = HashSet::new();
  let mut virtual_behind_head = false;
  let mut commits: Vec<RobotCommit> = Vec::with_capacity(virtual_commits.len() + remaining.len());
  for commit in &virtual_commits {
    let mut state = match virtual_marks.get(commit.id.as_str()) {
      Some('>') => match rebased.get(commit.id.as_str()) {
        Some(true) => RobotCommitState::Rebased,
        Some(false) => RobotCommitState::Changed,
        None => RobotCommitState::Added,
      },
      // reached from both, or a patch-equivalent copy (`=`)
      _ => RobotCommitState::Tested,
    };
    let local = by_note.get(commit.id.as_str()).or_else(|| by_key.get(&commit.key)).copied();
    if let Some(local) = local {
      shown.insert(local.commit.id.as_str());
      // the HEAD commit changed since the sync copied it
      if mapped_virtual(&local.commit) != Some(commit.id.as_str()) {
        virtual_behind_head = true;
        if matches!(state, RobotCommitState::Tested | RobotCommitState::Rebased) {
          state = RobotCommitState::Changed;
        }
      }
    }
    commits.push(RobotCommit {
      local_id: local.map(|local| local.commit.id.clone()),
      subject: local.map_or_else(|| commit.subject.clone(), |local| local.commit.subject.clone()),
      author_time: commit.key.time,
      branch: local.map(|local| local.branch.clone()),
      state,
    });
  }
  virtual_behind_head |= head.iter().any(|head| !shown.contains(head.commit.id.as_str()));

  commits.extend(remaining.iter().map(|commit| RobotCommit {
    local_id: None,
    subject: commit.subject.clone(),
    author_time: commit.key.time,
    branch: None,
    state: if landed.contains(&commit.key) {
      RobotCommitState::Landed
    } else {
      RobotCommitState::Removed
    },
  }));
  debug!(commits = commits.len(), virtual_behind_head, "robot commit map");
  Ok(RobotCommitMap {
    commits,
    source_missing: false,
    virtual_behind_head,
  })
}

#[cfg(test)]
#[path = "robot_commits_test.rs"]
mod robot_commits_test;

/// The failure of a robot with the commit map of its branch and the checked-out branch, for the prompt for an agent: the
/// app and `bd robot` build it alike. No map without a branch or a source revision. One process besides the map:
/// `symbolic-ref` names the checked-out branch.
#[instrument(skip(git_executor, failure), fields(robot = %failure.robot.id))]
pub fn robot_failure_report(
  git_executor: &GitCommandExecutor,
  repository_path: &str,
  branch_prefix: &str,
  ancestors: &[String],
  baseline_branch: &str,
  failure: RobotFailure,
) -> Result<RobotFailureReport> {
  let commits = match (failure.robot.branch_name.as_deref(), failure.robot.source_revision.as_deref()) {
    (Some(branch_name), Some(source_revision)) => Some(robot_commit_map(
      git_executor,
      repository_path,
      branch_prefix,
      branch_name,
      source_revision,
      baseline_branch,
      ancestors,
    )?),
    _ => None,
  };
  let head_branch = current_branch(git_executor, repository_path)?;
  Ok(RobotFailureReport { failure, commits, head_branch })
}
