use crate::git::model::{CommitDetail, CommitInfo};
use crate::progress::SyncEvent;
use anyhow::anyhow;
use git2::{Commit, Oid, Repository, Signature, Tree};
use similar::TextDiff;
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
  commit_info: &CommitInfo,
  new_parent_oid: Oid,
  reuse_if_possible: bool,
  repo: &Repository,
  progress: &Channel<SyncEvent>,
  progress_info: &ProgressInfo,
  task_index: i16,
) -> anyhow::Result<(CommitDetail, Oid)> {
  if reuse_if_possible {
    if let Ok(note) = repo.find_note(None, commit_info.id) {
      if let Some(message) = note.message() {
        if let Some(hash) = message.strip_prefix(PREFIX)
          && !hash.is_empty()
        {
          return Ok((
            CommitDetail {
              original_hash: commit_info.id.to_string(),
              hash: hash.to_string(),
              is_new: false,
              time: commit_info.time,
              message: commit_info.message.clone(),
            },
            Oid::from_str(hash)?,
          ));
        }
      }
    }
  }

  let original_commit = &repo.find_commit(commit_info.id)?;

  let new_parent_commit = repo.find_commit(new_parent_oid)?;
  let original_commit_parent = original_commit.parent(0)?;

  // Commits are processed in order (oldest to newest).
  // We can directly compare if the new parent tree is the same as the cherry-picked original parent tree.
  // This helps us identify if the parent relationship is preserved.
  // If the tree IDs match, we can skip the merge and reuse the original tree directly.

  // If the trees match, it means the new parent has exactly the same content as the original parent.
  // In this case, we can apply the original commit directly without merging.
  let new_tree = if original_commit_parent.tree_id() == new_parent_commit.tree_id() {
    progress.send(SyncEvent {
      message: format!(
        "[{}/{}] {}: Creating commit {}/{} ({:.7}) with existing tree",
        progress_info.current_branch_idx + 1,
        progress_info.total_branches,
        progress_info.branch_name,
        progress_info.current_commit_idx + 1,
        progress_info.total_commits_in_branch,
        commit_info.id
      ),
      index: task_index,
    })?;
    // trees are identical, we can skip the merge and just use the original tree
    original_commit.tree()?
  } else {
    progress.send(SyncEvent {
      message: format!(
        "[{}/{}] {}: Creating commit {}/{} ({:.7}) using merge",
        progress_info.current_branch_idx + 1,
        progress_info.total_branches,
        progress_info.branch_name,
        progress_info.current_commit_idx + 1,
        progress_info.total_commits_in_branch,
        commit_info.id
      ),
      index: task_index,
    })?;
    // trees are different, use merge with the cached new_parent_commit
    perform_merge(repo, &new_parent_commit.tree()?, original_commit, original_commit_parent)?
  };
  let committer = Signature::now("branch-deck", original_commit.author().email().unwrap_or_default())?;

  let new_commit_oid = repo.commit(
    None, // don't update any references
    &original_commit.author(),
    &committer,
    &commit_info.message,
    &new_tree,
    &[&new_parent_commit],
  )?;

  // store the mapping in git notes
  // create a new commit with the merged tree
  repo.note(&original_commit.author(), &committer, None, commit_info.id, &format!("v-commit:{new_commit_oid}"), true)?;

  Ok((
    CommitDetail {
      original_hash: commit_info.id.to_string(),
      hash: new_commit_oid.to_string(),
      time: commit_info.time,
      message: commit_info.message.clone(),
      is_new: true,
    },
    new_commit_oid,
  ))
}

