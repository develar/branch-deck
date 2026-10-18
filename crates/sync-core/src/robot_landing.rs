//! What a merged Patronus robot landed of the branches as the table has them, by content (docs/design/landing.md,
//! "Landed by content"; "Robot landing" in docs/design/git-operations.md). The app asks it when a change below the
//! robot's commit (an amend, a new commit of a branch it is built on, any change of the whole stack) gave the branches
//! new copies, so their hashes no longer tell.
//!
//! The robot tested the commits of its source revision. A commit of a branch landed when the virtual branch (or the
//! stack branch) the robot ran on has it as the robot took it: the same commit, a patch-equivalent copy, or a copy with
//! the same changes on a newer base (`SourceWalk` and `same_changes` of the robot commit map). A branch landed when its
//! oldest commits are such and only commits added after them are not: those are the work left. A commit changed since
//! (an amend), or a new one below a tested one, leaves the branch not landed: what landed is not what the branch is.

use crate::commit_key::CommitKey;
use crate::robot_commits::{HeadCommit, SourceWalk, head_commits, mapped_virtual, rebased_versions};
use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::branch_names::{to_final_branch_name, to_stack_branch_name};
use git_ops::cat_file_batch::object_ids_and_types;
use git_ops::revs::is_object_id;
use std::collections::HashMap;
use sync_types::GroupedBranchInfo;
use sync_types::robot_failure::RobotCommitState;
use sync_types::robot_landing::{BranchLanding, RobotLanding, RobotLandingRequest};
use tracing::{debug, instrument};

/// What the merged robots of `requests` landed (`RobotLanding`, one per request whose robot landed anything), the
/// commits of the branches read from HEAD as the sync groups them: the app's `get_robot_landings`, after the sync
#[instrument(skip(git_executor, requests), fields(robots = requests.len()))]
pub fn robot_landings(
  git_executor: &GitCommandExecutor,
  repository_path: &str,
  branch_prefix: &str,
  baseline_branch: &str,
  requests: &[RobotLandingRequest],
) -> Result<Vec<RobotLanding>> {
  if requests.is_empty() {
    return Ok(Vec::new());
  }
  let mut branches: Vec<&str> = requests.iter().flat_map(|request| request.branches.iter().map(String::as_str)).collect();
  branches.sort_unstable();
  branches.dedup();
  let heads = head_commits(git_executor, repository_path, baseline_branch, &branches)?;
  robot_landings_on(git_executor, repository_path, branch_prefix, baseline_branch, requests, &heads)
}

/// `robot_landings` on the grouped branches a caller read already (the report of a sync, a grouping), newest first
/// each, as `GroupedBranchInfo::commits` lists them
#[instrument(skip(git_executor, requests, grouped), fields(robots = requests.len()))]
pub fn robot_landings_of_grouped(
  git_executor: &GitCommandExecutor,
  repository_path: &str,
  branch_prefix: &str,
  baseline_branch: &str,
  requests: &[RobotLandingRequest],
  grouped: &[GroupedBranchInfo],
) -> Result<Vec<RobotLanding>> {
  if requests.is_empty() {
    return Ok(Vec::new());
  }
  let heads: Vec<HeadCommit> = grouped
    .iter()
    .filter(|branch| requests.iter().any(|request| request.branches.contains(&branch.name)))
    .flat_map(|branch| {
      branch.commits.iter().rev().map(|commit| HeadCommit {
        commit: commit.clone(),
        branch: branch.name.clone(),
      })
    })
    .collect();
  robot_landings_on(git_executor, repository_path, branch_prefix, baseline_branch, requests, &heads)
}

/// The full ref the robot of `request` ran on: the virtual branch, or the stack branch of the whole stack
fn source_ref(branch_prefix: &str, request: &RobotLandingRequest) -> Option<String> {
  let name = match (&request.branch_name, &request.stack_root) {
    (Some(branch), _) => to_final_branch_name(branch_prefix, branch).ok()?,
    (None, Some(root)) => to_stack_branch_name(branch_prefix, root).ok()?,
    (None, None) => return None,
  };
  Some(format!("refs/heads/{name}"))
}

/// How a request is answered: every commit at the robot's commit, or by the walks of `SourceWalk`
enum Answer {
  /// The ref is at the robot's commit: what it holds landed (a HEAD commit synced into it, by its mapping note)
  AtSource,
  Walked(SourceWalk),
}

