use crate::git::conflict_formatter::render_conflict_diff;
use anyhow::{Result, anyhow};
use git2::{Commit, Index, Oid, Repository, Tree, TreeBuilder};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use tracing::field::Empty;
use tracing::{Span, debug, info, instrument};

// https://github.com/libgit2/libgit2/issues/6036

/// Options for fast cherry-pick operations
#[derive(Clone, Debug)]
pub struct CherryPickFastOptions {
  /// Detect if a commit is being applied onto a parent with the same tree,
  /// and skip applying the patch in that case.
  pub reuse_parent_tree_if_possible: bool,
}

impl Default for CherryPickFastOptions {
  fn default() -> Self {
    Self {
      reuse_parent_tree_if_possible: true,
    }
  }
}

/// Detailed conflict information for user display
#[derive(Debug, Clone)]
pub struct ConflictInfo {
  pub path: PathBuf,
  pub our_content: String,
  pub their_content: String,
  pub ancestor_content: Option<String>,
}

/// Error types for fast cherry-pick operations
#[derive(Debug)]
pub enum FastCherryPickError {
  MergeConflict {
    conflicting_paths: HashSet<PathBuf>,
    detailed_conflicts: Vec<ConflictInfo>,
  },
  GetPaths(anyhow::Error),
  DehydrateTree(anyhow::Error),
  HydrateTree(anyhow::Error),
  Git(git2::Error),
  Other(anyhow::Error),
}

impl std::fmt::Display for FastCherryPickError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      FastCherryPickError::MergeConflict {
        conflicting_paths,
        detailed_conflicts,
      } => {
        writeln!(f, "Cherry-pick resulted in conflicts in {} file(s):", conflicting_paths.len())?;
        for conflict in detailed_conflicts {
          writeln!(f, "\n📁 File: {}", conflict.path.display())?;
          write!(f, "{}", render_conflict_diff(conflict))?;
        }
        Ok(())
      }
      FastCherryPickError::GetPaths(err) => {
        write!(f, "could not get paths touched by commit: {err}")
      }
      FastCherryPickError::DehydrateTree(err) => {
        write!(f, "could not dehydrate tree: {err}")
      }
      FastCherryPickError::HydrateTree(err) => {
        write!(f, "could not hydrate tree: {err}")
      }
      FastCherryPickError::Git(err) => {
        write!(f, "git error: {err}")
      }
      FastCherryPickError::Other(err) => {
        write!(f, "other error: {err}")
      }
    }
  }
}

impl std::error::Error for FastCherryPickError {
  fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
    match self {
      FastCherryPickError::MergeConflict { .. } => None,
      FastCherryPickError::GetPaths(err) => Some(err.as_ref()),
      FastCherryPickError::DehydrateTree(err) => Some(err.as_ref()),
      FastCherryPickError::HydrateTree(err) => Some(err.as_ref()),
      FastCherryPickError::Git(err) => Some(err),
      FastCherryPickError::Other(err) => Some(err.as_ref()),
    }
  }
}

impl From<git2::Error> for FastCherryPickError {
  fn from(err: git2::Error) -> Self {
    FastCherryPickError::Git(err)
  }
}

impl From<anyhow::Error> for FastCherryPickError {
  fn from(err: anyhow::Error) -> Self {
    FastCherryPickError::Other(err)
  }
}