/// Use a 3-way merge with explicit merge base to isolate specific commit changes
fn perform_merge<'a>(repo: &'a Repository, base_tree: &Tree, cherry_commit: &Commit, cherry_parent: Commit) -> anyhow::Result<Tree<'a>> {
  // Perform a 3-way merge: merge_base -> base_commit vs merge_base -> cherry_commit
  // This ensures we only apply the specific changes from the cherry commit
  let cherry_tree = cherry_commit.tree()?;
  let merge_base_tree = cherry_parent.tree()?;

  let merge_options = git2::MergeOptions::new();
  let mut index = repo.merge_trees(&merge_base_tree, base_tree, &cherry_tree, Some(&merge_options))?;

  if index.has_conflicts() {
    let conflict_diff = render_conflict_diffs(repo, &index)?;
    return Err(anyhow!("Cherry-pick resulted in conflicts:\n{}", conflict_diff));
  }

  // No conflicts - write the tree and return it
  let tree_id = index.write_tree_to(repo)?;
  Ok(repo.find_tree(tree_id)?)
}

/// Render conflicts as a readable diff using the similar crate
fn render_conflict_diffs(repo: &Repository, index: &git2::Index) -> anyhow::Result<String> {
  let mut conflict_info = Vec::new();

  for conflict in index.conflicts()? {
    let conflict = conflict?;
    let path = conflict
      .their
      .as_ref()
      .or(conflict.our.as_ref())
      .or(conflict.ancestor.as_ref())
      .map_or_else(|| "<unknown>".to_string(), |entry| String::from_utf8_lossy(&entry.path).to_string());

    // Get the different versions of the file
    let our_content = get_blob_content(repo, &conflict.our, "<file deleted in our version>");
    let their_content = get_blob_content(repo, &conflict.their, "<file deleted in their version>");

    // Use similar crate's TextDiff for better diff output
    let diff = TextDiff::from_lines(&our_content, &their_content);
    let diff_content: String = diff
      .unified_diff()
      .context_radius(3) // Add 3 lines of context
      .header(&format!("a/{path}"), &format!("b/{path}"))
      .to_string();

    conflict_info.push(diff_content);
  }

  Ok(conflict_info.join("\n"))
}

