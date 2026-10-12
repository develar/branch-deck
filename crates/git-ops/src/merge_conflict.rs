use crate::commit_list::Commit;
use crate::commit_utils::get_commits_info;
use crate::conflict_analysis::{ConflictAnalysis, FileDiff, FileInfo, PATCH_ARGS, analyze_conflict, find_merge_base, get_files_content_at_commit, split_diff_by_file};
use crate::copy_commit::CopyCommitError;
use crate::model::{ConflictDetail, ConflictMarkerCommitInfo, MergeConflictInfo};
use anyhow::anyhow;
use git_executor::git_command_executor::GitCommandExecutor;
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::{debug, instrument};

/// The commits and trees of a conflicted merge, as needed to describe the conflict to the user
#[derive(Clone, Copy)]
pub struct ConflictContext<'a> {
  pub git_executor: &'a GitCommandExecutor,
  pub repo_path: &'a str,
  /// Conflicted file paths, without duplicates
  pub conflict_files: &'a [PathBuf],
  /// Tree-ish holding the conflicted files with conflict markers
  pub merge_tree_oid: &'a str,
  /// Original parent of the commit being applied
  pub parent_commit_id: &'a str,
  /// Commit the change is applied onto (ours)
  pub target_commit_id: &'a str,
  /// Commit being applied (theirs)
  pub cherry_commit_id: &'a str,
}

/// Build the full conflict report: commit metadata, per-file contents and diffs, and the conflict analysis.
/// Runs a constant number of git processes regardless of the number of conflicted files.
#[instrument(skip_all, fields(conflict_files = ctx.conflict_files.len()))]
pub fn build_merge_conflict_info(ctx: ConflictContext<'_>) -> Result<MergeConflictInfo, CopyCommitError> {
  let ConflictContext {
    git_executor,
    repo_path,
    conflict_files,
    parent_commit_id,
    target_commit_id,
    cherry_commit_id,
    ..
  } = ctx;

  // The merge base of parent and target shows the actual divergence point
  let found_merge_base = find_merge_base(git_executor, repo_path, parent_commit_id, target_commit_id)
    .inspect_err(|e| debug!(error = %e, "merge-base failed, falling back to the parent commit"))
    .ok();
  let merge_base_id = found_merge_base.as_deref().unwrap_or(parent_commit_id);

  let commits = get_commits_info(git_executor, repo_path, &[parent_commit_id, target_commit_id, cherry_commit_id, merge_base_id])
    .map_err(|e| CopyCommitError::Other(anyhow!("Failed to get commit info for conflict: {e}")))?;
  let cherry = find_commit(&commits, cherry_commit_id)?;
  let parent = find_commit(&commits, parent_commit_id)?;
  let target = find_commit(&commits, target_commit_id)?;
  let merge_base = find_commit(&commits, merge_base_id)?;

  let conflicting_files = extract_conflict_details(&ctx, merge_base_id)?;

  let conflict_analysis = if found_merge_base.is_some() {
    analyze_conflict(git_executor, repo_path, parent_commit_id, target_commit_id, merge_base, conflict_files).unwrap_or_else(|e| {
      debug!(error = %e, "failed to analyze conflict");
      ConflictAnalysis::default()
    })
  } else {
    ConflictAnalysis::default()
  };

  Ok(MergeConflictInfo {
    commit_message: cherry.subject.clone(),
    commit_hash: cherry.id.clone(),
    commit_author_time: cherry.author_timestamp,
    commit_committer_time: cherry.committer_timestamp,
    original_parent_message: parent.subject.clone(),
    original_parent_hash: parent.id.clone(),
    original_parent_author_time: parent.author_timestamp,
    original_parent_committer_time: parent.committer_timestamp,
    target_branch_message: target.subject.clone(),
    target_branch_hash: target.id.clone(),
    target_branch_author_time: target.author_timestamp,
    target_branch_committer_time: target.committer_timestamp,
    conflicting_files,
    conflict_analysis,
    conflict_marker_commits: commits.iter().map(|(id, commit)| (id.clone(), ConflictMarkerCommitInfo::from(commit))).collect(),
  })
}

/// Look up a commit by full hash, or by the abbreviated id the caller passed
fn find_commit<'a>(commits: &'a HashMap<String, Commit>, id: &str) -> Result<&'a Commit, CopyCommitError> {
  commits
    .get(id)
    .or_else(|| commits.values().find(|commit| commit.id.starts_with(id)))
    .ok_or_else(|| CopyCommitError::Other(anyhow!("Commit {id} not found")))
}