/// Get the file paths which were added, removed, or changed by the given commit.
pub fn get_paths_touched_by_commit(repo: &Repository, commit: &Commit) -> Result<HashSet<PathBuf>> {
  let current_tree = commit.tree()?;
  let parent_count = commit.parent_count();

  debug!("Analyzing commit {} with {} parent(s)", commit.id(), parent_count);

  let changed_paths = if parent_count == 0 {
    // For root commits, consider all files as changed
    debug!("Root commit detected, getting all paths in tree");
    get_all_paths_in_tree(repo, &current_tree)?
  } else if parent_count == 1 {
    // Optimize common case: single parent
    debug!("Single parent commit, comparing with parent");
    let parent_tree = commit.parent(0)?.tree()?;
    get_changed_paths_between_trees(repo, Some(&parent_tree), &current_tree)?
  } else {
    // Multiple parents (merge commits)
    debug!("Merge commit with {} parents, processing each parent", parent_count);
    let mut result = HashSet::with_capacity(parent_count * 32);

    // Process parents efficiently
    for i in 0..parent_count {
      let parent_commit = commit.parent(i)?;
      let parent_tree = parent_commit.tree()?;
      let changed_paths = get_changed_paths_between_trees(repo, Some(&parent_tree), &current_tree)?;
      debug!("Parent {}: found {} changed paths", i, changed_paths.len());
      result.extend(changed_paths);
    }
    debug!("Total unique changed paths across all parents: {}", result.len());
    result
  };
  Ok(changed_paths)
}

/// Get all file paths in a tree (for root commits)
fn get_all_paths_in_tree(repo: &Repository, tree: &Tree) -> Result<HashSet<PathBuf>> {
  // Pre-allocate capacity based on estimated number of files in a typical tree
  let mut paths = HashSet::with_capacity(tree.len() * 2);
  collect_tree_paths(repo, tree, &PathBuf::new(), &mut paths)?;
  Ok(paths)
}

/// Recursively collect all paths in a tree
fn collect_tree_paths(repo: &Repository, tree: &Tree, current_path: &Path, paths: &mut HashSet<PathBuf>) -> Result<()> {
  for entry in tree.iter() {
    let name = entry.name().ok_or_else(|| anyhow!("Invalid UTF-8 in tree entry name"))?;
    let entry_path = current_path.join(name);

    match entry.kind() {
      Some(git2::ObjectType::Tree) => {
        let subtree = repo.find_tree(entry.id())?;
        collect_tree_paths(repo, &subtree, &entry_path, paths)?;
      }
      _ => {
        paths.insert(entry_path);
      }
    }
  }
  Ok(())
}

/// Get changed paths between two trees
fn get_changed_paths_between_trees(repo: &Repository, old_tree: Option<&Tree>, new_tree: &Tree) -> Result<HashSet<PathBuf>> {
  let mut changed_paths = HashSet::with_capacity(32);

  let mut diff_options = git2::DiffOptions::new();
  diff_options
    .skip_binary_check(true)
    .disable_pathspec_match(true)
    .ignore_whitespace(false)
    .include_unmodified(false);

  let diff = repo.diff_tree_to_tree(old_tree, Some(new_tree), Some(&mut diff_options))?;
  diff.foreach(
    &mut |delta, _| {
      if let Some(old_path) = delta.old_file().path() {
        changed_paths.insert(old_path.to_path_buf());
      }
      if let Some(new_path) = delta.new_file().path() {
        changed_paths.insert(new_path.to_path_buf());
      }
      true
    },
    None,
    None,
    None,
  )?;

  Ok(changed_paths)
}

/// Create a "dehydrated" tree containing only the specified paths
pub fn dehydrate_tree(repo: &Repository, tree: &Tree, paths: &[&Path]) -> Result<Oid> {
  if paths.is_empty() {
    return Ok(repo.treebuilder(None)?.write()?);
  }

  let mut builder = repo.treebuilder(None)?;

  for path in paths {
    if let Ok(entry) = tree.get_path(path) {
      if path.parent().is_none() || path.parent() == Some(Path::new("")) {
        // Top-level entry
        let name = path.file_name().and_then(|n| n.to_str()).ok_or_else(|| anyhow!("Invalid path: {:?}", path))?;
        builder.insert(name, entry.id(), entry.filemode())?;
      } else {
        // Nested entry
        add_nested_entry(&mut builder, repo, tree, path)?;
      }
    }
  }

  Ok(builder.write()?)
}

