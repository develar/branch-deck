use crate::git::commit_list::get_commit_list;
use crate::git::copy_commit::create_or_update_commit;
use crate::git::model::{BranchInfo, BranchSyncStatus, CommitDetail, SyncBranchResult, to_final_branch_name};
use crate::progress::SyncEvent;
use git2::Oid;
use indexmap::IndexMap;
use regex::Regex;
use std::collections::HashSet;
use tauri::ipc::Channel;

#[tauri::command]
#[specta::specta]
pub async fn sync_branches<'a>(repository_path: &str, branch_prefix: &str, progress: Channel<SyncEvent<'a>>) -> Result<SyncBranchResult, String> {
  let repo = git2::Repository::open(repository_path).map_err(|e| format!("Failed to open repository: {e}"))?;

  progress
    .send(SyncEvent::Progress { message: "get commits" })
    .map_err(|e| format!("Failed to send progress: {e}"))?;

  let commits = get_commit_list(&repo, "master").map_err(|e| format!("Failed to get_commit_list: {e}"))?;

  let oldest_head_commit = match commits.first() {
    Some(commit) => commit,
    None => return Ok(SyncBranchResult { branches: Vec::new() }),
  };

  let parent_commit_hash = oldest_head_commit.parent(0).map_err(|e| format!("{e}"))?.id();

  let existing_branches = check_existing_branches(&repo, branch_prefix).map_err(|e| format!("Failed to get_commit_list: {e}"))?;

  let mut results: Vec<BranchInfo> = Vec::new();
  // group commits by prefix first to get all branch names
  // for each prefix, create a branch with only the relevant commits
  for (branch_name, branch_commits) in group_commits_by_prefix(&commits) {
    let mut current_parent_hash = parent_commit_hash;
    let mut last_commit_hash: Oid = Oid::zero();
    let mut commit_details: Vec<CommitDetail> = Vec::new();
    let mut is_any_commit_changed = false;
    let mut branch_error: Option<String> = None;

    let full_branch_name = to_final_branch_name(branch_prefix, branch_name.as_str())?;
    let is_existing_branch = existing_branches.contains(full_branch_name.as_str());

    // let mut prev_original_commit: Oid = Oid::zero();

    // recreate each commit on top of the last one
    for (clean_message, original_commit) in branch_commits {
      // If any commit in the branch’s history up to this point has changed, we still need to copy this commit —
      // even if its own content didn’t change — so that its parent reference is updated.
      let reuse_if_possible = is_existing_branch && !is_any_commit_changed;
      // check if we can reuse the tree directly (avoid merge)
      // let reuse_tree_without_merge = prev_original_commit.is_zero() || original_commit.parent_id(0).unwrap() != prev_original_commit;
      match create_or_update_commit(&clean_message, &original_commit, current_parent_hash, reuse_if_possible, &repo, progress.clone()) {
        Ok((detail, new_id)) => {
          if detail.is_new {
            is_any_commit_changed = true;
          }

          current_parent_hash = new_id;
          last_commit_hash = new_id;
          commit_details.push(detail);
        }
        Err(err) => {
          branch_error = Some(format!("Failed to create or update commit: {err:?}"));
          break;
        }
      }

      // prev_original_commit = original_commit.id();
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
      match repo.branch(&full_branch_name, &repo.find_commit(last_commit_hash).map_err(|e| format!("{e}"))?, true) {
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
      commit_count: u32::try_from(commit_details.len()).unwrap_or_default(),
      commit_details,
      error: None,
    });
  }

  // reverse - newest first
  results.reverse();

  progress.send(SyncEvent::Finished {}).map_err(|e| format!("Failed to send progress: {e}"))?;

  Ok(SyncBranchResult { branches: results })
}

// prepare branch refs for all branches at once
fn check_existing_branches(repo: &git2::Repository, branch_prefix: &str) -> anyhow::Result<HashSet<String>> {
  let branches = repo.branches(Some(git2::BranchType::Local))?;

  let mut existing_branches = HashSet::new();
  for branch in branches {
    let (branch, _) = branch?;
    let branch_name = branch.name()?.unwrap_or("");
    if branch_name.starts_with(branch_prefix) {
      existing_branches.insert(branch_name.to_string());
    }
  }

  Ok(existing_branches)
}

fn group_commits_by_prefix<'a>(commits: &'a [git2::Commit<'a>]) -> IndexMap<String, Vec<(String, git2::Commit<'a>)>> {
  // use index map - preserve insertion order
  let mut prefix_to_commits: IndexMap<String, Vec<(String, git2::Commit)>> = IndexMap::new();
  let prefix_pattern = Regex::new(r"\[(.+?)](.*?)(?:\r?\n|$)").unwrap();

  // group commits by prefix
  for commit in commits {
    match commit.message() {
      None => {}
      Some(message) => {
        if let Some(captures) = prefix_pattern.captures(message)
          && let (Some(prefix_match), Some(message_match)) = (captures.get(1), captures.get(2))
        {
          let message = message_match.as_str().trim().to_string();
          prefix_to_commits
            .entry(prefix_match.as_str().trim().to_string())
            .or_default()
            .push((message, commit.clone()));
        }
      }
    }
  }
  prefix_to_commits
}