/// Per-file patches between two tree-ish objects, keyed by path. Paths absent on one side come out as `/dev/null` diffs.
fn diff_by_file(git_executor: &GitCommandExecutor, repo_path: &str, from: &str, to: &str, file_paths: &[String]) -> Result<HashMap<String, String>, CopyCommitError> {
  if from == to {
    return Ok(HashMap::new());
  }
  let mut args = vec!["-c", "core.quotePath=false", "--no-pager", "diff-tree"];
  args.extend(PATCH_ARGS);
  args.extend(["--end-of-options", from, to, "--"]);
  args.extend(file_paths.iter().map(String::as_str));
  let output = git_executor
    .execute_command(&args, repo_path)
    .map_err(|e| CopyCommitError::Other(anyhow!("git diff-tree failed: {e}")))?;
  Ok(split_diff_by_file(&output).into_iter().collect())
}

/// Content of the conflicted files at `rev`; on failure every file is empty, as if absent
fn contents_or_empty(git_executor: &GitCommandExecutor, repo_path: &str, rev: &str, file_paths: &[String]) -> HashMap<String, String> {
  get_files_content_at_commit(git_executor, repo_path, rev, file_paths).unwrap_or_else(|e| {
    debug!(error = %e, rev, "failed to read file contents, using empty content");
    HashMap::new()
  })
}

/// Parse one file's patch into hunks, each prefixed with the file header expected by git-diff-view.
/// Only the lines before the first hunk are metadata; inside a hunk every line is content.
fn parse_diff_hunks(file_patch: &str, file_path: &str) -> Vec<String> {
  let mut hunks = Vec::new();
  let mut old_side = format!("a/{file_path}");
  let mut new_side = format!("b/{file_path}");
  let mut current_hunk: Option<String> = None;

  for line in file_patch.lines() {
    if line.starts_with("@@") {
      if let Some(hunk) = current_hunk.take() {
        hunks.push(hunk);
      }
      current_hunk = Some(format!("--- {old_side}\n+++ {new_side}\n{line}"));
    } else if let Some(hunk) = current_hunk.as_mut() {
      hunk.push('\n');
      hunk.push_str(line);
    } else if line.starts_with("new file mode") {
      "/dev/null".clone_into(&mut old_side);
    } else if line.starts_with("deleted file mode") {
      "/dev/null".clone_into(&mut new_side);
    }
  }
  hunks.extend(current_hunk);
  hunks
}

/// A single hunk showing `content` unchanged; empty content has no hunks
fn context_hunks(file_path: &str, content: &str) -> Vec<String> {
  let line_count = content.lines().count();
  if line_count == 0 {
    return vec![];
  }
  let mut hunk = format!("--- a/{file_path}\n+++ b/{file_path}\n@@ -1,{line_count} +1,{line_count} @@");
  for line in content.lines() {
    hunk.push_str("\n ");
    hunk.push_str(line);
  }
  vec![hunk]
}

/// A single hunk replacing all of `from` with all of `to`, for changes git reports without hunks (e.g. binary files)
fn replacement_hunk(file_path: &str, from: &str, to: &str) -> String {
  let mut hunk = format!("--- a/{file_path}\n+++ b/{file_path}\n@@ -1,{} +1,{} @@", from.lines().count(), to.lines().count());
  for (prefix, content) in [("\n-", from), ("\n+", to)] {
    for line in content.lines() {
      hunk.push_str(prefix);
      hunk.push_str(line);
    }
  }
  hunk
}

/// Diff of one file between two versions for the 3-way view. Identical versions show the whole file as context.
fn version_diff(file_path: &str, file_lang: &str, from: String, to: String, file_patch: Option<&String>) -> FileDiff {
  let hunks = if from == to {
    context_hunks(file_path, &from)
  } else {
    let hunks = file_patch.map_or_else(Vec::new, |patch| parse_diff_hunks(patch, file_path));
    if hunks.is_empty() { vec![replacement_hunk(file_path, &from, &to)] } else { hunks }
  };
  FileDiff {
    old_file: file_info(file_path, file_lang, from),
    new_file: file_info(file_path, file_lang, to),
    hunks,
  }
}