/// Add a nested entry to the tree builder, creating intermediate directories as needed
fn add_nested_entry(builder: &mut TreeBuilder, repo: &Repository, original_tree: &Tree, path: &Path) -> Result<()> {
  let components: Vec<_> = path.components().collect();
  if components.is_empty() {
    return Ok(());
  }

  let first_component = components[0].as_os_str().to_str().ok_or_else(|| anyhow!("Invalid UTF-8 in path component"))?;

  if components.len() == 1 {
    // Leaf entry
    if let Ok(entry) = original_tree.get_path(path) {
      builder.insert(first_component, entry.id(), entry.filemode())?;
    }
  } else {
    // Need to create/update a subtree
    let subtree_path: PathBuf = components.iter().take(1).collect();

    if let Ok(entry) = original_tree.get_path(&subtree_path) {
      if entry.kind() == Some(git2::ObjectType::Tree) {
        let subtree = repo.find_tree(entry.id())?;
        let remaining_path: PathBuf = components.iter().skip(1).collect();

        let mut sub_builder = repo.treebuilder(Some(&subtree))?;
        add_nested_entry(&mut sub_builder, repo, &subtree, &remaining_path)?;
        let new_subtree_oid = sub_builder.write()?;

        builder.insert(first_component, new_subtree_oid, git2::FileMode::Tree.into())?;
      }
    }
  }

  Ok(())
}

/// Update a nested tree entry in the tree builder, creating intermediate directories as needed
fn update_nested_tree_entry(builder: &mut TreeBuilder, repo: &Repository, base_tree: Option<&Tree>, path: &Path, oid: Oid, filemode: i32) -> Result<()> {
  let components: Vec<_> = path.components().collect();
  if components.is_empty() {
    return Ok(());
  }

  let first_component = components[0].as_os_str().to_str().ok_or_else(|| anyhow!("Invalid UTF-8 in path component"))?;

  if components.len() == 1 {
    // Leaf entry - update it directly
    builder.insert(first_component, oid, filemode)?;
  } else {
    // Need to create/update a subtree
    let subtree_path: PathBuf = components.iter().take(1).collect();

    // Get existing subtree or create new one
    let existing_subtree = if let Some(base) = base_tree {
      base.get_path(&subtree_path).ok().and_then(|entry| {
        if entry.kind() == Some(git2::ObjectType::Tree) {
          repo.find_tree(entry.id()).ok()
        } else {
          None
        }
      })
    } else {
      None
    };

    let mut sub_builder = repo.treebuilder(existing_subtree.as_ref())?;
    let remaining_path: PathBuf = components.iter().skip(1).collect();
    update_nested_tree_entry(&mut sub_builder, repo, existing_subtree.as_ref(), &remaining_path, oid, filemode)?;
    let new_subtree_oid = sub_builder.write()?;

    builder.insert(first_component, new_subtree_oid, git2::FileMode::Tree.into())?;
  }

  Ok(())
}

/// Remove a nested tree entry from the tree builder
fn remove_nested_tree_entry(builder: &mut TreeBuilder, repo: &Repository, base_tree: Option<&Tree>, path: &Path) -> Result<()> {
  let components: Vec<_> = path.components().collect();
  if components.is_empty() {
    return Ok(());
  }

  let first_component = components[0].as_os_str().to_str().ok_or_else(|| anyhow!("Invalid UTF-8 in path component"))?;

  if components.len() == 1 {
    // Leaf entry - remove it directly
    builder.remove(first_component)?;
  } else {
    // Need to update a subtree
    let subtree_path: PathBuf = components.iter().take(1).collect();

    // Get existing subtree
    if let Some(base) = base_tree {
      if let Ok(entry) = base.get_path(&subtree_path) {
        if entry.kind() == Some(git2::ObjectType::Tree) {
          let subtree = repo.find_tree(entry.id())?;
          let mut sub_builder = repo.treebuilder(Some(&subtree))?;
          let remaining_path: PathBuf = components.iter().skip(1).collect();
          remove_nested_tree_entry(&mut sub_builder, repo, Some(&subtree), &remaining_path)?;
          let new_subtree_oid = sub_builder.write()?;

          builder.insert(first_component, new_subtree_oid, git2::FileMode::Tree.into())?;
        }
      }
    }
  }

  Ok(())
}

