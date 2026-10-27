//! The app's move of commits to another branch or out of one, as the smallest regroup (`move_plan.rs`): the issue
//! groups for a whole issue, the `(prefix)` of the subjects for the rest, one rewrite and one ref transaction for all.

use crate::move_plan::{move_target, plan_move};
use crate::regroup::{RegroupOptions, RegroupOutcome, RegroupedCommit, UndoToken, regroup_planned};
use crate::repository_lock::LockWait;
use anyhow::{Result, ensure};
use domain_types::branch_names::sanitize_branch_name;
use domain_types::rewrite::{MoveChange, MoveCommitsResult, MovedCommit};
use git_executor::git_command_executor::GitCommandExecutor;
use serde::Deserialize;
use std::collections::BTreeSet;
use sync_utils::commit_subject::parse_subject;
use tracing::{info, instrument};

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct MoveCommitsParams {
  pub repository_path: String,
  /// Commits on the current branch (the original commits of virtual branch commits)
  pub commit_ids: Vec<String>,
  /// The branch to move the commits to, existing or new; `None` moves them back: out of their prefix branch, or out
  /// of the issue group of their key
  pub target_branch: Option<String>,
  /// Compute what the move would do, write nothing (the preview of the "Move to…" form)
  #[serde(default)]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub dry_run: bool,
  /// The HEAD the request was made for; another HEAD refuses the move
  #[serde(default)]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub expect_head: Option<String>,
}

impl MoveCommitsParams {
  /// A move that writes, without an expected HEAD
  pub fn new(repository_path: &str, commit_ids: Vec<String>, target_branch: Option<&str>) -> Self {
    Self {
      repository_path: repository_path.to_string(),
      commit_ids,
      target_branch: target_branch.map(str::to_string),
      dry_run: false,
      expect_head: None,
    }
  }
}

/// Moves commits to another branch, or back, with the smallest regroup (`move_plan.rs`): a whole issue by the issue
/// groups, without a rewrite; the other commits by setting or removing the `(branch)` prefix of their subjects. One
/// history rewrite for all commits and one ref transaction with the issue groups; the next sync rebuilds the virtual
/// branches. The lock is waited for as `lock_wait` says
#[instrument(skip(git_executor, params), fields(count = params.commit_ids.len(), target = ?params.target_branch, dry_run = params.dry_run))]
pub fn move_commits_to_branch_core(git_executor: &GitCommandExecutor, params: &MoveCommitsParams, lock_wait: LockWait) -> Result<MoveCommitsResult> {
  let target = params.target_branch.as_deref();
  if let Some(target) = target {
    validate_branch_name(target)?;
  }
  ensure!(!params.commit_ids.is_empty(), "No commits to move");

  let options = RegroupOptions {
    expect_head: params.expect_head.clone(),
    dry_run: params.dry_run,
    lock_wait,
    ..RegroupOptions::default()
  };
  let mut leaf = None;
  let outcome = regroup_planned(git_executor, &params.repository_path, &options, |snapshot| {
    leaf = target.map(|target| move_target(snapshot, target).leaf);
    plan_move(snapshot, &params.commit_ids, target)
  })?;
  let result = move_result(&outcome, &params.commit_ids, leaf);
  info!(commits = result.commits.len(), grouped = ?result.grouped_issues, ungrouped = ?result.ungrouped_issues, "Moved commits");
  Ok(result)
}

/// The result of a regroup as a move: the `requested` commits (full or abbreviated ids) in request order, then every
/// other commit whose message or branch changed
pub fn move_result(outcome: &RegroupOutcome, requested: &[String], target: Option<String>) -> MoveCommitsResult {
  let moved_commit = |id: &str, commit: &RegroupedCommit| MovedCommit {
    old_id: id.to_string(),
    new_id: commit.new_id.clone(),
    change: if commit.changed {
      if parse_subject(&commit.message).prefix.is_some() {
        MoveChange::Prefixed
      } else {
        MoveChange::PrefixRemoved
      }
    } else if commit.group != commit.previous_group {
      MoveChange::Regrouped
    } else {
      MoveChange::Unchanged
    },
  };
  let mut listed = BTreeSet::new();
  let mut commits: Vec<MovedCommit> = requested
    .iter()
    .filter_map(|id| {
      let commit = outcome.commits.iter().find(|commit| commit.id.starts_with(id.as_str()))?;
      listed.insert(commit.id.as_str()).then(|| moved_commit(id, commit))
    })
    .collect();
  commits.extend(
    outcome
      .commits
      .iter()
      .filter(|commit| !listed.contains(commit.id.as_str()) && (commit.changed || commit.group != commit.previous_group))
      .map(|commit| moved_commit(&commit.id, commit)),
  );
  let (mut grouped_issues, mut ungrouped_issues) = (Vec::new(), Vec::new());
  if let Some(change) = &outcome.issue_groups {
    grouped_issues.extend(
      change
        .entries
        .iter()
        .filter(|(key, name)| change.previous_entries.get(*key) != Some(*name))
        .map(|(key, _)| key.clone()),
    );
    ungrouped_issues.extend(change.previous_entries.keys().filter(|key| !change.entries.contains_key(*key)).cloned());
  }
  MoveCommitsResult {
    commits,
    grouped_issues,
    ungrouped_issues,
    target,
    head: outcome.new_head.clone(),
    orphaned_children: outcome.orphaned_children.clone(),
    undo_token: UndoToken::of(outcome).map(|token| token.to_string()),
  }
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
