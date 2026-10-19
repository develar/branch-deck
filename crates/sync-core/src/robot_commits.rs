//! The commit map of a Patronus robot: which commits of a branch the robot checked, by the hashes of the checked-out
//! branch (`spec/branch/safe-push-failure-prompt.spec.md`, docs/design/git-operations.md, "Robot commit map").
//!
//! The robot ran on `source_revision`, the tip of the virtual branch when it started. Since then the branch may have
//! been amended, extended, rebuilt on a newer baseline, or a commit of it may have landed in the baseline. The virtual
//! branch is compared with the source revision by content, and its commits are translated to the commits of the
//! checked-out branch they are copies of: an agent edits those, no hash the CI reports is local.

use crate::commit_key::CommitKey;
use crate::commit_walk::{WalkedCommit, walk_commits, walk_commits_by_authors};
use crate::remote_status::same_changes;
use crate::review_fetch::present_commits;
use crate::snapshot::read_grouping_inputs;
use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::branch_names::{to_final_branch_name, to_stack_branch_name};
use git_ops::commit_list::{Commit, get_commit_list};
use git_ops::revs::{current_branch, is_object_id};
use std::collections::{HashMap, HashSet};
use sync_types::robot_failure::{RobotCommit, RobotCommitMap, RobotCommitState, RobotFailure, RobotFailureReport};
use tracing::{debug, instrument};

/// The commits of a `rev-list` walk, newest first, each with its mark
fn walk(git_executor: &GitCommandExecutor, repository_path: &str, options: &[&str], revs: &[&str]) -> Result<Vec<WalkedCommit>> {
  walk_commits(git_executor, repository_path, &[&["--topo-order"], options].concat(), revs)
}

/// A commit of the checked-out branch with the virtual branch it is grouped into
pub(crate) struct HeadCommit {
  pub(crate) commit: Commit,
  pub(crate) branch: String,
}