/// Create a "hydrated" tree by merging changed entries back into the base tree
pub fn hydrate_tree(repo: &Repository, base_tree: Option<&Tree>, changed_entries: HashMap<PathBuf, Option<(Oid, i32)>>) -> Result<Oid> {
  // Early return if no changes
  if changed_entries.is_empty() {
    return match base_tree {
      Some(tree) => Ok(tree.id()),
      None => Ok(repo.treebuilder(None)?.write()?),
    };
  }

  let mut builder = repo.treebuilder(base_tree)?;

  // Process entries while preserving directory structure
  for (path, entry) in changed_entries {
    match entry {
      Some((oid, filemode)) => {
        // For nested paths, we need to update the tree structure properly
        if path.parent().is_some() {
          update_nested_tree_entry(&mut builder, repo, base_tree, &path, oid, filemode)?;
        } else {
          // Top-level entry
          let name = path.file_name().and_then(|n| n.to_str()).ok_or_else(|| anyhow!("Invalid path: {:?}", path))?;
          builder.insert(name, oid, filemode)?;
        }
      }
      None => {
        // Remove the entry
        if path.parent().is_some() {
          remove_nested_tree_entry(&mut builder, repo, base_tree, &path)?;
        } else {
          let name = path.file_name().and_then(|n| n.to_str()).ok_or_else(|| anyhow!("Invalid path: {:?}", path))?;
          builder.remove(name)?;
        }
      }
    }
  }

  Ok(builder.write()?)
}

