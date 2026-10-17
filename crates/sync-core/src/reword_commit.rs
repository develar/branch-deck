use crate::baseline::detect_baseline_branch;
use crate::commit_grouper::stripped_subject;
use crate::repository_lock::RewriteOptions;
use anyhow::{Result, anyhow};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::commit_list::Commit;
use git_ops::commit_utils::{find_commit, get_commits_info};
use git_ops::commit_writer::{StoreUpdate, write_store_updates};
use git_ops::reword_commits::{RewordCommitParams, reword_commits_batch_at};
use git_ops::rewrite_guard::get_current_branch;
use git_ops::rewrite_plan::short_id;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{info, instrument};

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct RewordCommitsRequest {
  pub repository_path: String,
  pub rewrites: Vec<CommitRewrite>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct CommitRewrite {
  /// The commit on the current branch (the original commit of a virtual branch commit)
  pub commit_id: String,
  /// The whole new message, the grouping prefix `(branch)` included
  pub message: String,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct RewordCommitsResult {
  /// Request commit id -> new commit id; a commit that was not recreated maps to its own (full) id
  pub new_commit_ids: HashMap<String, String>,
}

/// Replaces the messages of commits on the current branch and recreates the commits after the oldest one (same trees,
/// so no conflicts and no working tree changes) in one pass, under the repository lock; maps each request id to the new
/// id, an unchanged commit to itself. Messages are cleaned up like `git commit` does; every commit must still be on the
/// current branch. The next sync rebuilds the virtual branches from the new commits.
#[instrument(skip(git_executor, request, options), fields(count = request.rewrites.len()))]
pub fn reword_commits_core(git_executor: &GitCommandExecutor, request: &RewordCommitsRequest, options: &RewriteOptions) -> Result<RewordCommitsResult> {
  let repository_path = &request.repository_path;
  let _lock = options.lock(git_executor, repository_path)?;
  let expected_head = options.expected_full_head(git_executor, repository_path)?;
  let ids: Vec<&str> = request.rewrites.iter().map(|r| r.commit_id.as_str()).collect();
  let commits = get_commits_info(git_executor, repository_path, &ids)?;
  let rewrites = request.rewrites.iter().map(|r| (r.commit_id.as_str(), r.message.as_str()));
  let reworded = reword_read_commits_at(git_executor, repository_path, &commits, rewrites, expected_head.as_deref(), &[])?;
  Ok(RewordCommitsResult {
    new_commit_ids: reworded.requested,
  })
}

/// What `reword_read_commits_at` wrote
pub(crate) struct Reworded {
  /// Request id -> new id, an unchanged commit mapped to itself
  pub requested: HashMap<String, String>,
  /// Old -> new full id of every recreated commit, the ones after the oldest reworded one included
  pub recreated: HashMap<String, String>,
  /// The new commits of the stores, in input order
  pub store_commits: Vec<String>,
}

/// Rewords `(commit id, whole new message)` pairs of commits the caller read with `get_commits_info`, for messages
/// computed from the history at `expected_head` (`reword_commits_batch_at`), with `stores` written in the same run and
/// moved in the transaction that moves the branch. The caller holds the repository lock
pub(crate) fn reword_read_commits_at<'a>(
  git_executor: &GitCommandExecutor,
  repository_path: &str,
  commits: &HashMap<String, Commit>,
  rewrites: impl IntoIterator<Item = (&'a str, &'a str)>,
  expected_head: Option<&str>,
  stores: &[StoreUpdate<'_>],
) -> Result<Reworded> {
  let rewrites: Vec<(&str, String)> = rewrites.into_iter().map(|(id, message)| (id, normalize_message(message))).collect();
  if rewrites.iter().any(|(_, message)| message.lines().next().is_none_or(|subject| subject.trim().is_empty())) {
    return Err(anyhow!("The commit message must start with a subject line"));
  }

  // (request id, full id)
  let mut resolved = Vec::with_capacity(rewrites.len());
  let mut params = Vec::new();
  for (id, message) in rewrites {
    // keyed by the full hash; the request may use a short one
    let commit = find_commit(commits, id).ok_or_else(|| anyhow!("Commit {} not found", short_id(id)))?;
    if normalize_message(&commit.message) != message {
      params.push(RewordCommitParams {
        commit_id: commit.id.clone(),
        keeps_virtual_commit: keeps_virtual_commit(&commit.message, &message),
        new_message: message,
      });
    }
    resolved.push((id, commit.id.clone()));
  }

  let reworded_count = params.len();
  // also maps the commits recreated on top of a reworded one, unchanged messages included;
  // `reword_commits_batch` refuses a commit that is no longer on the current branch
  let (mapping, store_commits) = if params.is_empty() {
    // no message changes once cleaned up: the stores still move, on the same HEAD
    let store_commits = if stores.is_empty() {
      Vec::new()
    } else {
      let branch_ref = format!("refs/heads/{}", get_current_branch(git_executor, repository_path)?);
      let verify = expected_head.map(|head| (branch_ref.as_str(), head));
      write_store_updates(git_executor, repository_path, stores, verify, "reword")?
    };
    (HashMap::new(), store_commits)
  } else {
    let baseline_branch = detect_baseline_branch(git_executor, repository_path, "master")?;
    reword_commits_batch_at(git_executor, repository_path, &baseline_branch, params, expected_head, stores)?
  };
  info!(reworded_count, "Reworded commits");
  let requested = resolved
    .into_iter()
    .map(|(id, full_id)| {
      let new_id = mapping.get(&full_id).cloned().unwrap_or(full_id);
      (id.to_string(), new_id)
    })
    .collect();
  Ok(Reworded {
    requested,
    recreated: mapping,
    store_commits,
  })
}

/// Whether the virtual branch commit of a commit stays the same when its message changes from `old` to `new`: the
/// subject without its grouping prefix and the body are the same, so a move to another branch keeps it
pub(crate) fn keeps_virtual_commit(old: &str, new: &str) -> bool {
  fn virtual_message(message: &str) -> (&str, &str) {
    let message = message.trim();
    let (subject, body) = message.split_once('\n').unwrap_or((message, ""));
    (stripped_subject(subject), body)
  }
  virtual_message(old) == virtual_message(new)
}

/// LF line endings, no trailing whitespace on a line, no blank lines at the start or the end (as `git commit` cleans up)
pub(crate) fn normalize_message(message: &str) -> String {
  let lines: Vec<&str> = message.lines().map(str::trim_end).collect();
  lines.join("\n").trim_matches('\n').to_string()
}

#[cfg(test)]
#[path = "reword_commit_test.rs"]
mod reword_commit_test;
