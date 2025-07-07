use crate::git::commit_list::get_commit_list;
use crate::git::copy_commit::create_or_update_commit;
use crate::git::model::{BranchInfo, BranchSyncStatus, CommitDetail, CommitInfo, SyncBranchResult, to_final_branch_name};
use crate::progress::SyncEvent;
use git2::Oid;
use indexmap::IndexMap;
use regex::Regex;
use tauri::ipc::Channel;
use tracing::{debug, info, warn, error, instrument};

/// Parameters for processing a single branch
struct BranchProcessingParams {
  repository_path: String,
  branch_prefix: String,
  branch_name: String,
  commit_data: Vec<CommitInfo>,
  parent_commit_hash: Oid,
  current_branch_idx: usize,
  total_branches: usize,
  progress: Channel<SyncEvent>,
}

/// Synchronizes branches by grouping commits by prefix and creating/updating branches
#[tauri::command]
#[specta::specta]
#[instrument(skip(progress))]
pub async fn sync_branches(repository_path: &str, branch_prefix: &str, progress: Channel<SyncEvent>) -> Result<SyncBranchResult, String> {
  info!("Starting branch synchronization for repository: {}, prefix: {}", repository_path, branch_prefix);
  do_sync_branches(repository_path, branch_prefix, progress).await.map_err(|e| {
    error!("Branch synchronization failed: {}", e);
    format!("{e:?}")
  })
}

async fn do_sync_branches(repository_path: &str, branch_prefix: &str, progress: Channel<SyncEvent>) -> anyhow::Result<SyncBranchResult> {
  progress.send(SyncEvent {
    message: "get commits".to_string(),
    index: -1,
  })?;

  // extract all the data we need from git2 types before spawning tasks
  let (grouped_commit_data, parent_commit_hash) = {
    let repo = git2::Repository::open(repository_path)?;
    let commits = get_commit_list(&repo, "master")?;

    let Some(oldest_head_commit) = commits.first() else {
      return Ok(SyncBranchResult { branches: Vec::new() });
    };

    let parent_commit_hash = oldest_head_commit.parent(0)?.id();

    // group commits by prefix first to get all branch names
    // for each prefix, create a branch with only the relevant commits
    let grouped_commits = group_commits_by_prefix(&commits);

    // extract commit IDs and messages to avoid lifetime issues with git2 types
    let grouped_commit_data: Vec<(String, Vec<CommitInfo>)> = grouped_commits
      .into_iter()
      .map(|(branch_name, branch_commits)| {
        let commit_data = branch_commits
          .into_iter()
          .map(|(message, commit)| CommitInfo {
            message,
            id: commit.id(),
            time: commit.author().when().seconds() as u32,
          })
          .collect();
        (branch_name, commit_data)
      })
      .collect();

    (grouped_commit_data, parent_commit_hash)
  }; // repo and commits are dropped here, before we spawn tasks

  let total_branches = grouped_commit_data.len();

  info!("Fetched and grouped commits. Total branches: {}.", total_branches);

  // process branches in parallel
  let repository_path_owned = repository_path.to_string();
  let branch_prefix_owned = branch_prefix.to_string();

  let mut tasks = Vec::new();

  for (current_branch_idx, (branch_name, commit_data)) in grouped_commit_data.into_iter().enumerate() {
    debug!("Processing branch {} of {}: {}", current_branch_idx + 1, total_branches, branch_name);
    let repository_path = repository_path_owned.clone();
    let branch_prefix = branch_prefix_owned.clone();
    let progress_clone = progress.clone();
    let branch_name_for_error = branch_name.clone();

    let params = BranchProcessingParams {
      repository_path,
      branch_prefix,
      branch_name,
      commit_data,
      parent_commit_hash,
      current_branch_idx,
      total_branches,
      progress: progress_clone,
    };

    let task = tauri::async_runtime::spawn(async move { process_single_branch(params).await });

    tasks.push((task, branch_name_for_error));
  }

  // wait for all tasks to complete
  let mut results = Vec::new();
  for (task, branch_name) in tasks {
    match task.await {
      Ok(Ok(branch_info)) => results.push(branch_info),
      Ok(Err(e)) => {
        warn!("Branch {} processing returned an error: {:?}", branch_name, e);
        // Task completed but returned an error
        results.push(BranchInfo {
          name: branch_name,
          sync_status: BranchSyncStatus::Error,
          commit_count: 0,
          commit_details: Vec::new(),
          error: Some(format!("Branch processing failed: {e:?}")),
        });
      }
      Err(e) => {
        error!("Task for branch {} failed to join or panicked: {:?}", branch_name, e);
        // Task panicked or failed to join
        results.push(BranchInfo {
          name: branch_name,
          sync_status: BranchSyncStatus::Error,
          commit_count: 0,
          commit_details: Vec::new(),
          error: Some(format!("Task failed: {e:?}")),
        });
      }
    }
  }

  // reverse - newest first
  results.reverse();

  progress.send(SyncEvent {
    message: "finished".to_string(),
    index: -1,
  })?;

  Ok(SyncBranchResult { branches: results })
}