/// Fast cherry-pick implementation based on git-branchless approach
#[instrument(
  skip(repo, patch_commit, target_commit, options),
  fields(
    patch_commit_id = %patch_commit.id(),
    target_commit_id = %target_commit.id(),
    changed_paths = Empty,
    conflict = false,
  )
)]
pub fn cherry_pick_fast<'repo>(
  repo: &'repo Repository,
  patch_commit: &'repo Commit,
  target_commit: &'repo Commit,
  options: &CherryPickFastOptions,
) -> Result<Tree<'repo>, FastCherryPickError> {
  info!("Starting fast cherry-pick: patch_commit={}, target_commit={}", patch_commit.id(), target_commit.id());

  // Fast path: if trees are identical, reuse the patch tree
  if options.reuse_parent_tree_if_possible && patch_commit.parent_count() == 1 {
    let parent = patch_commit.parent(0)?;
    if parent.tree_id() == target_commit.tree_id() {
      info!("Fast path: reusing patch tree as parent tree matches target tree");
      return Ok(patch_commit.tree()?);
    }
  }

  // Get paths touched by the commit
  let changed_pathbufs: Vec<PathBuf> = get_paths_touched_by_commit(repo, patch_commit)
    .map_err(FastCherryPickError::GetPaths)?
    .into_iter()
    .collect();

  info!("Found {} changed paths in commit", changed_pathbufs.len());
  debug!("Changed paths: {:?}", changed_pathbufs);

  // Record the number of changed paths in the current span
  Span::current().record("changed_paths", changed_pathbufs.len());

  // Early return if no paths changed
  if changed_pathbufs.is_empty() {
    info!("No paths changed, returning target tree");
    return Ok(target_commit.tree()?);
  }

  let changed_paths: Vec<&Path> = changed_pathbufs.iter().map(|p| p.as_path()).collect();

  // Create dehydrated trees for the merge
  let patch_tree = patch_commit.tree()?;
  let target_tree = target_commit.tree()?;

  // Get the ancestor tree for three-way merge
  let ancestor_tree = if patch_commit.parent_count() > 0 {
    patch_commit.parent(0)?.tree()?
  } else {
    // For root commits, use an empty tree as ancestor
    let empty_tree_oid = repo.treebuilder(None)?.write()?;
    repo.find_tree(empty_tree_oid)?
  };

  // Create dehydrated trees containing only the changed paths
  debug!("Creating dehydrated trees for {} paths", changed_paths.len());
  let dehydrated_ancestor_oid = dehydrate_tree(repo, &ancestor_tree, &changed_paths).map_err(FastCherryPickError::DehydrateTree)?;
  let dehydrated_patch_oid = dehydrate_tree(repo, &patch_tree, &changed_paths).map_err(FastCherryPickError::DehydrateTree)?;
  let dehydrated_target_oid = dehydrate_tree(repo, &target_tree, &changed_paths).map_err(FastCherryPickError::DehydrateTree)?;

  debug!(
    "Dehydrated trees created: ancestor={}, patch={}, target={}",
    dehydrated_ancestor_oid, dehydrated_patch_oid, dehydrated_target_oid
  );

  let dehydrated_ancestor = repo.find_tree(dehydrated_ancestor_oid)?;
  let dehydrated_patch = repo.find_tree(dehydrated_patch_oid)?;
  let dehydrated_target = repo.find_tree(dehydrated_target_oid)?;

  // Perform three-way merge on the dehydrated trees
  let mut merge_options = git2::MergeOptions::new();
  merge_options.find_renames(false); // Disable rename detection for better performance
  let rebased_index = repo.merge_trees(&dehydrated_ancestor, &dehydrated_target, &dehydrated_patch, Some(&merge_options))?;

  if rebased_index.has_conflicts() {
    let conflicting_paths = get_conflicting_paths(&rebased_index)?;
    info!("Merge conflicts detected in {} files", conflicting_paths.len());

    // Record conflict in span
    Span::current().record("conflict", true);

    let detailed_conflicts = get_detailed_conflicts(repo, &rebased_index, &dehydrated_target, &dehydrated_patch)?;
    return Err(FastCherryPickError::MergeConflict {
      conflicting_paths,
      detailed_conflicts,
    });
  }

  // Convert index back to entries
  let rebased_entries = extract_entries_from_index(&rebased_index, &changed_pathbufs)?;

  // Hydrate back to full tree
  debug!("Hydrating tree with {} entries", rebased_entries.len());
  let rebased_tree_oid = hydrate_tree(repo, Some(&target_commit.tree()?), rebased_entries).map_err(FastCherryPickError::HydrateTree)?;

  info!("Cherry-pick completed successfully, result tree: {}", rebased_tree_oid);
  Ok(repo.find_tree(rebased_tree_oid)?)
}

/// Extract conflicting paths from index
fn get_conflicting_paths(index: &Index) -> Result<HashSet<PathBuf>, FastCherryPickError> {
  let mut conflicting_paths = HashSet::new();

  for conflict in index.conflicts()? {
    let conflict = conflict?;

    let entries = [conflict.ancestor, conflict.our, conflict.their];
    for entry in entries.into_iter().flatten() {
      if let Ok(path_str) = std::str::from_utf8(&entry.path) {
        conflicting_paths.insert(PathBuf::from(path_str));
      }
    }
  }

  Ok(conflicting_paths)
}

