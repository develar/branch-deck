use crate::git::conflict_formatter::render_conflict_diff;
use anyhow::{Result, anyhow};
use git2::{Commit, Index, Oid, Repository, Signature, Tree, TreeBuilder};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

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

  let changed_paths = if parent_count == 0 {
    // For root commits, consider all files as changed
    get_all_paths_in_tree(repo, &current_tree)?
  } else {
    // Pre-allocate capacity based on typical number of changed files
    let mut result = HashSet::with_capacity(parent_count * 32);

    // Process parents efficiently
    for i in 0..parent_count {
      let parent_commit = commit.parent(i)?;
      let parent_tree = parent_commit.tree()?;
      let changed_paths = get_changed_paths_between_trees(repo, Some(&parent_tree), &current_tree)?;
      result.extend(changed_paths);
    }
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
  // Pre-allocate capacity based on typical git diff output
  let mut changed_paths = HashSet::with_capacity(32);

  let diff = repo.diff_tree_to_tree(old_tree, Some(new_tree), None)?;
  diff.foreach(
    &mut |delta, _| {
      // Optimize: check both paths in a single match to avoid duplicate checks
      match (delta.old_file().path(), delta.new_file().path()) {
        (Some(old_path), Some(new_path)) if old_path == new_path => {
          // Same path, only add once
          changed_paths.insert(old_path.to_path_buf());
        }
        (Some(old_path), Some(new_path)) => {
          // Different paths (rename), add both
          changed_paths.insert(old_path.to_path_buf());
          changed_paths.insert(new_path.to_path_buf());
        }
        (Some(path), None) | (None, Some(path)) => {
          // Add/delete, single path
          changed_paths.insert(path.to_path_buf());
        }
        (None, None) => {}
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
  let mut builder = repo.treebuilder(None)?;

  for path in paths {
    if let Ok(entry) = tree.get_path(path) {
      let name = path.file_name().and_then(|n| n.to_str()).ok_or_else(|| anyhow!("Invalid path: {:?}", path))?;

      // For directories, we need to create sub-trees
      if path.parent().is_some() && path.parent() != Some(Path::new("")) {
        // This is a nested path - we need to handle directory structure
        add_nested_entry(&mut builder, repo, tree, path)?;
      } else {
        // Top-level entry
        builder.insert(name, entry.id(), entry.filemode())?;
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
        add_nested_entry_to_builder(&mut sub_builder, repo, &subtree, &remaining_path)?;
        let new_subtree_oid = sub_builder.write()?;

        builder.insert(first_component, new_subtree_oid, git2::FileMode::Tree.into())?;
      }
    }
  }

  Ok(())
}

/// Helper function to add nested entries to a tree builder
fn add_nested_entry_to_builder(builder: &mut TreeBuilder, _repo: &Repository, tree: &Tree, path: &Path) -> Result<()> {
  if let Ok(entry) = tree.get_path(path) {
    let name = path.file_name().and_then(|n| n.to_str()).ok_or_else(|| anyhow!("Invalid path: {:?}", path))?;
    builder.insert(name, entry.id(), entry.filemode())?;
  }
  Ok(())
}

/// Create a "hydrated" tree by merging changed entries back into the base tree
pub fn hydrate_tree(repo: &Repository, base_tree: Option<&Tree>, changed_entries: HashMap<PathBuf, Option<(Oid, i32)>>) -> Result<Oid> {
  let mut builder = repo.treebuilder(base_tree)?;

  for (path, entry) in changed_entries {
    let name = path.file_name().and_then(|n| n.to_str()).ok_or_else(|| anyhow!("Invalid path: {:?}", path))?;

    match entry {
      Some((oid, filemode)) => {
        builder.insert(name, oid, filemode)?;
      }
      None => {
        // Remove the entry
        builder.remove(name)?;
      }
    }
  }

  Ok(builder.write()?)
}

/// Fast cherry-pick implementation based on git-branchless approach
pub fn cherry_pick_fast<'repo>(
  repo: &'repo Repository,
  patch_commit: &'repo Commit,
  target_commit: &'repo Commit,
  options: &CherryPickFastOptions,
) -> Result<Tree<'repo>, FastCherryPickError> {
  // Fast path: if trees are identical, reuse the patch tree
  if options.reuse_parent_tree_if_possible && patch_commit.parent_count() == 1 {
    let parent = patch_commit.parent(0)?;
    if parent.tree_id() == target_commit.tree_id() {
      return Ok(patch_commit.tree()?);
    }
  }

  // Get paths touched by the commit
  let changed_pathbufs: Vec<PathBuf> = get_paths_touched_by_commit(repo, patch_commit)
    .map_err(FastCherryPickError::GetPaths)?
    .into_iter()
    .collect();
  let changed_paths: Vec<&Path> = changed_pathbufs.iter().map(|p| p.as_path()).collect();
  let dehydrated_patch_commit = create_dehydrated_commit(repo, patch_commit, &changed_paths, true)?;
  let dehydrated_target_commit = create_dehydrated_commit(repo, target_commit, &changed_paths, false)?;

  // Perform cherry-pick on the smaller trees
  let rebased_index = repo.cherrypick_commit(&dehydrated_patch_commit, &dehydrated_target_commit, 0, None)?;

  if rebased_index.has_conflicts() {
    let conflicting_paths = get_conflicting_paths(&rebased_index)?;
    let detailed_conflicts = get_detailed_conflicts(repo, &rebased_index, &dehydrated_target_commit.tree()?, &dehydrated_patch_commit.tree()?)?;
    return Err(FastCherryPickError::MergeConflict {
      conflicting_paths,
      detailed_conflicts,
    });
  }

  // Convert index back to entries
  let rebased_entries = extract_entries_from_index(&rebased_index, &changed_pathbufs)?;

  // Hydrate back to full tree
  let rebased_tree_oid = hydrate_tree(repo, Some(&target_commit.tree()?), rebased_entries).map_err(FastCherryPickError::HydrateTree)?;

  Ok(repo.find_tree(rebased_tree_oid)?)
}

/// Create a dehydrated commit containing only the specified paths
fn create_dehydrated_commit<'repo>(repo: &'repo Repository, commit: &Commit, changed_paths: &[&Path], base_on_parent: bool) -> Result<Commit<'repo>, FastCherryPickError> {
  let tree = commit.tree()?;
  let dehydrated_tree_oid = dehydrate_tree(repo, &tree, changed_paths).map_err(FastCherryPickError::DehydrateTree)?;
  let dehydrated_tree = repo.find_tree(dehydrated_tree_oid)?;

  let signature = Signature::now("git-branchless", "git-branchless@example.com")?;
  let message = format!("generated by branch-deck: temporary dehydrated commit\n\nOriginal commit: {}", commit.id());

  let parents = if base_on_parent && commit.parent_count() > 0 {
    let parent = commit.parent(0)?;
    let dehydrated_parent = create_dehydrated_commit(repo, &parent, changed_paths, false)?;
    vec![dehydrated_parent]
  } else {
    vec![]
  };

  let parent_refs: Vec<&Commit> = parents.iter().collect();
  let dehydrated_commit_oid = repo.commit(None, &signature, &signature, &message, &dehydrated_tree, &parent_refs)?;

  Ok(repo.find_commit(dehydrated_commit_oid)?)
}