#[instrument(skip(params), fields(branch_name = %params.branch_name))]
async fn process_single_branch(params: BranchProcessingParams) -> anyhow::Result<BranchInfo> {
  let BranchProcessingParams {
    repository_path,
    branch_prefix,
    branch_name,
    commit_data,
    parent_commit_hash,
    current_branch_idx,
    total_branches,
    progress,
  } = params;

  info!("Starting to process branch: {} ({} commits)", branch_name, commit_data.len());

  let task_index = current_branch_idx as i16;
  let repo = git2::Repository::open(&repository_path)?;
  let full_branch_name = to_final_branch_name(&branch_prefix, &branch_name)?;

  let is_existing_branch = check_branch_exists(&repo, &full_branch_name);
  debug!("Branch {} exists: {}", full_branch_name, is_existing_branch);

  let mut current_parent_hash = parent_commit_hash;
  let mut last_commit_hash: Oid = Oid::zero();
  let mut commit_details: Vec<CommitDetail> = Vec::new();
  let mut is_any_commit_changed = false;

  // recreate each commit on top of the last one
  let total_commits_in_branch = commit_data.len();
  for (current_commit_idx, commit_info) in commit_data.into_iter().enumerate() {
    debug!("Processing commit {}/{} in branch {}: {}", 
           current_commit_idx + 1, total_commits_in_branch, branch_name, commit_info.id);
    // If any commit in the branch's history up to this point has changed, we still need to copy this commit —
    // even if its own content didn't change — so that its parent reference is updated.
    let reuse_if_possible = is_existing_branch && !is_any_commit_changed;
    // check if we can reuse the tree directly (avoid merge)
    let progress_info = crate::git::copy_commit::ProgressInfo {
      branch_name: &branch_name,
      current_commit_idx,
      total_commits_in_branch,
      current_branch_idx,
      total_branches,
    };

    let (detail, new_id) = create_or_update_commit(&commit_info, current_parent_hash, reuse_if_possible, &repo, &progress, &progress_info, task_index)?;

    if detail.is_new {
      is_any_commit_changed = true;
    }

    current_parent_hash = new_id;
    last_commit_hash = new_id;
    commit_details.push(detail);
  }

  let branch_sync_status: BranchSyncStatus;
  if is_existing_branch {
    if is_any_commit_changed {
      branch_sync_status = BranchSyncStatus::Updated;
      info!("Branch {} was updated with new commits", branch_name);
    } else {
      branch_sync_status = BranchSyncStatus::Unchanged;
      debug!("Branch {} is unchanged", branch_name);
    }
  } else {
    branch_sync_status = BranchSyncStatus::Created;
    info!("Branch {} was created", branch_name);
  }

  // only update the branch if it's new or changed
  if branch_sync_status != BranchSyncStatus::Unchanged {
    let _ = progress.send(SyncEvent {
      message: format!("[{}/{}] {}: Setting branch reference", current_branch_idx + 1, total_branches, branch_name),
      index: task_index,
    });

    repo.find_commit(last_commit_hash).and_then(|commit| repo.branch(&full_branch_name, &commit, true))?;
  }
  // reverse - newest first
  commit_details.reverse();

  // send end-of-task progress with empty message
  progress.send(SyncEvent {
    message: String::new(),
    index: task_index,
  })?;

  Ok(BranchInfo {
    name: branch_name,
    sync_status: branch_sync_status,
    commit_count: u32::try_from(commit_details.len())?,
    commit_details,
    error: None,
  })
}

pub(crate) fn check_branch_exists(repo: &git2::Repository, branch_name: &str) -> bool {
  repo.find_branch(branch_name, git2::BranchType::Local).is_ok()
}

#[instrument(skip(commits))]
pub(crate) fn group_commits_by_prefix<'repo>(commits: &[git2::Commit<'repo>]) -> IndexMap<String, Vec<(String, git2::Commit<'repo>)>> {
  debug!("Grouping {} commits by prefix", commits.len());
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
  info!("Grouped commits into {} branches", prefix_to_commits.len());
  for (prefix, commits) in &prefix_to_commits {
    debug!("Branch '{}' has {} commits", prefix, commits.len());
  }
  prefix_to_commits
}
