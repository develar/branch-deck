use crate::commit_grouper::parse_subject;
use crate::regroup::{RegroupOptions, RegroupPlan, RegroupedCommit, regroup_commits_core};
use anyhow::{Result, ensure};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::model::sanitize_branch_name;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use tracing::{info, instrument};

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct MoveCommitsParams {
  pub repository_path: String,
  /// Commits on the current branch (the original commits of virtual branch commits)
  pub commit_ids: Vec<String>,
  /// The explicit branch prefix to set; `None` removes it, so the commits fall back to their issue branch or to the
  /// unassigned commits
  pub target_branch: Option<String>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct MovedCommit {
  /// The request commit id
  pub old_id: String,
  pub new_id: String,
  /// The message before the move, for an undo
  pub previous_message: String,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct MoveCommitsResult {
  /// The commits whose message changed, in request order, then the fixups that moved with them; a commit already in
  /// the target branch is not listed
  pub moved: Vec<MovedCommit>,
  /// The issue groups before and after, when the operation changed them (a rename of an issue group): an undo moves
  /// them back with `undo_issue_groups`
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub issue_groups: Option<IssueGroupsMove>,
}

/// A change of the issue groups (`refs/branch-deck/issue-groups`), for an undo
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct IssueGroupsMove {
  /// The commit of the issue groups before; `None` when there were none
  pub previous: Option<String>,
  /// The commit the operation wrote
  pub new: String,
}

/// Moves commits to another branch by setting the `(branch)` prefix of their subjects, replacing an existing one: a
/// `regroup_commits_core` plan of one branch (or of `unassign`), so their fixups move along. One history rewrite for all
/// commits; the next sync rebuilds the virtual branches.
#[instrument(skip(git_executor, params), fields(count = params.commit_ids.len(), target = ?params.target_branch))]
pub fn move_commits_to_branch_core(git_executor: &GitCommandExecutor, params: &MoveCommitsParams) -> Result<MoveCommitsResult> {
  let target = params.target_branch.as_deref();
  if let Some(target) = target {
    validate_branch_name(target)?;
  }
  ensure!(!params.commit_ids.is_empty(), "No commits to move");

  let plan = match target {
    Some(target) => RegroupPlan {
      branches: BTreeMap::from([(target.to_string(), params.commit_ids.clone())]),
      ..RegroupPlan::default()
    },
    None => RegroupPlan {
      unassign: params.commit_ids.clone(),
      ..RegroupPlan::default()
    },
  };
  let outcome = regroup_commits_core(git_executor, &params.repository_path, &plan, &RegroupOptions::default())?;

  // the requested commits in request order, then the fixups that followed them
  let changed = |commit: &&RegroupedCommit| commit.changed;
  let requested: Vec<(&str, &RegroupedCommit)> = params
    .commit_ids
    .iter()
    .filter_map(|id| {
      outcome
        .commits
        .iter()
        .filter(changed)
        .find(|commit| commit.id.starts_with(id.as_str()))
        .map(|commit| (id.as_str(), commit))
    })
    .collect();
  let followed = outcome
    .commits
    .iter()
    .filter(changed)
    .filter(|commit| !requested.iter().any(|(_, requested)| requested.id == commit.id));
  let moved: Vec<MovedCommit> = requested
    .iter()
    .map(|(id, commit)| (*id, *commit))
    .chain(followed.map(|commit| (commit.id.as_str(), commit)))
    .map(|(id, commit)| MovedCommit {
      old_id: id.to_string(),
      new_id: commit.new_id.clone(),
      previous_message: commit.previous_message.clone(),
    })
    .collect();
  info!(moved = moved.len(), "Moved commits");
  Ok(MoveCommitsResult { moved, issue_groups: None })
}

/// The message with the explicit branch prefix of its subject set to `target` (or removed): `fixup! (a) X` becomes
/// `fixup! (b) X`, `ABC-1 X` becomes `(b) ABC-1 X`. The body is kept as is.
pub fn with_branch_prefix(message: &str, target: Option<&str>) -> String {
  let (subject, body) = match message.split_once('\n') {
    Some((subject, body)) => (subject, Some(body)),
    None => (message, None),
  };
  let parsed = parse_subject(subject);
  let mut result = String::with_capacity(message.len() + target.map_or(0, |t| t.len() + 3));
  result.push_str(parsed.autosquash);
  if let Some(target) = target {
    result.push('(');
    result.push_str(target);
    result.push_str(") ");
  }
  result.push_str(parsed.rest);
  if let Some(body) = body {
    result.push('\n');
    result.push_str(body);
  }
  result
}

/// A name the grouper keeps as is: Git-safe characters only, so the virtual branch gets exactly this name
pub fn validate_branch_name(name: &str) -> Result<()> {
  ensure!(!name.is_empty(), "Branch name cannot be empty");
  ensure!(
    name.chars().all(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | '.' | '/')),
    "Branch name can only contain letters, numbers, hyphens, underscores, dots and slashes"
  );
  ensure!(!name.starts_with('.'), "Branch name cannot start with a dot");
  ensure!(
    sanitize_branch_name(name) == name && !name.contains("//") && !name.contains(".."),
    "'{name}' is not a valid branch name"
  );
  Ok(())
}

#[cfg(test)]
#[path = "move_commits_test.rs"]
mod move_commits_test;