/// Extract conflicting paths from index
fn get_conflicting_paths(index: &Index) -> Result<HashSet<PathBuf>, FastCherryPickError> {
  let mut conflicting_paths = HashSet::new();

  for conflict in index.conflicts()? {
    let conflict = conflict?;

    if let Some(ancestor) = conflict.ancestor {
      if let Ok(path_str) = std::str::from_utf8(&ancestor.path) {
        conflicting_paths.insert(PathBuf::from(path_str));
      }
    }
    if let Some(our) = conflict.our {
      if let Ok(path_str) = std::str::from_utf8(&our.path) {
        conflicting_paths.insert(PathBuf::from(path_str));
      }
    }
    if let Some(their) = conflict.their {
      if let Ok(path_str) = std::str::from_utf8(&their.path) {
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
    let path = if let Some(their) = &conflict.their {
      PathBuf::from(std::str::from_utf8(&their.path).unwrap_or("<invalid UTF-8>"))
    } else if let Some(our) = &conflict.our {
      PathBuf::from(std::str::from_utf8(&our.path).unwrap_or("<invalid UTF-8>"))
    } else if let Some(ancestor) = &conflict.ancestor {
      PathBuf::from(std::str::from_utf8(&ancestor.path).unwrap_or("<invalid UTF-8>"))
    } else {
      continue; // Skip if no path information
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
  let mut entries = HashMap::new();

  for path in changed_paths {
    let entry = index.get_path(path, 0);
    match entry {
      Some(index_entry) => {
        if index_entry.id.is_zero() {
          entries.insert(path.clone(), None);
        } else {
          entries.insert(path.clone(), Some((index_entry.id, index_entry.mode as i32)));
        }
      }
      None => {
        entries.insert(path.clone(), None);
      }
    }
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
}
