use crate::commit_grouper::parse_subject;
use crate::reword_commit::reword_messages;
use anyhow::{Result, anyhow, ensure};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::commit_utils::get_commits_info;
use git_ops::model::sanitize_branch_name;
use serde::{Deserialize, Serialize};
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
  /// The commits whose message changed, in request order; a commit already in the target branch is not listed
  pub moved: Vec<MovedCommit>,
}

/// Moves commits to another branch by setting the `(branch)` prefix of their subjects, replacing an existing one.
/// One history rewrite for all commits; the next sync rebuilds the virtual branches.
#[instrument(skip(git_executor, params), fields(count = params.commit_ids.len(), target = ?params.target_branch))]
pub fn move_commits_to_branch_core(git_executor: &GitCommandExecutor, params: &MoveCommitsParams) -> Result<MoveCommitsResult> {
  let target = params.target_branch.as_deref();
  if let Some(target) = target {
    validate_branch_name(target)?;
  }
  ensure!(!params.commit_ids.is_empty(), "No commits to move");

  let ids: Vec<&str> = params.commit_ids.iter().map(String::as_str).collect();
  let commits = get_commits_info(git_executor, &params.repository_path, &ids)?;
  // (request id, previous message, new message)
  let mut rewrites = Vec::with_capacity(ids.len());
  for id in ids {
    let commit = commits.values().find(|commit| commit.id.starts_with(id)).ok_or_else(|| anyhow!("Commit {id} not found"))?;
    let previous_message = commit.message.trim_end().to_string();
    let new_message = with_branch_prefix(&previous_message, target);
    if new_message != previous_message {
      rewrites.push((id, previous_message, new_message));
    }
  }

  let new_ids = reword_messages(
    git_executor,
    &params.repository_path,
    rewrites.iter().map(|(id, _, new_message)| (*id, new_message.as_str())),
  )?;
  let moved: Vec<MovedCommit> = rewrites
    .into_iter()
    .map(|(id, previous_message, _)| MovedCommit {
      old_id: id.to_string(),
      new_id: new_ids.get(id).cloned().unwrap_or_else(|| id.to_string()),
      previous_message,
    })
    .collect();
  info!(moved = moved.len(), "Moved commits");
  Ok(MoveCommitsResult { moved })
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
