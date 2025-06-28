use crate::git::commit_list::Commit;
use crate::git::git_command::GitCommandExecutor;
use crate::git::model::CommitDetail;
use std::time::UNIX_EPOCH;
use tauri::State;

// Create or update a commit based on an original commit
pub(crate) fn create_or_update_commit(
  clean_message_and_original_commit: &(String, Commit),
  parent_hash: &str,
  repository_path: &str,
  reuse_if_possible: bool,
  git: &State<'_, GitCommandExecutor>,
) -> Result<(CommitDetail, String), String> {
  let clean_message = &clean_message_and_original_commit.0;
  let original_commit = &clean_message_and_original_commit.1;

  // check if this commit was already cherry-picked using git notes
  let mut cherry_picked_commit_hash = String::new();
  if reuse_if_possible && !original_commit.notes.is_empty() {
    let prefix = "v-commit:";
    if let Some(idx) = original_commit.notes.find(prefix) {
      let note_end = original_commit.notes[idx..].find('\n').unwrap_or(original_commit.notes[idx..].len());
      cherry_picked_commit_hash = original_commit.notes[idx + prefix.len()..idx + note_end].trim().to_string();
    }
  }

  if !cherry_picked_commit_hash.is_empty() {
    return Ok((
      CommitDetail {
        original_hash: original_commit.hash.clone(),
        hash: cherry_picked_commit_hash.clone(),
        is_new: false,
        time: time_to_js(original_commit)?,
        message: clean_message.to_string(),
      },
      cherry_picked_commit_hash,
    ));
  }

  // use git2 for proper cherry-pick that isolates only the changes from the target commit
  let repo = git2::Repository::open(repository_path).map_err(|e| format!("Failed to open repository: {}", e))?;

  // get the original commit to be cherry-picked
  let original_oid = git2::Oid::from_str(&original_commit.hash).map_err(|e| format!("Invalid commit hash: {}", e))?;
  let original_commit_obj = repo.find_commit(original_oid).map_err(|e| format!("Failed to find original commit: {}", e))?;

  // get the parent of the original commit
  let original_parent_oid = original_commit_obj.parent_id(0).map_err(|e| format!("Original commit has no parent: {}", e))?;
  let original_parent_commit = repo
    .find_commit(original_parent_oid)
    .map_err(|e| format!("Failed to find parent of original commit: {}", e))?;

  // get the new parent commit that we'll cherry-pick onto
  let new_parent_oid = git2::Oid::from_str(parent_hash).map_err(|e| format!("Invalid parent hash: {}", e))?;
  let new_parent_commit = repo.find_commit(new_parent_oid).map_err(|e| format!("Failed to find new parent commit: {}", e))?;

  // perform a 3-way merge to properly cherry-pick
  let original_tree = original_commit_obj.tree().map_err(|e| format!("Failed to get original commit tree: {}", e))?;
  let original_parent_tree = original_parent_commit.tree().map_err(|e| format!("Failed to get original parent tree: {}", e))?;
  let new_parent_tree = new_parent_commit.tree().map_err(|e| format!("Failed to get new parent tree: {}", e))?;

  // Create an in-memory index for the merge
  let mut index = repo
    .merge_trees(&original_parent_tree, &new_parent_tree, &original_tree, None)
    .map_err(|e| format!("Failed to merge trees: {}", e))?;

  // Check for conflicts
  if index.has_conflicts() {
    return Err("Cherry-pick resulted in conflicts that could not be resolved automatically".to_string());
  }

  // Write the merge result as a tree
  let new_tree_oid = index.write_tree_to(&repo).map_err(|e| format!("Failed to write merged tree: {}", e))?;
  let new_tree = repo.find_tree(new_tree_oid).map_err(|e| format!("Failed to find merged tree: {}", e))?;

  // Create a new commit with the merged tree
  let signature = repo.signature().map_err(|e| format!("Failed to get signature: {}", e))?;

  let new_commit_oid = repo
    .commit(
      None, // Don't update any references
      &signature,
      &signature,
      clean_message,
      &new_tree,
      &[&new_parent_commit],
    )
    .map_err(|e| format!("Failed to create commit: {}", e))?;

  let new_hash = new_commit_oid.to_string();

  // store the mapping in git notes
  git.execute_status(&["notes", "add", "-f", "-m", &format!("v-commit:{new_hash}"), &original_commit.hash], repository_path)?;

  Ok((
    CommitDetail {
      original_hash: original_commit.hash.clone(),
      time: time_to_js(original_commit)?,
      hash: new_hash,
      is_new: true,
      message: clean_message.to_string(),
    },
    new_hash,
  ))
}

fn time_to_js(original_commit: &Commit) -> Result<u32, String> {
  Ok(original_commit.date.duration_since(UNIX_EPOCH).map_err(|e| format!("Cannot convert time: {e}"))?.as_secs() as u32)
}
