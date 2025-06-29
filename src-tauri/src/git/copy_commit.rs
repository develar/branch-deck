use crate::git::model::CommitDetail;
use crate::progress::SyncEvent;
use anyhow::anyhow;
use git2::{Oid, Repository, Signature, Tree};
use tauri::ipc::Channel;

const PREFIX: &str = "v-commit:";

// Progress information for logging and user feedback
pub struct ProgressInfo<'a> {
  pub branch_name: &'a str,
  pub current_commit_idx: usize,
  pub total_commits_in_branch: usize,
  pub current_branch_idx: usize,
  pub total_branches: usize,
}

// Create or update a commit based on an original commit
pub(crate) fn create_or_update_commit(
  clean_message: &str,
  original_commit: &git2::Commit,
  new_parent_oid: Oid,
  reuse_if_possible: bool,
  repo: &Repository,
  progress: &Channel<SyncEvent>,
  progress_info: &ProgressInfo,
) -> anyhow::Result<(CommitDetail, Oid)> {
  if reuse_if_possible {
    if let Ok(note) = repo.find_note(None, original_commit.id()) {
      if let Some(message) = note.message() {
        if let Some(hash) = message.strip_prefix(PREFIX) {
          if !hash.is_empty() {
            return Ok((
              CommitDetail {
                original_hash: original_commit.id().to_string(),
                hash: hash.to_string(),
                is_new: false,
                time: time_to_js(original_commit),
                message: clean_message.to_owned(),
              },
              Oid::from_str(hash)?,
            ));
          }
        }
      }
    }
  }

  // get the parent of the original commit
  let original_parent_commit = original_commit.parent(0)?;

  // get the new parent commit that we'll cherry-pick onto
  let new_parent_commit = repo.find_commit(new_parent_oid)?;

  // Commits are processed in order (oldest to newest).
  // We can directly compare if the new parent commit is the same as the cherry-picked original parent.
  // This helps us identify if the parent relationship is preserved.
  // If the parent hash matches the hash of the previously cherry-picked commit, we can skip the merge.

  // get the original tree (we'll need this in either case)
  let original_tree = original_commit.tree()?;

  // check if we can reuse the tree directly (avoid merge)
  let original_parent_tree_id = original_parent_commit.tree_id();
  let new_parent_tree_id = new_parent_commit.tree_id();

  // If the trees match, it means the new parent has exactly the same content as the original parent.
  // In this case, we can apply the original commit directly without merging.
  let new_tree = if original_parent_tree_id == new_parent_tree_id {
    progress.send(SyncEvent {
      message: &format!(
        "[{}/{}] {}: Creating commit {}/{} ({:.7}) with existing tree",
        progress_info.current_branch_idx + 1,
        progress_info.total_branches,
        progress_info.branch_name,
        progress_info.current_commit_idx + 1,
        progress_info.total_commits_in_branch,
        original_commit.id()
      ),
    })?;
    // trees are identical, we can skip the merge and just use the original tree
    original_tree
  } else {
    progress.send(SyncEvent {
      message: &format!(
        "[{}/{}] {}: Creating commit {}/{} ({:.7}) using git2 merge",
        progress_info.current_branch_idx + 1,
        progress_info.total_branches,
        progress_info.branch_name,
        progress_info.current_commit_idx + 1,
        progress_info.total_commits_in_branch,
        original_commit.id()
      ),
    })?;
    // trees are different, use git2's built-in merge_commits functionality
    perform_git2_merge(repo, &new_parent_commit, original_commit)?
  };

  let committer = Signature::now("branch-deck", original_commit.author().email().unwrap_or_default())?;

  let new_commit_oid = repo.commit(
    None, // Don't update any references
    &original_commit.author(),
    &committer,
    clean_message,
    &new_tree,
    &[&new_parent_commit],
  )?;

  // store the mapping in git notes
  // create a new commit with the merged tree
  repo.note(
    &original_commit.author(),
    &committer,
    None,
    original_commit.id(),
    &format!("v-commit:{new_commit_oid}"),
    true,
  )?;

  Ok((
    CommitDetail {
      original_hash: original_commit.id().to_string(),
      hash: new_commit_oid.to_string(),
      time: time_to_js(original_commit),
      message: clean_message.to_owned(),
      is_new: true,
    },
    new_commit_oid,
  ))
}

/// Use git2's built-in `merge_commits` function - this is the proper way!
fn perform_git2_merge<'a>(repo: &'a Repository, base_commit: &git2::Commit, cherry_commit: &git2::Commit) -> anyhow::Result<Tree<'a>> {
  // git2's merge_commits does exactly what we want: a 3-way merge without touching working dir
  let merge_options = git2::MergeOptions::new();
  
  // Let git2 handle the merge - this is much more robust than our custom logic
  let mut index = repo.merge_commits(base_commit, cherry_commit, Some(&merge_options))?;
  
  if index.has_conflicts() {
    // Build a simple error message showing the conflicts
    let mut conflict_files = Vec::new();
    for conflict_result in index.conflicts()? {
      let conflict = conflict_result?;
      let path = conflict.their
        .as_ref()
        .or(conflict.our.as_ref())
        .or(conflict.ancestor.as_ref()).map_or_else(|| "<unknown>".to_string(), |entry| String::from_utf8_lossy(&entry.path).to_string());
      conflict_files.push(path);
    }
    
    return Err(anyhow!(
      "Cherry-pick resulted in conflicts in files: {}", 
      conflict_files.join(", ")
    ));
  }
  
  // No conflicts - write the tree and return it
  let tree_id = index.write_tree_to(repo)?;
  Ok(repo.find_tree(tree_id)?)
}


#[allow(clippy::cast_possible_truncation)]
fn time_to_js(original_commit: &git2::Commit) -> u32 {
  original_commit.author().when().seconds() as u32
}