/// Get content from a blob entry, with fallback message
fn get_blob_content(repo: &Repository, entry: &Option<git2::IndexEntry>, fallback: &str) -> String {
  if let Some(entry) = entry {
    match repo.find_blob(entry.id) {
      Ok(blob) => String::from_utf8_lossy(blob.content()).to_string(),
      Err(_) => "<could not read version>".to_string(),
    }
  } else {
    fallback.to_string()
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::test_utils::git_test_utils::{create_commit, create_test_repo};

  #[test]
  fn test_perform_merge_displays_conflict_diffs() {
    let (_dir, repo) = create_test_repo();

    // Create initial commit
    let initial_id = create_commit(&repo, "Initial commit", "file.txt", "line1\nline2\nline3\n");
    let initial_commit = repo.find_commit(initial_id).unwrap();

    // Create base commit that modifies the file
    let base_id = create_commit(&repo, "Base changes", "file.txt", "base_line1\nbase_line2\nbase_line3\n");
    let base_commit = repo.find_commit(base_id).unwrap().tree().unwrap();

    // Reset to initial commit and create conflicting changes
    repo.reset(initial_commit.as_object(), git2::ResetType::Hard, None).unwrap();
    let cherry_id = create_commit(&repo, "Cherry changes", "file.txt", "cherry_line1\ncherry_line2\ncherry_line3\n");
    let cherry_commit = repo.find_commit(cherry_id).unwrap();

    // Attempt the merge, which should have conflicts
    let result = perform_merge(&repo, &base_commit, &cherry_commit, cherry_commit.parent(0).unwrap());

    // Test that the error is reported and contains the diff format
    assert!(result.is_err());
    let error = result.unwrap_err();
    let error_message = error.to_string();

    // Check that it reports conflicts
    assert!(error_message.contains("Cherry-pick resulted in conflicts:"));

    // Check that it shows diff format with file headers
    assert!(error_message.contains("--- a/file.txt"));
    assert!(error_message.contains("+++ b/file.txt"));

    // Check that it shows actual diff content with - and + prefixes
    assert!(error_message.contains("-base_line") || error_message.contains("+cherry_line"));

    println!("Conflict diff error message:");
    println!("{error_message}");
  }

  #[test]
  fn test_perform_merge_with_context_lines() {
    let (_dir, repo) = create_test_repo();

    // Create initial commit with more content to show context
    let initial_content = "line1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\n";
    let initial_id = create_commit(&repo, "Initial commit", "file.txt", initial_content);
    let initial_commit = repo.find_commit(initial_id).unwrap();

    // Create base commit that modifies middle lines
    let base_content = "line1\nline2\nline3\nBASE_MODIFIED_4\nBASE_MODIFIED_5\nline6\nline7\nline8\nline9\nline10\n";
    let base_id = create_commit(&repo, "Base changes", "file.txt", base_content);
    let base_commit = repo.find_commit(base_id).unwrap().tree().unwrap();

    // Reset to initial and create conflicting changes to same lines
    repo.reset(initial_commit.as_object(), git2::ResetType::Hard, None).unwrap();
    let cherry_content = "line1\nline2\nline3\nCHERRY_MODIFIED_4\nCHERRY_MODIFIED_5\nline6\nline7\nline8\nline9\nline10\n";
    let cherry_id = create_commit(&repo, "Cherry changes", "file.txt", cherry_content);
    let cherry_commit = repo.find_commit(cherry_id).unwrap();

    // Attempt the merge, which should have conflicts
    let result = perform_merge(&repo, &base_commit, &cherry_commit, cherry_commit.parent(0).unwrap());

    // Test that the error shows context lines
    assert!(result.is_err());
    let error = result.unwrap_err();
    let error_message = error.to_string();

    println!("Conflict diff with context lines:");
    println!("{error_message}");

    // Should show context lines around the conflict
    assert!(error_message.contains(" line3")); // context before
    assert!(error_message.contains(" line6")); // context after
    assert!(error_message.contains("-BASE_MODIFIED_4"));
    assert!(error_message.contains("+CHERRY_MODIFIED_4"));
  }

  #[test]
  fn test_perform_merge_success_no_conflicts() {
    let (_dir, repo) = create_test_repo();

    // Create initial commit
    let initial_id = create_commit(&repo, "Initial commit", "base.txt", "base content\n");
    let initial_commit = repo.find_commit(initial_id).unwrap();

    // Create base commit that adds a file
    let base_id = create_commit(&repo, "Base changes", "base.txt", "base content\nadded by base\n");
    let base_commit = repo.find_commit(base_id).unwrap().tree().unwrap();

    // Reset to initial and create non-conflicting changes (different file)
    repo.reset(initial_commit.as_object(), git2::ResetType::Hard, None).unwrap();
    let cherry_id = create_commit(&repo, "Cherry changes", "cherry.txt", "cherry content\n");
    let cherry_commit = repo.find_commit(cherry_id).unwrap();

    // Attempt the merge, which should succeed
    let result = perform_merge(&repo, &base_commit, &cherry_commit, cherry_commit.parent(0).unwrap());

    // Should succeed without conflicts
    assert!(result.is_ok());
    let tree = result.unwrap();

    // The resulting tree should contain both files
    assert!(tree.get_name("base.txt").is_some());
    assert!(tree.get_name("cherry.txt").is_some());
  }

  #[test]
  fn test_perform_merge_isolates_specific_commit_changes() {
    // This test reproduces the exact scenario from the idea-1 repository:
    // - Commit [392] modifies files A, B, C
    // - Commit [258] is based on [392] and modifies files D, E
    // - When cherry-picking [258], we should only get changes to D, E
    // - NOT the changes to A, B, C from [392]

    let (_dir, repo) = create_test_repo();

    // Create initial state (master branch)
    let initial_id = create_commit(&repo, "Initial commit", "README.md", "# Project\n");
    let _initial_commit = repo.find_commit(initial_id).unwrap();

    // Create commit [392] - modifies Kotlin files (analogous to the workspace model changes)
    let commit_392_id = create_commit(
      &repo,
      "[392] pass workspace model to applyLoadedStorage",
      "ModuleBridgeLoaderService.kt",
      "// workspace model changes\nclass ModuleBridgeLoaderService {\n  // updated implementation\n}\n",
    );
    create_commit(
      &repo,
      "Update more files for 392",
      "DelayedProjectSynchronizer.kt",
      "// delayed synchronizer changes\nclass DelayedProjectSynchronizer {\n  // implementation\n}\n",
    );
    create_commit(
      &repo,
      "Final 392 changes",
      "JpsProjectModelSynchronizer.kt",
      "// jps synchronizer changes\nclass JpsProjectModelSynchronizer {\n  // implementation\n}\n",
    );

    // Create commit [258] based on [392] - modifies Java files (analogous to EditorConfig changes)
    let commit_258_id = create_commit(
      &repo,
      "[258] CPP-45258 EditorConfig Code Style settings are not loaded",
      "LanguageCodeStyleSettingsProvider.java",
      "// EditorConfig code style changes\npublic class LanguageCodeStyleSettingsProvider {\n  // implementation\n}\n",
    );
    create_commit(
      &repo,
      "Complete 258 changes",
      "LanguageCodeStyleSettingsProviderService.java",
      "// service implementation\npublic class LanguageCodeStyleSettingsProviderService {\n  // implementation\n}\n",
    );

    let commit_392 = repo.find_commit(commit_392_id).unwrap();
    let commit_258 = repo.find_commit(commit_258_id).unwrap();

    // Verify the relationship: commit_258 should be descendant of commit_392
    assert!(repo.graph_descendant_of(commit_258.id(), commit_392.id()).unwrap());

    // Create a target branch base (simulating the branch we're cherry-picking onto)
    // This represents the state before any of our changes
    let target_base = repo.find_commit(initial_id).unwrap().tree().unwrap();

    // Perform the merge: cherry-pick commit [258] onto the target base
    // This should ONLY include the Java file changes from [258]
    // NOT the Kotlin file changes from [392]
    let result = perform_merge(&repo, &target_base, &commit_258, commit_258.parent(0).unwrap());

    // Should succeed without conflicts
    assert!(result.is_ok(), "Merge should succeed without conflicts");
    let merged_tree = result.unwrap();

    // Verify the merged tree contains:
    // 1. The original README.md from target_base
    assert!(merged_tree.get_name("README.md").is_some(), "Should contain original README.md");

    // 2. The Java files from commit [258] (the cherry-picked commit)
    assert!(
      merged_tree.get_name("LanguageCodeStyleSettingsProvider.java").is_some(),
      "Should contain Java file from commit [258]"
    );

    // 3. But NOT the Kotlin files from commit [392] (the ancestor)
    assert!(
      merged_tree.get_name("ModuleBridgeLoaderService.kt").is_none(),
      "Should NOT contain Kotlin file from ancestor commit [392]"
    );
    assert!(
      merged_tree.get_name("DelayedProjectSynchronizer.kt").is_none(),
      "Should NOT contain other Kotlin files from ancestor commits"
    );
    assert!(
      merged_tree.get_name("JpsProjectModelSynchronizer.kt").is_none(),
      "Should NOT contain more Kotlin files from ancestor commits"
    );

    // Verify the content of the Java file matches what was in commit [258]
    let java_entry = merged_tree.get_name("LanguageCodeStyleSettingsProvider.java").unwrap();
    let java_blob = repo.find_blob(java_entry.id()).unwrap();
    let java_content = String::from_utf8_lossy(java_blob.content());
    assert!(
      java_content.contains("EditorConfig code style changes"),
      "Java file should contain the specific changes from commit [258]"
    );

    println!("✅ Successfully isolated commit [258] changes:");
    println!("   - Included Java files: LanguageCodeStyleSettingsProvider.java");
    println!("   - Excluded Kotlin files from ancestor [392]: ModuleBridgeLoaderService.kt, etc.");
    println!("   - This proves the fix prevents cross-contamination between logical branches");
  }
}