/// Get detailed conflict information with file contents for user display
fn get_detailed_conflicts(repo: &Repository, index: &Index, our_tree: &Tree, their_tree: &Tree) -> Result<Vec<ConflictInfo>, FastCherryPickError> {
  let mut detailed_conflicts = Vec::new();

  for conflict in index.conflicts()? {
    let conflict = conflict?;

    // Get the path from any available version
    let Some(path) = [&conflict.their, &conflict.our, &conflict.ancestor]
      .into_iter()
      .flatten()
      .find_map(|entry| std::str::from_utf8(&entry.path).ok())
      .map(PathBuf::from)
    else {
      continue; // Skip if no valid path
    };

    // Get file contents
    let our_content = get_file_content_from_tree(repo, our_tree, &path).unwrap_or_else(|_| "<file not found or binary>".to_string());
    let their_content = get_file_content_from_tree(repo, their_tree, &path).unwrap_or_else(|_| "<file not found or binary>".to_string());

    // For ancestor content, try to get it from the conflict entry
    let ancestor_content = if let Some(ancestor) = &conflict.ancestor {
      match repo.find_blob(ancestor.id) {
        Ok(blob) => match std::str::from_utf8(blob.content()) {
          Ok(content) => Some(content.to_string()),
          Err(_) => Some("<binary file>".to_string()),
        },
        Err(_) => None,
      }
    } else {
      None
    };

    detailed_conflicts.push(ConflictInfo {
      path,
      our_content,
      their_content,
      ancestor_content,
    });
  }

  Ok(detailed_conflicts)
}

/// Get file content from a tree
fn get_file_content_from_tree(repo: &Repository, tree: &Tree, path: &Path) -> Result<String> {
  match tree.get_path(path) {
    Ok(entry) => {
      let blob = repo.find_blob(entry.id())?;
      match std::str::from_utf8(blob.content()) {
        Ok(content) => Ok(content.to_string()),
        Err(_) => Ok("<binary file>".to_string()),
      }
    }
    Err(_) => Ok("<file deleted>".to_string()),
  }
}

