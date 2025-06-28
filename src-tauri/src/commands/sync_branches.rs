use indexmap::IndexMap;
use regex::Regex;
use std::collections::HashSet;
use tauri::State;

use crate::git::commit_list::{Commit, get_commit_list};
use crate::git::copy_commit::create_or_update_commit;
use crate::git::git_command::GitCommandExecutor;
use crate::git::model::{BranchInfo, BranchSyncStatus, CommitDetail, SyncBranchResult, to_final_branch_name};

#[tauri::command]
#[specta::specta]
pub async fn sync_branches(git: State<'_, GitCommandExecutor>, repository_path: &str, branch_prefix: &str) -> Result<SyncBranchResult, String> {
  let commits = get_commit_list(repository_path, "master", &git)?;

  let oldest_head_commit_hash = match commits.first() {
    Some(commit) => commit.hash.clone(),
    None => return Ok(SyncBranchResult { branches: Vec::new() }),
  };

  let parent_commit_hash = git.rev_parse(&format!("{oldest_head_commit_hash}^"), repository_path)?;
  let existing_branches = check_existing_branches(&git, repository_path, branch_prefix)?;

  let mut results: Vec<BranchInfo> = Vec::new();
  // group commits by prefix first to get all branch names
  // for each prefix, create a branch with only the relevant commits
  for (branch_name, branch_commits) in group_commits_by_prefix(&commits) {
    let mut current_parent_hash = parent_commit_hash.clone();
    let mut last_commit_hash = String::new();
    let mut commit_details: Vec<CommitDetail> = Vec::new();
    let mut is_any_commit_changed = false;
    let mut branch_error: Option<String> = None;

    let full_branch_name = to_final_branch_name(branch_prefix, branch_name.as_str())?;
    let is_existing_branch = existing_branches.contains(full_branch_name.as_str());

    // recreate each commit on top of the last one
    for original_commit in branch_commits {
      // If any commit in the branch’s history up to this point has changed, we still need to copy this commit —
      // even if its own content didn’t change — so that its parent reference is updated.
      let reuse_if_possible = is_existing_branch && !is_any_commit_changed;
      match create_or_update_commit(&original_commit, &current_parent_hash, repository_path, reuse_if_possible, &git) {
        Ok((detail, new_hash)) => {
          if detail.is_new {
            is_any_commit_changed = true;
          }

          commit_details.push(detail);
          current_parent_hash = new_hash.clone();
          last_commit_hash = new_hash;
        }
        Err(err) => {
          branch_error = Some(err);
          break;
        }
      }
    }

    // If we had an error processing commits, add an error result and continue
    if let Some(err) = branch_error {
      results.push(BranchInfo {
        name: branch_name,
        sync_status: BranchSyncStatus::Error,
        commit_count: 0,
        commit_details: Vec::new(),
        error: Some(err),
      });
      continue;
    }

    let branch_sync_status: BranchSyncStatus;
    if is_existing_branch {
      if is_any_commit_changed {
        branch_sync_status = BranchSyncStatus::Updated;
      } else {
        branch_sync_status = BranchSyncStatus::Unchanged;
      }
    } else {
      branch_sync_status = BranchSyncStatus::Created;
    }

    // only update the branch if it's new or changed
    if branch_sync_status != BranchSyncStatus::Unchanged {
      match git.update_ref(format!("refs/heads/{full_branch_name}").as_str(), &last_commit_hash, repository_path) {
        Ok(_) => {}
        Err(e) => {
          results.push(BranchInfo {
            name: branch_name,
            sync_status: BranchSyncStatus::Error,
            commit_count: 0,
            commit_details: Vec::new(),
            error: Some(format!("Failed to set branch reference: {e}")),
          });
          continue;
        }
      }
    }

    // reverse - newest first
    commit_details.reverse();

    results.push(BranchInfo {
      name: branch_name,
      sync_status: branch_sync_status,
      commit_count: commit_details.len() as u32,
      commit_details,
      error: None,
    });
  }

  // reverse - newest first
  results.reverse();

  Ok(SyncBranchResult { branches: results })
}

// prepare branch refs for all branches at once
fn check_existing_branches(
  git: &State<GitCommandExecutor>,
  repository_path: &str,
  branch_prefix: &str,
) -> Result<HashSet<String>, String> {
  let git_info = &git.get_info()?;
  let mut cmd = git.new_command(git_info, repository_path)?;
  cmd.arg("branch");
  cmd.arg("--list");
  cmd.arg(format!("{}/virtual/*", branch_prefix.replace("*", "\\*")));

  let output = git.execute(&mut cmd, git_info)?;
  let existing_branches: HashSet<String> = output
    .lines()
    .filter_map(|line| {
      // remove leading whitespace and asterisk (if present)
      let branch_name = line.trim_start_matches([' ', '*']).trim();
      if branch_name.is_empty() {
        None
      } else {
        Some(branch_name.to_string())
      }
    })
    .collect();

  Ok(existing_branches)
}

fn group_commits_by_prefix(commits: &[Commit]) -> IndexMap<String, Vec<(String, Commit)>> {
  // use index map - preserve insertion order
  let mut prefix_to_commits: IndexMap<String, Vec<(String, Commit)>> = IndexMap::new();
  let prefix_pattern = Regex::new(r"\[(.+?)](.*?)(?:\r?\n|$)").unwrap();

  // group commits by prefix
  for commit in commits {
    if let Some(captures) = prefix_pattern.captures(&commit.message)
      && let (Some(prefix_match), Some(message_match)) = (captures.get(1), captures.get(2))
    {
      let message = message_match.as_str().trim().to_string();
      prefix_to_commits
        .entry(prefix_match.as_str().trim().to_string())
        .or_default()
        .push((message, commit.clone()));
    }
  }
  prefix_to_commits
}
