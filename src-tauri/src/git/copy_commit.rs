use crate::git::conflict_formatter::format_conflicts_for_user;
use crate::git::fast_cherry_pick::{CherryPickFastOptions, FastCherryPickError, cherry_pick_fast};
use crate::git::model::{CommitDetail, CommitInfo};
use crate::progress::SyncEvent;
use anyhow::anyhow;
use git2::{Commit, Oid, Repository, Signature, Tree};
use tauri::ipc::Channel;
use tracing::instrument;

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
    // trees are different, use fast cherry-pick for better performance
    perform_fast_cherry_pick(repo, original_commit, &new_parent_commit)?
  };

  let author = original_commit.author();
  let committer = Signature::now("branch-deck", author.email().unwrap_or_default())?;

  let new_commit_oid = repo.commit(
    None, // don't update any references
    &author,
    &committer,
    &commit_info.message,
    &new_tree,
    &[&new_parent_commit],
  )?;

  // store the mapping in git notes
  // create a new commit with the merged tree
  repo.note(&author, &committer, None, commit_info.id, &format!("v-commit:{new_commit_oid}"), true)?;

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

/// Use fast cherry-pick for better performance on large repositories
#[instrument(skip_all)]
fn perform_fast_cherry_pick<'a>(repo: &'a Repository, cherry_commit: &'a Commit, target_commit: &'a Commit) -> anyhow::Result<Tree<'a>> {
  let options = CherryPickFastOptions {
    reuse_parent_tree_if_possible: true,
  };

  match cherry_pick_fast(repo, cherry_commit, target_commit, &options) {
    Ok(tree) => Ok(tree),
    Err(FastCherryPickError::MergeConflict { detailed_conflicts, .. }) => {
      // Provide detailed conflict information to the user
      Err(anyhow!(
        "Cherry-pick resulted in conflicts that could not be resolved automatically:\n\n{}",
        format_conflicts_for_user(&detailed_conflicts)
      ))
    }
    Err(other_err) => Err(anyhow!("Cherry-pick failed: {}", other_err)),
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::test_utils::git_test_utils::{create_commit, create_test_repo};

  #[test]
  fn test_perform_merge_displays_conflict_diffs() {
    let (_dir, repo) = create_test_repo();

    // Create initial commit with realistic code content
    let initial_content = "function calculateTotal(items) {\n  let total = 0;\n  for (let item of items) {\n    total += item.price;\n  }\n  return total;\n}";
    let initial_id = create_commit(&repo, "Initial commit", "calculator.js", initial_content);
    let initial_commit = repo.find_commit(initial_id).unwrap();

    // Create base commit that modifies the function (target branch version)
    let base_content = "function calculateTotal(items) {\n  let total = 0;\n  for (let item of items) {\n    total += item.price * item.quantity; // Added quantity\n  }\n  return Math.round(total * 100) / 100; // Round to 2 decimals\n}";
    let base_id = create_commit(&repo, "Target: Add quantity and rounding", "calculator.js", base_content);
    let _base_commit = repo.find_commit(base_id).unwrap().tree().unwrap();

    // Reset to initial commit and create conflicting changes (cherry-pick version)
    repo.reset(initial_commit.as_object(), git2::ResetType::Hard, None).unwrap();
    let cherry_content = "function calculateTotal(items) {\n  let total = 0;\n  for (let item of items) {\n    total += item.price + item.tax; // Added tax calculation\n  }\n  return total.toFixed(2); // Format to 2 decimals\n}";
    let cherry_id = create_commit(&repo, "Cherry-pick: Add tax calculation", "calculator.js", cherry_content);
    let cherry_commit = repo.find_commit(cherry_id).unwrap();

    // Attempt the fast cherry-pick, which should have conflicts
    let target_commit = repo.find_commit(base_id).unwrap();
    let result = perform_fast_cherry_pick(&repo, &cherry_commit, &target_commit);

    // Test that the error is reported with enhanced formatting
    assert!(result.is_err());
    let error = result.unwrap_err();
    let error_message = error.to_string();

    println!("\n{}", "=".repeat(70));
    println!("🚀 ENHANCED CONFLICT RENDERING DEMO");
    println!("{}", "=".repeat(70));
    println!("{error_message}");
    println!("{}", "=".repeat(70));

    // Verify enhanced conflict rendering features
    assert!(error_message.contains("🔥 **MERGE CONFLICTS DETECTED** 🔥"));
    assert!(error_message.contains("📄 **calculator.js**"));
    assert!(error_message.contains("Conflict Preview:"));
    assert!(error_message.contains("🔴 -")); // Shows deletions with red circle
    assert!(error_message.contains("🔵 +")); // Shows additions with blue circle
    assert!(error_message.contains("⚠️  **What to do next:**"));
    assert!(error_message.contains("Review the conflicting files manually"));

    // Check that it mentions the conflicting file
    assert!(error_message.contains("calculator.js"));
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
    let _base_commit = repo.find_commit(base_id).unwrap().tree().unwrap();

    // Reset to initial and create conflicting changes to same lines
    repo.reset(initial_commit.as_object(), git2::ResetType::Hard, None).unwrap();
    let cherry_content = "line1\nline2\nline3\nCHERRY_MODIFIED_4\nCHERRY_MODIFIED_5\nline6\nline7\nline8\nline9\nline10\n";
    let cherry_id = create_commit(&repo, "Cherry changes", "file.txt", cherry_content);
    let cherry_commit = repo.find_commit(cherry_id).unwrap();

    // Attempt the fast cherry-pick, which should have conflicts
    let target_commit = repo.find_commit(base_id).unwrap();
    let result = perform_fast_cherry_pick(&repo, &cherry_commit, &target_commit);

    // Test that the error shows context lines
    assert!(result.is_err());
    let error = result.unwrap_err();
    let error_message = error.to_string();

    println!("Conflict diff with context lines:");
    println!("{error_message}");

    // Should mention the conflicting file
    assert!(error_message.contains("file.txt"));
  }

  #[test]
  fn test_perform_merge_success_no_conflicts() {
    let (_dir, repo) = create_test_repo();

    // Create initial commit
    let initial_id = create_commit(&repo, "Initial commit", "base.txt", "base content\n");
    let initial_commit = repo.find_commit(initial_id).unwrap();

    // Create base commit that adds a file
    let base_id = create_commit(&repo, "Base changes", "base.txt", "base content\nadded by base\n");
    let _base_commit = repo.find_commit(base_id).unwrap().tree().unwrap();

    // Reset to initial and create non-conflicting changes (different file)
    repo.reset(initial_commit.as_object(), git2::ResetType::Hard, None).unwrap();
    let cherry_id = create_commit(&repo, "Cherry changes", "cherry.txt", "cherry content\n");
    let cherry_commit = repo.find_commit(cherry_id).unwrap();

    // Attempt the fast cherry-pick, which should succeed
    let target_commit = repo.find_commit(base_id).unwrap();
    let result = perform_fast_cherry_pick(&repo, &cherry_commit, &target_commit);

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
    let _target_base = repo.find_commit(initial_id).unwrap().tree().unwrap();

    // Perform the fast cherry-pick: cherry-pick commit [258] onto the target base
    // This should ONLY include the Java file changes from [258]
    // NOT the Kotlin file changes from [392]
    let target_commit = repo.find_commit(initial_id).unwrap();
    let result = perform_fast_cherry_pick(&repo, &commit_258, &target_commit);

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
