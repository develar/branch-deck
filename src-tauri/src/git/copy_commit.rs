use crate::git::model::CommitDetail;
use crate::progress::SyncEvent;
use anyhow::anyhow;
use git2::{Oid, Repository, Signature, Tree};
use std::fmt::Write;
use tauri::ipc::Channel;

const PREFIX: &str = "v-commit:";

// Create or update a commit based on an original commit
#[allow(clippy::too_many_lines)]
pub(crate) fn create_or_update_commit(
  clean_message: &str,
  original_commit: &git2::Commit,
  new_parent_oid: Oid,
  reuse_if_possible: bool,
  // reuse_tree_without_merge: bool,
  repo: &Repository,
  progress: &Channel<SyncEvent<'_>>,
  branch_name: &str,
  current_commit_idx: usize,
  total_commits_in_branch: usize,
  current_branch_idx: usize,
  total_branches: usize,
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
        current_branch_idx + 1, 
        total_branches, 
        branch_name, 
        current_commit_idx + 1, 
        total_commits_in_branch, 
        original_commit.id()
      ),
    })?;
    // trees are identical, we can skip the merge and just use the original tree
    original_tree
  } else {
    progress.send(SyncEvent {
      message: &format!(
        "[{}/{}] {}: Creating commit {}/{} ({:.7}) using 3-way merge", 
        current_branch_idx + 1, 
        total_branches, 
        branch_name, 
        current_commit_idx + 1, 
        total_commits_in_branch, 
        original_commit.id()
      ),
    })?;
    // trees are different, we need to perform a 3-way merge
    perform_three_merge(repo, &original_parent_commit, &new_parent_commit, &original_tree)?
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

fn perform_three_merge<'a>(repo: &'a Repository, original_parent_commit: &git2::Commit, new_parent_commit: &git2::Commit, original_tree: &Tree) -> anyhow::Result<Tree<'a>> {
  let original_parent_tree = original_parent_commit.tree()?;
  let new_parent_tree = new_parent_commit.tree()?;

  // create an in-memory index for the merge
  let mut index = repo.merge_trees(&original_parent_tree, &new_parent_tree, original_tree, None)?;

  // check for conflicts
  if index.has_conflicts() {
    let mut conflict_details = String::new();
    let mut conflicts = index.conflicts()?;
    while let Some(Ok(conflict)) = conflicts.next() {
      let ancestor = conflict.ancestor.map_or_else(|| "none".to_string(), |c| String::from_utf8_lossy(&c.path).to_string());
      let ours = conflict.our.map_or_else(|| "none".to_string(), |c| String::from_utf8_lossy(&c.path).to_string());
      let theirs = conflict.their.map_or_else(|| "none".to_string(), |c| String::from_utf8_lossy(&c.path).to_string());
      let _ = writeln!(conflict_details, "  ancestor: {ancestor}, ours: {ours}, theirs: {theirs}");
    }
    return Err(anyhow!("Cherry-pick resulted in conflicts that could not be resolved automatically:\n{conflict_details}"));
  }

  // write the merge result as a tree
  let oid = index.write_tree_to(repo)?;
  Ok(repo.find_tree(oid)?)
}

#[allow(clippy::cast_possible_truncation)]
fn time_to_js(original_commit: &git2::Commit) -> u32 {
  original_commit.author().when().seconds() as u32
}
