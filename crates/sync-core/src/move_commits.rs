use crate::commit_grouper::parse_subject;
use crate::move_plan::{move_target, plan_move};
use crate::regroup::{RegroupOptions, RegroupOutcome, RegroupedCommit, UndoToken, regroup_planned};
use anyhow::{Result, ensure};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::model::sanitize_branch_name;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
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

/// How a move changes a commit
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum MoveChange {
  /// Its message got the prefix of the target
  Prefixed,
  /// Its message lost its prefix: its issue key puts it where it goes
  PrefixRemoved,
  /// The issue groups moved it, its message stays
  Regrouped,
  /// It is where it goes already
  Unchanged,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct MovedCommit {
  /// The request commit id
  pub old_id: String,
  /// The id after the rewrite; `old_id` for a commit older than every rewritten one and for a dry run
  pub new_id: String,
  pub change: MoveChange,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct MoveCommitsResult {
  /// The requested commits in request order, then the other commits that moved: the fixups that followed them, the
  /// commits of a renamed branch
  pub commits: Vec<MovedCommit>,
  /// The issue keys whose commits the issue groups now put into another branch, without a rewrite
  pub grouped_issues: Vec<String>,
  /// The issue keys whose issue group was removed: their commits are in the issue branch of the key again
  pub ungrouped_issues: Vec<String>,
  /// The branch the commits are in afterwards (the leaf with stacked branches on); `None` for a move back
  #[cfg_attr(feature = "specta", specta(optional))]
  pub target: Option<String>,
  /// HEAD afterwards
  pub head: String,
  /// With stacked branches on, the branches whose parent the operation emptied: they stay stacked on it and are built on
  /// the nearest ancestor with commits, or the baseline, until it has commits again
  pub orphaned_children: Vec<String>,
  /// What `undo_regroup` moves back; `None` for a dry run and when nothing changed
  #[cfg_attr(feature = "specta", specta(optional))]
  pub undo_token: Option<String>,
}

impl MoveCommitsResult {
  /// Whether the operation moves anything
  pub fn moves(&self) -> bool {
    self.commits.iter().any(|commit| commit.change != MoveChange::Unchanged)
  }
}

/// Moves commits to another branch, or back, with the smallest regroup (`move_plan.rs`): a whole issue by the issue
/// groups, without a rewrite; the other commits by setting or removing the `(branch)` prefix of their subjects. One
/// history rewrite for all commits and one ref transaction with the issue groups; the next sync rebuilds the virtual
/// branches.
#[instrument(skip(git_executor, params), fields(count = params.commit_ids.len(), target = ?params.target_branch, dry_run = params.dry_run))]
pub fn move_commits_to_branch_core(git_executor: &GitCommandExecutor, params: &MoveCommitsParams) -> Result<MoveCommitsResult> {
  let target = params.target_branch.as_deref();
  if let Some(target) = target {
    validate_branch_name(target)?;
  }
  ensure!(!params.commit_ids.is_empty(), "No commits to move");

  let options = RegroupOptions {
    expect_head: params.expect_head.clone(),
    dry_run: params.dry_run,
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
pub(crate) fn move_result(outcome: &RegroupOutcome, requested: &[String], target: Option<String>) -> MoveCommitsResult {
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

#[cfg(test)]
#[path = "move_commits_test.rs"]
mod move_commits_test;