fn file_info(file_path: &str, file_lang: &str, content: String) -> FileInfo {
  FileInfo {
    file_name: file_path.to_string(),
    file_lang: file_lang.to_string(),
    content,
  }
}

/// Per-file conflict details: the merged content with conflict markers and the three versions with their diffs.
/// Reads contents and diffs with a constant number of batched git calls.
#[instrument(skip_all, fields(conflict_files = ctx.conflict_files.len()))]
pub(crate) fn extract_conflict_details(ctx: &ConflictContext<'_>, merge_base_id: &str) -> Result<Vec<ConflictDetail>, CopyCommitError> {
  let ConflictContext {
    git_executor,
    repo_path,
    merge_tree_oid,
    target_commit_id,
    cherry_commit_id,
    ..
  } = *ctx;
  let file_paths: Vec<String> = ctx.conflict_files.iter().map(|p| p.display().to_string()).collect();

  // In the amend case target, cherry and merge tree are the same commit: read it once
  let base_contents = contents_or_empty(git_executor, repo_path, merge_base_id, &file_paths);
  let target_contents = contents_or_empty(git_executor, repo_path, target_commit_id, &file_paths);
  let cherry_contents = (cherry_commit_id != target_commit_id).then(|| contents_or_empty(git_executor, repo_path, cherry_commit_id, &file_paths));
  // If the conflict is detected, the merged content must exist: do not hide errors
  let conflict_contents = if merge_tree_oid == target_commit_id {
    None
  } else {
    Some(
      get_files_content_at_commit(git_executor, repo_path, merge_tree_oid, &file_paths)
        .map_err(|e| CopyCommitError::Other(anyhow!("Failed to get file content from merge tree: {e}")))?,
    )
  };
  let cherry_contents = cherry_contents.as_ref().unwrap_or(&target_contents);
  let conflict_contents = conflict_contents.as_ref().unwrap_or(&target_contents);

  let conflict_patches = diff_by_file(git_executor, repo_path, target_commit_id, merge_tree_oid, &file_paths)?;
  let base_to_target_patches = diff_by_file(git_executor, repo_path, merge_base_id, target_commit_id, &file_paths)?;
  let base_to_cherry_patches = if cherry_commit_id == target_commit_id {
    None
  } else {
    Some(diff_by_file(git_executor, repo_path, merge_base_id, cherry_commit_id, &file_paths)?)
  };
  let base_to_cherry_patches = base_to_cherry_patches.as_ref().unwrap_or(&base_to_target_patches);

  let content_of = |contents: &HashMap<String, String>, file_path: &str| contents.get(file_path).cloned().unwrap_or_default();

  let mut conflict_details = Vec::with_capacity(file_paths.len());
  for file_path in file_paths {
    let file_lang = file_path.split('.').next_back().unwrap_or("txt").to_string();

    // Conflict view: empty "before", so the merged content with conflict markers shows as additions to the target version
    let conflict_content = content_of(conflict_contents, &file_path);
    let conflict_hunks = if conflict_content.is_empty() {
      vec![]
    } else {
      conflict_patches.get(&file_path).map_or_else(Vec::new, |patch| parse_diff_hunks(patch, &file_path))
    };
    let file_diff = FileDiff {
      old_file: file_info(&file_path, &file_lang, String::new()),
      new_file: file_info(&file_path, &file_lang, conflict_content),
      hunks: conflict_hunks,
    };

    // 3-way merge view
    let base_content = content_of(&base_contents, &file_path);
    let target_content = content_of(&target_contents, &file_path);
    let cherry_content = content_of(cherry_contents, &file_path);
    let base_to_target_diff = version_diff(&file_path, &file_lang, base_content.clone(), target_content.clone(), base_to_target_patches.get(&file_path));
    let base_to_cherry_diff = version_diff(&file_path, &file_lang, base_content.clone(), cherry_content.clone(), base_to_cherry_patches.get(&file_path));

    conflict_details.push(ConflictDetail {
      status: "modified".to_string(),
      file_diff,
      base_file: Some(file_info(&file_path, &file_lang, base_content)),
      target_file: Some(file_info(&file_path, &file_lang, target_content)),
      cherry_file: Some(file_info(&file_path, &file_lang, cherry_content)),
      base_to_target_diff,
      base_to_cherry_diff,
      file: file_path,
    });
  }

  Ok(conflict_details)
}