/// The commits of `branches` on HEAD, oldest first, grouped as the sync groups them, with their mapping notes. Three
/// or four processes: the stack setting and one `cat-file --batch` for the issue groups and the stack parents, one `log`
/// of the commits ahead of the baseline (plus a `rev-parse` on a local baseline)
pub(crate) fn head_commits(git_executor: &GitCommandExecutor, repository_path: &str, baseline_branch: &str, branches: &[&str]) -> Result<Vec<HeadCommit>> {
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
pub(crate) fn mapped_virtual(commit: &Commit) -> Option<&str> {
  let mapped = commit.mapped_commit_id.as_deref()?;
  match &commit.mapped_source {
    Some(source) if source.tree != commit.tree_id => None,
    _ => Some(mapped),
  }
}

/// The identities of the baseline commits since `source_revision` by the authors of `commits`: one `rev-list`, the
/// authors as fixed strings, so a monorepo baseline stays small (as `landed` reads the upstream)
fn landed_keys(git_executor: &GitCommandExecutor, repository_path: &str, baseline_branch: &str, source_revision: &str, commits: &[&WalkedCommit]) -> Result<HashSet<CommitKey>> {
  let exclusion = format!("^{source_revision}");
  let emails = commits.iter().map(|commit| commit.key.email.as_str());
  Ok(
    walk_commits_by_authors(git_executor, repository_path, emails, &[baseline_branch, exclusion.as_str()])?
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
  let mut stack: Vec<&str> = ancestors.iter().map(String::as_str).collect();
  stack.push(branch_name);
  commit_map(git_executor, repository_path, &virtual_ref, &stack, source_revision, baseline_branch, MapBy::Notes)
}

/// The commits of the whole stack of `root` against those the robot of its stack branch (`{prefix}/stack/<root>`)
/// checked, by the hashes of the checked-out branch: the commits of every member (`members`, the root first), each
/// with its branch, in the phases of `robot_commit_map`. The stack branch has no mapping notes: its commits are shown
/// by the HEAD commits of their identity, and a HEAD commit of a member that no commit of the stack branch shows was
/// made after the last sync (`virtual_behind_head`).
#[instrument(skip(git_executor))]
pub fn stack_robot_commit_map(
  git_executor: &GitCommandExecutor,
  repository_path: &str,
  branch_prefix: &str,
  root: &str,
  source_revision: &str,
  baseline_branch: &str,
  members: &[String],
) -> Result<RobotCommitMap> {
  let stack_ref = format!("refs/heads/{}", to_stack_branch_name(branch_prefix, root)?);
  let branches: Vec<&str> = members.iter().map(String::as_str).collect();
  commit_map(git_executor, repository_path, &stack_ref, &branches, source_revision, baseline_branch, MapBy::Identity)
}

/// How the commits of the source branch are shown by the HEAD commits they are copies of
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MapBy {
  /// The mapping note of a HEAD commit names its copy in the virtual branch, else its identity
  Notes,
  /// Only the identity (`CommitKey`): the stack branch writes no mapping notes
  Identity,
}

/// The commits of a virtual branch (or the stack branch) against the source revision of a robot: phase 3 of
/// `robot_commit_map`, two `rev-list` walks without the baseline
pub(crate) struct SourceWalk {
  /// The commits of the virtual branch, newest first; empty without one
  pub(crate) virtual_commits: Vec<WalkedCommit>,
  /// `--left-right --cherry-mark` of `<source>...<virtual branch>`: `<` a robot commit the virtual branch lacks, `>` a
  /// virtual commit the robot lacks, `=` a patch-equivalent copy. Without a virtual branch every robot commit, as `<`
  marked: Vec<WalkedCommit>,
  /// By commit id, the mark of `marked`
  marks: HashMap<String, char>,
}

impl SourceWalk {
  /// The two walks; without a virtual branch (archived, deleted) one walk of the robot's commits
  pub(crate) fn read(
    git_executor: &GitCommandExecutor,
    repository_path: &str,
    virtual_ref: &str,
    source_revision: &str,
    baseline_branch: &str,
    virtual_present: bool,
  ) -> Result<Self> {
    let baseline_exclusion = format!("^{baseline_branch}");
    let (virtual_commits, marked) = if virtual_present {
      let virtual_commits = walk(git_executor, repository_path, &[], &[&baseline_exclusion, virtual_ref])?;
      let range = format!("{source_revision}...{virtual_ref}");
      let marked = walk(git_executor, repository_path, &["--left-right", "--cherry-mark"], &[&baseline_exclusion, &range])?;
      (virtual_commits, marked)
    } else {
      let source_commits = walk(git_executor, repository_path, &[], &[&baseline_exclusion, source_revision])?;
      (Vec::new(), source_commits.into_iter().map(|commit| WalkedCommit { mark: '<', ..commit }).collect())
    };
    let marks = marked.iter().map(|commit| (commit.id.clone(), commit.mark)).collect();
    Ok(Self { virtual_commits, marked, marks })
  }

  /// The robot commits the virtual branch lacks
  pub(crate) fn robot_only(&self) -> impl Iterator<Item = &WalkedCommit> {
    self.marked.iter().filter(|commit| commit.mark == '<')
  }

  /// `(robot commit, virtual commit)` of one identity (`CommitKey`), the virtual commit new to the robot: a version of
  /// it, rebased or changed, as `same_changes` tells
  pub(crate) fn versions(&self) -> Vec<(&str, &str)> {
    let robot_by_key: HashMap<&CommitKey, &WalkedCommit> = self.robot_only().map(|commit| (&commit.key, commit)).collect();
    self
      .virtual_commits
      .iter()
      .filter(|commit| self.marks.get(&commit.id) == Some(&'>'))
      .filter_map(|commit| robot_by_key.get(&commit.key).map(|robot| (robot.id.as_str(), commit.id.as_str())))
      .collect()
  }

  /// The state of a commit of the virtual branch: `tested` when both reach it or it is a patch-equivalent copy (`=`);
  /// a new one is `rebased` or `changed` by `rebased` (the answer of `same_changes` for its version), else `added`
  pub(crate) fn state_of(&self, commit: &WalkedCommit, rebased: &HashMap<&str, bool>) -> RobotCommitState {
    match self.marks.get(&commit.id) {
      Some('>') => match rebased.get(commit.id.as_str()) {
        Some(true) => RobotCommitState::Rebased,
        Some(false) => RobotCommitState::Changed,
        None => RobotCommitState::Added,
      },
      // reached from both, or a patch-equivalent copy (`=`)
      _ => RobotCommitState::Tested,
    }
  }
}

/// By the newer commit of each `(robot commit, newer commit)` version pair (`SourceWalk::versions`), whether it has the
/// same changes (`same_changes`, one `diff-tree` and one `patch-id`, none without pairs): rebased, else changed
pub(crate) fn rebased_versions<'a>(git_executor: &GitCommandExecutor, repository_path: &str, versions: &[(&'a str, &'a str)]) -> Result<HashMap<&'a str, bool>> {
  Ok(
    versions
      .iter()
      .map(|(_, newer)| *newer)
      .zip(same_changes(git_executor, repository_path, versions)?)
      .collect(),
  )
}

/// The map of the commits of `virtual_ref` (the virtual branch, or the stack branch) against `source_revision`, the
/// commits of `branches` on HEAD showing them (`robot_commit_map`)
fn commit_map(
  git_executor: &GitCommandExecutor,
  repository_path: &str,
  virtual_ref: &str,
  branches: &[&str],
  source_revision: &str,
  baseline_branch: &str,
  map_by: MapBy,
) -> Result<RobotCommitMap> {
  let [source_present, virtual_present] = if is_object_id(source_revision) {
    present_commits(git_executor, repository_path, [source_revision, virtual_ref])?
  } else {
    [false, false]
  };

  let head = head_commits(git_executor, repository_path, baseline_branch, branches)?;
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

  let walked = SourceWalk::read(git_executor, repository_path, virtual_ref, source_revision, baseline_branch, virtual_present)?;
  let versions = walked.versions();
  let rebased = rebased_versions(git_executor, repository_path, &versions)?;
  let paired: HashSet<&str> = versions.iter().map(|(older, _)| *older).collect();
  let virtual_commits = &walked.virtual_commits;

  let remaining: Vec<&WalkedCommit> = walked.robot_only().filter(|commit| !paired.contains(commit.id.as_str())).collect();
  let landed = if remaining.is_empty() {
    HashSet::new()
  } else {
    landed_keys(git_executor, repository_path, baseline_branch, source_revision, &remaining)?
  };

  // the HEAD commit of each virtual commit: by its mapping note, else by its identity
  let by_note: HashMap<&str, &HeadCommit> = match map_by {
    MapBy::Notes => head.iter().filter_map(|head| mapped_virtual(&head.commit).map(|mapped| (mapped, head))).collect(),
    MapBy::Identity => HashMap::new(),
  };
  let by_key: HashMap<CommitKey, &HeadCommit> = head
    .iter()
    .map(|head| (CommitKey::from_subject(&head.commit.author_email, head.commit.author_timestamp, &head.commit.subject), head))
    .collect();
  let mut shown: HashSet<&str> = HashSet::new();
  let mut virtual_behind_head = false;
  let mut commits: Vec<RobotCommit> = Vec::with_capacity(virtual_commits.len() + remaining.len());
  for commit in virtual_commits {
    let mut state = walked.state_of(commit, &rebased);
    let local = by_note.get(commit.id.as_str()).or_else(|| by_key.get(&commit.key)).copied();
    if let Some(local) = local {
      shown.insert(local.commit.id.as_str());
      // the HEAD commit changed since the sync copied it; the stack branch has no notes that would say it
      if map_by == MapBy::Notes && mapped_virtual(&local.commit) != Some(commit.id.as_str()) {
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
pub(crate) mod robot_commits_test;

/// The failure of a robot with the commit map of its branch and the checked-out branch, for the prompt for an agent: the
/// app and `bd robot` build it alike. For a robot of the whole stack (`stack_root`), `ancestors` are the members of the
/// stack, the root first (`stack_robot_commit_map`). No map without a branch, a stack with members, or a source
/// revision. One process besides the map: `symbolic-ref` names the checked-out branch.
#[instrument(skip(git_executor, failure), fields(robot = %failure.robot.id))]
pub fn robot_failure_report(
  git_executor: &GitCommandExecutor,
  repository_path: &str,
  branch_prefix: &str,
  ancestors: &[String],
  baseline_branch: &str,
  failure: RobotFailure,
) -> Result<RobotFailureReport> {
  let robot = &failure.robot;
  let commits = match (robot.branch_name.as_deref(), robot.stack_root.as_deref(), robot.source_revision.as_deref()) {
    (Some(branch_name), _, Some(source_revision)) => Some(robot_commit_map(
      git_executor,
      repository_path,
      branch_prefix,
      branch_name,
      source_revision,
      baseline_branch,
      ancestors,
    )?),
    (None, Some(root), Some(source_revision)) if !ancestors.is_empty() => Some(stack_robot_commit_map(
      git_executor,
      repository_path,
      branch_prefix,
      root,
      source_revision,
      baseline_branch,
      ancestors,
    )?),
    _ => None,
  };
  let head_branch = current_branch(git_executor, repository_path)?;
  Ok(RobotFailureReport { failure, commits, head_branch })
}