/// Extract entries from index for hydration
fn extract_entries_from_index(index: &Index, changed_paths: &[PathBuf]) -> Result<HashMap<PathBuf, Option<(Oid, i32)>>, FastCherryPickError> {
  let mut entries = HashMap::with_capacity(changed_paths.len());

  for path in changed_paths {
    let entry = index.get_path(path, 0);
    let value = match entry {
      Some(index_entry) if !index_entry.id.is_zero() => Some((index_entry.id, index_entry.mode as i32)),
      _ => None,
    };
    entries.insert(path.clone(), value);
  }

  Ok(entries)
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::test_utils::git_test_utils::{create_commit, create_test_repo};

  #[test]
  fn test_fast_cherry_pick_no_conflicts() {
    let (_dir, repo) = create_test_repo();

    // Create initial commit
    let initial_id = create_commit(&repo, "Initial commit", "base.txt", "base content\n");
    let initial_commit = repo.find_commit(initial_id).unwrap();

    // Create target commit (adds a file)
    let target_id = create_commit(&repo, "Target changes", "target.txt", "target content\n");
    let target_commit = repo.find_commit(target_id).unwrap();

    // Reset to initial and create patch commit (adds different file)
    repo.reset(initial_commit.as_object(), git2::ResetType::Hard, None).unwrap();
    let patch_id = create_commit(&repo, "Patch changes", "patch.txt", "patch content\n");
    let patch_commit = repo.find_commit(patch_id).unwrap();

    // Perform fast cherry-pick
    let options = CherryPickFastOptions::default();
    let result = cherry_pick_fast(&repo, &patch_commit, &target_commit, &options);

    assert!(result.is_ok());
    let tree = result.unwrap();

    // Verify both files exist in result
    assert!(tree.get_path(Path::new("target.txt")).is_ok());
    assert!(tree.get_path(Path::new("patch.txt")).is_ok());
  }

  #[test]
  fn test_fast_cherry_pick_with_conflicts() {
    let (_dir, repo) = create_test_repo();

    // Create initial commit
    let initial_id = create_commit(&repo, "Initial commit", "file.txt", "line1\nline2\n");
    let initial_commit = repo.find_commit(initial_id).unwrap();

    // Create target commit (modifies file)
    let target_id = create_commit(&repo, "Target changes", "file.txt", "line1\ntarget_line2\n");
    let target_commit = repo.find_commit(target_id).unwrap();

    // Reset to initial and create conflicting patch commit
    repo.reset(initial_commit.as_object(), git2::ResetType::Hard, None).unwrap();
    let patch_id = create_commit(&repo, "Patch changes", "file.txt", "line1\npatch_line2\n");
    let patch_commit = repo.find_commit(patch_id).unwrap();

    // Perform fast cherry-pick
    let options = CherryPickFastOptions::default();
    let result = cherry_pick_fast(&repo, &patch_commit, &target_commit, &options);

    // Should detect conflicts
    assert!(result.is_err());
    match result.unwrap_err() {
      FastCherryPickError::MergeConflict {
        conflicting_paths,
        detailed_conflicts,
      } => {
        assert!(conflicting_paths.contains(&PathBuf::from("file.txt")));
        assert_eq!(detailed_conflicts.len(), 1);
        assert_eq!(detailed_conflicts[0].path, PathBuf::from("file.txt"));
      }
      other => panic!("Expected MergeConflict, got: {other:?}"),
    }
  }

  #[test]
  fn test_reuse_parent_tree_optimization() {
    let (_dir, repo) = create_test_repo();

    // Create initial commit
    let initial_id = create_commit(&repo, "Initial commit", "file.txt", "content\n");
    let initial_commit = repo.find_commit(initial_id).unwrap();

    // Create patch commit based on initial
    let patch_id = create_commit(&repo, "Patch changes", "file.txt", "modified content\n");
    let patch_commit = repo.find_commit(patch_id).unwrap();

    // Apply patch to the same base (should trigger optimization)
    let options = CherryPickFastOptions {
      reuse_parent_tree_if_possible: true,
    };
    let result = cherry_pick_fast(&repo, &patch_commit, &initial_commit, &options);

    assert!(result.is_ok());
    let result_tree = result.unwrap();

    // Should reuse the patch commit's tree directly
    assert_eq!(result_tree.id(), patch_commit.tree_id());
  }

  #[test]
  fn test_nested_path_preserved_during_cherry_pick() {
    let (_dir, repo) = create_test_repo();

    // This test verifies that nested paths are preserved correctly during cherry-pick
    // It tests the scenario where both branches have the file (no add/delete conflict)

    // Create initial commit with the nested file structure
    let nested_path = "community/platform/platform-impl/src/com/intellij/ui/EditorNotificationsImpl.kt";
    std::fs::create_dir_all(repo.workdir().unwrap().join("community/platform/platform-impl/src/com/intellij/ui")).unwrap();
    std::fs::write(repo.workdir().unwrap().join(nested_path), "class EditorNotificationsImpl {\n  // original\n}\n").unwrap();

    let mut index = repo.index().unwrap();
    index.add_path(std::path::Path::new(nested_path)).unwrap();
    index.write().unwrap();

    let tree_oid = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_oid).unwrap();
    let sig = repo.signature().unwrap();
    let initial_id = repo.commit(None, &sig, &sig, "Initial with EditorNotificationsImpl", &tree, &[]).unwrap();
    let initial_commit = repo.find_commit(initial_id).unwrap();

    // Create a branch that modifies the file
    std::fs::write(
      repo.workdir().unwrap().join(nested_path),
      "class EditorNotificationsImpl {\n  // modified in patch\n  fun handleNotification() {}\n}\n",
    )
    .unwrap();
    index.add_path(std::path::Path::new(nested_path)).unwrap();
    index.write().unwrap();

    let tree_oid = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_oid).unwrap();
    let patch_id = repo
      .commit(
        Some("HEAD"),
        &sig,
        &sig,
        "IJPL-156594 adapter.createInstance in EditorNotificationsImpl can throw AlreadyDisposedException",
        &tree,
        &[&initial_commit],
      )
      .unwrap();
    let patch_commit = repo.find_commit(patch_id).unwrap();

    // Create another branch from initial with different changes to the same file
    repo.reset(initial_commit.as_object(), git2::ResetType::Hard, None).unwrap();
    std::fs::write(repo.workdir().unwrap().join(nested_path), "class EditorNotificationsImpl {\n  // modified in target\n}\n").unwrap();
    std::fs::write(repo.workdir().unwrap().join("other.txt"), "other content\n").unwrap();
    index.add_path(std::path::Path::new(nested_path)).unwrap();
    index.add_path(std::path::Path::new("other.txt")).unwrap();
    index.write().unwrap();

    let tree_oid = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_oid).unwrap();
    let target_id = repo.commit(Some("HEAD"), &sig, &sig, "Target changes", &tree, &[&initial_commit]).unwrap();
    let target_commit = repo.find_commit(target_id).unwrap();

    // Cherry-pick should create a conflict but paths should be preserved
    let options = CherryPickFastOptions::default();
    let result = cherry_pick_fast(&repo, &patch_commit, &target_commit, &options);

    // We expect a conflict here
    assert!(result.is_err(), "Should have a merge conflict");

    if let Err(FastCherryPickError::MergeConflict { conflicting_paths, .. }) = result {
      // Verify the conflict is at the correct nested path
      assert!(conflicting_paths.contains(&PathBuf::from(nested_path)), "Conflict should be at nested path: {nested_path}");

      // Verify there's no spurious conflict at root level
      assert!(
        !conflicting_paths.contains(&PathBuf::from("EditorNotificationsImpl.kt")),
        "Should NOT have conflict at root level"
      );
    } else {
      panic!("Expected MergeConflict error");
    }
  }

  #[test]
  fn test_nested_path_addition_during_cherry_pick() {
    let (_dir, repo) = create_test_repo();

    // This test verifies that when adding a new nested file via cherry-pick,
    // it's created at the correct location

    // Create initial commit
    let initial_id = create_commit(&repo, "Initial commit", "README.md", "# Project\n");
    let initial_commit = repo.find_commit(initial_id).unwrap();

    // Create a commit that adds a nested file
    let nested_path = "community/platform/platform-impl/src/com/intellij/ui/EditorNotificationsImpl.kt";
    std::fs::create_dir_all(repo.workdir().unwrap().join("community/platform/platform-impl/src/com/intellij/ui")).unwrap();
    std::fs::write(repo.workdir().unwrap().join(nested_path), "class EditorNotificationsImpl {\n  // new file\n}\n").unwrap();

    let mut index = repo.index().unwrap();
    index.add_path(std::path::Path::new(nested_path)).unwrap();
    index.write().unwrap();

    let tree_oid = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_oid).unwrap();
    let sig = repo.signature().unwrap();
    let patch_id = repo.commit(Some("HEAD"), &sig, &sig, "Add EditorNotificationsImpl", &tree, &[&initial_commit]).unwrap();
    let patch_commit = repo.find_commit(patch_id).unwrap();

    // Create a different branch from initial
    repo.reset(initial_commit.as_object(), git2::ResetType::Hard, None).unwrap();
    let target_id = create_commit(&repo, "Other changes", "other.txt", "other content\n");
    let target_commit = repo.find_commit(target_id).unwrap();

    // Cherry-pick the addition
    let options = CherryPickFastOptions::default();
    let result = cherry_pick_fast(&repo, &patch_commit, &target_commit, &options);

    assert!(result.is_ok(), "Cherry-pick of new file should succeed");
    let result_tree = result.unwrap();

    // Verify the file exists at the CORRECT nested path
    let nested_entry = result_tree.get_path(std::path::Path::new(nested_path));
    assert!(nested_entry.is_ok(), "File should exist at nested path: {nested_path}");

    // Verify the file does NOT exist at the root level
    let root_entry = result_tree.get_name("EditorNotificationsImpl.kt");
    assert!(root_entry.is_none(), "File should NOT exist at root level");

    // Verify the content is correct
    let entry = nested_entry.unwrap();
    let blob = repo.find_blob(entry.id()).unwrap();
    let content = std::str::from_utf8(blob.content()).unwrap();
    assert!(content.contains("// new file"), "File should have correct content");
  }
}