/// One process per phase, the walks per robot:
/// 1. one `cat-file --batch-check` of the source revisions and the refs the robots ran on, but of a robot whose `tip`
///    the caller passed at its commit: a source the repository lacks or a ref that is gone lands nothing; a ref at the
///    source revision needs no walk;
/// 2. per other robot the two `rev-list` walks of `SourceWalk`;
/// 3. one `same_changes` (a `diff-tree` and a `patch-id`) of the new commits with the identity of a robot commit, of all
///    robots, none without such pairs;
/// 4. per branch, oldest first: its commits tested or rebased, then only commits added after them (or not synced yet).
fn robot_landings_on(
  git_executor: &GitCommandExecutor,
  repository_path: &str,
  branch_prefix: &str,
  baseline_branch: &str,
  requests: &[RobotLandingRequest],
  heads: &[HeadCommit],
) -> Result<Vec<RobotLanding>> {
  let refs: Vec<Option<String>> = requests.iter().map(|request| source_ref(branch_prefix, request)).collect();
  // a robot whose ref the caller knows at its commit needs no git; the others' sources and refs are looked up
  let asked: Vec<usize> = (0..requests.len())
    .filter(|&index| {
      let request = &requests[index];
      refs[index].is_some() && is_object_id(&request.source_revision) && request.tip.as_deref() != Some(request.source_revision.as_str())
    })
    .collect();
  let names: Vec<&str> = asked
    .iter()
    .flat_map(|&index| [requests[index].source_revision.as_str(), refs[index].as_deref().unwrap_or_default()])
    .collect();
  let found = object_ids_and_types(git_executor, repository_path, &names)?;
  let commit_of = |entry: &Option<(String, &'static str)>| entry.as_ref().filter(|(_, kind)| *kind == "commit").map(|(id, _)| id.clone());
  // by request: whether the repository has the source revision, and the tip of the ref
  let resolved: HashMap<usize, (bool, Option<String>)> = asked
    .iter()
    .enumerate()
    .map(|(position, &index)| (index, (commit_of(&found[position * 2]).is_some(), commit_of(&found[position * 2 + 1]))))
    .collect();

  let mut answers: Vec<Option<Answer>> = Vec::with_capacity(requests.len());
  for (index, request) in requests.iter().enumerate() {
    let Some(full_ref) = refs[index].as_deref().filter(|_| is_object_id(&request.source_revision)) else {
      answers.push(None);
      continue;
    };
    if request.tip.as_deref() == Some(request.source_revision.as_str()) {
      answers.push(Some(Answer::AtSource));
      continue;
    }
    let (source_present, tip) = resolved.get(&index).cloned().unwrap_or((false, None));
    answers.push(match tip {
      Some(tip) if tip == request.source_revision => Some(Answer::AtSource),
      Some(_) if source_present => Some(Answer::Walked(SourceWalk::read(
        git_executor,
        repository_path,
        full_ref,
        &request.source_revision,
        baseline_branch,
        true,
      )?)),
      // the robot started on another machine, a gc pruned its commit, or the ref is gone: nothing tells
      _ => None,
    });
  }

  let versions: Vec<(&str, &str)> = answers
    .iter()
    .flat_map(|answer| match answer {
      Some(Answer::Walked(walked)) => walked.versions(),
      _ => Vec::new(),
    })
    .collect();
  let rebased = rebased_versions(git_executor, repository_path, &versions)?;

  let landings: Vec<RobotLanding> = requests
    .iter()
    .zip(&answers)
    .filter_map(|(request, answer)| {
      let answer = answer.as_ref()?;
      let state_by_key: HashMap<CommitKey, RobotCommitState> = match answer {
        Answer::AtSource => HashMap::new(),
        Answer::Walked(walked) => walked
          .virtual_commits
          .iter()
          .map(|commit| (commit.key.clone(), walked.state_of(commit, &rebased)))
          .collect(),
      };
      let state_of = |head: &HeadCommit| match answer {
        // synced into the ref at the robot's commit: its mapping note names its copy
        Answer::AtSource => mapped_virtual(&head.commit).map(|_| RobotCommitState::Tested),
        Answer::Walked(_) => state_by_key
          .get(&CommitKey::from_subject(&head.commit.author_email, head.commit.author_timestamp, &head.commit.subject))
          .copied(),
      };
      let branches: Vec<BranchLanding> = request
        .branches
        .iter()
        .filter_map(|name| {
          let commits: Vec<&HeadCommit> = heads.iter().filter(|head| &head.branch == name).collect();
          branch_landing(name, &commits, state_of)
        })
        .collect();
      (!branches.is_empty()).then(|| RobotLanding {
        robot_id: request.robot_id.clone(),
        branches,
      })
    })
    .collect();
  debug!(landings = landings.len(), "robot landings");
  Ok(landings)
}

/// The landing of a branch from the states of its commits, oldest first (`state_of`; `None`: no commit of the ref has
/// it, made after the last sync): the tested and rebased ones first, then only added ones, which are the work left.
/// `None` when nothing of it was tested, a commit changed since, or a new commit sits below a tested one.
fn branch_landing(name: &str, commits: &[&HeadCommit], state_of: impl Fn(&HeadCommit) -> Option<RobotCommitState>) -> Option<BranchLanding> {
  let mut landed = Vec::new();
  let mut after = Vec::new();
  for head in commits {
    match state_of(head) {
      Some(RobotCommitState::Tested | RobotCommitState::Rebased) if after.is_empty() => landed.push(head.commit.id.clone()),
      Some(RobotCommitState::Added) | None => after.push(head.commit.id.clone()),
      _ => return None,
    }
  }
  if landed.is_empty() {
    return None;
  }
  landed.reverse();
  after.reverse();
  Some(BranchLanding {
    branch: name.to_string(),
    landed,
    after,
  })
}

#[cfg(test)]
#[path = "robot_landing_test.rs"]
mod robot_landing_test;
